//! CSR_V2 请求帧与响应校验 [CSR_V2 framing and response checks]
//!
//! 移植自 Duck-ToolBox 的 `duck-rkp-device::protocol`（MIT），仅保留 V2 路径。
//! Ported from Duck-ToolBox `duck-rkp-device::protocol` (MIT); V2 only.
//!
//! 本地只校验结构与绑定关系（challenge、keysToSign）。DICE 链与 COSE 签名的
//! 密码学校验由 Google RKP 服务端完成，非法 CSR 不会签发证书。
//! Locally only structure and binding are checked. DICE and COSE signature
//! verification is performed by the RKP server; an invalid CSR is never signed.

use crate::error::{Result, TfError};
use ciborium::value::{Integer, Value};
use p256::PublicKey;
use std::io::Cursor;

/// TA 写入固定共享缓冲区的命令号 [Fixed shared-buffer command id].
pub(crate) const CSR_COMMAND: u32 = 0x3122;
/// TA 响应缓冲区大小 [TA response buffer size].
pub(crate) const BUFFER_SIZE: usize = 0xa000;
/// 超过该大小的请求在进入厂商代码前拒绝 [Requests above this never reach vendor code].
pub(crate) const MAX_REQUEST_SIZE: usize = 0x9000;

const MAX_CBOR_BYTES: usize = 256 * 1024;
const MAX_DEPTH: usize = 16;
const MAX_CONTAINER_ITEMS: u64 = 64;
const MAX_CHALLENGE_LEN: usize = 64;
/// AuthenticatedRequest 版本与 CSR payload 版本 [Versions from the RKP CDDL].
const AUTHENTICATED_REQUEST_VERSION: i128 = 1;
const CSR_PAYLOAD_VERSION: i128 = 3;

#[derive(Debug)]
pub(crate) struct CsrInfo {
    pub(crate) dice_entries: usize,
    pub(crate) version: i128,
}

fn fail(message: impl Into<String>) -> TfError {
    TfError::new(format!(
        "RKP CSR 无效 [invalid RKP CSR]: {}",
        message.into()
    ))
}

fn cb(value: i128) -> Value {
    if value >= 0 {
        Value::Integer(Integer::from(value as u64))
    } else {
        Value::Integer(Integer::from(value as i64))
    }
}

pub(crate) fn encode(value: &Value) -> Result<Vec<u8>> {
    let value = canonicalize(value)?;
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&value, &mut bytes)
        .map_err(|error| fail(format!("CBOR 编码失败 [encode]: {error}")))?;
    Ok(bytes)
}

fn encode_unordered(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(value, &mut bytes)
        .map_err(|error| fail(format!("CBOR 编码失败 [encode]: {error}")))?;
    Ok(bytes)
}

