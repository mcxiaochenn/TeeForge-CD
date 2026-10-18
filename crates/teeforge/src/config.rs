use crate::atomic_file;
use crate::error::{Result, TfError};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const SYS_CONFIG: &str = "/data/adb/teeforge/sys.conf";
pub(crate) const USER_CONFIG: &str = "/data/adb/teeforge/config.conf";
pub(crate) const DEFAULT_RKP_SERVER: &str = "https://remoteprovisioning.googleapis.com/v1";
pub(crate) const DEFAULT_UPDATE_BASE: &str = "https://teeforge.mcxiaochen.top/files/update";

/// Keybox 来源 [Where the installed keybox comes from].
///
/// 由用户显式选择，不再隐式回退；`Remote` 是升级后的默认值，保持旧行为。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyboxSource {
    /// 远程公共密钥池 [Shared keybox pool from the CDN]
    Remote,
    /// 本机 RKP 产物 [Artifacts produced by this device]
    Rkp,
}

impl KeyboxSource {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "remote" => Some(Self::Remote),
            "rkp" | "device" | "local" => Some(Self::Rkp),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Remote => "remote",
            Self::Rkp => "rkp",
        }
    }
}

/// 目标作用域模式 [How the target list is produced].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TargetMode {
    /// 每次扫描全部已安装用户应用 [Every installed user app]
    Auto,
    /// 只使用 `target-selection.json` 中的集合 [Only the saved selection]
    Manual,
}

impl TargetMode {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

/// 更新通道 [Update channel].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UpdateChannel {
    Stable,
    Dev,
}

impl UpdateChannel {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "stable" | "release" => Some(Self::Stable),
            "dev" | "canary" => Some(Self::Dev),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Dev => "dev",
        }
    }

    /// 通道对应的清单文件名 [Manifest file name for this channel].
    pub(crate) fn manifest(self) -> &'static str {
        match self {
            Self::Stable => "release.json",
            Self::Dev => "dev.json",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) packages_xml: PathBuf,
    pub(crate) target_txt: PathBuf,
    pub(crate) teesim_config: PathBuf,
    pub(crate) omk_enabled: bool,
    pub(crate) omk_injector_config: PathBuf,
    pub(crate) keybox_dir: PathBuf,
    pub(crate) rkp_dir: PathBuf,
    pub(crate) keybox_source: KeyboxSource,
    pub(crate) keybox_remote_slot: u8,
    pub(crate) target_mode: TargetMode,
    pub(crate) target_selection: PathBuf,
    pub(crate) rkp_server: String,
    pub(crate) update_base: String,
    pub(crate) update_dir: PathBuf,
    pub(crate) update_channel: UpdateChannel,
    pub(crate) sources_conf: PathBuf,
    pub(crate) log_dir: PathBuf,
    pub(crate) root_method: String,
    pub(crate) root_version: String,
    pub(crate) debug: bool,
    pub(crate) blhide: bool,
    pub(crate) blhide_boot: bool,
    pub(crate) blhide_security: bool,
    pub(crate) blhide_vendor: bool,
    pub(crate) blhide_oem: bool,
    pub(crate) blhide_secureboot: bool,
    pub(crate) blhide_realme: bool,
    pub(crate) blhide_recovery: bool,
    pub(crate) blhide_developer: bool,
    pub(crate) blhide_selinux: bool,
    pub(crate) blhide_virtual: bool,
    pub(crate) blhide_delete: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            packages_xml: "/data/system/packages.xml".into(),
            target_txt: "/data/adb/tricky_store/target.txt".into(),
            teesim_config: "/data/adb/teesim/config.json".into(),
            omk_enabled: false,
            omk_injector_config: "/data/misc/keystore/omk/injector.toml".into(),
            keybox_dir: "/data/adb/teeforge/keybox".into(),
            rkp_dir: "/data/adb/teeforge/RKP".into(),
            keybox_source: KeyboxSource::Remote,
            keybox_remote_slot: 1,
            target_mode: TargetMode::Auto,
            target_selection: "/data/adb/teeforge/target-selection.json".into(),
            rkp_server: DEFAULT_RKP_SERVER.into(),
            update_base: DEFAULT_UPDATE_BASE.into(),
            update_dir: "/data/adb/teeforge/update".into(),
            update_channel: UpdateChannel::Stable,
            sources_conf: "/data/adb/teeforge/sources.conf".into(),
            log_dir: "/data/adb/teeforge/logs".into(),
            root_method: "Unknown".into(),
            root_version: "unknown".into(),
            debug: false,
            blhide: true,
            blhide_boot: true,
            blhide_security: true,
            blhide_vendor: true,
            blhide_oem: true,
            blhide_secureboot: true,
            blhide_realme: true,
            blhide_recovery: true,
            blhide_developer: true,
            blhide_selinux: true,
            blhide_virtual: true,
            blhide_delete: true,
        }
    }
}

