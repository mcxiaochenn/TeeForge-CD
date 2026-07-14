# TeeForge-CD 任务清单

> 最后更新 [Last updated]：2026-07-14

---

## Phase 1：MVP — 自动填充 target.txt ✅ 已完成 [Completed]

### 1.1 项目基础设施 Project Infrastructure
- [x] 初始化 Git 仓库 [Initialize Git repo]
- [x] 创建 `.gitignore`（忽略 `out/`）
- [x] 编写 `build.sh` 一键构建脚本 [Build script]
- [x] 编写 `package.sh` 打包脚本 [Packaging script]
- [x] 编写 `clean.sh` 清理脚本 [Clean script]
- [x] 配置 Android NDK 交叉编译（aarch64-linux-android）
- [x] 整理目录结构（模块文件移至 `module/`）[Organize directory structure]

### 1.2 核心二进制 Core Binary（C）
- [x] `native/src/main.c` — 程序入口，参数解析，配置加载
- [x] `native/src/target.c` — 获取包列表并生成 target.txt
  - [x] 使用 `cmd package list packages -f`（替代 XML 解析）
  - [x] 过滤用户安装 app（路径以 `/data/app/` 开头）
  - [x] 输出到 `/data/adb/tricky_store/target.txt`
- [x] `native/src/utils.c` — 文件读写、字符串处理、日志、配置解析
- [x] `native/include/teeforge.h` — 公共头文件、配置结构体

### 1.3 模块框架 Module Framework
- [x] `module/module.prop` — 模块元信息
- [x] `module/META-INF/` — Magisk 安装脚本
- [x] `module/customize.sh` — 安装脚本（保留用户配置）
- [x] `module/service.sh` — 开机执行
- [x] `module/config.conf` — 外置配置文件 [External config file]

### 1.4 验证 Verification
- [x] 本地 NDK 编译通过 [Local NDK compile]
- [x] 推送到设备测试 [Device testing]
- [x] 确认 target.txt 内容正确 [Verify target.txt]
- [x] 中英双语日志 [Bilingual logging]

---

## Phase 2：Keybox 管理 Keybox Management ✅ 已完成 [Completed]

- [x] `native/src/keybox.c` — C 实现 keybox 获取与解密
- [x] `--keybox` 命令行选项
- [x] 从 jsdelivr CDN 获取（镜像 GitHub omg 分支）
- [x] 自动备份已有 keybox
- [x] 同步到 Tricky Store
- [x] `module/service.sh` 开机自动执行
- [x] `.github/workflows/keybox-sync.yml` — 自动同步上游
  - [x] 每12小时检查上游更新
  - [x] 新月份强制更新
  - [x] 同步状态文件到 main 分支

---

## Phase 3：弱隐 BL Weak Bootloader Hiding ✅ 已完成 [Completed]

- [x] `native/src/blhide.c` — C 实现弱隐逻辑（resetprop 属性伪装）
- [x] `--hide-bl` 命令行选项
- [x] `module/service.sh` 开机自动执行
- [x] 支持 30+ 个属性伪装
  - [x] Boot state (verifiedbootstate, device_state, flash.locked)
  - [x] Security (debuggable, secure, build.type, build.tags)
  - [x] Vendor properties
  - [x] OEM unlock
  - [x] Recovery 模式隐藏 (bootmode)
  - [x] Developer 选项 (developer_options, dev_mode)
  - [x] SELinux 伪装
  - [x] 虚拟设备检测 (virtual_device)
  - [x] Realme 设备支持
- [x] 自动检测 resetprop-rs（支持 --stealth 模式）
- [x] 属性删除（ro.build.selinux）
- [x] Compact 内存整理（resetprop-rs only）
- [x] 属性来源标注（参考 Integrity-Box）

---

## Phase 4：扩展 Extensions

- [x] 日志系统（分级、轮转）— debug 模式按日期分文件，自动清理保留 15 份
- [x] 自动更新模块 — KernelSU updateJson + jsdelivr CDN，release/dev 双通道
- [ ] 用户配置文件（排除列表等）
- [ ] WebUI / CLI（可选）

---

## 环境依赖 Dependencies

| 依赖 | 版本 | 状态 |
|------|------|------|
| Android NDK | r27+ | ✅ 已安装 [Installed] |
| Git | 任意 | ✅ 已确认 |
| adb | 任意 | ✅ 已确认 |

---

## 决策记录 Decision Log

| 日期 Date | 决策 Decision | 原因 Reason |
|-----------|--------------|-------------|
| 2026-07-14 | 核心语言选择 C | 二进制体积小、NDK 原生支持、底层操作直接 |
| 2026-07-14 | 模块名 TeeForge-CD | TEE + Forge 双关，CD 为作者标识 |
| 2026-07-14 | MVP 聚焦 target.txt | 最小可用，快速验证模块框架 |
| 2026-07-14 | 使用 cmd package list | 替代 XML 解析，兼容 Android 16 ABX 格式 |
| 2026-07-14 | 配置外置到 config.conf | 避免硬编码，用户可自定义路径 |
| 2026-07-14 | 日志注释中英双语 | 便于国际化，保留中文可读性 |
| 2026-07-14 | Keybox 来源参考 Integrity-Box | 使用 MeowDump/MeowDump/Megatron |
| 2026-07-14 | 使用 jsdelivr CDN | 中国大陆用户体验优化 |
