use sha2::{Digest, Sha256};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

type DynError = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, DynError>;

const MAX_BINARY_SIZE: u64 = 1_572_864;
const MAX_HIDE_BINARY_SIZE: u64 = 524_288;
const MAX_PACKAGE_SIZE: u64 = 6_291_456;

/// 单个可执行产物的构建描述；体积上限按职责分别设置。
/// One buildable artifact; size limits stay per-artifact because duties differ.
struct Artifact {
    package: &'static str,
    binary: &'static str,
    max_size: u64,
}

const ARTIFACTS: &[Artifact] = &[
    Artifact {
        package: "teeforge",
        binary: "teeforge",
        max_size: MAX_BINARY_SIZE,
    },
    Artifact {
        package: "teeforge-hide",
        binary: "teeforge-hide",
        max_size: MAX_HIDE_BINARY_SIZE,
    },
];

struct Abi {
    android: &'static str,
    rust: &'static str,
    clang: &'static str,
    cargo_linker_env: &'static str,
    elf_machine: u16,
}

// Android ABI、Rust target 与 ELF machine 必须成组维护，防止产物被放入错误目录。
// Keep Android ABI, Rust target, and ELF machine mappings together to prevent mispackaging.
const ABIS: &[Abi] = &[
    Abi {
        android: "arm64-v8a",
        rust: "aarch64-linux-android",
        clang: "aarch64-linux-android24",
        cargo_linker_env: "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER",
        elf_machine: 183,
    },
    Abi {
        android: "armeabi-v7a",
        rust: "armv7-linux-androideabi",
        clang: "armv7a-linux-androideabi24",
        cargo_linker_env: "CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER",
        elf_machine: 40,
    },
    Abi {
        android: "x86",
        rust: "i686-linux-android",
        clang: "i686-linux-android24",
        cargo_linker_env: "CARGO_TARGET_I686_LINUX_ANDROID_LINKER",
        elf_machine: 3,
    },
    Abi {
        android: "x86_64",
        rust: "x86_64-linux-android",
        clang: "x86_64-linux-android24",
        cargo_linker_env: "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER",
        elf_machine: 62,
    },
];

fn main() {
    if let Err(error) = run() {
        eprintln!("错误 Error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let command = env::args().nth(1).unwrap_or_else(|| "help".into());
    match command.as_str() {
        "build" => build_android(),
        "package" => package(),
        "verify" => verify_outputs(),
        "help" | "--help" | "-h" => {
            println!("用法 Usage: cargo run -p xtask -- <build|package|verify>");
            Ok(())
        }
        _ => Err(format!("未知 xtask 命令 [Unknown xtask command]: {command}").into()),
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be inside workspace")
        .to_path_buf()
}

fn command_ok(command: &mut Command, label: &str) -> Result<()> {
    println!("==> {label}");
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{label} 失败，退出码 {} [failed, exit code {}]",
            status.code().unwrap_or(128),
            status.code().unwrap_or(128)
        )
        .into())
    }
}

fn executable(base: &Path, name: &str) -> PathBuf {
    if cfg!(windows) {
        base.join(format!("{name}.exe"))
    } else {
        base.join(name)
    }
}

