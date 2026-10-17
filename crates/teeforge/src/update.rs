//! 模块更新检查与安装 [Module update check and install]
//!
//! 清单沿用 KernelSU 的 `updateJson` 字段（`version` / `versionCode` / `zipUrl` /
//! `changelog`），并额外读取 `sha256` 与 `size`。没有 `sha256` 的清单**不会**被自动安装，
//! 只会把包保存到 Download 目录由用户手动安装——避免把未校验的 ZIP 交给 root 安装器。
//!
//! Manifest follows KernelSU's updateJson plus optional `sha256` / `size`. A manifest
//! without a checksum is never auto-installed; the archive is only saved for manual use.

use crate::config::Config;
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MODULE_PROP: &str = "/data/adb/modules/teeforge_cd/module.prop";
const DOWNLOAD_DIR: &str = "/sdcard/Download";
const MAX_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024;
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

struct Manifest {
    version: String,
    version_code: u64,
    zip_url: String,
    changelog_url: Option<String>,
    sha256: Option<String>,
    size: Option<u64>,
}

struct Resolved {
    manifest: Manifest,
    endpoint: String,
    current_version: String,
    current_code: u64,
}

impl Resolved {
    fn available(&self) -> bool {
        self.manifest.version_code > self.current_code
    }
}

/// 检查更新，不写任何文件。
pub(crate) fn check_value(config: &Config) -> Result<Value> {
    let resolved = resolve(config)?;
    Ok(json!({
        "channel": config.update_channel.label(),
        "endpoint": resolved.endpoint,
        "current": {
            "version": resolved.current_version,
            "version_code": resolved.current_code,
        },
        "latest": {
            "version": resolved.manifest.version,
            "version_code": resolved.manifest.version_code,
            "zip_url": resolved.manifest.zip_url,
            "changelog_url": resolved.manifest.changelog_url,
            "sha256": resolved.manifest.sha256,
            "size": resolved.manifest.size,
        },
        "available": resolved.available(),
    }))
}

/// 下载并安装。返回安装器与结果；失败时带阶段与原因。
pub(crate) fn install_value(config: &Config) -> Result<Value> {
    let resolved = resolve(config)?;
    if !resolved.available() {
        return Ok(json!({
            "installed": false,
            "reason": "up_to_date",
            "current": resolved.current_version,
        }));
    }

    fs::create_dir_all(&config.update_dir)
        .map_err(|error| TfError::from(error).context(config.update_dir.display()))?;
    let archive = config.update_dir.join(format!(
        "teeforge_cd-{}.zip",
        resolved.manifest.version_code
    ));
    download(&resolved.manifest.zip_url, &archive)?;
    verify_archive(&archive, &resolved.manifest)?;

    let Some(checksum) = resolved.manifest.sha256.as_deref() else {
        // 没有校验和：只保存，不安装。
        let saved = save_for_manual_install(&archive, &resolved.manifest.version)?;
        logging::log(
            Level::Warn,
            format!(
                "清单缺少 sha256，未自动安装；已保存到 [saved to] {}",
                saved.display()
            ),
        );
        let _ = fs::remove_file(&archive);
        return Ok(json!({
            "installed": false,
            "reason": "checksum_missing",
            "saved_to": saved.display().to_string(),
            "version": resolved.manifest.version,
        }));
    };
    if !checksum_matches(&archive, checksum)? {
        let _ = fs::remove_file(&archive);
        return Err(TfError::staged(
            "update",
            "checksum_mismatch",
            "更新包校验和不匹配，已丢弃 [checksum mismatch; archive discarded]",
        )
        .with_hint("update.error.checksum_mismatch")
        .with_raw(json!({ "expected": checksum })));
    }

    let (installer, program, args) = installer_for_module()?;
    logging::log(
        Level::Info,
        format!("使用 {installer} 安装更新 [installing with {installer}]"),
    );
    let mut argv = args.clone();
    argv.push(archive.display().to_string());
    let code = process::status(&program, &argv)?;
    if code != 0 {
        return Err(TfError::staged(
            "update",
            "installer_failed",
            format!("{installer} 返回退出码 {code} [installer exited with {code}]"),
        )
        .with_hint("update.error.installer_failed")
        .with_raw(json!({ "installer": installer, "exit_code": code })));
    }
    let _ = fs::remove_file(&archive);
    Ok(json!({
        "installed": true,
        "installer": installer,
        "version": resolved.manifest.version,
        "needs_reboot": true,
    }))
}

