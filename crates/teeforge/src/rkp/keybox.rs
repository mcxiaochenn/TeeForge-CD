//! 签发证书链校验与 keybox 生成 [Issued chain validation and keybox output]
//!
//! 移植自 Duck-ToolBox 的 `duck-rkp-device::artifacts` 与 `duck-rkp::keybox_xml`（MIT）。
//! Ported from Duck-ToolBox `duck-rkp-device::artifacts` and `duck-rkp::keybox_xml` (MIT).
//!
//! 写入前必须确认：证书链是唯一的 leaf→root 路径，且叶子公钥与本机私钥匹配。
//! Before writing, the chain must be exactly one leaf-to-root path and its leaf
//! must carry the public key of the private key we keep.

use crate::error::{Result, TfError};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use p256::SecretKey;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use x509_cert::Certificate;
use x509_cert::der::asn1::ObjectIdentifier;
use x509_cert::der::{Decode, Encode};

const MAX_CERTIFICATES: usize = 8;
const EC_PUBLIC_KEY_OID: &str = "1.2.840.10045.2.1";
const P256_CURVE_OID: &str = "1.2.840.10045.3.1.7";

#[derive(Clone, Debug)]
struct ParsedCertificate {
    der: Vec<u8>,
    subject_der: Vec<u8>,
    issuer_der: Vec<u8>,
    /// 未压缩 P-256 公钥；非 EC P-256 主体时为 `None`。
    public_key: Option<Vec<u8>>,
}

fn fail(message: impl Into<String>) -> TfError {
    TfError::new(format!(
        "RKP 证书链 [RKP certificate chain]: {}",
        message.into()
    ))
}

/// 生成 keybox XML，并完成证书链与私钥的绑定校验。
pub(crate) fn build(secret: &SecretKey, chain_der: &[u8]) -> Result<String> {
    let chain = parse_chain(chain_der)?;
    ensure_leaf_matches(secret, &chain)?;
    let ordered = leaf_to_root(&chain)?;
    if ordered.len() != chain.len() {
        return Err(fail(
            "存在不在 leaf→root 路径上的证书 [certificate outside the path]",
        ));
    }
    let device_id = device_id(secret);
    let private_key = secret
        .to_sec1_der()
        .map_err(|error| fail(format!("导出私钥失败 [encode private key]: {error}")))?;
    let ordered_certificates = ordered
        .iter()
        .map(|index| &chain[*index])
        .collect::<Vec<_>>();
    Ok(render_xml(&private_key, &ordered_certificates, &device_id))
}

fn parse_chain(data: &[u8]) -> Result<Vec<ParsedCertificate>> {
    if data.is_empty() {
        return Err(fail("证书链为空 [empty chain]"));
    }
    if data.len() > crate::rkp::MAX_CHAIN_BYTES {
        return Err(fail("证书链过大 [chain too large]"));
    }
    let mut certificates = Vec::new();
    let mut offset = 0usize;
    while offset < data.len() {
        let slice = &data[offset..];
        let (header, body) = der_len(slice)?;
        let total = header
            .checked_add(body)
            .ok_or_else(|| fail("DER 长度溢出 [length overflow]"))?;
        let end = offset
            .checked_add(total)
            .ok_or_else(|| fail("DER 长度溢出 [length overflow]"))?;
        if end > data.len() {
            return Err(fail("证书截断 [truncated certificate]"));
        }
        let der = &data[offset..end];
        let certificate = Certificate::from_der(der)
            .map_err(|error| fail(format!("解析 X.509 证书失败 [parse certificate]: {error}")))?;
        let tbs = &certificate.tbs_certificate;
        certificates.push(ParsedCertificate {
            der: der.to_vec(),
            subject_der: tbs
                .subject
                .to_der()
                .map_err(|error| fail(format!("编码 subject 失败 [encode subject]: {error}")))?,
            issuer_der: tbs
                .issuer
                .to_der()
                .map_err(|error| fail(format!("编码 issuer 失败 [encode issuer]: {error}")))?,
            public_key: p256_spki(tbs),
        });
        if certificates.len() > MAX_CERTIFICATES {
            return Err(fail("证书数量超过 8 [more than 8 certificates]"));
        }
        offset = end;
    }
    Ok(certificates)
}

