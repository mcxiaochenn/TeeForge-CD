use std::fmt::{self, Display, Formatter};

#[derive(Debug)]
pub struct TfError {
    message: String,
}

impl TfError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn context(self, context: impl Display) -> Self {
        Self::new(format!("{context}: {}", self.message))
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
