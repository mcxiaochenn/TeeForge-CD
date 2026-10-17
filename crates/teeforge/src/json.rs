//! `--json` 输出外壳 [JSON envelope for `--json` mode]
//!
//! 约定 [Contract]
//! - 成功：`{"ok":true,"data":{...}}`
//! - 失败：`{"ok":false,"error":{"stage":..,"code":..,"message":..,"hint":..,"raw":..}}`
//! - JSON 只写 stdout；日志仍走 stderr（`logging` 已固定为 eprintln），互不污染。
//! - 无论成功失败退出码仍保持原有语义，调用方两者都要检查。

use crate::error::TfError;
use serde_json::{Value, json};

/// 打印成功信封。
pub(crate) fn ok(data: Value) {
    println!("{}", json!({ "ok": true, "data": data }));
}

/// 打印失败信封，并返回应使用的退出码。
pub(crate) fn fail(error: &TfError) -> i32 {
    let mut payload = json!({
        "message": error.message(),
    });
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
    println!("{}", json!({ "ok": false, "error": payload }));
    1
}

/// 便捷入口：成功打印数据并返回 0，失败打印信封并返回 1。
pub(crate) fn finish(result: crate::error::Result<Value>) -> i32 {
    match result {
        Ok(data) => {
            ok(data);
            0
        }
        Err(error) => fail(&error),
    }
}
