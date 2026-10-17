//! Google RKP 服务客户端 [Google RKP server client]
//!
//! 移植自 Duck-ToolBox 的 `duck-rkp::http`（MIT），但改用设备已有的
//! `curl` / `wget` 子进程，保持本项目不引入异步运行时。
//! Ported from `duck-rkp::http` (MIT), but re-implemented on top of the
//! device's existing `curl` / `wget` so this project keeps its synchronous model.

use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;
use crate::rkp::protocol::{decode, encode};
use crate::rkp::{unique_dir, write_private};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ciborium::value::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const MAX_RESPONSE: usize = 1024 * 1024;
const MAX_CHALLENGE_LEN: usize = 64;
const HTTP_TIMEOUT_SECS: u64 = 30;

fn fail(message: impl Into<String>) -> TfError {
    TfError::new(format!("RKP 服务 [RKP server]: {}", message.into()))
}

/// 取 challenge（`fetchEekChain` 的响应里带 anti-replay challenge）。
pub(crate) fn fetch_challenge(server: &str, fingerprint: &str, tmp_root: &Path) -> Result<Vec<u8>> {
    let url = method_url(server, "fetchEekChain")?;
    let body = encode(&Value::Map(vec![
        (
            Value::Text("fingerprint".into()),
            Value::Text(fingerprint.to_owned()),
        ),
        (Value::Text("id".into()), Value::Integer(42u64.into())),
    ]))?;
    let response = post(&url, &body, tmp_root)?;
    parse_challenge(&response)
}

/// 提交 CSR，返回拼接后的 leaf→root DER 链。
pub(crate) fn sign_certificates(
    server: &str,
    csr: &[u8],
    challenge: &[u8],
    tmp_root: &Path,
) -> Result<Vec<u8>> {
    let endpoint = method_url(server, "signCertificates")?;
    let url = format!("{endpoint}?challenge={}", URL_SAFE_NO_PAD.encode(challenge));
    let response = post(&url, csr, tmp_root)?;
    parse_certificates(&response)
}

fn method_url(server: &str, method: &str) -> Result<String> {
    let base = server.trim().trim_end_matches('/');
    if base.is_empty() || base.contains(['?', '#']) || base.contains(char::is_whitespace) {
        return Err(fail("服务地址无效 [invalid endpoint]"));
    }
    let loopback = base.starts_with("http://127.0.0.1") || base.starts_with("http://localhost");
    let https = base.starts_with("https://");
    if !https && !loopback {
        return Err(fail("服务地址必须是 https [endpoint must use https]"));
    }
    Ok(format!("{base}:{method}"))
}

fn post(url: &str, body: &[u8], tmp_root: &Path) -> Result<Vec<u8>> {
    let directory = unique_dir(tmp_root)?;
    let result = (|| {
        let request = directory.join("request.bin");
        let response = directory.join("response.bin");
        write_private(&request, body)?;
        post_with_tools(url, &request, &response)
    })();
    let _ = fs::remove_dir_all(&directory);
    result
}

fn post_with_tools(url: &str, request: &Path, response: &Path) -> Result<Vec<u8>> {
    let mut errors = Vec::new();
    for (label, command) in [
        ("curl", curl_command(url, request, response)),
        ("wget", wget_command("wget", url, request, response)),
        (
            "busybox wget",
            wget_command("busybox", url, request, response),
        ),
    ] {
        match run_post(command, label, response) {
            Ok(bytes) => {
                logging::log(
                    Level::Info,
                    format!("RKP 传输工具 [RKP transport]: {label}"),
                );
                return Ok(bytes);
            }
            Err(error) => errors.push(format!("{label}: {error}")),
        }
    }
    Err(fail(format!(
        "所有传输工具均失败 [all transports failed]: {}",
        errors.join("; ")
    )))
}

fn curl_command(url: &str, request: &Path, response: &Path) -> Command {
    let mut command = Command::new("curl");
    command.args([
        "-sS",
        "--max-time",
        &HTTP_TIMEOUT_SECS.to_string(),
        "--connect-timeout",
        "10",
        "--request",
        "POST",
        "--header",
        "Content-Type: application/cbor",
        "--header",
        "Accept: application/cbor",
        "--data-binary",
    ]);
    command
        .arg(format!("@{}", request.display()))
        .arg("--output")
        .arg(response)
        .arg("--write-out")
        .arg("%{http_code}")
        .arg(url);
    command
}

fn wget_command(program: &str, url: &str, request: &Path, response: &Path) -> Command {
    let mut command = Command::new(program);
    if program == "busybox" {
        command.arg("wget");
    }
    command
        .args(["-q", "--timeout=30", "-O"])
        .arg(response)
        .arg("--header=Content-Type: application/cbor")
        .arg("--header=Accept: application/cbor")
        .arg("--post-file")
        .arg(request)
        .arg(url);
    command
}

