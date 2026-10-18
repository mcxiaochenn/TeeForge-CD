# TeeForge-CD 项目概览

## 项目定位

TeeForge-CD 是面向 Magisk/KernelSU 的 Android 模块，主要负责：

- 一次扫描用户安装的应用，分别更新实际存在的目标配置：Tricky Store/TEESimulator-RS 的 `target.txt`，以及 TEESimulator 的 `config.json` 受管 profile。
- 根据安装时固化的状态更新 Oh My Keymint（OMK）`injector.toml` 的 `scoop`，仅维护应用作用域。
- 通过 standard resetprop 弱化隐藏 bootloader、Verified Boot 和调试相关属性。
- 在高通 arm64 设备上通过本机 Keymint TA 向 Google RKP 申请本机专属 Keybox；签发与安装独立，不自动回退远程来源。
- 从自建 CDN 选择三个远程槽位之一，校验格式后事务同步 Keybox；支持强制重写相同内容。
- 通过 KernelSU WebUI 提供手动操作入口。

当前原生核心是 Rust，目标是 Android API 24+ 的四种 ABI；TypeScript/Astro 负责 WebUI，Shell 负责模块生命周期。

## 开机流程

```text
module/service.sh
  ├─ teeforge --update-desc
  ├─ teeforge --hide-bl
  └─ teeforge --generate
```

二进制运行时数据位于 `/data/adb/teeforge/`；Tricky Store/TEESimulator-RS 文件位于 `/data/adb/tricky_store/`，TEESimulator 文件位于 `/data/adb/teesim/`。

## CLI 公共接口

```text
teeforge                      # banner、版本、root 信息和帮助
teeforge --generate           # 更新已存在的目标配置
teeforge --hide-bl            # 执行弱隐 BL
teeforge --keybox             # 按已保存来源和槽位获取并同步 Keybox
teeforge --keybox --source remote --remote-slot 2 --force --save-selection
                             # 强制替换远程槽位 2，成功后保存选择
teeforge --keybox-validate FILE --json # 只读校验格式，不联网、不安装
teeforge --rkp-status         # 检查设备端 RKP 前置条件（不联网）
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
| `crates/teeforge/` | Android 设备端 Rust CLI 与核心模块（含 `rkp/` 设备端签发） |
| `xtask/` | 四 ABI 构建、ELF 校验、WebUI 构建和模块打包 |
| `module/` | Magisk/KernelSU 模块脚本与安装资源 |
| `webroot/` | Astro + TypeScript KernelSU WebUI |
| `native/` | 真机验收完成前保留的旧 C 对照实现 |
| `docs/` | 当前项目文档、决策、维护和历史记录 |

## 配置文件

- `/data/adb/teeforge/sys.conf`：安装时生成的系统配置，不应手动编辑。
- `/data/adb/teeforge/config.conf`：用户配置，跨更新保留。
- `target_txt=/data/adb/tricky_store/target.txt`：Tricky Store/TEESimulator-RS 目标文件。
- `omk_enabled=0`：默认关闭；安装器检测 OMK 后写入系统配置，用户配置可覆盖。
- `omk_injector_config=/data/misc/keystore/omk/injector.toml`：OMK 活动作用域配置，不用于 Keybox。
- `teesim_config=/data/adb/teesim/config.json`：TEESimulator 配置；其 Keybox 路径由父目录派生。
- `keybox_source=remote`：来源为 `remote` 或 `rkp`，互不回退；旧 `rkp_mode` 不生效。
- `keybox_remote_slot=1`：远程池槽位 1/2/3；WebUI 安装成功后与来源一起保存，切换 RKP 不清除此值。
- `rkp_server=https://remoteprovisioning.googleapis.com/v1`：RKP 服务地址，仅调试或自建代理时修改。
- `rkp_refresh_days=30`：设备签发 Keybox 的复用窗口，`0` 表示每次重签。
- Rust 加载顺序为系统配置，再由用户配置覆盖；`--config FILE` 只覆盖用户配置来源。

详细键值和模块边界按需读取 [`ARCHITECTURE.md`](ARCHITECTURE.md)。
