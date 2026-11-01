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

// post-fs-data 阶段 RTC 尚未由 time_daemon 对齐，墙钟可能停在 1970，两阶段时间戳无法互相比较。
// 开机时长是唯一能把 early 与 late 排到同一时间轴上的量，因此随摘要一起落盘。
// The wall clock may still read 1970 during post-fs-data, so the two stages cannot be
// ordered by timestamp. Uptime is the only boot-relative scale, so record it too.
pub fn uptime_secs() -> Option<u64> {
    parse_uptime(&std::fs::read_to_string("/proc/uptime").ok()?)
}

fn parse_uptime(raw: &str) -> Option<u64> {
    raw.split_whitespace()
        .next()?
        .split('.')
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::parse_uptime;

    #[test]
    fn parses_first_field_of_proc_uptime() {
        assert_eq!(parse_uptime("12345.67 98765.43\n"), Some(12345));
        assert_eq!(parse_uptime("0.00 0.00\n"), Some(0));
        assert_eq!(parse_uptime("31"), Some(31));
    }

    #[test]
    fn rejects_unparsable_uptime() {
        assert_eq!(parse_uptime(""), None);
        assert_eq!(parse_uptime("abc 1.00\n"), None);
    }
}
