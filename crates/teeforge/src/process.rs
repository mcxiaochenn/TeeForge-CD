use crate::error::{Result, TfError};
use std::ffi::OsStr;
use std::process::{Command, Output, Stdio};

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
        return Err(TfError::new(format!(
            "{context}，退出码 {} [{context}, exit code {}]",
            output.status.code().unwrap_or(128),
            output.status.code().unwrap_or(128)
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| TfError::new(format!("{context} 输出不是 UTF-8 [output is not UTF-8]")))
}
