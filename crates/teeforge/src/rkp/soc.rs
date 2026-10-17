//! 高通 SoC 判定 [Qualcomm SoC detection]
//!
//! 多来源取证，任一来源冲突即拒绝，不靠单个属性猜测。
//! Multiple sources are inspected; conflicting evidence blocks issuance instead of guessing.
//! 移植自 Duck-ToolBox（MIT），见 `rkp/mod.rs` 顶部的来源说明。

use crate::process;
use std::fs::File;
use std::io::Read;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Qualcomm,
    Other,
    Unknown,
    Conflicting,
}

impl State {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Qualcomm => "Qualcomm",
            Self::Other => "Other",
            Self::Unknown => "Unknown",
            Self::Conflicting => "Conflicting",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Detection {
    pub(crate) state: State,
    pub(crate) evidence: Vec<String>,
}

impl Detection {
    /// 供 `--rkp-status` 与失败信息使用的单行摘要。
    pub(crate) fn summary(&self) -> String {
        if self.evidence.is_empty() {
            return self.state.label().to_owned();
        }
        format!("{} ({})", self.state.label(), self.evidence.join(", "))
    }
}

const PROPERTIES: [&str; 9] = [
    "ro.soc.manufacturer",
    "ro.vendor.soc.manufacturer",
    "ro.soc.model",
    "ro.vendor.soc.model",
    "ro.vendor.qti.soc_model",
    "ro.hardware",
    "ro.boot.hardware",
    "ro.board.platform",
    "ro.product.board",
];

const SYSFS: [&str; 4] = [
    "/sys/devices/soc0/family",
    "/sys/devices/soc0/machine",
    "/sys/devices/system/soc/soc0/family",
    "/sys/devices/system/soc/soc0/machine",
];

#[derive(Clone, Copy)]
enum Kind {
    Manufacturer,
    Model,
    Hardware,
    Family,
}

pub(crate) fn detect() -> Detection {
    let properties = PROPERTIES
        .iter()
        .filter_map(|name| read_property(name).map(|value| ((*name).to_owned(), value)))
        .collect::<Vec<_>>();
    let sysfs = SYSFS
        .iter()
        .filter_map(|path| read_sysfs(path).map(|value| ((*path).to_owned(), value)))
        .collect::<Vec<_>>();
    classify(&properties, &sysfs)
}

fn read_property(name: &str) -> Option<String> {
    let output = process::output("getprop", [name]).ok()?;
    let value = process::stdout_text(output, name).ok()?;
    non_empty(value.trim())
}

fn read_sysfs(path: &str) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut value = String::new();
    // sysfs 属性可能报告整页长度，按 256 字节实际上限截断读取。
    // sysfs attributes can report a page-sized length; bound the actual read at 256 bytes.
    file.take(257).read_to_string(&mut value).ok()?;
    if value.len() > 256 {
        return None;
    }
    non_empty(value.trim())
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "unknown" | "null" | "none" | "n/a"
        )
    {
        return None;
    }
    Some(value.to_owned())
}

/// 纯函数形式便于主机侧测试 [Pure form so host tests can cover it].
pub(crate) fn classify(properties: &[(String, String)], sysfs: &[(String, String)]) -> Detection {
    let mut evidence = Vec::new();
    let mut qualcomm = false;
    let mut other = false;

    for (source, value) in properties {
        let kind = if source.contains("manufacturer") {
            Kind::Manufacturer
        } else if source.contains("soc_model") || source.ends_with("soc.model") {
            Kind::Model
        } else {
            Kind::Hardware
        };
        if let Some(vendor) = vendor_of(value, kind) {
            evidence.push(format!("{source}={value}"));
            match vendor {
                Vendor::Qualcomm => qualcomm = true,
                Vendor::Other => other = true,
            }
        }
    }

    for (source, value) in sysfs {
        let kind = if source.ends_with("family") {
            Kind::Family
        } else {
            Kind::Model
        };
        if let Some(vendor) = vendor_of(value, kind) {
            evidence.push(format!("{source}={value}"));
            match vendor {
                Vendor::Qualcomm => qualcomm = true,
                Vendor::Other => other = true,
            }
        }
    }

    let state = match (qualcomm, other) {
        (true, false) => State::Qualcomm,
        (false, true) => State::Other,
        (false, false) => State::Unknown,
        (true, true) => State::Conflicting,
    };
    Detection { state, evidence }
}