fn parse_bool(value: &str) -> bool {
    value.trim().parse::<i32>().unwrap_or(0) != 0
}

impl Config {
    pub(crate) fn load(user_path: &Path) -> Self {
        let mut config = Self::default();
        let sys_path = existing_or_legacy(Path::new(SYS_CONFIG), Path::new("./sys.conf"));
        if let Some(path) = sys_path {
            config.apply_file(&path);
        }
        let fallback = (user_path == Path::new(USER_CONFIG)).then_some(Path::new("./config.conf"));
        if let Some(path) = existing_or_legacy(user_path, fallback.unwrap_or(user_path)) {
            config.apply_file(&path);
        }
        config
    }

    fn apply_file(&mut self, path: &Path) {
        let Ok(data) = fs::read_to_string(path) else {
            return;
        };
        for raw in data.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            self.apply(key.trim(), value.trim());
        }
    }

    fn apply(&mut self, key: &str, value: &str) {
        match key {
            "packages_xml" => self.packages_xml = value.into(),
            "target_txt" => self.target_txt = value.into(),
            "teesim_config" => self.teesim_config = value.into(),
            "omk_enabled" => self.omk_enabled = parse_bool(value),
            "omk_injector_config" => self.omk_injector_config = value.into(),
            "keybox_dir" => self.keybox_dir = value.into(),
            "rkp_dir" => self.rkp_dir = value.into(),
            "keybox_source" => {
                if let Some(source) = KeyboxSource::parse(value) {
                    self.keybox_source = source;
                }
            }
            "keybox_remote_slot" => {
                if let Ok(slot @ 1..=3) = value.parse::<u8>() {
                    self.keybox_remote_slot = slot;
                }
            }
            // 旧键：1.0 之前用自动优先设备端；升级后统一按 keybox_source 处理，
            // 这里仅解析不生效，避免用户配置报错。
            "rkp_mode" => {}
            "target_mode" => {
                if let Some(mode) = TargetMode::parse(value) {
                    self.target_mode = mode;
                }
            }
            "target_selection" => self.target_selection = value.into(),
            "rkp_server" => {
                let server = value.trim();
                if !server.is_empty() {
                    self.rkp_server = server.into();
                }
            }
            "update_base" => {
                let base = value.trim().trim_end_matches('/');
                if !base.is_empty() {
                    self.update_base = base.into();
                }
            }
            "update_dir" => self.update_dir = value.into(),
            "update_channel" => {
                if let Some(channel) = UpdateChannel::parse(value) {
                    self.update_channel = channel;
                }
            }
            "sources_conf" => self.sources_conf = value.into(),
            "log_dir" => self.log_dir = value.into(),
            "root_method" => self.root_method = value.into(),
            "root_version" => self.root_version = value.into(),
            "debug" => self.debug = parse_bool(value),
            "blhide" => self.blhide = parse_bool(value),
            "blhide_boot" => self.blhide_boot = parse_bool(value),
            "blhide_security" => self.blhide_security = parse_bool(value),
            "blhide_vendor" => self.blhide_vendor = parse_bool(value),
            "blhide_oem" => self.blhide_oem = parse_bool(value),
            "blhide_secureboot" => self.blhide_secureboot = parse_bool(value),
            "blhide_realme" => self.blhide_realme = parse_bool(value),
            "blhide_recovery" => self.blhide_recovery = parse_bool(value),
            "blhide_developer" => self.blhide_developer = parse_bool(value),
            "blhide_selinux" => self.blhide_selinux = parse_bool(value),
            "blhide_virtual" => self.blhide_virtual = parse_bool(value),
            "blhide_delete" => self.blhide_delete = parse_bool(value),
            _ => {}
        }
    }
}

fn existing_or_legacy(primary: &Path, legacy: &Path) -> Option<PathBuf> {
    if primary.is_file() {
        Some(primary.to_path_buf())
    } else if legacy.is_file() {
        Some(legacy.to_path_buf())
    } else {
        None
    }
}

