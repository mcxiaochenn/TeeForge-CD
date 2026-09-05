# TeeForge-CD 项目概览

## 项目定位

TeeForge-CD 是面向 Magisk/KernelSU 的 Android 模块，主要负责：

- 根据用户安装的应用生成 Tricky Store `target.txt`。
- 通过 resetprop 弱化隐藏 bootloader、Verified Boot 和调试相关属性。
- 从自建 CDN 获取、校验并同步 Keybox 文件。
- 通过 KernelSU WebUI 提供手动操作入口。

当前原生核心是 Rust，目标是 Android API 24+ 的四种 ABI；TypeScript/Astro 负责 WebUI，Shell 负责模块生命周期。

## 开机流程

```text
module/service.sh
  ├─ teeforge --update-desc
  ├─ teeforge --hide-bl
  └─ teeforge --generate
```

二进制运行时数据位于 `/data/adb/teeforge/`，Tricky Store 文件位于 `/data/adb/tricky_store/`。

## CLI 公共接口

```text
teeforge                      # banner、版本、root 信息和帮助
teeforge --generate           # 生成 target.txt
teeforge --hide-bl            # 执行弱隐 BL
teeforge --keybox             # 获取并同步 Keybox
teeforge --update-desc        # 更新模块描述
teeforge --rootdetect         # 输出 root 方法和版本
teeforge --no-rootdetect      # 跳过自动 root 检测
teeforge --volume SEC         # 监听音量键，输出 1/0/-1
teeforge --verbose            # 启用调试日志
teeforge --config FILE        # 覆盖用户配置来源
```

多动作执行保持兼容顺序；任一步失败都会使最终退出码为非零。

## 主要入口

| 路径 | 作用 |
|---|---|
| `crates/teeforge/` | Android 设备端 Rust CLI 与核心模块 |
| `xtask/` | 四 ABI 构建、ELF 校验、WebUI 构建和模块打包 |
| `module/` | Magisk/KernelSU 模块脚本与安装资源 |
| `webroot/` | Astro + TypeScript KernelSU WebUI |
| `native/` | 真机验收完成前保留的旧 C 对照实现 |
| `docs/` | 当前项目文档、决策、维护和历史记录 |

## 配置文件

- `/data/adb/teeforge/sys.conf`：安装时生成的系统配置，不应手动编辑。
- `/data/adb/teeforge/config.conf`：用户配置，跨更新保留。
- Rust 加载顺序为系统配置，再由用户配置覆盖；`--config FILE` 只覆盖用户配置来源。

详细键值和模块边界按需读取 [`ARCHITECTURE.md`](ARCHITECTURE.md)。
