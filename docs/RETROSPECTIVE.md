# 反思小结 Retrospective

**日期 Date**: 2026-07-14
**范围 Scope**: v0.1.0 阶段性 Bug 修复
**用户指令 User instruction**: "反思总结经验与不足，写小结放到docs目录下"

**影响的文件 Affected files**:
- `.github/workflows/dev.yml`, `.github/workflows/release.yml` — CI 版本注入
- `native/src/keybox.c` — 解密流程（调用方: `main.c` → `keybox_fetch()`）
- `native/src/blhide.c` — 弱隐 BL（调用方: `main.c` → `bl_hide()`）
- `module/customize.sh` — 模块安装脚本（Magisk 调用）
- `module/service.sh` — 开机服务（Magisk service.d 调用）

---

## 修复内容 Summary

本次修复了 4 个 Bug 和 1 项改进，涉及 CI 工作流、keybox 解密、弱隐 BL 权限、日志合并。

---

## 经验 Lessons Learned

### 1. 设备环境不能假设 Assume nothing about device environment

**问题**: keybox.c 使用 `openssl dgst -sha256`，但 Android 设备没有 openssl。
**教训**: Android 原生环境极简，只有 toybox 提供的基础工具。涉及加密/哈希的 shell 命令必须先在目标设备上验证可用性。`sha256sum` 是 toybox 自带的，比 `openssl` 更可靠。
**原则**: 写 shell 命令前，先 `adb shell` 确认命令存在。

### 2. 文件权限在模块安装中会被重置 Permissions get reset during module install

**问题**: `customize.sh` 中 `set_perm_recursive $MODPATH 0 0 0755 0644` 会把所有文件重置为 644，覆盖之前的 `chmod 755`。
**教训**: Magisk 的 `set_perm_recursive` 必须在所有 `chmod` 之后调用，且需要对特殊文件（如可执行二进制）再单独 `set_perm`。执行顺序是关键。
**原则**: `set_perm_recursive` 放最后，特殊权限在其后单独设置。

### 3. 上游数据格式需要溯源 Trace upstream data format

**问题**: 假设上游 Megatron 数据是简单的多层 base64，实际是 10 层 base64 + hex + ROT13 三层编码。
**教训**: 不能凭猜测解密。必须查看上游项目的实际解密代码（Integrity-Box 的 `key.sh`），理解完整的编码流程后再实现。"看起来像 base64" 不代表"只是 base64"。
**原则**: 解密逻辑必须有参考实现对照验证，不能靠试错。

### 4. CI 版本号需要多处同步 Version must be synced in multiple places

**问题**: CI 只更新了 `module.prop`，没有更新 `teeforge.h` 中的 `TEEFORGE_VERSION`，导致二进制的 `--version` 输出始终是硬编码的 "0.1.0"。
**教训**: 版本号散落在多处（module.prop、teeforge.h、build.sh）时，CI 必须全部覆盖。理想方案是单一来源（single source of truth），但当前架构下需要在 workflow 中多加一行 sed。
**原则**: 新增版本号引用点时，同步更新所有 CI workflow。

### 5. system() 的退出码含义 Exit code semantics of system()

**问题**: resetprop-rs 返回 code 32256，一开始误以为是 resetprop 本身的错误。
**教训**: `system()` 返回值是 `waitpid` 格式（exit code << 8 | signal）。32256 = 126 << 8，即 exit code 126 = "Command found but not executable"。理解这个编码可以快速定位权限问题。
**原则**: 遇到 `system()` 返回非零值时，先右移 8 位取 exit code。

### 6. 日志路径统一的重要性 Importance of unified log paths

**问题**: `service.sh` 写日志到 `/data/adb/teeforge/teeforge.log`，C 代码写到 `/data/adb/teeforge/logs/teeforge_YYYYMMDD.log`，用户需要查看两个位置。
**教训**: 日志输出应该由单一模块管理。既然 C 代码已有完善的日志系统（按日期分文件、自动清理），shell 脚本不应该另起炉灶。
**原则**: shell 脚本只负责调用二进制，不自己写日志。

---

## 不足 Shortcomings

### 1. 没有自动化测试 No automated testing

所有验证都是手动 `adb shell` 逐条执行。如果有一个本地 mock 测试或设备端集成测试，可以更快发现问题。

### 2. 解密流程缺乏文档 Decryption pipeline was undocumented

上游 Megatron 的编码流程（10x base64 → hex → ROT13）没有任何文档说明，全靠阅读 Integrity-Box 的 shell 脚本逆向。应该在 keybox.c 中添加详细注释说明完整流程。

### 3. 错误信息不够详细 Insufficient error messages

keybox 解密失败时只输出 "Invalid data"，没有说明在哪一步失败、中间数据大小是多少。后来加了 DEBUG 日志才定位问题。生产代码应该在关键步骤记录更多上下文。

### 4. 没有考虑设备差异 No device-specific testing

只在一台设备（arm64-v8a, KSU）上测试。不同设备的 toybox 版本、busybox 可用性、resetprop 路径可能不同。

---

## 改进建议 Improvement Ideas

1. **添加 `--test` 命令**: 在设备上运行自检（检查工具可用性、权限、网络连通性）
2. **keybox.c 添加错误阶段标记**: 每个解密步骤失败时输出具体阶段名
3. **CI 添加版本一致性检查**: 对比 module.prop 和 teeforge.h 中的版本号
4. **建立设备测试矩阵**: 至少覆盖 arm64 + armeabi-v7a、Magisk + KSU
