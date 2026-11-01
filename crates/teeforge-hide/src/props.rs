use crate::config::HideConfig;
use crate::log::{Level, log};
use crate::resetprop;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Boot,
    Security,
    Vendor,
    Oem,
    SecureBoot,
    Realme,
    Developer,
    Selinux,
    Virtual,
}

struct Entry {
    name: &'static str,
    value: &'static str,
    category: Category,
}

// 目标值对应「官方锁定 + user 版系统」；每条属性只写一次。
// Target values describe a locked user-release system; each property is written once.
const ENTRIES: &[Entry] = &[
    // Bootloader / AVB
    Entry {
        name: "ro.boot.flash.locked",
        value: "1",
        category: Category::Boot,
    },
    Entry {
        name: "ro.boot.verifiedbootstate",
        value: "green",
        category: Category::Boot,
    },
    Entry {
        name: "ro.boot.vbmeta.device_state",
        value: "locked",
        category: Category::Boot,
    },
    Entry {
        name: "ro.boot.veritymode",
        value: "enforcing",
        category: Category::Boot,
    },
    Entry {
        name: "ro.boot.veritymode.managed",
        value: "yes",
        category: Category::Boot,
    },
    Entry {
        name: "ro.boot.warranty_bit",
        value: "0",
        category: Category::Boot,
    },
    Entry {
        name: "ro.warranty_bit",
        value: "0",
        category: Category::Boot,
    },
    Entry {
        name: "ro.boot.vbmeta.hash_alg",
        value: "sha256",
        category: Category::Boot,
    },
    // 真机属性名是 ro.boot.avb_version，如实报告 1.4；不要把版本号降级改写。
    // The real property is ro.boot.avb_version and reports 1.4; never downgrade it.
    Entry {
        name: "ro.boot.vbmeta.invalidate_on_error",
        value: "yes",
        category: Category::Boot,
    },
    // 系统安全 / 调试
    Entry {
        name: "ro.secure",
        value: "1",
        category: Category::Security,
    },
    Entry {
        name: "ro.debuggable",
        value: "0",
        category: Category::Security,
    },
    Entry {
        name: "ro.force.debuggable",
        value: "0",
        category: Category::Security,
    },
    Entry {
        name: "ro.adb.secure",
        value: "1",
        category: Category::Security,
    },
    Entry {
        name: "service.adb.root",
        value: "0",
        category: Category::Security,
    },
    Entry {
        name: "ro.crypto.state",
        value: "encrypted",
        category: Category::Security,
    },
    // 厂商同步属性
    Entry {
        name: "ro.vendor.boot.warranty_bit",
        value: "0",
        category: Category::Vendor,
    },
    Entry {
        name: "ro.vendor.warranty_bit",
        value: "0",
        category: Category::Vendor,
    },
    Entry {
        name: "vendor.boot.vbmeta.device_state",
        value: "locked",
        category: Category::Vendor,
    },
    Entry {
        name: "vendor.boot.verifiedbootstate",
        value: "green",
        category: Category::Vendor,
    },
    Entry {
        name: "vendor.boot.veritymode",
        value: "enforcing",
        category: Category::Vendor,
    },
    // OEM 解锁 / 熔丝标记
    Entry {
        name: "sys.oem_unlock_allowed",
        value: "0",
        category: Category::Oem,
    },
    Entry {
        name: "ro.oem_unlock_supported",
        value: "0",
        category: Category::Oem,
    },
    Entry {
        name: "ro.is_ever_orange",
        value: "0",
        category: Category::Oem,
    },
    Entry {
        name: "ro.boot.fmp_config",
        value: "1",
        category: Category::Oem,
    },
    Entry {
        name: "ro.boot.dp_fw_check",
        value: "1",
        category: Category::Oem,
    },
    Entry {
        name: "ro.secureboot.lockstate",
        value: "locked",
        category: Category::SecureBoot,
    },
    Entry {
        name: "ro.boot.realmebootstate",
        value: "green",
        category: Category::Realme,
    },
    Entry {
        name: "ro.boot.realme.lockstate",
        value: "1",
        category: Category::Realme,
    },
    // 开发者选项残留
    Entry {
        name: "persist.sys.developer_options",
        value: "0",
        category: Category::Developer,
    },
    Entry {
        name: "persist.sys.dev_mode",
        value: "0",
        category: Category::Developer,
    },
    Entry {
        name: "persist.sys.debuggable",
        value: "0",
        category: Category::Developer,
    },
    Entry {
        name: "ro.boot.selinux",
        value: "enforcing",
        category: Category::Selinux,
    },
    Entry {
        name: "ro.hardware.virtual_device",
        value: "0",
        category: Category::Virtual,
    },
];

// 只在确实处于 recovery 时改写，正常开机不得留下 unknown。
// Rewrite only when the device really booted into recovery; normal boots keep their value.
const RECOVERY_PROPS: &[&str] = &[
    "ro.boot.mode",
    "ro.bootmode",
    "ro.boot.bootmode",
    "vendor.boot.mode",
    "vendor.boot.bootmode",
];

