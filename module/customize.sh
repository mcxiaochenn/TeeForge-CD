#!/system/bin/sh
# TeeForge-CD Installation Script
# 安装脚本 [Installation script]

ui_print " "
ui_print "  Installing TeeForge-CD / 正在安装"
ui_print " "

TEEFORGE_DIR="/data/adb/teeforge"
CONFIG_FILE="$TEEFORGE_DIR/config.conf"

# 检查已有安装 Check existing installation
if [ -d "$TEEFORGE_DIR" ]; then
    if [ -f "$CONFIG_FILE" ]; then
        ui_print "  检测到已有配置 [Existing config detected]"
        ui_print "  音量+ = 保留配置 [Volume+ = Keep config]"
        ui_print "  音量- = 全部清除 [Volume- = Clean all]"
        ui_print "  10秒超时自动保留 [10s timeout, keep by default]"
        ui_print ""

        # 使用 teeforge 音量键监听 Use teeforge volume key listener
        RESULT=$($MODPATH/teeforge --volume 10)
        if [ "$RESULT" = "0" ]; then
            ui_print "  清除所有数据 [Cleaning all data]"
            rm -rf "$TEEFORGE_DIR"
        else
            ui_print "  保留配置 [Keeping config]"
            # 删除除配置外的所有文件 Delete all except config
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

# 设置 teeforge 权限 Set teeforge permissions
chmod 755 $MODPATH/teeforge

# Download resetprop-rs
ui_print "  Preparing resetprop-rs..."
RESETPROP_DIR="$MODPATH/resetprop-rs"
mkdir -p "$RESETPROP_DIR"

ARCH=$(getprop ro.product.cpu.abi)
case "$ARCH" in
    arm64-v8a)   BINARY="resetprop-arm64-v8a" ;;
    armeabi-v7a) BINARY="resetprop-armeabi-v7a" ;;
    x86_64)      BINARY="resetprop-x86_64" ;;
    x86)         BINARY="resetprop-x86" ;;
    *)           BINARY="resetprop-arm64-v8a" ;;
esac
ui_print "  Arch: $ARCH -> $BINARY"

DOWNLOAD_URL="https://github.com/Enginex0/resetprop-rs/releases/download/v0.6.0/$BINARY"
$MODPATH/teeforge --config "$CONFIG_FILE" --download "$DOWNLOAD_URL" "$RESETPROP_DIR/$BINARY" 2>&1 | while IFS= read -r line; do
    ui_print "  $line"
done

if [ -f "$RESETPROP_DIR/$BINARY" ]; then
    SIZE=$(wc -c < "$RESETPROP_DIR/$BINARY")
    chmod 755 "$RESETPROP_DIR/$BINARY"
    ui_print "  Installed: $BINARY ($SIZE bytes)"
else
    ui_print "  ! Download failed / 下载失败"
    ui_print "  ! Using standard resetprop"
fi

# Set permissions
set_perm_recursive $MODPATH 0 0 0755 0644
set_perm $MODPATH/teeforge 0 0 0755

ui_print " "
ui_print "  Done! Reboot to activate"
ui_print "  完成！重启生效"
ui_print " "
