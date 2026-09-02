use crate::config::{self, Config};
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::rootdetect;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

const MODULE_PROP: &str = "/data/adb/modules/teeforge_cd/module.prop";

pub(crate) fn update(config: &Config) -> Result<()> {
    let path = Path::new(MODULE_PROP);
    if !path.is_file() {
        return Err(TfError::new(
            "模块未安装，跳过描述更新 [Module not installed; skipping description update]",
        ));
    }
    let root = rootdetect::detect();
    let keybox = config.keybox_dir.join("keybox.xml");
    let keybox_time = fs::metadata(&keybox)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or_else(|| "N/A".into(), |value| value.as_secs().to_string());
    let face = if root.method == "Unknown" {
        "(;´д`)"
    } else if keybox_time == "N/A" {
        "(・・?"
    } else {
        "( •̀ ω •́ )✧"
    };
    let description = format!(
        "{face} ✅ [{}] {} | arch: {} | keybox: {keybox_time}",
        root.method,
        crate::VERSION,
        android_abi()
    );
    config::update_key(path, "description", &description)?;
    logging::log(
        Level::Info,
        format!("模块描述已更新 [Description updated]: {description}"),
    );
    Ok(())
}

fn android_abi() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64-v8a"
    } else if cfg!(target_arch = "arm") {
        "armeabi-v7a"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "unknown"
    }
}
