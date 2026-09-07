use crate::error::{Result, TfError};
use std::ffi::OsStr;
use std::process::{Command, Output, Stdio};

const MAX_STDERR_CHARS: usize = 512;

pub(crate) fn output<I, S>(program: &str, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(program)
        .args(args)
        .output()
        .map_err(|e| TfError::from(e).context(program))
}

pub(crate) fn status<I, S>(program: &str, args: I) -> Result<i32>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .status()
        .map_err(|e| TfError::from(e).context(program))?;
    Ok(status.code().unwrap_or(128))
}

pub(crate) fn stdout_text(output: Output, context: &str) -> Result<String> {
    if !output.status.success() {
        // 外部命令输出不可信；只保留单行且有长度上限的错误摘要。
        // External command output is untrusted; retain only a bounded one-line summary.
        let stderr = stderr_summary(&output.stderr);
        let detail = (!stderr.is_empty()).then(|| format!(": {stderr}"));
        let code = output.status.code().unwrap_or(128);
        return Err(TfError::new(format!(
            "{context}，退出码 {code} [exit code {code}]{}",
            detail.unwrap_or_default()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| TfError::new(format!("{context} 输出不是 UTF-8 [output is not UTF-8]")))
}

fn stderr_summary(stderr: &[u8]) -> String {
    let normalized = String::from_utf8_lossy(stderr)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = normalized.chars();
    let summary = chars.by_ref().take(MAX_STDERR_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{summary}...")
    } else {
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stderr_summary_is_single_line_and_bounded() {
        assert_eq!(
            stderr_summary(b"  first\r\n second\tthird  "),
            "first second third"
        );
        let summary = stderr_summary("错".repeat(MAX_STDERR_CHARS + 1).as_bytes());
        assert_eq!(summary.chars().count(), MAX_STDERR_CHARS + 3);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn failed_command_reports_exit_code_and_stderr() {
        let output = if cfg!(windows) {
            Command::new("cmd")
                .args(["/C", "echo package manager unavailable 1>&2 & exit /B 7"])
                .output()
        } else {
            Command::new("sh")
                .args(["-c", "echo package manager unavailable >&2; exit 7"])
                .output()
        }
        .expect("run failing command");
        let error = stdout_text(output, "获取包列表失败 [Failed to list packages]")
            .expect_err("non-zero command must fail")
            .to_string();
        assert!(error.contains("exit code 7"));
        assert!(error.contains("package manager unavailable"));
    }
}
