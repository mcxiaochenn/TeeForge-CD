# TeeForge-CD

**开机即隐身，重启即更新，一行命令搞定一切。**

> *"不是我强，是别人太摆。"*
> — 辰渊尘

Android TEE 隐藏环境一体化工具模块（KernelSU / Magisk）

---

## 为什么选择 TeeForge-CD

你还在一个个手动添加 target.txt？
你还在为 bootloader 状态暴露而焦虑？
你还在满网找 keybox 然后手动替换？

**别了。**

| 你以前 | 现在 |
|--------|------|
| 手动编辑 target.txt | 开机自动填充 237+ 应用 |
| 手动跑 resetprop | 29 个属性 0.3 秒伪装到位 |
| 到处找 keybox | CDN 加密同步，一条命令搞定 |
| 担心逆向 | 三层加密 + 变量拆分，祝你好运 |

---

## 功能

- **弱隐 BL** — 29 个系统属性全量伪装，预置 resetprop-rs，`--stealth` 静默写入，权限自修复
- **自动填充 target.txt** — 兼容 Android 16 ABX 二进制格式，开机自动刷新
- **Keybox 管理** — CDN 加密同步，解密链路 `base64 → XOR(SHA256) → 10×base64 → hex → ROT13`
- **音量键交互** — 安装时音量键选择配置保留/清除
- **KernelSU 自动更新** — updateJson + jsdelivr CDN 加速，有新版自动提示
- **零依赖** — 不需要 openssl、不需要 busybox、不需要联网解密

## 命令行

```bash
teeforge --generate      # 刷一下，target.txt 搞定
teeforge --hide-bl       # 29 个属性瞬间到位
teeforge --keybox        # 下载、解密、部署，一条命令
teeforge --volume SEC    # 音量键监听（输出 1/0/-1）
teeforge --verbose       # 调试日志
teeforge --config FILE   # 自定义配置
```

## 技术细节

| 组件 | 选型 |
|------|------|
| 核心二进制 | 纯 C，NDK 静态链接，strip 后 < 500KB |
| 目标架构 | ARM64 / ARMv7 双预置 |
| 模块框架 | Magisk / KernelSU 通用 |
| keybox 解密 | 三层加密：XOR + 10×base64 + hex + ROT13 |
| 混淆方案 | URL/公钥 base64 编码后随机拆分为 5+ 变量 |
| 日志 | 按日期分文件，自动清理，15 份轮转，中英双语 |
| CI/CD | GitHub Actions 全自动（dev / release / keybox 12h 同步） |

## 快速开始

```bash
export NDK="/path/to/android-ndk"
./build.sh       # 构建
./package.sh     # 打包 Magisk 模块
```

## 配置

编辑 `module/config.conf`：

```ini
debug=0                         # 0=关闭, 1=开启（日志写入文件）
```

## 项目结构

```
TeeForge-CD/
├── native/src/            # C 源代码
│   ├── main.c             # 入口、CLI
│   ├── target.c           # 包列表解析（兼容 Android 16）
│   ├── blhide.c           # 弱隐 BL（权限自修复）
│   ├── keybox.c           # Keybox 加密解密
│   ├── volume.c           # 音量键监听
│   └── utils.c            # 日志、配置、文件 I/O
├── module/
│   ├── service.sh         # 开机服务
│   ├── customize.sh       # 安装脚本（音量键交互）
│   ├── action.sh          # 手动执行
│   └── resetprop-rs/      # 预置二进制
├── update/                # KernelSU 自动更新
│   └── release.json       # release 版本信息（CDN，dev 版在 dev 分支）
├── CHANGELOG.md           # 自动生成（release workflow）
├── .github/workflows/     # CI/CD 全自动
└── docs/                  # 架构、反思、待办
```

## License

GPL-3.0
