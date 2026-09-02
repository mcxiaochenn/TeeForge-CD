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
const MAX_PACKAGE_SIZE: u64 = 6_291_456;

struct Abi {
    android: &'static str,
    rust: &'static str,
    clang: &'static str,
    cargo_linker_env: &'static str,
    elf_machine: u16,
}

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
        let rustflags = format!("-Clink-arg=--target={}", abi.clang);
        let mut cargo = Command::new("cargo");
        cargo
            .current_dir(&workspace)
            .args(["build", "--release", "-p", "teeforge", "--target", abi.rust])
            .env(abi.cargo_linker_env, &clang)
            .env("TEEFORGE_VERSION", &version)
            .env("RUSTFLAGS", rustflags);
        command_ok(
            &mut cargo,
            &format!("构建 {} [Build {}]", abi.android, abi.android),
        )?;

        let source = workspace
            .join("target")
            .join(abi.rust)
            .join("release")
            .join("teeforge");
        let destination = output_root.join(abi.android).join("teeforge");
        fs::create_dir_all(destination.parent().expect("destination parent"))?;
        fs::copy(&source, &destination)?;
        fs::copy(
            &source,
            output_root
                .parent()
                .expect("out parent")
                .join(format!("teeforge-{}", abi.android)),
        )?;
        let size = fs::metadata(&destination)?.len();
        if size > MAX_BINARY_SIZE {
            return Err(format!(
                "{} 体积 {} 超过 1.5 MiB 上限 [binary exceeds size limit]",
                abi.android, size
            )
            .into());
        }
        verify_elf(&destination, abi.elf_machine, &ndk)?;
        println!("    {}: {} bytes", abi.android, size);
    }
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

fn build_webui(workspace: &Path) -> Result<()> {
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
    let listing = Command::new("tar")
        .args(["-tf"])
        .arg(destination)
        .output()?;
    if !listing.status.success()
        || !String::from_utf8(listing.stdout)?
            .lines()
            .any(|line| line.ends_with(".sha256"))
    {
        return Err("ZIP 缺少 .sha256 [ZIP is missing .sha256]".into());
    }
    Ok(())
}

fn package() -> Result<()> {
    build_android()?;
    let workspace = root();
    build_webui(&workspace)?;
    let out = workspace.join("out");
    let stage = out.join("build").join("teeforge_cd");
    if stage.exists() {
        fs::remove_dir_all(&stage)?;
    }
    copy_tree(&workspace.join("module"), &stage)?;
    let stale = stage.join("teeforge");
    if stale.exists() {
        fs::remove_file(stale)?;
    }
    let bin = stage.join("bin");
    if bin.exists() {
        fs::remove_dir_all(&bin)?;
    }
    copy_tree(&out.join("bin"), &bin)?;
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
    let output = root().join("out").join("bin");
    for abi in ABIS {
        let binary = output.join(abi.android).join("teeforge");
        let size = fs::metadata(&binary)?.len();
        if size == 0 || size > MAX_BINARY_SIZE {
            return Err(format!("{} 产物无效 [invalid output]", abi.android).into());
        }
        println!("{}: {} bytes", abi.android, size);
    }
    Ok(())
}
