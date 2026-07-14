/*
 * rootdetect.c — Root 方式检测 Root method detection
 * 参考 Zygisk-Next customize.sh 的环境变量检测逻辑
 */

#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* 环境变量检测 Detect via environment variables (set by root manager app) */
static int detect_by_env(char *method, size_t mlen, char *version, size_t vlen) {
    /* KernelSU: $KSU is set by KSU app during install/runtime */
    if (getenv("KSU") != NULL) {
        snprintf(method, mlen, "KernelSU");
        const char *ver = getenv("KSU_VER_CODE");
        snprintf(version, vlen, "%s", (ver && ver[0]) ? ver : "unknown");
        return 1;
    }

    /* APatch: $APATCH is set by APatch app */
    if (getenv("APATCH") != NULL) {
        snprintf(method, mlen, "APatch");
        const char *ver = getenv("APATCH_VER_CODE");
        snprintf(version, vlen, "%s", (ver && ver[0]) ? ver : "unknown");
        return 1;
    }

    /* Magisk: $MAGISK_VER_CODE is set by Magisk */
    if (getenv("MAGISK_VER_CODE") != NULL) {
        snprintf(method, mlen, "Magisk");
        const char *ver = getenv("MAGISK_VER_CODE");
        snprintf(version, vlen, "%s", (ver && ver[0]) ? ver : "unknown");
        return 1;
    }

    return 0;
}

/* 文件系统路径检测 Detect via filesystem paths (runtime fallback) */
static int detect_by_path(char *method, size_t mlen, char *version, size_t vlen) {
    /* KernelSU: /data/adb/ksu/ exists */
    if (dir_exists("/data/adb/ksu")) {
        snprintf(method, mlen, "KernelSU");
        snprintf(version, vlen, "unknown");
        return 1;
    }

    /* APatch: /data/adb/ap/ exists */
    if (dir_exists("/data/adb/ap")) {
        snprintf(method, mlen, "APatch");
        snprintf(version, vlen, "unknown");
        return 1;
    }

    /* Magisk: /data/adb/magisk/ exists */
    if (dir_exists("/data/adb/magisk")) {
        snprintf(method, mlen, "Magisk");
        snprintf(version, vlen, "unknown");
        return 1;
    }

    return 0;
}

int root_detect(char *method, size_t mlen, char *version, size_t vlen) {
    if (!method || mlen == 0) return -1;

    /* 优先环境变量 Prefer environment variables (more reliable) */
    if (detect_by_env(method, mlen, version, vlen)) {
        log_msg(LOG_DEBUG, "通过环境变量检测到 root [Detected root via env]: %s (v%s)", method, version);
        return 0;
    }

    /* 回退到路径检测 Fallback to path detection */
    if (detect_by_path(method, mlen, version, vlen)) {
        log_msg(LOG_DEBUG, "通过路径检测到 root [Detected root via path]: %s (v%s)", method, version);
        return 0;
    }

    snprintf(method, mlen, "Unknown");
    snprintf(version, vlen, "unknown");
    log_msg(LOG_WARN, "无法识别 root 方式 [Unknown root method]");
    return 1;
}

int root_detect_save(const char *config_path, const char *method) {
    if (!config_path || !method) return -1;

    /* 读取现有配置 Read existing config */
    char *data = read_file(config_path, NULL);
    if (!data) {
        /* 文件不存在，直接写入 File doesn't exist, write directly */
        return write_file(config_path, method, strlen(method));
    }

    /* 查找已有的 root_method 行 Search for existing root_method line */
    char key_pattern[] = "root_method=";
    char *pos = strstr(data, key_pattern);

    /* 构建新内容 Build new content */
    size_t new_len = strlen(data) + strlen(method) + 32;
    char *new_data = (char *)malloc(new_len);
    if (!new_data) {
        free(data);
        return -1;
    }

    if (pos) {
        /* 替换已有行 Replace existing line */
        size_t prefix_len = (size_t)(pos - data);
        char *after = strchr(pos, '\n');
        snprintf(new_data, new_len, "%.*sroot_method=%s%s",
                 (int)prefix_len, data, method, after ? after : "\n");
    } else {
        /* 追加到末尾 Append to end */
        size_t cur_len = strlen(data);
        /* 确保末尾有换行 Ensure trailing newline */
        if (cur_len > 0 && data[cur_len - 1] != '\n') {
            snprintf(new_data, new_len, "%s\nroot_method=%s\n", data, method);
        } else {
            snprintf(new_data, new_len, "%sroot_method=%s\n", data, method);
        }
    }

    int ret = write_file(config_path, new_data, strlen(new_data));
    free(new_data);
    free(data);
    return ret;
}
