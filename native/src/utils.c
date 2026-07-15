#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <sys/stat.h>
#include <errno.h>
#include <time.h>
#include <unistd.h>

/* ===== 全局配置 Global Config ===== */
config_t g_config;

/* 解析单个配置文件 Parse a single config file */
static void config_parse_file(const char *path) {
    FILE *f = fopen(path, "r");
    if (!f) return;

    char line[MAX_LINE];
    while (fgets(line, sizeof(line), f)) {
        char *trimmed = str_trim(line);
        if (!trimmed || trimmed[0] == '#' || trimmed[0] == '\0') continue;

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
        } else if (strcmp(key, "log_dir") == 0) {
            strncpy(g_config.log_dir, val, MAX_PATH_LEN - 1);
        } else if (strcmp(key, "root_method") == 0) {
            strncpy(g_config.root_method, val, 63);
        } else if (strcmp(key, "root_version") == 0) {
            strncpy(g_config.root_version, val, 31);
        } else if (strcmp(key, "debug") == 0) {
            g_config.debug = atoi(val);
        } else if (strcmp(key, "blhide") == 0) {
            g_config.blhide = atoi(val);
        } else if (strcmp(key, "blhide_boot") == 0) {
            g_config.blhide_boot = atoi(val);
        } else if (strcmp(key, "blhide_security") == 0) {
            g_config.blhide_security = atoi(val);
        } else if (strcmp(key, "blhide_vendor") == 0) {
            g_config.blhide_vendor = atoi(val);
        } else if (strcmp(key, "blhide_oem") == 0) {
            g_config.blhide_oem = atoi(val);
        } else if (strcmp(key, "blhide_secureboot") == 0) {
            g_config.blhide_secureboot = atoi(val);
        } else if (strcmp(key, "blhide_realme") == 0) {
            g_config.blhide_realme = atoi(val);
        } else if (strcmp(key, "blhide_recovery") == 0) {
            g_config.blhide_recovery = atoi(val);
        } else if (strcmp(key, "blhide_developer") == 0) {
            g_config.blhide_developer = atoi(val);
        } else if (strcmp(key, "blhide_selinux") == 0) {
            g_config.blhide_selinux = atoi(val);
        } else if (strcmp(key, "blhide_virtual") == 0) {
            g_config.blhide_virtual = atoi(val);
        } else if (strcmp(key, "blhide_delete") == 0) {
            g_config.blhide_delete = atoi(val);
        } else if (strcmp(key, "blhide_compact") == 0) {
            g_config.blhide_compact = atoi(val);
        } else if (strcmp(key, "prop_tool") == 0) {
            strncpy(g_config.prop_tool, val, 15);
        }
    }

    fclose(f);
}

/* 设置弱隐 BL 默认值（全开） Set BL hide defaults (all on) */
static void blhide_set_defaults(config_t *cfg) {
    cfg->blhide = 1;
    cfg->blhide_boot = 1;
    cfg->blhide_security = 1;
    cfg->blhide_vendor = 1;
    cfg->blhide_oem = 1;
    cfg->blhide_secureboot = 1;
    cfg->blhide_realme = 1;
    cfg->blhide_recovery = 1;
    cfg->blhide_developer = 1;
    cfg->blhide_selinux = 1;
    cfg->blhide_virtual = 1;
    cfg->blhide_delete = 1;
    cfg->blhide_compact = 1;
}

int config_load(const char *path) {
    /* 设置默认值 Set defaults */
    strncpy(g_config.packages_xml, DEFAULT_PACKAGES, MAX_PATH_LEN - 1);
    strncpy(g_config.target_txt, DEFAULT_TARGET_TXT, MAX_PATH_LEN - 1);
    strncpy(g_config.keybox_dir, DEFAULT_KEYBOX_DIR, MAX_PATH_LEN - 1);
    strncpy(g_config.sources_conf, DEFAULT_SOURCES, MAX_PATH_LEN - 1);
    strncpy(g_config.log_dir, DEFAULT_LOG_DIR, MAX_PATH_LEN - 1);
    strncpy(g_config.root_method, "Unknown", 63);
    strncpy(g_config.root_version, "unknown", 31);
    g_config.debug = 0;
    strncpy(g_config.prop_tool, "standard", 15);
    blhide_set_defaults(&g_config);

    /* 加载 sys.conf（系统配置）Load sys.conf (system config) */
    config_parse_file(SYS_CONFIG_FILE);

    /* 加载 config.conf（用户配置，可覆盖 debug）Load config.conf (user config, can override debug) */
    config_parse_file(path);

    /* 确保日志目录存在 Ensure log directory exists */
    if (g_config.debug) {
        ensure_dir(g_config.log_dir);
    }

    log_msg(LOG_INFO, "已加载配置 [Loaded config]: %s + %s (debug=%d)", SYS_CONFIG_FILE, path, g_config.debug);
    return 0;
}

/* ===== 日志系统 Logging System ===== */

#define MAX_LOG_FILES 15

static log_level_t current_level = LOG_INFO;
static FILE *log_fp = NULL;
static char log_file_path[MAX_PATH_LEN] = {0};

void log_set_level(log_level_t level) {
    current_level = level;
}

/* 清理旧日志，保留最新15份 Clean old logs, keep latest 15 */
static void log_cleanup_old(void) {
    char cmd[512];
    /* 按时间排序，删除第15个以后的 */
    snprintf(cmd, sizeof(cmd),
        "ls -t %s/teeforge_*.log 2>/dev/null | tail -n +%d | xargs rm -f 2>/dev/null",
        g_config.log_dir, MAX_LOG_FILES + 1);
    system(cmd);
}

