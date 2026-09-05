# TeeForge-CD 当前架构

## 整体数据流

```text
Android boot
  → module/service.sh
  → selected ABI teeforge
  → /data/adb/teeforge/ 配置和日志
  → Tricky Store target.txt / keybox.xml
```

WebUI 通过 `ksu.spawn()` 优先使用流式输出，缺少该接口时降级为 `ksu.exec()`；两者都显式使用模块目录和 `/data/adb/teeforge/config.conf`。

## Rust 核心模块

| 模块 | 职责 |
|---|---|
| `cli` | 兼容参数解析、动作顺序和统一退出码 |
| `config` | 默认值、系统/用户配置优先级和兼容回退 |
| `target` | 调用 `cmd package list packages -f`、筛选用户应用并原子写入 |
| `keybox` | 下载后备、严格有界解码、内容标记校验和双目标回滚 |
| `blhide` | 参数数组调用 resetprop，逐项收集失败 |
| `rootdetect` | 环境变量和 `/data/adb/*` 路径检测 |
| `volume` | 动态扫描 input event，封装唯一的设备输入 unsafe 适配 |
| `atomic_file` | 同目录临时文件、同步写入和原子替换 |
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
```

加载顺序为：默认值 → `sys.conf` → 用户配置；用户配置覆盖同名系统键。缺少规范文件时，开发环境允许回退到当前目录的旧配置文件。

## Keybox 数据流

```text
自建 CDN
  → wget / curl / 管理器 busybox 后备
  → 有界下载
  → 严格解码和 AndroidAttestation 标记校验
  → 本地 keybox.xml
  → Tricky Store keybox.xml
```

两个目标采用原子写入；第二个目标失败时恢复本地旧文件。具体私密维护细节只存在于被 Git 忽略的 `docs/private/KEYBOX_CRYPTO.md`，不得复制到公开文档。

## BL 隐藏

安装时选择 `standard` 或 ARM 可用的 `resetprop-rs`，保存到 `sys.conf` 的 `prop_tool`。Rust 使用参数数组逐条调用工具，不拼接 Shell 脚本；失败项会聚合到最终错误。x86/x86_64 没有仓库内的 rs 二进制，因此安装时固定使用 standard。

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