fn resolve(config: &Config) -> Result<Resolved> {
    let endpoint = format!(
        "{}/{}",
        config.update_base.trim_end_matches('/'),
        config.update_channel.manifest()
    );
    let body = fetch_text(&endpoint)?;
    let manifest = parse_manifest(&body, &endpoint)?;
    let (current_version, current_code) = current_module();
    Ok(Resolved {
        manifest,
        endpoint,
        current_version,
        current_code,
    })
}

fn parse_manifest(body: &str, endpoint: &str) -> Result<Manifest> {
    let value: Value = serde_json::from_str(body).map_err(|error| {
        TfError::staged(
            "update",
            "manifest_invalid",
            format!("更新清单不是合法 JSON [manifest is not valid JSON]: {error}"),
        )
        .with_raw(json!({ "endpoint": endpoint }))
    })?;
    let version = value
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            TfError::staged(
                "update",
                "manifest_invalid",
                "清单缺少 version [missing version]",
            )
        })?
        .to_owned();
    let version_code = value
        .get("versionCode")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            TfError::staged(
                "update",
                "manifest_invalid",
                "清单缺少 versionCode [missing versionCode]",
            )
        })?;
    let zip_url = value
        .get("zipUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            TfError::staged(
                "update",
                "manifest_invalid",
                "清单缺少 zipUrl [missing zipUrl]",
            )
        })?
        .to_owned();
    if !zip_url.starts_with("https://") {
        return Err(TfError::staged(
            "update",
            "manifest_invalid",
            "zipUrl 必须是 https [zipUrl must be https]",
        )
        .with_raw(json!({ "zip_url": zip_url })));
    }
    Ok(Manifest {
        version,
        version_code,
        zip_url,
        changelog_url: value
            .get("changelog")
            .and_then(Value::as_str)
            .map(str::to_owned),
        sha256: value
            .get("sha256")
            .and_then(Value::as_str)
            .map(|text| text.trim().to_ascii_lowercase())
            .filter(|text| text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit())),
        size: value.get("size").and_then(Value::as_u64),
    })
}

/// 当次运行的模块版本；以 module.prop 为准，读不到时退回编译期版本。
fn current_module() -> (String, u64) {
    let prop = fs::read_to_string(MODULE_PROP).unwrap_or_default();
    let version = prop
        .lines()
        .find_map(|line| line.strip_prefix("version="))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| crate::VERSION.to_owned());
    let code = prop
        .lines()
        .find_map(|line| line.strip_prefix("versionCode="))
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);
    (version, code)
}

fn fetch_text(url: &str) -> Result<String> {
    let mut errors = Vec::new();
    let attempts: [(&str, Vec<&str>); 2] = [
        ("curl", vec!["-fsSL", "--max-time", "20", "--url", url]),
        ("wget", vec!["-q", "-T", "20", "-O", "-", url]),
    ];
    for (program, args) in attempts {
        match process::output_bounded(
            program,
            args,
            HTTP_TIMEOUT + Duration::from_secs(5),
            1024 * 1024,
            "获取更新清单 [fetch update manifest]",
        ) {
            Ok(output) if output.status.success() => {
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }
            Ok(output) => errors.push(format!(
                "{program}: 退出码 {} [exit {}]",
                output.status.code().unwrap_or(128),
                output.status.code().unwrap_or(128)
            )),
            Err(error) => errors.push(format!("{program}: {error}")),
        }
    }
    Err(TfError::staged(
        "update",
        "manifest_unreachable",
        format!(
            "无法获取更新清单 [cannot fetch update manifest]: {}",
            errors.join("; ")
        ),
    )
    .with_hint("update.error.manifest_unreachable")
    .with_raw(json!({ "url": url })))
}

fn download(url: &str, destination: &Path) -> Result<()> {
    let destination = destination.display().to_string();
    let attempts: [(&str, Vec<&str>); 2] = [
        (
            "curl",
            vec!["-fL", "--max-time", "180", "-o", &destination, "--url", url],
        ),
        ("wget", vec!["-q", "-T", "180", "-O", &destination, url]),
    ];
    let mut errors = Vec::new();
    for (program, args) in attempts {
        match process::status(program, &args) {
            Ok(0) if Path::new(&destination).is_file() => return Ok(()),
            Ok(code) => errors.push(format!("{program}: 退出码 [exit] {code}")),
            Err(error) => errors.push(format!("{program}: {error}")),
        }
        let _ = fs::remove_file(&destination);
    }
    Err(TfError::staged(
        "update",
        "download_failed",
        format!("下载更新包失败 [download failed]: {}", errors.join("; ")),
    )
    .with_raw(json!({ "url": url })))
}

