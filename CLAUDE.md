# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TeeForge-CD is a Magisk/KernelSU module that:
- Auto-populates Tricky Store's `target.txt` with user-installed apps
- Implements weak bootloader hiding via resetprop (supports resetprop-rs)
- Manages keybox files with CDN mirror and obfuscation
- Auto-detects region (China/Global) and uses mirrors for downloads

Primary language: **C** (minimal binary size, direct NDK support).

## Build & Package Commands

```bash
# 设置 NDK 环境变量 [Set NDK environment]
export NDK="/path/to/android-ndk"

# 仅构建二进制 [Build binary only]
./build.sh

# 构建并打包 Magisk 模块 .zip [Build and package Magisk module]
./package.sh

# 清理后重新打包 [Clean and repackage]
./package.sh --clean

# 清理构建产物 [Clean build artifacts]
./clean.sh

# 构建并推送到设备 [Build and push to device]
./build.sh --push
```

## Architecture

### 开机流程 Boot Flow
```
service.sh
    ↓ 检测 resetprop-rs
    ↓ export RESETPROP_RS=...
    ↓ teeforge --hide-bl (弱隐 BL)
    ↓ teeforge --keybox (获取 keybox)
    ↓ teeforge --generate (生成 target.txt)
```

### 命令行 CLI
```
teeforge --generate           # 生成 target.txt
teeforge --hide-bl            # 弱隐 bootloader
teeforge --keybox             # 获取并更新 keybox
teeforge --download URL OUT   # 下载文件（带镜像回退）
teeforge --verbose            # 启用调试日志
teeforge --config FILE        # 使用自定义配置
```

### 配置文件 Configuration (`module/config.conf`)
```ini
region=auto                         # auto/cn/global
retry_count=3                       # 下载重试次数
cn_mirrors=https://gh-proxy.org,... # 中国镜像站
keybox_dir=/data/adb/teeforge/keybox/
target_txt=/data/adb/tricky_store/target.txt
```

### 关键实现细节 Key Implementation Details

- **keybox.c**: URL 和公钥经过 base64 编码后拆分成多个变量（A-Z），运行时拼合解码。源码备份在 `backup/`（gitignored）
- **blhide.c**: 从环境变量 `RESETPROP_RS` 获取 resetprop-rs 路径，支持 `--stealth`、`--compact`、`--delete`
- **download.c**: 自动检测地区（curl 测试 GitHub 连通性），中国大陆用户走镜像站
- **target.c**: 使用 `cmd package list packages -f` 获取包列表（非 XML 解析，兼容 Android 16）

### GitHub Action
`.github/workflows/keybox-sync.yml` 每12小时同步上游 keybox，推送到 `omg` 分支（混淆文件名，15个文件）

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
| `native/src/download.c` | 下载模块（地区检测、镜像回退） |
| `native/include/teeforge.h` | 公共头文件 |
| `module/config.conf` | 默认配置 |
| `module/service.sh` | 开机服务 |
| `module/customize.sh` | 安装脚本（下载 resetprop-rs） |
| `.github/workflows/keybox-sync.yml` | Keybox 同步 Action |
