# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TeeForge-CD is a Magisk/KernelSU module that:
- Auto-populates Tricky Store's `target.txt` with user-installed apps
- Implements weak bootloader hiding via resetprop (supports resetprop-rs)
- Manages keybox files with CDN and obfuscation

Primary language: **C** (minimal binary size, direct NDK support).

## Build & Package Commands

```bash
# 设置 NDK 环境变量 [Set NDK environment]
export NDK="/path/to/android-ndk"

# 仅构建二进制 [Build binary only]
./build.sh

# 构建并打包 Magisk 模块 .zip [Build and package Magisk module]
./package.sh

# 清理构建产物 [Clean build artifacts]
./clean.sh
```

## Architecture

### 开机流程 Boot Flow
```
service.sh
    ↓ teeforge --hide-bl (弱隐 BL)
    ↓ teeforge --generate (生成 target.txt)
```

### 命令行 CLI
```
teeforge                      # 展示 banner + 版本 + root 信息 + help
teeforge --generate           # 生成 target.txt
teeforge --hide-bl            # 弱隐 bootloader
teeforge --keybox             # 获取并更新 keybox
teeforge --rootdetect         # 检测 root 方式并输出到 stdout（供 shell 捕获）
teeforge --no-rootdetect      # 跳过 root 检测
teeforge --volume SEC         # 音量键监听（输出 1/0/-1）
teeforge --verbose            # 启用调试日志
teeforge --config FILE        # 使用自定义配置
```
- 无参数时展示 banner + 版本 + root 信息 + help，不执行任何任务
- 每次运行自动检测 root 方式并写入 sys.conf（`--rootdetect` 或 `--no-rootdetect` 可覆盖）

### 配置文件 Configuration
```ini
# config.conf（用户配置，保留跨更新 User config, preserved across updates）
debug=0                         # 0=关闭, 1=开启（日志写入文件）

# sys.conf（系统配置，安装时自动生成 System config, auto-generated at install）
packages_xml=/data/system/packages.xml
target_txt=/data/adb/tricky_store/target.txt
keybox_dir=/data/adb/teeforge/keybox/
sources_conf=/data/adb/teeforge/sources.conf
log_dir=/data/adb/teeforge/logs/
root_method=KernelSU            # 自动检测 auto-detected
root_version=1234               # 自动检测 auto-detected
```

### 关键实现细节 Key Implementation Details

- **keybox.c**: URL 和公钥经过 base64 编码后拆分成多个变量（G/M/T/A/P 等），运行时拼合解密。源码备份在 `backup/`（gitignored）
  - 解密流程：下载 → base64 解码 → XOR(SHA256(pubkey)) → 10x base64 → hex decode → ROT13 → XML
  - 使用 `sha256sum`（toybox 自带）代替 `openssl`（设备上通常不存在）
  - 下载降级策略：`curl -sL` → `wget -qO-` → busybox 路径（`/data/adb/{ksu,ap}/bin/busybox` 或 `/data/adb/magisk/busybox`）
  - 参考实现：Integrity-Box `webroot/common_scripts/key.sh`
- **blhide.c**: 检测 resetprop-rs 路径（环境变量 → 模块目录 → 系统 PATH），支持 `--stealth`、`--compact`、`--delete`。检测到无执行权限时自动 `chmod 755`
- **volume.c**: 独立音量键监听模块，返回 1（音量+）/ 0（音量-）/ -1（超时）
- **target.c**: 使用 `cmd package list packages -f` 获取包列表（非 XML 解析，兼容 Android 16）
- **日志系统**: debug 模式写入 `/data/adb/teeforge/logs/teeforge_YYYYMMDD.log`，自动清理保留最近 15 份。shell 脚本不单独写日志

### GitHub Action
- `keybox-sync.yml` — 每12小时同步上游 keybox，推送到 `page` 分支 `files/keybox/`（混淆文件名，15个文件）
- `dev.yml` — push 触发 dev 构建，产物推送到 `page` 分支 `files/dev/`。版本号同步更新 `teeforge.h` 和 `module.prop`，`updateJson` 改为指向自建 CDN
- `release.yml` — 推送版本标签触发 Release 构建，更新 `page` 分支 `files/` 下的 release.json 和 CHANGELOG.md
- 所有 CDN 文件统一推送到 `page` 分支的 `files/` 目录，通过自建域名 `teeforge.mcxiaochen.top/files/` 访问

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

## Source Files

| 文件 | 说明 |
|------|------|
| `native/src/main.c` | 入口、参数解析、CLI |
| `native/src/target.c` | 包列表解析、生成 target.txt |
| `native/src/utils.c` | 日志、文件 I/O、配置解析 |
| `native/src/blhide.c` | 弱隐 BL（resetprop 属性伪装） |
| `native/src/keybox.c` | Keybox 获取与解密（混淆） |
| `native/src/volume.c` | 音量键监听 |
| `native/src/rootdetect.c` | Root 方式检测（env + path fallback） |
| `native/include/teeforge.h` | 公共头文件 |
| `module/service.sh` | 开机服务 |
| `module/customize.sh` | 安装脚本（配置保留逻辑） |
| `module/uninstall.sh` | 卸载脚本 |
| `module/action.sh` | 手动执行（keybox + generate） |
| `module/resetprop-rs/` | 预置 resetprop-rs 二进制（arm64-v8a + armeabi-v7a） |