enum Vendor {
    Qualcomm,
    Other,
}

fn vendor_of(value: &str, kind: Kind) -> Option<Vendor> {
    match kind {
        Kind::Manufacturer => manufacturer(value),
        Kind::Model => qualcomm_model(value).then_some(Vendor::Qualcomm),
        Kind::Hardware => {
            manufacturer(value).or_else(|| qualcomm_model(value).then_some(Vendor::Qualcomm))
        }
        Kind::Family => {
            if value.eq_ignore_ascii_case("snapdragon") {
                Some(Vendor::Qualcomm)
            } else {
                manufacturer(value)
            }
        }
    }
}

fn manufacturer(value: &str) -> Option<Vendor> {
    let normalized = value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    match normalized.as_str() {
        "qti"
        | "qcom"
        | "qualcomm"
        | "qualcomm inc"
        | "qualcomm incorporated"
        | "qualcomm technologies"
        | "qualcomm technologies inc"
        | "qualcomm technologies incorporated"
        | "qualcomm technologies ltd" => Some(Vendor::Qualcomm),
        "mediatek"
        | "mediatek inc"
        | "mediatek incorporated"
        | "mtk"
        | "samsung"
        | "samsung electronics"
        | "samsung electronics co ltd"
        | "exynos"
        | "google"
        | "google inc"
        | "google tensor"
        | "tensor"
        | "hisilicon"
        | "hisilicon technologies co ltd"
        | "kirin"
        | "unisoc"
        | "spreadtrum"
        | "sprd"
        | "rockchip"
        | "amlogic"
        | "nvidia"
        | "intel"
        | "amd"
        | "apple"
        | "broadcom" => Some(Vendor::Other),
        _ => None,
    }
}

fn qualcomm_model(value: &str) -> bool {
    let model = value.to_ascii_uppercase();
    let (part, suffix) = model.split_once('-').unwrap_or((&model, ""));
    if suffix.len() > 4 || !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return false;
    }
    ["MSM", "APQ", "SDM", "SM", "QCM", "QCS"]
        .iter()
        .any(|prefix| {
            let Some(number) = part.strip_prefix(prefix) else {
                return false;
            };
            let digits = number.bytes().take_while(u8::is_ascii_digit).count();
            let expected = if matches!(*prefix, "SM" | "QCM" | "QCS") {
                4..=4
            } else {
                3..=4
            };
            expected.contains(&digits)
                && number.len() - digits <= 2
                && number[digits..]
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(values: &[(&str, &str)]) -> Vec<(String, String)> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn qualcomm_manufacturer_is_accepted() {
        let detection = classify(&pairs(&[("ro.soc.manufacturer", "Qualcomm")]), &[]);
        assert_eq!(detection.state, State::Qualcomm);
        assert_eq!(detection.evidence.len(), 1);
    }

    #[test]
    fn qualcomm_part_number_is_accepted_without_manufacturer() {
        let detection = classify(&pairs(&[("ro.board.platform", "SM8750")]), &[]);
        assert_eq!(detection.state, State::Qualcomm);
    }

    #[test]
    fn mediatek_is_rejected() {
        let detection = classify(&pairs(&[("ro.soc.manufacturer", "MediaTek")]), &[]);
        assert_eq!(detection.state, State::Other);
    }

    #[test]
    fn conflicting_sources_block_issuance() {
        let detection = classify(
            &pairs(&[
                ("ro.soc.manufacturer", "Qualcomm"),
                ("ro.vendor.soc.manufacturer", "MediaTek"),
            ]),
            &[],
        );
        assert_eq!(detection.state, State::Conflicting);
    }

    #[test]
    fn unknown_values_are_ignored() {
        let detection = classify(&pairs(&[("ro.soc.manufacturer", "unknown")]), &[]);
        assert_eq!(detection.state, State::Unknown);
        assert!(detection.evidence.is_empty());
    }

    #[test]
    fn sysfs_family_snapdragon_counts_as_qualcomm() {
        let detection = classify(&[], &pairs(&[("/sys/devices/soc0/family", "Snapdragon")]));
        assert_eq!(detection.state, State::Qualcomm);
    }
}