/// 规范化映射顺序，避免服务端因键序差异拒绝请求。
/// Canonical map ordering keeps the request byte-stable for the server.
fn canonicalize(value: &Value) -> Result<Value> {
    match value {
        Value::Array(items) => Ok(Value::Array(
            items.iter().map(canonicalize).collect::<Result<Vec<_>>>()?,
        )),
        Value::Map(entries) => {
            let mut sorted = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                let key = canonicalize(key)?;
                let value = canonicalize(value)?;
                sorted.push((encode_unordered(&key)?, key, value));
            }
            sorted.sort_by(|left, right| {
                left.0
                    .len()
                    .cmp(&right.0.len())
                    .then_with(|| left.0.cmp(&right.0))
            });
            Ok(Value::Map(
                sorted
                    .into_iter()
                    .map(|(_, key, value)| (key, value))
                    .collect(),
            ))
        }
        Value::Tag(tag, inner) => Ok(Value::Tag(*tag, Box::new(canonicalize(inner)?))),
        other => Ok(other.clone()),
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Value> {
    preflight(bytes)?;
    let mut reader = Cursor::new(bytes);
    let value: Value = ciborium::de::from_reader(&mut reader)
        .map_err(|error| fail(format!("CBOR 解码失败 [decode]: {error}")))?;
    if reader.position() != bytes.len() as u64 {
        return Err(fail("CBOR 存在尾随数据 [trailing data]"));
    }
    Ok(value)
}

/// 在通用解码器分配容器前检查长度与嵌套，避免深度炸弹。
/// Bound lengths and nesting before the general decoder allocates containers.
fn preflight(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_CBOR_BYTES {
        return Err(fail("CBOR 长度超出 1..262144 字节 [length out of range]"));
    }
    let mut scanner = Scanner { bytes, position: 0 };
    scanner.item(0)?;
    if scanner.position != bytes.len() {
        return Err(fail("CBOR 存在尾随数据 [trailing data]"));
    }
    Ok(())
}

struct Scanner<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Scanner<'_> {
    fn byte(&mut self) -> Result<u8> {
        let byte = *self
            .bytes
            .get(self.position)
            .ok_or_else(|| fail("CBOR 截断 [truncated]"))?;
        self.position += 1;
        Ok(byte)
    }

    fn argument(&mut self, additional: u8) -> Result<Option<u64>> {
        let count = match additional {
            0..=23 => return Ok(Some(u64::from(additional))),
            24 => 1,
            25 => 2,
            26 => 4,
            27 => 8,
            31 => return Ok(None),
            _ => return Err(fail("CBOR 保留的附加信息 [reserved additional info]")),
        };
        let mut value = 0;
        for _ in 0..count {
            value = (value << 8) | u64::from(self.byte()?);
        }
        Ok(Some(value))
    }

    fn item(&mut self, depth: usize) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(fail("CBOR 嵌套超过 16 层 [nesting exceeds 16]"));
        }
        let initial = self.byte()?;
        let major = initial >> 5;
        let argument = self.argument(initial & 31)?;
        match major {
            0 | 1 | 7 => {
                if argument.is_none() {
                    return Err(fail("意外的 CBOR break [unexpected break]"));
                }
            }
            2 | 3 => match argument {
                Some(length) => {
                    let length = usize::try_from(length)
                        .map_err(|_| fail("CBOR 长度溢出 [length overflow]"))?;
                    self.position = self
                        .position
                        .checked_add(length)
                        .ok_or_else(|| fail("CBOR 长度溢出 [length overflow]"))?;
                    if self.position > self.bytes.len() {
                        return Err(fail("CBOR 字符串截断 [truncated string]"));
                    }
                }
                None => {
                    while self.bytes.get(self.position) != Some(&0xff) {
                        let chunk = *self
                            .bytes
                            .get(self.position)
                            .ok_or_else(|| fail("CBOR 字符串截断 [truncated string]"))?;
                        if chunk >> 5 != major || chunk & 31 == 31 {
                            return Err(fail("非法的不定长 CBOR 字符串块 [invalid chunk]"));
                        }
                        self.item(depth + 1)?;
                    }
                    self.position += 1;
                }
            },
            4 | 5 => {
                if let Some(length) = argument {
                    if length > MAX_CONTAINER_ITEMS {
                        return Err(fail("CBOR 容器超过 64 项 [container exceeds 64]"));
                    }
                    for _ in 0..length {
                        self.item(depth + 1)?;
                        if major == 5 {
                            self.item(depth + 1)?;
                        }
                    }
                } else {
                    let mut count = 0;
                    while self.bytes.get(self.position) != Some(&0xff) {
                        count += 1;
                        if count > MAX_CONTAINER_ITEMS {
                            return Err(fail("CBOR 容器超过 64 项 [container exceeds 64]"));
                        }
                        self.item(depth + 1)?;
                        if major == 5 {
                            self.item(depth + 1)?;
                        }
                    }
                    self.position += 1;
                }
            }
            6 => {
                if argument.is_none() {
                    return Err(fail("非法 CBOR 标签 [invalid tag]"));
                }
                self.item(depth + 1)?;
            }
            _ => return Err(fail("非法 CBOR 主类型 [invalid major type]")),
        }
        Ok(())
    }
}

fn as_array<'a>(value: &'a Value, label: &str) -> Result<&'a [Value]> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(fail(format!("{label} 不是数组 [not an array]"))),
    }
}

fn as_map<'a>(value: &'a Value, label: &str) -> Result<&'a [(Value, Value)]> {
    match value {
        Value::Map(entries) => Ok(entries),
        _ => Err(fail(format!("{label} 不是映射 [not a map]"))),
    }
}

fn as_bytes<'a>(value: &'a Value, label: &str) -> Result<&'a [u8]> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(fail(format!("{label} 不是字节串 [not a byte string]"))),
    }
}

fn as_int(value: &Value, label: &str) -> Result<i128> {
    match value {
        Value::Integer(number) => Ok(i128::from(*number)),
        _ => Err(fail(format!("{label} 不是整数 [not an integer]"))),
    }
}

fn map_get(entries: &[(Value, Value)], key: i128) -> Option<&Value> {
    entries
        .iter()
        .find(|(candidate, _)| as_int(candidate, "key").ok() == Some(key))
        .map(|(_, value)| value)
}

fn array_get<'a>(items: &'a [Value], index: usize, label: &str) -> Result<&'a Value> {
    items
        .get(index)
        .ok_or_else(|| fail(format!("{label} 缺少第 {index} 项 [missing entry {index}]")))
}

