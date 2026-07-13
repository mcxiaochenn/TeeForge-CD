#!/bin/bash
#
# TeeForge-CD Clean Script
# 清理构建产物 [Clean build artifacts]
# Usage: ./clean.sh [--all]
#

set -e

# Colors 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Parse args 解析参数
ALL=0
for arg in "$@"; do
    case $arg in
        --all) ALL=1 ;;
    esac
done

echo -e "${YELLOW}清理构建产物 [Cleaning build artifacts]...${NC}"

# 清理 out 目录（构建输出）
if [ -d "$SCRIPT_DIR/out" ]; then
    rm -rf "$SCRIPT_DIR/out"
    echo -e "  已删除 [Removed]: out/"
fi

# 清理根目录残留的 zip 文件
rm -f "$SCRIPT_DIR"/*.zip
echo -e "  已删除 [Removed]: *.zip"

if [ $ALL -eq 1 ]; then
    # 清理 build 目录（如果存在）
    if [ -d "$SCRIPT_DIR/build" ]; then
        rm -rf "$SCRIPT_DIR/build"
        echo -e "  已删除 [Removed]: build/"
    fi
fi

echo -e "${GREEN}清理完成 [Clean complete]!${NC}"