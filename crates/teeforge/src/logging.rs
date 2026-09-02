use crate::config::Config;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

struct State {
    level: Level,
    debug_file: Option<std::path::PathBuf>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

pub(crate) fn init(config: &Config, verbose: bool) {
    let timestamp = now();
    let debug_file = config.debug.then(|| {
        config
            .log_dir
            .join(format!("teeforge_{}.log", timestamp / 86_400))
    });
    if let Some(path) = &debug_file {
        let _ = fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new(".")));
        if let Some(directory) = path.parent() {
            cleanup_old_logs(directory);
        }
    }
    let state = State {
        level: if verbose { Level::Debug } else { Level::Info },
        debug_file,
    };
    let _ = STATE.set(Mutex::new(state));
}

fn cleanup_old_logs(directory: &std::path::Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut logs = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name_text = name.to_string_lossy();
            (entry.file_type().ok()?.is_file()
                && name_text.starts_with("teeforge_")
                && name_text.ends_with(".log"))
            .then_some((name, entry.path()))
        })
        .collect::<Vec<_>>();
    logs.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in logs.into_iter().skip(15) {
        let _ = fs::remove_file(path);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_secs())
}

pub(crate) fn log(level: Level, message: impl AsRef<str>) {
    if let Some(state) = STATE.get()
        && let Ok(state) = state.lock()
        && level < state.level
    {
        return;
    }
    let label = match level {
        Level::Debug => "DEBUG",
        Level::Info => "INFO",
        Level::Warn => "WARN",
        Level::Error => "ERROR",
    };
    let line = format!("[{}] [{label}] {}", now(), message.as_ref());
    eprintln!("{line}");

    let Some(state) = STATE.get() else {
        return;
    };
    let Ok(state) = state.lock() else {
        return;
    };
    if let Some(path) = &state.debug_file
        && let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_newest_fifteen_log_files() {
        let directory =
            std::env::temp_dir().join(format!("teeforge-logs-{}-{}", std::process::id(), now()));
        fs::create_dir_all(&directory).expect("create log fixture");
        for index in 0..18 {
            fs::write(directory.join(format!("teeforge_{index:02}.log")), b"log")
                .expect("write log fixture");
        }
        fs::write(directory.join("unrelated.txt"), b"keep").expect("write unrelated fixture");
        cleanup_old_logs(&directory);
        let remaining = fs::read_dir(&directory)
            .expect("read fixture")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".log"))
            .count();
        assert_eq!(remaining, 15);
        assert!(directory.join("unrelated.txt").is_file());
        let _ = fs::remove_dir_all(directory);
    }
}
