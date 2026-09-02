# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TeeForge-CD is a Magisk/KernelSU module that:
- Auto-populates Tricky Store's `target.txt` with user-installed apps
- Implements weak bootloader hiding via resetprop (supports resetprop-rs)
- Manages keybox files with CDN and obfuscation

Primary language: **Rust** (four Android ABIs, memory-safe core). Secondary: **TypeScript** (WebUI) and shell (module lifecycle). The previous C implementation remains in `native/` only for migration comparison until authorized device acceptance.

## Build & Package Commands

```bash
# 设置 NDK 环境变量 [Set NDK environment]
export NDK="/path/to/android-ndk"

# 仅构建二进制 [Build binary only]
./build.sh

# 构建并推送到设备 [Build and push to device via adb]
./build.sh --push

# 构建并打包 Magisk 模块 .zip [Build and package Magisk module]
./package.sh

# 清理构建产物 [Clean build artifacts]
./clean.sh
```

`build.sh` and `package.sh` are compatibility wrappers over Cargo `xtask`. Packaging builds all four Android ABIs, builds the WebUI, verifies ELF metadata/checksums, and creates the installable `.zip` in `out/`.

## Architecture

### 开机流程 Boot Flow
```
service.sh
    ↓ teeforge --update-desc (更新模块描述 Update module description)
    ↓ teeforge --hide-bl (弱隐 BL)
    ↓ teeforge --generate (生成 target.txt)
```

### 命令行 CLI
```
teeforge                      # 展示 banner + 版本 + root 信息 + help
teeforge --generate           # 生成 target.txt
teeforge --hide-bl            # 弱隐 bootloader
teeforge --keybox             # 获取并更新 keybox
teeforge --update-desc        # 更新模块描述（root、arch、keybox 时间）
teeforge --rootdetect         # 检测 root 方式并输出到 stdout（供 shell 捕获）
teeforge --no-rootdetect      # 跳过 root 检测
teeforge --volume SEC         # 音量键监听（输出 1/0/-1）
teeforge --verbose            # 启用调试日志
teeforge --config FILE        # 使用自定义配置
```
- 无参数时展示 banner + 版本 + root 信息 + help，不执行任何任务
- 每次运行自动检测 root 方式并写入 sys.conf（`--rootdetect` 或 `--no-rootdetect` 可覆盖）
- root 检测优先级：环境变量 `$KSU` / `$APATCH` / `$MAGISK_VER_CODE` → 文件系统路径 `/data/adb/{ksu,ap,magisk}/`

### 配置文件 Configuration

两个配置文件，安装时动态生成（不预打包），用户配置跨更新保留：
- `config.conf` — 用户可编辑的开关（debug、blhide 系列）
- `sys.conf` — 系统自动生成的路径和检测结果（勿手动编辑）

```ini
# config.conf（用户配置，保留跨更新 User config, preserved across updates）
debug=0                         # 0=关闭, 1=开启（日志写入文件）
blhide=1                        # 弱隐 BL 总开关 Master switch
blhide_boot=1                   # Boot 状态
blhide_security=1               # 安全属性
blhide_vendor=1                 # Vendor 属性
blhide_oem=1                    # OEM 解锁
blhide_secureboot=1             # 安全启动
blhide_realme=1                 # Realme 设备
blhide_recovery=1               # Recovery 模式
blhide_developer=1              # Developer 选项
blhide_selinux=1                # SELinux 伪装
blhide_virtual=1                # 虚拟设备
blhide_delete=1                 # 属性删除
blhide_compact=1                # 内存整理

# sys.conf（系统配置，安装时自动生成 System config, auto-generated at install）
packages_xml=/data/system/packages.xml
target_txt=/data/adb/tricky_store/target.txt
keybox_dir=/data/adb/teeforge/keybox/
sources_conf=/data/adb/teeforge/sources.conf
log_dir=/data/adb/teeforge/logs/
root_method=KernelSU            # 自动检测 auto-detected
root_version=1234               # 自动检测 auto-detected
prop_tool=standard              # 安装时选择 install-time choice
```

**加载顺序 Loading order**: Rust `Config::load()` reads canonical `/data/adb/teeforge/sys.conf` first, then the selected user config on top. Missing canonical files fall back to legacy relative files. User settings override system defaults.

### 安装流程 Installation Flow

