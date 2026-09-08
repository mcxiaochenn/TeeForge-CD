use crate::atomic_file;
use crate::config::Config;
use crate::error::{Result, TfError};
use crate::logging::{self, Level};
use crate::process;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const TARGET_BEGIN: &str = "# BEGIN TeeForge-CD managed targets";
const TARGET_END: &str = "# END TeeForge-CD managed targets";
const PACKAGE_LIST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PACKAGE_LIST_BYTES: usize = 4 * 1024 * 1024;
const TEESIM_PROFILE: &str = "teeforge";
const TEESIM_MARKER: &str = "_teeforgeManaged";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PackageRecord {
    pub(crate) name: String,
    pub(crate) uids: Vec<u32>,
}

pub(crate) fn parse_user_packages(text: &str) -> Result<Vec<PackageRecord>> {
    let mut packages = BTreeMap::<String, HashSet<u32>>::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let rest = line.strip_prefix("package:").ok_or_else(|| {
            TfError::new(format!(
                "包列表输出含无法识别的行 [package listing contains malformed line]: {line}"
            ))
        })?;
        // Android 的随机安装目录可能包含 Base64 padding；真正的分隔符是最后一个 '='。
        // Randomized Android install paths may contain Base64 padding; the final '=' is the delimiter.
        let (path, details) = rest.rsplit_once('=').ok_or_else(|| {
            TfError::new(format!(
                "包列表输出缺少路径分隔符 [package listing lacks path separator]: {line}"
            ))
        })?;
        if !path.starts_with('/') {
            return Err(TfError::new(format!(
                "包列表输出含无效 APK 路径 [package listing contains invalid APK path]: {line}"
            )));
        }
        let mut fields = details.split_whitespace();
        let name = fields
            .next()
            .filter(|name| valid_package_name(name))
            .ok_or_else(|| {
                TfError::new(format!(
                    "包列表输出含无效包名 [package listing contains invalid package]: {line}"
                ))
            })?;
        let mut line_uids = HashSet::new();
        for field in fields {
            let uids = field.strip_prefix("uid:").ok_or_else(|| {
                TfError::new(format!(
                    "包列表输出含无法识别字段 [package listing contains unknown field]: {line}"
                ))
            })?;
            if uids.is_empty() {
                return Err(TfError::new(format!(
                    "包列表输出含空 UID [package listing contains empty UID]: {line}"
                )));
            }
            for uid in uids.split(',') {
                line_uids.insert(uid.parse::<u32>().map_err(|_| {
                    TfError::new(format!(
                        "包列表输出含无效 UID [package listing contains invalid UID]: {line}"
                    ))
                })?);
            }
        }
        if line_uids.is_empty() {
            return Err(TfError::new(format!(
                "包列表输出缺少 UID [package listing lacks UID]: {line}"
            )));
        }
        if path.starts_with("/data/app/") {
            packages
                .entry(name.to_owned())
                .or_default()
                .extend(line_uids);
        }
    }
    Ok(packages
        .into_iter()
        .map(|(name, uids)| {
            let mut uids = uids.into_iter().collect::<Vec<_>>();
            uids.sort_unstable();
            PackageRecord { name, uids }
        })
        .collect())
}

fn valid_package_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.')
}

fn package_line_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
        return None;
    }
    let name = trimmed
        .strip_suffix('!')
        .or_else(|| trimmed.strip_suffix('?'));
    let name = name.unwrap_or(trimmed).trim();
    valid_package_name(name).then_some(name)
}

