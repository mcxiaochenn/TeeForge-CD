# TeeForge-CD

面向 Magisk、KernelSU、APatch 的 Android 模块：维护 Tricky Store、TEESimulator、TEESimulator-RS 的目标应用列表，弱化 bootloader 相关属性，并按需同步 Keybox。目标配置开机自动更新，Keybox 只在主动操作时同步。

> [!WARNING]
> 三个目标的配置契约不同：Tricky Store 与 TEESimulator-RS 使用 `/data/adb/tricky_store/target.txt`，TEESimulator 使用 `/data/adb/teesim/config.json` 的 `profiles.teeforge.apps`，Keybox 分别位于对应目录。Oh My Keymint（OMK）只适配应用作用域，不接管其 Keybox 或服务配置。BL 弱化不等于重新锁定 bootloader，也不保证通过任何外部检测。

[下载稳定版](https://github.com/mcxiaochenn/TeeForge-CD/releases/latest) · [项目文档](docs/README.md)

## 功能

- 目标配置维护：一次扫描用户应用，更新已存在的兼容目标配置；事务写入，失败回滚，不创建目标目录。
- BL 属性弱化：resetprop 覆盖 bootloader、Verified Boot、调试和厂商相关属性，可按类别开关，开机分两阶段执行。
- Keybox 同步：来源显式二选一——远程公共密钥池（3 个槽位）或本机 RKP 产物，两条路互不回退。
- 设备端 RKP 签发：仅限高通 arm64，调用本机 Keymint TA 向 Google RKP 申请本机专属证书链，产物与安装解耦。
- 目标作用域：全量（auto）或手动勾选（manual），手动选择保存在本机，重启后保持。
- OMK 作用域适配：安装时检测并固化启用状态，只维护 `injector.toml` 的 `scoop`。
- 模块自更新：WebUI 检查更新，走自建 CDN，校验清单 sha256 后调用管理器安装。
- 操作入口：模块 Action（音量键菜单）、WebUI、设备端 CLI。

## 兼容性与前置条件

| 项目 | 范围 |
|---|---|
| Android | 7.0 / API 24 及以上（Root 管理器自身可能要求更高） |
| Root 环境 | Magisk、KernelSU、APatch 的模块环境 |
| ABI | `arm64-v8a`、`armeabi-v7a`、`x86`、`x86_64` |
| 目标模块 | Tricky Store、TEESimulator、TEESimulator-RS；OMK 仅应用作用域，OMK 自身要求 Android 12+ |
| Keybox 路径 | Tricky Store/TEESimulator-RS：`/data/adb/tricky_store/keybox.xml`；TEESimulator：`/data/adb/teesim/keybox.xml` |
| 设备端 RKP | `arm64-v8a` + 高通 SoC + `/vendor/lib64/libkeymasterdeviceutils.so` + `/dev/smcinvoke`；其余环境只能用远程 Keybox |
| WebUI | 需要支持 KernelSU 模块 WebUI 接口的管理器；其他环境用 Action 或终端 |

四 ABI 自动化构建已验证；正式版在对应 Dev 版通过真机测试后发布，实际支持范围以 Release 说明和安装输出为准，记录见[路线图](docs/maintenance/ROADMAP.md)。

安装前：

- 从 [GitHub Releases](https://github.com/mcxiaochenn/TeeForge-CD/releases/latest) 下载模块 ZIP，不要用自动生成的 Source code 压缩包。
- 使用目标配置或 Keybox 前先装好任一兼容目标模块；TeeForge 只更新真实存在的后端配置。
- 知道当前 Root 管理器的安全模式或模块禁用入口，异常启动时靠它恢复。
- 需要保留旧配置时提前备份 `/data/adb/teeforge/config.conf`。

## 下载与安装

普通用户只用 GitHub Releases 的正式模块 ZIP。Dev 包和 GitHub Actions Artifact 仅用于测试，可能包含未发布行为，不能当稳定版或长期回退包；稳定版与 Dev 版走不同更新渠道。

1. 在 Magisk、KernelSU 或 APatch 的“模块”页面选择本地安装。
2. 选中 TeeForge-CD ZIP，不要解压。
3. 安装器校验包内 `.sha256` 清单，再选择并验证设备 ABI；清单缺失、文件异常或 ABI 不支持会中止安装。
4. 检测到已有配置：音量+ 保留，音量- 全部清除，10 秒无操作默认保留。
5. 安装输出会显示 Root 环境、ABI 和 OMK 作用域检测结果。
6. 重启生效。

Recovery 刷入不在当前验收范围，请用 Root 管理器安装。

## 安装后验证

重启后在模块页面看 TeeForge-CD 描述：应从“等待重启”变为包含 Root 类型、模块版本、ABI 和 Keybox 状态的信息。`keybox: N/A` 表示还没有本地 Keybox 记录，不是安装失败；描述一直没变说明描述更新未完成。描述更新先于其他开机动作，仍需按下面步骤确认实际功能。

装了 Tricky Store / TEESimulator-RS：

```sh
su -c 'test -s /data/adb/tricky_store/target.txt && wc -l /data/adb/tricky_store/target.txt'
```

应返回非零行数。装了 TEESimulator 时检查受管 profile：

```sh
su -c "grep -n 'teeforge\|_teeforgeManaged' /data/adb/teesim/config.json"
```

文件不存在、为空、缺少受管 profile 或命令退出码非零时，按“故障排查与反馈”收集信息。

只用 BL 弱化、未装目标模块时跳过上面两步，直接跑一次弱隐（退出码 0 表示执行完成，单条属性失败只记 WARN）：

```sh
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge-hide'
```

也可以在管理器里运行模块 Action，一步完成目标配置与 Keybox 更新，成功输出 `完成 [Done]`，无需再重启；任一步失败都返回非零，不算成功。

## 使用与配置

### 入口

- 开机自动：post-fs-data 阶段弱隐一次；开机完成后更新模块描述、二次弱隐、更新目标配置。
- 模块 Action：音量键两级菜单（音量下切换、音量上确认、15 秒无操作取消）。先确认是否执行，再选“一键更新目标作用域和 Keybox”“更新目标作用域”或“更新 Keybox”；涉及 Keybox 时先选来源。一键流程先写目标配置，目标配置失败就跳过 Keybox 并返回非零。日志写入 `/data/adb/teeforge/logs/action-*.log`。
- WebUI：顶部设备信息栏，下面四个入口——更新目标作用域、Keybox 配置、本机 RKP 签发、模块与更新。
- CLI：终端操作和问题定位，命令见下。

Keybox 选远程池时可选槽位 1/2/3（默认 1），安装成功后记住来源和槽位。内容相同的文件默认跳过；“强制拉取并替换”会重新请求并重写模块副本及所有已检测到的后端。所选槽位当月未发布时沿用该槽位最近已发布的版本（实际月份写入日志），失败不换槽；下载、校验或写入失败保留旧数据。格式有效不代表未吊销、后端接受或通过外部检测。

### CLI

常用命令（需要 Root）：

```sh
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge --generate'
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge-hide --verbose'
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge --keybox --source remote --remote-slot 1 --save-selection'
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge --keybox --source rkp'
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge --rkp-status'
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge --rkp-issue'
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge --target-list --json'
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge --self-update check'
su -c '/data/adb/modules/teeforge_cd/runtime/teeforge --help'
```

- `--remote-slot`、`--force` 只作用于远程 Keybox；不带 `--save-selection` 时参数只对本次生效。
- `--keybox-validate FILE --json` 只读校验格式，不联网、不安装；反馈中不要粘贴 Keybox 正文。
- `--rkp-status` 只查本地条件（SoC、厂商库、`/dev/smcinvoke`、fingerprint），不联网。`--rkp-probe` 是固件诊断命令：8 个调用变体各跑一个子进程，输出对照数据和厂商库符号表。
- 查询类命令支持 `--json`，输出 `{ok,data}` / `{ok,error}` 信封；`--json` 一次只搭配一个动作。任一步失败退出码非零。

更多参数见[项目概览](docs/project/OVERVIEW.md)。

### 设备端 RKP

用本机高通 Keymint TA 向 Google RKP 申请本机专属证书链，产物仍是标准 Keybox，后续写入与远程 Keybox 一致。

- `--rkp-status` 先确认条件，不满足时不发任何请求。
- `--rkp-issue` 只产出产物：`/data/adb/teeforge/RKP/` 下的 `keybox.xml`、`cert-chain.pem`、`meta.json`（权限 0600），不改动任何后端。
- `--keybox --source rkp` 安装本机产物，`--source remote` 走远程池，互不回退；`--rkp-delete` 删除 RKP 目录。
- 失败按阶段透传状态与原始码（如 TA 返回 `-18`）；帧类失败带 `head_hex` 与 `written`，`meta.json` 的 `frame_layout` 记录实际命中的帧布局。仅部分高通设备与固件支持。
- 签发成功只代表拿到一把 RKP 证书链，不代表通过任何外部检测。

实现移植自 [Duck-ToolBox](https://github.com/eltavine/Duck-ToolBox) 的 Device RKP Issuer（MIT），裁剪为 V2 路径。

### OMK 应用作用域

先装 OMK 再装 TeeForge；同次重启前装齐也能检测。之后才安装或重新启用 OMK 的，重装 TeeForge 刷新检测结果。OMK 活动配置尚未生成时输出 warning 并跳过，等 OMK 初始化后再运行 `--generate`。

只维护 `# BEGIN TeeForge-CD managed scoop` 与 `# END TeeForge-CD managed scoop` 之间的自动条目，手动条目和区块外配置原样保留；需要长期固定的条目放在区块外。更新前备份 `injector.toml.teeforge.bak`。OMK 作用域更新不涉及 Keybox；TeeForge 不自动重启 OMK，文件写入成功不代表 OMK 已加载。

### 配置

用户配置 `/data/adb/teeforge/config.conf`，升级默认保留。常用开关：

```ini
debug=0
blhide=1
keybox_source=remote
target_mode=auto
update_channel=stable
```

- `debug=1`：运行日志写入 `/data/adb/teeforge/logs/`。
- `blhide=0`：关闭全部 BL 弱化；`blhide_*` 分类开关同样只由 `runtime/teeforge-hide` 读取。
- `keybox_source=remote|rkp`：Keybox 来源，默认 `remote`。
- `target_mode=auto|manual`：`auto` 写入全部用户应用；`manual` 只写 `/data/adb/teeforge/target-selection.json` 中的集合，WebUI 手动勾选写这里。
- `update_channel=stable|dev`：WebUI 更新检查通道；`update_base` 默认 `https://teeforge.mcxiaochen.top/files/update`。
- `rkp_server`：默认 `https://remoteprovisioning.googleapis.com/v1`，仅调试或自建代理时修改。
- `omk_enabled` 由安装器固化（默认 0），用户配置可覆盖；`omk_injector_config` 默认 `/data/misc/keystore/omk/injector.toml`。
- 旧键 `rkp_mode`、`rkp_refresh_days`、`prop_tool`、`blhide_compact` 只为兼容解析，不再生效，也不再生成。

`/data/adb/teeforge/sys.conf` 由安装器维护，不要手动编辑。修改后的配置在下一次命令或开机服务执行时生效。数据流与安全边界见[当前架构](docs/project/ARCHITECTURE.md)。

## 更新、卸载与恢复

更新：从稳定 Release 或管理器的稳定入口安装新版。已有 `config.conf` 默认保留，其他运行时数据清理；安装时选“全部清除”会删除 `/data/adb/teeforge/` 下全部配置、日志和缓存。

卸载：在管理器卸载后重启。卸载脚本删除整个 `/data/adb/teeforge/`（含配置、日志、本地 Keybox 副本），需要保留先自行备份。目标模块的 `target.txt`、`keybox.xml`、teesim 配置和 OMK 的 `injector.toml` 不会删除，确认不再需要时自行备份后清理。

异常恢复：BL 弱化行为异常但能开机时，把 `blhide` 设为 `0` 再重启；无法正常启动时用管理器的安全模式或模块禁用机制进入系统后卸载 TeeForge-CD。官方说明：[Magisk 安全模式](https://topjohnwu.github.io/Magisk/faq.html) · [KernelSU Bootloop 恢复](https://kernelsu.org/guide/rescue-from-bootloop.html) · [APatch Bootloop 恢复](https://apatch.dev/rescue-bootloop.html)

## 故障排查与反馈

- 安装提示校验失败：删除 ZIP，从 Release 重新下载；不要修改或重新打包。
- ABI 不支持或 ELF 不匹配：设备主 ABI 不在支持列表内；不要用其他架构二进制替换。
- 目标配置没更新：确认后端是 `/data/adb/tricky_store/target.txt` 或 `/data/adb/teesim/config.json`，手动跑 `--generate` 看真实退出码；没有任何兼容后端时命令成功但只记 warning。
- Keybox 同步失败：检查网络和命令输出；失败会保留已有文件，非空响应不等于有效结果。
- 设备端 RKP 没有生效：先跑 `--rkp-status`；日志和 WebUI 会给出失败阶段、原始码与建议。
- 报“无法识别 TA 响应帧”或“TA 未写入响应缓冲区”：看 `--rkp-issue --json` 的 `error.raw.head_hex`（响应头 64 字节）和 `written`（最后一个非零字节位置）。全 `00` 开头且 `written=0` 是缓冲区未被写入（ABI 或参数约定不符）；开头是 `82`、`a2`、`d2` 等 CBOR 首字节说明厂商返回裸 payload；前 4 字节是负值（如 `eeffffff`）说明 TA 返回了 KeyMint 错误码。签发成功时 `meta.json.frame_layout` 记录命中布局，可对比同型号固件。
- `--rkp-probe` 判读：`status` 是 `send_cmd` 返回值，`written` 是响应写入长度，`request_changed` 表示请求缓冲区是否被厂商改写。常见对照：

| 观察 | 结论 |
|---|---|
| 只有 `baseline` 空、`zero-buffer` 有数据 | 前 4 字节哨兵干扰厂商逻辑，默认初始化应改为全 0 |
| 全部变体 `written=0` 且 `request_changed=false` | 请求未到达 TA，先查 SELinux 拒绝与 TA 服务状态 |
| 某变体 `request_changed=true` | out 参数落在请求缓冲区上，参数顺序需要调整 |
| `level-0` 有数据 | 该固件要求 `security_level = 0` |
| `length-prefix` 有数据 | 请求需要长度前缀 |
| `object-0x80` / `object-0x100` 有数据 | 厂商对象比 0x40 大，`baseline` 构造写越界 |
| 多个变体 `crashed: true` | 厂商对象布局与该固件不匹配 |
| `library_symbols` 有 `init` / `initialize` / `connect` 入口 | 调用前需要先完成初始化 |

需要文件日志时把 `debug` 设为 `1`，复现后查看 `/data/adb/teeforge/logs/`，分享前移除设备标识等隐私内容。

提交 Issue 请提供：TeeForge-CD 版本与下载渠道；Android 版本、ABI、Root 实现及版本；相关目标模块名称及版本；复现步骤、期望与实际结果；安装输出、命令退出码和脱敏日志。[提交 Issue](https://github.com/mcxiaochenn/TeeForge-CD/issues)

## 开发文档

- [项目概览与 CLI](docs/project/OVERVIEW.md)
- [技术架构](docs/project/ARCHITECTURE.md)
- [构建、测试与发布](docs/development/BUILD_AND_RELEASE.md)
- [安装与设备验收](docs/operations/INSTALLATION.md)
- [路线图](docs/maintenance/ROADMAP.md)

## License 与致谢

TeeForge-CD 采用 [GNU GPL v3 或更高版本](LICENSE)发布。

模块接口、兼容目标或基础能力来自：

- [Magisk](https://github.com/topjohnwu/Magisk)、[KernelSU](https://github.com/tiann/KernelSU)、[APatch](https://github.com/bmax121/APatch)
- [Tricky Store](https://github.com/5ec1cff/TrickyStore)、[TEESimulator](https://github.com/JingMatrix/TEESimulator)、[TEESimulator-RS](https://github.com/Enginex0/TEESimulator-RS)、[Oh My Keymint](https://github.com/qwq233/OhMyKeymint)
- [Duck-ToolBox](https://github.com/eltavine/Duck-ToolBox)：设备端 RKP 签发流程、CSR_V2 帧格式与证书链校验（MIT，见 `module/licenses/duck-toolbox.txt`）；WebUI 视觉分层参考其设计思路，代码与样式为独立实现

第三方项目名称仅用于说明依赖与兼容关系，不代表官方合作或背书。本 README 描述当前 `master` 的用户可见行为，正式包内容以 Release 说明和安装输出为准。