pub(crate) fn update_key(path: &Path, key: &str, value: &str) -> Result<()> {
    if key.contains(['=', '\n', '\r']) || value.contains(['\n', '\r']) {
        return Err(TfError::new(
            "配置键值包含非法字符 [Configuration key/value contains invalid characters]",
        ));
    }
    let current = fs::read_to_string(path).unwrap_or_default();
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in current.lines() {
        if line.split_once('=').is_some_and(|(found, _)| found == key) {
            lines.push(format!("{key}={value}"));
            replaced = true;
        } else {
            lines.push(line.to_owned());
        }
    }
    if !replaced {
        lines.push(format!("{key}={value}"));
    }
    atomic_file::write(path, format!("{}\n", lines.join("\n")).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_values_override_defaults() {
        let mut config = Config::default();
        config.apply("debug", "1");
        config.apply("prop_tool", "rs");
        config.apply("target_txt", "/tmp/target");
        assert!(config.debug);
        assert_eq!(config.target_txt, PathBuf::from("/tmp/target"));
        assert_eq!(
            config.teesim_config,
            PathBuf::from("/data/adb/teesim/config.json")
        );
    }

    #[test]
    fn invalid_boolean_is_off() {
        let mut config = Config::default();
        config.apply("blhide", "invalid");
        assert!(!config.blhide);
    }

    #[test]
    fn omk_defaults_and_user_overrides_ignore_legacy_rs_keys() {
        let mut config = Config::default();
        assert!(!config.omk_enabled);
        let original = config.clone();
        config.apply("prop_tool", "rs");
        config.apply("blhide_compact", "1");
        assert_eq!(config, original);
        config.apply("omk_enabled", "1");
        config.apply("omk_injector_config", "/tmp/omk/injector.toml");
        assert!(config.omk_enabled);
        assert_eq!(
            config.omk_injector_config,
            PathBuf::from("/tmp/omk/injector.toml")
        );
        config.apply("omk_enabled", "0");
        assert!(!config.omk_enabled);
    }

    #[test]
    fn keybox_source_defaults_to_remote_and_accepts_overrides() {
        let mut config = Config::default();
        assert_eq!(config.keybox_source, KeyboxSource::Remote);
        assert_eq!(config.keybox_remote_slot, 1);
        config.apply("keybox_remote_slot", "3");
        assert_eq!(config.keybox_remote_slot, 3);
        for invalid in ["0", "4", "-1", "bad"] {
            config.apply("keybox_remote_slot", invalid);
            assert_eq!(config.keybox_remote_slot, 3);
        }

        config.apply("keybox_source", "rkp");
        assert_eq!(config.keybox_source, KeyboxSource::Rkp);
        assert_eq!(config.keybox_remote_slot, 3);
        config.apply("keybox_source", "remote");
        assert_eq!(config.keybox_source, KeyboxSource::Remote);
        config.apply("keybox_source", "nonsense");
        assert_eq!(
            config.keybox_source,
            KeyboxSource::Remote,
            "unknown values stay safe"
        );
    }

    #[test]
    fn legacy_rkp_mode_is_ignored_without_error() {
        let mut config = Config::default();
        config.apply("rkp_mode", "device");
        assert_eq!(config.keybox_source, KeyboxSource::Remote);
    }

    #[test]
    fn target_mode_and_update_settings_are_validated() {
        let mut config = Config::default();
        assert_eq!(config.target_mode, TargetMode::Auto);
        assert_eq!(config.update_channel, UpdateChannel::Stable);
        assert_eq!(config.update_base, DEFAULT_UPDATE_BASE);

        config.apply("target_mode", "manual");
        assert_eq!(config.target_mode, TargetMode::Manual);
        config.apply("target_mode", "bogus");
        assert_eq!(config.target_mode, TargetMode::Manual);

        config.apply("update_channel", "dev");
        assert_eq!(config.update_channel, UpdateChannel::Dev);
        config.apply("update_base", "https://example.test/update/");
        assert_eq!(config.update_base, "https://example.test/update");
        config.apply("update_base", "  ");
        assert_eq!(config.update_base, "https://example.test/update");

        config.apply("rkp_server", "  ");
        assert_eq!(config.rkp_server, DEFAULT_RKP_SERVER);
    }

    #[test]
    fn user_fixture_overrides_system_fixture() {
        let mut config = Config::default();
        for (key, value) in include_str!("../../../tests/fixtures/sys.conf")
            .lines()
            .filter_map(|line| line.split_once('='))
        {
            config.apply(key, value);
        }
        for (key, value) in include_str!("../../../tests/fixtures/config.conf")
            .lines()
            .filter_map(|line| line.split_once('='))
        {
            config.apply(key, value);
        }
        assert_eq!(config.root_method, "KernelSU");
        assert!(config.debug);
        assert!(!config.blhide);
    }
}
