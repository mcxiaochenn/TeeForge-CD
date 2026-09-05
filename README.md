# TeeForge-CD

面向 Magisk、KernelSU 和 APatch 模块环境的 Android 工具，用于维护 TEE 目标模块的应用列表、执行 bootloader 属性弱化处理，并按需同步 Keybox。

**目标列表开机更新，Keybox 按需同步。**

> [!WARNING]
> `target.txt` 与 Keybox 可供三个对接目标使用：[Tricky Store](https://github.com/5ec1cff/TrickyStore)、[TEESimulator](https://github.com/JingMatrix/TEESimulator) 和 [TEESimulator-RS](https://github.com/Enginex0/TEESimulator-RS)。BL 弱化不等于重新锁定 bootloader，也不能保证通过任何外部检测。

> [!NOTE]
> 当前 `master` 构建基线为 Android 7.0（API 24）及以上，包含 `arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64` 四种 ABI。四架构自动化构建已经验证，真机与模拟器覆盖仍需分别验收；稳定 Release 可能晚于 `master`，实际支持范围以对应 Release 说明和安装输出为准。

[下载稳定版](https://github.com/mcxiaochenn/TeeForge-CD/releases/latest) · [项目文档](docs/README.md)

## 功能

- **自动维护 target.txt**：开机扫描用户安装的应用并更新兼容目标模块的应用列表，失败时保留旧文件。
- **弱化 BL 相关属性**：通过 resetprop 处理 bootloader、Verified Boot、调试和厂商相关属性，可按类别关闭。
- **按需同步 Keybox**：通过模块 Action、WebUI 或 CLI 获取并校验内容，更新失败时保留已有文件。
- **安装时架构选择**：模块包内置四种 ABI，安装器按设备 ABI 选择并校验对应 ELF，不使用其他架构兜底。
- **配置与 Root 检测**：识别 Magisk、KernelSU 或 APatch 环境，用户配置在升级时默认保留。
- **WebUI 与命令行**：支持模块 Action、兼容的模块 WebUI 以及设备端 CLI。

开机服务会更新模块描述、执行 BL 弱化并生成 `target.txt`；Keybox 下载只在用户主动运行 Action、WebUI 操作或 `--keybox` 时执行。

## 兼容性与前置条件

| 项目 | 当前 `master` 范围 |
|---|---|
| Android | Android 7.0 / API 24 及以上；Root 管理器自身可能有更高要求 |
| Root 环境 | Magisk、KernelSU、APatch 的兼容 Magisk 模块环境；未设置硬编码最低管理器版本 |
| ABI | `arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64` |
| 目标模块 | Tricky Store、TEESimulator、TEESimulator-RS |
| 属性工具 | 全架构支持 standard resetprop；ARM 可选预置 resetprop-rs，x86/x86_64 固定使用 standard |
| WebUI | 需要支持 KernelSU 模块 WebUI 接口的管理器；其他环境可使用 Action 或终端 |

安装前请确保：

- 已从本仓库官方 Release 下载模块 ZIP，而不是 GitHub 自动生成的 Source code 压缩包。
- 如需使用 `target.txt` 或 Keybox 功能，已安装并配置上述任一兼容目标模块。
- 知道当前 Root 管理器的模块禁用或安全模式入口，以便异常启动时恢复。
- 需要保留旧配置时，提前备份 `/data/adb/teeforge/config.conf`。

仓库当前没有宣称所有 Root 管理器版本、ROM 和设备组合都已完成真机验证。自动化结果与待验收项见[当前路线图](docs/maintenance/ROADMAP.md)。

## 下载与安装

普通用户应使用 [GitHub Releases](https://github.com/mcxiaochenn/TeeForge-CD/releases/latest) 中的正式模块 ZIP。

> [!WARNING]
> Dev 包和 GitHub Actions Artifact 仅用于测试，可能包含未发布行为，不能作为稳定版或长期回退包。稳定版与 Dev 版使用不同的更新渠道。

1. 在 Magisk、KernelSU 或 APatch 管理器的“模块”页面选择从本地安装。
2. 选择从官方 Release 下载的 TeeForge-CD ZIP，不要解压。
3. 安装器先校验包内 `.sha256` 清单，再选择并验证当前设备 ABI；清单缺失、文件异常或 ABI 不支持时会中止安装。
4. 检测到已有配置时：音量加保留配置，音量减清除全部 TeeForge 数据；10 秒无操作默认保留。
5. ARM 设备可选择属性工具：音量加使用兼容性优先的 standard resetprop，音量减使用 resetprop-rs；10 秒无操作默认 standard。x86/x86_64 自动使用 standard。
6. 安装完成后重启设备。

Recovery 刷入未纳入当前验收范围，请优先使用 Root 管理器安装。

## 安装后验证

重启并等待系统完成启动后，在 Root 管理器的模块页面检查 TeeForge-CD 描述：正常情况下会由“等待重启”更新为包含 Root 类型、TeeForge-CD 版本、ABI 和 Keybox 状态的信息。`keybox: N/A` 表示尚无本地 Keybox 记录，不代表安装失败；描述仍停留在等待状态，则表示描述更新尚未完成。描述更新发生在其他开机动作之前，因此还需继续验证实际使用的功能。

随后检查兼容目标模块共用的目标文件：

```sh
su -c 'test -s /data/adb/tricky_store/target.txt && wc -l /data/adb/tricky_store/target.txt'
```

命令应成功返回非零行数。文件不存在、为空或命令退出码非零时，应按“故障排查与反馈”收集信息。

如果只使用 BL 弱化且未安装兼容目标模块，可跳过 `target.txt` 检查，改为执行以下命令；退出码为 0 表示本次属性命令均已成功执行：

```sh
su -c '/data/adb/modules/teeforge_cd/teeforge --hide-bl'
```

还可以在 Root 管理器中运行模块 Action。Action 会依次同步 Keybox 和更新 `target.txt`；两步均成功时输出 `完成 [Done]`，无需再次重启。任一步失败都会输出非零状态，不应视为成功。

## 使用与配置

### 常用入口

- **开机自动执行**：更新模块描述、执行 BL 弱化、生成 `target.txt`。
- **模块 Action**：同步 Keybox 并重新生成 `target.txt`，无需重启。
- **WebUI**：在支持模块 WebUI 的管理器中运行对应操作并查看输出。
- **CLI**：适用于终端操作和问题定位。

常用 CLI 命令需要 Root：

```sh
su -c '/data/adb/modules/teeforge_cd/teeforge --generate'
su -c '/data/adb/modules/teeforge_cd/teeforge --hide-bl'
su -c '/data/adb/modules/teeforge_cd/teeforge --keybox'
su -c '/data/adb/modules/teeforge_cd/teeforge --help'
```

CLI 任一步失败都会返回非零退出码。完整参数见[项目概览](docs/project/OVERVIEW.md)。

### 配置

用户配置位于 `/data/adb/teeforge/config.conf`，升级时默认保留。常用总开关：

```ini
debug=0
blhide=1
```

- `debug=1`：将运行日志写入 `/data/adb/teeforge/logs/`。
- `blhide=0`：关闭全部 BL 弱化操作。
- 安装器生成的分类开关可分别设为 `0` 或 `1`。

`/data/adb/teeforge/sys.conf` 由安装器维护，不应手动编辑。配置修改会在下一次相关命令或开机服务执行时读取。数据流与安全边界见[当前架构](docs/project/ARCHITECTURE.md)。

## 更新、卸载与恢复

### 更新

- 从稳定 Release 或管理器的稳定更新入口安装新版。
- 检测到已有 `config.conf` 时，安装器默认保留该文件并清理其他运行时数据。
- 如果在安装提示中选择“全部清除”，`/data/adb/teeforge/` 下的配置、日志和缓存数据都会被删除。

### 卸载

在 Root 管理器中卸载 TeeForge-CD 后重启。卸载脚本会删除整个 `/data/adb/teeforge/` 数据目录，包括用户配置、日志和 TeeForge 的本地 Keybox 副本；需要保留配置时请先自行备份。

卸载脚本不会删除目标目录中的 `/data/adb/tricky_store/target.txt` 和 `/data/adb/tricky_store/keybox.xml`。这些文件可能仍被 Tricky Store、TEESimulator 或 TEESimulator-RS 使用；如确认不再需要，应在卸载并重启后先备份，再自行清理。

### 异常恢复

- BL 弱化行为异常但系统仍可启动时，先将 `blhide=0`，再重启或手动重新执行相关命令。
- 选择 resetprop-rs 后出现兼容性问题时，重新安装模块并选择 standard resetprop。
- 无法正常启动时，使用当前 Root 管理器的安全模式或模块禁用机制，进入系统后禁用或卸载 TeeForge-CD。

官方恢复说明：[Magisk 模块安全模式](https://topjohnwu.github.io/Magisk/faq.html) · [KernelSU Bootloop 恢复](https://kernelsu.org/guide/rescue-from-bootloop.html) · [APatch Bootloop 恢复](https://apatch.dev/rescue-bootloop.html)

## 故障排查与反馈

### 常见情况

- **安装提示校验失败**：删除当前 ZIP，从官方 Release 重新下载；不要修改或重新打包模块。
- **提示 ABI 不支持或 ELF 不匹配**：确认设备报告的主 ABI 属于支持列表，不要用其他架构二进制强行替换。
- **`target.txt` 没有生成**：确认目标模块使用 `/data/adb/tricky_store/` 兼容目录，然后手动运行 `--generate` 并检查真实退出码。
- **Keybox 同步失败**：检查网络和命令输出；失败时程序会保留已有文件，不要把非空响应直接视为有效结果。
- **x86/x86_64 没有 resetprop-rs 选项**：这是当前安装策略，使用 standard resetprop。
- **模块描述仍显示等待重启**：确认系统已经完成启动，运行 `--update-desc` 后检查退出码和日志。

需要文件日志时，将 `config.conf` 中的 `debug` 改为 `1`，复现问题后查看 `/data/adb/teeforge/logs/`。分享前请移除设备标识、账号信息和其他隐私内容。

提交 Issue 时请提供：

- TeeForge-CD 版本及下载渠道（Release 或 Dev）。
- Android 版本、设备 ABI、Root 实现及版本。
- Tricky Store、TEESimulator、TEESimulator-RS 或其他相关模块的名称及版本。
- 清晰的复现步骤、期望结果和实际结果。
- 安装输出、相关命令退出码及已脱敏日志。

[提交 GitHub Issue](https://github.com/mcxiaochenn/TeeForge-CD/issues)

## 开发文档

- [项目概览与 CLI](docs/project/OVERVIEW.md)
- [当前技术架构](docs/project/ARCHITECTURE.md)
- [构建、测试与发布](docs/development/BUILD_AND_RELEASE.md)
- [安装与设备验收](docs/operations/INSTALLATION.md)
- [当前路线图](docs/maintenance/ROADMAP.md)

本 README 描述当前 `master` 的用户可见行为；正式包的具体内容以对应 Release 说明和安装输出为准。

## License 与致谢

TeeForge-CD 采用 [GNU GPL v3 或更高版本](LICENSE)发布。

感谢以下项目提供模块接口、兼容目标或基础能力：

- [Magisk](https://github.com/topjohnwu/Magisk)
- [KernelSU](https://github.com/tiann/KernelSU)
- [APatch](https://github.com/bmax121/APatch)
- [Tricky Store](https://github.com/5ec1cff/TrickyStore)
- [TEESimulator](https://github.com/JingMatrix/TEESimulator)
- [TEESimulator-RS](https://github.com/Enginex0/TEESimulator-RS)
- [resetprop-rs](https://github.com/5ec1cff/resetprop-rs) 及其贡献者

第三方项目名称仅用于说明依赖与兼容关系，不代表官方合作或背书。
