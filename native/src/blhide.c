#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

/* ===== 弱隐 BL Weak Bootloader Hiding ===== */
/*
 * 部分属性列表参考 Integrity-Box 项目
 * Some property list entries参考 Integrity-Box project
 * Source: https://github.com/MeowDump/Integrity-Box
 * License: GPL-3.0
 *
 * 新增属性（相比初始版本） [Added properties (vs initial version)]:
 * - Recovery 模式隐藏 [Recovery mode hiding]
 * - Developer 选项 [Developer options]
 * - SELinux 伪装 [SELinux spoofing]
 * - 虚拟设备检测 [Virtual device detection]
 * - compact 内存整理 [Compact memory]
 * - 属性删除 [Property deletion]
 */

/* resetprop 工具类型 Resetprop tool type */
typedef enum {
    PROP_TOOL_STANDARD = 0,  /* 标准 Magisk resetprop */
    PROP_TOOL_RS       = 1   /* resetprop-rs (KernelSU) */
} prop_tool_t;

static prop_tool_t g_prop_tool = PROP_TOOL_STANDARD;
static const char *g_resetprop_cmd = NULL;  /* 缓存检测结果 Cache detection result */

/* 弱隐 BL 属性类别 Weak BL hiding property categories */
typedef enum {
    BLCAT_BOOT       = 0,
    BLCAT_SECURITY   = 1,
    BLCAT_VENDOR     = 2,
    BLCAT_OEM        = 3,
    BLCAT_SECUREBOOT = 4,
    BLCAT_REALME     = 5,
    BLCAT_RECOVERY   = 6,
    BLCAT_DEVELOPER  = 7,
    BLCAT_SELINUX    = 8,
    BLCAT_VIRTUAL    = 9,
} bl_category_t;

/* resetprop 属性列表 Property list for resetprop */
typedef struct {
    const char *key;
    const char *value;
    bl_category_t category;
} prop_entry_t;

static const prop_entry_t bl_props[] = {
    /* Boot 状态 Boot state */
    {"ro.boot.vbmeta.device_state",     "locked",           BLCAT_BOOT},
    {"ro.boot.verifiedbootstate",       "green",            BLCAT_BOOT},
    {"ro.boot.flash.locked",            "1",                BLCAT_BOOT},
    {"ro.boot.veritymode",              "enforcing",        BLCAT_BOOT},
    {"ro.boot.warranty_bit",            "0",                BLCAT_BOOT},
    {"ro.warranty_bit",                 "0",                BLCAT_BOOT},

    /* 安全属性 Security properties */
    {"ro.debuggable",                   "0",                BLCAT_SECURITY},
    {"ro.force.debuggable",             "0",                BLCAT_SECURITY},
    {"ro.secure",                       "1",                BLCAT_SECURITY},
    {"ro.adb.secure",                   "1",                BLCAT_SECURITY},
    {"ro.build.type",                   "user",             BLCAT_SECURITY},
    {"ro.build.tags",                   "release-keys",     BLCAT_SECURITY},

    /* Vendor 属性 Vendor properties */
    {"ro.vendor.boot.warranty_bit",     "0",                BLCAT_VENDOR},
    {"ro.vendor.warranty_bit",          "0",                BLCAT_VENDOR},
    {"vendor.boot.vbmeta.device_state", "locked",           BLCAT_VENDOR},
    {"vendor.boot.verifiedbootstate",   "green",            BLCAT_VENDOR},

    /* OEM 解锁 OEM unlock */
    {"sys.oem_unlock_allowed",          "0",                BLCAT_OEM},
    {"ro.oem_unlock_supported",         "0",                BLCAT_OEM},

    /* 安全启动 Secure boot */
    {"ro.secureboot.lockstate",         "locked",           BLCAT_SECUREBOOT},

    /* Realme 设备 Realme devices */
    {"ro.boot.realmebootstate",         "green",            BLCAT_REALME},
    {"ro.boot.realme.lockstate",        "1",                BLCAT_REALME},

    /* Recovery 模式隐藏 Recovery mode hiding (from Integrity-Box) */
    {"ro.bootmode",                     "unknown",          BLCAT_RECOVERY},
    {"ro.boot.bootmode",                "unknown",          BLCAT_RECOVERY},
    {"vendor.boot.bootmode",            "unknown",          BLCAT_RECOVERY},

    /* Developer 选项 Developer options (from Integrity-Box) */
    {"persist.sys.developer_options",   "0",                BLCAT_DEVELOPER},
    {"persist.sys.dev_mode",            "0",                BLCAT_DEVELOPER},
    {"persist.sys.debuggable",          "0",                BLCAT_DEVELOPER},

    /* SELinux (from Integrity-Box) */
    {"ro.boot.selinux",                 "enforcing",        BLCAT_SELINUX},

    /* 虚拟设备 Virtual device (from Integrity-Box) */
    {"ro.hardware.virtual_device",      "0",                BLCAT_VIRTUAL},

    /* 结束标记 End marker */
    {NULL, NULL, 0}
};

