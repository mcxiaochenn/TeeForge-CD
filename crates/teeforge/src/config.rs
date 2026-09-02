use crate::atomic_file;
use crate::error::{Result, TfError};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const SYS_CONFIG: &str = "/data/adb/teeforge/sys.conf";
pub(crate) const USER_CONFIG: &str = "/data/adb/teeforge/config.conf";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) packages_xml: PathBuf,
    pub(crate) target_txt: PathBuf,
    pub(crate) keybox_dir: PathBuf,
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
    pub(crate) blhide_compact: bool,
    pub(crate) prop_tool: PropTool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PropTool {
    Standard,
    Rs,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            packages_xml: "/data/system/packages.xml".into(),
            target_txt: "/data/adb/tricky_store/target.txt".into(),
            keybox_dir: "/data/adb/teeforge/keybox".into(),
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
            blhide_compact: true,
            prop_tool: PropTool::Standard,
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
            "keybox_dir" => self.keybox_dir = value.into(),
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
            "blhide_compact" => self.blhide_compact = parse_bool(value),
            "prop_tool" => {
                self.prop_tool = if value == "rs" {
                    PropTool::Rs
                } else {
                    PropTool::Standard
                }
            }
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
        assert_eq!(config.prop_tool, PropTool::Rs);
        assert_eq!(config.target_txt, PathBuf::from("/tmp/target"));
    }

    #[test]
    fn invalid_boolean_is_off() {
        let mut config = Config::default();
        config.apply("blhide", "invalid");
        assert!(!config.blhide);
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
        assert_eq!(config.prop_tool, PropTool::Rs);
        assert!(config.debug);
        assert!(!config.blhide);
    }
}