const DELETE_PROPS: &[&str] = &[
    "ro.boot.verifiedbooterror",
    "ro.boot.verifyerrorpart",
    "ro.build.selinux",
];

// 分区构建标记：ro.build.* 之外，各分区还各有一份 ro.<part>.build.*。
// Per-partition build markers: ro.build.* plus ro.<part>.build.* for each partition.
const BUILD_SUFFIXES: &[(&str, &str)] = &[(".build.type", "user"), (".build.tags", "release-keys")];

pub struct Assignment {
    pub name: String,
    pub value: String,
    pub needs_write: bool,
}

#[derive(Default)]
pub struct Plan {
    pub assignments: Vec<Assignment>,
    pub deletes: Vec<String>,
    pub missing: Vec<String>,
    pub absent_deletes: usize,
}

#[derive(Default)]
pub struct Outcome {
    pub applied: usize,
    pub skipped: usize,
    pub missing: usize,
    pub failed: usize,
    pub removed: usize,
    pub absent: usize,
    pub failures: Vec<String>,
}

pub fn plan(config: &HideConfig, dump: &[(String, String)], vbmeta_size: Option<u64>) -> Plan {
    let lookup = dump
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect::<HashMap<_, _>>();
    let mut assignments = Vec::new();
    let mut missing = Vec::new();
    let mut seen = HashSet::new();

    for entry in ENTRIES {
        if !config.category(entry.category) {
            continue;
        }
        match lookup.get(entry.name) {
            Some(current) => push(
                &mut assignments,
                &mut seen,
                entry.name,
                entry.value,
                current,
            ),
            None => missing.push(entry.name.to_owned()),
        }
    }

    if config.security {
        for (name, current) in dump {
            for (suffix, value) in BUILD_SUFFIXES {
                if matches_build(name, suffix) {
                    push(&mut assignments, &mut seen, name, value, current);
                    break;
                }
            }
        }
    }

    if config.boot
        && let Some(size) = vbmeta_size
        && let Some(current) = lookup.get("ro.boot.vbmeta.size")
    {
        push(
            &mut assignments,
            &mut seen,
            "ro.boot.vbmeta.size",
            &size.to_string(),
            current,
        );
    }

    if config.recovery {
        for name in RECOVERY_PROPS {
            if let Some(current) = lookup.get(name)
                && *current == "recovery"
            {
                push(&mut assignments, &mut seen, name, "unknown", current);
            }
        }
    }

    let mut deletes = Vec::new();
    let mut absent_deletes = 0;
    if config.delete {
        for name in DELETE_PROPS {
            if lookup.contains_key(name) {
                deletes.push((*name).to_owned());
            } else {
                // 属性本来就不存在：跳过即可，绝不当作失败。
                // The property never existed: skipping is the correct outcome, not a failure.
                absent_deletes += 1;
            }
        }
    }

    Plan {
        assignments,
        deletes,
        missing,
        absent_deletes,
    }
}

fn push(
    assignments: &mut Vec<Assignment>,
    seen: &mut HashSet<String>,
    name: &str,
    value: &str,
    current: &str,
) {
    if !seen.insert(name.to_owned()) {
        return;
    }
    assignments.push(Assignment {
        name: name.to_owned(),
        value: value.to_owned(),
        needs_write: current != value,
    });
}

/// ro.build.type 或 ro.<分区>.build.type；只接受单段分区名，避免误伤。
/// Matches ro.build.type or ro.<partition>.build.type; a single segment only.
fn matches_build(name: &str, suffix: &str) -> bool {
    let Some(rest) = name.strip_prefix("ro.") else {
        return false;
    };
    if rest == &suffix[1..] {
        return true;
    }
    rest.strip_suffix(suffix)
        .is_some_and(|middle| !middle.is_empty() && !middle.contains('.'))
}

impl Plan {
    pub fn execute(&self, tool: &Path, verbose: bool) -> Outcome {
        let mut outcome = Outcome {
            missing: self.missing.len(),
            absent: self.absent_deletes,
            ..Outcome::default()
        };
        for assignment in &self.assignments {
            if !assignment.needs_write {
                outcome.skipped += 1;
                if verbose {
                    log(
                        Level::Debug,
                        format!("跳过 [skip] {}={}", assignment.name, assignment.value),
                    );
                }
                continue;
            }
            match resetprop::set(tool, &assignment.name, &assignment.value) {
                Ok(()) => {
                    outcome.applied += 1;
                    if verbose {
                        log(
                            Level::Debug,
                            format!("写入 [set] {}={}", assignment.name, assignment.value),
                        );
                    }
                }
                Err(error) => {
                    outcome.failed += 1;
                    outcome
                        .failures
                        .push(format!("{}: {error}", assignment.name));
                }
            }
        }
        for name in &self.deletes {
            match resetprop::delete(tool, name) {
                Ok(()) => {
                    outcome.removed += 1;
                    if verbose {
                        log(Level::Debug, format!("删除 [delete] {name}"));
                    }
                }
                Err(error) => {
                    outcome.failed += 1;
                    outcome.failures.push(format!("{name}: {error}"));
                }
            }
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn assignment<'a>(plan: &'a Plan, name: &str) -> Option<&'a Assignment> {
        plan.assignments
            .iter()
            .find(|assignment| assignment.name == name)
    }