/* 检查文件可执行 Check if file is executable */
static int is_executable(const char *path) {
    return access(path, X_OK) == 0;
}

/* 检测 resetprop 工具类型并缓存命令路径 Detect and cache resetprop command */
static void detect_prop_tool(void) {
    /* 用户配置优先 User config takes priority */
    if (strcmp(g_config.prop_tool, "standard") == 0) {
        g_prop_tool = PROP_TOOL_STANDARD;
        /* 标准 resetprop 降级策略 Standard resetprop fallback */
        const char *std_paths[] = {
            "/data/adb/ksu/bin/resetprop",
            "/data/adb/ap/bin/resetprop",
            "/data/adb/magisk/resetprop",
            NULL
        };
        /* 尝试固定路径（PATH 在 popen/system 中不一定可用） */
        for (int i = 0; std_paths[i] != NULL; i++) {
            if (file_exists(std_paths[i]) && is_executable(std_paths[i])) {
                g_resetprop_cmd = std_paths[i];
                log_msg(LOG_INFO, "使用标准 resetprop（用户配置）[Using standard resetprop (user config)]: %s", g_resetprop_cmd);
                return;
            }
        }
        /* 回退 PATH 中的 resetprop */
        g_resetprop_cmd = "resetprop";
        log_msg(LOG_INFO, "使用标准 resetprop（用户配置）[Using standard resetprop (user config)]: PATH");
        return;
    }

    /* --- resetprop-rs 检测 --- */
    log_msg(LOG_INFO, "检测 resetprop-rs [Detecting resetprop-rs]...");

    /* 1. 环境变量 Environment variable */
    const char *env_path = getenv("RESETPROP_RS");
    if (env_path && file_exists(env_path)) {
        if (!is_executable(env_path)) {
            log_msg(LOG_WARN, "  resetprop-rs 无执行权限，尝试修复 [No execute permission, attempting fix]: %s", env_path);
            char cmd[512];
            snprintf(cmd, sizeof(cmd), "chmod 755 '%s'", env_path);
            system(cmd);
        }
        if (is_executable(env_path)) {
            g_prop_tool = PROP_TOOL_RS;
            g_resetprop_cmd = env_path;
            log_msg(LOG_INFO, "  使用 resetprop-rs [Using resetprop-rs]: %s", env_path);
            return;
        }
        log_msg(LOG_WARN, "  resetprop-rs 无法执行，回退标准 resetprop [Cannot execute, falling back]");
    }

    /* 2. 模块目录 Module directory */
    const char *mod_paths[] = {
        "/data/adb/modules/teeforge_cd/resetprop-rs/resetprop-arm64-v8a",
        "/data/adb/modules/teeforge_cd/resetprop-rs/resetprop-armeabi-v7a",
        "/data/adb/modules_update/teeforge_cd/resetprop-rs/resetprop-arm64-v8a",
        "/data/adb/modules_update/teeforge_cd/resetprop-rs/resetprop-armeabi-v7a",
        NULL
    };

    for (int i = 0; mod_paths[i] != NULL; i++) {
        if (file_exists(mod_paths[i])) {
            if (!is_executable(mod_paths[i])) {
                log_msg(LOG_WARN, "  无执行权限，尝试修复 [No execute permission, attempting fix]: %s", mod_paths[i]);
                char cmd[512];
                snprintf(cmd, sizeof(cmd), "chmod 755 '%s'", mod_paths[i]);
                system(cmd);
            }
            if (is_executable(mod_paths[i])) {
                g_prop_tool = PROP_TOOL_RS;
                g_resetprop_cmd = mod_paths[i];
                log_msg(LOG_INFO, "  使用 resetprop-rs [Using resetprop-rs]: %s", mod_paths[i]);
                return;
            }
        }
    }

    /* 3. 系统 PATH */
    int ret = system("command -v resetprop-rs > /dev/null 2>&1");
    if (ret == 0) {
        g_prop_tool = PROP_TOOL_RS;
        g_resetprop_cmd = "resetprop-rs";
        log_msg(LOG_INFO, "  使用系统 resetprop-rs [Using system resetprop-rs]");
        return;
    }

    /* rs 检测全部失败，降级标准 resetprop All rs detection failed, fallback to standard */
    g_prop_tool = PROP_TOOL_STANDARD;
    const char *std_paths[] = {
        "/data/adb/ksu/bin/resetprop",
        "/data/adb/ap/bin/resetprop",
        "/data/adb/magisk/resetprop",
        NULL
    };
    for (int i = 0; std_paths[i] != NULL; i++) {
        if (file_exists(std_paths[i]) && is_executable(std_paths[i])) {
            g_resetprop_cmd = std_paths[i];
            log_msg(LOG_INFO, "  resetprop-rs 未找到，降级 [rs not found, fallback]: %s", std_paths[i]);
            return;
        }
    }
    g_resetprop_cmd = "resetprop";
    log_msg(LOG_INFO, "  resetprop-rs 未找到，降级 PATH resetprop [rs not found, fallback to PATH]");
}

