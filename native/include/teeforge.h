#ifndef TEEFORGE_H
#define TEEFORGE_H

#include <stdint.h>

/* ===== 版本 Version ===== */
#define TEEFORGE_VERSION "0.1.0"

/* ===== 默认路径 Default Paths ===== */
#define DEFAULT_PACKAGES    "/data/system/packages.xml"
#define DEFAULT_TARGET_TXT  "/data/adb/tricky_store/target.txt"
#define DEFAULT_KEYBOX_DIR  "/data/adb/teeforge/keybox/"
#define DEFAULT_SOURCES     "/data/adb/teeforge/sources.conf"
#define DEFAULT_LOG_DIR     "/data/adb/teeforge/logs/"

#define CONFIG_FILE         "./config.conf"

/* ===== 限制 Limits ===== */
#define MAX_LINE         4096
#define MAX_PACKAGES     2048
#define MAX_PATH_LEN     256
#define MAX_PKG_NAME     256

/* ===== 配置结构 Configuration ===== */
typedef struct {
    char packages_xml[MAX_PATH_LEN];
    char target_txt[MAX_PATH_LEN];
    char keybox_dir[MAX_PATH_LEN];
    char sources_conf[MAX_PATH_LEN];
    char log_dir[MAX_PATH_LEN];
    int debug;                         /* 调试开关 Debug switch */
} config_t;

/* 全局配置实例 Global config instance */
extern config_t g_config;

/* 加载配置文件 Load config from file */
int config_load(const char *path);

/* ===== 日志系统 Logging (utils.c) ===== */
typedef enum {
    LOG_DEBUG = 0,
    LOG_INFO  = 1,
    LOG_WARN  = 2,
    LOG_ERROR = 3
} log_level_t;

void log_set_level(log_level_t level);
void log_msg(log_level_t level, const char *fmt, ...);

/* ===== 文件操作 File I/O (utils.c) ===== */
int file_exists(const char *path);
int dir_exists(const char *path);
int ensure_dir(const char *path);
char *read_file(const char *path, size_t *out_len);
int write_file(const char *path, const char *data, size_t len);
int append_file(const char *path, const char *data);

/* ===== 字符串工具 String Helpers (utils.c) ===== */
char *str_trim(char *s);
int str_starts_with(const char *s, const char *prefix);
int str_contains(const char *s, const char *sub);
char *str_dup_range(const char *start, const char *end);

/* ===== 目标管理 Target Management (target.c) ===== */
int target_generate(void);

/* ===== 弱隐 BL Weak Bootloader Hiding (blhide.c) ===== */
int bl_hide(void);

/* ===== Keybox 管理 Keybox Management (keybox.c) ===== */
int keybox_fetch(void);

/* ===== 音量键监听 Volume Key Listener (volume.c) ===== */
int volume_listen(int timeout_sec);

#endif /* TEEFORGE_H */
