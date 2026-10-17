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

## 待验证

- arm64-v8a 主力设备安装、升级保留配置、音量键选择和开机服务。
- 设备端 RKP：`--rkp-status` 判定、真实签发、`-18` 等 TA 错误码、Google 服务端签发结果、`rkp_refresh_days` 复用、回退远程行为；小米 17 Pro 等社区成功机型优先。
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

最近更新：2026-10-17
