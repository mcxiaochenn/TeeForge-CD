//! TA 调用的隔离子进程 [Isolated TA worker process]
//!
//! 移植自 Duck-ToolBox 的 `duck-rkp-device::worker`（MIT）。
//! Ported from Duck-ToolBox `duck-rkp-device::worker` (MIT).
//!
//! 厂商库可能崩溃或挂起，无法在主进程内安全中断；由父进程写入请求，子进程调用 TA
//! 并回写 JSON 回执，父进程超时后 kill 并回收。
//! Vendor calls cannot be interrupted in-process; a child owns the call and the
//! parent kills and reaps it on timeout.

use crate::error::{Result, TfError};
use crate::process;
use crate::rkp::protocol;
use crate::rkp::ta::RawFrame;
use crate::rkp::{unique_dir, write_private};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(30);
/// 0xa000 字节的 base64 约 54 KB，回执还带诊断字段，留足余量。
/// A 0xa000-byte frame is ~54 KB in base64; leave room for diagnostics.
const MAX_REPLY_BYTES: usize = 96 * 1024;
/// Bionic 在 main 之前读取该值；保留默认命名空间以继续解析系统依赖。
/// Bionic reads this before main; the default namespace keeps system dependency links.
const VENDOR_LIBRARY_PATH: &str =
    "/system/lib64:/vendor/lib64:/odm/lib64:/vendor/lib64/hw:/odm/lib64/hw";

pub(crate) fn sign(tmp_root: &Path, request: &[u8]) -> Result<RawFrame> {
    let directory = unique_dir(tmp_root)?;
    let result = (|| {
        write_private(&directory.join("request.bin"), request)?;
        let executable = env::current_exe().map_err(|error| {
            TfError::new(format!("无法定位当前可执行文件 [current_exe]: {error}"))
        })?;
        let mut command = Command::new(&executable);
        command
            .arg("--rkp-ta-worker")
            .arg(&directory)
            .env("LD_LIBRARY_PATH", VENDOR_LIBRARY_PATH);
        let output = process::run_bounded(
            command,
            "teeforge RKP worker",
            TIMEOUT,
            4096,
            "设备端 RKP 调用 [device RKP call]",
        )?;
        if !output.status.success() {
            return Err(TfError::new(format!(
                "RKP worker 退出码 {}，厂商 ABI 可能不兼容 [worker exit {}; vendor ABI may be incompatible]{}",
                output.status.code().unwrap_or(128),
                output.status.code().unwrap_or(128),
                stderr_detail(&output.stderr)
            )));
        }
        let reply = read_bounded(&directory.join("reply.json"), MAX_REPLY_BYTES)?;
        parse_reply(&reply)
    })();
    let _ = fs::remove_dir_all(&directory);
    result
}

/// 子进程入口：读取请求、调用 TA、写回执。
///
/// 回执必须保留结构化错误：早期用 `to_string()` 传字符串，父进程无法还原
/// `stage/code/hint/raw`，诊断信息（hex、长度）全部丢失。
/// Structured errors must survive the process boundary.
pub(crate) fn execute(directory: &Path) -> Result<()> {
    let result = (|| {
        let request = read_bounded(&directory.join("request.bin"), protocol::MAX_REQUEST_SIZE)?;
        protocol::ta_payload(&request)?;
        crate::rkp::ta::request(&request)
    })();
    let reply = match result {
        Ok(frame) => json!({
            "ok": true,
            "status": frame.status,
            "head_hex": protocol::head_hex(&frame.bytes),
            "frame": STANDARD.encode(&frame.bytes),
        }),
        Err(error) => json!({ "ok": false, "error": describe(&error) }),
    };
    let encoded = serde_json::to_vec(&reply)
        .map_err(|error| TfError::new(format!("编码 worker 回执失败 [encode reply]: {error}")))?;
    write_private(&directory.join("reply.json"), &encoded)
}

