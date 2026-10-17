//! 设备端 RKP keybox 签发 [Device-backed RKP keybox issuance]
//!
//! 来源与许可 [Origin and license]
//! 本模块移植并适配自 Duck-ToolBox 的 Device RKP Issuer：
//! Ported and adapted from Duck-ToolBox's Device RKP Issuer:
//!   https://github.com/eltavine/Duck-ToolBox
//!   参考修订 Revision: fb6b1e427f2773ac6139d2eba27068fbdfd089de
//!   Copyright (c) 2026 Eltavine, MIT License（见 module/licenses/duck-toolbox.txt）
//!
//! 相对上游的裁剪 [Deviations from upstream]
//! - 仅保留 V2（`CSR_V2` / 0x3122）设备路径，不包含 V1 AIDL、原生 V1 与离线工作台。
//!   V2 only; the V1 AIDL bridge, native V1 and the offline workbench are not included.
//! - HTTP 改用设备上的 `curl` / `wget`，不引入 reqwest 与异步运行时。
//!   HTTP uses the device's `curl` / `wget`; no reqwest and no async runtime.
//! - 本地只校验 CSR 结构与 challenge/keysToSign 绑定；DICE/COSE 密码学校验由 Google
//!   RKP 服务端完成，非法 CSR 不会签发证书。
//!   Only structure and binding are checked locally; Google performs the
//!   cryptographic validation and never signs an invalid CSR.

use crate::config::Config;
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;
use p256::SecretKey;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod keybox;
mod protocol;
mod server;
mod soc;
mod ta;
mod worker;

const VENDOR_LIBRARY: &str = "/vendor/lib64/libkeymasterdeviceutils.so";
const SMCINVOKE: &str = "/dev/smcinvoke";
const MARKER: &str = ".rkp-issued";
/// 服务端证书链的拼接上限 [Upper bound for the concatenated server chain].
pub(crate) const MAX_CHAIN_BYTES: usize = 0x4000;

struct Prerequisites {
    fingerprint: String,
    manufacturer: String,
}

/// 执行一次完整签发：前置检查 → 生成密钥 → 取 challenge → 调 TA → 取证书 → 生成 XML。
pub(crate) fn issue(config: &Config) -> Result<Vec<u8>> {
    let prerequisites = prerequisites()?;
    logging::log(
        Level::Info,
        format!(
            "设备端 RKP 签发 [Device RKP issuance]: {}",
            if prerequisites.manufacturer.is_empty() {
                "Qualcomm"
            } else {
                &prerequisites.manufacturer
            }
        ),
    );

    let key = generate_key()?;
    let tmp = work_dir(config);
    let challenge = server::fetch_challenge(&config.rkp_server, &prerequisites.fingerprint, &tmp)?;
    logging::log(
        Level::Debug,
        format!("RKP challenge: {} 字节 [bytes]", challenge.len()),
    );

    let cose_key = protocol::cose_key(&key.public_key())?;
    let request = protocol::build_request(&challenge, &cose_key)?;
    let response = worker::sign(&tmp, &request)?;
    let csr = protocol::unwrap_csr(&protocol::response_payload(&response)?)?;
    let info = protocol::validate_csr(&csr, &challenge, &cose_key)?;
    logging::log(
        Level::Debug,
        format!(
            "CSR 校验通过 [CSR verified]: DICE={} version={}",
            info.dice_entries, info.version
        ),
    );

    let chain = server::sign_certificates(&config.rkp_server, &csr, &challenge, &tmp)?;
    keybox::build(&key, &chain).map(String::into_bytes)
}