/// 构造 COSE_Key（ES256 / P-256，未压缩点）。
pub(crate) fn cose_key(public_key: &PublicKey) -> Result<Value> {
    let point = public_key.to_sec1_bytes();
    if point.len() != 65 || point[0] != 4 {
        return Err(fail(
            "公钥不是未压缩 P-256 点 [not an uncompressed P-256 point]",
        ));
    }
    Ok(Value::Map(vec![
        (cb(1), cb(2)),
        (cb(3), cb(-7)),
        (cb(-1), cb(1)),
        (cb(-2), Value::Bytes(point[1..33].to_vec())),
        (cb(-3), Value::Bytes(point[33..65].to_vec())),
    ]))
}

/// 构造 `{4: challenge, 57: bstr([COSE_Key])}` 并加上小端命令头。
pub(crate) fn build_request(challenge: &[u8], key: &Value) -> Result<Vec<u8>> {
    let keys = Value::Bytes(encode(&Value::Array(vec![key.clone()]))?);
    let payload = encode(&Value::Map(vec![
        (cb(4), Value::Bytes(challenge.to_vec())),
        (cb(57), keys),
    ]))?;
    validate_request(&payload)?;
    let mut request = CSR_COMMAND.to_le_bytes().to_vec();
    request.extend_from_slice(&payload);
    Ok(request)
}

fn validate_request(payload: &[u8]) -> Result<()> {
    if payload.len() > MAX_REQUEST_SIZE - 4 {
        return Err(fail("请求超出 TA 缓冲区 [request exceeds TA buffer]"));
    }
    let value = decode(payload)?;
    let entries = as_map(&value, "CSR 请求 [CSR request]")?;
    if entries.len() != 2 || map_get(entries, 4).is_none() || map_get(entries, 57).is_none() {
        return Err(fail(
            "请求只能包含 challenge 与 keysToSign [unexpected fields]",
        ));
    }
    let challenge = as_bytes(
        map_get(entries, 4).ok_or_else(|| fail("缺少 challenge [missing challenge]"))?,
        "challenge",
    )?;
    let keys = decode(as_bytes(
        map_get(entries, 57).ok_or_else(|| fail("缺少 keysToSign [missing keysToSign]"))?,
        "keysToSign",
    )?)?;
    let keys = as_array(&keys, "keysToSign")?;
    if keys.len() != 1 {
        return Err(fail(
            "必须且只能包含一把 P-256 密钥 [exactly one key required]",
        ));
    }
    validate_inputs(challenge, &keys[0])
}

fn validate_inputs(challenge: &[u8], key: &Value) -> Result<()> {
    if challenge.is_empty() || challenge.len() > MAX_CHALLENGE_LEN {
        return Err(fail(
            "challenge 必须为 1..64 字节 [challenge must be 1..64 bytes]",
        ));
    }
    let entries = as_map(key, "P-256 密钥 [P-256 key]")?;
    for label in [1, 3, -1, -2, -3] {
        if map_get(entries, label).is_none() {
            return Err(fail("P-256 密钥字段不完整 [incomplete P-256 key]"));
        }
    }
    for (label, expected) in [(1, 2), (3, -7), (-1, 1)] {
        if as_int(map_get(entries, label).unwrap(), "COSE 字段")? != expected {
            return Err(fail("必须使用 ES256 / P-256 [ES256 / P-256 required]"));
        }
    }
    let mut point = vec![4];
    for label in [-2, -3] {
        let coordinate = as_bytes(map_get(entries, label).unwrap(), "P-256 坐标")?;
        if coordinate.len() != 32 {
            return Err(fail(
                "P-256 坐标必须为 32 字节 [coordinate must be 32 bytes]",
            ));
        }
        point.extend_from_slice(coordinate);
    }
    PublicKey::from_sec1_bytes(&point)
        .map_err(|_| fail("P-256 公钥点无效 [invalid P-256 point]"))?;
    Ok(())
}

