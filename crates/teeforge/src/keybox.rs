use crate::atomic_file;
use crate::config::Config;
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MAX_DOWNLOAD: usize = 2 * 1024 * 1024;

fn decode_text(encoded: &str, label: &str) -> Result<String> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| TfError::new(format!("{label} base64 解码失败 [decode failed]")))?;
    String::from_utf8(bytes).map_err(|_| TfError::new(format!("{label} 不是 UTF-8 [is not UTF-8]")))
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

fn month_filename() -> Result<String> {
    let digest = Sha256::digest(current_month()?.as_bytes());
    Ok(digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn read_limited(mut reader: impl Read) -> Result<Vec<u8>> {
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
    let mut current = STANDARD
        .decode(encrypted)
        .map_err(|_| TfError::new("首次 base64 解码失败 [First base64 decode failed]"))?;
    let key = Sha256::digest(public_key.as_bytes());
    for (index, byte) in current.iter_mut().enumerate() {
        *byte ^= key[index % key.len()];
    }
    for round in 1..=10 {
        current = STANDARD.decode(&current).map_err(|_| {
            TfError::new(format!(
                "base64 第 {round} 层无效 [Invalid base64 at layer {round}]"
            ))
        })?;
    }
    current = decode_hex(&current)?;
    for byte in &mut current {
        *byte = match *byte {
            b'a'..=b'm' | b'A'..=b'M' => *byte + 13,
            b'n'..=b'z' | b'N'..=b'Z' => *byte - 13,
            _ => *byte,
        };
    }
    if current.len() < 100
        || !current
            .windows(b"AndroidAttestation".len())
            .any(|window| window == b"AndroidAttestation")
    {
        return Err(TfError::new(
            "解码数据缺少 AndroidAttestation 标记 [Decoded data lacks AndroidAttestation marker]",
        ));
    }
    Ok(current)
}

fn install_keybox(local: &Path, destination: &Path, decoded: &[u8]) -> Result<()> {
    let previous_local = fs::read(local).ok();
    atomic_file::write_with_backup(local, decoded)?;
    if let Err(error) = atomic_file::write_with_backup(destination, decoded) {
        let rollback = match previous_local {
            Some(previous) => atomic_file::write(local, &previous),
            None => fs::remove_file(local).map_err(TfError::from),
        };
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(TfError::new(format!(
                "Keybox 同步失败且本地回滚失败 [Keybox sync and local rollback failed]: {error}; {rollback_error}"
            ))),
        };
    }
    Ok(())
}

pub(crate) fn fetch(config: &Config) -> Result<()> {
    logging::log(Level::Info, "开始获取 keybox [Starting keybox fetch]...");
    let (cdn, public_key) = endpoints()?;
    let url = format!("{cdn}{}", month_filename()?);
    let encrypted = download(&url)?;
    let decoded = decrypt(&encrypted, &public_key)?;

    let tricky_store = Path::new("/data/adb/tricky_store");
    if !tricky_store.is_dir() {
        return Err(TfError::new(
            "Tricky Store 目录不存在，保留现有 Keybox [Tricky Store directory is missing; existing Keybox preserved]",
        ));
    }
    fs::create_dir_all(&config.keybox_dir)?;
    let local = config.keybox_dir.join("keybox.xml");
    install_keybox(&local, &tricky_store.join("keybox.xml"), &decoded)?;
    logging::log(
        Level::Info,
        "Keybox 已更新并同步 [Keybox updated and synchronized]",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_hex_rejects_partial_and_invalid_input() {
        assert!(decode_hex(b"abc").is_err());
        assert!(decode_hex(b"00xz").is_err());
        assert_eq!(decode_hex(b"4142").expect("valid hex"), b"AB");
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
        let xml = b"<AndroidAttestation>abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz</AndroidAttestation>";
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
        let encrypted = STANDARD.encode(encoded);
        assert_eq!(
            decrypt(encrypted.as_bytes(), public_key).expect("valid fixture"),
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
}
