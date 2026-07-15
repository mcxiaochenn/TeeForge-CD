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

# 检测 root 方式 Detect root method (安装时安装前，环境变量可用)
ROOT_RESULT=$($MODPATH/teeforge --rootdetect 2>/dev/null)
ROOT_METHOD=$(echo "$ROOT_RESULT" | sed -n '1p')
ROOT_VERSION=$(echo "$ROOT_RESULT" | sed -n '2p')
[ -n "$ROOT_METHOD" ] && ui_print "  Root: $ROOT_METHOD (v$ROOT_VERSION)"

# 检查已有安装 Check existing installation
if [ -d "$TEEFORGE_DIR" ]; then
    if [ -f "$CONFIG_FILE" ]; then
        ui_print "  检测到已有配置 [Existing config detected]"
        ui_print "  音量+ = 保留配置 [Volume+ = Keep config]"
        ui_print "  音量- = 全部清除 [Volume- = Clean all]"
        ui_print "  10秒超时自动保留 [10s timeout, keep by default]"
        ui_print ""

        RESULT=$($MODPATH/teeforge --volume 10 --no-rootdetect)
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

# 选择 resetprop 工具 Select resetprop tool
ui_print "  选择属性修改工具 [Select prop tool]"
ui_print "  音量+ = 传统 resetprop（推荐）"
ui_print "  [Volume+ = Traditional resetprop (Recommended)]"
ui_print "  音量- = resetprop-rs"
ui_print "  [Volume- = resetprop-rs]"
ui_print "  隐蔽性更佳，但可能被环境检测软件识别"
ui_print "  [Better stealth, but may be detected]"
ui_print "  10秒超时默认传统方式 [10s timeout, default traditional]"
ui_print ""

PROP_TOOL="standard"
PROP_RESULT=$($MODPATH/teeforge --volume 10 --no-rootdetect)
if [ "$PROP_RESULT" = "1" ]; then
    PROP_TOOL="standard"
    ui_print "  已选择传统 resetprop [Selected traditional resetprop]"
    # 删除 resetprop-rs 二进制，减小体积 Remove resetprop-rs binaries, reduce size
    rm -rf "$MODPATH/resetprop-rs"
else
    PROP_TOOL="rs"
    ui_print "  已选择 resetprop-rs [Selected resetprop-rs]"

    # 检测架构，只保留对应二进制 Detect arch, keep matching binary only
    ARCH=$(getprop ro.product.cpu.abi)
    case "$ARCH" in
        arm64-v8a)
            ui_print "  架构 [Arch]: arm64-v8a"
            rm -f "$MODPATH/resetprop-rs/resetprop-armeabi-v7a"
            ;;
        armeabi-v7a)
            ui_print "  架构 [Arch]: armeabi-v7a"
            rm -f "$MODPATH/resetprop-rs/resetprop-arm64-v8a"
            ;;
        x86|x86_64)
            ui_print "  !! 设备架构不支持 resetprop-rs !!"
            ui_print "  !! Arch not supported for resetprop-rs !!"
            ui_print "  $ARCH"
            ui_print "  请重新安装并选择传统方式"
            ui_print "  [Please reinstall and select traditional]"
            abort "  Installation aborted: resetprop-rs not available for $ARCH"
            ;;
        *)
            ui_print "  未知架构，保留全部 [Unknown arch, keeping all]: $ARCH"
            ;;
    esac
fi

# 生成 sys.conf（系统配置，动态生成）Generate sys.conf (system config, dynamic)
cat > "$TEEFORGE_DIR/sys.conf" << EOF
# TeeForge-CD System Configuration (auto-generated, don't edit)
# 系统配置（自动生成，勿手动修改）

packages_xml=/data/system/packages.xml
target_txt=/data/adb/tricky_store/target.txt
keybox_dir=/data/adb/teeforge/keybox/
sources_conf=/data/adb/teeforge/sources.conf
log_dir=/data/adb/teeforge/logs/
prop_tool=$PROP_TOOL
EOF
ui_print "  sys.conf 已生成 [sys.conf generated]"

# 生成用户配置（仅 debug 设置）Generate user config (debug setting only)
if [ ! -f "$CONFIG_FILE" ]; then
    # 从模块包读取 debug 值（dev 构建为 1，release 为 0）
    # Read debug value from module zip (dev=1, release=0)
    MODULE_DEBUG=0
    if [ -f "$MODPATH/config.conf" ]; then
        MODULE_DEBUG=$(grep '^debug=' "$MODPATH/config.conf" | cut -d'=' -f2)
        [ -z "$MODULE_DEBUG" ] && MODULE_DEBUG=0
    fi
    cat > "$CONFIG_FILE" << EOF
# TeeForge-CD User Configuration
# 用户配置 [User configuration]

# 0: 关闭 Off (默认 default)
# 1: 开启 On（日志写入文件 Logs written to file）
debug=$MODULE_DEBUG

# 弱隐 BL 开关 Weak BL Hiding Switches
# 0: 关闭 Off  1: 开启 On (默认 default)
blhide=1
blhide_boot=1
blhide_security=1
blhide_vendor=1
blhide_oem=1
blhide_secureboot=1
blhide_realme=1
blhide_recovery=1
blhide_developer=1
blhide_selinux=1
blhide_virtual=1
blhide_delete=1
blhide_compact=1
EOF
    ui_print "  config.conf 已创建 [config.conf created] (debug=$MODULE_DEBUG)"
else
    ui_print "  config.conf 已保留 [config.conf preserved]"
fi

# Set permissions
set_perm_recursive $MODPATH 0 0 0755 0644
set_perm $MODPATH/teeforge 0 0 0755

# resetprop-rs 需要执行权限（set_perm_recursive 会重置为 0644）
# resetprop-rs needs execute permission (set_perm_recursive resets to 0644)
# 传统方式已删除此目录，循环安全跳过 Dir removed for standard, loop safely skips
for f in "$MODPATH/resetprop-rs"/resetprop-*; do
    [ -f "$f" ] && set_perm "$f" 0 0 0755
done

ui_print " "
ui_print "  Done! Reboot to activate"
ui_print "  完成！重启生效"
ui_print " "
