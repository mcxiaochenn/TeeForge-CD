# Rust 四架构迁移状态

## 当前默认实现

- `build.sh` 与 `package.sh` 默认调用 Cargo `xtask`。
- `TEEFORGE_LEGACY_C=1` 可在迁移验收期间运行旧 C 构建流程。
- 旧 `native/` 只用于行为对照；完成授权真机验收后删除，不长期双维护。

## 验证门槛

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
3. `cargo test --workspace --locked`
4. `cargo run --locked -p xtask -- package`
5. 复核四个 ELF machine、仅依赖 Android `libc.so`/`libdl.so`、ZIP 内 `.sha256` 全部通过

## 尚需人工授权的验收

- arm64-v8a 主力机安装、升级保留配置、音量键选择和开机服务。
- 有条件时在 ARMv7 真机以及 x86/x86_64 Android 环境验证。
- 自动化通过不替代上述设备验收。
