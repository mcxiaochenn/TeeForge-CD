use crate::blhide;
use crate::config::{Config, KeyboxSource, SYS_CONFIG, TargetMode, USER_CONFIG, UpdateChannel};
use crate::description;
use crate::error::{Result, TfError};
use crate::info;
use crate::json;
use crate::keybox;
use crate::logging::{self, Level};
use crate::rkp;
use crate::rootdetect;
use crate::target;
use crate::update;
use crate::volume;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Options {
    generate: bool,
    hide_bl: bool,
    keybox: bool,
    source: Option<String>,
    rkp_status: bool,
    rkp_issue: bool,
    rkp_artifacts: bool,
    rkp_delete: bool,
    target_list: bool,
    target_apply: Option<PathBuf>,
    target_mode: Option<String>,
    self_update: Option<String>,
    update_channel: Option<String>,
    bootstrap: bool,
    json: bool,
    rootdetect: bool,
    update_desc: bool,
    skip_rootdetect: bool,
    volume: Option<u64>,
    verbose: bool,
    help: bool,
    config: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Options> {
    let mut options = Options::default();
    let mut index = 1;
    while index < args.len() {
        let need_value = |index: usize, name: &str| -> Result<&String> {
            args.get(index + 1)
                .ok_or_else(|| TfError::new(format!("{name} 需要参数 [{name} requires a value]")))
        };
        match args[index].as_str() {
            "--generate" => options.generate = true,
            "--hide-bl" => options.hide_bl = true,
            "--keybox" => options.keybox = true,
            "--source" => {
                index += 1;
                options.source = Some(need_value(index - 1, "--source")?.clone());
            }
            "--rkp-status" => options.rkp_status = true,
            "--rkp-issue" => options.rkp_issue = true,
            "--rkp-artifacts" => options.rkp_artifacts = true,
            "--rkp-delete" => options.rkp_delete = true,
            "--target-list" => options.target_list = true,
            "--target-apply" => {
                index += 1;
                options.target_apply =
                    Some(PathBuf::from(need_value(index - 1, "--target-apply")?));
            }
            "--target-mode" => {
                index += 1;
                options.target_mode = Some(need_value(index - 1, "--target-mode")?.clone());
            }
            "--self-update" => {
                index += 1;
                options.self_update = Some(need_value(index - 1, "--self-update")?.clone());
            }
            "--update-channel" => {
                index += 1;
                options.update_channel = Some(need_value(index - 1, "--update-channel")?.clone());
            }
            "--bootstrap" => options.bootstrap = true,
            "--json" => options.json = true,
            "--rootdetect" => options.rootdetect = true,
            "--update-desc" => options.update_desc = true,
            "--no-rootdetect" => options.skip_rootdetect = true,
            "--verbose" => options.verbose = true,
            "--help" | "-h" => options.help = true,
            "--volume" => {
                let mut timeout = 10;
                if args
                    .get(index + 1)
                    .is_some_and(|value| !value.starts_with('-'))
                {
                    index += 1;
                    timeout = args[index].parse::<u64>().map_err(|_| {
                        TfError::new(
                            "--volume 需要非负秒数 [--volume requires non-negative seconds]",
                        )
                    })?;
                }
                options.volume = Some(timeout);
            }
            "--config" => {
                index += 1;
                let path = need_value(index - 1, "--config")?;
                options.config = Some(path.into());
            }
            unknown => {
                return Err(TfError::new(format!(
                    "未知选项 [Unknown option]: {unknown}"
                )));
            }
        }
        index += 1;
    }
    Ok(options)
}

fn usage(program: &str) {
    println!("用法 Usage: {program} [options]");
    println!("\n选项 Options:");
    println!("  --generate          更新目标应用配置 [Update target app configuration]");
    println!("  --target-list       列出已安装应用与选择状态 [List apps and selection]");
    println!("  --target-apply FILE 应用手动选择集合 [Apply a selection file]");
    println!("  --target-mode MODE  切换 auto/manual 目标模式 [Switch target mode]");
    println!("  --keybox            同步 keybox [Sync keybox]");
    println!("  --source SOURCE     keybox 来源 remote/rkp [Keybox source]");
    println!("  --rkp-status        检查设备端 RKP 条件 [Check device RKP prerequisites]");
    println!("  --rkp-issue         设备端 RKP 签发并保存产物 [Issue RKP artifacts]");
    println!("  --rkp-artifacts     列出 RKP 产物 [List RKP artifacts]");
    println!("  --rkp-delete        删除 RKP 产物 [Delete RKP artifacts]");
    println!("  --self-update MODE  check/install 更新模块 [Check or install updates]");
    println!("  --update-channel C  stable/dev 更新通道 [Update channel]");
    println!("  --bootstrap         输出首屏信息 [Emit bootstrap information]");
    println!("  --json              以 JSON 输出 [Emit JSON]");
    println!("  --hide-bl           弱隐 bootloader [Weak bootloader hiding]");
    println!("  --rootdetect        检测 root 方式并输出 [Detect root method]");
    println!("  --update-desc       更新模块描述 [Update module description]");
    println!("  --no-rootdetect     跳过 root 检测 [Skip root detection]");
    println!("  --volume SEC        音量键监听 [Volume key listen]");
    println!("  --verbose           启用调试日志 [Enable debug logging]");
    println!("  --config FILE       使用自定义配置 [Use custom config]");
    println!("  --help              显示帮助 [Show help]");
}

fn banner() {
    println!("TeeForge-CD {}", crate::VERSION);
}

fn record(result: Result<()>, failed: &mut bool) {
    if let Err(error) = result {
        logging::log(Level::Error, error.to_string());
        *failed = true;
    }
}

pub fn run(args: Vec<String>) -> i32 {
    let program = args.first().map_or("teeforge", String::as_str);
    // 内部入口：TA 子进程只做一次厂商调用，不初始化配置、日志或 root 检测。
    if let Some(index) = args.iter().position(|arg| arg == "--rkp-ta-worker") {
        let Some(directory) = args.get(index + 1) else {
            eprintln!("--rkp-ta-worker 需要目录路径 [requires a directory]");
            return 2;
        };
        return match rkp::execute_worker(std::path::Path::new(directory)) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("{error}");
                1
            }
        };
    }
    let options = match parse(&args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            usage(program);
            return 1;
        }
    };
    if options.help {
        usage(program);
        return 0;
    }

    let user_config = options
        .config
        .clone()
        .unwrap_or_else(|| PathBuf::from(USER_CONFIG));
    let config = Config::load(&user_config);
    logging::init(&config, options.verbose);

    if options.json {
        return run_json(&options, &config, &user_config);
    }

    if options.rkp_status {
        for line in rkp::status_lines(&config) {
            println!("{line}");
        }
        return if rkp::available() { 0 } else { 1 };
    }

    let no_action = !options.generate
        && !options.hide_bl
        && !options.keybox
        && !options.rootdetect
        && !options.update_desc
        && !options.rkp_issue
        && !options.rkp_artifacts
        && !options.rkp_delete
        && !options.target_list
        && options.target_apply.is_none()
        && options.target_mode.is_none()
        && options.self_update.is_none()
        && options.update_channel.is_none()
        && !options.bootstrap
        && options.volume.is_none();
    if args.len() == 1 && no_action {
        banner();
        let root = rootdetect::detect();
        println!("Root: {} (v{})\n", root.method, root.version);
        let _ = rootdetect::save(std::path::Path::new(SYS_CONFIG), &root);
        usage(program);
        return 0;
    }

    let mut failed = false;
    if !options.skip_rootdetect && !options.rootdetect {
        let root = rootdetect::detect();
        logging::log(
            Level::Info,
            format!("Root: {} (v{})", root.method, root.version),
        );
        record(
            rootdetect::save(std::path::Path::new(SYS_CONFIG), &root),
            &mut failed,
        );
    }
    if options.bootstrap {
        for line in info::bootstrap_lines(&config) {
            println!("{line}");
        }
    }
    if options.generate {
        match target::generate(&config) {
            Ok(outcome) => outcome.log(),
            Err(error) => record(Err(error), &mut failed),
        }
    }
    if options.hide_bl {
        record(blhide::hide(&config), &mut failed);
    }
    if options.keybox {
        let mut effective = config.clone();
        if let Some(source) = options.source.as_deref() {
            match KeyboxSource::parse(source) {
                Some(parsed) => effective.keybox_source = parsed,
                None => {
                    record(
                        Err(TfError::new(format!("未知来源 [unknown source]: {source}"))),
                        &mut failed,
                    );
                    return 1;
                }
            }
        }
        match keybox::fetch(&effective) {
            Ok(report) => {
                println!(
                    "来源 Source: {}，目标 {} 个 [targets]",
                    report.source.label(),
                    report.targets.len()
                );
                record(description::update(&effective), &mut failed);
            }
            Err(error) => record(Err(error), &mut failed),
        }
    }
    if options.rkp_issue {
        match rkp::issue_and_save(&config) {
            Ok(saved) => {
                println!("已签发 [issued]: {}", saved.keybox.display());
                println!("证书链 [chain]: {}", saved.chain.display());
                record(description::update(&config), &mut failed);
            }
            Err(error) => record(Err(error), &mut failed),
        }
    }
    if options.rkp_artifacts {
        let value = rkp::artifacts_value(&config);
        println!("{value:#}");
    }
    if options.rkp_delete {
        match rkp::delete_artifacts(&config) {
            Ok(value) => println!("{value:#}"),
            Err(error) => record(Err(error), &mut failed),
        }
    }
    if options.target_list {
        match target::list_value(&config) {
            Ok(value) => {
                let count = value["counts"]["installed"].as_u64().unwrap_or(0);
                let selected = value["counts"]["selected"].as_u64().unwrap_or(0);
                println!("已安装 {count} 个应用，当前选中 {selected} 个 [installed/selected]");
                println!("模式 Mode: {}", value["mode"].as_str().unwrap_or("-"));
            }
            Err(error) => record(Err(error), &mut failed),
        }
    }
    if let Some(path) = options.target_apply.clone() {
        match read_selection(&path)
            .and_then(|names| target::apply_selection(&config, &user_config, names).map(|_| ()))
        {
            Ok(()) => println!("目标作用域已更新 [target scope updated]"),
            Err(error) => record(Err(error), &mut failed),
        }
    }
    if let Some(mode) = options.target_mode.as_deref() {
        let parsed = TargetMode::parse(mode);
        match parsed {
            Some(parsed) => record(
                target::switch_mode(&config, &user_config, parsed).map(|_| ()),
                &mut failed,
            ),
            None => record(
                Err(TfError::new(format!("未知模式 [unknown mode]: {mode}"))),
                &mut failed,
            ),
        }
    }
    if let Some(channel) = options.update_channel.as_deref() {
        match UpdateChannel::parse(channel) {
            Some(parsed) => record(
                crate::config::update_key(&user_config, "update_channel", parsed.label()),
                &mut failed,
            ),
            None => record(
                Err(TfError::new(format!(
                    "未知通道 [unknown channel]: {channel}"
                ))),
                &mut failed,
            ),
        }
    }
    if let Some(action) = options.self_update.as_deref() {
        match action {
            "check" => match update::check_value(&config) {
                Ok(value) => {
                    println!(
                        "当前 [current]: {} ({})  最新 [latest]: {} ({})",
                        value["current"]["version"].as_str().unwrap_or("-"),
                        value["current"]["version_code"],
                        value["latest"]["version"].as_str().unwrap_or("-"),
                        value["latest"]["version_code"]
                    );
                    println!(
                        "{}",
                        if value["available"] == json!(true) {
                            "有可用更新 [update available]"
                        } else {
                            "已是最新 [up to date]"
                        }
                    );
                }
                Err(error) => record(Err(error), &mut failed),
            },
            "install" => match update::install_value(&config) {
                Ok(value) => {
                    if value["installed"] == json!(true) {
                        println!(
                            "已安装 [installed]: {}，重启后生效 [reboot to apply]",
                            value["version"].as_str().unwrap_or("-")
                        );
                    } else {
                        println!(
                            "未安装 [not installed]: {}",
                            value["reason"].as_str().unwrap_or("-")
                        );
                        if let Some(path) = value["saved_to"].as_str() {
                            println!("已保存 [saved]: {path}");
                        }
                    }
                }
                Err(error) => record(Err(error), &mut failed),
            },
            other => record(
                Err(TfError::new(format!(
                    "--self-update 只支持 check/install，收到 [got]: {other}"
                ))),
                &mut failed,
            ),
        }
    }
    if options.rootdetect {
        let root = rootdetect::detect();
        println!("{}\n{}", root.method, root.version);
        record(
            rootdetect::save(std::path::Path::new(SYS_CONFIG), &root),
            &mut failed,
        );
    }
    if let Some(timeout) = options.volume {
        match volume::listen(timeout) {
            Ok(value) => {
                println!("{value}");
                if value < 0 {
                    failed = true;
                }
            }
            Err(error) => record(Err(error), &mut failed),
        }
    }
    if options.update_desc {
        record(description::update(&config), &mut failed);
    }
    if failed { 1 } else { 0 }
}

