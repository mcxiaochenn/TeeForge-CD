//! 设备端 RKP 签发 [Device-backed RKP issuance]
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
//!
//! 职责边界 [Boundary]
//! 本模块只负责"产出产物"（写入 `RKP/`），不负责安装到 Tricky Store 等后端；
//! 安装由 `keybox` 模块按 `--source rkp` 完成。两者解耦，互不回退。
//! This module only produces artifacts; installation lives in `keybox`.

use crate::config::Config;
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;
use p256::SecretKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

mod elf;
mod explain;
mod keybox;
mod protocol;
mod server;
mod soc;
mod ta;
mod worker;

const VENDOR_LIBRARY: &str = "/vendor/lib64/libkeymasterdeviceutils.so";
const SMCINVOKE: &str = "/dev/smcinvoke";
/// 服务端证书链的拼接上限 [Upper bound for the concatenated server chain].
pub(crate) const MAX_CHAIN_BYTES: usize = 0x4000;

/// 一次签发的结果，足以落盘与展示。
pub(crate) struct Issued {
    pub(crate) keybox_xml: String,
    pub(crate) chain_pem: String,
    pub(crate) device_id: String,
    pub(crate) certificates: usize,
    pub(crate) fingerprint: String,
    pub(crate) duration_ms: u128,
    /// 命中的响应帧布局，写入 `meta.json` 便于回溯固件差异。
    pub(crate) frame_layout: &'static str,
}

/// 落盘后的产物路径。
pub(crate) struct SavedArtifacts {
    pub(crate) keybox: PathBuf,
    pub(crate) chain: PathBuf,
    pub(crate) meta: PathBuf,
}

