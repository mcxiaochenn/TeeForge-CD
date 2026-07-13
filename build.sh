#!/bin/bash
#
# TeeForge-CD Build Script
# 构建脚本 [Build script]
# Usage: ./build.sh [--clean] [--push]
#

set -e

# Colors 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Check NDK 检查 NDK
if [ -z "$NDK" ]; then
    echo -e "${RED}错误: NDK 环境变量未设置 [Error: NDK environment variable not set]${NC}"
    echo "运行 Run: export NDK=/path/to/android-ndk"
    exit 1
fi

# Detect host OS 检测主机系统
HOST_OS=$(uname -s | tr '[:upper:]' '[:lower:]')
case "$HOST_OS" in
    linux*)  PREBUILT="linux-x86_64" ;;
    darwin*) PREBUILT="darwin-x86_64" ;;
    msys*|mingw*|cygwin*) PREBUILT="windows-x86_64" ;;
    *) echo -e "${RED}不支持的操作系统 [Unsupported OS]: $HOST_OS${NC}"; exit 1 ;;
esac

CC="$NDK/toolchains/llvm/prebuilt/$PREBUILT/bin/aarch64-linux-android34-clang"
STRIP="$NDK/toolchains/llvm/prebuilt/$PREBUILT/bin/llvm-strip"

# Verify compiler 验证编译器
if [ ! -f "$CC" ]; then
    echo -e "${RED}编译器未找到 [Compiler not found]: $CC${NC}"
    exit 1
fi

# Directories 目录
SRC_DIR="native/src"
INC_DIR="native/include"
OUT_DIR="out"
BUILD_DIR="$OUT_DIR/build"
OBJ_DIR="$BUILD_DIR/obj"

# Parse args 解析参数
CLEAN=0
PUSH=0
for arg in "$@"; do
    case $arg in
        --clean) CLEAN=1 ;;
        --push)  PUSH=1 ;;
    esac
done

# Clean 清理
if [ $CLEAN -eq 1 ]; then
    echo -e "${YELLOW}清理中 [Cleaning]...${NC}"
    rm -rf "$OUT_DIR"
fi

# Create output dirs 创建输出目录
mkdir -p "$OBJ_DIR"

# 优先使用环境变量（CI 传入），否则从本地提取
# Priority: env vars (CI), fallback to local extraction
if [ -z "$VERSION" ]; then
    VERSION=$(grep 'TEEFORGE_VERSION' native/include/teeforge.h | sed 's/.*"\(.*\)".*/\1/')
fi
if [ -z "$VERSION_CODE" ]; then
    VERSION_CODE=$(git rev-list --count HEAD 2>/dev/null || echo "1")
fi

echo -e "${GREEN}版本 [Version]: $VERSION (code: $VERSION_CODE)${NC}"

# 动态写入 module.prop Dynamically write module.prop
sed -i "s/^version=.*/version=$VERSION/" module/module.prop
sed -i "s/^versionCode=.*/versionCode=$VERSION_CODE/" module/module.prop

# Source files 源文件
SOURCES="$SRC_DIR/main.c $SRC_DIR/target.c $SRC_DIR/utils.c $SRC_DIR/blhide.c $SRC_DIR/keybox.c $SRC_DIR/download.c"

# Compile objects 编译目标文件
echo -e "${GREEN}编译中 [Compiling]...${NC}"
for src in $SOURCES; do
    obj="$OBJ_DIR/$(basename "$src" .c).o"
    echo "  CC  $src"
    "$CC" -c -o "$obj" "$src" -I"$INC_DIR" -Wall -Wextra -O2
done

# Link 链接
echo -e "${GREEN}链接中 [Linking]...${NC}"
"$CC" -o "$BUILD_DIR/teeforge" "$OBJ_DIR"/*.o -static
"$STRIP" "$BUILD_DIR/teeforge"

# Copy to output directory 复制到输出目录
cp "$BUILD_DIR/teeforge" "$OUT_DIR/"

SIZE=$(stat -f%z "$OUT_DIR/teeforge" 2>/dev/null || stat -c%s "$OUT_DIR/teeforge" 2>/dev/null)
echo -e "${GREEN}构建完成 [Build complete]: $OUT_DIR/teeforge ($SIZE bytes)${NC}"

# Push to device 推送到设备
if [ $PUSH -eq 1 ]; then
    echo -e "${YELLOW}推送到设备 [Pushing to device]...${NC}"
    adb push "$OUT_DIR/teeforge" /data/adb/modules/teeforge_cd/
    adb shell chmod 755 /data/adb/modules/teeforge_cd/teeforge
    echo -e "${GREEN}完成 [Done]. 运行 Run: adb shell /data/adb/modules/teeforge_cd/teeforge --generate${NC}"
fi