/// JSON 模式：只允许一个命令，保证输出是一个完整的信封。
fn run_json(options: &Options, config: &Config, user_config: &Path) -> i32 {
    let mut commands = Vec::new();
    if options.bootstrap {
        commands.push("bootstrap");
    }
    if options.generate {
        commands.push("generate");
    }
    if options.keybox {
        commands.push("keybox");
    }
    if options.rkp_status {
        commands.push("rkp-status");
    }
    if options.rkp_issue {
        commands.push("rkp-issue");
    }
    if options.rkp_artifacts {
        commands.push("rkp-artifacts");
    }
    if options.rkp_delete {
        commands.push("rkp-delete");
    }
    if options.target_list {
        commands.push("target-list");
    }
    if options.target_apply.is_some() {
        commands.push("target-apply");
    }
    if options.target_mode.is_some() {
        commands.push("target-mode");
    }
    if options.self_update.is_some() {
        commands.push("self-update");
    }
    if options.update_channel.is_some() {
        commands.push("update-channel");
    }
    if commands.len() != 1 {
        return json::fail(
            &TfError::staged(
                "cli",
                "command_count",
                "--json 需要且只能搭配一个命令 [--json expects exactly one command]",
            )
            .with_raw(json!({ "commands": commands })),
        );
    }

    if options.bootstrap {
        json::ok(info::bootstrap_value(config));
        return 0;
    }
    if options.generate {
        let value = target::generate(config).map(|outcome| match outcome {
            target::GenerateOutcome::SkippedNoBackend => json!({
                "state": "skipped",
                "reason": "no_backend",
            }),
            target::GenerateOutcome::Unchanged {
                package_count,
                backend_count,
            } => json!({
                "state": "unchanged",
                "packages": package_count,
                "backends": backend_count,
            }),
            target::GenerateOutcome::Updated {
                package_count,
                backend_count,
            } => json!({
                "state": "updated",
                "packages": package_count,
                "backends": backend_count,
            }),
        });
        return json::finish(value);
    }
    if options.keybox {
        let mut effective = config.clone();
        if let Some(source) = options.source.as_deref() {
            match KeyboxSource::parse(source) {
                Some(parsed) => effective.keybox_source = parsed,
                None => {
                    return json::fail(&TfError::staged(
                        "cli",
                        "source",
                        format!("未知来源 [unknown source]: {source}"),
                    ));
                }
            }
        }
        let value = keybox::fetch(&effective).map(|report| {
            json!({
                "source": report.source.label(),
                "targets": report.targets.iter().map(|(path, changed)| json!({
                    "path": path.display().to_string(),
                    "changed": changed,
                })).collect::<Vec<_>>(),
            })
        });
        let code = json::finish(value);
        let _ = description::update(&effective);
        return code;
    }
    if options.rkp_status {
        json::ok(rkp::status_value(config));
        return 0;
    }
    if options.rkp_issue {
        let value = rkp::issue_and_save(config).map(|saved| {
            let meta =
                serde_json::from_str::<Value>(&fs::read_to_string(&saved.meta).unwrap_or_default())
                    .unwrap_or(Value::Null);
            json!({
                "keybox": saved.keybox.display().to_string(),
                "chain": saved.chain.display().to_string(),
                "meta_file": saved.meta.display().to_string(),
                "meta": meta,
            })
        });
        let code = json::finish(value);
        let _ = description::update(config);
        return code;
    }
    if options.rkp_artifacts {
        json::ok(rkp::artifacts_value(config));
        return 0;
    }
    if options.rkp_delete {
        return json::finish(rkp::delete_artifacts(config));
    }
    if options.target_list {
        return json::finish(target::list_value(config));
    }
    if let Some(path) = options.target_apply.as_deref() {
        let value = read_selection(path)
            .and_then(|names| target::apply_selection(config, user_config, names));
        return json::finish(value);
    }
    if let Some(mode) = options.target_mode.as_deref() {
        let Some(parsed) = TargetMode::parse(mode) else {
            return json::fail(&TfError::staged(
                "cli",
                "target_mode",
                format!("未知模式 [unknown mode]: {mode}"),
            ));
        };
        return json::finish(target::switch_mode(config, user_config, parsed));
    }
    if let Some(channel) = options.update_channel.as_deref() {
        let Some(parsed) = UpdateChannel::parse(channel) else {
            return json::fail(&TfError::staged(
                "cli",
                "update_channel",
                format!("未知通道 [unknown channel]: {channel}"),
            ));
        };
        return match crate::config::update_key(user_config, "update_channel", parsed.label()) {
            Ok(()) => {
                json::ok(json!({ "channel": parsed.label() }));
                0
            }
            Err(error) => json::fail(&error),
        };
    }
    if let Some(action) = options.self_update.as_deref() {
        return match action {
            "check" => json::finish(update::check_value(config)),
            "install" => json::finish(update::install_value(config)),
            other => json::fail(&TfError::staged(
                "cli",
                "self_update",
                format!("--self-update 只支持 check/install，收到 [got]: {other}"),
            )),
        };
    }
    json::fail(&TfError::new("未识别的命令 [no command]"))
}

