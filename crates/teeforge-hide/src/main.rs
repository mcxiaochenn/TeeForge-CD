// TeeForge-CD 弱隐 BL 独立二进制。
// Standalone weak bootloader hiding for TeeForge-CD.
//
// 设计要点 [Design notes]
// - 只做 resetprop，不依赖主程序的 Keybox/RKP/WebUI 依赖，开机早期可独立执行。
// - 先读后写：值已一致就不写，减少 init 监听与 SELinux 审计噪音。
// - 只改已存在的属性，不凭空创建，避免在非对应品牌设备上留下新的属性指纹。
// - 删除前先探测；属性不存在属于正常结果，不是失败。
// - 单条失败降级为 WARN，只有 resetprop 完全不可用才返回非零。

mod config;
mod log;
mod props;
mod resetprop;
mod vbmeta;

use crate::log::{Level, log};
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_CONFIG: &str = "/data/adb/teeforge/config.conf";
const BOOT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_FAILURE_LINES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Early,
    Late,
}

impl Stage {
    fn label(self) -> &'static str {
        match self {
            Stage::Early => "early",
            Stage::Late => "late",
        }
    }
}

struct Args {
    config: PathBuf,
    stage: Stage,
    verbose: bool,
    strict: bool,
    dry_run: bool,
}

fn main() {
    process::exit(run());
}

fn run() -> i32 {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("使用 --help 查看用法 [Use --help for usage]");
            return 2;
        }
    };

    let config = config::HideConfig::load(&args.config);
    if !config.enabled {
        log(Level::Info, "弱隐 BL 已关闭 [Weak BL hiding disabled]");
        return 0;
    }

    if args.stage == Stage::Late {
        wait_for_boot();
    }

    let tool = resetprop::find_tool();
    let dump = match resetprop::dump(&tool) {
        Ok(dump) => dump,
        Err(error) => {
            log(
                Level::Error,
                format!("无法读取系统属性 [cannot read system properties]: {error}"),
            );
            return 1;
        }
    };

    let plan = props::plan(&config, &dump, vbmeta::size());

    if args.dry_run {
        for assignment in &plan.assignments {
            println!("{}={}", assignment.name, assignment.value);
        }
        for name in &plan.deletes {
            println!("--delete {name}");
        }
        return 0;
    }

    let outcome = plan.execute(&tool, args.verbose);
    let summary = format!(
        "stage={} applied={} skipped={} missing={} failed={} removed={} absent={}",
        args.stage.label(),
        outcome.applied,
        outcome.skipped,
        outcome.missing,
        outcome.failed,
        outcome.removed,
        outcome.absent
    );

    if outcome.failed == 0 {
        log(
            Level::Info,
            format!("弱隐 BL 完成 [Weak BL hiding done] {summary}"),
        );
        return 0;
    }

    // 单条属性失败是常见且可恢复的（只读属性被拒、厂商节点缺失），不升级为整条命令失败。
    // A single rejected property is common and recoverable; it must not fail the whole run.
    log(
        Level::Warn,
        format!("弱隐 BL 部分完成 [Weak BL hiding partially applied] {summary}"),
    );
    for failure in outcome.failures.iter().take(MAX_FAILURE_LINES) {
        log(Level::Warn, failure);
    }
    if outcome.failures.len() > MAX_FAILURE_LINES {
        log(
            Level::Warn,
            format!(
                "其余 {} 条失败已省略 [{} more failures omitted]",
                outcome.failures.len() - MAX_FAILURE_LINES,
                outcome.failures.len() - MAX_FAILURE_LINES
            ),
        );
    }
    if args.strict { 1 } else { 0 }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        config: PathBuf::from(DEFAULT_CONFIG),
        stage: Stage::Early,
        verbose: false,
        strict: false,
        dry_run: false,
    };
    let args = args.collect::<Vec<_>>();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--config 需要参数 [--config requires a value]".to_owned())?;
                parsed.config = PathBuf::from(value);
            }
            "--stage" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--stage 需要参数 [--stage requires a value]".to_owned())?;
                parsed.stage = match value.as_str() {
                    "early" => Stage::Early,
                    "late" => Stage::Late,
                    other => {
                        return Err(format!("未知阶段 [Unknown stage]: {other}（early|late）"));
                    }
                };
            }
            "--verbose" | "-v" => parsed.verbose = true,
            "--strict" => parsed.strict = true,
            "--dry-run" => parsed.dry_run = true,
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            other => return Err(format!("未知参数 [Unknown argument]: {other}")),
        }
        index += 1;
    }
    Ok(parsed)
}

fn usage() {
    println!("用法 Usage: teeforge-hide [options]");
    println!("\n选项 Options:");
    println!(
        "  --config PATH       配置文件 [Configuration file] (默认 default: {DEFAULT_CONFIG})"
    );
    println!(
        "  --stage early|late  执行阶段 [Stage]（early 立即执行，late 先等 sys.boot_completed）"
    );
    println!("  --dry-run           只打印计划 [Print the plan only]");
    println!("  --verbose           逐条输出 [Verbose per-property output]");
    println!("  --strict            有属性失败时返回 1 [Exit 1 on any property failure]");
    println!("  --help              显示帮助 [Show this help]");
}

fn wait_for_boot() {
    let started = Instant::now();
    while started.elapsed() < BOOT_TIMEOUT {
        if boot_completed() {
            return;
        }
        thread::sleep(Duration::from_secs(1));
    }
    log(
        Level::Warn,
        format!(
            "等待 sys.boot_completed 超时，继续执行 [boot wait timed out, continuing]: {}s",
            BOOT_TIMEOUT.as_secs()
        ),
    );
}

fn boot_completed() -> bool {
    std::process::Command::new("getprop")
        .arg("sys.boot_completed")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(values: &[&str]) -> Result<Args, String> {
        parse_args(values.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn defaults_are_early_and_non_strict() {
        let args = parse(&[]).expect("defaults");
        assert_eq!(args.stage, Stage::Early);
        assert!(!args.strict);
        assert!(!args.dry_run);
        assert_eq!(args.config, PathBuf::from(DEFAULT_CONFIG));
    }

    #[test]
    fn parses_stage_and_flags() {
        let args =
            parse(&["--stage", "late", "--verbose", "--strict", "--dry-run"]).expect("valid");
        assert_eq!(args.stage, Stage::Late);
        assert!(args.verbose && args.strict && args.dry_run);
    }

    #[test]
    fn rejects_unknown_stage_and_missing_values() {
        assert!(parse(&["--stage", "boot"]).is_err());
        assert!(parse(&["--config"]).is_err());
        assert!(parse(&["--nope"]).is_err());
    }
}
