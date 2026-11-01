#!/system/bin/sh
# 必须保持 LF 换行，供 Android sh 直接执行 Must remain LF-only
# TeeForge-CD Service Script
# 开机服务脚本 [Boot service script]
# Runs at boot via Magisk service.d

MODDIR=${0%/*}
CONFIG="/data/adb/teeforge/config.conf"
LOG_DIR="/data/adb/teeforge/logs"
TF="$MODDIR/runtime/teeforge"
HIDE="$MODDIR/runtime/teeforge-hide"

# 等待系统启动完成 Wait for system to boot
while [ "$(getprop sys.boot_completed)" != "1" ]; do
  sleep 1
done

sleep 5

# 更新模块描述 Update module description
"$TF" --config "$CONFIG" --update-desc

# 弱隐二次加固：post-fs-data 已写过一次，这里防止厂商脚本在启动后期写回。
# Second weak-hiding pass; OEM scripts may restore values late in boot.
mkdir -p "$LOG_DIR" 2>/dev/null
"$HIDE" --config "$CONFIG" --stage late >>"$LOG_DIR/blhide.log" 2>&1

# 更新目标应用配置 Update target app configuration
"$TF" --config "$CONFIG" --generate
