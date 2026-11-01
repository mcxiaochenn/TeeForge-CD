use std::path::{Path, PathBuf};
use std::process::Command;

const CANDIDATES: &[&str] = &[
    "/data/adb/ksu/bin/resetprop",
    "/data/adb/ap/bin/resetprop",
    "/data/adb/magisk/resetprop",
];

pub struct Dump {
    pub entries: Vec<(String, String)>,
    pub source: &'static str,
}

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

/// 读取属性基线。`getprop` 优先：格式固定、不依赖各 root 管理器的 resetprop 实现。
/// 部分 KernelSU 分支的 resetprop 无参数执行不输出属性列表，只能拿到空 dump。
/// Read the property baseline. `getprop` comes first: its format is stable and it does
/// not depend on the root manager's resetprop build; some KernelSU forks print nothing
/// when resetprop is invoked without arguments.
pub fn dump() -> Result<Dump, String> {
    let mut errors = Vec::new();

    match dump_getprop() {
        Ok(entries) if !entries.is_empty() => {
            return Ok(Dump {
                entries,
                source: "getprop",
            });
        }
        Ok(_) => errors.push("getprop 输出为空 [getprop returned nothing]".to_owned()),
        Err(error) => errors.push(error),
    }

    let tool = find_tool();
    match dump_resetprop(&tool) {
        Ok(entries) if !entries.is_empty() => {
            return Ok(Dump {
                entries,
                source: "resetprop",
            });
        }
        Ok(_) => errors.push(format!(
            "{} 输出为空 [{} returned nothing]",
            tool.display(),
            tool.display()
        )),
        Err(error) => errors.push(error),
    }

    Err(errors.join("; "))
}

fn dump_getprop() -> Result<Vec<(String, String)>, String> {
    let output = Command::new("getprop")
        .output()
        .map_err(|error| format!("无法执行 getprop: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "getprop 退出码 {} [exit code {}]",
            output.status.code().unwrap_or(128),
            output.status.code().unwrap_or(128)
        ));
    }
    Ok(parse_getprop(&String::from_utf8_lossy(&output.stdout)))
}

fn dump_resetprop(tool: &Path) -> Result<Vec<(String, String)>, String> {
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
    let mut entries = parse_dump(&String::from_utf8_lossy(&output.stdout));
    if entries.is_empty() {
        // 少数实现把列表写到 stderr；两边都空才判定为空。
        // A few builds write the list to stderr; treat it as empty only when both are.
        entries = parse_dump(&String::from_utf8_lossy(&output.stderr));
    }
    Ok(entries)
}

/// `getprop` 全量输出格式：`[ro.boot.flash.locked]: [0]`
/// Full `getprop` output looks like `[ro.boot.flash.locked]: [0]`.
pub fn parse_getprop(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix('[')?;
            let (name, rest) = rest.split_once("]: [")?;
            let value = rest.strip_suffix(']')?;
            (!name.is_empty()).then(|| (name.to_owned(), value.to_owned()))
        })
        .collect()
}

/// `resetprop` 全量输出格式：`ro.boot.flash.locked=0`
/// Full `resetprop` output looks like `ro.boot.flash.locked=0`.
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
    match run(tool, &["-n", name, value]) {
        Ok(()) => Ok(()),
        // 部分 resetprop 不接受 -n；不带该参数时 ro. 属性仍直接写属性区。
        // Some builds reject -n; without it ro. properties are still written directly.
        Err(with_flag) => match run(tool, &[name, value]) {
            Ok(()) => Ok(()),
            Err(without_flag) => Err(format!("带 -n: {with_flag}；不带 -n: {without_flag}")),
        },
    }
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
    fn parses_getprop_brackets() {
        let parsed = parse_getprop(
            "[ro.boot.flash.locked]: [0]\n[ro.build.type]: [user]\n[empty]: []\nmalformed\n",
        );
        assert_eq!(
            parsed,
            vec![
                ("ro.boot.flash.locked".to_owned(), "0".to_owned()),
                ("ro.build.type".to_owned(), "user".to_owned()),
                ("empty".to_owned(), String::new()),
            ]
        );
    }

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
    fn empty_lists_stay_empty() {
        assert!(parse_getprop("").is_empty());
        assert!(parse_dump("").is_empty());
    }

    #[test]
    fn missing_tool_reports_spawn_failure() {
        let error = run(Path::new("teeforge-resetprop-does-not-exist"), &["a", "b"])
            .expect_err("missing tool must fail");
        assert!(error.contains("无法执行"));
    }
}