fn der_len(bytes: &[u8]) -> Result<(usize, usize)> {
    if bytes.len() < 2 {
        return Err(fail("DER 序列截断 [truncated DER]"));
    }
    if bytes[0] != 0x30 {
        return Err(fail(
            "DER 证书必须以 SEQUENCE 开头 [must start with SEQUENCE]",
        ));
    }
    if bytes[1] & 0x80 == 0 {
        return Ok((2, usize::from(bytes[1])));
    }
    if bytes[1] == 0x80 {
        return Err(fail("不支持 DER 不定长 [indefinite length]"));
    }
    let count = usize::from(bytes[1] & 0x7f);
    if count == 0 || count > std::mem::size_of::<usize>() {
        return Err(fail("DER 长度字段无效 [invalid length field]"));
    }
    let length_bytes = bytes
        .get(2..2 + count)
        .ok_or_else(|| fail("DER 长度截断 [truncated length]"))?;
    let mut length = 0usize;
    for byte in length_bytes {
        length = (length << 8) | usize::from(*byte);
    }
    Ok((2 + count, length))
}

/// 叶子证书：链中没有其他证书以其为 issuer。
fn leaf_index(chain: &[ParsedCertificate]) -> Option<usize> {
    chain.iter().position(|candidate| {
        !chain.iter().any(|other| {
            other.subject_der != other.issuer_der && other.issuer_der == candidate.subject_der
        })
    })
}

fn ensure_leaf_matches(secret: &SecretKey, chain: &[ParsedCertificate]) -> Result<()> {
    let leaf = leaf_index(chain)
        .and_then(|index| chain[index].public_key.as_deref())
        .ok_or_else(|| fail("找不到可用的叶子证书 [no usable leaf certificate]"))?;
    if leaf != secret.public_key().to_sec1_bytes().as_ref() {
        return Err(fail(
            "叶子公钥与本机私钥不匹配 [leaf key does not match the private key]",
        ));
    }
    Ok(())
}

fn leaf_to_root(chain: &[ParsedCertificate]) -> Result<Vec<usize>> {
    let start = leaf_index(chain).ok_or_else(|| fail("找不到叶子证书 [no leaf certificate]"))?;
    let mut current = start;
    let mut ordered = vec![start];
    while chain[current].issuer_der != chain[current].subject_der {
        let issuer = chain
            .iter()
            .position(|candidate| candidate.subject_der == chain[current].issuer_der)
            .ok_or_else(|| fail("证书链缺少签发者 [missing issuer]"))?;
        if ordered.contains(&issuer) {
            return Err(fail("证书链存在环 [cycle in chain]"));
        }
        ordered.push(issuer);
        current = issuer;
    }
    Ok(ordered)
}

fn p256_spki(tbs: &x509_cert::TbsCertificate) -> Option<Vec<u8>> {
    let spki = &tbs.subject_public_key_info;
    if spki.algorithm.oid.to_string() != EC_PUBLIC_KEY_OID {
        return None;
    }
    let curve = spki
        .algorithm
        .parameters
        .as_ref()?
        .decode_as::<ObjectIdentifier>()
        .ok()?;
    if curve.to_string() != P256_CURVE_OID {
        return None;
    }
    let point = spki.subject_public_key.raw_bytes();
    p256::PublicKey::from_sec1_bytes(point)
        .ok()
        .map(|key| key.to_sec1_bytes().to_vec())
}

fn device_id(secret: &SecretKey) -> String {
    let digest = Sha256::digest(secret.public_key().to_sec1_bytes());
    let suffix = digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("teeforge-{suffix}")
}

