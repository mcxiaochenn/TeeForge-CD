use crate::atomic_file;
use crate::config::{Config, KeyboxSource};
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MAX_DOWNLOAD: usize = 2 * 1024 * 1024;

fn without_ascii_whitespace(data: &[u8]) -> Cow<'_, [u8]> {
    if data.iter().any(u8::is_ascii_whitespace) {
        Cow::Owned(
            data.iter()
                .copied()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect(),
        )
    } else {
        Cow::Borrowed(data)
    }
}

fn decode_text(encoded: &str, label: &str) -> Result<String> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| TfError::new(format!("{label} base64 解码失败 [decode failed]")))?;
    String::from_utf8(bytes).map_err(|_| TfError::new(format!("{label} 不是 UTF-8 [is not UTF-8]")))
}

fn decode_transport_base64(data: &[u8], error: impl Into<String>) -> Result<Vec<u8>> {
    let normalized = without_ascii_whitespace(data);
    STANDARD
        .decode(normalized.as_ref())
        .map_err(|_| TfError::new(error))
}

fn endpoints() -> Result<(String, String)> {
    let cdn = decode_text(
        &[
            "aHR0cHM6Ly90",
            "ZWVmb3JnZS5t",
            "Y3hpYW9jaGVu",
            "LnRvcC9maWxl",
            "cy9rZXlib3gv",
        ]
        .concat(),
        "CDN URL",
    )?;
    let public_key = decode_text(
        &[
            "c3NoLWVkMjU1MTkg",
            "QUFBQUMzTnphQzFs",
            "WkRJMU5URTVBQUFB",
            "SUU5K1J3NVhadklV",
            "bW1Wc3pKR0FZRHIwV0krRWp6cHFnSCtVZ0NwL05pZlM=",
        ]
        .concat(),
        "Public key",
    )?;
    Ok((cdn, public_key))
}

fn current_month() -> Result<String> {
    let output = process::output("date", ["+%Y-%m"])?;
    let month = process::stdout_text(output, "无法读取系统月份 [Unable to read system month]")?;
    validate_month(month.trim())
}

fn validate_month(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    let month = value.get(5..).and_then(|part| part.parse::<u8>().ok());
    if bytes.len() == 7
        && bytes[4] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..].iter().all(u8::is_ascii_digit)
        && month.is_some_and(|month| (1..=12).contains(&month))
    {
        return Ok(value.to_owned());
    }
    Err(TfError::new(format!(
        "系统月份格式无效 [Invalid system month format]: {value}"
    )))
}