fn verify_archive(path: &Path, manifest: &Manifest) -> Result<()> {
    let metadata =
        fs::metadata(path).map_err(|error| TfError::from(error).context(path.display()))?;
    if metadata.len() == 0 || metadata.len() > MAX_ARCHIVE_BYTES {
        let _ = fs::remove_file(path);
        return Err(TfError::staged(
            "update",
            "archive_size",
            format!(
                "更新包大小异常 [unexpected archive size]: {} 字节 [bytes]",
                metadata.len()
            ),
        ));
    }
    if let Some(expected) = manifest.size
        && expected != metadata.len()
    {
        let _ = fs::remove_file(path);
        return Err(TfError::staged(
            "update",
            "archive_size",
            format!(
                "更新包大小与清单不一致 [size mismatch]: {} != {expected}",
                metadata.len()
            ),
        )
        .with_raw(json!({ "actual": metadata.len(), "expected": expected })));
    }
    let mut head = [0u8; 4];
    {
        use std::io::Read;
        let mut file = fs::File::open(path)?;
        file.read_exact(&mut head)?;
    }
    if head != *b"PK\x03\x04" {
        let _ = fs::remove_file(path);
        return Err(TfError::staged(
            "update",
            "archive_not_zip",
            "更新包不是 ZIP [archive is not a ZIP]",
        ));
    }
    Ok(())
}

fn checksum_matches(path: &Path, expected: &str) -> Result<bool> {
    let bytes = fs::read(path).map_err(|error| TfError::from(error).context(path.display()))?;
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(digest == expected)
}

fn save_for_manual_install(archive: &Path, version: &str) -> Result<PathBuf> {
    let directory = Path::new(DOWNLOAD_DIR);
    fs::create_dir_all(directory).map_err(|error| TfError::from(error).context(DOWNLOAD_DIR))?;
    let safe = version.replace(['/', '\\', ' '], "_");
    let target = directory.join(format!("teeforge_cd-{safe}.zip"));
    fs::copy(archive, &target).map_err(|error| TfError::from(error).context(target.display()))?;
    Ok(target)
}

/// 按当前 root 管理器选择安装入口，与上游 Duck-ToolBox 的做法一致。
fn installer_for_module() -> Result<(&'static str, String, Vec<String>)> {
    let root = crate::rootdetect::detect();
    match root.method.as_str() {
        "KernelSU" => {
            let program = existing_or_default("/data/adb/ksud", "ksud");
            Ok(("ksud", program, vec!["module".into(), "install".into()]))
        }
        "APatch" => {
            let program = existing_or_default("/data/adb/apd", "apd");
            Ok(("apd", program, vec!["module".into(), "install".into()]))
        }
        "Magisk" => Ok((
            "magisk",
            "magisk".to_owned(),
            vec!["--install-module".to_owned()],
        )),
        other => Err(TfError::staged(
            "update",
            "installer_unknown",
            format!("无法识别的 root 管理器 [unknown root manager]: {other}"),
        )
        .with_hint("update.error.installer_unknown")),
    }
}

fn existing_or_default(absolute: &str, fallback: &str) -> String {
    if Path::new(absolute).is_file() {
        absolute.to_owned()
    } else {
        fallback.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(body: &str) -> Result<Manifest> {
        parse_manifest(body, "https://example.test/release.json")
    }

    #[test]
    fn parses_kernelsu_manifest_with_checksum() {
        let parsed = manifest(
            r#"{"version":"v0.7.0","versionCode":129,"zipUrl":"https://example.test/a.zip",
                "changelog":"https://example.test/CHANGELOG.md",
                "sha256":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","size":123}"#,
        )
        .expect("valid manifest");
        assert_eq!(parsed.version_code, 129);
        assert_eq!(parsed.size, Some(123));
        assert!(parsed.sha256.is_some(), "digest is normalised to lowercase");
    }

    #[test]
    fn checksum_is_optional_and_malformed_values_are_dropped() {
        let parsed = manifest(
            r#"{"version":"v0.7.0","versionCode":129,"zipUrl":"https://example.test/a.zip",
                "sha256":"not-a-digest"}"#,
        )
        .expect("valid manifest");
        assert!(parsed.sha256.is_none());
    }

    #[test]
    fn rejects_missing_fields_and_insecure_url() {
        assert!(manifest(r#"{"version":"v1","zipUrl":"https://a/b.zip"}"#).is_err());
        assert!(manifest(r#"{"version":"v1","versionCode":1,"zipUrl":"http://a/b.zip"}"#).is_err());
    }
}
