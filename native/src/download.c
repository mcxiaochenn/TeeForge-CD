#include "teeforge.h"
#include <unistd.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>

/* ===== 下载模块 Download Module ===== */
/*
 * 功能 Features:
 * - 自动检测地区（中国大陆/海外）Auto-detect region (CN/Global)
 * - 小文件测速（避免 ping 被禁）Small file speed test (avoid ping block)
 * - 镜像站回退 Mirror fallback
 * - 失败重试 Retry on failure
 */

/* 镜像站列表 Mirror list */

/* 检测地区 Detect region */
int dl_detect_region(void) {
    if (g_config.region != REGION_AUTO) {
        return g_config.region;
    }

    log_msg(LOG_INFO, "正在检测地区 [Detecting region]...");

    /* 测试多个 GitHub 端点 Test multiple GitHub endpoints */
    /* raw.githubusercontent.com 在中国几乎必被墙 */
    const char *test_urls[] = {
        "https://raw.githubusercontent.com/robots.txt",
        "https://gist.github.com/robots.txt",
        NULL
    };

    int accessible = 0;
    for (int i = 0; test_urls[i] != NULL; i++) {
        char cmd[256];
        snprintf(cmd, sizeof(cmd),
            "curl -sL --connect-timeout 2 --max-time 3 "
            "-o /dev/null -w '%%{http_code}' '%s' 2>/dev/null",
            test_urls[i]);

        FILE *fp = popen(cmd, "r");
        if (fp) {
            char code[8] = {0};
            if (fgets(code, sizeof(code), fp)) {
                /* 200/301/302 都算可达 */
                if (atoi(code) > 0 && atoi(code) < 400) {
                    accessible = 1;
                }
            }
            pclose(fp);
        }

        if (accessible) break;
    }

    if (accessible) {
        log_msg(LOG_INFO, "检测结果 [Region detected]: 海外 [Global]");
        g_config.region = REGION_GLOBAL;
    } else {
        log_msg(LOG_INFO, "检测结果 [Region detected]: 中国大陆 [China]");
        g_config.region = REGION_CN;
    }

    return g_config.region;
}

/* 测速 Speed test */
static long speed_test_url(const char *url) {
    char cmd[1024];
    char result[64];
    FILE *fp;

    /* 使用 curl 测量下载时间 Use curl to measure download time */
    snprintf(cmd, sizeof(cmd),
        "curl -sL --connect-timeout 3 --max-time 10 "
        "-o /dev/null -w '%%{time_total}' '%s' 2>/dev/null",
        url);

    fp = popen(cmd, "r");
    if (!fp) return -1;

    if (fgets(result, sizeof(result), fp)) {
        pclose(fp);
        /* 转换为毫秒 Convert to milliseconds */
        double seconds = atof(result);
        return (long)(seconds * 1000);
    }

    pclose(fp);
    return -1;
}

/* 测速并选择最快镜像 Speed test and select fastest mirror */
int dl_speed_test(const char *test_url) {
    log_msg(LOG_INFO, "开始测速 [Starting speed test]...");

    if (g_config.region != REGION_CN) {
        log_msg(LOG_DEBUG, "非中国大陆，跳过镜像测速 [Not CN, skip mirror test]");
        return 0;
    }

    /* 测试 GitHub 直连 Test direct GitHub */
    long best_time = speed_test_url(test_url);
    int best_mirror = -1;  /* -1 = 直连 */

    log_msg(LOG_DEBUG, "GitHub 直连 [Direct]: %ld ms", best_time);

    /* 测试每个镜像站 Test each mirror */
    for (int i = 0; i < g_config.cn_mirror_count && i < MAX_MIRRORS; i++) {
        if (g_config.cn_mirrors[i][0] == '\0') continue;

        /* 构建镜像测试 URL Build mirror test URL */
        char mirror_url[MAX_URL_LEN + 128];
        snprintf(mirror_url, sizeof(mirror_url), "%s/%s",
                 g_config.cn_mirrors[i], test_url);

        long time = speed_test_url(mirror_url);
        log_msg(LOG_DEBUG, "镜像 [Mirror] %s: %ld ms",
                g_config.cn_mirrors[i], time);

        if (time > 0 && (best_time <= 0 || time < best_time)) {
            best_time = time;
            best_mirror = i;
        }
    }

    if (best_mirror >= 0) {
        log_msg(LOG_INFO, "最快镜像 [Fastest mirror]: %s (%ld ms)",
                g_config.cn_mirrors[best_mirror], best_time);
    } else {
        log_msg(LOG_INFO, "直连最快 [Direct fastest]: %ld ms", best_time);
    }

    return best_mirror;
}