fn slot_filename(month: &str, slot: u8) -> Result<String> {
    validate_month(month)?;
    if !(1..=3).contains(&slot) {
        return Err(TfError::new("远程槽位无效 [Invalid remote slot]"));
    }
    let input = if slot == 1 {
        month.to_owned()
    } else {
        format!("{month}_slot_{slot}")
    };
    let digest = Sha256::digest(input.as_bytes());
    Ok(digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(crate) fn read_limited(mut reader: impl Read) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    reader
        .by_ref()
        .take((MAX_DOWNLOAD + 1) as u64)
        .read_to_end(&mut data)?;
    if data.is_empty() {
        return Err(TfError::new("下载结果为空 [Downloaded response is empty]"));
    }
    if data.len() > MAX_DOWNLOAD {
        return Err(TfError::new(
            "下载结果超过 2 MiB 限制 [Downloaded response exceeds the 2 MiB limit]",
        ));
    }
    Ok(data)
}

fn try_download(program: &Path, args: &[&str], url: &str) -> Result<Vec<u8>> {
    let mut command = Command::new(program);
    command
        .args(args)
        .arg(url)
        .stderr(Stdio::null())
        .stdout(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| TfError::from(error).context(program.display()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| TfError::new("无法读取下载输出 [Unable to read download output]"))?;
    let data = read_limited(stdout);
    if data.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(TfError::new(format!(
            "下载工具退出码 {} [Downloader exit code {}]",
            status.code().unwrap_or(128),
            status.code().unwrap_or(128)
        )));
    }
    data
}

fn download(url: &str) -> Result<Vec<u8>> {
    let mut tools: Vec<(PathBuf, Vec<&str>, &str)> = vec![
        ("wget".into(), vec!["-qO-"], "wget"),
        ("curl".into(), vec!["-fsSL"], "curl"),
    ];
    for (dir, label) in [
        ("/data/adb/ksu/bin", "KernelSU busybox wget"),
        ("/data/adb/ap/bin", "APatch busybox wget"),
        ("/data/adb/magisk", "Magisk busybox wget"),
    ] {
        if Path::new(dir).is_dir() {
            tools.push((Path::new(dir).join("busybox"), vec!["wget", "-qO-"], label));
        }
    }
    download_with(tools, |program, args| try_download(program, args, url))
}

fn download_with(
    tools: Vec<(PathBuf, Vec<&str>, &str)>,
    mut attempt: impl FnMut(&Path, &[&str]) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut errors = Vec::new();
    for (program, args, label) in tools {
        match attempt(&program, &args) {
            Ok(data) => {
                logging::log(Level::Info, format!("下载工具 [Download tool]: {label}"));
                return Ok(data);
            }
            Err(error) => errors.push(format!("{label}: {error}")),
        }
    }
    Err(TfError::new(format!(
        "所有下载工具均失败 [All download tools failed]: {}",
        errors.join("; ")
    )))
}

fn decode_hex(data: &[u8]) -> Result<Vec<u8>> {
    let data = without_ascii_whitespace(data);
    if data.is_empty() || data.len() % 2 != 0 {
        return Err(TfError::new("hex 长度无效 [Invalid hex length]"));
    }
    fn nibble(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }
    data.chunks_exact(2)
        .map(|pair| {
            let high = nibble(pair[0])
                .ok_or_else(|| TfError::new("非法 hex 字符 [Invalid hex character]"))?;
            let low = nibble(pair[1])
                .ok_or_else(|| TfError::new("非法 hex 字符 [Invalid hex character]"))?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn decrypt(encrypted: &[u8], public_key: &str) -> Result<Vec<u8>> {
    let mut current = decode_transport_base64(
        encrypted,
        "首次 base64 解码失败 [First base64 decode failed]",
    )?;
    let key = Sha256::digest(public_key.as_bytes());
    for (index, byte) in current.iter_mut().enumerate() {
        *byte ^= key[index % key.len()];
    }
    for round in 1..=10 {
        current = decode_transport_base64(
            &current,
            format!("base64 第 {round} 层无效 [Invalid base64 at layer {round}]"),
        )?;
    }
    current = decode_hex(&current)?;
    for byte in &mut current {
        *byte = match *byte {
            b'a'..=b'm' | b'A'..=b'M' => *byte + 13,
            b'n'..=b'z' | b'N'..=b'Z' => *byte - 13,
            _ => *byte,
        };
    }
    crate::keybox_format::validate(&current)?;
    Ok(current)
}

struct SavedFile {
    bytes: Option<Vec<u8>>,
    metadata: Option<atomic_file::PreservedMetadata>,
}

impl SavedFile {
    fn read(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(bytes) => Ok(Self {
                bytes: Some(bytes),
                metadata: Some(atomic_file::PreservedMetadata::read(path)?),
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                bytes: None,
                metadata: None,
            }),
            Err(error) => Err(error.into()),
        }
    }

    fn restore(&self, path: &Path) -> Result<()> {
        match (&self.bytes, &self.metadata) {
            (Some(bytes), Some(metadata)) => atomic_file::write_preserved(path, bytes, metadata),
            _ => match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            },
        }
    }
}

struct KeyboxUpdate {
    path: PathBuf,
    data: Vec<u8>,
    original: SavedFile,
    backup: Option<(PathBuf, SavedFile)>,
    private: bool,
}

fn selection_bytes(path: &Path, config: &Config) -> Result<Vec<u8>> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let mut lines: Vec<String> = text
        .lines()
        .filter(|line| {
            !line.split_once('=').is_some_and(|(key, _)| {
                matches!(key.trim(), "keybox_source" | "keybox_remote_slot")
            })
        })
        .map(str::to_owned)
        .collect();
    lines.push(format!("keybox_source={}", config.keybox_source.label()));
    lines.push(format!("keybox_remote_slot={}", config.keybox_remote_slot));
    Ok(format!("{}\n", lines.join("\n")).into_bytes())
}

fn install_keyboxes_with(
    paths: &[PathBuf],
    decoded: &[u8],
    force: bool,
    selection: Option<(&Path, &Config)>,
    mut write: impl FnMut(&Path, &[u8], bool, bool) -> Result<()>,
) -> Result<Vec<(PathBuf, bool)>> {
    let mut updates = Vec::new();
    let mut targets = Vec::new();
    for path in paths {
        let original = SavedFile::read(path)?;
        let changed = force || original.bytes.as_deref() != Some(decoded);
        targets.push((path.clone(), changed));
        if changed {
            let backup = if original.bytes.is_some() {
                let path = PathBuf::from(format!("{}.bak", path.display()));
                let saved = SavedFile::read(&path)?;
                Some((path, saved))
            } else {
                None
            };
            updates.push(KeyboxUpdate {
                path: path.clone(),
                data: decoded.to_vec(),
                original,
                backup,
                private: true,
            });
        }
    }
    if let Some((path, config)) = selection {
        if paths.iter().any(|target| target == path) {
            return Err(TfError::new(
                "配置路径不能与 Keybox 目标相同 [Config overlaps keybox target]",
            ));
        }
        let original = SavedFile::read(path)?;
        let data = selection_bytes(path, config)?;
        if original.bytes.as_deref() != Some(data.as_slice()) {
            updates.push(KeyboxUpdate {
                path: path.into(),
                data,
                original,
                backup: None,
                private: false,
            });
        }
    }
    for (index, update) in updates.iter().enumerate() {
        let result = write(
            &update.path,
            &update.data,
            update.private,
            update.backup.is_some(),
        )
        .and_then(|()| {
            let actual = fs::read(&update.path)?;
            if actual != update.data {
                return Err(TfError::staged(
                    "keybox",
                    "readback",
                    "写入回读不一致 [Written file verification failed]",
                ));
            }
            Ok(())
        });
        if let Err(error) = result {
            let mut rollback_errors = Vec::new();
            // 包含本次失败的写入，因为文件可能已替换但回读失败。
            // Include the failing write: replacement may have succeeded before readback.
            for previous in updates[..=index].iter().rev() {
                if let Err(e) = previous.original.restore(&previous.path) {
                    rollback_errors.push(e.to_string());
                }
                if let Some((path, saved)) = &previous.backup {
                    if let Err(e) = saved.restore(path) {
                        rollback_errors.push(e.to_string());
                    }
                }
            }
            if !rollback_errors.is_empty() {
                return Err(TfError::new(format!(
                    "Keybox 回滚失败 [Rollback failed]: {error}; {}",
                    rollback_errors.join("; ")
                )));
            }
            return Err(error);
        }
    }
    Ok(targets)
}

fn install_keyboxes_options(
    paths: &[PathBuf],
    decoded: &[u8],
    force: bool,
    selection: Option<(&Path, &Config)>,
) -> Result<Vec<(PathBuf, bool)>> {
    install_keyboxes_with(
        paths,
        decoded,
        force,
        selection,
        |path, bytes, private, backup| match (private, backup) {
            (true, true) => atomic_file::write_private_with_backup(path, bytes),
            (true, false) => atomic_file::write_private(path, bytes),
            (false, _) => atomic_file::write(path, bytes),
        },
    )
}

#[cfg(test)]
fn install_keyboxes(paths: &[PathBuf], decoded: &[u8]) -> Result<Vec<(PathBuf, bool)>> {
    install_keyboxes_options(paths, decoded, false, None)
}

#[cfg(test)]
fn install_keybox(local: &Path, destination: &Path, decoded: &[u8]) -> Result<()> {
    install_keyboxes(&[local.to_path_buf(), destination.to_path_buf()], decoded).map(|_| ())
}

/// 安装结果：来源与每个目标的写入状态，供 JSON 输出使用。
pub(crate) struct InstallReport {
    pub(crate) source: KeyboxSource,
    pub(crate) remote_slot: Option<u8>,
    /// (路径, 是否发生写入) [path and whether it changed]
    pub(crate) targets: Vec<(PathBuf, bool)>,
}

pub(crate) fn fetch(config: &Config, force: bool, save: Option<&Path>) -> Result<InstallReport> {
    logging::log(Level::Info, "开始获取 keybox [Starting keybox fetch]...");
    let decoded = match config.keybox_source {
        KeyboxSource::Remote => remote_keybox(config.keybox_remote_slot, force)?,
        KeyboxSource::Rkp => rkp_keybox(config)?,
    };
    let targets = install(config, &decoded, force, save)?;
    logging::log(
        Level::Info,
        "Keybox 已更新并同步 [Keybox updated and synchronized]",
    );
    Ok(InstallReport {
        source: config.keybox_source,
        remote_slot: (config.keybox_source == KeyboxSource::Remote)
            .then_some(config.keybox_remote_slot),
        targets,
    })
}

/// 读取本机 RKP 产物；不联网、不回退。
fn rkp_keybox(config: &Config) -> Result<Vec<u8>> {
    let path = config.rkp_dir.join("keybox.xml");
    let bytes = match fs::read(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TfError::staged(
                "keybox",
                "rkp_missing",
                "本机 RKP 产物不存在，请先在 RKP 页面签发 [RKP artifacts missing]",
            )
            .with_hint("keybox.error.rkp_missing")
            .with_raw(serde_json::json!({ "path": path.display().to_string() })));
        }
        Err(error) => {
            return Err(TfError::staged(
                "keybox",
                "rkp_unreadable",
                format!("无法读取 RKP 产物 [cannot read RKP artifacts]: {error}"),
            )
            .with_raw(serde_json::json!({ "path": path.display().to_string() })));
        }
    };
    crate::keybox_format::validate(&bytes).map_err(|_| {
        TfError::staged(
            "keybox",
            "rkp_invalid",
            "RKP 产物格式无效 [Invalid RKP artifact]",
        )
        .with_hint("keybox.error.rkp_invalid")
    })?;
    Ok(bytes)
}