`customize.sh` 执行顺序：
1. 文件完整性校验（`verify.sh`，基于 `.sha256` 校验文件，失败则中止安装）
2. 根据 `ro.product.cpu.abi` 选择并校验对应 ELF，随后删除其余架构
3. 检测 root 方式（`teeforge --rootdetect`，环境变量可用）
4. 音量键选择：保留/清除已有配置（10s 超时默认保留）
5. 音量键选择 resetprop 工具（10s 超时默认传统方式）
   - 传统 resetprop（推荐）→ 删除 resetprop-rs/ 目录减小体积
- resetprop-rs → 仅 arm64-v8a/armeabi-v7a 可选；x86/x86_64 自动使用 standard
6. 生成 sys.conf（含 `prop_tool=standard|rs`）
7. 生成 config.conf（含 debug 和 blhide 开关）

**完整性校验 Integrity Verification**: `package.sh` 打包时对所有模块文件（排除 `.sha256` 自身和 `META-INF/`）生成 SHA256 校验和写入 `.sha256`。安装时 `verify.sh` 逐文件比对，校验失败则 `abort` 中止安装。README.md 也会打包进模块（重命名为 `README`，无扩展名）。

### 关键实现细节 Key Implementation Details

- **keybox.rs**: URL 和公钥经混淆编码拆分后运行时拼合；严格执行有界的 base64/XOR/hex/ROT13 解码与 Keybox 内容校验。**具体流程与维护步骤只记录在 gitignored 的本地维护文档中，不写入公开文档。**
  - SHA256 在进程内使用锁定版本的 `sha2`，不依赖设备上的 `openssl`
  - 下载降级策略：`wget -qO-` → `curl -sL` → busybox 路径（`/data/adb/{ksu,ap}/bin/busybox` 或 `/data/adb/magisk/busybox`）
  - 参考实现：上游 Integrity-Box 项目（解密流程对照参考）
- **blhide.rs**: 安装时选择 resetprop 工具（传统 / resetprop-rs），选择保存到 sys.conf `prop_tool=standard|rs`
  - 传统方式降级策略：`resetprop`（PATH）→ `/data/adb/ksu/bin/resetprop` → `/data/adb/ap/bin/resetprop` → `/data/adb/magisk/resetprop`
  - resetprop-rs：环境变量 → 模块目录 → 系统 PATH，使用 `--stealth`、`--compact`、`--delete` 参数
  - 使用 `Command` 参数数组逐条执行，汇总所有非零退出状态，不拼接 shell 脚本
  - 功能开关：`blhide`（总开关）+ 10 个类别开关（boot/security/vendor/oem/secureboot/recovery/realme/developer/selinux/virtual）+ delete + compact，用户配置文件控制，默认全开
- **target.rs**: 使用 `cmd package list packages -f` 获取包列表（非 XML 解析，兼容 Android 16），成功后原子替换 target.txt
- **volume.rs**: 动态扫描 `/dev/input/event*`，通过 `libc` 的目标 ABI 布局读取按键事件，返回 1（音量+）/ 0（音量-）/ -1（超时）
- **日志系统**: debug 模式写入 `/data/adb/teeforge/logs/teeforge_YYYYMMDD.log`，自动清理保留最近 15 份。shell 脚本不单独写日志

### WebUI（KernelSU 管理界面）

`webroot/` 是一个 Astro + TypeScript 的 WebUI 项目，提供 KernelSU WebUI 界面：
- `npm ci && npm run build` 在 `webroot/` 下构建，产物输出到 `module/webroot/`
- `package.sh` 自动检测 Node.js 可用性，有则构建 WebUI，无则跳过
- 使用 `ksu.spawn()`（流式输出，优先）或 `ksu.exec()`（一次性降级）调用 teeforge，并显式传入 `/data/adb/teeforge/config.conf`
- 命令执行逻辑在 `index.astro` 的 `runAction()`，流式日志追加到 LogDialog；`ksu.spawn` 不存在时降级到 `ksu.exec`

### GitHub Action
- `keybox-sync.yml` — 每12小时同步上游 keybox，推送到 `page` 分支 `files/keybox/`（混淆文件名，15个文件）
- `dev.yml` — push 触发 dev 构建，产物推送到 `page` 分支 `files/dev/`。版本号同步更新 `teeforge.h`（`module.prop` 由 `build.sh` 自动生成），`updateJson` 改为指向自建 CDN
- `release.yml` — 推送版本标签触发 Release 构建，更新 `page` 分支 `files/` 下的 release.json 和 CHANGELOG.md
- 所有 CDN 文件统一推送到 `page` 分支的 `files/` 目录，通过自建域名 `teeforge.mcxiaochen.top/files/` 访问
- **版本注入 Version injection**: 稳定版本唯一手写源是 `crates/teeforge/Cargo.toml`；xtask 在暂存目录生成 module.prop，CI 可通过 `VERSION`/`VERSION_CODE` 注入渠道版本。