/* 下载文件 Download file */
char *dl_download(const char *url, size_t *out_len) {
    char cmd[1024];
    FILE *fp;
    char *data = NULL;
    size_t len = 0;
    size_t capacity = 65536;
    char buf[4096];

    data = malloc(capacity);
    if (!data) return NULL;

    snprintf(cmd, sizeof(cmd), "curl -sL --connect-timeout 10 --max-time 60 '%s'", url);
    fp = popen(cmd, "r");
    if (!fp) {
        free(data);
        return NULL;
    }

    while (1) {
        size_t n = fread(buf, 1, sizeof(buf), fp);
        if (n == 0) break;

        if (len + n > capacity) {
            capacity *= 2;
            char *new_data = realloc(data, capacity);
            if (!new_data) {
                free(data);
                pclose(fp);
                return NULL;
            }
            data = new_data;
        }

        memcpy(data + len, buf, n);
        len += n;
    }

    pclose(fp);

    if (len == 0) {
        free(data);
        return NULL;
    }

    data[len] = '\0';
    if (out_len) *out_len = len;
    return data;
}

/* 获取镜像 URL Get mirror URL */
int dl_get_mirror_url(const char *original_url, char *mirror_url, size_t size) {
    /* 非中国大陆直接返回原 URL */
    if (g_config.region != REGION_CN) {
        strncpy(mirror_url, original_url, size - 1);
        return 0;
    }

    /* 测速选择最快镜像 Speed test to select fastest mirror */
    int best = dl_speed_test(original_url);

    if (best >= 0 && best < g_config.cn_mirror_count) {
        /* 使用镜像 Use mirror */
        snprintf(mirror_url, size, "%s/%s",
                 g_config.cn_mirrors[best], original_url);
        log_msg(LOG_INFO, "使用镜像 [Using mirror]: %s", mirror_url);
    } else {
        /* 使用直连 Use direct */
        strncpy(mirror_url, original_url, size - 1);
        log_msg(LOG_INFO, "使用直连 [Using direct]");
    }

    return 0;
}

/* 带重试的下载 Download with retry */
char *dl_download_with_retry(const char *url, size_t *out_len) {
    int retry_count = g_config.retry_count > 0 ? g_config.retry_count : 3;
    char mirror_url[MAX_URL_LEN];

    /* 获取镜像 URL（根据地区）Get mirror URL (based on region) */
    dl_get_mirror_url(url, mirror_url, sizeof(mirror_url));

    for (int attempt = 1; attempt <= retry_count; attempt++) {
        log_msg(LOG_INFO, "下载尝试 [%d/%d] [Download attempt]: %s",
                attempt, retry_count, mirror_url);

        size_t len = 0;
        char *data = dl_download(mirror_url, &len);

        if (data && len > 0) {
            log_msg(LOG_INFO, "下载成功 [Download success]: %zu bytes", len);
            if (out_len) *out_len = len;
            return data;
        }

        log_msg(LOG_WARN, "下载失败 [Download failed], 重试中 [retrying]...");

        /* 如果是镜像失败，尝试下一个镜像 If mirror fails, try next */
        if (g_config.region == REGION_CN) {
            /* 尝试下一个镜像 Try next mirror */
            for (int i = 0; i < g_config.cn_mirror_count; i++) {
                if (g_config.cn_mirrors[i][0] == '\0') continue;

                snprintf(mirror_url, sizeof(mirror_url), "%s/%s",
                         g_config.cn_mirrors[i], url);

                log_msg(LOG_DEBUG, "尝试镜像 [Trying mirror]: %s", mirror_url);
                data = dl_download(mirror_url, &len);

                if (data && len > 0) {
                    log_msg(LOG_INFO, "镜像下载成功 [Mirror download success]: %zu bytes", len);
                    if (out_len) *out_len = len;
                    return data;
                }
            }
        }

        /* 短暂等待后重试 Brief wait before retry */
        if (attempt < retry_count) {
            sleep(2);
        }
    }

    log_msg(LOG_ERROR, "所有重试失败 [All retries failed]");
    return NULL;
}