/* 获取 resetprop 命令路径（已缓存）Get resetprop command path (cached) */
static const char *get_resetprop_cmd(void) {
    return g_resetprop_cmd ? g_resetprop_cmd : "resetprop";
}

/* 执行 resetprop 命令 Execute resetprop command */
static int run_resetprop(const char *key, const char *value) {
    char cmd[512];
    const char *cmd_path = get_resetprop_cmd();

    if (g_prop_tool == PROP_TOOL_RS) {
        /* resetprop-rs 支持 --stealth 模式 */
        snprintf(cmd, sizeof(cmd), "%s --stealth %s %s", cmd_path, key, value);
    } else {
        snprintf(cmd, sizeof(cmd), "%s %s %s", cmd_path, key, value);
    }

    log_msg(LOG_DEBUG, "  %s", cmd);

    int ret = system(cmd);
    if (ret != 0) {
        log_msg(LOG_WARN, "resetprop 失败 [failed]: %s %s (code %d)", key, value, ret);
    }
    return ret;
}

/* 删除属性 Delete property */
static int run_prop_delete(const char *key) {
    char cmd[512];
    const char *cmd_path = get_resetprop_cmd();

    snprintf(cmd, sizeof(cmd), "%s --delete %s", cmd_path, key);

    log_msg(LOG_DEBUG, "  %s", cmd);

    int ret = system(cmd);
    return ret;
}

/* 压缩属性内存 Compact property memory */
static int run_prop_compact(void) {
    if (g_prop_tool != PROP_TOOL_RS) {
        log_msg(LOG_DEBUG, "跳过 compact（仅支持 resetprop-rs）[Skip compact (resetprop-rs only)]");
        return 0;
    }

    const char *cmd_path = get_resetprop_cmd();
    char cmd[256];
    snprintf(cmd, sizeof(cmd), "%s --compact", cmd_path);

    log_msg(LOG_DEBUG, "压缩属性内存 [Compacting property memory]...");
    int ret = system(cmd);
    if (ret != 0) {
        log_msg(LOG_WARN, "compact 失败 [compact failed] (code %d)", ret);
    }
    return ret;
}