/// 校验 TA 帧头并取出 payload。
pub(crate) fn response_payload(response: &[u8]) -> Result<Vec<u8>> {
    if response.len() < 8 {
        return Err(crate::error::TfError::staged(
            "frame",
            "truncated_header",
            "TA 响应头截断 [truncated header]",
        )
        .with_hint("rkp.frame.truncated_header")
        .with_raw(serde_json::json!({ "response_len": response.len() })));
    }
    let status = i32::from_le_bytes(response[..4].try_into().unwrap());
    if status != 0 {
        return Err(crate::rkp::explain::status_error(status));
    }
    let length = u32::from_le_bytes(response[4..8].try_into().unwrap()) as usize;
    if length == 0 || length > BUFFER_SIZE - 8 || length > response.len() - 8 {
        // 这个分支通常意味着厂商 send_cmd 的返回结构与我们的假设不一致，
        // 而不是 TA 拒绝了请求；把原始帧头一并带出去，便于区分两者。
        // This usually means the vendor response layout differs from our assumption.
        let head = response
            .iter()
            .take(32)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        return Err(crate::error::TfError::staged(
            "frame",
            "payload_length",
            "TA 响应长度无效 [invalid payload length]",
        )
        .with_hint("rkp.frame.payload_length")
        .with_raw(serde_json::json!({
            "status": status,
            "length": length,
            "buffer": BUFFER_SIZE,
            "response_len": response.len(),
            "head_hex": head,
        })));
    }
    Ok(response[8..8 + length].to_vec())
}

/// 固件可能返回裸 AuthenticatedRequest，也可能包在字段 61 中。
pub(crate) fn unwrap_csr(payload: &[u8]) -> Result<Vec<u8>> {
    let value = decode(payload)?;
    if matches!(value, Value::Array(_)) {
        return Ok(payload.to_vec());
    }
    let entries = as_map(&value, "KeyMint CSR 响应 [KeyMint CSR response]")?;
    if entries.len() != 1 {
        return Err(fail(
            "CSR 响应必须只含一个字段 [exactly one field required]",
        ));
    }
    let csr = as_bytes(
        map_get(entries, 61).ok_or_else(|| fail("CSR 响应缺少字段 61 [missing field 61]"))?,
        "AuthenticatedRequest",
    )?;
    if csr.is_empty() {
        return Err(fail("TA 返回空 CSR [empty CSR]"));
    }
    Ok(csr.to_vec())
}

/// 校验 CSR 结构，并绑定本次 challenge 与本次生成的公钥。
pub(crate) fn validate_csr(csr: &[u8], challenge: &[u8], key: &Value) -> Result<CsrInfo> {
    let value = decode(csr)?;
    let request = as_array(&value, "AuthenticatedRequest")?;
    if request.len() != 4 {
        return Err(fail(
            "AuthenticatedRequest 必须包含 4 项 [expected 4 entries]",
        ));
    }
    if as_int(&request[0], "版本")? != AUTHENTICATED_REQUEST_VERSION {
        return Err(fail(
            "AuthenticatedRequest 版本不受支持 [unsupported version]",
        ));
    }
    let dice = as_array(&request[2], "DICE 链 [DICE chain]")?;
    if dice.len() < 2 {
        return Err(fail("DICE 链过短 [DICE chain too short]"));
    }
    let signed = as_array(&request[3], "SignedData")?;
    if signed.len() != 4 {
        return Err(fail("SignedData 必须包含 4 项 [expected 4 entries]"));
    }
    if as_bytes(&signed[0], "protected")?.is_empty() {
        return Err(fail("SignedData protected 为空 [empty protected header]"));
    }
    if as_bytes(&signed[3], "signature")?.is_empty() {
        return Err(fail("SignedData 缺少签名 [missing signature]"));
    }

    let payload = decode(as_bytes(&signed[2], "SignedData payload")?)?;
    let payload = as_array(&payload, "signed payload")?;
    if as_bytes(array_get(payload, 0, "challenge")?, "challenge")? != challenge {
        return Err(fail("CSR challenge 与本次请求不一致 [challenge mismatch]"));
    }
    let contents = decode(as_bytes(
        array_get(payload, 1, "CSR payload")?,
        "CSR payload",
    )?)?;
    let contents = as_array(&contents, "CSR payload")?;
    if as_int(array_get(contents, 0, "CSR 版本")?, "CSR 版本")? != CSR_PAYLOAD_VERSION {
        return Err(fail(
            "CSR payload 版本不受支持 [unsupported payload version]",
        ));
    }
    let keys = as_array(array_get(contents, 3, "keysToSign")?, "keysToSign")?;
    if keys.len() != 1 {
        return Err(fail("keysToSign 必须只有一项 [exactly one key]"));
    }
    if encode(&keys[0])? != encode(key)? {
        return Err(fail("CSR 公钥与本次生成密钥不一致 [public key mismatch]"));
    }

    Ok(CsrInfo {
        dice_entries: dice.len(),
        version: AUTHENTICATED_REQUEST_VERSION,
    })
}