fn prerequisites() -> Result<Prerequisites> {
    if !device_path_supported() {
        return Err(TfError::new(
            "设备端 RKP 仅支持 Android arm64 [device RKP requires Android arm64]",
        ));
    }
    if !Path::new(VENDOR_LIBRARY).is_file() {
        return Err(TfError::new(format!(
            "缺少 {VENDOR_LIBRARY} [vendor library missing]"
        )));
    }
    if !Path::new(SMCINVOKE).exists() {
        return Err(TfError::new("缺少 /dev/smcinvoke [smcinvoke missing]"));
    }
    let detection = soc::detect();
    if detection.state != soc::State::Qualcomm {
        return Err(TfError::new(format!(
            "需要高通 SoC [Qualcomm SoC required]: {}",
            detection.summary()
        )));
    }
    let fingerprint = getprop("ro.build.fingerprint");
    if fingerprint.is_empty() {
        return Err(TfError::new(
            "ro.build.fingerprint 为空 [fingerprint is empty]",
        ));
    }
    Ok(Prerequisites {
        fingerprint,
        manufacturer: getprop("ro.soc.manufacturer"),
    })
}

/// `--rkp-status` 输出：不联网、不调用 TA。
pub(crate) fn status_lines(config: &Config) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "模式 Mode: {} / 复用窗口 Refresh: {} 天 [days]",
        config.rkp_mode.label(),
        config.rkp_refresh_days
    ));
    lines.push(format!("服务 Server: {}", config.rkp_server));
    lines.push(format!(
        "平台 Platform: {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    let detection = soc::detect();
    lines.push(format!(
        "SoC: {} {}",
        detection.state.label(),
        if detection.evidence.is_empty() {
            "(无证据 no evidence)".to_owned()
        } else {
            format!("[{}]", detection.evidence.join(", "))
        }
    ));
    lines.push(format!(
        "厂商库 Vendor library: {} ({})",
        VENDOR_LIBRARY,
        if Path::new(VENDOR_LIBRARY).is_file() {
            "存在 present"
        } else {
            "缺失 missing"
        }
    ));
    lines.push(format!(
        "SMCInvoke: {} ({})",
        SMCINVOKE,
        if Path::new(SMCINVOKE).exists() {
            "存在 present"
        } else {
            "缺失 missing"
        }
    ));
    let fingerprint = getprop("ro.build.fingerprint");
    lines.push(format!(
        "Fingerprint: {}",
        if fingerprint.is_empty() {
            "(缺失 missing)"
        } else {
            &fingerprint
        }
    ));
    match prerequisites() {
        Ok(_) => lines.push("结论 Result: 可用 [available]".to_owned()),
        Err(error) => lines.push(format!("结论 Result: 不可用 [unavailable]: {error}")),
    }
    lines
}

pub(crate) fn available() -> bool {
    prerequisites().is_ok()
}

/// 厂商调用路径只在 arm64 Android 编译；其余 ABI 直接走远程，不产生无意义告警。
/// The vendor path only exists on arm64 Android; other ABIs go straight to remote.
pub(crate) fn device_path_supported() -> bool {
    cfg!(all(target_os = "android", target_arch = "aarch64"))
}

/// 复用窗口内的本地 RKP keybox；文件被替换后摘要不符则不复用。
pub(crate) fn cached(config: &Config) -> Option<Vec<u8>> {
    if config.rkp_refresh_days == 0 {
        return None;
    }
    let bytes = fs::read(config.keybox_dir.join("keybox.xml")).ok()?;
    let marker = fs::read_to_string(config.keybox_dir.join(MARKER)).ok()?;
    let mut parts = marker.split_whitespace();
    let issued = parts.next()?.parse::<u64>().ok()?;
    let digest = parts.next()?;
    let age = now().checked_sub(issued)?;
    if age > config.rkp_refresh_days.saturating_mul(86_400) {
        return None;
    }
    if hex_sha256(&bytes) != digest {
        return None;
    }
    logging::log(
        Level::Info,
        format!(
            "复用本地 RKP keybox [reusing local RKP keybox]: {} 天前 [days ago]",
            age / 86_400
        ),
    );
    Some(bytes)
}

/// 记录本次设备签发，供复用窗口判定。
pub(crate) fn mark_issued(config: &Config, xml: &[u8]) -> Result<()> {
    create_private_dir(&config.keybox_dir)?;
    let marker = format!("{} {}\n", now(), hex_sha256(xml));
    crate::atomic_file::write(&config.keybox_dir.join(MARKER), marker.as_bytes())
}

/// 远程回退成功后清除标记，避免把池子 keybox 当成设备签发结果复用。
pub(crate) fn clear_marker(config: &Config) {
    match fs::remove_file(config.keybox_dir.join(MARKER)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => logging::log(
            Level::Warn,
            format!("清除 RKP 标记失败 [failed to clear RKP marker]: {error}"),
        ),
    }
}

pub(crate) fn execute_worker(directory: &Path) -> Result<()> {
    worker::execute(directory)
}

fn generate_key() -> Result<SecretKey> {
    let mut file = fs::File::open("/dev/urandom")
        .map_err(|error| TfError::new(format!("无法读取 /dev/urandom [urandom]: {error}")))?;
    for _ in 0..8 {
        let mut bytes = [0u8; 32];
        file.read_exact(&mut bytes)?;
        if let Ok(key) = SecretKey::from_slice(&bytes) {
            return Ok(key);
        }
    }
    Err(TfError::new(
        "无法生成有效 P-256 私钥 [key generation failed]",
    ))
}

fn work_dir(config: &Config) -> PathBuf {
    config.keybox_dir.join("tmp")
}

fn getprop(name: &str) -> String {
    process::output("getprop", [name])
        .ok()
        .and_then(|output| process::stdout_text(output, name).ok())
        .map(|value| value.trim().to_owned())
        .unwrap_or_default()
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

/// 创建 0700 目录并返回唯一的运行子目录。
pub(super) fn unique_dir(root: &Path) -> Result<PathBuf> {
    create_private_dir(root)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    for attempt in 0..16 {
        let candidate = root.join(format!("rkp-{}-{nanos}-{attempt}", std::process::id()));
        match fs::create_dir(&candidate) {
            Ok(()) => {
                restrict_directory(&candidate);
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(TfError::new(format!(
                    "无法创建 RKP 临时目录 [cannot create RKP temp dir]: {error}"
                )));
            }
        }
    }
    Err(TfError::new(
        "无法创建唯一 RKP 临时目录 [cannot allocate a unique RKP temp dir]",
    ))
}

pub(super) fn create_private_dir(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    restrict_directory(path);
    Ok(())
}

fn restrict_directory(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(_path, fs::Permissions::from_mode(0o700));
    }
}

pub(super) fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_directories_do_not_collide() {
        let root = std::env::temp_dir().join(format!("teeforge-rkp-unique-{}", std::process::id()));
        let first = unique_dir(&root).unwrap();
        let second = unique_dir(&root).unwrap();
        assert_ne!(first, second);
        assert!(first.is_dir() && second.is_dir());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn marker_roundtrip_controls_reuse() {
        let root = std::env::temp_dir().join(format!("teeforge-rkp-cache-{}", std::process::id()));
        create_private_dir(&root).unwrap();
        let mut config = Config {
            keybox_dir: root.clone(),
            rkp_refresh_days: 30,
            ..Config::default()
        };

        assert!(cached(&config).is_none());
        fs::write(root.join("keybox.xml"), b"<AndroidAttestation/>").unwrap();
        mark_issued(&config, b"<AndroidAttestation/>").unwrap();
        assert_eq!(cached(&config).unwrap(), b"<AndroidAttestation/>".to_vec());

        fs::write(root.join("keybox.xml"), b"other").unwrap();
        assert!(cached(&config).is_none(), "digest mismatch must not reuse");

        config.rkp_refresh_days = 0;
        assert!(cached(&config).is_none());
        clear_marker(&config);
        assert!(!root.join(MARKER).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sha256_helper_is_lowercase_hex() {
        let digest = hex_sha256(b"abc");
        assert_eq!(digest.len(), 64);
        assert!(
            digest
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );
        assert!(
            digest
                .chars()
                .all(|character| !character.is_ascii_uppercase())
        );
    }
}
