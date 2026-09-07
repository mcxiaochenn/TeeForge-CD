#!/system/bin/sh
# 必须保持 LF 换行，供 Android sh 直接执行 Must remain LF-only
# TeeForge-CD Action Script
# 热更新脚本 [Hot update script]
# 无需重启 No reboot required

MODDIR=${0%/*}
CONFIG="/data/adb/teeforge/config.conf"

echo "========================================"
echo " TeeForge-CD Action"
echo "========================================"
echo ""

# 获取 keybox
echo "[1/2] 获取 keybox [Fetching keybox]..."
echo ""
# 分别记录两个子任务，确保正常跳过与失败都能进入最终汇总。
# Track both subtasks so successful skips and failures always reach the final summary.
"$MODDIR/teeforge" --config "$CONFIG" --keybox 2>&1
KEYBOX_RET=$?
echo ""

# 更新目标应用配置
echo "[2/2] 更新目标应用配置 [Updating target app configuration]..."
echo ""
"$MODDIR/teeforge" --config "$CONFIG" --generate 2>&1
TARGET_RET=$?
echo ""

echo "========================================"
if [ "$KEYBOX_RET" -eq 0 ] && [ "$TARGET_RET" -eq 0 ]; then
    echo " 完成 [Done] - 无需重启 [No reboot needed]"
    ACTION_RET=0
else
    echo " 失败 [Failed] - keybox=$KEYBOX_RET target=$TARGET_RET"
    ACTION_RET=1
fi
echo "========================================"
exit "$ACTION_RET"