fn remote_keybox(slot: u8, force: bool) -> Result<Vec<u8>> {
    let (cdn, public_key) = endpoints()?;
    let mut url = format!("{cdn}{}", slot_filename(&current_month()?, slot)?);
    if force {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| TfError::new("系统时间无效 [Invalid system time]"))?
            .as_nanos();
        url.push_str(&format!("?refresh={nonce}"));
    }
    let encrypted = download(&url)?;
    decrypt(&encrypted, &public_key)
}

fn install(
    config: &Config,
    decoded: &[u8],
    force: bool,
    save: Option<&Path>,
) -> Result<Vec<(PathBuf, bool)>> {
    crate::keybox_format::validate(decoded)?;
    let mut destinations = Vec::new();
    if config
        .target_txt
        .parent()
        .is_some_and(|parent| parent.is_dir())
    {
        destinations.push(config.target_txt.parent().unwrap().join("keybox.xml"));
    }
    if config.teesim_config.is_file() {
        if let Some(parent) = config.teesim_config.parent() {
            destinations.push(parent.join("keybox.xml"));
        }
    }
    destinations.sort();
    destinations.dedup();
    if destinations.is_empty() {
        return Err(TfError::new(
            "未发现兼容目标目录，保留现有 Keybox [No compatible target directory found; existing Keybox preserved]",
        ));
    }
    fs::create_dir_all(&config.keybox_dir)?;
    let local = config.keybox_dir.join("keybox.xml");
    destinations.insert(0, local);
    destinations.sort();
    destinations.dedup();
    install_keyboxes_options(
        &destinations,
        decoded,
        force,
        save.map(|path| (path, config)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &[u8] = include_bytes!("../../../tests/fixtures/keybox-valid.xml");

    #[test]
    fn slots_are_distinct_and_slot_one_is_backward_compatible() {
        let month = "2026-10";
        let legacy = format!("{:x}", Sha256::digest(month.as_bytes()));
        assert_eq!(slot_filename(month, 1).unwrap(), &legacy[..12]);
        let names = (1..=3)
            .map(|slot| slot_filename(month, slot).unwrap())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(names.len(), 3);
        assert!(slot_filename(month, 0).is_err());
        assert!(slot_filename("2026-13", 1).is_err());
    }

    fn directory(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("teeforge-keybox-{label}-{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn force_rewrites_identical_bytes_and_saves_selection_only_at_end() {
        let dir = directory("force-selection");
        let path = dir.join("keybox.xml");
        let config_path = dir.join("config.conf");
        fs::write(&path, VALID).unwrap();
        fs::write(&config_path, "# keep\ndebug=1\nkeybox_remote_slot = 1\n").unwrap();
        let config = Config {
            keybox_remote_slot: 3,
            ..Config::default()
        };
        let normal =
            install_keyboxes_options(std::slice::from_ref(&path), VALID, false, None).unwrap();
        assert_eq!(normal, vec![(path.clone(), false)]);
        assert!(!path.with_extension("xml.bak").exists());
        let forced = install_keyboxes_options(
            std::slice::from_ref(&path),
            VALID,
            true,
            Some((&config_path, &config)),
        )
        .unwrap();
        assert_eq!(forced, vec![(path.clone(), true)]);
        assert_eq!(fs::read(path.with_extension("xml.bak")).unwrap(), VALID);
        assert_eq!(fs::read(&path).unwrap(), VALID);
        let saved = fs::read_to_string(&config_path).unwrap();
        assert!(saved.contains("# keep\ndebug=1"));
        assert_eq!(saved.matches("keybox_remote_slot").count(), 1);
        assert_eq!(Config::load(&config_path).keybox_remote_slot, 3);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn readback_or_config_failure_restores_targets_backups_and_selection() {
        for corrupt in [false, true] {
            let dir = directory(if corrupt {
                "bad-readback"
            } else {
                "bad-config"
            });
            let first = dir.join("first.xml");
            let second = dir.join("second.xml");
            let backup = first.with_extension("xml.bak");
            let config_path = dir.join("config.conf");
            fs::write(&first, b"old-first").unwrap();
            fs::write(&backup, b"older-backup").unwrap();
            fs::write(&config_path, b"debug=1\nkeybox_remote_slot=1\n").unwrap();
            let config = Config {
                keybox_remote_slot: 2,
                ..Config::default()
            };
            let result = install_keyboxes_with(
                &[first.clone(), second.clone()],
                VALID,
                true,
                Some((&config_path, &config)),
                |path, data, private, needs_backup| {
                    if needs_backup {
                        atomic_file::write_private_with_backup(path, data)?;
                    } else if private {
                        atomic_file::write_private(path, data)?;
                    } else {
                        assert_eq!(fs::read(&first).unwrap(), VALID);
                        assert_eq!(fs::read(&second).unwrap(), VALID);
                        return Err(TfError::new("injected config failure"));
                    }
                    if corrupt && path == second {
                        fs::write(path, b"corrupt")?;
                    }
                    Ok(())
                },
            );
            assert!(result.is_err());
            assert_eq!(fs::read(&first).unwrap(), b"old-first");
            assert_eq!(fs::read(&backup).unwrap(), b"older-backup");
            assert!(!second.exists());
            assert_eq!(
                fs::read(&config_path).unwrap(),
                b"debug=1\nkeybox_remote_slot=1\n"
            );
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn invalid_install_preserves_existing_files_and_config() {
        let dir = directory("invalid-install");
        let target = dir.join("backend");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keybox.xml"), b"old").unwrap();
        let config_path = dir.join("config.conf");
        let config = Config {
            target_txt: target.join("target.txt"),
            keybox_dir: dir.join("local"),
            teesim_config: dir.join("missing"),
            ..Config::default()
        };
        assert!(
            install(
                &config,
                b"<AndroidAttestation>broken",
                true,
                Some(&config_path)
            )
            .is_err()
        );
        assert_eq!(fs::read(target.join("keybox.xml")).unwrap(), b"old");
        assert!(!config_path.exists());
        assert!(!config.keybox_dir.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ci_python_to_module_decode_install_and_readback() {
        use std::io::Write;
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/scripts/keybox_sync.py");
        let dir = directory("ci-roundtrip");
        let target = dir.join("backend");
        fs::create_dir(&target).unwrap();
        for slot in 1..=3 {
            let xml = String::from_utf8(VALID.to_vec())
                .unwrap()
                .replace("synthetic-test", &format!("synthetic-slot-{slot}"));
            let rotated: Vec<u8> = xml
                .bytes()
                .map(|b| match b {
                    b'a'..=b'm' | b'A'..=b'M' => b + 13,
                    b'n'..=b'z' | b'N'..=b'Z' => b - 13,
                    _ => b,
                })
                .collect();
            let mut raw = rotated
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                .into_bytes();
            for _ in 0..10 {
                raw = STANDARD.encode(raw).into_bytes();
            }
            // 使用真正的 CI 包装函数，避免测试复制一份相同实现。
            // Exercise the actual CI wrapper rather than duplicating it in the test.
            let mut child = Command::new(if cfg!(windows) { "python" } else { "python3" })
                .args(["-c", "import runpy,sys; m=runpy.run_path(sys.argv[1]); sys.stdout.buffer.write(m['wrap'](sys.stdin.buffer.read(), b'synthetic-key'))"])
                .arg(&script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
            child.stdin.take().unwrap().write_all(&raw).unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success(), "CI wrapper failed");
            let decoded = decrypt(&output.stdout, "synthetic-key").unwrap();
            assert!(decoded == xml.as_bytes(), "decoded bytes differ");
            let config = Config {
                target_txt: target.join("target.txt"),
                keybox_dir: dir.join("local"),
                teesim_config: dir.join("missing"),
                keybox_remote_slot: slot,
                ..Config::default()
            };
            let config_path = dir.join("config.conf");
            let report = install(&config, &decoded, false, Some(&config_path)).unwrap();
            assert_eq!(report.len(), 2);
            for (path, changed) in report {
                assert!(changed);
                assert!(fs::read(path).unwrap() == xml.as_bytes());
            }
            assert_eq!(Config::load(&config_path).keybox_remote_slot, slot);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn strict_hex_rejects_partial_and_invalid_input() {
        assert!(decode_hex(b"abc").is_err());
        assert!(decode_hex(b"00xz").is_err());
        assert!(decode_hex(b" \t\r\n").is_err());
        assert_eq!(decode_hex(b"4142").expect("valid hex"), b"AB");
    }

    #[test]
    fn hex_decoder_accepts_ascii_transport_whitespace() {
        assert_eq!(decode_hex(b" \t41\r\n42\n").expect("wrapped hex"), b"AB");
    }

    #[test]
    fn month_format_is_strict() {
        assert_eq!(validate_month("2026-09").expect("valid month"), "2026-09");
        assert!(validate_month("2026-13").is_err());
        assert!(validate_month("26-09").is_err());
    }

    #[test]
    fn download_limit_rejects_oversized_input() {
        let data = vec![b'x'; MAX_DOWNLOAD + 1];
        assert!(read_limited(data.as_slice()).is_err());
    }

    #[test]
    fn downloader_falls_back_after_a_failed_tool() {
        let tools = vec![
            (PathBuf::from("first"), vec!["-q"], "first"),
            (PathBuf::from("second"), vec!["-q"], "second"),
        ];
        let mut attempts = 0;
        let result = download_with(tools, |program, _| {
            attempts += 1;
            if program == Path::new("first") {
                Err(TfError::new("mock failure"))
            } else {
                Ok(b"payload".to_vec())
            }
        })
        .expect("second downloader succeeds");
        assert_eq!(attempts, 2);
        assert_eq!(result, b"payload");
    }

    #[test]
    fn base64_decoder_rejects_invalid_characters() {
        assert!(STANDARD.decode(b"%%%=").is_err());
        assert!(decode_transport_base64(b" %%%=\r\n", "invalid").is_err());
        assert!(decode_transport_base64(b"AA=A", "invalid padding").is_err());
        assert!(decode_transport_base64(b"A", "truncated").is_err());
        assert!(
            decrypt(
                include_bytes!("../../../tests/fixtures/invalid-download.txt"),
                "test-key"
            )
            .is_err()
        );
    }

    #[test]
    fn decrypts_a_valid_contract_fixture() {
        let public_key = "ssh-ed25519 test-key";
        let xml = include_bytes!("../../../tests/fixtures/keybox-valid.xml");
        let mut encoded = xml
            .iter()
            .map(|byte| match *byte {
                b'a'..=b'm' | b'A'..=b'M' => *byte + 13,
                b'n'..=b'z' | b'N'..=b'Z' => *byte - 13,
                _ => *byte,
            })
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            .into_bytes();
        for _ in 0..10 {
            encoded = STANDARD.encode(encoded).into_bytes();
        }
        let key = Sha256::digest(public_key.as_bytes());
        for (index, byte) in encoded.iter_mut().enumerate() {
            *byte ^= key[index % key.len()];
        }
        let encrypted = format!(" \t{}\r\n", STANDARD.encode(encoded));
        assert_eq!(
            decrypt(encrypted.as_bytes(), public_key).expect("valid fixture"),
            xml
        );
    }

    #[test]
    fn decrypts_base64_with_transport_whitespace() {
        fn wrap(data: String) -> Vec<u8> {
            let mut wrapped = Vec::new();
            for (index, chunk) in data.as_bytes().chunks(48).enumerate() {
                if index > 0 {
                    wrapped.extend_from_slice(if index % 2 == 0 { b"\n" } else { b"\r\n" });
                }
                wrapped.extend_from_slice(chunk);
            }
            wrapped
        }

        let public_key = "ssh-ed25519 test-key";
        let xml = include_bytes!("../../../tests/fixtures/keybox-valid.xml");
        let mut encoded = xml
            .iter()
            .map(|byte| match *byte {
                b'a'..=b'm' | b'A'..=b'M' => *byte + 13,
                b'n'..=b'z' | b'N'..=b'Z' => *byte - 13,
                _ => *byte,
            })
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            .into_bytes();
        for _ in 0..10 {
            encoded = wrap(STANDARD.encode(encoded));
        }
        let key = Sha256::digest(public_key.as_bytes());
        for (index, byte) in encoded.iter_mut().enumerate() {
            *byte ^= key[index % key.len()];
        }
        let encrypted = STANDARD.encode(encoded);

        assert_eq!(
            decrypt(encrypted.as_bytes(), public_key).expect("wrapped base64 fixture"),
            xml
        );
    }

    #[test]
    fn failed_second_write_restores_local_keybox() {
        let directory =
            std::env::temp_dir().join(format!("teeforge-keybox-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("create keybox fixture");
        let local = directory.join("keybox.xml");
        fs::write(&local, b"old").expect("write old keybox");
        let invalid_destination = directory.join("missing").join("keybox.xml");
        fs::create_dir_all(&invalid_destination).expect("block destination parent with a file");
        assert!(install_keybox(&local, &invalid_destination, b"new").is_err());
        assert_eq!(fs::read(&local).expect("read rolled back keybox"), b"old");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn unchanged_keybox_is_not_rewritten() {
        let directory =
            std::env::temp_dir().join(format!("teeforge-keybox-unchanged-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("create keybox fixture");
        let path = directory.join("keybox.xml");
        fs::write(&path, b"same").expect("write keybox");
        install_keyboxes(std::slice::from_ref(&path), b"same").expect("skip unchanged keybox");
        assert!(!path.with_extension("xml.bak").exists());
        assert_eq!(fs::read(&path).expect("read keybox"), b"same");
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rkp_source_without_artifacts_fails_without_network() {
        let directory =
            std::env::temp_dir().join(format!("teeforge-keybox-rkp-{}", std::process::id()));
        let config = Config {
            keybox_source: KeyboxSource::Rkp,
            rkp_dir: directory.clone(),
            ..Config::default()
        };
        let error = rkp_keybox(&config).expect_err("missing artifacts must fail");
        assert_eq!(error.stage(), Some("keybox"));
        assert_eq!(error.code(), Some("rkp_missing"));

        std::fs::create_dir_all(&directory).expect("create fixture");
        std::fs::write(directory.join("keybox.xml"), b"<html/>").expect("write bad artifact");
        let error = rkp_keybox(&config).expect_err("invalid artifact must fail");
        assert_eq!(error.code(), Some("rkp_invalid"));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn keybox_is_written_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let directory =
            std::env::temp_dir().join(format!("teeforge-keybox-mode-{}", std::process::id()));
        fs::create_dir_all(&directory).expect("create keybox fixture");
        let path = directory.join("keybox.xml");
        install_keyboxes(std::slice::from_ref(&path), b"<AndroidAttestation/>")
            .expect("write keybox");
        let mode = fs::metadata(&path)
            .expect("keybox metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = fs::remove_dir_all(directory);
    }
}
