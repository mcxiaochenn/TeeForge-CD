use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

// 与主程序日志格式保持一致，便于两段日志落进同一个文件后对照。
// Keep the same line format as the main binary so both stages read alike in one log.
pub fn log(level: Level, message: impl AsRef<str>) {
    eprintln!(
        "[{}] [{}] {}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |value| value.as_secs()),
        level.label(),
        message.as_ref()
    );
}
