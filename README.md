# TeeForge-CD

Android TEE 隐藏环境一体化工具模块（KernelSU / Magisk）

## 功能

- **自动填充 target.txt** — 读取用户已安装 app，写入 Tricky Store 目标列表
- **弱隐 BL** — 通过 resetprop 伪装 bootloader 状态（支持 resetprop-rs --stealth）
- **Keybox 管理** — 从 CDN 获取 keybox，自动解密同步到 Tricky Store
- **地区检测** — 自动检测中国大陆/海外，国内用户走镜像站下载

## 快速开始

```bash
# 设置 NDK
export NDK="/path/to/android-ndk"

# 构建
./build.sh

# 打包 Magisk 模块
./package.sh

# 推送到设备测试
./build.sh --push
```

## 命令行

```bash
teeforge --generate           # 生成 target.txt
teeforge --hide-bl            # 弱隐 bootloader
teeforge --keybox             # 获取并更新 keybox
teeforge --download URL OUT   # 下载文件（带镜像回退）
teeforge --verbose            # 调试日志
teeforge --config FILE        # 自定义配置
```

## 配置

编辑 `module/config.conf`：

```ini
region=auto                         # auto/cn/global
retry_count=3                       # 下载重试次数
cn_mirrors=https://gh-proxy.org,... # 中国镜像站
```

## 技术栈

| 组件 | 选型 |
|------|------|
| 核心二进制 | C（Android NDK 交叉编译） |
| 目标架构 | ARM64 (aarch64) |
| 模块框架 | Magisk / KernelSU 通用 |
| 构建 | NDK standalone toolchain |

## 项目结构

```
TeeForge-CD/
├── native/src/          # C 源代码
│   ├── main.c           # 入口、CLI
│   ├── target.c         # 包列表解析
│   ├── blhide.c         # 弱隐 BL
│   ├── keybox.c         # Keybox 获取
│   ├── download.c       # 下载模块
│   └── utils.c          # 工具函数
├── module/              # 模块文件
│   ├── service.sh       # 开机服务
│   ├── customize.sh     # 安装脚本
│   └── config.conf      # 配置
├── .github/workflows/   # GitHub Actions
├── build.sh             # 构建脚本
├── package.sh           # 打包脚本
└── clean.sh             # 清理脚本
```

## License

GPL-3.0
