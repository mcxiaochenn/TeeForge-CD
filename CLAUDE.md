# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

TeeForge-CD is a Magisk/KernelSU module that:
- Auto-populates Tricky Store's `target.txt` with user-installed apps
- Manages keybox files from configurable remote sources
- Implements weak bootloader hiding

Primary language: **C** (chosen for minimal binary size and direct NDK support).

## Build Commands

```bash
# One-click build (planned)
./build.sh

# Manual NDK cross-compile for aarch64
$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android34-clang \
    -o out/teeforge native/src/*.c -I native/include

# Deploy to device
adb push out/teeforge /data/adb/modules/teeforge_cd/
adb shell /data/adb/modules/teeforge_cd/service.sh
```

## Architecture

### Module Installation Flow
```
update-binary → customize.sh → (push files to /data/adb/modules/teeforge_cd/)
service.sh → (triggered by init at boot) → teeforge → target.txt
```

### Key Paths
| Path | Purpose |
|------|---------|
| `/data/system/packages.xml` | System package database (input) |
| `/data/adb/tricky_store/target.txt` | Tricky Store target list (output) |
| `/data/adb/teeforge/keybox/` | Managed keybox files |
| `/data/adb/teeforge/sources.conf` | Remote keybox source URLs |

### Source Files (planned)
- `native/src/main.c` — Entry point, argument parsing
- `native/src/target.c` — Parse packages.xml (lightweight XML state machine, no libxml), filter user apps, write target.txt
- `native/src/keybox.c` — HTTP GET for keybox sources, parse and store
- `native/src/utils.c` — File I/O, string ops, logging
- `native/include/teeforge.h` — Public headers

## Technical Decisions

- **No XML library**: Hand-rolled state machine for parsing packages.xml
- **Boot-aware**: `service.sh` runs at boot via Magisk service.d
- **Config persistence**: Module updates preserve config via `$MODPATH/teeforge.conf` symlinked to `/data/adb/teeforge/`

## Code Style

- C code targeting Android API 24+ (Magisk 20.4+ compatibility)
- Error handling: log to stdout/stderr, never crash — degrade gracefully
- Binary size priority: avoid large dependencies, prefer static functions
