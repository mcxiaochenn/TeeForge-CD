# 构建、测试与发布

## 环境

- Rust toolchain：读取 `rust-toolchain.toml`。
- Android NDK：API 24 linker，建议 NDK r27+。
- Node.js：用于 `webroot/` 的 Astro 构建。
- Android ABI 构建需要设置 `NDK` 或 `ANDROID_NDK_HOME`。

## 常用入口

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- build
cargo run --locked -p xtask -- package
cargo run --locked -p xtask -- verify
```

`build.sh` 和 `package.sh` 是兼容入口，默认委托给 xtask；迁移期只有显式设置 `TEEFORGE_LEGACY_C=1` 才运行旧 C 流程。

## xtask 阶段

1. 按四 ABI 配置 Cargo target 和 NDK clang linker。
2. 以 release 配置构建并复制到 `out/bin/<abi>/teeforge`。
3. 用 ELF machine 和 `llvm-readelf` 检查架构及动态依赖。
4. 在锁定依赖环境中构建 WebUI。
5. 在暂存目录注入 `version`、`versionCode` 和 dev 渠道信息。
6. 生成 `.sha256`，创建 ZIP，复核清单存在并执行体积门禁。

限制：单个 stripped 二进制不超过 1.5 MiB，完整 ZIP 不超过 6 MiB；依赖只允许 Android `libc.so` 和 `libdl.so`。

## 版本与渠道

- 稳定版本手写来源是 `crates/teeforge/Cargo.toml`。
- CI 可通过 `VERSION` 和 `VERSION_CODE` 注入 dev/release 渠道版本。
- 稳定和 dev 的 `updateJson`、ZIP 路径和 page 分支目录分开维护。
- `module/module.prop` 的构建注入发生在暂存目录，不修改源码树。

## GitHub Actions

- `ci.yml`：格式、Clippy、主机测试、四 ABI 打包、ELF/清单/体积验证。
- `dev.yml`：master push 后构建 dev 包、上传 Artifact，并更新 page 分支 dev 文件。
- `release.yml`：标签或手动版本触发正式 Release。
- `keybox-sync.yml`：按计划同步上游 Keybox 到 page 分支。

CI 成功只证明自动化流程和产物门禁通过，不代替设备安装、升级保留配置或运行行为验收。push、tag、Release 和 page 分支写入都需要明确授权。
