#!/system/bin/sh
# TeeForge-CD Service Script
# 开机服务脚本 [Boot service script]
# Runs at boot via Magisk service.d

MODDIR=${0%/*}

# 检测 resetprop-rs Detect resetprop-rs
RESETPROP_RS=""
for f in "$MODDIR/resetprop-rs"/resetprop-*; do
    [ -x "$f" ] && RESETPROP_RS="$f" && break
done

if [ -n "$RESETPROP_RS" ]; then
    export RESETPROP_RS
fi

# 等待系统启动完成 Wait for system to boot
while [ "$(getprop sys.boot_completed)" != "1" ]; do
  sleep 1
done

sleep 5

# 执行弱隐 BL Execute weak bootloader hiding
$MODDIR/teeforge --hide-bl

# 生成 target.txt Generate target.txt
$MODDIR/teeforge --generate

# 记录完成 Log completion
echo "[TeeForge-CD] 开机服务完成 [Boot service completed]" >> /data/adb/teeforge/teeforge.log
