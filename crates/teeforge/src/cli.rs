use crate::blhide;
use crate::config::{Config, SYS_CONFIG, USER_CONFIG};
use crate::description;
use crate::error::{Result, TfError};
use crate::keybox;
use crate::logging::{self, Level};
use crate::rootdetect;
use crate::target;
use crate::volume;
use std::path::PathBuf;

#[derive(Default)]
struct Options {
    generate: bool,
    hide_bl: bool,
    keybox: bool,
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
        match args[index].as_str() {
            "--generate" => options.generate = true,
            "--hide-bl" => options.hide_bl = true,
            "--keybox" => options.keybox = true,
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
                let path = args.get(index).ok_or_else(|| {
                    TfError::new("--config 需要文件路径 [--config requires a file path]")
                })?;
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
    println!("  --generate      生成 target.txt [Generate target.txt]");
    println!("  --hide-bl       弱隐 bootloader [Weak bootloader hiding]");
    println!("  --keybox        获取并更新 keybox [Fetch and update keybox]");
    println!("  --rootdetect    检测 root 方式并输出 [Detect root method]");
    println!("  --update-desc   更新模块描述 [Update module description]");
    println!("  --no-rootdetect 跳过 root 检测 [Skip root detection]");
    println!("  --volume SEC    音量键监听 [Volume key listen]");
    println!("  --verbose       启用调试日志 [Enable debug logging]");
    println!("  --config FILE   使用自定义配置 [Use custom config]");
    println!("  --help          显示帮助 [Show help]");
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

    let no_action = !options.generate
        && !options.hide_bl
        && !options.keybox
        && !options.rootdetect
        && !options.update_desc
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
    if options.generate {
        record(target::generate(&config), &mut failed);
    }
    if options.hide_bl {
        record(blhide::hide(&config), &mut failed);
    }
    if options.keybox {
        match keybox::fetch(&config) {
            Ok(()) => record(description::update(&config), &mut failed),
            Err(error) => record(Err(error), &mut failed),
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
    fn rejects_unknown_option() {
        assert!(parse(&["teeforge".into(), "--unknown".into()]).is_err());
    }
}