fn render_target_file(text: &str, packages: &[PackageRecord]) -> Result<String> {
    let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let begin = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (line.trim() == TARGET_BEGIN).then_some(index))
        .collect::<Vec<_>>();
    let end = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (line.trim() == TARGET_END).then_some(index))
        .collect::<Vec<_>>();
    if begin.len() > 1 || end.len() > 1 || begin.len() != end.len() {
        return Err(TfError::new(
            "target.txt 管理区块标记不完整或重复 [target.txt managed markers are incomplete or duplicated]",
        ));
    }
    if let (Some(begin), Some(end)) = (begin.first(), end.first())
        && begin >= end
    {
        return Err(TfError::new(
            "target.txt 管理区块顺序无效 [target.txt managed marker order is invalid]",
        ));
    }

    let (outside, legacy_generated) = if let (Some(begin), Some(end)) = (begin.first(), end.first())
    {
        let mut outside = Vec::with_capacity(lines.len().saturating_sub(end - begin + 1));
        outside.extend(lines[..*begin].iter().cloned());
        outside.extend(lines[*end + 1..].iter().cloned());
        // 区块后的第一个空行是本工具生成的分隔符；先移除再统一补回，保证重复执行幂等。
        // The first blank after the block is our separator; remove and re-add it canonically.
        if outside.first().is_some_and(String::is_empty) {
            outside.remove(0);
        }
        (outside, false)
    } else {
        let legacy_generated = lines.iter().all(|line| {
            let trimmed = line.trim();
            trimmed.is_empty() || package_line_name(trimmed) == Some(trimmed)
        });
        if legacy_generated {
            (Vec::new(), true)
        } else {
            (lines, false)
        }
    };

    // 受管区块之外的合法条目归用户所有；自动生成时不得重复或改变其模式。
    // Valid entries outside the managed block belong to the user and keep their modes.
    let user_packages = if legacy_generated {
        HashSet::new()
    } else {
        outside
            .iter()
            .filter_map(|line| package_line_name(line))
            .map(str::to_owned)
            .collect::<HashSet<_>>()
    };
    let managed = packages
        .iter()
        .filter(|package| !user_packages.contains(&package.name))
        .map(|package| package.name.as_str())
        .collect::<Vec<_>>();

    let mut rendered = Vec::with_capacity(managed.len() + outside.len() + 2);
    rendered.push(TARGET_BEGIN.to_owned());
    rendered.extend(managed.into_iter().map(str::to_owned));
    rendered.push(TARGET_END.to_owned());
    if !outside.is_empty() {
        rendered.push(String::new());
        rendered.extend(outside);
    }
    Ok(format!("{}\n", rendered.join("\n")))
}

fn profile_entry(entry: &str) -> Result<ProfileEntry<'_>> {
    if let Some(uid) = entry.strip_prefix("uid:") {
        let uid = uid.parse::<u32>().map_err(|_| {
            TfError::new(format!(
                "TEESimulator apps 含无效 UID [TEESimulator apps contains invalid UID]: {entry}"
            ))
        })?;
        return Ok(ProfileEntry::Uid(uid));
    }
    let (package, user) = match entry.rsplit_once('@') {
        Some((package, user)) if user.bytes().all(|byte| byte.is_ascii_digit()) => {
            (package, Some(user.parse::<u32>().map_err(|_| {
                TfError::new(format!(
                    "TEESimulator apps 含无效用户 [TEESimulator apps contains invalid user]: {entry}"
                ))
            })?))
        }
        _ => (entry, None),
    };
    if !valid_package_name(package) {
        return Err(TfError::new(format!(
            "TEESimulator apps 含无效包名 [TEESimulator apps contains invalid package]: {entry}"
        )));
    }
    Ok(ProfileEntry::Package(package, user.unwrap_or(0)))
}

enum ProfileEntry<'a> {
    Package(&'a str, u32),
    Uid(u32),
}

