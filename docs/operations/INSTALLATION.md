# 安装与设备验收

## 安装顺序

`customize.sh` 的主要顺序：

1. 读取并执行 `.sha256` 完整性校验；清单缺失或任一文件异常立即中止。
2. 读取 `ro.product.cpu.abi`，选择 `bin/<abi>/teeforge`。
3. 校验 ELF machine，复制为模块根目录的 `teeforge`，删除其他 ABI。
4. 检测 Root 方法和版本。
5. 询问是否保留已有 `/data/adb/teeforge/` 配置，超时默认保留。
6. 全架构使用 standard resetprop；检测 OMK 模块并固化启用状态，待更新目录优先，禁用或待卸载时关闭。
7. 生成 `sys.conf`，只在不存在时生成用户 `config.conf`。
8. 在所有递归权限设置之后重新授予 teeforge 二进制执行权限。

不支持的 ABI、缺少对应 ELF 或 machine 不匹配时不得使用其他架构兜底。

## 配置和升级

- `config.conf` 是用户配置，升级时保留。
- `sys.conf` 是系统生成配置，不应手动编辑；包含 `omk_enabled` 和 `omk_injector_config`，用户配置仍可覆盖。
- 后来安装或启用 OMK，需要重装 TeeForge 刷新安装检测。活动配置缺失只跳过 OMK，不创建其配置。
- 旧 `prop_tool`、`blhide_compact` 键保留但不再生效。
- 音量键清理选择只影响 `/data/adb/teeforge/` 数据，不改变模块包本身。
- Keybox 和 target 更新均应使用原子替换，失败时保留旧文件。

## 验收分层

自动化或模拟器可以验证：

- CLI 参数、配置优先级和非零失败码。
- 四架构 ELF、包完整性和安装分支。
- 下载后备、严格解码和错误聚合。

真机还必须单独验证：

- 至少一台 arm64-v8a 主力设备的安装和升级。
- 配置保留、音量键选择、开机服务和 target/keybox 实际结果。
- OMK 的普通文件 UID/GID、0600 权限、SELinux 标签、热加载和应用实际路由；验证先装 OMK 未重启、升级、禁用再启用，以及三种目标共存。
- OMK 仅支持作用域更新，不应验证或宣称其 Keybox 已被 TeeForge 同步。
- 有条件时补充 ARMv7、x86、x86_64 环境。

连接、推送、安装或修改用户主力设备之前，必须取得明确授权；最终报告要区分自动化结果和真机结果。
