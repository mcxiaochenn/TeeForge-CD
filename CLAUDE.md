# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TeeForge-CD is a Magisk/KernelSU module that:
- Auto-populates Tricky Store's `target.txt` with user-installed apps
- Implements weak bootloader hiding via resetprop (supports resetprop-rs)
- Manages keybox files with CDN and obfuscation

Primary language: **C** (minimal binary size, direct NDK support). Secondary: **TypeScript** (WebUI, KernelSU interface).

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

`package.sh` runs `build.sh` internally, then also builds the WebUI (if Node.js is available), and creates the installable `.zip` in `out/`.

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

**加载顺序 Loading order**: `config_load()` in `utils.c` reads `sys.conf` first, then the user config (`config.conf`) on top — user settings override system defaults. blhide defaults are all set to 1 before any file is read.

### 安装流程 Installation Flow

`customize.sh` 执行顺序：
1. 文件完整性校验（`verify.sh`，基于 `.sha256` 校验文件，失败则中止安装）
2. 检测 root 方式（`teeforge --rootdetect`，环境变量可用）
3. 音量键选择：保留/清除已有配置（10s 超时默认保留）
4. 音量键选择 resetprop 工具（10s 超时默认传统方式）
   - 传统 resetprop（推荐）→ 删除 resetprop-rs/ 目录减小体积
   - resetprop-rs → 检测架构（arm64-v8a/armeabi-v7a），只保留对应二进制；x86/x86_64 中断安装
5. 生成 sys.conf（含 `prop_tool=standard|rs`）
6. 生成 config.conf（含 debug 和 blhide 开关）

**完整性校验 Integrity Verification**: `package.sh` 打包时对所有模块文件（排除 `.sha256` 自身和 `META-INF/`）生成 SHA256 校验和写入 `.sha256`。安装时 `verify.sh` 逐文件比对，校验失败则 `abort` 中止安装。README.md 也会打包进模块（重命名为 `README`，无扩展名）。

### 关键实现细节 Key Implementation Details

- **keybox.c**: URL 和公钥经过 base64 编码后拆分成多个变量（G/M/T/A/P 等），运行时拼合解密。源码备份在 `backup/`（gitignored）
  - 解密流程：下载 → base64 解码 → XOR(SHA256(pubkey)) → 10x base64 → hex decode → ROT13 → XML
  - base64/hex 解码为纯 C 实现（`base64_decode()`/`hex_decode()`），无临时文件无 fork
  - SHA256 使用 `sha256sum`（toybox 自带）代替 `openssl`（设备上通常不存在）
  - 下载降级策略：`wget -qO-` → `curl -sL` → busybox 路径（`/data/adb/{ksu,ap}/bin/busybox` 或 `/data/adb/magisk/busybox`）
  - 函数拆分：`keybox_build_urls()`、`keybox_decrypt()`、`keybox_compute_sha256()`、`keybox_write()`
  - 参考实现：Integrity-Box `webroot/common_scripts/key.sh`
- **blhide.c**: 安装时选择 resetprop 工具（传统 / resetprop-rs），选择保存到 sys.conf `prop_tool=standard|rs`
  - 传统方式降级策略：`resetprop`（PATH）→ `/data/adb/ksu/bin/resetprop` → `/data/adb/ap/bin/resetprop` → `/data/adb/magisk/resetprop`
  - resetprop-rs：环境变量 → 模块目录 → 系统 PATH，使用 `--stealth`、`--compact`、`--delete` 参数
  - 批量执行：`bl_build_script()` 将所有属性命令拼成一个 shell 脚本，单次 `system()` 执行（非逐条 fork）
  - 功能开关：`blhide`（总开关）+ 10 个类别开关（boot/security/vendor/oem/secureboot/recovery/realme/developer/selinux/virtual）+ delete + compact，用户配置文件控制，默认全开
- **target.c**: 使用 `cmd package list packages -f` 获取包列表（非 XML 解析，兼容 Android 16）
  - 包名数组声明为 `static`（`static char packages[MAX_PACKAGES][MAX_PKG_NAME]`），避免 ~512KB 栈溢出
- **volume.c**: 独立音量键监听模块，返回 1（音量+）/ 0（音量-）/ -1（超时）
- **日志系统**: debug 模式写入 `/data/adb/teeforge/logs/teeforge_YYYYMMDD.log`，自动清理保留最近 15 份。shell 脚本不单独写日志

### WebUI（KernelSU 管理界面）

