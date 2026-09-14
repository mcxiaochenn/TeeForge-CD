use crate::error::{Result, TfError};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreservedMetadata {
    #[cfg(unix)]
    uid: u32,
    #[cfg(unix)]
    gid: u32,
    #[cfg(unix)]
    mode: u32,
    #[cfg(not(unix))]
    readonly: bool,
    #[cfg(target_os = "android")]
    selinux: Vec<u8>,
}

impl PreservedMetadata {
    pub(crate) fn read(path: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file() {
            return Err(TfError::new(
                "OMK 配置必须是普通文件 [OMK config must be a regular file]",
            ));
        }
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        #[cfg(target_os = "android")]
        let selinux = {
            use std::os::fd::AsRawFd;
            let source = fs::File::open(path)?;
            let mut label = vec![0u8; 4096];
            // 文件句柄和缓冲区在调用期间有效；失败时不替换文件。
            // The descriptor and buffer remain valid; failure prevents replacement.
            let size = unsafe {
                libc::fgetxattr(
                    source.as_raw_fd(),
                    c"security.selinux".as_ptr(),
                    label.as_mut_ptr().cast(),
                    label.len(),
                )
            };
            if size < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            label.truncate(size as usize);
            label
        };
        Ok(Self {
            #[cfg(unix)]
            uid: metadata.uid(),
            #[cfg(unix)]
            gid: metadata.gid(),
            #[cfg(unix)]
            mode: metadata.mode() & 0o7777,
            #[cfg(not(unix))]
            readonly: metadata.permissions().readonly(),
            #[cfg(target_os = "android")]
            selinux,
        })
    }

    fn apply(&self, file: &fs::File) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            use std::os::unix::fs::PermissionsExt;
            // fchown 只作用于本轮新建且仍持有的临时文件。
            // fchown targets only this operation's open temporary file.
            if unsafe { libc::fchown(file.as_raw_fd(), self.uid, self.gid) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            file.set_permissions(fs::Permissions::from_mode(self.mode))?;
        }
        #[cfg(not(unix))]
        {
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(self.readonly);
            file.set_permissions(permissions)?;
        }
        #[cfg(target_os = "android")]
        {
            use std::os::fd::AsRawFd;
            // 标签来自原文件；不猜测或硬编码 SELinux 上下文。
            // Copy the source label rather than guessing a SELinux context.
            if unsafe {
                libc::fsetxattr(
                    file.as_raw_fd(),
                    c"security.selinux".as_ptr(),
                    self.selinux.as_ptr().cast(),
                    self.selinux.len(),
                    0,
                )
            } != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        Ok(())
    }
}

pub(crate) fn write_preserved(
    path: &Path,
    data: &[u8],
    metadata: &PreservedMetadata,
) -> Result<()> {
    // 不创建 OMK 目录；保留原文件元数据后才公开替换结果。
    // Never create OMK directories; prepare source metadata before publishing.
    for attempt in 0..32 {
        let temporary = temp_path(path, attempt)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let result = (|| -> Result<()> {
            file.write_all(data)?;
            metadata.apply(&file)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return result;
    }
    Err(TfError::new(
        "无法创建 OMK 临时文件 [Cannot create OMK temporary file]",
    ))
}

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

    // 临时文件必须独占创建；写入并落盘成功后才替换目标，失败则清理本轮临时文件。
    // Create the temporary file exclusively and replace the target only after a synced write.
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
        // 备份保存替换前的完整内容，供上层事务在后续目标失败时恢复。
        // The backup preserves the full pre-replacement content for higher-level rollback.
        let old = fs::read(path).map_err(|e| TfError::from(e).context(path.display()))?;
        let backup = PathBuf::from(format!("{}.bak", path.display()));
        write(&backup, &old)?;
    }
    write(path, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn preserves_unix_owner_mode_and_cleans_failed_temporary_file() -> Result<()> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let dir =
            std::env::temp_dir().join(format!("teeforge-unix-metadata-{}", std::process::id()));
        fs::create_dir_all(&dir)?;
        let path = dir.join("injector.toml");
        fs::write(&path, "scoop = []")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        let metadata = PreservedMetadata::read(&path)?;
        write_preserved(&path, b"scoop = ['new.app']", &metadata)?;
        let after = fs::metadata(&path)?;
        assert_eq!(after.uid(), metadata.uid);
        assert_eq!(after.gid(), metadata.gid);
        assert_eq!(after.mode() & 0o7777, 0o600);
        let blocked = dir.join("directory");
        fs::create_dir(&blocked)?;
        assert!(write_preserved(&blocked, b"must not replace directory", &metadata).is_err());
        assert_eq!(fs::read_dir(&dir)?.count(), 2);
        assert_eq!(fs::read(&path)?, b"scoop = ['new.app']");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

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