    #[test]
    fn missing_properties_are_never_created() {
        let plan = plan(
            &HideConfig::default(),
            &dump(&[("ro.boot.flash.locked", "0")]),
            None,
        );
        assert_eq!(
            assignment(&plan, "ro.boot.flash.locked").map(|item| item.value.as_str()),
            Some("1")
        );
        assert!(assignment(&plan, "ro.boot.verifiedbootstate").is_none());
        assert!(
            plan.missing
                .contains(&"ro.boot.verifiedbootstate".to_owned())
        );
    }

    #[test]
    fn equal_values_are_not_rewritten() {
        let plan = plan(
            &HideConfig::default(),
            &dump(&[
                ("ro.boot.flash.locked", "1"),
                ("ro.boot.verifiedbootstate", "orange"),
            ]),
            None,
        );
        assert!(
            !assignment(&plan, "ro.boot.flash.locked")
                .unwrap()
                .needs_write
        );
        assert!(
            assignment(&plan, "ro.boot.verifiedbootstate")
                .unwrap()
                .needs_write
        );
    }

    #[test]
    fn recovery_values_are_only_fixed_in_recovery() {
        let plan = plan(
            &HideConfig::default(),
            &dump(&[("ro.bootmode", "normal"), ("ro.boot.mode", "recovery")]),
            None,
        );
        assert!(assignment(&plan, "ro.bootmode").is_none());
        assert_eq!(
            assignment(&plan, "ro.boot.mode").map(|item| item.value.as_str()),
            Some("unknown")
        );
    }

    #[test]
    fn build_markers_cover_ro_build_and_partitions() {
        let plan = plan(
            &HideConfig::default(),
            &dump(&[
                ("ro.build.type", "userdebug"),
                ("ro.system.build.type", "userdebug"),
                ("ro.vendor.build.tags", "test-keys"),
                ("ro.build.type.extra", "userdebug"),
            ]),
            None,
        );
        assert_eq!(
            assignment(&plan, "ro.build.type").map(|item| item.value.as_str()),
            Some("user")
        );
        assert_eq!(
            assignment(&plan, "ro.system.build.type").map(|item| item.value.as_str()),
            Some("user")
        );
        assert_eq!(
            assignment(&plan, "ro.vendor.build.tags").map(|item| item.value.as_str()),
            Some("release-keys")
        );
        assert!(assignment(&plan, "ro.build.type.extra").is_none());
    }

    #[test]
    fn disabled_categories_drop_their_entries() {
        let config = HideConfig {
            boot: false,
            security: false,
            ..HideConfig::default()
        };
        let plan = plan(
            &config,
            &dump(&[
                ("ro.boot.flash.locked", "0"),
                ("ro.debuggable", "1"),
                ("ro.boot.selinux", "permissive"),
            ]),
            None,
        );
        assert!(assignment(&plan, "ro.boot.flash.locked").is_none());
        assert!(assignment(&plan, "ro.debuggable").is_none());
        assert!(assignment(&plan, "ro.boot.selinux").is_some());
        assert!(!plan.missing.contains(&"ro.boot.flash.locked".to_owned()));
        assert!(!plan.missing.contains(&"ro.debuggable".to_owned()));
    }

    #[test]
    fn deletes_only_touch_existing_properties() {
        let plan = plan(
            &HideConfig::default(),
            &dump(&[("ro.build.selinux", "1")]),
            None,
        );
        assert_eq!(plan.deletes, vec!["ro.build.selinux".to_owned()]);
        assert_eq!(plan.absent_deletes, 2);
    }

    #[test]
    fn deletion_switch_keeps_properties() {
        let config = HideConfig {
            delete: false,
            ..HideConfig::default()
        };
        let plan = plan(&config, &dump(&[("ro.build.selinux", "1")]), None);
        assert!(plan.deletes.is_empty());
        assert_eq!(plan.absent_deletes, 0);
    }

    #[test]
    fn vbmeta_size_uses_probed_value_only_when_present() {
        let plan = plan(
            &HideConfig::default(),
            &dump(&[("ro.boot.vbmeta.size", "4096")]),
            Some(8192),
        );
        assert_eq!(
            assignment(&plan, "ro.boot.vbmeta.size").map(|item| item.value.as_str()),
            Some("8192")
        );

        let second = super::plan(&HideConfig::default(), &dump(&[]), Some(8192));
        assert!(assignment(&second, "ro.boot.vbmeta.size").is_none());
    }

    #[test]
    fn build_suffix_matcher_stays_narrow() {
        assert!(matches_build("ro.build.type", ".build.type"));
        assert!(matches_build("ro.system.build.type", ".build.type"));
        assert!(!matches_build("ro.build.type.extra", ".build.type"));
        assert!(!matches_build("vendor.build.type", ".build.type"));
        assert!(!matches_build("ro.system.extra.build.type", ".build.type"));
    }
}
