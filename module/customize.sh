#!/system/bin/sh
# 必须保持 LF 换行，供 Android sh 直接执行 Must remain LF-only
# TeeForge-CD Installation Script
# 安装脚本 [Installation script]

ui_print " "
ui_print "  Installing TeeForge-CD / 正在安装"
ui_print " "

# 文件完整性校验 File integrity verification
. "$MODPATH/verify.sh"
VERIFY_RET=$?
if [ "$VERIFY_RET" -ne 0 ]; then
    ui_print "  !! 安装中止 Installation aborted !!"
    ui_print "  !! 请重新下载模块 Please re-download the module"
    abort "  Integrity verification failed / 文件校验失败"
fi
ui_print " "

# 选择当前设备对应的 TeeForge 二进制 Select TeeForge binary for current ABI
ARCH=$(getprop ro.product.cpu.abi)
case "$ARCH" in
    arm64-v8a) EXPECTED_MACHINE=183 ;;
    armeabi-v7a) EXPECTED_MACHINE=40 ;;
    x86) EXPECTED_MACHINE=3 ;;
    x86_64) EXPECTED_MACHINE=62 ;;
    *) abort "  不支持的设备架构 [Unsupported device ABI]: $ARCH" ;;
esac

TEE_BIN="$MODPATH/bin/$ARCH/teeforge"
if [ ! -f "$TEE_BIN" ]; then
    abort "  缺少 $ARCH 二进制 [Missing binary for $ARCH]"
fi

ELF_MACHINE=$(od -An -t u2 -j 18 -N 2 "$TEE_BIN" 2>/dev/null | tr -d ' ')
if [ "$ELF_MACHINE" != "$EXPECTED_MACHINE" ]; then
    abort "  ELF 架构校验失败 [ELF architecture mismatch]: $ARCH/$ELF_MACHINE"
fi

cp "$TEE_BIN" "$MODPATH/teeforge" || abort "  无法安装 teeforge [Cannot install teeforge]"
chmod 755 "$MODPATH/teeforge"
rm -rf "$MODPATH/bin"
ui_print "  架构 [ABI]: $ARCH"
ui_print " "

TEEFORGE_DIR="/data/adb/teeforge"
CONFIG_FILE="$TEEFORGE_DIR/config.conf"

# 先设置权限（音量键检测需要执行二进制）
# Set permissions first (volume key detection needs binary to be executable)
chmod 755 "$MODPATH/teeforge"

# 检测 root 方式 Detect root method (安装时安装前，环境变量可用)
ROOT_RESULT=$("$MODPATH/teeforge" --rootdetect 2>/dev/null)
ROOT_METHOD=$(echo "$ROOT_RESULT" | sed -n '1p')
ROOT_VERSION=$(echo "$ROOT_RESULT" | sed -n '2p')
[ -n "$ROOT_METHOD" ] && ui_print "  Root: $ROOT_METHOD (v$ROOT_VERSION)"
ui_print " "

# 检查已有安装 Check existing installation
if [ -d "$TEEFORGE_DIR" ]; then
    if [ -f "$CONFIG_FILE" ]; then
        ui_print "  检测到已有配置 [Existing config detected]"
        ui_print "  10秒超时自动保留 [10s timeout, keep by default]"
        ui_print " "
        ui_print "  音量+ = 保留配置 [Volume+ = Keep config]"
        ui_print "  音量- = 全部清除 [Volume- = Clean all]"

        RESULT=$("$MODPATH/teeforge" --volume 10 --no-rootdetect)
        ui_print " "
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
ui_print " "

# 创建目录 Create directories
ui_print "  创建目录 [Creating dirs]..."
mkdir -p "$TEEFORGE_DIR/keybox"
mkdir -p "$TEEFORGE_DIR/logs"
ui_print " "

ui_print "  属性工具 [Property tool]: standard resetprop"

# 生成 sys.conf（系统配置，动态生成）Generate sys.conf (system config, dynamic)
cat > "$TEEFORGE_DIR/sys.conf" << EOF
# TeeForge-CD System Configuration (auto-generated, don't edit)
# 系统配置（自动生成，勿手动修改）

packages_xml=/data/system/packages.xml
target_txt=/data/adb/tricky_store/target.txt
teesim_config=/data/adb/teesim/config.json
keybox_dir=/data/adb/teeforge/keybox/
sources_conf=/data/adb/teeforge/sources.conf
log_dir=/data/adb/teeforge/logs/
root_method=$ROOT_METHOD
root_version=$ROOT_VERSION
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
EOF
    ui_print "  config.conf 已创建 [config.conf created] (debug=$MODULE_DEBUG)"
else
    ui_print "  config.conf 已保留 [config.conf preserved]"
fi

# Set permissions
set_perm_recursive "$MODPATH" 0 0 0755 0644
set_perm "$MODPATH/teeforge" 0 0 0755

ui_print " "
ui_print "  Done! Reboot to activate"
ui_print "  完成！重启生效"
ui_print " "
