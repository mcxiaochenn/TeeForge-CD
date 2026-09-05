# 历史快照：Keybox Sync 工作流修复报告

> 本文记录历史修复过程，不代表当前同步状态。当前构建和同步规则见 [`../development/BUILD_AND_RELEASE.md`](../development/BUILD_AND_RELEASE.md)，安全边界见 [`../project/ARCHITECTURE.md`](../project/ARCHITECTURE.md)。

## 问题描述

GitHub Actions 的 Keybox Sync 工作流持续失败，最近5次运行中有4次失败（最近一次失败：2026-08-01T03:15:41Z）。

**错误信息：**
```
curl: (22) The requested URL returned error: 404
##[error]Process completed with exit code 22.
```

## 根本原因

上游仓库 `MeowDump/Integrity-Box` 的 `keybox/` 目录结构发生了变更：

- **仍然存在：** `key-status` ✓
- **已移除：** `key-control` ✗、`key-hash` ✗

工作流中仍然尝试下载已不存在的文件，导致 404 错误。

## 验证过程

```bash
# 测试上游 URL
curl -fsSL "https://raw.githubusercontent.com/MeowDump/Integrity-Box/refs/heads/main/keybox/key-control" -o /dev/null
# 结果：curl: (22) The requested URL returned error: 404

curl -fsSL "https://raw.githubusercontent.com/MeowDump/Integrity-Box/refs/heads/main/keybox/key-hash" -o /dev/null
# 结果：curl: (22) The requested URL returned error: 404

# 查看上游目录结构
curl -sL "https://api.github.com/repos/MeowDump/Integrity-Box/contents/keybox"
# 结果：只返回 key-status 一个文件
```

## 解决方案

### 1. 修改 `.github/workflows/keybox-sync.yml`

**移除的行：**
```yaml
CONTROL_URL="https://raw.githubusercontent.com/MeowDump/Integrity-Box/refs/heads/main/keybox/key-control"
HASH_URL="https://raw.githubusercontent.com/MeowDump/Integrity-Box/refs/heads/main/keybox/key-hash"
curl -fsSL "$CONTROL_URL" -o /tmp/key-control --retry 3
curl -fsSL "$HASH_URL" -o /tmp/key-hash --retry 3
```

**修改的步骤：**
- `Fetch upstream keybox`：移除 `key-control` 和 `key-hash` 的下载
- `Build keybox files`：移除 `cp /tmp/key-control` 和 `cp /tmp/key-hash`
- `Sync status to master`：移除 `cp /tmp/key-control keybox/` 和 `cp /tmp/key-hash keybox/`

### 2. 清理本地仓库

```bash
rm -f keybox/key-control keybox/key-hash
```

**修改后的工作流结构：**
```
Fetch upstream keybox
  ├── 下载 Megatron（加密数据）
  └── 下载 key-status（状态文件）

Build keybox files
  ├── 复制 key-status
  ├── 生成 upstream_hash
  ├── 生成 month
  └── 加密并生成混淆文件

Sync status to master
  ├── 同步 key-status
  ├── 同步 upstream_hash
  └── 同步 month
```

## 测试结果

**修复后的工作流运行：**
- Run ID: `30690675357`
- 状态：`success`
- 结论：`success`
- 耗时：13秒

**所有步骤通过：**
- ✓ Fetch upstream keybox
- ✓ Build keybox files
- ✓ Force push to page branch
- ✓ Sync status to master

## 影响分析

### 功能影响
- **无负面影响**：`key-control` 和 `key-hash` 文件在本项目中未被任何代码引用
- **同步功能完整**：核心同步逻辑（Megatron 加密、keybox 生成、CDN 部署）不受影响

### 文件变更
- 删除：`keybox/key-control`（内容：`VERIFY=TRUE`）
- 删除：`keybox/key-hash`（内容：SHA256 哈希值）
- 保留：`keybox/key-status`、`keybox/upstream_hash`、`keybox/month`

## 建议

1. **定期监控上游变更**：上游仓库可能继续调整目录结构
2. **添加错误处理**：可以在工作流中添加更详细的错误日志
3. **文档更新**：更新 README 中关于 keybox 同步的说明

## 相关提交

- Commit: `e5354b3`
- Message: `fix(ci): remove deprecated key-control and key-hash from keybox sync`
- Date: 2026-08-01

---

**报告生成时间：** 2026-08-01 15:55 UTC+8