fn render_xml(private_key_der: &[u8], chain: &[&ParsedCertificate], device_id: &str) -> String {
    let mut xml = String::new();
    writeln!(xml, "<?xml version=\"1.0\"?>").unwrap();
    writeln!(xml, "<AndroidAttestation>").unwrap();
    writeln!(xml, "    <NumberOfKeyboxes>1</NumberOfKeyboxes>").unwrap();
    writeln!(
        xml,
        "    <Keybox DeviceID=\"{}\">",
        escape_attribute(device_id)
    )
    .unwrap();
    writeln!(xml, "        <Key algorithm=\"ecdsa\">").unwrap();
    writeln!(xml, "            <PrivateKey format=\"pem\">").unwrap();
    writeln!(
        xml,
        "{}",
        indent(&pem("EC PRIVATE KEY", private_key_der), "                ")
    )
    .unwrap();
    writeln!(xml, "            </PrivateKey>").unwrap();
    writeln!(xml, "            <CertificateChain>").unwrap();
    writeln!(
        xml,
        "                <NumberOfCertificates>{}</NumberOfCertificates>",
        chain.len()
    )
    .unwrap();
    for certificate in chain {
        let certificate_pem = pem("CERTIFICATE", &certificate.der);
        writeln!(xml, "                <Certificate format=\"pem\">").unwrap();
        writeln!(xml, "{}", indent(&certificate_pem, "                    ")).unwrap();
        writeln!(xml, "                </Certificate>").unwrap();
    }
    writeln!(xml, "            </CertificateChain>").unwrap();
    writeln!(xml, "        </Key>").unwrap();
    writeln!(xml, "    </Keybox>").unwrap();
    writeln!(xml, "</AndroidAttestation>").unwrap();
    xml
}

fn pem(label: &str, der: &[u8]) -> String {
    let encoded = STANDARD.encode(der);
    let mut pem = String::new();
    writeln!(pem, "-----BEGIN {label}-----").unwrap();
    for chunk in encoded.as_bytes().chunks(64) {
        writeln!(pem, "{}", String::from_utf8_lossy(chunk)).unwrap();
    }
    write!(pem, "-----END {label}-----").unwrap();
    pem
}

fn indent(block: &str, prefix: &str) -> String {
    block
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(seed: u8) -> SecretKey {
        SecretKey::from_slice(&[seed; 32]).expect("valid scalar")
    }

    #[test]
    fn der_length_parser_handles_short_and_long_forms() {
        assert_eq!(der_len(&[0x30, 0x05]).unwrap(), (2, 5));
        assert_eq!(der_len(&[0x30, 0x81, 0x80]).unwrap(), (3, 128));
        assert_eq!(der_len(&[0x30, 0x82, 0x01, 0x00]).unwrap(), (4, 256));
        assert!(der_len(&[0x30, 0x80]).is_err());
        assert!(der_len(&[0x31, 0x00]).is_err());
    }

    #[test]
    fn truncated_chain_is_rejected() {
        assert!(parse_chain(&[0x30, 0x05, 0x01]).is_err());
        assert!(parse_chain(&[]).is_err());
    }

    #[test]
    fn device_id_is_stable_and_escaped() {
        let first = device_id(&secret(1));
        assert_eq!(first, device_id(&secret(1)));
        assert!(first.starts_with("teeforge-"));
        assert_eq!(escape_attribute("a\"<&>'"), "a&quot;&lt;&amp;&gt;&apos;");
    }

    #[test]
    fn pem_wraps_at_64_columns() {
        let block = pem("CERTIFICATE", &[0u8; 96]);
        assert!(block.starts_with("-----BEGIN CERTIFICATE-----\n"));
        assert!(block.ends_with("-----END CERTIFICATE-----"));
        for line in block
            .lines()
            .skip(1)
            .take_while(|line| !line.starts_with("-----"))
        {
            assert!(line.len() <= 64);
        }
    }
}