`webroot/` 是一个 Astro + TypeScript 的 WebUI 项目，提供 KernelSU WebUI 界面：
- `npm ci && npm run build` 在 `webroot/` 下构建，产物输出到 `module/webroot/`
- `package.sh` 自动检测 Node.js 可用性，有则构建 WebUI，无则跳过
- 使用 `ksu.exec()` 调用 teeforge 二进制，需设置 `cwd` 为模块目录（`sys.conf` 用相对路径 `./`）
- `ksu.resetpropBusybox()` 用于 WebUI 内的 resetprop 调用

### GitHub Action
- `keybox-sync.yml` — 每12小时同步上游 keybox，推送到 `page` 分支 `files/keybox/`（混淆文件名，15个文件）
- `dev.yml` — push 触发 dev 构建，产物推送到 `page` 分支 `files/dev/`。版本号同步更新 `teeforge.h` 和 `module.prop`，`updateJson` 改为指向自建 CDN
- `release.yml` — 推送版本标签触发 Release 构建，更新 `page` 分支 `files/` 下的 release.json 和 CHANGELOG.md
- 所有 CDN 文件统一推送到 `page` 分支的 `files/` 目录，通过自建域名 `teeforge.mcxiaochen.top/files/` 访问
- **版本注入 Version injection**: CI 同时更新 `module/module.prop`（version, versionCode）和 `native/include/teeforge.h`（TEEFORGE_VERSION），两处必须同步

### 自动更新 Auto Update
- `module.prop` 中 `updateJson` 指向 `teeforge.mcxiaochen.top/files/update/release.json`
- dev 构建会将 `updateJson` 改为指向 `teeforge.mcxiaochen.top/files/dev/update/dev.json`
- release zip 和 dev zip 均通过自建 CDN 分发（`page` 分支 `files/`）

## Code Style

- **双语要求**: 所有日志和注释使用中英双语
  All logs and comments must be bilingual (Chinese + English)
- C 代码目标 Android API 24+ (Magisk 20.4+ 兼容)
- 错误处理：记录日志，永不崩溃，优雅降级
- 二进制体积优先：静态链接，避免大依赖
- **连接设备是用户主力机，调试前必须获得用户同意**

## Gotchas

- **`system()` 返回值是 `waitpid` 格式**：`(exit_code << 8) | signal`。32256 = 126 << 8 表示 "命令存在但不可执行"（权限问题），不是 resetprop 本身的错误。遇到非零返回值时先右移 8 位取真实 exit code。
- **`set_perm_recursive` 会重置权限**：Magisk 的 `set_perm_recursive` 必须在所有 `chmod` 之后调用，可执行二进制需要单独 `set_perm` 再次设置 755。
- **Android 设备没有 openssl**：涉及哈希的 shell 命令只能用 `sha256sum`（toybox 自带），不能用 `openssl`。
- **大数组用 `static`**：`target.c` 中 `packages[MAX_PACKAGES][MAX_PKG_NAME]`（~512KB）声明为 `static` 以避免栈溢出，但这也意味着该函数非线程安全（当前是单线程，无问题）。
- **`temp/Integrity-Box/`** 是上游参考项目的本地克隆（gitignored），用于对照解密实现和属性列表，不要提交。
- **CI 版本号双写**：版本号同时存在于 `module/module.prop` 和 `native/include/teeforge.h`，CI 用 `sed` 同时更新两处。本地开发改版本号时也要两处同步。
- **config.conf 不在仓库中**：`config.conf` 在 `customize.sh` 安装时动态生成，dev 构建由 CI 动态生成（debug=1）。不要提交 config.conf 到仓库。
- **`.sha256` 校验文件**：由 `package.sh` 打包时自动生成，不在仓库中。校验范围为模块内所有文件（排除 `.sha256` 自身和 `META-INF/`），使用 `sha256sum`（toybox 自带）。

## Project Directories

- `native/` — C 源码和头文件
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
| `native/src/main.c` | 入口、参数解析、CLI |
| `native/src/target.c` | 包列表解析、生成 target.txt |
| `native/src/utils.c` | 日志、文件 I/O、配置解析、模块描述更新 |
| `native/src/blhide.c` | 弱隐 BL（resetprop 属性伪装） |
| `native/src/keybox.c` | Keybox 获取与解密（混淆） |
| `native/src/volume.c` | 音量键监听 |
| `native/src/rootdetect.c` | Root 方式检测（env + path fallback） |
| `native/include/teeforge.h` | 公共头文件、config_t 结构体、版本号 |
| `module/service.sh` | 开机服务 |
| `module/customize.sh` | 安装脚本（配置保留逻辑） |
| `module/verify.sh` | 安装前文件完整性校验 |
| `module/uninstall.sh` | 卸载脚本 |
| `module/action.sh` | 手动执行（keybox + generate） |
| `module/resetprop-rs/` | 预置 resetprop-rs 二进制（arm64-v8a + armeabi-v7a） |
