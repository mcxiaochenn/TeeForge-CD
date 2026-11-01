#!/system/bin/sh
# 必须保持 LF 换行，供 Android sh 直接执行 Must remain LF-only
# TeeForge-CD Post-FS-Data Script
# 早期弱隐 [Early weak bootloader hiding]
#
# 在 zygote 启动前覆盖属性，供开机早期读取属性的检测使用。
# 本阶段串行阻塞开机，二进制内部不做任何等待，只做一次属性写入。
# Overrides properties before zygote starts so early readers see the spoofed values.
# This stage blocks boot, so the binary never waits; it writes once.

MODDIR=${0%/*}
CONFIG="/data/adb/teeforge/config.conf"
LOG_DIR="/data/adb/teeforge/logs"
HIDE="$MODDIR/runtime/teeforge-hide"

[ -x "$HIDE" ] || exit 0

mkdir -p "$LOG_DIR" 2>/dev/null
"$HIDE" --config "$CONFIG" --stage early >"$LOG_DIR/blhide.log" 2>&1

exit 0