fn render_teesim_config(text: &str, packages: &[PackageRecord]) -> Result<String> {
    let mut root: Value = serde_json::from_str(text).map_err(|error| {
        TfError::new(format!(
            "TEESimulator config.json 无效 [invalid config.json]: {error}"
        ))
    })?;
    let root_object = root.as_object_mut().ok_or_else(|| {
        TfError::new("TEESimulator config.json 根节点不是对象 [config.json root is not an object]")
    })?;
    if root_object.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(TfError::new(
            "TEESimulator config.json 版本不是 1 [config.json version is not 1]",
        ));
    }
    let profiles = root_object
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            TfError::new(
                "TEESimulator config.json 缺少 profiles 对象 [config.json lacks profiles object]",
            )
        })?;
    if profiles.is_empty() {
        return Err(TfError::new(
            "TEESimulator config.json 没有 profile [config.json has no profiles]",
        ));
    }

    // 其他 profile 对主用户包名和 UID 的声明优先，避免同一应用被重复分配。
    // Claims from other profiles take priority for primary-user package names and UIDs.
    let mut claimed_packages = HashSet::new();
    let mut claimed_uids = HashSet::new();
    for (id, profile) in profiles.iter() {
        if id == TEESIM_PROFILE {
            continue;
        }
        let profile = profile.as_object().ok_or_else(|| {
            TfError::new(format!(
                "TEESimulator profile 不是对象 [profile is not an object]: {id}"
            ))
        })?;
        let Some(apps) = profile.get("apps") else {
            continue;
        };
        let apps = apps.as_array().ok_or_else(|| {
            TfError::new(format!(
                "TEESimulator profile.apps 不是数组 [profile.apps is not an array]: {id}"
            ))
        })?;
        for entry in apps {
            let entry = entry.as_str().ok_or_else(|| {
                TfError::new(format!(
                    "TEESimulator profile.apps 含非字符串 [profile.apps contains non-string]: {id}"
                ))
            })?;
            match profile_entry(entry)? {
                ProfileEntry::Package(package, 0) => {
                    claimed_packages.insert(package.to_owned());
                }
                ProfileEntry::Package(_, _) => {}
                ProfileEntry::Uid(uid) => {
                    claimed_uids.insert(uid);
                }
            }
        }
    }

    let selected = if let Some(profile) = profiles.get_mut(TEESIM_PROFILE) {
        let profile = profile.as_object_mut().ok_or_else(|| {
            TfError::new(
                "TEESimulator teeforge profile 不是对象 [teeforge profile is not an object]",
            )
        })?;
        if profile.get(TEESIM_MARKER) != Some(&Value::Bool(true)) {
            return Err(TfError::new(
                "TEESimulator teeforge profile 缺少管理标记 [teeforge profile lacks management marker]",
            ));
        }
        if !profile.get("apps").is_some_and(Value::is_array) {
            return Err(TfError::new(
                "TEESimulator teeforge.apps 不是数组 [teeforge.apps is not an array]",
            ));
        }
        profile
    } else {
        let template = profiles.get("default").and_then(Value::as_object).ok_or_else(|| {
            TfError::new(
                "TEESimulator 首次创建需要合法 default profile [first setup requires a valid default profile]",
            )
        })?;
        profiles.insert(TEESIM_PROFILE.to_owned(), Value::Object(template.clone()));
        profiles
            .get_mut(TEESIM_PROFILE)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| {
                TfError::new(
                    "无法创建 TEESimulator teeforge profile [unable to create teeforge profile]",
                )
            })?
    };
    selected.insert(TEESIM_MARKER.to_owned(), Value::Bool(true));
    let apps = packages
        .iter()
        .filter(|package| {
            !claimed_packages.contains(&package.name)
                && !package.uids.iter().any(|uid| claimed_uids.contains(uid))
        })
        .map(|package| Value::String(package.name.clone()))
        .collect::<Vec<_>>();
    selected.insert("apps".to_owned(), Value::Array(apps));

    serde_json::to_string_pretty(&root)
        .map(|json| format!("{json}\n"))
        .map_err(|error| {
            TfError::new(format!(
                "TEESimulator config.json 序列化失败 [failed to serialize config.json]: {error}"
            ))
        })
}

