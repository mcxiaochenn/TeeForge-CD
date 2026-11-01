use std::path::{Path, PathBuf};
use std::process::Command;

const CANDIDATES: &[&str] = &[
    "/data/adb/ksu/bin/resetprop",
    "/data/adb/ap/bin/resetprop",
    "/data/adb/magisk/resetprop",
];

/// 优先使用各 root 管理器的自带工具，最后回落到 PATH。
/// Prefer each root manager's bundled tool, then fall back to PATH lookup.
pub fn find_tool() -> PathBuf {
    for path in CANDIDATES {
        if Path::new(path).is_file() {
            return PathBuf::from(path);
        }
    }
    PathBuf::from("resetprop")
}

/// 一次全量读取，避免为每条属性多 fork 一次 resetprop。
/// One full dump instead of one extra fork per property.
pub fn dump(tool: &Path) -> Result<Vec<(String, String)>, String> {
    let output = Command::new(tool)
        .output()
        .map_err(|error| format!("无法执行 {}: {error}", tool.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} 退出码 {} [exit code {}]",
            tool.display(),
            output.status.code().unwrap_or(128),
            output.status.code().unwrap_or(128)
        ));
    }
    Ok(parse_dump(&String::from_utf8_lossy(&output.stdout)))
}

pub fn parse_dump(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_owned(), value.to_owned()))
        })
        .collect()
}

pub fn set(tool: &Path, name: &str, value: &str) -> Result<(), String> {
    run(tool, &["-n", name, value])
}

pub fn delete(tool: &Path, name: &str) -> Result<(), String> {
    run(tool, &["--delete", name])
}

fn run(tool: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new(tool)
        .args(args)
        .output()
        .map_err(|error| format!("无法执行 {}: {error}", tool.display()))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.split_whitespace().collect::<Vec<_>>().join(" ");
    let detail = if detail.is_empty() {
        String::new()
    } else {
        format!(": {}", detail.chars().take(160).collect::<String>())
    };
    Err(format!(
        "退出码 {} [exit code {}]{detail}",
        output.status.code().unwrap_or(128),
        output.status.code().unwrap_or(128)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dump_lines_and_skips_noise() {
        let parsed = parse_dump("ro.boot.flash.locked=1\ninvalid line\n\nro.build.type=user\n");
        assert_eq!(
            parsed,
            vec![
                ("ro.boot.flash.locked".to_owned(), "1".to_owned()),
                ("ro.build.type".to_owned(), "user".to_owned()),
            ]
        );
    }

    #[test]
    fn keeps_empty_values() {
        let parsed = parse_dump("ro.build.selinux=\n");
        assert_eq!(parsed, vec![("ro.build.selinux".to_owned(), String::new())]);
    }

    #[test]
    fn missing_tool_reports_spawn_failure() {
        let error = dump(Path::new("teeforge-resetprop-does-not-exist"))
            .expect_err("missing tool must fail");
        assert!(error.contains("无法执行"));
    }
}