/* 打开日志文件（每次开机一个文件）Open log file (one file per boot) */
static void log_open_file(void) {
    if (log_fp) return;
    if (!g_config.debug) return;

    ensure_dir(g_config.log_dir);

    /* 文件名按日期，不按时间（同一开机会话复用） */
    /* Filename by date, not time (reuse same file per boot session) */
    time_t now = time(NULL);
    struct tm *tm = localtime(&now);

    snprintf(log_file_path, sizeof(log_file_path),
             "%s/teeforge_%04d%02d%02d.log",
             g_config.log_dir,
             tm->tm_year + 1900, tm->tm_mon + 1, tm->tm_mday);

    log_fp = fopen(log_file_path, "a");
    if (log_fp) {
        /* 每次运行用空行隔开 Separate each run with blank line */
        fprintf(log_fp, "\n=== TeeForge-CD %04d-%02d-%02d %02d:%02d:%02d ===\n",
                tm->tm_year + 1900, tm->tm_mon + 1, tm->tm_mday,
                tm->tm_hour, tm->tm_min, tm->tm_sec);
        fflush(log_fp);

        /* 清理旧日志 Clean old logs */
        log_cleanup_old();
    }
}

void log_msg(log_level_t level, const char *fmt, ...) {
    if (level < current_level) return;

    static const char *level_str[] = { "DEBUG", "INFO", "WARN", "ERROR" };

    /* 时间戳 Timestamp */
    time_t now = time(NULL);
    struct tm *tm = localtime(&now);
    char timebuf[32];
    strftime(timebuf, sizeof(timebuf), "%Y-%m-%d %H:%M:%S", tm);

    /* 输出到 stderr Output to stderr */
    fprintf(stderr, "[%s] [%s] ", timebuf, level_str[level]);

    va_list ap;
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);

    fprintf(stderr, "\n");

    /* debug 模式写入文件 Write to file in debug mode */
    if (g_config.debug) {
        log_open_file();
        if (log_fp) {
            fprintf(log_fp, "[%s] [%s] ", timebuf, level_str[level]);

            va_list ap2;
            va_start(ap2, fmt);
            vfprintf(log_fp, fmt, ap2);
            va_end(ap2);

            fprintf(log_fp, "\n");
            fflush(log_fp);
        }
    }
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

/* ===== 文件时间戳 File Timestamp ===== */

char *file_mtime_str(const char *path, char *buf, size_t len) {
    struct stat st;
    if (stat(path, &st) != 0) {
        snprintf(buf, len, "N/A");
        return buf;
    }
    struct tm *tm = localtime(&st.st_mtime);
    strftime(buf, len, "%Y-%m-%d %H:%M", tm);
    return buf;
}

/* ===== 模块描述更新 Module Description Update ===== */

int update_description(void) {
    /* 模块描述文件路径 Module prop file path */
    const char *prop_path = "/data/adb/modules/teeforge_cd/module.prop";

    if (!file_exists(prop_path)) {
        log_msg(LOG_WARN, "模块未安装，跳过描述更新 [Module not installed, skip description update]");
        return -1;
    }

    /* 架构 Architecture */
    const char *arch =
#if defined(__aarch64__)
        "arm64-v8a";
#elif defined(__arm__)
        "armeabi-v7a";
#elif defined(__x86_64__)
        "x86_64";
#elif defined(__i386__)
        "x86";
#else
        "unknown";
#endif

    /* Root 检测（重试 3 次，兼容开机早期 env 未就绪） */
    /* Root detection (retry 3x, compatible with early boot env not ready) */
    char method[64], version[32];
    snprintf(method, sizeof(method), "%s", g_config.root_method);
    snprintf(version, sizeof(version), "%s", g_config.root_version);

    for (int i = 0; i < 3; i++) {
        if (strcmp(method, "Unknown") != 0) break;
        log_msg(LOG_DEBUG, "Root 未知，重试 %d/3 [Root unknown, retry %d/3]", i + 1, i + 1);
        sleep(2);
        root_detect(method, sizeof(method), version, sizeof(version));
        if (strcmp(method, "Unknown") != 0) {
            /* 更新配置文件 Update config file */
            strncpy(g_config.root_method, method, 63);
            strncpy(g_config.root_version, version, 31);
            root_detect_save(SYS_CONFIG_FILE, method, version);
        }
    }

    /* keybox 更新时间 Keybox update time */
    char timebuf[32];
    const char *kb_time = "N/A";
    char keybox_path[MAX_PATH_LEN];
    snprintf(keybox_path, sizeof(keybox_path), "%s/keybox.xml", g_config.keybox_dir);
    if (file_exists(keybox_path)) {
        kb_time = file_mtime_str(keybox_path, timebuf, sizeof(timebuf));
    }

    /* 状态指示 + 颜文字 Status indicator + kaomoji */
    const char *kaomoji;
    if (strcmp(method, "Unknown") == 0) {
        kaomoji = "(;´д`)";
    } else if (strcmp(kb_time, "N/A") == 0) {
        kaomoji = "(・・?";
    } else {
        kaomoji = "( •̀ ω •́ )✧";
    }

    /* 拼接描述 Build description */
    char desc[512];
    snprintf(desc, sizeof(desc),
        "%s ✅ [%s] %s | arch: %s | keybox: %s",
        kaomoji,
        method,
        TEEFORGE_VERSION,
        arch,
        kb_time);

    /* 更新 module.prop 的 description 行 Update description line in module.prop */
    config_update_key(prop_path, "description", desc);

    log_msg(LOG_INFO, "模块描述已更新 [Description updated]: %s", desc);
    return 0;
}