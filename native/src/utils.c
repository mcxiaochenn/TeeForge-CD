#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <sys/stat.h>
#include <errno.h>
#include <time.h>

/* ===== 全局配置 Global Config ===== */
config_t g_config;

int config_load(const char *path) {
    /* 设置默认值 Set defaults */
    strncpy(g_config.packages_xml, DEFAULT_PACKAGES, MAX_PATH_LEN - 1);
    strncpy(g_config.target_txt, DEFAULT_TARGET_TXT, MAX_PATH_LEN - 1);
    strncpy(g_config.keybox_dir, DEFAULT_KEYBOX_DIR, MAX_PATH_LEN - 1);
    strncpy(g_config.sources_conf, DEFAULT_SOURCES, MAX_PATH_LEN - 1);
    strncpy(g_config.log_file, DEFAULT_LOG_FILE, MAX_PATH_LEN - 1);

    /* 尝试加载配置文件 Try to load config file */
    FILE *f = fopen(path, "r");
    if (!f) {
        log_msg(LOG_WARN, "配置文件未找到: %s, 使用默认值 [Config file not found: %s, using defaults]", path, path);
        return 0;  /* 非致命错误，使用默认值 Not fatal, use defaults */
    }

    char line[MAX_LINE];
    while (fgets(line, sizeof(line), f)) {
        char *trimmed = str_trim(line);

        /* 跳过注释和空行 Skip comments and empty lines */
        if (!trimmed || trimmed[0] == '#' || trimmed[0] == '\0') continue;

        /* 解析 key=value Parse key=value */
        char *eq = strchr(trimmed, '=');
        if (!eq) continue;

        *eq = '\0';
        char *key = str_trim(trimmed);
        char *val = str_trim(eq + 1);

        if (strcmp(key, "packages_xml") == 0) {
            strncpy(g_config.packages_xml, val, MAX_PATH_LEN - 1);
        } else if (strcmp(key, "target_txt") == 0) {
            strncpy(g_config.target_txt, val, MAX_PATH_LEN - 1);
        } else if (strcmp(key, "keybox_dir") == 0) {
            strncpy(g_config.keybox_dir, val, MAX_PATH_LEN - 1);
        } else if (strcmp(key, "sources_conf") == 0) {
            strncpy(g_config.sources_conf, val, MAX_PATH_LEN - 1);
        } else if (strcmp(key, "log_file") == 0) {
            strncpy(g_config.log_file, val, MAX_PATH_LEN - 1);
        } else {
            log_msg(LOG_WARN, "未知配置项: %s [Unknown config key: %s]", key, key);
        }
    }

    fclose(f);
    log_msg(LOG_INFO, "已加载配置: %s [Loaded config from %s]", path, path);
    return 0;
}

/* ===== 日志系统 Logging System ===== */

static log_level_t current_level = LOG_INFO;

void log_set_level(log_level_t level) {
    current_level = level;
}

void log_msg(log_level_t level, const char *fmt, ...) {
    if (level < current_level) return;

    static const char *level_str[] = { "DEBUG", "INFO", "WARN", "ERROR" };

    /* 时间戳 Timestamp */
    time_t now = time(NULL);
    struct tm *tm = localtime(&now);
    char timebuf[32];
    strftime(timebuf, sizeof(timebuf), "%Y-%m-%d %H:%M:%S", tm);

    fprintf(stderr, "[%s] [%s] ", timebuf, level_str[level]);

    va_list ap;
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);

    fprintf(stderr, "\n");
}

/* ===== 文件操作 File I/O ===== */

int file_exists(const char *path) {
    struct stat st;
    return stat(path, &st) == 0 && S_ISREG(st.st_mode);
}

int dir_exists(const char *path) {
    struct stat st;
    return stat(path, &st) == 0 && S_ISDIR(st.st_mode);
}

int ensure_dir(const char *path) {
    if (dir_exists(path)) return 0;

    /* 尝试创建目录，已存在则静默失败 Try mkdir, fail silently if exists */
    if (mkdir(path, 0755) == 0) return 0;
    if (errno == EEXIST && dir_exists(path)) return 0;

    log_msg(LOG_ERROR, "创建目录失败: %s (%s) [Failed to create directory: %s (%s)]", path, strerror(errno), path, strerror(errno));
    return -1;
}

char *read_file(const char *path, size_t *out_len) {
    FILE *f = fopen(path, "r");
    if (!f) {
        log_msg(LOG_ERROR, "无法打开文件: %s (%s) [Cannot open file: %s (%s)]", path, strerror(errno), path, strerror(errno));
        return NULL;
    }

    /* 获取文件大小 Get file size */
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);

    if (size <= 0) {
        fclose(f);
        return NULL;
    }

    char *buf = malloc(size + 1);
    if (!buf) {
        fclose(f);
        return NULL;
    }

    size_t read = fread(buf, 1, size, f);
    fclose(f);

    buf[read] = '\0';
    if (out_len) *out_len = read;
    return buf;
}

int write_file(const char *path, const char *data, size_t len) {
    FILE *f = fopen(path, "w");
    if (!f) {
        log_msg(LOG_ERROR, "无法写入文件: %s (%s) [Cannot write file: %s (%s)]", path, strerror(errno), path, strerror(errno));
        return -1;
    }

    size_t written = fwrite(data, 1, len, f);
    fclose(f);

    if (written != len) {
        log_msg(LOG_ERROR, "写入不完整: %s (%zu/%zu 字节) [Short write: %s (%zu/%zu bytes)]", path, written, len, path, written, len);
        return -1;
    }

    return 0;
}

int append_file(const char *path, const char *data) {
    FILE *f = fopen(path, "a");
    if (!f) {
        log_msg(LOG_ERROR, "无法追加文件: %s (%s) [Cannot append file: %s (%s)]", path, strerror(errno), path, strerror(errno));
        return -1;
    }

    fputs(data, f);
    fclose(f);
    return 0;
}

/* ===== 字符串工具 String Helpers ===== */

char *str_trim(char *s) {
    if (!s) return NULL;
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    char *end = s + strlen(s) - 1;
    while (end > s && (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r')) *end-- = '\0';
    return s;
}

int str_starts_with(const char *s, const char *prefix) {
    return strncmp(s, prefix, strlen(prefix)) == 0;
}

int str_contains(const char *s, const char *sub) {
    return strstr(s, sub) != NULL;
}

char *str_dup_range(const char *start, const char *end) {
    size_t len = end - start;
    char *s = malloc(len + 1);
    if (!s) return NULL;
    memcpy(s, start, len);
    s[len] = '\0';
    return s;
}