pub(crate) struct Diagnosis {
    pub(crate) arm64_android: bool,
    pub(crate) library_present: bool,
    pub(crate) device_present: bool,
    pub(crate) fingerprint: String,
    pub(crate) manufacturer: String,
    pub(crate) soc_model: String,
    pub(crate) detection: soc::Detection,
    pub(crate) blockers: Vec<(&'static str, String)>,
}

/// 执行一次完整签发：前置检查 → 生成密钥 → 取 challenge → 调 TA → 取证书 → 生成 XML。
pub(crate) fn issue(config: &Config) -> Result<Issued> {
    let started = Instant::now();
    let diagnosis = diagnose();
    if let Some((id, detail)) = diagnosis.blockers.first() {
        return Err(blocker_error(id, detail));
    }
    logging::log(
        Level::Info,
        format!(
            "设备端 RKP 签发 [Device RKP issuance]: {}",
            if diagnosis.manufacturer.is_empty() {
                "Qualcomm"
            } else {
                &diagnosis.manufacturer
            }
        ),
    );

    let key = generate_key()?;
    let tmp = work_dir(config);
    let challenge = server::fetch_challenge(&config.rkp_server, &diagnosis.fingerprint, &tmp)?;
    logging::log(
        Level::Debug,
        format!("RKP challenge: {} 字节 [bytes]", challenge.len()),
    );

    let cose_key = protocol::cose_key(&key.public_key())?;
    let request = protocol::build_request(&challenge, &cose_key)?;
    let frame = worker::sign(&tmp, &request)?;
    // 帧只在这里解一次。worker 返回原始缓冲区，布局由业务结构判定。
    // The frame is decoded exactly once here; the worker returns raw bytes.
    let (layout, csr) = protocol::decode_frame(&frame.bytes, protocol::unwrap_csr)?;
    logging::log(
        Level::Debug,
        format!("TA 帧布局 [frame layout]: {}", layout.label()),
    );
    let info = protocol::validate_csr(&csr, &challenge, &cose_key)?;
    logging::log(
        Level::Debug,
        format!(
            "CSR 校验通过 [CSR verified]: DICE={} version={}",
            info.dice_entries, info.version
        ),
    );

    let chain = server::sign_certificates(&config.rkp_server, &csr, &challenge, &tmp)?;
    let built = keybox::build_full(&key, &chain)?;
    Ok(Issued {
        keybox_xml: built.xml,
        chain_pem: built.chain_pem,
        device_id: built.device_id,
        certificates: built.certificates,
        fingerprint: diagnosis.fingerprint,
        duration_ms: started.elapsed().as_millis(),
        frame_layout: layout.label(),
    })
}

/// 多变异探测：定位「空成功」的成因。
///
/// 现场出现过 `send_cmd` 返回 0 但缓冲区零字节的情况：既不是帧格式问题，也不是
/// 长度字段问题。探测把缓冲区初始化、security_level、请求封装三个变量拆开，
/// 每个变体独立子进程执行，用一次装包换回全部对照数据。
/// Probes one variable at a time to locate the "empty success" path.
pub(crate) fn probe(config: &Config) -> Result<Value> {
    let diagnosis = diagnose();
    if let Some((id, detail)) = diagnosis.blockers.first() {
        return Err(blocker_error(id, detail));
    }
    let key = generate_key()?;
    let tmp = work_dir(config);
    let challenge = server::fetch_challenge(&config.rkp_server, &diagnosis.fingerprint, &tmp)?;
    let cose_key = protocol::cose_key(&key.public_key())?;
    let request = protocol::build_request(&challenge, &cose_key)?;
    logging::log(
        Level::Info,
        format!(
            "开始探测 TA 响应，共 {} 个变体 [probing TA response with {} variants]",
            ta::PROBES.len(),
            ta::PROBES.len()
        ),
    );
    let variants = worker::probe_all(&tmp, &request)?;
    // 符号表能说明固件是否提供初始化入口或其它重载；读不到不影响探测结果。
    let mut library_symbols = elf::function_symbols(VENDOR_LIBRARY, "KeymasterUtils")
        .unwrap_or_else(|error| {
            logging::log(
                Level::Warn,
                format!("读取库符号失败 [symbols unavailable]: {error}"),
            );
            Vec::new()
        });
    if let Ok(more) = elf::function_symbols(VENDOR_LIBRARY, "send_cmd") {
        library_symbols.extend(more);
    }
    library_symbols.sort();
    library_symbols.dedup();
    Ok(json!({
        "soc": diagnosis.soc_model,
        "fingerprint": diagnosis.fingerprint,
        "challenge_bytes": challenge.len(),
        "request_bytes": request.len(),
        "library_symbols": library_symbols,
        "variants": variants,
    }))
}

/// 签发并写入 `RKP/`：keybox.xml、cert-chain.pem、meta.json。
pub(crate) fn issue_and_save(config: &Config) -> Result<SavedArtifacts> {
    let issued = issue(config)?;
    save(config, &issued)
}

fn save(config: &Config, issued: &Issued) -> Result<SavedArtifacts> {
    create_private_dir(&config.rkp_dir)?;
    let keybox = config.rkp_dir.join("keybox.xml");
    let chain = config.rkp_dir.join("cert-chain.pem");
    let meta = config.rkp_dir.join("meta.json");

    let meta_value = json!({
        "version": 1,
        "created_at": now(),
        "fingerprint": issued.fingerprint,
        "device_id": issued.device_id,
        "certificates": issued.certificates,
        "protocol": "csr_v2",
        "frame_layout": issued.frame_layout,
        "duration_ms": issued.duration_ms,
        "ta_status": 0,
        "tool_version": crate::VERSION,
        "sha256": hex_sha256(issued.keybox_xml.as_bytes()),
    });
    let meta_text = format!(
        "{}\n",
        serde_json::to_string_pretty(&meta_value).unwrap_or_default()
    );

    // 三个文件都是私钥材料，统一 0600 + 原子替换。
    // All three files contain private material: 0600 and atomic replace.
    crate::atomic_file::write_private(&keybox, issued.keybox_xml.as_bytes())?;
    crate::atomic_file::write_private(&chain, issued.chain_pem.as_bytes())?;
    crate::atomic_file::write_private(&meta, meta_text.as_bytes())?;
    logging::log(
        Level::Info,
        format!(
            "RKP 产物已写入 [RKP artifacts written]: {}（{} 张证书）",
            keybox.display(),
            issued.certificates
        ),
    );
    Ok(SavedArtifacts {
        keybox,
        chain,
        meta,
    })
}

/// 只读探测：不联网、不调用 TA。
pub(crate) fn diagnose() -> Diagnosis {
    let arm64_android = device_path_supported();
    let library_present = Path::new(VENDOR_LIBRARY).is_file();
    let device_present = Path::new(SMCINVOKE).exists();
    let fingerprint = getprop("ro.build.fingerprint");
    let detection = soc::detect();
    let mut blockers = Vec::new();
    if !arm64_android {
        blockers.push((
            "arch",
            "设备端 RKP 仅支持 Android arm64 [requires Android arm64]".to_owned(),
        ));
    }
    if !library_present {
        blockers.push((
            "library",
            format!("缺少 {VENDOR_LIBRARY} [vendor library missing]"),
        ));
    }
    if !device_present {
        blockers.push((
            "smcinvoke",
            "缺少 /dev/smcinvoke [smcinvoke missing]".to_owned(),
        ));
    }
    if detection.state != soc::State::Qualcomm {
        blockers.push((
            "soc",
            format!(
                "需要高通 SoC [Qualcomm SoC required]: {}",
                detection.summary()
            ),
        ));
    }
    if fingerprint.is_empty() {
        blockers.push((
            "fingerprint",
            "ro.build.fingerprint 为空 [fingerprint is empty]".to_owned(),
        ));
    }
    Diagnosis {
        arm64_android,
        library_present,
        device_present,
        manufacturer: getprop("ro.soc.manufacturer"),
        soc_model: getprop("ro.soc.model"),
        fingerprint,
        detection,
        blockers,
    }
}

fn blocker_error(id: &'static str, detail: &str) -> TfError {
    TfError::staged(
        "prerequisite",
        id,
        format!("前置条件不满足 [prerequisite]: {detail}"),
    )
    .with_hint(format!("rkp.blocker.{id}"))
    .with_raw(json!({ "blocker": id }))
}

/// `--rkp-status` 的结构化输出。
pub(crate) fn status_value(config: &Config) -> Value {
    let diagnosis = diagnose();
    let blockers = diagnosis
        .blockers
        .iter()
        .map(|(id, detail)| json!({ "id": id, "detail": detail }))
        .collect::<Vec<_>>();
    let artifact_meta = read_meta(config);
    json!({
        "supported": blockers.is_empty(),
        "platform": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "arm64_android": diagnosis.arm64_android,
        },
        "root": {
            "method": config.root_method,
            "version": config.root_version,
        },
        "soc": {
            "state": diagnosis.detection.state.label(),
            "manufacturer": diagnosis.manufacturer,
            "model": diagnosis.soc_model,
            "evidence": diagnosis.detection.evidence,
        },
        "library": { "path": VENDOR_LIBRARY, "present": diagnosis.library_present },
        "device": { "path": SMCINVOKE, "present": diagnosis.device_present },
        "fingerprint": diagnosis.fingerprint,
        "server": config.rkp_server,
        "blockers": blockers,
        "artifacts": {
            "exists": config.rkp_dir.join("keybox.xml").is_file(),
            "dir": config.rkp_dir.display().to_string(),
            "meta": artifact_meta,
        },
    })
}