/* 检查属性类别是否启用 Check if property category is enabled */
static int is_category_enabled(bl_category_t cat) {
    if (!g_config.blhide) return 0;  /* 总开关关闭 Master switch off */

    switch (cat) {
        case BLCAT_BOOT:       return g_config.blhide_boot;
        case BLCAT_SECURITY:   return g_config.blhide_security;
        case BLCAT_VENDOR:     return g_config.blhide_vendor;
        case BLCAT_OEM:        return g_config.blhide_oem;
        case BLCAT_SECUREBOOT: return g_config.blhide_secureboot;
        case BLCAT_REALME:     return g_config.blhide_realme;
        case BLCAT_RECOVERY:   return g_config.blhide_recovery;
        case BLCAT_DEVELOPER:  return g_config.blhide_developer;
        case BLCAT_SELINUX:    return g_config.blhide_selinux;
        case BLCAT_VIRTUAL:    return g_config.blhide_virtual;
        default:               return 1;
    }
}

/* 需要删除的属性 Properties to delete (from Integrity-Box) */
static const char *del_props[] = {
    "ro.build.selinux",  /* Integrity-Box: 删除而非覆盖 Delete instead of override */
    NULL
};

int bl_hide(void) {
    log_msg(LOG_INFO, "开始弱隐 BL [Starting weak bootloader hiding]...");

    /* 检测 resetprop 工具 Detect resetprop tool */
    detect_prop_tool();

    /* 确认 boot 已完成（service.sh 已等待，此处仅做二次确认） */
    /* Confirm boot completed (service.sh already waited, double-check here) */
    log_msg(LOG_DEBUG, "确认 boot 完成 [Confirming boot completed]...");
    int boot_wait = 0;
    while (system("getprop sys.boot_completed | grep -q 1") != 0) {
        if (++boot_wait > 30) {
            log_msg(LOG_WARN, "等待 boot 超时 [Boot wait timeout]");
            break;
        }
        sleep(1);
    }

    int success = 0;
    int fail = 0;
    int skipped = 0;

    /* 遍历属性列表 Iterate property list */
    for (int i = 0; bl_props[i].key != NULL; i++) {
        /* 检查类别开关 Check category toggle */
        if (!is_category_enabled(bl_props[i].category)) {
            skipped++;
            log_msg(LOG_DEBUG, "跳过属性 [Skipped property] (类别已禁用 [category disabled]): %s", bl_props[i].key);
            continue;
        }

        if (run_resetprop(bl_props[i].key, bl_props[i].value) == 0) {
            success++;
        } else {
            fail++;
        }
    }

    /* 删除敏感属性 Delete sensitive properties (需 blhide_delete 开关) */
    if (g_config.blhide && g_config.blhide_delete) {
        for (int i = 0; del_props[i] != NULL; i++) {
            log_msg(LOG_DEBUG, "删除属性 [Delete property]: %s", del_props[i]);
            run_prop_delete(del_props[i]);
        }
    } else {
        log_msg(LOG_DEBUG, "跳过属性删除 [Skipped property deletion]");
    }

    /* 压缩属性内存 Compact property memory (需 blhide_compact 开关) */
    if (g_config.blhide && g_config.blhide_compact) {
        run_prop_compact();
    } else {
        log_msg(LOG_DEBUG, "跳过内存整理 [Skipped compact]");
    }

    log_msg(LOG_INFO, "弱隐 BL 完成 [Weak bootloader hiding done]: %d 成功 [success], %d 失败 [fail], %d 跳过 [skipped]", success, fail, skipped);

    return (fail > 0) ? -1 : 0;
}
