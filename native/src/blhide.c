#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>

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
 * - 属性删除 [Property deletion]
 */

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

/* 标准 resetprop 固定路径 Standard resetprop fixed paths */
static const char *std_resetprop_paths[] = {
    "/data/adb/ksu/bin/resetprop",
    "/data/adb/ap/bin/resetprop",
    "/data/adb/magisk/resetprop",
    NULL
};

/* 查找可用的标准 resetprop Find available standard resetprop */
static const char *find_std_resetprop(void) {
    for (int i = 0; std_resetprop_paths[i] != NULL; i++) {
        if (is_executable(std_resetprop_paths[i])) return std_resetprop_paths[i];
    }
    return NULL;
}

/* 检测 resetprop 工具类型并缓存命令路径 Detect and cache resetprop command */
static void detect_prop_tool(void) {
    const char *found = find_std_resetprop();
    g_resetprop_cmd = found ? found : "resetprop";
}

/* 获取 resetprop 命令路径（已缓存）Get resetprop command path (cached) */
static const char *get_resetprop_cmd(void) {
    return g_resetprop_cmd ? g_resetprop_cmd : "resetprop";
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

/*
 * bl_build_script — 构建批量 resetprop 脚本 Build batch resetprop script
 * 将所有属性设置命令拼成一个 shell 脚本，一次 system() 执行
 * Concatenates all property commands into one shell script, single system() call
 * 返回脚本字符串（调用方 free），失败返回 NULL
 * Returns script string (caller frees), NULL on failure
 */
static char *bl_build_script(void) {
    const char *cmd_path = get_resetprop_cmd();

    /* 估算大小：每行约 100 字节，最多 30 属性 + 删除 */
    /* Estimate: ~100 bytes per line, max 30 props + delete */
    size_t buf_sz = 4096;
    char *script = malloc(buf_sz);
    if (!script) return NULL;

    size_t pos = 0;
    pos += snprintf(script + pos, buf_sz - pos, "#!/bin/sh\n");

    /* 属性设置 Property sets */
    for (int i = 0; bl_props[i].key != NULL; i++) {
        if (!is_category_enabled(bl_props[i].category)) continue;

        log_msg(LOG_DEBUG, "  %s %s %s", cmd_path, bl_props[i].key, bl_props[i].value);

        if (pos < buf_sz - 200) {
            pos += snprintf(script + pos, buf_sz - pos,
                "%s %s %s\n", cmd_path, bl_props[i].key, bl_props[i].value);
        }
    }

    /* 属性删除 Property deletes */
    if (g_config.blhide && g_config.blhide_delete) {
        for (int i = 0; del_props[i] != NULL; i++) {
            log_msg(LOG_DEBUG, "删除属性 [Delete property]: %s", del_props[i]);
            if (pos < buf_sz - 100) {
                pos += snprintf(script + pos, buf_sz - pos,
                    "%s --delete %s\n", cmd_path, del_props[i]);
            }
        }
    }

    return script;
}


int bl_hide(void) {
    log_msg(LOG_INFO, "开始弱隐 BL [Starting weak bootloader hiding]...");

    /* 检测 resetprop 工具 Detect resetprop tool */
    detect_prop_tool();

    /* 确认 boot 已完成（service.sh 已等待，此处仅做二次确认） */
    /* Confirm boot completed (service.sh already waited, double-check here) */
    log_msg(LOG_DEBUG, "确认 boot 完成 [Confirming boot completed]...");
    int boot_wait = 0;
    while (boot_wait < 30) {
        /* 用 popen 替代 system("getprop | grep")，减少 fork */
        /* Use popen instead of system("getprop | grep"), fewer forks */
        FILE *fp = popen("getprop sys.boot_completed", "r");
        if (fp) {
            char val[8] = {0};
            if (fgets(val, sizeof(val), fp) && val[0] == '1') {
                pclose(fp);
                break;
            }
            pclose(fp);
        }
        boot_wait++;
        sleep(1);
    }
    if (boot_wait >= 30) {
        log_msg(LOG_WARN, "等待 boot 超时 [Boot wait timeout]");
    }

    /* 构建批量脚本并一次性执行 Build batch script and execute in one call */
    char *script = bl_build_script();
    if (!script) {
        log_msg(LOG_ERROR, "无法构建属性脚本 [Failed to build property script]");
        return -1;
    }

    log_msg(LOG_INFO, "执行批量属性设置 [Executing batch property set]...");
    int ret = system(script);
    free(script);

    if (ret != 0) {
        log_msg(LOG_WARN, "批量属性执行返回 %d [Batch property execution returned %d]", ret, ret);
    }

    log_msg(LOG_INFO, "弱隐 BL 完成 [Weak bootloader hiding done]");

    return (ret != 0) ? -1 : 0;
}
