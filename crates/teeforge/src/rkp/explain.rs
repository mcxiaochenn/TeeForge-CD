//! KeyMint 状态码解释 [KeyMint status code explanations]
//!
//! 名称来自 AOSP `hardware/interfaces/security/keymint/aidl/.../ErrorCode.aidl`，
//! 说明是本项目针对 RKP 场景补充的。未收录的码统一回落到原始码 + 通用建议，
//! 不猜测含义。
//! Names come from AOSP's ErrorCode.aidl; the explanations are ours. Unknown
//! codes fall back to the raw value with a generic hint instead of a guess.

use crate::error::TfError;
use serde_json::json;

struct Meaning {
    name: &'static str,
    zh: &'static str,
    en: &'static str,
}

const fn entry(name: &'static str, zh: &'static str, en: &'static str) -> Meaning {
    Meaning { name, zh, en }
}

fn lookup(code: i32) -> Option<Meaning> {
    let meaning = match code {
        -18 => entry(
            "INCOMPATIBLE_KEY_FORMAT",
            "TA 不接受当前密钥格式，软件密钥调用 CSR_V2 时最常见",
            "The TA rejects this key format; common with software keys on CSR_V2",
        ),
        -38 => entry(
            "INVALID_ARGUMENT",
            "请求参数被 TA 拒绝，多为固件实现差异",
            "The TA rejected the request arguments, usually a firmware difference",
        ),
        -39 => entry(
            "UNSUPPORTED_TAG",
            "TA 不支持请求中的某个 tag",
            "The TA does not support one of the requested tags",
        ),
        -40 => entry(
            "INVALID_TAG",
            "请求中的 tag 取值非法",
            "A tag in the request has an invalid value",
        ),
        -63 => entry(
            "ATTESTATION_CHALLENGE_MISSING",
            "请求缺少证明 challenge",
            "The attestation challenge is missing",
        ),
        -64 => entry(
            "KEYMINT_NOT_CONFIGURED",
            "TA 尚未完成配置",
            "The TA is not configured yet",
        ),
        -65 => entry(
            "ATTESTATION_APPLICATION_ID_MISSING",
            "请求缺少应用标识",
            "The attestation application id is missing",
        ),
        -68 => entry(
            "HARDWARE_TYPE_UNAVAILABLE",
            "请求的硬件类型不可用（例如要求 TEE 但只有 StrongBox）",
            "The requested hardware type is unavailable",
        ),
        -74 => entry(
            "ATTESTATION_KEYS_NOT_PROVISIONED",
            "设备未预置证明密钥，无法本机签发",
            "Attestation keys are not provisioned on this device",
        ),
        -75 => entry(
            "ATTESTATION_IDS_NOT_PROVISIONED",
            "设备未预置证明 ID",
            "Attestation ids are not provisioned",
        ),
        -100 => entry(
            "UNIMPLEMENTED",
            "固件未实现该命令",
            "The firmware does not implement this command",
        ),
        -101 => entry(
            "VERSION_MISMATCH",
            "命令版本与 TA 不匹配",
            "The command version does not match the TA",
        ),
        -1000 => entry(
            "UNKNOWN_ERROR",
            "TA 内部错误",
            "The TA reported an internal error",
        ),
        _ => return None,
    };
    Some(meaning)
}

/// 把 TA 状态码转成带阶段与原始值的错误。
pub(crate) fn status_error(code: i32) -> TfError {
    let Some(meaning) = lookup(code) else {
        return TfError::staged(
            "ta",
            code.to_string(),
            format!("KeyMint 返回 {code}（未收录的代号）[KeyMint returned {code}: unmapped]"),
        )
        .with_hint("rkp.ta.status")
        .with_raw(json!({ "keymint": code }));
    };
    TfError::staged(
        "ta",
        code.to_string(),
        format!(
            "KeyMint 返回 {code}（{}）：{} [{}]",
            meaning.name, meaning.zh, meaning.en
        ),
    )
    .with_hint("rkp.ta.status")
    .with_raw(json!({ "keymint": code, "name": meaning.name }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_code_carries_name_and_stage() {
        let error = status_error(-18);
        assert_eq!(error.stage(), Some("ta"));
        assert_eq!(error.code(), Some("-18"));
        assert!(error.message().contains("INCOMPATIBLE_KEY_FORMAT"));
        assert_eq!(error.hint(), Some("rkp.ta.status"));
    }

    #[test]
    fn unknown_code_is_not_guessed() {
        let error = status_error(-4242);
        assert_eq!(error.code(), Some("-4242"));
        assert!(error.message().contains("未收录的代号"));
    }
}
