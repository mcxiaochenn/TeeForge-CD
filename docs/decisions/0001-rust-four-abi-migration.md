# ADR-0001：Rust 四架构迁移

## 背景

旧核心是单一 AArch64 C ELF，安装器虽然处理了部分 resetprop-rs 架构，但主程序实际不具备四 ABI 产物。构建、错误处理和跨平台打包逻辑也分散在脚本中。

## 决策

使用 Rust 作为默认原生核心，建立 Cargo workspace 和跨平台 `xtask`，在同一个模块 ZIP 中放置四个 ABI ELF，安装时根据设备 ABI 选择一个。

映射为：

```text
arm64-v8a    → aarch64-linux-android
armeabi-v7a  → armv7-linux-androideabi
x86          → i686-linux-android
x86_64       → x86_64-linux-android
```

## 选择原因

- Rust 官方提供四种 Android target。
- 设备端可保持同步标准库实现，不引入异步运行时和大型 CLI 框架。
- `Result`、原子文件和参数化进程调用能减少 C 版本的错误传播风险。
- Go 的 Android 非 arm64 链接仍需额外接入 NDK，运行时体积也不适合该启动工具。

## 兼容与回退

- 保留原 CLI 参数、配置路径、模块 ID 和 `/data/adb/teeforge/` 数据目录。
- `build.sh`、`package.sh` 继续作为兼容入口。
- `TEEFORGE_LEGACY_C=1` 在迁移期保留旧 C 构建回退。
- `native/` 只有在授权真机验收完成后才删除，不长期双维护。

## 验证门槛

- fmt、Clippy、主机契约测试全部通过。
- 四 ABI 构建、ELF machine、动态依赖、SHA256 和包体积均通过。
- 至少完成 arm64-v8a 真机安装和运行验收，其余 ABI 用对应环境验证。

## 当前状态

Rust 已是默认实现，自动化构建与打包已验证；真机验收仍是删除旧 C 实现前的必要门槛。动态待办见 [`../maintenance/ROADMAP.md`](../maintenance/ROADMAP.md)。
