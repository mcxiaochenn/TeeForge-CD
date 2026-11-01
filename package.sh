#!/bin/bash
# 必须保持 LF 换行，供 Linux/Android 环境直接执行 Must remain LF-only
#
# TeeForge-CD Packaging Script
# 构建并打包 Magisk 模块 [Build and package Magisk module]
# Usage: ./package.sh [--clean]
#

set -e

if [ "${1:-}" = "--clean" ]; then
    rm -rf out/build out/bin
    shift
fi
[ "$#" -eq 0 ] || { echo "未知参数 [Unknown argument]: $1" >&2; exit 2; }
cargo run -p xtask -- package