### 自动更新 Auto Update
- `module.prop` 中 `updateJson` 指向 `teeforge.mcxiaochen.top/files/update/release.json`
- dev 构建会将 `updateJson` 改为指向 `teeforge.mcxiaochen.top/files/dev/update/dev.json`
- release zip 和 dev zip 均通过自建 CDN 分发（`page` 分支 `files/`）

## Code Style

- **双语要求**: 所有日志和注释使用中英双语
  All logs and comments must be bilingual (Chinese + English)
- Rust 核心目标 Android API 24+ (Magisk 20.4+ 兼容)
- 错误处理：记录日志，永不崩溃，优雅降级
- 二进制体积优先：静态链接，避免大依赖
- **连接设备是用户主力机，调试前必须获得用户同意**

## Gotchas

- **外部命令状态必须检查**：Rust 使用 `ExitStatus` 读取真实退出状态；下载、属性修改和包列表命令失败都要向 CLI 聚合为非零结果。
- **`set_perm_recursive` 会重置权限**：Magisk 的 `set_perm_recursive` 必须在所有 `chmod` 之后调用，可执行二进制需要单独 `set_perm` 再次设置 755。
- **Android 设备没有 openssl**：设备端 Rust 使用 `sha2`；安装包校验继续使用 toybox 的 `sha256sum`。
- **`temp/Integrity-Box/`** 是上游参考项目的本地克隆（gitignored），用于对照解密实现和属性列表，不要提交。
- **版本号单点维护**：稳定版本只修改 `crates/teeforge/Cargo.toml`；versionCode 默认取 Git 提交数，源码树中的 module.prop 不在构建时改写。
- **config.conf 不在仓库中**：`config.conf` 在 `customize.sh` 安装时动态生成，dev 构建由 CI 动态生成（debug=1）。不要提交 config.conf 到仓库。
- **`.sha256` 校验文件**：由 `package.sh` 打包时自动生成，不在仓库中。校验范围为模块内所有文件（排除 `.sha256` 自身和 `META-INF/`），使用 `sha256sum`（toybox 自带）。

## Project Directories

- `crates/teeforge/` — Rust CLI 与设备端核心逻辑
- `xtask/` — 四 ABI 构建、打包和验证
- `native/` — 迁移期旧 C 对照实现（真机验收后删除）
- `module/` — Magisk 模块框架（shell 脚本、module.prop、resetprop-rs 二进制）
- `webroot/` — Astro WebUI 源码（KernelSU 管理界面）
- `config/` — 默认 sources.conf
- `keybox/` — 上游同步状态文件（upstream_hash、month、key-status 等）
- `docs/` — 架构文档、反思小结、任务清单
- `out/` — 构建产物（gitignored）
- `temp/` — 本地参考项目（gitignored）

## Source Files

| 文件 | 说明 |
|------|------|
| `crates/teeforge/src/cli.rs` | 参数解析、动作顺序与退出码聚合 |
| `crates/teeforge/src/config.rs` | 配置默认值、优先级与兼容回退 |
| `crates/teeforge/src/target.rs` | 包列表解析、原子生成 target.txt |
| `crates/teeforge/src/keybox.rs` | 下载后备、严格解码与内容校验 |
| `crates/teeforge/src/blhide.rs` | resetprop 参数化执行与错误聚合 |
| `crates/teeforge/src/volume.rs` | Android input_event 适配与音量键监听 |
| `crates/teeforge/src/rootdetect.rs` | Root 方式检测（env + path fallback） |
| `xtask/src/main.rs` | 四 ABI 构建、ELF 校验与模块打包 |
| `module/service.sh` | 开机服务 |
| `module/customize.sh` | 安装脚本（配置保留逻辑） |
| `module/verify.sh` | 安装前文件完整性校验 |
| `module/uninstall.sh` | 卸载脚本 |
| `module/action.sh` | 手动执行（keybox + generate） |
| `module/resetprop-rs/` | 预置 resetprop-rs 二进制（arm64-v8a + armeabi-v7a） |