/// 把 `TfError` 拆成可跨进程传输的对象。
fn describe(error: &TfError) -> Value {
    let mut payload = json!({ "message": error.message() });
    if let Some(value) = error.stage() {
        payload["stage"] = json!(value);
    }
    if let Some(value) = error.code() {
        payload["code"] = json!(value);
    }
    if let Some(value) = error.hint() {
        payload["hint"] = json!(value);
    }
    if let Some(value) = error.raw() {
        payload["raw"] = value.clone();
    }
    payload
}

fn parse_reply(bytes: &[u8]) -> Result<RawFrame> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| TfError::new(format!("解析 worker 回执失败 [decode reply]: {error}")))?;
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(restore_error(value.get("error")));
    }
    let frame = value
        .get("frame")
        .and_then(Value::as_str)
        .ok_or_else(|| TfError::new("worker 回执缺少 frame [missing frame]"))?;
    let decoded = STANDARD
        .decode(frame)
        .map_err(|_| TfError::new("worker 回执 frame 解码失败 [frame decode failed]"))?;
    if decoded.len() > protocol::BUFFER_SIZE {
        return Err(TfError::new(
            "worker 回执超过 TA 缓冲区 [reply exceeds TA buffer]",
        ));
    }
    let status = value.get("status").and_then(Value::as_i64).unwrap_or(0) as i32;
    Ok(RawFrame {
        bytes: decoded,
        status,
    })
}

/// 还原子进程错误；兼容旧版纯字符串回执。
fn restore_error(value: Option<&Value>) -> TfError {
    let Some(value) = value else {
        return TfError::new("worker 回执缺少错误信息 [missing error]");
    };
    if let Some(text) = value.as_str() {
        return TfError::new(text.to_owned());
    }
    let text = |key: &str| value.get(key).and_then(Value::as_str);
    TfError::from_parts(
        text("stage"),
        text("code").map(str::to_owned),
        text("message").unwrap_or("未知错误 [unknown error]"),
        text("hint").map(str::to_owned),
        value.get("raw").cloned(),
    )
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        TfError::new(format!(
            "读取 {} 失败 [read failed]: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(TfError::new(format!(
            "{} 不是普通文件 [not a regular file]",
            path.display()
        )));
    }
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(TfError::new(format!(
            "{} 超出 {} 字节限制 [exceeds limit]",
            path.display(),
            limit
        )));
    }
    Ok(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_without_frame_is_rejected() {
        assert!(parse_reply(br#"{"ok":true}"#).is_err());
    }

    #[test]
    fn legacy_string_error_is_accepted() {
        let error =
            parse_reply(r#"{"ok":false,"error":"TA 返回状态 -18"}"#.as_bytes()).unwrap_err();
        assert!(error.to_string().contains("-18"));
    }

    #[test]
    fn structured_error_keeps_stage_and_raw() {
        let reply = br#"{"ok":false,"error":{"stage":"frame","code":"unparsed","message":"x","hint":"rkp.frame.unparsed","raw":{"head_hex":"00"}}}"#;
        let error = parse_reply(reply).unwrap_err();
        assert_eq!(error.stage(), Some("frame"));
        assert_eq!(error.code(), Some("unparsed"));
        assert_eq!(error.hint(), Some("rkp.frame.unparsed"));
        assert!(error.raw().is_some());
    }

    #[test]
    fn frame_is_decoded() {
        let frame = parse_reply(br#"{"ok":true,"status":0,"frame":"YWJj"}"#).unwrap();
        assert_eq!(frame.bytes, b"abc");
        assert_eq!(frame.status, 0);
    }

    #[test]
    fn non_regular_reply_files_are_rejected() {
        let directory =
            std::env::temp_dir().join(format!("teeforge-rkp-worker-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        assert!(read_bounded(&directory, 16).is_err());
        let _ = fs::remove_dir_all(directory);
    }
}
