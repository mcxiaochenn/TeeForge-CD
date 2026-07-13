# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TeeForge-CD is a Magisk/KernelSU module that:
- Auto-populates Tricky Store's `target.txt` with user-installed apps
- Implements weak bootloader hiding via resetprop
- Manages keybox files from configurable remote sources (Phase 2)

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

**产出 Output:**
- `out/teeforge` — 编译后的二进制 [Compiled binary]
- `out/build/` — 构建临时文件 [Build temp files]
- `out/teeforge_cd-v0.1.0.zip` — 可安装的 Magisk 模块 [Installable Magisk module]

## Architecture

### 目录结构 Directory Structure
```
TeeForge-CD/
├── module/              # 模块文件（安装到设备）[Module files]
│   ├── META-INF/        # Magisk 安装脚本
│   ├── module.prop      # 模块元信息
│   ├── customize.sh     # 安装脚本
│   ├── service.sh       # 开机服务
│   └── config.conf      # 默认配置
├── native/              # C 源代码 [C source code]
│   ├── src/
│   └── include/
├── docs/                # 文档 [Documentation]
├── build.sh             # 构建脚本
├── package.sh           # 打包脚本
├── clean.sh             # 清理脚本
└── .gitignore
```

### 核心流程 Core Flow
```
package.sh → build.sh → NDK 编译 [compile] → out/teeforge
         ↓
    复制模块文件 [Copy module files]
         ↓
    打包模块 zip [Package module zip]
         ↓
    out/teeforge_cd-v0.1.0.zip
         ↓
    Magisk/KernelSU 安装 [Install]
         ↓
    service.sh (开机启动 [boot]) → teeforge --hide-bl (弱隐BL)
                                 → teeforge --generate (生成target.txt)
```

### 包列表获取 Package Detection
使用 `cmd package list packages -f` 命令获取已安装应用（非 XML 解析）：
Uses `cmd package list packages -f` to get installed apps (not XML parsing):
- 输出格式 [Output format]: `package:/path/to/base.apk=package.name`
- 用户应用 [User apps]: 路径以 `/data/app/` 开头
- 系统应用 [System apps]: `/system/`, `/vendor/`, `/product/`, `/apex/`

### 配置文件 Configuration
配置通过 `module/config.conf` 外置（非硬编码）：
Config is externalized via `module/config.conf` (not hardcoded):
```ini
packages_xml=/data/system/packages.xml
target_txt=/data/adb/tricky_store/target.txt
keybox_dir=/data/adb/teeforge/keybox/
sources_conf=/data/adb/teeforge/sources.conf
log_file=/data/adb/teeforge/teeforge.log
```

### 关键路径 Key Paths
| 设备路径 Device Path | 用途 Purpose |
|---------------------|-------------|
| `/data/adb/tricky_store/target.txt` | 输出：Tricky Store 目标列表 |
| `/data/adb/teeforge/config.conf` | 用户配置（持久化） |
| `/data/adb/teeforge/keybox/` | Keybox 文件存储 |

## Code Style

- **双语要求 Bilingual requirement**: 所有日志和注释使用中英双语
  All logs and comments must be bilingual (Chinese + English)
  ```c
  log_msg(LOG_INFO, "发现 %d 个应用 [Found %d packages]", count, count);
  /* 解析参数 Parse arguments */
  ```

- C 代码目标 Android API 24+ (Magisk 20.4+ 兼容)
  C code targets Android API 24+ (Magisk 20.4+ compatibility)

- 错误处理：记录日志，永不崩溃，优雅降级
  Error handling: log to stderr, never crash, degrade gracefully

- 二进制体积优先：静态链接，避免大依赖
  Binary size priority: static linking, avoid large dependencies

## Source Files

| 文件 File | 说明 Description |
|----------|-----------------|
| `native/src/main.c` | 入口、参数解析、配置加载 |
| `native/src/target.c` | 包列表解析、生成 target.txt |
| `native/src/utils.c` | 日志、文件 I/O、字符串工具、配置解析 |
| `native/src/blhide.c` | 弱隐 BL（resetprop 属性伪装） |
| `native/include/teeforge.h` | 公共头文件、配置结构体 |
| `module/config.conf` | 默认配置文件 |
| `build.sh` | NDK 构建脚本 |
| `package.sh` | 打包脚本（构建 + zip） |
| `clean.sh` | 清理脚本 |
| `module/customize.sh` | Magisk 安装脚本 |
| `module/service.sh` | 开机服务脚本 |
