use crate::config::Config;
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

fn find_tool() -> PathBuf {
    for path in [
        "/data/adb/ksu/bin/resetprop",
        "/data/adb/ap/bin/resetprop",
        "/data/adb/magisk/resetprop",
    ] {
        if executable(Path::new(path)) {
            return path.into();
        }
    }
    "resetprop".into()
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

fn apply_properties(config: &Config, mut apply: impl FnMut(&[&str]) -> Result<()>) -> Vec<String> {
    if !config.blhide {
        return Vec::new();
    }
    let mut failures = Vec::new();
    for (key, value, category) in PROPERTIES {
        if !enabled(config, *category) {
            continue;
        }
        if let Err(error) = apply(&[*key, *value]) {
            failures.push(format!("{key}: {error}"));
        }
    }
    if config.blhide_delete
        && let Err(error) = apply(&["--delete", "ro.build.selinux"])
    {
        failures.push(format!("ro.build.selinux: {error}"));
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

    let tool = find_tool();
    let failures = apply_properties(config, |args| run_one(&tool, args));
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
        let failures = apply_properties(&config, |_| {
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
        let failures = apply_properties(&config, |_| {
            attempts += 1;
            Ok(())
        });
        assert_eq!(attempts, 0);
        assert!(failures.is_empty());
    }

    #[test]
    fn standard_arguments_never_include_rs_options() {
        let mut calls = Vec::new();
        assert!(
            apply_properties(&Config::default(), |args| {
                calls.push(
                    args.iter()
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>(),
                );
                Ok(())
            })
            .is_empty()
        );
        assert!(calls.iter().all(|args| args.len() == 2));
        assert!(calls.iter().all(|args| {
            !args
                .iter()
                .any(|arg| arg == "--stealth" || arg == "--compact")
        }));
        assert!(
            calls
                .iter()
                .any(|args| args == &["--delete", "ro.build.selinux"])
        );
    }
}