struct PendingUpdate {
    label: &'static str,
    path: PathBuf,
    original: Option<Vec<u8>>,
    rendered: Vec<u8>,
}

impl PendingUpdate {
    fn from_text(label: &'static str, path: &Path, rendered: String) -> Result<Self> {
        let original = match fs::read(path) {
            Ok(value) => Some(value),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(TfError::from(error).context(path.display())),
        };
        Ok(Self {
            label,
            path: path.to_path_buf(),
            original,
            rendered: rendered.into_bytes(),
        })
    }

    fn changed(&self) -> bool {
        self.original.as_deref() != Some(self.rendered.as_slice())
    }
}

fn commit_updates(updates: &[PendingUpdate]) -> Result<()> {
    // 写入前再次核对原始内容，避免覆盖读取后由其他进程完成的更新。
    // Recheck originals before writing so concurrent updates are never overwritten.
    for update in updates.iter().filter(|update| update.changed()) {
        let current = match fs::read(&update.path) {
            Ok(value) => Some(value),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(TfError::from(error).context(update.path.display())),
        };
        if current != update.original {
            return Err(TfError::new(format!(
                "目标配置在读取后发生变化，未覆盖 [target config changed during update]: {}",
                update.path.display()
            )));
        }
    }

    let mut written: Vec<&PendingUpdate> = Vec::new();
    for update in updates.iter().filter(|update| update.changed()) {
        let result = if update.original.is_some() {
            atomic_file::write_with_backup(&update.path, &update.rendered)
        } else {
            atomic_file::write(&update.path, &update.rendered)
        };
        if let Err(error) = result {
            // 多后端作为一个事务提交，任一失败都恢复本轮已写入的目标。
            // Multiple backends commit as one transaction; any failure rolls back this run.
            let mut rollback_errors = Vec::new();
            for previous in written.into_iter().rev() {
                if let Err(rollback_error) = restore_update(previous) {
                    rollback_errors.push(format!("{}: {rollback_error}", previous.path.display()));
                }
            }
            if !rollback_errors.is_empty() {
                return Err(TfError::new(format!(
                    "目标配置写入失败且回滚失败 [target config write and rollback failed]: {error}; {}",
                    rollback_errors.join("; ")
                )));
            }
            return Err(error.context(update.path.display()));
        }
        written.push(update);
    }
    Ok(())
}

