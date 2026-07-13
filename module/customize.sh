#!/system/bin/sh
# TeeForge-CD Installation Script
# 安装脚本 [Installation script]

ui_print "- Installing TeeForge-CD"
ui_print "- 正在安装 TeeForge-CD"

# Create directories 创建目录
ui_print "- Creating directories... / 创建目录..."
mkdir -p /data/adb/teeforge/keybox
mkdir -p /data/adb/teeforge/logs

# Copy config file if not exists 复制配置文件（如不存在）
if [ ! -f /data/adb/teeforge/config.conf ]; then
    cp $MODPATH/config.conf /data/adb/teeforge/config.conf
    ui_print "- Created default config.conf / 已创建默认配置"
else
    ui_print "- Preserved existing config.conf / 保留已有配置"
fi

# 设置 teeforge 权限 Set teeforge permissions
chmod 755 $MODPATH/teeforge

# Download resetprop-rs 下载 resetprop-rs
ui_print "- Preparing resetprop-rs... / 准备下载 resetprop-rs..."
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
ui_print "- Architecture: $ARCH -> $BINARY"

# 使用 teeforge 下载（日志输出到安装页面）
# Use teeforge download (logs shown on install page)
DOWNLOAD_URL="https://github.com/Enginex0/resetprop-rs/releases/download/v0.6.0/$BINARY"
ui_print "- Downloading... / 下载中..."

$MODPATH/teeforge --download "$DOWNLOAD_URL" "$RESETPROP_DIR/$BINARY" 2>&1 | while IFS= read -r line; do
    ui_print "  $line"
done

# 设置权限 Set permissions
if [ -f "$RESETPROP_DIR/$BINARY" ]; then
    SIZE=$(wc -c < "$RESETPROP_DIR/$BINARY")
    chmod 755 "$RESETPROP_DIR/$BINARY"
    ui_print "- Installed: $BINARY ($SIZE bytes) / 已安装"
else
    ui_print "! Download failed / 下载失败"
    ui_print "! Using standard resetprop / 使用标准 resetprop"
fi

# Set permissions 设置权限
ui_print "- Setting permissions... / 设置权限..."
set_perm_recursive $MODPATH 0 0 0755 0644
set_perm $MODPATH/teeforge 0 0 0755

ui_print "- Installation complete! / 安装完成！"
ui_print "  Reboot to activate / 重启生效"