/// 产物列表：目录不存在时返回 `exists=false`，不报错。
pub(crate) fn artifacts_value(config: &Config) -> Value {
    let entry = |name: &str| {
        let path = config.rkp_dir.join(name);
        match fs::metadata(&path) {
            Ok(metadata) => json!({
                "path": path.display().to_string(),
                "size": metadata.len(),
                "mtime": metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|value| value.as_secs()),
            }),
            Err(_) => Value::Null,
        }
    };
    json!({
        "exists": config.rkp_dir.is_dir(),
        "dir": config.rkp_dir.display().to_string(),
        "keybox": entry("keybox.xml"),
        "chain": entry("cert-chain.pem"),
        "meta_file": entry("meta.json"),
        "meta": read_meta(config),
    })
}

/// 删除整个 `RKP/` 目录。目录原本不存在按成功处理。
pub(crate) fn delete_artifacts(config: &Config) -> Result<Value> {
    let freed = directory_size(&config.rkp_dir);
    match fs::remove_dir_all(&config.rkp_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(TfError::staged(
                "artifacts",
                "delete_failed",
                format!("无法删除 RKP 目录 [cannot delete RKP dir]: {error}"),
            )
            .with_raw(json!({ "dir": config.rkp_dir.display().to_string() })));
        }
    }
    logging::log(
        Level::Info,
        format!("RKP 产物已删除 [RKP artifacts removed]: {freed} 字节 [bytes]"),
    );
    Ok(json!({
        "deleted": true,
        "dir": config.rkp_dir.display().to_string(),
        "freed_bytes": freed,
    }))
}

