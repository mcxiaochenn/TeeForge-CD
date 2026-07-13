#!/system/bin/sh
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
$MODDIR/teeforge --config "$CONFIG" --keybox 2>&1 | while IFS= read -r line; do
    echo "  $line"
done
echo ""

# 更新 target.txt
echo "[2/2] 更新 target.txt [Updating target.txt]..."
echo ""
$MODDIR/teeforge --config "$CONFIG" --generate 2>&1 | while IFS= read -r line; do
    echo "  $line"
done
echo ""

echo "========================================"
echo " 完成 [Done] - 无需重启 [No reboot needed]"
echo "========================================"
