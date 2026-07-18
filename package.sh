#!/bin/bash
#
# TeeForge-CD Packaging Script
# 构建并打包 Magisk 模块 [Build and package Magisk module]
# Usage: ./package.sh [--clean]
#

set -e

# Colors 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Directories 目录
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MODULE_SRC="$SCRIPT_DIR/module"
BUILD_DIR="$SCRIPT_DIR/out/build"
MODULE_DIR="$BUILD_DIR/teeforge_cd"
OUT_DIR="$SCRIPT_DIR/out"

# Parse args 解析参数
CLEAN=0
for arg in "$@"; do
    case $arg in
        --clean) CLEAN=1 ;;
    esac
done

# Clean 清理
if [ $CLEAN -eq 1 ]; then
    echo -e "${YELLOW}清理构建目录 [Cleaning build directory]...${NC}"
    rm -rf "$OUT_DIR"
fi

# Step 1: Build binary 构建二进制
echo -e "${GREEN}步骤 1/5: 构建二进制 [Step 1/5: Building binary]...${NC}"
"$SCRIPT_DIR/build.sh"
if [ ! -f "$OUT_DIR/teeforge" ]; then
    echo -e "${RED}错误: 构建失败 [Error: Build failed]${NC}"
    exit 1
fi

# Step 2: Build WebUI 构建 WebUI
echo -e "${GREEN}步骤 2/5: 构建 WebUI [Step 2/5: Building WebUI]...${NC}"
WEBROOT_SRC="$SCRIPT_DIR/webroot"
if [ -f "$WEBROOT_SRC/package.json" ] && command -v node &> /dev/null; then
    echo -e "  Node.js 版本 [Node.js version]: $(node -v)"
    (cd "$WEBROOT_SRC" && npm ci --ignore-scripts 2>&1 | tail -1 && npm run build 2>&1)
    if [ -d "$MODULE_SRC/webroot" ]; then
        WEBUI_SIZE=$(du -sh "$MODULE_SRC/webroot" | cut -f1)
        echo -e "  WebUI 构建完成 [WebUI built]: ${WEBUI_SIZE}"
    else
        echo -e "${YELLOW}  警告: WebUI 构建产物未找到 [Warning: WebUI build output not found]${NC}"
    fi
else
    echo -e "${YELLOW}  跳过 WebUI 构建（node 或 package.json 不可用）[Skipping WebUI build (node or package.json unavailable)]${NC}"
fi

# Step 3: Read version 读取版本
echo -e "${GREEN}步骤 3/5: 读取版本信息 [Step 3/5: Reading version]...${NC}"
VERSION=$(grep "^version=" "$MODULE_SRC/module.prop" | cut -d'=' -f2)
VERSION_CODE=$(grep "^versionCode=" "$MODULE_SRC/module.prop" | cut -d'=' -f2)
MODULE_ID=$(grep "^id=" "$MODULE_SRC/module.prop" | cut -d'=' -f2)
echo -e "  版本 [Version]: ${VERSION} (${VERSION_CODE})"

# Step 4: Prepare module directory 准备模块目录
echo -e "${GREEN}步骤 4/5: 准备模块目录 [Step 4/5: Preparing module directory]...${NC}"
rm -rf "$MODULE_DIR"
mkdir -p "$MODULE_DIR"

# Copy module files 复制模块文件
cp -r "$MODULE_SRC"/* "$MODULE_DIR/"

# Copy binary 复制二进制
cp "$OUT_DIR/teeforge" "$MODULE_DIR/"

# Copy README 复制说明文档
if [ -f "$SCRIPT_DIR/README.md" ]; then
    cp "$SCRIPT_DIR/README.md" "$MODULE_DIR/README"
    echo -e "  已复制 [Copied]: README"
fi

# Set permissions 设置权限
chmod 755 "$MODULE_DIR/teeforge"
chmod 755 "$MODULE_DIR/customize.sh"
chmod 755 "$MODULE_DIR/service.sh"
chmod 755 "$MODULE_DIR/META-INF/com/google/android/update-binary"

echo -e "  模块目录 [Module directory]: $MODULE_DIR"

# Generate integrity checksums 生成完整性校验
echo -e "  生成校验文件 [Generating checksums]..."
(
    cd "$MODULE_DIR"
    # 排除校验文件自身和 META-INF（由 Magisk 安装器管理）
    # Exclude checksums file itself and META-INF (managed by Magisk installer)
    find . -type f ! -name '.sha256' ! -path './META-INF/*' -print0 \
        | sort -z \
        | xargs -0 sha256sum \
        | sed 's|^\([a-f0-9]*\) [*]\./|\1  |' \
        > .sha256
)
CHECKSUM_COUNT=$(wc -l < "$MODULE_DIR/.sha256" | tr -d ' ')
echo -e "  校验文件已生成 [Checksums generated]: $CHECKSUM_COUNT 个文件 [files]"

# Step 5: Create zip 创建压缩包
echo -e "${GREEN}步骤 5/5: 创建安装包 [Step 5/5: Creating zip]...${NC}"
ZIP_NAME="${MODULE_ID}-${VERSION}.zip"
ZIP_PATH="$OUT_DIR/$ZIP_NAME"

# Remove old zip if exists
rm -f "$ZIP_PATH"

# Create zip from module directory (files at root, no nested folder)
(
    cd "$BUILD_DIR/teeforge_cd" || exit 1
    if command -v zip &> /dev/null; then
        zip -r "$ZIP_PATH" .
    elif command -v powershell &> /dev/null; then
        # 转换为 Windows 路径格式
        WIN_OUT_DIR=$(cd "$OUT_DIR" && pwd -W 2>/dev/null || pwd)
        WIN_ZIP_PATH="$WIN_OUT_DIR\\$ZIP_NAME"
        powershell -Command "Compress-Archive -Path '.\\*' -DestinationPath '$WIN_ZIP_PATH' -Force"
    else
        echo -e "${RED}错误: 需要 zip 或 PowerShell [Error: zip or PowerShell required]${NC}"
        exit 1
    fi
)

# Verify 检查
if [ -f "$ZIP_PATH" ]; then
    SIZE=$(stat -f%z "$ZIP_PATH" 2>/dev/null || stat -c%s "$ZIP_PATH" 2>/dev/null)
    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}打包完成 [Packaging complete]!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo -e "  文件 [File]: ${ZIP_NAME}"
    echo -e "  大小 [Size]: ${SIZE} bytes"
    echo -e "  路径 [Path]: ${ZIP_PATH}"
    echo ""
    echo -e "安装方法 [Installation]:"
    echo -e "  1. 将 ${ZIP_NAME} 推送到设备"
    echo -e "     Push ${ZIP_NAME} to device"
    echo -e "  2. 在 Magisk/KernelSU 中安装"
    echo -e "     Install via Magisk/KernelSU"
    echo ""
else
    echo -e "${RED}错误: 创建压缩包失败 [Error: Failed to create zip]${NC}"
    exit 1
fi