/// worker 只接受 CSR_V2 请求。
pub(crate) fn ta_payload(request: &[u8]) -> Result<&[u8]> {
    let (command, payload) = request
        .split_at_checked(4)
        .ok_or_else(|| fail("请求缺少命令头 [missing command header]"))?;
    if command != CSR_COMMAND.to_le_bytes() {
        return Err(fail("worker 只接受 CSR_V2 请求 [only CSR_V2 accepted]"));
    }
    validate_request(payload)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::SecretKey;

    fn secret(seed: u8) -> SecretKey {
        SecretKey::from_slice(&[seed; 32]).expect("valid scalar")
    }

    #[test]
    fn request_has_command_header_and_two_fields() {
        let key = cose_key(&secret(7).public_key()).expect("cose key");
        let request = build_request(b"challenge", &key).expect("request");
        assert_eq!(&request[..4], &CSR_COMMAND.to_le_bytes());
        let value = decode(&request[4..]).expect("payload");
        assert_eq!(as_map(&value, "request").unwrap().len(), 2);
    }

    #[test]
    fn request_rejects_empty_and_oversized_challenges() {
        let key = cose_key(&secret(9).public_key()).expect("cose key");
        assert!(build_request(b"", &key).is_err());
        assert!(build_request(&[0u8; 65], &key).is_err());
    }

    #[test]
    fn ta_payload_rejects_foreign_command() {
        let mut request = build_request(b"c", &cose_key(&secret(3).public_key()).unwrap()).unwrap();
        request[0] = 0x1e;
        assert!(ta_payload(&request).is_err());
    }

    #[test]
    fn response_header_rejects_nonzero_status() {
        let mut response = vec![0u8; 12];
        response[..4].copy_from_slice(&(-18i32).to_le_bytes());
        let error = response_payload(&response).unwrap_err().to_string();
        assert!(error.contains("-18"));
    }

    #[test]
    fn response_header_returns_bounded_payload() {
        let mut response = vec![0u8; 12];
        response[4..8].copy_from_slice(&4u32.to_le_bytes());
        response[8..12].copy_from_slice(b"data");
        assert_eq!(response_payload(&response).unwrap(), b"data");
    }

    /// 构造一个结构合法、绑定正确的 CSR，用于验证本地检查逻辑。
    fn build_test_csr(challenge: &[u8], _key: &Value, planted: &Value) -> Vec<u8> {
        let contents = encode(&Value::Array(vec![
            cb(3),
            Value::Text("keymint".into()),
            Value::Map(vec![]),
            Value::Array(vec![planted.clone()]),
        ]))
        .unwrap();
        let payload = encode(&Value::Array(vec![
            Value::Bytes(challenge.to_vec()),
            Value::Bytes(contents),
        ]))
        .unwrap();
        let signed = Value::Array(vec![
            Value::Bytes(vec![1, 2, 3]),
            Value::Map(vec![]),
            Value::Bytes(payload),
            Value::Bytes(vec![9, 9]),
        ]);
        encode(&Value::Array(vec![
            cb(1),
            Value::Map(vec![]),
            Value::Array(vec![Value::Map(vec![]), Value::Map(vec![])]),
            signed,
        ]))
        .unwrap()
    }

    #[test]
    fn valid_csr_binds_challenge_and_key() {
        let key = cose_key(&secret(5).public_key()).unwrap();
        let csr = build_test_csr(b"nonce", &key, &key);
        let info = validate_csr(&csr, b"nonce", &key).expect("valid csr");
        assert_eq!(info.dice_entries, 2);
    }

    #[test]
    fn csr_with_other_challenge_is_rejected() {
        let key = cose_key(&secret(5).public_key()).unwrap();
        let csr = build_test_csr(b"nonce", &key, &key);
        assert!(validate_csr(&csr, b"other", &key).is_err());
    }

    #[test]
    fn csr_with_other_public_key_is_rejected() {
        let key = cose_key(&secret(5).public_key()).unwrap();
        let planted = cose_key(&secret(6).public_key()).unwrap();
        let csr = build_test_csr(b"nonce", &key, &planted);
        assert!(validate_csr(&csr, b"nonce", &key).is_err());
    }

    #[test]
    fn wrapped_csr_field_61_is_unwrapped() {
        let inner = encode(&Value::Array(vec![cb(1)])).unwrap();
        let wrapper = encode(&Value::Map(vec![(cb(61), Value::Bytes(inner.clone()))])).unwrap();
        assert_eq!(unwrap_csr(&wrapper).unwrap(), inner);
    }

    #[test]
    fn deep_nesting_is_rejected_before_decoding() {
        let mut bytes = vec![0x81; 20];
        bytes.push(0x00);
        assert!(decode(&bytes).is_err());
    }
}
