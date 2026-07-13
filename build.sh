#!/bin/bash
#
# TeeForge-CD Build Script
# Usage: ./build.sh [--clean] [--push]
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Check NDK
if [ -z "$NDK" ]; then
    echo -e "${RED}Error: NDK environment variable not set${NC}"
    echo "Run: export NDK=/path/to/android-ndk"
    exit 1
fi

# Detect host OS
HOST_OS=$(uname -s | tr '[:upper:]' '[:lower:]')
case "$HOST_OS" in
    linux*)  PREBUILT="linux-x86_64" ;;
    darwin*) PREBUILT="darwin-x86_64" ;;
    msys*|mingw*|cygwin*) PREBUILT="windows-x86_64" ;;
    *) echo -e "${RED}Unsupported OS: $HOST_OS${NC}"; exit 1 ;;
esac

CC="$NDK/toolchains/llvm/prebuilt/$PREBUILT/bin/aarch64-linux-android34-clang"
STRIP="$NDK/toolchains/llvm/prebuilt/$PREBUILT/bin/llvm-strip"

# Verify compiler
if [ ! -f "$CC" ]; then
    echo -e "${RED}Compiler not found: $CC${NC}"
    exit 1
fi

# Directories
SRC_DIR="native/src"
INC_DIR="native/include"
OUT_DIR="out"
OBJ_DIR="$OUT_DIR/obj"

# Parse args
CLEAN=0
PUSH=0
for arg in "$@"; do
    case $arg in
        --clean) CLEAN=1 ;;
        --push)  PUSH=1 ;;
    esac
done

# Clean
if [ $CLEAN -eq 1 ]; then
    echo -e "${YELLOW}Cleaning...${NC}"
    rm -rf "$OUT_DIR"
fi

# Create output dirs
mkdir -p "$OBJ_DIR"

# Source files
SOURCES="$SRC_DIR/main.c $SRC_DIR/target.c $SRC_DIR/utils.c"

# Compile objects
echo -e "${GREEN}Compiling...${NC}"
for src in $SOURCES; do
    obj="$OBJ_DIR/$(basename "$src" .c).o"
    echo "  CC  $src"
    "$CC" -c -o "$obj" "$src" -I"$INC_DIR" -Wall -Wextra -O2
done

# Link
echo -e "${GREEN}Linking...${NC}"
"$CC" -o "$OUT_DIR/teeforge" "$OBJ_DIR"/*.o -static
"$STRIP" "$OUT_DIR/teeforge"

SIZE=$(stat -f%z "$OUT_DIR/teeforge" 2>/dev/null || stat -c%s "$OUT_DIR/teeforge" 2>/dev/null)
echo -e "${GREEN}Build complete: $OUT_DIR/teeforge ($SIZE bytes)${NC}"

# Copy config file
if [ -f "config.conf" ]; then
    cp config.conf "$OUT_DIR/"
    echo -e "${GREEN}Copied config.conf to $OUT_DIR/${NC}"
fi

# Push to device
if [ $PUSH -eq 1 ]; then
    echo -e "${YELLOW}Pushing to device...${NC}"
    adb push "$OUT_DIR/teeforge" /data/adb/modules/teeforge_cd/
    adb push "$OUT_DIR/config.conf" /data/adb/modules/teeforge_cd/
    adb shell chmod 755 /data/adb/modules/teeforge_cd/teeforge
    echo -e "${GREEN}Done. Run: adb shell /data/adb/modules/teeforge_cd/teeforge --generate${NC}"
fi