fn ndk_bin() -> Result<PathBuf> {
    let ndk = env::var_os("NDK")
        .or_else(|| env::var_os("ANDROID_NDK_HOME"))
        .ok_or("请设置 NDK 或 ANDROID_NDK_HOME [Set NDK or ANDROID_NDK_HOME]")?;
    let host = if cfg!(windows) {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else {
        "linux-x86_64"
    };
    let bin = PathBuf::from(ndk)
        .join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .join(host)
        .join("bin");
    if !bin.is_dir() {
        return Err(format!(
            "NDK 工具目录不存在 [NDK tool directory missing]: {}",
            bin.display()
        )
        .into());
    }
    Ok(bin)
}

fn build_android() -> Result<()> {
    let workspace = root();
    let ndk = ndk_bin()?;
    let output_root = workspace.join("out").join("bin");
    let version = env::var("VERSION").unwrap_or(teeforge_version(&workspace)?);
    fs::create_dir_all(&output_root)?;

    for abi in ABIS {
        let clang = executable(&ndk, "clang");
        if !clang.is_file() {
            return Err(format!("NDK clang 不存在 [missing]: {}", clang.display()).into());
        }
        for artifact in ARTIFACTS {
            build_artifact(
                &workspace,
                &ndk,
                &clang,
                abi,
                artifact,
                &version,
                &output_root,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_artifact(
    workspace: &Path,
    ndk: &Path,
    clang: &Path,
    abi: &Abi,
    artifact: &Artifact,
    version: &str,
    output_root: &Path,
) -> Result<()> {
    let rustflags = format!("-Clink-arg=--target={}", abi.clang);
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(workspace)
        .args([
            "build",
            "--release",
            "-p",
            artifact.package,
            "--target",
            abi.rust,
        ])
        .env(abi.cargo_linker_env, clang)
        .env("RUSTFLAGS", rustflags);
    // 只有主程序把版本号编进二进制；弱隐二进制不读这个变量。
    // Only the main binary embeds the version; the weak-hiding binary ignores it.
    if artifact.package == "teeforge" {
        cargo.env("TEEFORGE_VERSION", version);
    }
    command_ok(
        &mut cargo,
        &format!(
            "构建 {} {} [Build {} {}]",
            abi.android, artifact.package, abi.android, artifact.package
        ),
    )?;

    let source = workspace
        .join("target")
        .join(abi.rust)
        .join("release")
        .join(artifact.binary);
    let destination = output_root.join(abi.android).join(artifact.binary);
    fs::create_dir_all(destination.parent().expect("destination parent"))?;
    fs::copy(&source, &destination)?;
    if artifact.package == "teeforge" {
        fs::copy(
            &source,
            output_root
                .parent()
                .expect("out parent")
                .join(format!("teeforge-{}", abi.android)),
        )?;
    }
    let size = fs::metadata(&destination)?.len();
    if size > artifact.max_size {
        return Err(format!(
            "{} {} 体积 {} 超过上限 [binary exceeds size limit]: {}",
            abi.android, artifact.binary, size, artifact.max_size
        )
        .into());
    }
    verify_elf(&destination, abi.elf_machine, ndk)?;
    println!("    {} {}: {} bytes", abi.android, artifact.binary, size);
    Ok(())
}

fn teeforge_version(workspace: &Path) -> Result<String> {
    let manifest = fs::read_to_string(workspace.join("crates/teeforge/Cargo.toml"))?;
    manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = \"")?.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| "无法读取 teeforge 版本 [Unable to read teeforge version]".into())
}

fn verify_elf(path: &Path, expected_machine: u16, ndk_bin: &Path) -> Result<()> {
    let header = fs::read(path)?;
    if header.len() < 20 || &header[..4] != b"\x7fELF" {
        return Err(format!("不是 ELF 文件 [Not an ELF file]: {}", path.display()).into());
    }
    let machine = u16::from_le_bytes([header[18], header[19]]);
    if machine != expected_machine {
        return Err(format!(
            "ELF 架构不匹配 [ELF machine mismatch]: {} != {}",
            machine, expected_machine
        )
        .into());
    }
    let readelf = executable(ndk_bin, "llvm-readelf");
    let output = Command::new(readelf).args(["-d"]).arg(path).output()?;
    if !output.status.success() {
        return Err(format!("无法检查 ELF 依赖 [Cannot inspect ELF]: {}", path.display()).into());
    }
    let dynamic = String::from_utf8(output.stdout)?;
    for line in dynamic.lines().filter(|line| line.contains("NEEDED")) {
        if !line.contains("[libc.so]") && !line.contains("[libdl.so]") {
            return Err(format!("发现意外动态依赖 [Unexpected dynamic dependency]: {line}").into());
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn collect_shell_files(current: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_shell_files(&path, output)?;
        } else if path.extension() == Some(OsStr::new("sh")) {
            output.push(path);
        }
    }
    Ok(())
}

fn verify_shell_files_lf(files: impl IntoIterator<Item = PathBuf>) -> Result<()> {
    for path in files {
        if fs::read(&path)?.contains(&b'\r') {
            return Err(format!(
                "Shell 文件不是纯 LF [Shell file is not LF-only]: {}",
                path.display()
            )
            .into());
        }
    }
    Ok(())
}

fn verify_source_shell_files(workspace: &Path) -> Result<()> {
    // 模块脚本在打包时按字节复制；源码门禁可阻止 Windows 换行进入 Android sh。
    // Module scripts are copied byte-for-byte, so reject Windows line endings at the source.
    let mut files = ["build.sh", "clean.sh", "package.sh"]
        .into_iter()
        .map(|name| workspace.join(name))
        .collect::<Vec<_>>();
    collect_shell_files(&workspace.join("module"), &mut files)?;
    verify_shell_files_lf(files)
}

fn build_webui(workspace: &Path) -> Result<()> {
    // 仅用于无法执行 npm 的受限环境（例如本机 node_modules 损坏且无法重装）；
    // CI 和正式打包不设置该变量，仍然必须从锁定依赖完整构建。
    // Escape hatch for restricted hosts only; CI and release builds never set it.
    if env::var_os("TEEFORGE_SKIP_WEBUI").is_some() {
        println!("==> 跳过 WebUI 构建 [Skipping WebUI build] (TEEFORGE_SKIP_WEBUI)");
        return Ok(());
    }
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let webroot = workspace.join("webroot");
    let npm_cache = workspace.join("out").join("npm-cache");
    fs::create_dir_all(&npm_cache)?;
    command_ok(
        Command::new(npm)
            .current_dir(&webroot)
            .env("NPM_CONFIG_CACHE", &npm_cache)
            .args(["ci", "--ignore-scripts"]),
        "安装 WebUI 锁定依赖 [Install locked WebUI dependencies]",
    )?;
    command_ok(
        Command::new(npm)
            .current_dir(&webroot)
            .env("NPM_CONFIG_CACHE", &npm_cache)
            .env("ASTRO_TELEMETRY_DISABLED", "1")
            .args(["run", "build"]),
        "构建 WebUI [Build WebUI]",
    )
}

fn git_output(workspace: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(workspace)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!("git {} 失败 [failed]", args.join(" ")).into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}

fn replace_property(data: &mut String, key: &str, value: &str) {
    let prefix = format!("{key}=");
    let mut found = false;
    let mut lines = data
        .lines()
        .map(|line| {
            if line.starts_with(&prefix) {
                found = true;
                format!("{prefix}{value}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>();
    if !found {
        lines.push(format!("{prefix}{value}"));
    }
    *data = format!("{}\n", lines.join("\n"));
}

fn collect_files(root: &Path, current: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).expect("entry under root");
        if relative == Path::new(".sha256")
            || relative
                .components()
                .next()
                .is_some_and(|part| part.as_os_str() == "META-INF")
        {
            continue;
        }
        if entry.file_type()?.is_dir() {
            collect_files(root, &path, output)?;
        } else {
            output.push(relative.to_path_buf());
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn write_checksums(stage: &Path) -> Result<()> {
    let mut files = Vec::new();
    collect_files(stage, stage, &mut files)?;
    files.sort();
    let mut checksums = String::new();
    for relative in files {
        let hash = sha256_file(&stage.join(&relative))?;
        checksums.push_str(&format!(
            "{hash}  {}\n",
            relative.to_string_lossy().replace('\\', "/")
        ));
    }
    fs::write(stage.join(".sha256"), checksums)?;
    Ok(())
}

fn create_zip(stage: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    if cfg!(windows) {
        command_ok(
            Command::new("tar").args([
                OsStr::new("-a"),
                OsStr::new("-cf"),
                destination.as_os_str(),
                OsStr::new("-C"),
                stage.as_os_str(),
                OsStr::new("."),
            ]),
            "创建模块 ZIP [Create module ZIP]",
        )?;
    } else {
        command_ok(
            Command::new("zip")
                .current_dir(stage)
                .args(["-qr"])
                .arg(destination)
                .arg("."),
            "创建模块 ZIP [Create module ZIP]",
        )?;
    }
    let listing = if cfg!(windows) {
        Command::new("tar")
            .args(["-tf"])
            .arg(destination)
            .output()?
    } else {
        Command::new("unzip")
            .args(["-Z1"])
            .arg(destination)
            .output()?
    };
    if !listing.status.success() {
        return Err("无法读取 ZIP 清单 [Unable to read ZIP listing]".into());
    }
    let entries = String::from_utf8(listing.stdout)?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !entries.iter().any(|line| line.ends_with(".sha256")) {
        return Err("ZIP 缺少 .sha256 [ZIP is missing .sha256]".into());
    }
    verify_archive_shell_files_lf(destination, &entries)?;
    Ok(())
}

fn verify_archive_shell_files_lf(archive: &Path, entries: &[String]) -> Result<()> {
    for entry in entries.iter().filter(|entry| entry.ends_with(".sh")) {
        let output = if cfg!(windows) {
            Command::new("tar")
                .args([OsStr::new("-xOf"), archive.as_os_str(), OsStr::new(entry)])
                .output()?
        } else {
            Command::new("unzip")
                .args([OsStr::new("-p"), archive.as_os_str(), OsStr::new(entry)])
                .output()?
        };
        if !output.status.success() {
            return Err(
                format!("无法读取 ZIP 内脚本 [Unable to read script in ZIP]: {entry}").into(),
            );
        }
        if output.stdout.contains(&b'\r') {
            return Err(format!(
                "ZIP 内 Shell 文件不是纯 LF [Shell file in ZIP is not LF-only]: {entry}"
            )
            .into());
        }
    }
    Ok(())
}

fn package() -> Result<()> {
    let workspace = root();
    verify_source_shell_files(&workspace)?;
    build_android()?;
    build_webui(&workspace)?;
    let out = workspace.join("out");
    let stage = out.join("build").join("teeforge_cd");
    if stage.exists() {
        fs::remove_dir_all(&stage)?;
    }
    copy_tree(&workspace.join("module"), &stage)?;
    let mut staged_shell_files = Vec::new();
    collect_shell_files(&stage, &mut staged_shell_files)?;
    verify_shell_files_lf(staged_shell_files)?;
    let stale = stage.join("teeforge");
    if stale.exists() {
        fs::remove_file(stale)?;
    }
    // 二进制统一落在 runtime/<abi>/；安装时裁剪并扁平化为 runtime/ 下两个文件。
    // Binaries live under runtime/<abi>/ and are flattened into runtime/ at install time.
    let runtime = stage.join("runtime");
    if runtime.exists() {
        fs::remove_dir_all(&runtime)?;
    }
    copy_tree(&out.join("bin"), &runtime)?;
    fs::copy(workspace.join("README.md"), stage.join("README"))?;

    let version = env::var("VERSION").unwrap_or(teeforge_version(&workspace)?);
    let version_code = env::var("VERSION_CODE")
        .unwrap_or(git_output(&workspace, &["rev-list", "--count", "HEAD"])?);
    let channel = env::var("BUILD_CHANNEL").unwrap_or_else(|_| "release".into());
    let mut module_prop = fs::read_to_string(stage.join("module.prop"))?;
    replace_property(&mut module_prop, "version", &version);
    replace_property(&mut module_prop, "versionCode", &version_code);
    if channel == "dev" {
        replace_property(&mut module_prop, "name", "TeeForge-CD [Dev]");
        replace_property(
            &mut module_prop,
            "updateJson",
            "https://teeforge.mcxiaochen.top/files/dev/update/dev.json",
        );
    }
    fs::write(stage.join("module.prop"), module_prop)?;
    write_checksums(&stage)?;

    let module_id = "teeforge_cd";
    let zip = out.join(format!("{module_id}-{version}.zip"));
    create_zip(&stage, &zip)?;
    let size = fs::metadata(&zip)?.len();
    if size > MAX_PACKAGE_SIZE {
        return Err(format!("ZIP 体积 {size} 超过 6 MiB 上限 [package exceeds size limit]").into());
    }
    println!(
        "打包完成 [Package complete]: {} ({size} bytes)",
        zip.display()
    );
    Ok(())
}

fn verify_outputs() -> Result<()> {
    let workspace = root();
    verify_source_shell_files(&workspace)?;
    let output = workspace.join("out").join("bin");
    for abi in ABIS {
        for artifact in ARTIFACTS {
            let binary = output.join(abi.android).join(artifact.binary);
            let size = fs::metadata(&binary)?.len();
            if size == 0 || size > artifact.max_size {
                return Err(format!(
                    "{} {} 产物无效 [invalid output]",
                    abi.android, artifact.binary
                )
                .into());
            }
            println!("{} {}: {} bytes", abi.android, artifact.binary, size);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_freezes_omk_detection_and_prefers_pending_module() -> Result<()> {
        let directory = env::temp_dir().join(format!("teeforge-install-{}", std::process::id()));
        fs::create_dir_all(&directory)?;
        let source = fs::read_to_string(root().join("module/customize.sh"))?;
        let start = source
            .find("OMK_ENABLED=0")
            .ok_or("missing OMK detection")?;
        let end = source
            .find("# 生成用户配置")
            .ok_or("missing system config boundary")?;
        let snippet = &source[start..end];
        let shell = if cfg!(windows) {
            "C:/Program Files/Git/bin/bash.exe"
        } else {
            "sh"
        };
        // 同一沙箱连续模拟安装与升级，不接触设备路径。
        // Simulate install/upgrade in one sandbox without touching device paths.
        let active = directory.join("modules/oh_my_keymint");
        let pending = directory.join("modules_update/oh_my_keymint");
        fs::create_dir_all(directory.join("teeforge"))?;
        let run = |expected: bool| -> Result<()> {
            let sandbox = directory.to_string_lossy().replace('\\', "/");
            let script = format!(
                "set -eu\nui_print() {{ :; }}\nTEEFORGE_DIR='{sandbox}/teeforge'\nROOT_METHOD=Test\nROOT_VERSION=1\n{}",
                snippet
                    .replace(
                        "/data/adb/modules_update",
                        &format!("{sandbox}/modules_update")
                    )
                    .replace("/data/adb/modules/", &format!("{sandbox}/modules/"))
            );
            let output = Command::new(shell).args(["-c", &script]).output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let config = fs::read_to_string(directory.join("teeforge/sys.conf"))?;
            assert!(config.contains(&format!("omk_enabled={}\n", u8::from(expected))));
            assert!(config.contains("omk_injector_config=/data/misc/keystore/omk/injector.toml\n"));
            assert!(!config.contains("prop_tool="));
            Ok(())
        };
        run(false)?;
        fs::create_dir_all(directory.join("omk-data"))?;
        run(false)?;
        fs::create_dir_all(&active)?;
        fs::write(active.join("module.prop"), "id=wrong\n")?;
        run(false)?;
        fs::write(active.join("module.prop"), "id=oh_my_keymint\n")?;
        run(true)?;
        for marker in ["disable", "remove"] {
            fs::write(active.join(marker), "")?;
            run(false)?;
            fs::remove_file(active.join(marker))?;
        }
        fs::create_dir_all(&pending)?;
        fs::write(pending.join("module.prop"), "id=oh_my_keymint\n")?;
        fs::write(active.join("disable"), "")?;
        run(true)?;
        fs::write(pending.join("disable"), "")?;
        run(false)?;
        fs::remove_file(pending.join("disable"))?;
        fs::write(pending.join("remove"), "")?;
        run(false)?;
        fs::remove_file(pending.join("remove"))?;
        run(true)?;
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn installer_flattens_runtime_for_current_abi() -> Result<()> {
        let directory = env::temp_dir().join(format!("teeforge-runtime-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory)?;
        let source = fs::read_to_string(root().join("module/customize.sh"))?;
        let start = source
            .find("# 选择当前设备对应的二进制")
            .ok_or("missing ABI selection")?;
        let banner = "ui_print \"  架构 [ABI]: $ARCH\"";
        let end = source.find(banner).ok_or("missing ABI banner")? + banner.len();
        let snippet = &source[start..end];

        // 四个 ABI 目录都放最小 ELF 头，只有 arm64 的 machine 与模拟设备一致。
        // Every ABI directory gets a minimal ELF header; only arm64 matches the fake device.
        for (abi, machine) in [
            ("arm64-v8a", 183_u16),
            ("armeabi-v7a", 40),
            ("x86", 3),
            ("x86_64", 62),
        ] {
            let abi_dir = directory.join("runtime").join(abi);
            fs::create_dir_all(&abi_dir)?;
            for name in ["teeforge", "teeforge-hide"] {
                fs::write(abi_dir.join(name), elf_header(machine))?;
            }
        }

        let script = format!(
            "set -eu\nui_print() {{ :; }}\nabort() {{ echo \"abort: $*\" >&2; exit 1; }}\ngetprop() {{ echo arm64-v8a; }}\nMODPATH='{}'\n{snippet}\n",
            directory.to_string_lossy().replace('\\', "/")
        );
        let shell = if cfg!(windows) {
            "C:/Program Files/Git/bin/bash.exe"
        } else {
            "sh"
        };
        let output = Command::new(shell).args(["-c", &script]).output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        // 安装后只保留 runtime/ 根下的两个固定路径。
        // After install only the two fixed paths under runtime/ remain.
        assert!(directory.join("runtime/teeforge").is_file());
        assert!(directory.join("runtime/teeforge-hide").is_file());
        assert!(!directory.join("runtime/arm64-v8a").exists());
        assert!(!directory.join("runtime/x86").exists());
        let _ = fs::remove_dir_all(&directory);
        Ok(())
    }

    fn elf_header(machine: u16) -> Vec<u8> {
        let mut header = vec![0_u8; 20];
        header[..4].copy_from_slice(b"\x7fELF");
        header[18..20].copy_from_slice(&machine.to_le_bytes());
        header
    }

    #[test]
    fn shell_lf_check_rejects_carriage_returns() -> Result<()> {
        let directory = env::temp_dir().join(format!("teeforge-xtask-lf-{}", std::process::id()));
        fs::create_dir_all(&directory)?;
        let valid = directory.join("valid.sh");
        let invalid = directory.join("invalid.sh");
        fs::write(&valid, b"#!/system/bin/sh\necho ok\n")?;
        fs::write(&invalid, b"#!/system/bin/sh\r\necho bad\r\n")?;
        assert!(verify_shell_files_lf([valid]).is_ok());
        assert!(verify_shell_files_lf([invalid]).is_err());
        let _ = fs::remove_dir_all(directory);
        Ok(())
    }

    #[test]
    fn webui_binary_path_matches_installed_layout() -> Result<()> {
        // WebUI 与安装脚本必须指向同一个二进制路径。v0.8.0 改成 runtime/ 布局时
        // 只改了安装脚本，WebUI 仍调用旧路径，导致所有命令 exit 127。
        // The WebUI and installer must agree on the binary path; the runtime/ layout
        // change left the WebUI pointing at the old path, failing every command with 127.
        let ksu = fs::read_to_string(root().join("webroot/src/lib/ksu.ts"))?;
        let installer = fs::read_to_string(root().join("module/customize.sh"))?;
        let marker = "export const TEEFORGE = MODDIR + '/";
        let start = ksu.find(marker).ok_or("missing TEEFORGE constant")? + marker.len();
        let end = ksu[start..]
            .find('\'')
            .ok_or("unterminated TEEFORGE constant")?
            + start;
        let relative = &ksu[start..end];
        assert!(
            installer.contains(&format!("\"$MODPATH/{relative}\"")),
            "customize.sh does not install $MODPATH/{relative}"
        );
        Ok(())
    }
}
