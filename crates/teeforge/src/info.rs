//! 首屏所需的一次性信息采集 [One-shot information gathering for the UI]
//!
//! WebUI 打开时会调用 `--bootstrap --json`：一次 shell 调用拿到设备、Root、
//! 模块版本与能力位，避免首屏发四五条命令。
//! The UI calls `--bootstrap --json` once instead of issuing several shell calls.

use crate::config::{Config, KeyboxSource, TargetMode};
use crate::logging::{self, Level};
use crate::process;
use crate::rkp;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;

const MODULE_PROP: &str = "/data/adb/modules/teeforge_cd/module.prop";

/// `getprop` 全量输出解析成键值表。
fn properties() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Ok(output) = process::output("getprop", std::iter::empty::<&str>()) else {
        return map;
    };
    let Ok(text) = process::stdout_text(output, "getprop") else {
        return map;
    };
    for line in text.lines() {
        // 形如 `[ro.product.brand]: [Xiaomi]`
        let Some(rest) = line.strip_prefix('[') else {
            continue;
        };
        let Some((key, value)) = rest.split_once("]: [") else {
            continue;
        };
        let value = value.strip_suffix(']').unwrap_or(value);
        map.insert(key.to_owned(), value.trim().to_owned());
    }
    map
}

fn prop<'a>(map: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    map.get(key).map(String::as_str).unwrap_or_default()
}

fn command_text(program: &str, args: &[&str]) -> String {
    process::output(program, args)
        .ok()
        .and_then(|output| process::stdout_text(output, program).ok())
        .map(|text| text.trim().to_owned())
        .unwrap_or_default()
}

fn module_identity() -> (String, u64) {
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

/// 组装首屏数据。
pub(crate) fn bootstrap_value(config: &Config) -> Value {
    let props = properties();
    let brand = prop(&props, "ro.product.brand");
    let market = prop(&props, "ro.product.marketname");
    let model = prop(&props, "ro.product.model");
    let display_model = if market.is_empty() {
        format!("{brand} {model}").trim().to_owned()
    } else {
        market.to_owned()
    };
    let (version, version_code) = module_identity();
    let diagnosis = rkp::diagnose();
    let artifacts_dir = config.rkp_dir.join("keybox.xml");

    logging::log(Level::Debug, "首屏信息采集完成 [bootstrap collected]");
    json!({
        "device": {
            "brand": brand,
            "model": model,
            "name": display_model,
            "android": prop(&props, "ro.build.version.release"),
            "sdk": prop(&props, "ro.build.version.sdk"),
            "security_patch": prop(&props, "ro.build.version.security_patch"),
            "abi": prop(&props, "ro.product.cpu.abi"),
            "soc_model": prop(&props, "ro.soc.model"),
            "soc_manufacturer": prop(&props, "ro.soc.manufacturer"),
            "verified_boot": prop(&props, "ro.boot.verifiedbootstate"),
        },
        "system": {
            "kernel": command_text("uname", &["-r"]),
            "selinux": command_text("getenforce", &[]),
        },
        "module": {
            "version": version,
            "version_code": version_code,
        },
        "root": {
            "method": config.root_method,
            "version": config.root_version,
        },
        "config": {
            "keybox_source": config.keybox_source.label(),
            "target_mode": config.target_mode.label(),
            "update_channel": config.update_channel.label(),
        },
        "capabilities": {
            "rkp_platform": rkp::device_path_supported(),
            "rkp_supported": diagnosis.blockers.is_empty(),
            "rkp_artifacts": artifacts_dir.is_file(),
        },
        "paths": {
            "rkp_dir": config.rkp_dir.display().to_string(),
            "keybox_dir": config.keybox_dir.display().to_string(),
        },
        "defaults": {
            "keybox_source": KeyboxSource::Remote.label(),
            "target_mode": TargetMode::Auto.label(),
        }
    })
}

/// 供 CLI 文本模式使用的摘要行。
pub(crate) fn bootstrap_lines(config: &Config) -> Vec<String> {
    let value = bootstrap_value(config);
    let text = |path: &[&str]| {
        path.iter()
            .fold(&value, |current, key| &current[*key])
            .as_str()
            .unwrap_or_default()
            .to_owned()
    };
    vec![
        format!(
            "设备 Device: {} {}",
            text(&["device", "brand"]),
            text(&["device", "model"])
        ),
        format!(
            "系统 System: Android {} (SDK {})",
            text(&["device", "android"]),
            text(&["device", "sdk"])
        ),
        format!(
            "模块 Module: {} ({})",
            text(&["module", "version"]),
            text(&["module", "version_code"])
        ),
        format!(
            "Root: {} {}",
            text(&["root", "method"]),
            text(&["root", "version"])
        ),
        format!(
            "Keybox 来源 Source: {} / 目标模式 Target: {}",
            text(&["config", "keybox_source"]),
            text(&["config", "target_mode"])
        ),
        format!(
            "设备端 RKP: {} / 产物: {}",
            if value["capabilities"]["rkp_supported"] == json!(true) {
                "可用 available"
            } else {
                "不可用 unavailable"
            },
            if value["capabilities"]["rkp_artifacts"] == json!(true) {
                "已生成 present"
            } else {
                "未生成 absent"
            }
        ),
    ]
}
