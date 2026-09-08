use crate::error::{Result, TfError};
use std::ffi::OsStr;
use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_STDERR_CHARS: usize = 512;
const MAX_CAPTURED_STDERR_BYTES: usize = 16 * 1024;
const WAIT_INTERVAL: Duration = Duration::from_millis(10);

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

struct BoundedRead {
    data: Vec<u8>,
    overflowed: bool,
}

fn read_bounded(
    mut reader: impl Read,
    limit: usize,
    overflow_signal: Option<&AtomicBool>,
) -> io::Result<BoundedRead> {
    let mut data = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0_u8; 8192];
    let mut overflowed = false;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(data.len());
        let retained = remaining.min(count);
        data.extend_from_slice(&buffer[..retained]);
        overflowed |= retained < count;
        if overflowed && let Some(signal) = overflow_signal {
            signal.store(true, Ordering::Release);
        }
    }
    Ok(BoundedRead { data, overflowed })
}

fn join_reader(handle: JoinHandle<io::Result<BoundedRead>>, context: &str) -> Result<BoundedRead> {
    handle
        .join()
        .map_err(|_| TfError::new(format!("{context}，读取线程异常 [reader thread failed]")))?
        .map_err(|error| TfError::from(error).context(context))
}

pub(crate) fn output_bounded<I, S>(
    program: &str,
    args: I,
    timeout: Duration,
    stdout_limit: usize,
    context: &str,
) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            TfError::new(format!(
                "{context}，无法启动 {program} [failed to start {program}]: {error}"
            ))
        })?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(TfError::new(format!(
            "{context}，无法读取 stdout [stdout unavailable]"
        )));
    };
    let Some(stderr) = child.stderr.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(TfError::new(format!(
            "{context}，无法读取 stderr [stderr unavailable]"
        )));
    };
    // 同时排空两个管道，避免子进程因任一缓冲区写满而无法退出。
    // Drain both pipes concurrently so neither full buffer can block child exit.
    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let reader_overflow = Arc::clone(&stdout_overflow);
    let stdout_reader =
        thread::spawn(move || read_bounded(stdout, stdout_limit, Some(&reader_overflow)));
    let stderr_reader =
        thread::spawn(move || read_bounded(stderr, MAX_CAPTURED_STDERR_BYTES, None));
    let started = Instant::now();

    let status = loop {
        if stdout_overflow.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = join_reader(stdout_reader, context);
            let _ = join_reader(stderr_reader, context);
            return Err(TfError::new(format!(
                "{context}，输出超过限制 [output exceeds limit]: {stdout_limit} bytes"
            )));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                thread::sleep(WAIT_INTERVAL.min(timeout.saturating_sub(started.elapsed())));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = join_reader(stdout_reader, context);
                let _ = join_reader(stderr_reader, context);
                return Err(TfError::new(format!(
                    "{context}，执行超时 [timed out]: {}s",
                    timeout.as_secs()
                )));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = join_reader(stdout_reader, context);
                let _ = join_reader(stderr_reader, context);
                return Err(TfError::new(format!(
                    "{context}，无法等待子进程 [failed to wait for child]: {error}"
                )));
            }
        }
    };
    let stdout = join_reader(stdout_reader, context);
    let stderr = join_reader(stderr_reader, context);
    let stdout = stdout?;
    let stderr = stderr?;
    if stdout.overflowed {
        return Err(TfError::new(format!(
            "{context}，输出超过限制 [output exceeds limit]: {stdout_limit} bytes"
        )));
    }
    Ok(Output {
        status,
        stdout: stdout.data,
        stderr: stderr.data,
    })
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
    use std::time::Duration;

    fn shell_output(command: &str, limit: usize, timeout: Duration) -> Result<Output> {
        if cfg!(windows) {
            output_bounded(
                "cmd",
                ["/C", command],
                timeout,
                limit,
                "测试命令 [test command]",
            )
        } else {
            output_bounded(
                "sh",
                ["-c", command],
                timeout,
                limit,
                "测试命令 [test command]",
            )
        }
    }

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
            shell_output(
                "echo package manager unavailable 1>&2 & exit /B 7",
                1024,
                Duration::from_secs(2),
            )
        } else {
            shell_output(
                "echo package manager unavailable >&2; exit 7",
                1024,
                Duration::from_secs(2),
            )
        }
        .expect("run failing command");
        let error = stdout_text(output, "获取包列表失败 [Failed to list packages]")
            .expect_err("non-zero command must fail")
            .to_string();
        assert!(error.contains("exit code 7"));
        assert!(error.contains("package manager unavailable"));
    }

    #[test]
    fn bounded_command_returns_stdout_and_stderr() {
        let command = if cfg!(windows) {
            "echo package-list & echo diagnostic 1>&2"
        } else {
            "printf 'package-list\\n'; printf 'diagnostic\\n' >&2"
        };
        let output = shell_output(command, 1024, Duration::from_secs(2)).expect("bounded output");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("package-list"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("diagnostic"));
    }

    #[test]
    fn bounded_command_rejects_stdout_over_limit() {
        let command = if cfg!(windows) {
            "echo 123456789"
        } else {
            "printf '123456789\\n'"
        };
        let error = shell_output(command, 4, Duration::from_secs(2))
            .expect_err("oversized stdout must fail")
            .to_string();
        assert!(error.contains("输出超过限制 [output exceeds limit]"));
    }

    #[test]
    fn bounded_command_times_out_and_reaps_child() {
        let command = if cfg!(windows) {
            "ping -n 3 127.0.0.1 >NUL"
        } else {
            "sleep 2"
        };
        let error = shell_output(command, 1024, Duration::from_millis(50))
            .expect_err("slow command must time out")
            .to_string();
        assert!(error.contains("执行超时 [timed out]"));
    }

    #[test]
    fn bounded_stderr_capture_is_truncated() {
        let input = vec![b'x'; MAX_CAPTURED_STDERR_BYTES + 128];
        let captured = read_bounded(input.as_slice(), MAX_CAPTURED_STDERR_BYTES, None)
            .expect("read bounded stderr");
        assert_eq!(captured.data.len(), MAX_CAPTURED_STDERR_BYTES);
        assert!(captured.overflowed);
    }

    #[test]
    fn bounded_command_reports_spawn_failure() {
        let error = output_bounded(
            "teeforge-command-that-does-not-exist",
            std::iter::empty::<&str>(),
            Duration::from_secs(1),
            1024,
            "测试命令 [test command]",
        )
        .expect_err("missing command must fail")
        .to_string();
        assert!(error.contains("无法启动 teeforge-command-that-does-not-exist"));
    }
}
