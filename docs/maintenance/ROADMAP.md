# 当前路线图

## 当前阶段

Rust 四 ABI 核心和跨平台打包链已作为默认实现；公开文档正在完成统一入口和按需读取治理。

## 已完成

- Rust CLI、四 ABI 构建、xtask 打包和 CI 门禁。
- Keybox 有界下载、严格解码、内容校验和原子回滚。
- 多后端目标配置适配：Tricky Store/TEESimulator-RS `target.txt` 受管区块、TEESimulator `teeforge` profile，以及 Keybox 多目录事务同步。
- 目标更新终态日志、受管区块幂等回归，以及包管理命令的并发读取、30 秒超时和输出边界测试。
- 下载输入在本地 Release 二进制中完成真实复现，并补齐最终文本空白兼容回归。
- Shell 源码、暂存目录和模块 ZIP 的 LF 换行门禁。
- 安装时 ABI 选择和 ELF 校验；全架构统一 standard resetprop，Rust、旧 C、安装器和预置资源移除 rs 支持。
- OMK 仅应用作用域适配：安装检测固化、TOML 受管区块、原始快照、元数据保留和专用备份；旧配置键兼容忽略。
- 2026-09-14 本地验证：Windows 工作区 59 项、Linux 工作区 60 项测试通过；Linux 包含 UID/GID、0600 权限及失败临时文件清理验证。格式、Clippy、旧 C 的 Linux 语法检查与 Android arm64 编译链接通过；设备行为仍列为待验收。
- 同轮四 ABI/WebUI 打包通过，ZIP 为 1,365,361 字节；21 项归档校验和、5 个 Shell 文件 LF 检查通过，归档无 resetprop-rs 路径。
- Dev 配置模板同步删除废弃的 blhide_compact；用户将使用 Dev 包进行后续真机验收，自动化通过不替代设备结论。
- WebUI、Shell 生命周期和公开架构文档同步到 Rust 实现。
- 设备端 RKP 签发（移植自 Duck-ToolBox，MIT）：高通/arm64 前置检查、CSR_V2 帧、TA 隔离子进程、Google RKP 交互、证书链校验、keybox 生成与 `rkp_mode` 回退链；主机侧 90 项测试与四 ABI 编译通过。
- WebUI v2：Astro → Vite + Vue 3 + hash 路由 + Tailwind 3.4；五个页面（Home / 目标作用域 / Keybox 配置 / 本机 RKP / 模块与更新）、偏好浮层、统一 `--json` 契约。
- 真机问题修复（2026-10-18）：RKP 帧解析改为「原始缓冲区 + 业务判定布局」并删除重复解帧，worker 错误结构化跨进程；WebUI 消除负外边距导致的横向溢出与 `100vh` 导致的纵向溢出；版本号统一去掉 `v` 前缀。
- 二进制拆分：RKP 独立为 `--rkp-issue/-status/-artifacts/-delete`，产物写入 `/data/adb/teeforge/RKP/`；`--keybox` 恢复为纯远程，新增 `--source`。
- 目标作用域手动模式：`target_mode` + `target-selection.json`，新增 `--target-list/-apply/-target-mode`。
- 更新：`--self-update check/install`，自建 CDN 清单 + sha256/大小/ZIP 头校验，按 root 管理器选择安装器。
- Action 重做：音量键两级菜单（音量下切换、音量上确认、15 秒取消），全程落盘 `logs/action-*.log`；包列表超时 12s + `pm` 兜底 + 3s 心跳。
- 主机验证：99 项测试、Clippy/格式通过、Vite 构建通过；真机验收待执行。

## 2026-10-19：Keybox 三槽位与安装校验