/// `--rkp-status` 的文本输出（非 JSON 模式）。
pub(crate) fn status_lines(config: &Config) -> Vec<String> {
    let diagnosis = diagnose();
    let mut lines = Vec::new();
    lines.push(format!("服务 Server: {}", config.rkp_server));
    lines.push(format!(
        "平台 Platform: {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    lines.push(format!(
        "SoC: {} {}",
        diagnosis.detection.state.label(),
        if diagnosis.detection.evidence.is_empty() {
            "(无证据 no evidence)".to_owned()
        } else {
            format!("[{}]", diagnosis.detection.evidence.join(", "))
        }
    ));
    lines.push(format!(
        "厂商库 Vendor library: {} ({})",
        VENDOR_LIBRARY,
        if diagnosis.library_present {
            "存在 present"
        } else {
            "缺失 missing"
        }
    ));
    lines.push(format!(
        "SMCInvoke: {} ({})",
        SMCINVOKE,
        if diagnosis.device_present {
            "存在 present"
        } else {
            "缺失 missing"
        }
    ));
    lines.push(format!(
        "Fingerprint: {}",
        if diagnosis.fingerprint.is_empty() {
            "(缺失 missing)"
        } else {
            &diagnosis.fingerprint
        }
    ));
    match diagnosis.blockers.first() {
        Some((_, detail)) => lines.push(format!("结论 Result: 不可用 [unavailable]: {detail}")),
        None => lines.push("结论 Result: 可用 [available]".to_owned()),
    }
    let artifacts = config.rkp_dir.join("keybox.xml");
    lines.push(format!(
        "产物 Artifacts: {} ({})",
        config.rkp_dir.display(),
        if artifacts.is_file() {
            "已生成 present"
        } else {
            "未生成 absent"
        }
    ));
    lines
}

pub(crate) fn available() -> bool {
    diagnose().blockers.is_empty()
}

/// 厂商调用路径只在 arm64 Android 编译；其余 ABI 无法进行设备端签发。
/// The vendor path only exists on arm64 Android.
pub(crate) fn device_path_supported() -> bool {
    cfg!(all(target_os = "android", target_arch = "aarch64"))
}

pub(crate) fn execute_worker(directory: &Path) -> Result<()> {
    worker::execute(directory)
}

/// 探测子进程入口，供 CLI 的隐藏参数转发。
pub(crate) fn execute_probe(directory: &Path, variant: usize) -> Result<()> {
    worker::execute_probe(directory, variant)
}

fn read_meta(config: &Config) -> Value {
    fs::read_to_string(config.rkp_dir.join("meta.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or(Value::Null)
}

fn directory_size(directory: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
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
    fn artifacts_are_written_owner_only_and_removed() {
        let root = std::env::temp_dir().join(format!("teeforge-rkp-store-{}", std::process::id()));
        let config = Config {
            rkp_dir: root.clone(),
            ..Config::default()
        };
        let issued = Issued {
            keybox_xml: "<AndroidAttestation/>".into(),
            chain_pem: "-----BEGIN CERTIFICATE-----\n".into(),
            device_id: "1A2B".into(),
            certificates: 2,
            fingerprint: "brand/device".into(),
            duration_ms: 12,
            frame_layout: "status+len",
        };
        let saved = save(&config, &issued).unwrap();
        assert!(saved.keybox.is_file() && saved.chain.is_file() && saved.meta.is_file());

        let value = artifacts_value(&config);
        assert_eq!(value["exists"], json!(true));
        assert_eq!(value["meta"]["certificates"], json!(2));
        assert_eq!(value["meta"]["device_id"], json!("1A2B"));

        let removed = delete_artifacts(&config).unwrap();
        assert_eq!(removed["deleted"], json!(true));
        assert!(!root.exists());
        // 目录不存在时删除仍然成功（幂等）。
        assert!(delete_artifacts(&config).is_ok());
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
