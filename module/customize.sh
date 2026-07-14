#!/system/bin/sh
# TeeForge-CD Installation Script
# 安装脚本 [Installation script]

ui_print " "
ui_print "  Installing TeeForge-CD / 正在安装"
ui_print " "

TEEFORGE_DIR="/data/adb/teeforge"
CONFIG_FILE="$TEEFORGE_DIR/config.conf"

# 先设置权限（音量键检测需要执行二进制）
# Set permissions first (volume key detection needs binary to be executable)
chmod 755 $MODPATH/teeforge

# 设置 resetprop-rs 权限
for f in "$MODPATH/resetprop-rs"/resetprop-*; do
    [ -f "$f" ] && chmod 755 "$f"
done

# 检查已有安装 Check existing installation
if [ -d "$TEEFORGE_DIR" ]; then
    if [ -f "$CONFIG_FILE" ]; then
        ui_print "  检测到已有配置 [Existing config detected]"
        ui_print "  音量+ = 保留配置 [Volume+ = Keep config]"
        ui_print "  音量- = 全部清除 [Volume- = Clean all]"
        ui_print "  10秒超时自动保留 [10s timeout, keep by default]"
        ui_print ""

        RESULT=$($MODPATH/teeforge --volume 10)
        if [ "$RESULT" = "0" ]; then
            ui_print "  清除所有数据 [Cleaning all data]"
            rm -rf "$TEEFORGE_DIR"
        else
            ui_print "  保留配置 [Keeping config]"
            find "$TEEFORGE_DIR" -type f ! -name "config.conf" -delete 2>/dev/null
            find "$TEEFORGE_DIR" -type d -empty -delete 2>/dev/null
        fi
    else
        ui_print "  无配置文件，清除目录 [No config, cleaning dir]"
        rm -rf "$TEEFORGE_DIR"
    fi
fi

# 创建目录 Create directories
ui_print "  创建目录 [Creating dirs]..."
mkdir -p "$TEEFORGE_DIR/keybox"
mkdir -p "$TEEFORGE_DIR/logs"

# 复制配置文件 Copy config file
if [ ! -f "$CONFIG_FILE" ]; then
    cp $MODPATH/config.conf "$CONFIG_FILE"
    ui_print "  配置已创建 [Config created]"
else
    ui_print "  配置已保留 [Config preserved]"
fi

# 检测 root 方式 Detect root method (via binary)
ROOT_RESULT=$($MODPATH/teeforge --rootdetect --config "$CONFIG_FILE" 2>/dev/null)
ROOT_METHOD=$(echo "$ROOT_RESULT" | sed -n '1p')
ROOT_VERSION=$(echo "$ROOT_RESULT" | sed -n '2p')
[ -n "$ROOT_METHOD" ] && ui_print "  Root: $ROOT_METHOD (v$ROOT_VERSION)"

# 显示预置的 resetprop-rs
ARCH=$(getprop ro.product.cpu.abi)
ui_print "  Arch: $ARCH"

# Set permissions
set_perm_recursive $MODPATH 0 0 0755 0644
set_perm $MODPATH/teeforge 0 0 0755

# resetprop-rs 需要执行权限（set_perm_recursive 会重置为 0644）
# resetprop-rs needs execute permission (set_perm_recursive resets to 0644)
for f in "$MODPATH/resetprop-rs"/resetprop-*; do
    [ -f "$f" ] && set_perm "$f" 0 0 0755
done

ui_print " "
ui_print "  Done! Reboot to activate"
ui_print "  完成！重启生效"
ui_print " "
