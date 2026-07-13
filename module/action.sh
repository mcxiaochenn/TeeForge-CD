#!/system/bin/sh
# TeeForge-CD Action Script
# 用户手动执行脚本 [User manual action script]
# 显示日志，不卡顿 Show logs, no stall

MODDIR=${0%/*}

echo "========================================"
echo " TeeForge-CD Action"
echo "========================================"
echo ""

# 检测 resetprop-rs Detect resetprop-rs
echo "[1/2] 检测 resetprop-rs [Detecting resetprop-rs]..."
RESETPROP_RS=""
for f in "$MODDIR/resetprop-rs"/resetprop-*; do
    [ -x "$f" ] && RESETPROP_RS="$f" && break
done

if [ -n "$RESETPROP_RS" ]; then
    export RESETPROP_RS
    echo "  ✓ 使用 [Using]: $RESETPROP_RS"
else
    echo "  ✗ 未找到 resetprop-rs，使用标准 resetprop"
    echo "    [Not found, using standard resetprop]"
fi
echo ""

# 生成 target.txt
echo "[2/2] 生成 target.txt [Generating target.txt]..."
echo "  扫描已安装应用 [Scanning installed apps]..."
$MODDIR/teeforge --generate 2>&1 | while IFS= read -r line; do
    echo "  $line"
done
echo ""

echo "========================================"
echo " 完成 [Done] - 重启生效 [Reboot to apply]"
echo "========================================"
