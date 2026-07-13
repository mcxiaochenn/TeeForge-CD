#ifndef TEEFORGE_H
#define TEEFORGE_H

#include <stdint.h>

/* ===== 版本 Version ===== */
#define TEEFORGE_VERSION "0.1.0"

/* ===== 默认路径（可通过 config.conf 覆盖）===== */
/* ===== Default Paths (overridable via config.conf) ===== */
#define DEFAULT_PACKAGES    "/data/system/packages.xml"
#define DEFAULT_TARGET_TXT  "/data/adb/tricky_store/target.txt"
#define DEFAULT_KEYBOX_DIR  "/data/adb/teeforge/keybox/"
#define DEFAULT_SOURCES     "/data/adb/teeforge/sources.conf"
#define DEFAULT_LOG_FILE    "/data/adb/teeforge/teeforge.log"

#define CONFIG_FILE         "./config.conf"

/* ===== 限制 Limits ===== */
#define MAX_LINE         4096
#define MAX_PACKAGES     2048
#define MAX_PATH_LEN     256
#define MAX_PKG_NAME     256
#define MAX_MIRRORS      8
#define MAX_URL_LEN      512

/* ===== 地区 Region ===== */
typedef enum {
    REGION_AUTO   = 0,  /* 自动检测 Auto-detect */
    REGION_CN     = 1,  /* 中国大陆 China mainland */
    REGION_GLOBAL = 2   /* 海外 Overseas */
} region_t;

/* ===== 配置结构 Configuration ===== */
typedef struct {
    char packages_xml[MAX_PATH_LEN];  /* 包数据库路径 Package database path */
    char target_txt[MAX_PATH_LEN];    /* 目标文件路径 Target file path */
    char keybox_dir[MAX_PATH_LEN];    /* Keybox 目录 Keybox directory */
    char sources_conf[MAX_PATH_LEN];  /* 源配置文件 Source config file */
    char log_file[MAX_PATH_LEN];      /* 日志文件 Log file */
    region_t region;                  /* 地区 Region */
    int retry_count;                  /* 重试次数 Retry count */
    int speed_test_size;              /* 测速文件大小 Speed test size */
    char cn_mirrors[MAX_MIRRORS][MAX_URL_LEN];  /* 中国镜像站 China mirrors */
    int cn_mirror_count;              /* 镜像站数量 Mirror count */
} config_t;

/* 全局配置实例 Global config instance */
extern config_t g_config;

/* 加载配置文件，缺失项填充默认值 */
/* Load config from file, fill defaults for missing keys */
int config_load(const char *path);

/* ===== 日志系统 Logging (utils.c) ===== */
typedef enum {
    LOG_DEBUG = 0,  /* 调试 Debug */
    LOG_INFO  = 1,  /* 信息 Info */
    LOG_WARN  = 2,  /* 警告 Warning */
    LOG_ERROR = 3   /* 错误 Error */
} log_level_t;

void log_set_level(log_level_t level);
void log_msg(log_level_t level, const char *fmt, ...);

/* ===== 文件操作 File I/O (utils.c) ===== */
int file_exists(const char *path);      /* 检查文件是否存在 Check if file exists */
int dir_exists(const char *path);       /* 检查目录是否存在 Check if directory exists */
int ensure_dir(const char *path);       /* 确保目录存在 Ensure directory exists */
char *read_file(const char *path, size_t *out_len);  /* 读取文件 Read file */
int write_file(const char *path, const char *data, size_t len);  /* 写入文件 Write file */
int append_file(const char *path, const char *data);  /* 追加文件 Append to file */

/* ===== 字符串工具 String Helpers (utils.c) ===== */
char *str_trim(char *s);                /* 去除首尾空白 Trim whitespace */
int str_starts_with(const char *s, const char *prefix);  /* 前缀检查 Check prefix */
int str_contains(const char *s, const char *sub);        /* 包含检查 Contains check */
char *str_dup_range(const char *start, const char *end); /* 复制范围 Duplicate range */

/* ===== 目标管理 Target Management (target.c) ===== */
int target_generate(void);  /* 生成 target.txt Generate target.txt */

/* ===== 弱隐 BL Weak Bootloader Hiding (blhide.c) ===== */
int bl_hide(void);  /* 执行弱隐 BL Execute weak bootloader hiding */

/* ===== Keybox 管理 Keybox Management (keybox.c) ===== */
int keybox_fetch(void);  /* 获取并更新 keybox Fetch and update keybox */

/* ===== 下载模块 Download Module (download.c) ===== */
/* 检测地区 Detect region */
int dl_detect_region(void);
/* 测速并获取最快镜像 Speed test and get fastest mirror */
int dl_speed_test(const char *test_url);
/* 下载文件 Download file */
char *dl_download(const char *url, size_t *out_len);
/* 带重试的下载 Download with retry */
char *dl_download_with_retry(const char *url, size_t *out_len);
/* 获取镜像 URL（根据地区）Get mirror URL based on region */
int dl_get_mirror_url(const char *original_url, char *mirror_url, size_t size);

#endif /* TEEFORGE_H */