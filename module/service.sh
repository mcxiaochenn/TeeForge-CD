#!/system/bin/sh
# TeeForge-CD Service Script
# 开机服务脚本 [Boot service script]
# Runs at boot via Magisk service.d

MODDIR=${0%/*}
CONFIG="/data/adb/teeforge/config.conf"

# 等待系统启动完成 Wait for system to boot
while [ "$(getprop sys.boot_completed)" != "1" ]; do
  sleep 1
done

sleep 5

# 更新模块描述 Update module description
"$MODDIR/teeforge" --config "$CONFIG" --update-desc

# 执行弱隐 BL Execute weak bootloader hiding
"$MODDIR/teeforge" --config "$CONFIG" --hide-bl

# 生成 target.txt Generate target.txt
"$MODDIR/teeforge" --config "$CONFIG" --generate
