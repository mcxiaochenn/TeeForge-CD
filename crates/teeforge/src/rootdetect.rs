use crate::config;
use crate::error::Result;
use crate::logging::{self, Level};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RootInfo {
    pub(crate) method: String,
    pub(crate) version: String,
}

pub(crate) fn detect() -> RootInfo {
    detect_with(
        |name| std::env::var_os(name).map(|value| value.to_string_lossy().into_owned()),
        |path| Path::new(path).is_dir(),
    )
}

fn detect_with(
    env_value: impl Fn(&str) -> Option<String>,
    is_dir: impl Fn(&str) -> bool,
) -> RootInfo {
    for (marker, version, method) in [
        ("KSU", "KSU_VER_CODE", "KernelSU"),
        ("APATCH", "APATCH_VER_CODE", "APatch"),
        ("MAGISK_VER_CODE", "MAGISK_VER_CODE", "Magisk"),
    ] {
        if env_value(marker).is_some() {
            return RootInfo {
                method: method.into(),
                version: env_value(version).unwrap_or_else(|| "unknown".into()),
            };
        }
    }

    for (path, method) in [
        ("/data/adb/ksu", "KernelSU"),
        ("/data/adb/ap", "APatch"),
        ("/data/adb/magisk", "Magisk"),
    ] {
        if is_dir(path) {
            return RootInfo {
                method: method.into(),
                version: "unknown".into(),
            };
        }
    }

    logging::log(Level::Warn, "无法识别 root 方式 [Unknown root method]");
    RootInfo {
        method: "Unknown".into(),
        version: "unknown".into(),
    }
}

pub(crate) fn save(path: &Path, info: &RootInfo) -> Result<()> {
    config::update_key(path, "root_method", &info.method)?;
    config::update_key(path, "root_version", &info.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_info_is_stable() {
        let info = detect_with(|_| None, |_| false);
        assert_eq!(info.method, "Unknown");
        assert_eq!(info.version, "unknown");
    }

    #[test]
    fn environment_has_priority_over_filesystem() {
        let info = detect_with(
            |name| match name {
                "KSU" => Some("true".into()),
                "KSU_VER_CODE" => Some("12345".into()),
                _ => None,
            },
            |path| path == "/data/adb/magisk",
        );
        assert_eq!(info.method, "KernelSU");
        assert_eq!(info.version, "12345");
    }
}