fn run_post(command: Command, label: &str, response: &Path) -> Result<Vec<u8>> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = process::run_bounded(
        command,
        label,
        Duration::from_secs(HTTP_TIMEOUT_SECS + 10),
        32,
        "RKP 请求 [RKP request]",
    )?;
    if !output.status.success() {
        return Err(TfError::new(format!(
            "{program} 退出码 {} [exit code {}]{}",
            output.status.code().unwrap_or(128),
            output.status.code().unwrap_or(128),
            stderr_detail(&output.stderr)
        )));
    }
    if label == "curl" {
        let text = String::from_utf8_lossy(&output.stdout);
        if let Ok(status) = text.trim().parse::<u16>() {
            if status == 444 {
                return Err(fail("设备未在 Google RKP 注册 [device not registered]"));
            }
            if !(200..300).contains(&status) {
                return Err(fail(format!("HTTP {status}")));
            }
        }
    }
    read_response(response)
}

fn stderr_detail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let summary = normalized.chars().take(200).collect::<String>();
    if summary.is_empty() {
        String::new()
    } else {
        format!(": {summary}")
    }
}

fn read_response(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let file = fs::File::open(path).map_err(|_| fail("响应文件不存在 [response missing]"))?;
    let mut bytes = Vec::new();
    file.take((MAX_RESPONSE + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() {
        return Err(fail("响应为空 [empty response]"));
    }
    if bytes.len() > MAX_RESPONSE {
        return Err(fail("响应超过 1 MiB [response exceeds 1 MiB]"));
    }
    Ok(bytes)
}

fn parse_challenge(body: &[u8]) -> Result<Vec<u8>> {
    let value = decode(body)?;
    let items = match &value {
        Value::Array(items) => items,
        _ => return Err(fail("challenge 响应不是数组 [response is not an array]")),
    };
    let challenge = items
        .get(1)
        .and_then(|value| match value {
            Value::Bytes(bytes) => Some(bytes.as_slice()),
            _ => None,
        })
        .ok_or_else(|| fail("challenge 响应缺少 challenge [missing challenge]"))?;
    if challenge.is_empty() || challenge.len() > MAX_CHALLENGE_LEN {
        return Err(fail("challenge 长度无效 [invalid challenge length]"));
    }
    Ok(challenge.to_vec())
}

fn parse_certificates(body: &[u8]) -> Result<Vec<u8>> {
    let value = decode(body)?;
    let top = match &value {
        Value::Array(items) if !items.is_empty() => items.as_slice(),
        _ => return Err(fail("签发响应为空 [empty issuance response]")),
    };
    let inner = match &top[0] {
        Value::Array(items) => items.as_slice(),
        _ => top,
    };
    if inner.len() < 2 {
        return Err(fail("签发响应缺少证书链 [missing certificate chains]"));
    }
    let shared = match &inner[0] {
        Value::Bytes(bytes) => bytes,
        _ => return Err(fail("共享证书链不是字节串 [shared chain is not bytes]")),
    };
    let unique = match &inner[1] {
        Value::Array(items) => items,
        _ => return Err(fail("唯一证书链列表无效 [invalid unique chain list]")),
    };
    if unique.len() != 1 {
        return Err(fail(format!(
            "预期一条证书链，实际 {} [expected one chain, got {}]",
            unique.len(),
            unique.len()
        )));
    }
    let unique = match &unique[0] {
        Value::Bytes(bytes) => bytes,
        _ => return Err(fail("唯一证书链不是字节串 [unique chain is not bytes]")),
    };
    let mut chain = shared.clone();
    chain.extend_from_slice(unique);
    if chain.is_empty() || chain.len() > crate::rkp::MAX_CHAIN_BYTES {
        return Err(fail(
            "证书链长度为 1..16384 字节 [chain length out of range]",
        ));
    }
    Ok(chain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_value(value: Value) -> Vec<u8> {
        encode(&value).unwrap()
    }

    #[test]
    fn endpoint_requires_https_or_loopback() {
        assert!(method_url("https://example.test/v1", "fetchEekChain").is_ok());
        assert!(method_url("http://127.0.0.1:8080/v1", "fetchEekChain").is_ok());
        assert!(method_url("http://example.test/v1", "fetchEekChain").is_err());
        assert!(method_url("https://example.test/v1?x=1", "fetchEekChain").is_err());
    }

    #[test]
    fn challenge_is_read_from_second_item() {
        let body = encode_value(Value::Array(vec![
            Value::Array(vec![]),
            Value::Bytes(b"nonce".to_vec()),
        ]));
        assert_eq!(parse_challenge(&body).unwrap(), b"nonce");
    }

    #[test]
    fn challenge_length_is_bounded() {
        let body = encode_value(Value::Array(vec![
            Value::Array(vec![]),
            Value::Bytes(vec![1; 65]),
        ]));
        assert!(parse_challenge(&body).is_err());
    }

    #[test]
    fn certificate_chains_are_concatenated() {
        let body = encode_value(Value::Array(vec![Value::Array(vec![
            Value::Bytes(vec![1, 2]),
            Value::Array(vec![Value::Bytes(vec![3, 4])]),
        ])]));
        assert_eq!(parse_certificates(&body).unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn multiple_unique_chains_are_rejected() {
        let body = encode_value(Value::Array(vec![Value::Array(vec![
            Value::Bytes(vec![1]),
            Value::Array(vec![Value::Bytes(vec![2]), Value::Bytes(vec![3])]),
        ])]));
        assert!(parse_certificates(&body).is_err());
    }
}
