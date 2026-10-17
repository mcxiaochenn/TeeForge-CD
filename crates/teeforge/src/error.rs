use std::fmt::{self, Display, Formatter};

/// 统一错误类型。
///
/// `stage` / `code` / `hint` / `raw` 只在需要结构化输出时填充；
/// 文本模式下 `Display` 的行为保持不变，因此既有调用点无需改动。
/// Structured fields are optional so existing `Display`-only call sites keep working.
#[derive(Debug)]
pub struct TfError {
    message: String,
    stage: Option<&'static str>,
    code: Option<String>,
    hint: Option<String>,
    raw: Option<serde_json::Value>,
}

impl TfError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stage: None,
            code: None,
            hint: None,
            raw: None,
        }
    }

    pub(crate) fn context(mut self, context: impl Display) -> Self {
        self.message = format!("{context}: {}", self.message);
        self
    }

    /// 带阶段与错误码的错误；`code` 保持原始值（如 TA 状态码 `-18`）。
    pub(crate) fn staged(
        stage: &'static str,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            stage: Some(stage),
            code: Some(code.into()),
            hint: None,
            raw: None,
        }
    }

    /// 附加 i18n 提示键，供 WebUI 直接查表。
    pub(crate) fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// 附加原始诊断数据（状态码、长度、hex 片段等）。
    pub(crate) fn with_raw(mut self, raw: serde_json::Value) -> Self {
        self.raw = Some(raw);
        self
    }

    pub(crate) fn stage(&self) -> Option<&'static str> {
        self.stage
    }

    pub(crate) fn code(&self) -> Option<&str> {
        self.code.as_deref()
    }

    pub(crate) fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    pub(crate) fn raw(&self) -> Option<&serde_json::Value> {
        self.raw.as_ref()
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl Display for TfError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TfError {}

impl From<std::io::Error> for TfError {
    fn from(value: std::io::Error) -> Self {
        Self::new(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, TfError>;