fn restore_update(update: &PendingUpdate) -> Result<()> {
    match &update.original {
        Some(original) => atomic_file::write(&update.path, original),
        None => match fs::remove_file(&update.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        },
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GenerateOutcome {
    SkippedNoBackend,
    Unchanged {
        package_count: usize,
        backend_count: usize,
    },
    Updated {
        package_count: usize,
        backend_count: usize,
    },
}

impl GenerateOutcome {
    fn message(&self) -> String {
        match self {
            Self::SkippedNoBackend => "未发现兼容目标配置，已跳过目标更新 [No compatible target backend found; target update skipped]".to_owned(),
            Self::Unchanged {
                package_count,
                backend_count,
            } => format!(
                "目标配置已是最新，无需写入（已处理 {package_count} 个用户应用、检查 {backend_count} 个目标配置） [Target configs are already current; no write needed; processed {package_count} user apps and checked {backend_count} target configs]"
            ),
            Self::Updated {
                package_count,
                backend_count,
            } => format!(
                "目标配置更新完成（已处理 {package_count} 个用户应用、更新 {backend_count} 个目标配置） [Target config update complete; processed {package_count} user apps and updated {backend_count} target configs]"
            ),
        }
    }

    pub(crate) fn log(&self) {
        let level = match self {
            Self::SkippedNoBackend => Level::Warn,
            Self::Unchanged { .. } | Self::Updated { .. } => Level::Info,
        };
        logging::log(level, self.message());
    }
}

fn generate_with(
    config: &Config,
    list_packages: impl FnOnce() -> Result<String>,
) -> Result<GenerateOutcome> {
    let target_active = config
        .target_txt
        .parent()
        .is_some_and(|parent| parent.is_dir());
    let teesim_active = config.teesim_config.is_file();
    if !target_active && !teesim_active {
        return Ok(GenerateOutcome::SkippedNoBackend);
    }
    let backend_count = usize::from(target_active) + usize::from(teesim_active);
    logging::log(
        Level::Info,
        format!(
            "检测到 {backend_count} 个兼容目标配置 [Detected {backend_count} compatible target configs]"
        ),
    );
    logging::log(
        Level::Info,
        "正在获取已安装包列表，最多等待 30 秒... [Listing installed packages; timeout: 30s]",
    );
    let text = list_packages()?;
    let packages = parse_user_packages(&text)?;
    logging::log(
        Level::Info,
        format!(
            "已发现 {} 个用户应用 [Found {} user apps]",
            packages.len(),
            packages.len()
        ),
    );
    let mut updates = Vec::new();
    if target_active {
        let current = match fs::read(&config.target_txt) {
            Ok(value) => String::from_utf8(value).map_err(|_| {
                TfError::new(format!(
                    "target.txt 不是 UTF-8，未覆盖 [target.txt is not UTF-8; refusing to overwrite]: {}",
                    config.target_txt.display()
                ))
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(TfError::from(error).context(config.target_txt.display())),
        };
        let rendered = render_target_file(&current, &packages)?;
        updates.push(PendingUpdate::from_text(
            "Tricky Store/TEESimulator-RS target.txt",
            &config.target_txt,
            rendered,
        )?);
    }
    if teesim_active {
        let current = fs::read_to_string(&config.teesim_config)
            .map_err(|error| TfError::from(error).context(config.teesim_config.display()))?;
        let rendered = render_teesim_config(&current, &packages)?;
        updates.push(PendingUpdate::from_text(
            "TEESimulator config.json",
            &config.teesim_config,
            rendered,
        )?);
    }
    let changed = updates.iter().filter(|update| update.changed()).count();
    commit_updates(&updates)?;
    for update in &updates {
        let state = if update.changed() {
            "已更新 [updated]"
        } else {
            "已是最新 [already current]"
        };
        logging::log(Level::Info, format!("{}：{state}", update.label));
    }
    if changed == 0 {
        Ok(GenerateOutcome::Unchanged {
            package_count: packages.len(),
            backend_count: updates.len(),
        })
    } else {
        Ok(GenerateOutcome::Updated {
            package_count: packages.len(),
            backend_count: changed,
        })
    }
}

pub(crate) fn generate(config: &Config) -> Result<GenerateOutcome> {
    generate_with(config, || {
        let context = "获取包列表失败 [Failed to list packages]";
        let output = process::output_bounded(
            "cmd",
            ["package", "list", "packages", "-f", "-U", "--user", "0"],
            PACKAGE_LIST_TIMEOUT,
            MAX_PACKAGE_LIST_BYTES,
            context,
        )?;
        let text = process::stdout_text(output, context)?;
        logging::log(
            Level::Info,
            "包列表获取完成，正在解析... [Package listing received; parsing...]",
        );
        Ok(text)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packages(names: &[&str]) -> Vec<PackageRecord> {
        names
            .iter()
            .map(|name| PackageRecord {
                name: (*name).into(),
                uids: Vec::new(),
            })
            .collect()
    }

    fn test_config(name: &str) -> (Config, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("teeforge-target-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let config = Config {
            target_txt: root.join("tricky_store").join("target.txt"),
            teesim_config: root.join("teesim").join("config.json"),
            ..Config::default()
        };
        (config, root)
    }

    #[test]
    fn parses_only_primary_user_apps_and_uids() {
        let input = "package:/system/app/System/System.apk=com.android.system uid:1000\npackage:/data/app/~~token/example/base.apk=com.example.user uid:10123\npackage:/data/app/duplicate/base.apk=com.example.user uid:10123\n";
        assert_eq!(
            parse_user_packages(input).unwrap(),
            vec![PackageRecord {
                name: "com.example.user".into(),
                uids: vec![10123],
            }]
        );
    }

    #[test]
    fn parses_user_app_when_apk_path_contains_equals_signs() {
        let input = "package:/data/app/~~token==/com.example.user-random==/base.apk=com.example.user uid:10123\n";
        assert_eq!(
            parse_user_packages(input).expect("path padding is valid"),
            vec![PackageRecord {
                name: "com.example.user".into(),
                uids: vec![10123],
            }]
        );
    }

    #[test]
    fn parses_no_fixed_limit_and_sorts_packages() {
        let mut input = String::new();
        for index in (0..2_100).rev() {
            input.push_str(&format!(
                "package:/data/app/~~token/app{index}/base.apk=com.example.app{index:04} uid:{}\n",
                10_000 + index
            ));
        }
        let result = parse_user_packages(&input).unwrap();
        assert_eq!(result.len(), 2_100);
        assert_eq!(result[0].name, "com.example.app0000");
        assert_eq!(result[2_099].name, "com.example.app2099");
    }

    #[test]
    fn rejects_malformed_package_listing_lines() {
        assert!(parse_user_packages("not-a-package-line\n").is_err());
        assert!(parse_user_packages("package:/data/app/a.apk=com.example.app\n").is_err());
        assert!(
            parse_user_packages("package:/data/app/a.apk=com.example.app uid:not-a-number\n")
                .is_err()
        );
    }

    #[test]
    fn migrates_legacy_bare_target_file_to_managed_block() {
        let rendered = render_target_file("com.old\n", &packages(&["com.new"])).unwrap();
        assert_eq!(
            rendered,
            "# BEGIN TeeForge-CD managed targets\ncom.new\n# END TeeForge-CD managed targets\n"
        );
    }

    #[test]
    fn preserves_custom_target_content_and_modes() {
        let current = "# user config\ncom.keep!\n[custom.xml]\ncom.custom?\n";
        let rendered = render_target_file(current, &packages(&["com.keep", "com.new"])).unwrap();
        assert!(rendered.starts_with("# BEGIN TeeForge-CD managed targets\ncom.new\n"));
        assert!(rendered.contains("# user config\ncom.keep!\n[custom.xml]\ncom.custom?\n"));
        assert!(!rendered.contains("\ncom.keep\n"));
    }

    #[test]
    fn managed_block_rebuilds_and_removes_uninstalled_packages() {
        let current = "# BEGIN TeeForge-CD managed targets\ncom.old\n# END TeeForge-CD managed targets\n\ncom.user?\n";
        let rendered = render_target_file(current, &packages(&["com.new"])).unwrap();
        assert!(rendered.contains("com.new"));
        assert!(!rendered.contains("com.old"));
        assert!(rendered.contains("com.user?"));
    }

    #[test]
    fn rejects_ambiguous_target_markers() {
        assert!(
            render_target_file(
                "# BEGIN TeeForge-CD managed targets\ncom.a\n",
                &packages(&["com.a"])
            )
            .is_err()
        );
        assert!(
            render_target_file(
                "# END TeeForge-CD managed targets\n# BEGIN TeeForge-CD managed targets\n",
                &packages(&["com.a"])
            )
            .is_err()
        );
        assert!(
            render_target_file(
                "# BEGIN TeeForge-CD managed targets\n# END TeeForge-CD managed targets\n# BEGIN TeeForge-CD managed targets\n# END TeeForge-CD managed targets\n",
                &packages(&["com.a"])
            )
            .is_err()
        );
    }

    #[test]
    fn creates_and_updates_teeforge_profile_without_touching_others() {
        let input = r#"{
  "version": 1,
  "unknown": {"keep": true},
  "profiles": {
    "default": {"keybox": "keybox.xml", "mode": "patch", "apps": ["com.default"]},
    "other": {"keybox": "other.xml", "apps": ["com.claimed", "uid:10124"]}
  }
}"#;
        let rendered = render_teesim_config(
            input,
            &[
                PackageRecord {
                    name: "com.claimed".into(),
                    uids: vec![10124],
                },
                PackageRecord {
                    name: "com.new".into(),
                    uids: vec![10125],
                },
            ],
        )
        .unwrap();
        let value: Value = serde_json::from_str(&rendered).unwrap();
        let profiles = value["profiles"].as_object().unwrap();
        assert_eq!(profiles["other"]["apps"][0], "com.claimed");
        assert_eq!(profiles["teeforge"]["apps"], serde_json::json!(["com.new"]));
        assert_eq!(profiles["teeforge"]["mode"], "patch");
        assert_eq!(value["unknown"]["keep"], true);
        assert_eq!(profiles["teeforge"][TEESIM_MARKER], true);
    }

    #[test]
    fn rejects_unmarked_teeforge_profile_and_invalid_apps() {
        let unmarked = r#"{"version":1,"profiles":{"default":{"apps":[]},"teeforge":{"apps":[]}}}"#;
        assert!(render_teesim_config(unmarked, &packages(&["com.a"])).is_err());
        let invalid = r#"{"version":1,"profiles":{"default":{"apps":"bad"}}}"#;
        assert!(render_teesim_config(invalid, &packages(&["com.a"])).is_err());
        let invalid_managed = r#"{"version":1,"profiles":{"default":{"apps":[]},"teeforge":{"_teeforgeManaged":true,"apps":"bad"}}}"#;
        assert!(render_teesim_config(invalid_managed, &packages(&["com.a"])).is_err());
    }

    #[test]
    fn claims_only_primary_user_packages_and_matching_uids() {
        let input = r#"{
  "version": 1,
  "profiles": {
    "default": {"apps": ["com.primary@0", "com.work@10", "uid:10101"]}
  }
}"#;
        let rendered = render_teesim_config(
            input,
            &[
                PackageRecord {
                    name: "com.primary".into(),
                    uids: vec![10001],
                },
                PackageRecord {
                    name: "com.work".into(),
                    uids: vec![10002],
                },
                PackageRecord {
                    name: "com.uid".into(),
                    uids: vec![10101, 10103],
                },
                PackageRecord {
                    name: "com.free".into(),
                    uids: vec![10102],
                },
            ],
        )
        .unwrap();
        let value: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(
            value["profiles"]["teeforge"]["apps"],
            serde_json::json!(["com.work", "com.free"])
        );
    }

    #[test]
    fn no_compatible_backend_is_a_successful_noop() {
        let (config, root) = test_config("no-backend");
        let outcome = generate_with(&config, || panic!("package command must not run"))
            .expect("no backend is a successful skip");
        assert_eq!(outcome, GenerateOutcome::SkippedNoBackend);
        assert_eq!(
            outcome.message(),
            "未发现兼容目标配置，已跳过目标更新 [No compatible target backend found; target update skipped]"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn target_update_reports_updated_then_unchanged() {
        let (config, root) = test_config("updated-unchanged");
        fs::create_dir_all(config.target_txt.parent().unwrap()).expect("create target directory");
        fs::write(&config.target_txt, b"com.user?\n").expect("write user target");
        let listing = "package:/data/app/example/base.apk=com.example.app uid:10123\npackage:/data/app/user/base.apk=com.user uid:10124\n";

        let updated = generate_with(&config, || Ok(listing.to_owned())).expect("update target");
        assert_eq!(
            updated,
            GenerateOutcome::Updated {
                package_count: 2,
                backend_count: 1
            }
        );
        assert!(updated.message().contains("目标配置更新完成"));
        let rendered = fs::read_to_string(&config.target_txt).expect("read updated target");
        assert!(rendered.contains("com.example.app"));
        assert!(rendered.contains("com.user?"));
        assert!(!rendered.contains("\ncom.user\n"));
        let backup = config.target_txt.with_extension("txt.bak");
        let backup_before = fs::read(&backup).expect("first update creates backup");

        let unchanged = generate_with(&config, || Ok(listing.to_owned())).expect("repeat target");
        assert_eq!(
            unchanged,
            GenerateOutcome::Unchanged {
                package_count: 2,
                backend_count: 1
            }
        );
        assert!(unchanged.message().contains("目标配置已是最新，无需写入"));
        assert_eq!(fs::read(&backup).expect("backup remains"), backup_before);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn target_update_removes_uninstalled_managed_package() {
        let (config, root) = test_config("remove-uninstalled");
        fs::create_dir_all(config.target_txt.parent().unwrap()).expect("create target directory");
        fs::write(
            &config.target_txt,
            b"# BEGIN TeeForge-CD managed targets\ncom.old\n# END TeeForge-CD managed targets\n",
        )
        .expect("write managed target");
        let listing = "package:/data/app/new/base.apk=com.new uid:10123\n";

        let outcome = generate_with(&config, || Ok(listing.to_owned())).expect("replace package");
        assert!(matches!(outcome, GenerateOutcome::Updated { .. }));
        let rendered = fs::read_to_string(&config.target_txt).expect("read target");
        assert!(rendered.contains("com.new"));
        assert!(!rendered.contains("com.old"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn both_backends_report_updated_then_unchanged() {
        let (config, root) = test_config("both-backends");
        fs::create_dir_all(config.target_txt.parent().unwrap()).expect("create target directory");
        fs::create_dir_all(config.teesim_config.parent().unwrap())
            .expect("create teesim directory");
        fs::write(
            &config.teesim_config,
            br#"{"version":1,"profiles":{"default":{"apps":[]}}}"#,
        )
        .expect("write teesim config");
        let listing = "package:/data/app/example/base.apk=com.example.app uid:10123\n";

        let updated = generate_with(&config, || Ok(listing.to_owned())).expect("update both");
        assert_eq!(
            updated,
            GenerateOutcome::Updated {
                package_count: 1,
                backend_count: 2
            }
        );
        let unchanged = generate_with(&config, || Ok(listing.to_owned())).expect("repeat both");
        assert_eq!(
            unchanged,
            GenerateOutcome::Unchanged {
                package_count: 1,
                backend_count: 2
            }
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn empty_listing_is_valid_and_command_error_is_preserved() {
        let (config, root) = test_config("empty-and-error");
        fs::create_dir_all(config.target_txt.parent().unwrap()).expect("create target directory");
        let empty = generate_with(&config, || Ok(String::new())).expect("empty listing is valid");
        assert_eq!(
            empty,
            GenerateOutcome::Updated {
                package_count: 0,
                backend_count: 1
            }
        );

        let error = generate_with(&config, || Err(TfError::new("mock command failure")))
            .expect_err("command error must propagate");
        assert_eq!(error.to_string(), "mock command failure");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_command_failure_preserves_target_and_backup() {
        let (config, root) = test_config("command-failure-preserves-files");
        fs::create_dir_all(config.target_txt.parent().unwrap()).expect("create target directory");
        fs::write(&config.target_txt, b"com.existing?\n").expect("write target");
        let backup = config.target_txt.with_extension("txt.bak");
        fs::write(&backup, b"previous backup\n").expect("write backup");

        let error = generate_with(&config, || Err(TfError::new("mock timeout")))
            .expect_err("command failure must propagate");
        assert_eq!(error.to_string(), "mock timeout");
        assert_eq!(
            fs::read(&config.target_txt).expect("read target"),
            b"com.existing?\n"
        );
        assert_eq!(
            fs::read(&backup).expect("read backup"),
            b"previous backup\n"
        );
        let _ = fs::remove_dir_all(root);
    }
}
