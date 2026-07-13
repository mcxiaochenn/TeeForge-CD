#!/system/bin/sh
# TeeForge-CD Installation Script
# 安装脚本 [Installation script]

ui_print "- Installing TeeForge-CD"

# Create directories 创建目录
mkdir -p /data/adb/teeforge/keybox
mkdir -p /data/adb/teeforge/logs

# Copy config file if not exists 复制配置文件（如不存在）
if [ ! -f /data/adb/teeforge/config.conf ]; then
    cp $MODPATH/config.conf /data/adb/teeforge/config.conf
    ui_print "- Created default config.conf"
else
    ui_print "- Preserved existing config.conf"
fi

# 设置 teeforge 权限 Set teeforge permissions
chmod 755 $MODPATH/teeforge

# Download resetprop-rs 下载 resetprop-rs
ui_print "- Downloading resetprop-rs..."
RESETPROP_DIR="$MODPATH/resetprop-rs"
mkdir -p "$RESETPROP_DIR"

# 检测设备架构 Detect device architecture
ARCH=$(getprop ro.product.cpu.abi)
case "$ARCH" in
    arm64-v8a)   BINARY="resetprop-arm64-v8a" ;;
    armeabi-v7a) BINARY="resetprop-armeabi-v7a" ;;
    x86_64)      BINARY="resetprop-x86_64" ;;
    x86)         BINARY="resetprop-x86" ;;
    *)           BINARY="resetprop-arm64-v8a" ;;
esac

# 使用 teeforge 下载（支持地区检测和镜像回退）
# Use teeforge download (supports region detection and mirror fallback)
DOWNLOAD_URL="https://github.com/Enginex0/resetprop-rs/releases/download/v0.6.0/$BINARY"
$MODPATH/teeforge --download "$DOWNLOAD_URL" "$RESETPROP_DIR/$BINARY" 2>/dev/null

# 设置权限 Set permissions
if [ -f "$RESETPROP_DIR/$BINARY" ]; then
    chmod 755 "$RESETPROP_DIR/$BINARY"
    ui_print "- resetprop-rs installed ($BINARY)"
else
    ui_print "- Warning: resetprop-rs download failed, will use standard resetprop"
fi

# Set permissions 设置权限
set_perm_recursive $MODPATH 0 0 0755 0644
set_perm $MODPATH/teeforge 0 0 0755

ui_print "- Installation complete"
