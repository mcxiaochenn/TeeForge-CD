#!/bin/bash
# 必须保持 LF 换行，供 Linux/Android 环境直接执行 Must remain LF-only
#
# TeeForge-CD Build Script
# 构建脚本 [Build script]
# Usage: ./build.sh [--clean] [--push]
#

set -e

PUSH=0
CLEAN=0
for arg in "$@"; do
    case "$arg" in
        --push) PUSH=1 ;;
        --clean) CLEAN=1 ;;
        *) echo "未知参数 [Unknown argument]: $arg" >&2; exit 2 ;;
    esac
done

if [ "$CLEAN" -eq 1 ]; then
    cargo clean
    rm -rf out/bin
fi

cargo run -p xtask -- build

if [ "$PUSH" -eq 1 ]; then
    ARCH=$(adb shell getprop ro.product.cpu.abi | tr -d '\r')
    BIN="out/bin/$ARCH/teeforge"
    HIDE="out/bin/$ARCH/teeforge-hide"
    [ -f "$BIN" ] || { echo "没有当前设备架构的产物 [No output for device ABI]: $ARCH" >&2; exit 1; }
    [ -f "$HIDE" ] || { echo "缺少弱隐二进制 [Missing weak-hiding binary]: $ARCH" >&2; exit 1; }
    adb push "$BIN" /data/adb/modules/teeforge_cd/runtime/teeforge
    adb push "$HIDE" /data/adb/modules/teeforge_cd/runtime/teeforge-hide
    adb shell chmod 755 /data/adb/modules/teeforge_cd/runtime/teeforge /data/adb/modules/teeforge_cd/runtime/teeforge-hide
fi
