# TeeForge-CD

Android TEE 隐藏环境一体化工具模块（KernelSU / Magisk）

## 定位

围绕 **TEE（Trusted Execution Environment）隐藏** 构建的 All-in-One 模块，通过编译后的二进制实现核心逻辑，防止简单拆包篡改。

## 功能路线

### MVP — v0.1.0
- [ ] 开机自动读取用户已安装 app 包名
- [ ] 写入 `/data/adb/tricky_store/target.txt`（每行一个包名）
- [ ] C 交叉编译工具链配置（Android NDK → ARM64）
- [ ] Magisk/KernelSU 模块安装脚本

### v0.2.0 — Keybox 管理
- [ ] 从远程站点抓取 keybox 文件
- [ ] 自动放置到对应路径
- [ ] 支持多 keybox 源配置

### v0.3.0 — 弱隐 BL
- [ ] Bootloader 状态伪装 / 弱隐藏
- [ ] 与 Tricky Store 联动

### v0.4.0+ — 扩展
- [ ] 自动更新机制
- [ ] 日志系统
- [ ] 配置文件支持
- [ ] WebUI 或 CLI 交互（可选）

## 技术栈

| 组件 | 选型 |
|------|------|
| 核心二进制 | **C**（Android NDK 交叉编译） |
| 目标架构 | ARM64 (aarch64) 为主，兼容 ARMv7 |
| 模块框架 | Magisk / KernelSU 通用 |
| 脚本层 | Shell（安装/启动钩子） |
| 构建 | NDK standalone toolchain + Makefile |

## 模块结构（规划）

```
TeeForge-CD/
├── module.prop
├── META-INF/com/google/android/
│   ├── update-binary
│   └── updater-script
├── customize.sh
├── post-fs-data.sh
├── service.sh
├── common/functions.sh
├── native/
│   ├── Makefile
│   └── src/
│       ├── main.c
│       ├── target.c
│       ├── keybox.c       # v0.2
│       └── utils.c
├── config/sources.conf    # v0.2
├── build.sh
├── docs/
│   ├── TODO.md
│   └── ARCHITECTURE.md
└── README.md
```

## 构建

> 环境要求：Android NDK r26+

```bash
export ANDROID_NDK_HOME=/path/to/ndk
./build.sh
# 输出位于 out/
```

## License

待定
