use crate::props::Category;
use std::fs;
use std::path::Path;

/// 只保留弱隐相关的开关，其余配置键由主程序负责。
/// Only the weak-hiding switches live here; every other key belongs to the main binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HideConfig {
    pub enabled: bool,
    pub boot: bool,
    pub security: bool,
    pub vendor: bool,
    pub oem: bool,
    pub secureboot: bool,
    pub realme: bool,
    pub recovery: bool,
    pub developer: bool,
    pub selinux: bool,
    pub virtual_device: bool,
    pub delete: bool,
}

impl Default for HideConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            boot: true,
            security: true,
            vendor: true,
            oem: true,
            secureboot: true,
            realme: true,
            recovery: true,
            developer: true,
            selinux: true,
            virtual_device: true,
            delete: true,
        }
    }
}

impl HideConfig {
    pub fn load(path: &Path) -> Self {
        let mut config = Self::default();
        let Ok(data) = fs::read_to_string(path) else {
            // 配置缺失时按全开执行：模块已装但用户还没生成配置也不该静默不工作。
            // Missing config falls back to everything on; an installed module must still work.
            return config;
        };
        for raw in data.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            config.apply(key.trim(), value.trim());
        }
        config
    }

    fn apply(&mut self, key: &str, value: &str) {
        let flag = parse_bool(value);
        match key {
            "blhide" => self.enabled = flag,
            "blhide_boot" => self.boot = flag,
            "blhide_security" => self.security = flag,
            "blhide_vendor" => self.vendor = flag,
            "blhide_oem" => self.oem = flag,
            "blhide_secureboot" => self.secureboot = flag,
            "blhide_realme" => self.realme = flag,
            "blhide_recovery" => self.recovery = flag,
            "blhide_developer" => self.developer = flag,
            "blhide_selinux" => self.selinux = flag,
            "blhide_virtual" => self.virtual_device = flag,
            "blhide_delete" => self.delete = flag,
            _ => {}
        }
    }

    pub fn category(&self, category: Category) -> bool {
        match category {
            Category::Boot => self.boot,
            Category::Security => self.security,
            Category::Vendor => self.vendor,
            Category::Oem => self.oem,
            Category::SecureBoot => self.secureboot,
            Category::Realme => self.realme,
            Category::Developer => self.developer,
            Category::Selinux => self.selinux,
            Category::Virtual => self.virtual_device,
        }
    }
}

// 与主程序一致：非 0 为开，无法解析按关处理。
// Same as the main binary: non-zero is on, unparseable counts as off.
fn parse_bool(value: &str) -> bool {
    value.trim().parse::<i32>().is_ok_and(|value| value != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str, body: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "teeforge-hide-config-{}-{name}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("create fixture directory");
        let path = directory.join("config.conf");
        fs::write(&path, body).expect("write fixture");
        path
    }

    #[test]
    fn defaults_enable_every_category() {
        let config = HideConfig::default();
        for category in [
            Category::Boot,
            Category::Security,
            Category::Vendor,
            Category::Oem,
            Category::SecureBoot,
            Category::Realme,
            Category::Developer,
            Category::Selinux,
            Category::Virtual,
        ] {
            assert!(config.category(category), "{category:?} should default on");
        }
        assert!(config.delete);
    }

    #[test]
    fn missing_file_falls_back_to_defaults() {
        let config = HideConfig::load(Path::new("/nonexistent/teeforge/config.conf"));
        assert_eq!(config, HideConfig::default());
    }

    #[test]
    fn reads_switches_and_ignores_unrelated_keys() {
        let path = fixture(
            "overrides",
            "# comment\nblhide=1\nblhide_boot=0\nblhide_selinux=0\nblhide_delete=0\ndebug=1\n",
        );
        let config = HideConfig::load(&path);
        assert!(config.enabled);
        assert!(!config.boot);
        assert!(!config.selinux);
        assert!(!config.delete);
        assert!(config.security);
        let _ = fs::remove_dir_all(path.parent().expect("fixture parent"));
    }

    #[test]
    fn master_switch_and_invalid_values() {
        let path = fixture("master", "blhide=0\nblhide_boot=yes\n");
        let config = HideConfig::load(&path);
        assert!(!config.enabled);
        assert!(!config.boot);
        let _ = fs::remove_dir_all(path.parent().expect("fixture parent"));
    }
}