- 已实现远程池二级三选一、强制重新获取并重写、安装成功后保存选择；默认槽位兼容旧客户端。
- 同步发布与模块共用格式校验，安装逐目标回读，配置最后保存，失败回滚目标与备份。主机合成数据覆盖三个槽位全链路；浏览器模拟宿主验证普通/强制、失败、保存重载、忙碌锁定及 RKP 切换。
- 本地格式、Clippy、工作区测试、发布门禁、WebUI 构建和四 ABI 打包/产物验证通过；第一阶段远程 CI、Dev Build 与三槽位同步通过，线上三个槽位回读与格式检查通过。
- 手机安装后格式损坏尚未取得样本，主机未复现；不能将校验增强写成真机故障已定位或修复。实际后端与设备验收仍待授权。

## 2026-10-22：v0.7.0 正式版发布范围

- 汇总自 v0.6.0 以来的 Rust 四 ABI 核心、WebUI v2、多后端作用域适配、RKP 独立命令、模块更新和音量键 Action 交互。
- 源码与模块模板版本统一为 `0.7.0`；发布产物的 `versionCode` 继续由 CI 按提交数注入。
- 本地格式、Clippy、125 项 Rust/xtask 测试、2 项发布门禁测试、WebUI 构建及模拟宿主回归通过；四 ABI 产物由远程 CI 验证。
- 正式版发布不代表下列真机待验收项已完成；不包含新增设备连接、安装或调试授权。

## 待验证

- arm64-v8a 主力设备安装、升级保留配置、音量键选择和开机服务。
- 设备端 RKP：`--rkp-status` 判定、`--rkp-issue` 真实签发、`-18` 等 TA 错误码的透传与解释、产物落盘/删除、`--keybox --source rkp` 安装；小米 17 Pro 等社区成功机型优先。
- RKP 帧布局：小米 17 Pro 实测 `--rkp-issue` 在旧版报 `invalid payload length`，需确认 `decode_frame` 命中的 `frame_layout`（预期为 `bare` 或 `status`），并提供 `error.raw.head_hex` 用于比对。
- RKP 空响应：小米 17 Pro 在修复版上 `send_cmd` 返回 0 但缓冲区零字节（`written: 0`），需用 `--rkp-probe --json` 确认是缓冲区初始化、`security_level` 还是命令本身的问题；判读表见 README。
- WebUI v2：五个页面在真机 WebView 下的布局与滚动、`ksu.spawn` 流式日志、`ksu.getPackagesInfo` / `ksu://icon` 缺失时的降级、手动选择重启后保持。
- 视口：小米 17 Pro 上确认四边无溢出、首页不满一屏时不能上下拖动、双指缩放无效、吸顶与吸底正常。
- Action：音量键在 Action 上下文可读、三个分支（RKP / 远程池 / 作用域）、超时取消不写文件、日志文件完整。
- 更新：清单 sha256 校验、`ksud`/`apd`/`magisk` 安装器识别、无校验和时只保存到 Download。
- ARMv7、x86、x86_64 对应环境的安装和运行行为。
- Tricky Store：在真机上对带注释、`!`、`?` 的现有 `target.txt` 连续执行两次 `--generate`，确认本地自动化结论与设备行为一致。
- TEESimulator-RS：含 `[custom.xml]` 分段的 `target.txt` 归属保持与新增包更新。
- TEESimulator：多 profile、`package@N`、`uid:N`、未知字段、FileObserver 热加载及 Keybox 路径。
- 双目录共存、第二目标写失败回滚、BL-only 无后端 warning、升级迁移和三种 Keybox 组合。
- OMK 的安装顺序、UID/GID、0600 权限、SELinux 标签、配置热加载与应用路由；不包含 OMK Keybox 或服务配置。
- 真机结果与自动化结果分开记录。

## 阻塞项

- 未取得用户明确设备授权前，不执行推送、安装或主力机调试。
- 旧 `native/` C 实现暂不删除，等待真机验收完成。

## 下一步

1. 完成文档入口重组并验证按需阅读路径。
2. 取得授权后执行分层设备验收。
3. 验收通过后单独评估删除旧 C 实现、提交稳定版本和发布流程。
4. 在 CDN 更新清单中补 `sha256` 与 `size`，否则 WebUI 只能保存安装包、无法自动安装。

最近更新：2026-10-22
