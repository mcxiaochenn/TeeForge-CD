# TeeForge-CD

Android TEE 隐藏环境一体化工具模块（KernelSU / Magisk）

## 功能

- **自动填充 target.txt** — 读取用户已安装 app，写入 Tricky Store 目标列表
- **弱隐 BL** — 通过 resetprop-rs 伪装 bootloader 状态（预置 resetprop-rs，支持 --stealth / --compact / --delete）
- **Keybox 管理** — 从 CDN 获取加密 keybox，自动解密同步到 Tricky Store
- **音量键交互** — 安装时音量键选择配置保留/清除
- **调试日志** — debug 模式下日志按日期写入文件，自动清理保留最近 15 份

## 快速开始

```bash
# 设置 NDK
export NDK="/path/to/android-ndk"

# 构建
./build.sh

# 打包 Magisk 模块
./package.sh
```

## 命令行

```bash
teeforge --generate           # 生成 target.txt
teeforge --hide-bl            # 弱隐 bootloader
teeforge --keybox             # 获取并更新 keybox
teeforge --volume SEC         # 音量键监听（输出 1/0/-1）
teeforge --verbose            # 调试日志
teeforge --config FILE        # 自定义配置
```

## 配置

编辑 `module/config.conf`：

```ini
packages_xml=/data/system/packages.xml
target_txt=/data/adb/tricky_store/target.txt
keybox_dir=/data/adb/teeforge/keybox/
debug=0                         # 0=关闭, 1=开启（日志写入文件）
log_dir=/data/adb/teeforge/logs/
```

## 技术栈

| 组件 | 选型 |
|------|------|
| 核心二进制 | C（Android NDK 交叉编译） |
| 目标架构 | ARM64 (aarch64) / ARMv7 (armeabi-v7a) |
| 模块框架 | Magisk / KernelSU 通用 |
| 构建 | NDK standalone toolchain |
| CI/CD | GitHub Actions（dev / release / keybox-sync） |

## 项目结构

```
TeeForge-CD/
├── native/
│   ├── src/               # C 源代码
│   │   ├── main.c         # 入口、CLI
│   │   ├── target.c       # 包列表解析（cmd package list，兼容 Android 16）
│   │   ├── blhide.c       # 弱隐 BL（自动检测 resetprop-rs，权限自修复）
│   │   ├── keybox.c       # Keybox 获取（sha256sum 兼容无 openssl 设备）
│   │   ├── volume.c       # 音量键监听
│   │   └── utils.c        # 日志、配置解析、文件 I/O
│   └── include/
│       └── teeforge.h     # 公共头文件
├── module/
│   ├── service.sh         # 开机服务（弱隐 BL + 生成 target.txt）
│   ├── customize.sh       # 安装脚本（音量键配置保留）
│   ├── uninstall.sh       # 卸载脚本
│   ├── action.sh          # 手动执行（keybox + generate）
│   ├── config.conf        # 默认配置
│   └── resetprop-rs/      # 预置 resetprop-rs 二进制
│       ├── resetprop-arm64-v8a
│       └── resetprop-armeabi-v7a
├── .github/workflows/
│   ├── dev.yml            # push 触发 dev 构建
│   ├── release.yml        # tag 触发 Release 构建
│   └── keybox-sync.yml    # 每 12 小时同步上游 keybox
├── docs/
│   ├── ARCHITECTURE.md    # 架构说明
│   ├── TODO.md            # 待办事项
│   └── RETROSPECTIVE.md   # 反思小结
├── build.sh               # 构建脚本
├── package.sh             # 打包脚本
└── clean.sh               # 清理脚本
```

## 开机流程

```
service.sh
    ↓ 检测 resetprop-rs 并设置权限
    ↓ 等待系统启动完成
    ↓ teeforge --hide-bl（弱隐 BL，29 个属性）
    ↓ teeforge --generate（生成 target.txt）
```

## License

GPL-3.0