/// 读取选择文件：接受 `["pkg"]` 或 `{"packages":["pkg"]}` 两种形态。
fn read_selection(path: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(path).map_err(|error| {
        TfError::staged(
            "target",
            "selection_unreadable",
            format!("无法读取选择文件 [cannot read selection file]: {error}"),
        )
        .with_raw(json!({ "path": path.display().to_string() }))
    })?;
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        TfError::staged(
            "target",
            "selection_invalid",
            format!("选择文件不是合法 JSON [selection is not valid JSON]: {error}"),
        )
    })?;
    let array = match &value {
        Value::Array(items) => items.clone(),
        other => other
            .get("packages")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                TfError::staged(
                    "target",
                    "selection_invalid",
                    "选择文件缺少 packages 数组 [missing packages array]",
                )
            })?,
    };
    Ok(array
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compatible_options() {
        let args = vec![
            "teeforge".into(),
            "--generate".into(),
            "--hide-bl".into(),
            "--volume".into(),
            "12".into(),
            "--config".into(),
            "/tmp/config".into(),
        ];
        let options = parse(&args).expect("valid options");
        assert!(options.generate);
        assert!(options.hide_bl);
        assert_eq!(options.volume, Some(12));
        assert_eq!(options.config, Some(PathBuf::from("/tmp/config")));
    }

    #[test]
    fn parses_structured_commands() {
        let args = vec![
            "teeforge".into(),
            "--json".into(),
            "--keybox".into(),
            "--source".into(),
            "rkp".into(),
        ];
        let options = parse(&args).expect("valid options");
        assert!(options.json);
        assert_eq!(options.source.as_deref(), Some("rkp"));

        let args = vec![
            "teeforge".into(),
            "--target-apply".into(),
            "/tmp/selection.json".into(),
        ];
        let options = parse(&args).expect("valid options");
        assert_eq!(
            options.target_apply,
            Some(PathBuf::from("/tmp/selection.json"))
        );
    }

    #[test]
    fn rejects_unknown_option() {
        assert!(parse(&["teeforge".into(), "--unknown".into()]).is_err());
    }
}
