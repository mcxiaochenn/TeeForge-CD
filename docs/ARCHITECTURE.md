# TeeForge-CD 架构设计

## 整体架构

```
┌─────────────────────────────────────────────┐
│              Android Boot                    │
├─────────────────────────────────────────────┤
│  service.sh     →  teeforge 二进制          │
│         │                                   │
│         ▼                                   │
│  ┌───────────────────────────────────────┐  │
│  │  teeforge (C, 静态链接)               │  │
│  │                                       │  │
│  │  1. --update-desc  更新模块描述       │  │
│  │  2. --hide-bl      弱隐 BL 属性伪装   │  │
│  │  3. --generate    生成 target.txt     │  │
│  │  4. --keybox      获取并解密 keybox   │  │
│  └───────────────────────────────────────┘  │
│         │                                   │
│         ▼                                   │
│  /data/adb/tricky_store/                    │
│    ├── target.txt    ← 生成                 │
│    └── keybox.xml    ← 同步                 │
└─────────────────────────────────────────────┘
```

配套组件：
- **WebUI**（`webroot/`，Astro + TypeScript）— KernelSU 管理界面，通过 `ksu.spawn()`/`ksu.exec()` 调用 teeforge
- **GitHub Actions** — `dev.yml`（dev 构建）/`release.yml`（Release 构建）/`keybox-sync.yml`（keybox 同步），产物统一推送到 `page` 分支 `files/`，经自建域名 `teeforge.mcxiaochen.top` 分发

## 核心模块设计

### 1. target 生成器（MVP 核心）

**输入**：`cmd package list packages -f`（Android 包管理命令，兼容 Android 16 ABX 格式，不使用 XML 解析）

**逻辑**：
```
popen("cmd package list packages -f")
→ 逐行解析（每行格式 package:codePath=name）
→ 用 strrchr 取最后一个 '=' 拆分（路径本身可能含 '='）
→ 过滤 codePath 以 /data/app/ 开头的用户应用
→ 写入 target.txt（每行一个包名）
```

**输出**：`/data/adb/tricky_store/target.txt`

**关键点**：
- 包名数组声明为 `static`（`packages[MAX_PACKAGES][MAX_PKG_NAME]`，约 512KB），避免栈溢出（代价是非线程安全，当前单线程无碍）
- 只列用户应用，不含 Tricky Store 的 `*` 通配符或系统应用前缀

### 2. Keybox 管理器

**数据流**：
```
CDN URL（混淆变量拼合还原）
→ 下载降级：wget → curl → busybox wget（KSU/AP/Magisk 路径）
→ 多层解码（base64/XOR/hex/ROT13 组合，具体流程见本地维护文档 `backup/KEYBOX_CRYPTO.md`）
→ 校验含 "AndroidAttestation" 标记
→ 写入 keybox_dir/keybox.xml（旧文件改名 .bak）
→ 同步到 /data/adb/tricky_store/keybox.xml
```

**实现选型**：
- 下载用 `wget`/`curl`/`busybox wget` 命令降级；设备无 openssl，SHA256 用 toybox 自带 `sha256sum`
- base64/hex/ROT13 解码为纯 C 实现，无临时文件、无 fork
- URL 与公钥经混淆编码拆分为多变量，运行时拼合解码（防静态分析）

**CDN 同步**：
- 上游 MeowDump/Integrity-Box 每 12 小时同步（`keybox-sync.yml`）
- 加密混淆后以混淆文件名（含假文件干扰）推送到 page 分支 `files/keybox/`
- 设备端按月份派生混淆文件名（具体机制见本地维护文档 `backup/KEYBOX_CRYPTO.md`）

### 3. 弱隐 BL

**实现**：通过 resetprop（传统 / resetprop-rs）伪装 bootloader 解锁相关系统属性

**逻辑**：
```
安装时用户选择 resetprop 工具（standard/rs）→ 保存到 sys.conf prop_tool
→ bl_build_script() 将所有属性命令拼成一个 shell 脚本
→ 单次 system() 执行（非逐条 fork）
→ 覆盖 30+ 个属性：boot state / security / vendor / OEM unlock / recovery / developer / SELinux / realme / virtual
→ 属性删除（--delete）与内存整理（--compact，resetprop-rs only）
```

**工具降级**：
- 传统：`resetprop`（PATH）→ `/data/adb/ksu/bin/resetprop` → `/data/adb/ap/bin/resetprop` → `/data/adb/magisk/resetprop`
- resetprop-rs：环境变量 → 模块目录 → 系统 PATH，支持 `--stealth`/`--compact`/`--delete`

**功能开关**：`blhide`（总开关）+ 10 个类别开关 + delete + compact，由用户 `config.conf` 控制，默认全开

## 构建系统

### Rust + NDK 交叉编译配置

Cargo workspace 由 `xtask` 统一驱动，API 基线为 Android 24。每个 ABI 独立编译，
产物输出到 `out/bin/<abi>/teeforge`：

| Android ABI | Rust target | NDK linker target |
|---|---|---|
| arm64-v8a | aarch64-linux-android | aarch64-linux-android24 |
| armeabi-v7a | armv7-linux-androideabi | armv7a-linux-androideabi24 |
| x86 | i686-linux-android | i686-linux-android24 |
| x86_64 | x86_64-linux-android | x86_64-linux-android24 |

### build.sh 流程

```
1. 检测 NDK 环境变量
2. 按主机 OS 选择 prebuilt 工具链（linux/darwin/windows-x86_64）
3. 调用 `cargo run -p xtask -- build`
4. 为四个 Rust target 配置 NDK clang linker
5. 执行 LTO、体积优化和 strip
6. 校验 ELF machine、动态依赖与 1.5 MiB 单文件门禁
7. （可选 --push）按连接设备 ABI 推送对应产物
```

### package.sh 打包流程

```
1. 调用 xtask 构建四 ABI
2. 使用锁定依赖构建 WebUI
3. 在暂存目录注入 version/versionCode/渠道信息，不修改源码树 module.prop
4. 写入 `bin/<abi>/teeforge` 与 README
5. 生成 `.sha256` 校验清单（排除自身与 META-INF/）
6. 创建 ZIP 并复核 `.sha256` 存在，执行 6 MiB 包体门禁
```

## 安全设计

### 包一致性
- 安装前 `verify.sh` 基于 `.sha256` 逐文件校验，清单缺失或校验失败均 `abort`
- `.sha256` 用于发现下载损坏和不完整打包，不等同于签名，也不阻止重新制作整个 ZIP
- 四架构 ELF 在构建和安装阶段分别检查 machine 字段

### 运行权限
- 二进制以 root 执行（Magisk/KernelSU 上下文）
- 最小权限原则：只读包列表，只写 target.txt / keybox.xml

## 配置系统

- **sys.conf**：安装时自动生成，保存系统路径与 root 检测结果（勿手动编辑）
- **config.conf**：用户可编辑开关（debug、blhide 系列），跨更新保留
- **加载顺序**：`config_load()` 先读 sys.conf，再读 config.conf 覆盖；blhide 默认值全为 1

## 兼容性矩阵

| 环境 | 支持 |
|------|------|
| Magisk 20.4+ (API 24+) | ✅ |
| KernelSU 0.6+ | ✅ |
| Android 7.0 - 15 | ✅ |
| ARM64 设备 | ✅ arm64-v8a |
| ARMv7 设备 | ✅ armeabi-v7a |
| x86 环境 | ✅ standard resetprop |
| x86_64 环境 | ✅ standard resetprop |

> resetprop-rs 仅随 ARM 两种 ABI 提供；x86/x86_64 安装时固定使用 standard。
