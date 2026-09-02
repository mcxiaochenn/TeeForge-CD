use crate::error::{Result, TfError};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static TEMP_COUNTER: AtomicU32 = AtomicU32::new(0);

fn temp_path(path: &Path, attempt: u32) -> Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| TfError::new("目标路径没有文件名 [Target path has no file name]"))?
        .to_string_lossy();
    Ok(path.with_file_name(format!(
        ".{name}.tmp.{}.{}.{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
        attempt
    )))
}

pub(crate) fn write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| TfError::new("目标路径没有父目录 [Target path has no parent]"))?;
    fs::create_dir_all(parent).map_err(|e| TfError::from(e).context(parent.display()))?;

    for attempt in 0..32 {
        let temporary = temp_path(path, attempt)?;
        let opened = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary);
        let mut file = match opened {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(TfError::from(error).context(temporary.display())),
        };

        let result = (|| -> Result<()> {
            file.write_all(data)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path).map_err(|e| TfError::from(e).context(path.display()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return result;
    }

    Err(TfError::new(
        "无法创建原子写入临时文件 [Unable to create atomic-write temporary file]",
    ))
}

pub(crate) fn write_with_backup(path: &Path, data: &[u8]) -> Result<()> {
    if path.is_file() {
        let old = fs::read(path).map_err(|e| TfError::from(e).context(path.display()))?;
        let backup = PathBuf::from(format!("{}.bak", path.display()));
        write(&backup, &old)?;
    }
    write(path, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_file_and_keeps_backup() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("teeforge-atomic-{}", std::process::id()));
        fs::create_dir_all(&dir)?;
        let path = dir.join("value.txt");
        fs::write(&path, b"old")?;
        write_with_backup(&path, b"new")?;
        assert_eq!(fs::read(&path)?, b"new");
        assert_eq!(fs::read(dir.join("value.txt.bak"))?, b"old");
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }
}
