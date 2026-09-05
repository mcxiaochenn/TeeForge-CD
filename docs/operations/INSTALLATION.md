# 安装与设备验收

## 安装顺序

`customize.sh` 的主要顺序：

1. 读取并执行 `.sha256` 完整性校验；清单缺失或任一文件异常立即中止。
2. 读取 `ro.product.cpu.abi`，选择 `bin/<abi>/teeforge`。
3. 校验 ELF machine，复制为模块根目录的 `teeforge`，删除其他 ABI。
4. 检测 Root 方法和版本。
5. 询问是否保留已有 `/data/adb/teeforge/` 配置，超时默认保留。
6. ARM 设备询问 standard 或 resetprop-rs；x86/x86_64 固定 standard。
7. 生成 `sys.conf`，只在不存在时生成用户 `config.conf`。
8. 在所有递归权限设置之后重新授予二进制和 resetprop-rs 执行权限。

不支持的 ABI、缺少对应 ELF 或 machine 不匹配时不得使用其他架构兜底。

## 配置和升级

- `config.conf` 是用户配置，升级时保留。
- `sys.conf` 是系统生成配置，不应手动编辑。
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
- 有条件时补充 ARMv7、x86、x86_64 环境。

连接、推送、安装或修改用户主力设备之前，必须取得明确授权；最终报告要区分自动化结果和真机结果。
