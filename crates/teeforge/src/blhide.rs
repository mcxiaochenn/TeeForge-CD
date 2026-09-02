use crate::config::{Config, PropTool};
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Category {
    Boot,
    Security,
    Vendor,
    Oem,
    SecureBoot,
    Realme,
    Recovery,
    Developer,
    Selinux,
    Virtual,
}

const PROPERTIES: &[(&str, &str, Category)] = &[
    ("ro.boot.vbmeta.device_state", "locked", Category::Boot),
    ("ro.boot.verifiedbootstate", "green", Category::Boot),
    ("ro.boot.flash.locked", "1", Category::Boot),
    ("ro.boot.veritymode", "enforcing", Category::Boot),
    ("ro.boot.warranty_bit", "0", Category::Boot),
    ("ro.warranty_bit", "0", Category::Boot),
    ("ro.debuggable", "0", Category::Security),
    ("ro.force.debuggable", "0", Category::Security),
    ("ro.secure", "1", Category::Security),
    ("ro.adb.secure", "1", Category::Security),
    ("ro.build.type", "user", Category::Security),
    ("ro.build.tags", "release-keys", Category::Security),
    ("ro.vendor.boot.warranty_bit", "0", Category::Vendor),
    ("ro.vendor.warranty_bit", "0", Category::Vendor),
    (
        "vendor.boot.vbmeta.device_state",
        "locked",
        Category::Vendor,
    ),
    ("vendor.boot.verifiedbootstate", "green", Category::Vendor),
    ("sys.oem_unlock_allowed", "0", Category::Oem),
    ("ro.oem_unlock_supported", "0", Category::Oem),
    ("ro.secureboot.lockstate", "locked", Category::SecureBoot),
    ("ro.boot.realmebootstate", "green", Category::Realme),
    ("ro.boot.realme.lockstate", "1", Category::Realme),
    ("ro.bootmode", "unknown", Category::Recovery),
    ("ro.boot.bootmode", "unknown", Category::Recovery),
    ("vendor.boot.bootmode", "unknown", Category::Recovery),
    ("persist.sys.developer_options", "0", Category::Developer),
    ("persist.sys.dev_mode", "0", Category::Developer),
    ("persist.sys.debuggable", "0", Category::Developer),
    ("ro.boot.selinux", "enforcing", Category::Selinux),
    ("ro.hardware.virtual_device", "0", Category::Virtual),
];

fn enabled(config: &Config, category: Category) -> bool {
    config.blhide
        && match category {
            Category::Boot => config.blhide_boot,
            Category::Security => config.blhide_security,
            Category::Vendor => config.blhide_vendor,
            Category::Oem => config.blhide_oem,
            Category::SecureBoot => config.blhide_secureboot,
            Category::Realme => config.blhide_realme,
            Category::Recovery => config.blhide_recovery,
            Category::Developer => config.blhide_developer,
            Category::Selinux => config.blhide_selinux,
            Category::Virtual => config.blhide_virtual,
        }
}

fn executable(path: &Path) -> bool {
    path.is_file()
}

fn find_tool(config: &Config) -> (PathBuf, bool) {
    if config.prop_tool == PropTool::Rs {
        if let Some(path) = std::env::var_os("RESETPROP_RS").map(PathBuf::from)
            && executable(&path)
        {
            return (path, true);
        }
        for path in [
            "/data/adb/modules/teeforge_cd/resetprop-rs/resetprop-arm64-v8a",
            "/data/adb/modules/teeforge_cd/resetprop-rs/resetprop-armeabi-v7a",
            "/data/adb/modules_update/teeforge_cd/resetprop-rs/resetprop-arm64-v8a",
            "/data/adb/modules_update/teeforge_cd/resetprop-rs/resetprop-armeabi-v7a",
        ] {
            if executable(Path::new(path)) {
                return (path.into(), true);
            }
        }
    }
    for path in [
        "/data/adb/ksu/bin/resetprop",
        "/data/adb/ap/bin/resetprop",
        "/data/adb/magisk/resetprop",
    ] {
        if executable(Path::new(path)) {
            return (path.into(), false);
        }
    }
    ("resetprop".into(), false)
}

fn boot_completed() -> bool {
    process::output("getprop", ["sys.boot_completed"])
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|value| value.trim() == "1")
}

fn run_one(tool: &Path, args: &[&str]) -> Result<()> {
    let program = tool.to_string_lossy();
    let code = process::status(&program, args)?;
    if code == 0 {
        Ok(())
    } else {
        Err(TfError::new(format!(
            "resetprop 失败，退出码 {code} [resetprop failed, exit code {code}]"
        )))
    }
}

fn apply_properties(
    config: &Config,
    is_rs: bool,
    mut apply: impl FnMut(&[&str]) -> Result<()>,
) -> Vec<String> {
    if !config.blhide {
        return Vec::new();
    }
    let mut failures = Vec::new();
    for (key, value, category) in PROPERTIES {
        if !enabled(config, *category) {
            continue;
        }
        let mut args = Vec::with_capacity(3);
        if is_rs {
            args.push("--stealth");
        }
        args.extend([*key, *value]);
        if let Err(error) = apply(&args) {
            failures.push(format!("{key}: {error}"));
        }
    }
    if config.blhide_delete
        && let Err(error) = apply(&["--delete", "ro.build.selinux"])
    {
        failures.push(format!("ro.build.selinux: {error}"));
    }
    if is_rs
        && config.blhide_compact
        && let Err(error) = apply(&["--compact"])
    {
        failures.push(format!("compact: {error}"));
    }
    failures
}

pub(crate) fn hide(config: &Config) -> Result<()> {
    if !config.blhide {
        logging::log(Level::Info, "弱隐 BL 已关闭 [Weak BL hiding is disabled]");
        return Ok(());
    }
    for _ in 0..30 {
        if boot_completed() {
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }

    let (tool, is_rs) = find_tool(config);
    let failures = apply_properties(config, is_rs, |args| run_one(&tool, args));
    if failures.is_empty() {
        logging::log(Level::Info, "弱隐 BL 完成 [Weak BL hiding done]");
        Ok(())
    } else {
        Err(TfError::new(format!(
            "{} 个属性操作失败 [property operations failed]: {}",
            failures.len(),
            failures.join("; ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_all_property_failures() {
        let config = Config::default();
        let mut attempts = 0;
        let failures = apply_properties(&config, false, |_| {
            attempts += 1;
            Err(TfError::new("mock failure"))
        });
        assert!(attempts > 1);
        assert_eq!(failures.len(), attempts);
        assert!(
            failures
                .iter()
                .any(|line| line.contains("ro.boot.vbmeta.device_state"))
        );
        assert!(
            failures
                .iter()
                .any(|line| line.contains("ro.build.selinux"))
        );
    }

    #[test]
    fn master_switch_skips_property_updates() {
        let config = Config {
            blhide: false,
            ..Config::default()
        };
        let mut attempts = 0;
        let failures = apply_properties(&config, false, |_| {
            attempts += 1;
            Ok(())
        });
        assert_eq!(attempts, 0);
        assert!(failures.is_empty());
    }
}
