# TeeForge-CD 当前架构

## 整体数据流

```text
Android boot
  → module/service.sh
  → selected ABI teeforge
  → /data/adb/teeforge/ 配置和日志
  → 一次用户应用扫描
      ├─ Tricky Store / TEESimulator-RS：target.txt
      ├─ TEESimulator：config.json profiles.teeforge.apps
      └─ OMK（安装时启用）：injector.toml scoop
```

WebUI 通过 `ksu.spawn()` 优先使用流式输出，缺少该接口时降级为 `ksu.exec()`；两者都显式使用模块目录和 `/data/adb/teeforge/config.conf`。

## Rust 核心模块

| 模块 | 职责 |
|---|---|
| `cli` | 兼容参数解析、动作顺序和统一退出码 |
| `config` | 默认值、系统/用户配置优先级和兼容回退 |
| `target` | 调用 `cmd package list packages -f -U --user 0`，筛选用户应用/UID，并适配三种配置格式后原子事务写入 |
| `omk` | 用锁定的 toml_edit 定位 scoop，保留手动项、注释及扩展表头，更新受管区块 |
| `keybox` | 下载后备、严格有界解码、内容标记校验和按实际后端多目标回滚 |
| `blhide` | 参数数组调用 resetprop，逐项收集失败 |
| `rootdetect` | 环境变量和 `/data/adb/*` 路径检测 |
| `volume` | 动态扫描 input event，封装设备输入 unsafe 适配 |
| `atomic_file` | 同目录临时文件、同步写入和原子替换；OMK 专用路径保留 UID/GID、mode 和 Android SELinux 标签 |
| `logging` | 分级日志、debug 文件和最近 15 份清理 |

所有设备端操作返回 `Result`；外部命令必须检查真实退出状态。target、配置、Keybox 和模块描述更新均不直接覆盖旧内容。

## 配置与路径

系统规范路径：

```text
/data/adb/teeforge/sys.conf
/data/adb/teeforge/config.conf
/data/adb/teeforge/logs/
/data/adb/teeforge/keybox/
/data/adb/tricky_store/target.txt
/data/adb/tricky_store/keybox.xml
/data/adb/teesim/config.json
/data/adb/teesim/keybox.xml
```

加载顺序为：默认值 → `sys.conf` → 用户配置；用户配置覆盖同名系统键。缺少规范文件时，开发环境允许回退到当前目录的旧配置文件。

## Keybox 数据流

```text
自建 CDN
  → wget / curl / 管理器 busybox 后备
  → 有界下载
  → 严格解码和 AndroidAttestation 标记校验
  → 本地 keybox.xml
  → 已存在的 Tricky Store/TEESimulator-RS 目录和/或 TEESimulator 目录中的 keybox.xml
```

每个已存在的目标采用原子写入；任一目标失败时恢复本轮已写目标和本地旧文件。具体私密维护细节只存在于被 Git 忽略的 `docs/private/KEYBOX_CRYPTO.md`，不得复制到公开文档。

`target.txt` 只属于 Tricky Store/TEESimulator-RS 兼容接口；当前 TEESimulator 使用独立的 JSON profile。目标适配器只处理真实存在的配置，不创建目标目录。

## 目标配置事务

`target` 模块先完成一次稳定的主用户 0 应用快照，再分别渲染所有已发现后端。`target.txt` 使用受管区块，区块外的注释、模式后缀和 Keybox 分段保持原顺序；TEESimulator 只更新带 `_teeforgeManaged: true` 标记的 `teeforge.apps`，首次创建时深复制 `default` 的非 `apps` 设置。所有渲染和源文件一致性检查通过后才写入，任一写入失败回滚本轮已写文件。

## OMK 作用域适配

OMK 启用状态只在安装时检测并写入 sys.conf 的 `omk_enabled`；默认关闭，用户配置可覆盖。安装器按模块 ID `oh_my_keymint` 检查待更新目录与已安装目录，待更新目录优先；disable/remove 标记使该目录不参与启用。运行时只根据固化状态和 `omk_injector_config` 工作，不重新发现模块。活动文件缺失时 warning 跳过，不创建目录；读取或解析失败时中止本轮目标事务。

适配基线为上游 `a1f3241f614948a0c17d9d127474878600d596f1`，兼容 Issue #1 附件对应 `67dc5e7`。接受缺失版本号、version=0、version=1，但不迁移版本；其他版本拒绝写入。只处理顶层字符串数组 scoop，保留其他字段。历史 [scoop.<package>] 扩展表头只在解析副本中等长映射，不写回原文件。

现有条目首次视为手动项；仅注释标记 BEGIN/END TeeForge-CD managed scoop 之间的自动项随主用户应用扫描更新。自动项排序去重、排除手动项，重复执行字节不变。非法语法、类型和标记拒绝写入，日志不包含 TOML 原文。

PendingUpdate 保存渲染使用的原始字节。OMK 替换前检查原始内容与元数据，并在限制权限的临时文件中准备原 UID/GID、mode 和 Android SELinux 标签；失败不提交。专用备份为 injector.toml.teeforge.bak，不占用 OMK 的备份名；回滚也保留元数据。回滚发现目标已被其他进程修改时拒绝覆盖并报告失败。跨文件事务无法保证多个热加载服务同时切换；写入完成不代表 OMK 已加载或应用路由已生效。

不接管 OMK config.toml、Keybox、filter、intercept 或服务重启。旧 C 不新增 OMK 支持。

## BL 隐藏

全架构的 Rust 与旧 C 均使用 standard resetprop，删除预置 rs 二进制和安装选择。Rust 使用参数数组逐条调用工具；失败项聚合到最终错误。旧 prop_tool 和 blhide_compact 键按未知键忽略，保留其他 BL 分类开关。

## 构建产物和 ABI

| Android ABI | Rust target | NDK linker target | ELF machine |
|---|---|---|---:|
| arm64-v8a | `aarch64-linux-android` | `aarch64-linux-android24` | 183 |
| armeabi-v7a | `armv7-linux-androideabi` | `armv7a-linux-androideabi24` | 40 |
| x86 | `i686-linux-android` | `i686-linux-android24` | 3 |
| x86_64 | `x86_64-linux-android` | `x86_64-linux-android24` | 62 |

ZIP 内置四个 ABI，`customize.sh` 根据 `ro.product.cpu.abi` 选择一个并校验 ELF 后删除其余版本。

## 安全边界

- `.sha256` 用于发现损坏或不完整，不等同于签名，也不能阻止重新制作 ZIP。
- 下载内容必须检查真实工具退出码、大小和格式，不能只检查非空。
- 日志不得输出派生密钥、解码内容或私密维护信息。
- Android 设备不依赖 `openssl`；Rust 使用锁定的 `sha2`，安装校验使用 toybox `sha256sum`。
