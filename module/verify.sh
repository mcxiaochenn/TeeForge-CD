#!/system/bin/sh
# TeeForge-CD Integrity Verification
# 文件完整性校验 [File integrity verification]
# Called by customize.sh before installation starts
#
# Exit codes:
#   0 — 校验通过 All files verified
#   1 — 校验失败 Integrity check failed
#   2 — 校验文件缺失 Checksum file missing

MODDIR=${0%/*}
CHECKSUMS="$MODDIR/.sha256"

# 检查校验文件是否存在 Check if checksum file exists
if [ ! -f "$CHECKSUMS" ]; then
    ui_print "  !! 校验文件缺失 [.sha256 missing] !!"
    ui_print "  !! 模块包可能被重新打包 Module may be repackaged"
    ui_print "  !! 跳过校验 Skipping verification"
    return 0 2>/dev/null || exit 2
fi

ui_print "  正在校验文件完整性 [Verifying file integrity]..."

FAIL_COUNT=0
TOTAL=0

# 临时关闭 set -e，校验失败不应中断脚本
# Disable set -e, verification failure shouldn't exit the script
set +e

while IFS='  ' read -r expected_hash filepath; do
    # 跳过空行和注释 Skip empty lines and comments
    [ -z "$filepath" ] && continue

    TOTAL=$((TOTAL + 1))

    # 检查文件是否存在 Check file existence
    if [ ! -f "$MODDIR/$filepath" ]; then
        ui_print "    缺失 [MISSING]: $filepath"
        FAIL_COUNT=$((FAIL_COUNT + 1))
        continue
    fi

    # 计算 SHA256 并比对 Compute SHA256 and compare
    actual_hash=$(sha256sum "$MODDIR/$filepath" | cut -d' ' -f1)
    if [ "$actual_hash" != "$expected_hash" ]; then
        ui_print "    篡改 [TAMPERED]: $filepath"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
done < "$CHECKSUMS"

# 恢复 set -e Restore set -e
set -e

# 结果汇总 Summary
if [ "$FAIL_COUNT" -eq 0 ]; then
    ui_print "  校验通过 [Verified]: $TOTAL 个文件 [files] ✓"
    return 0 2>/dev/null || exit 0
else
    ui_print "  !! 校验失败 [Verification FAILED]: $FAIL_COUNT/$TOTAL 个文件异常 [files altered] !!"
    return 1 2>/dev/null || exit 1
fi
