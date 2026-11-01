use std::path::Path;
use std::process::Command;

const CANDIDATES: &[&str] = &[
    "/dev/block/by-name/vbmeta",
    "/dev/block/by-name/vbmeta_a",
    "/dev/block/bootdevice/by-name/vbmeta",
];

/// 读取真实 vbmeta 分区大小；读不到返回 None，由调用方跳过该属性。
/// Read the real vbmeta partition size; None means the property is skipped.
pub fn size() -> Option<u64> {
    for path in CANDIDATES {
        if !Path::new(path).exists() {
            continue;
        }
        let Ok(output) = Command::new("blockdev")
            .args(["--getsize64", path])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        if let Ok(value) = text.trim().parse::<u64>() {
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_cover_legacy_and_ab_layouts() {
        assert!(CANDIDATES.contains(&"/dev/block/by-name/vbmeta"));
        assert!(CANDIDATES.contains(&"/dev/block/by-name/vbmeta_a"));
        assert!(CANDIDATES.contains(&"/dev/block/bootdevice/by-name/vbmeta"));
    }
}
