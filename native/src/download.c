#include "teeforge.h"
#include <unistd.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <errno.h>

/* ===== 下载模块 Download Module ===== */

/* 检测地区 Detect region */
int dl_detect_region(void) {
    if (g_config.region != REGION_AUTO) {
        return g_config.region;
    }

    log_msg(LOG_INFO, "请选择地区 [Please select region]:");
    log_msg(LOG_INFO, "  音量+ = 中国大陆 [Volume+ = China]");
    log_msg(LOG_INFO, "  音量- = 海外直连 [Volume- = Global]");

    /* 音量键选择 Volume key selection */
    int selected = volume_listen(10);

    if (selected >= 0) {
        g_config.region = selected;
    } else {
        log_msg(LOG_INFO, "超时，默认海外 [Timeout, defaulting to Global]");
        g_config.region = REGION_GLOBAL;
    }

    if (g_config.region == REGION_CN) {
        log_msg(LOG_INFO, "已选择 [Selected]: 中国大陆 [China]");
    } else {
        log_msg(LOG_INFO, "已选择 [Selected]: 海外 [Global]");
    }

    return g_config.region;
}

/* 测速 Speed test */
static long speed_test_url(const char *url) {
    char cmd[1024];
    char result[64];
    FILE *fp;

    snprintf(cmd, sizeof(cmd),
        "curl -sL --connect-timeout 3 --max-time 10 "
        "-o /dev/null -w '%%{time_total}' '%s' 2>/dev/null",
        url);

    fp = popen(cmd, "r");
    if (!fp) return -1;

    if (fgets(result, sizeof(result), fp)) {
        pclose(fp);
        double seconds = atof(result);
        return (long)(seconds * 1000);
    }

    pclose(fp);
    return -1;
}

/* 测速并选择最快镜像 Speed test and select fastest mirror */
int dl_speed_test(const char *test_url) {
    log_msg(LOG_INFO, "测速中 [Testing speed]...");

    if (g_config.region != REGION_CN) {
        log_msg(LOG_INFO, "海外用户，跳过镜像测速 [Global user, skip mirror test]");
        return -1;
    }

    /* 测试直连 Test direct */
    log_msg(LOG_INFO, "  测试 GitHub 直连 [Testing GitHub direct]...");
    long best_time = speed_test_url(test_url);
    int best_mirror = -1;

    if (best_time > 0) {
        log_msg(LOG_INFO, "  GitHub 直连 [Direct]: %ld ms", best_time);
    } else {
        log_msg(LOG_INFO, "  GitHub 直连 [Direct]: 超时 [timeout]");
    }

    /* 测试每个镜像 Test each mirror */
    for (int i = 0; i < g_config.cn_mirror_count && i < MAX_MIRRORS; i++) {
        if (g_config.cn_mirrors[i][0] == '\0') continue;

        char mirror_url[MAX_URL_LEN + 128];
        snprintf(mirror_url, sizeof(mirror_url), "%s/%s",
                 g_config.cn_mirrors[i], test_url);

        log_msg(LOG_INFO, "  测试镜像 [Testing mirror]: %s", g_config.cn_mirrors[i]);
        long time = speed_test_url(mirror_url);

        if (time > 0) {
            log_msg(LOG_INFO, "    结果 [Result]: %ld ms", time);
            if (best_time <= 0 || time < best_time) {
                best_time = time;
                best_mirror = i;
            }
        } else {
            log_msg(LOG_INFO, "    结果 [Result]: 超时 [timeout]");
        }
    }

    if (best_mirror >= 0) {
        log_msg(LOG_INFO, "最快 [Fastest]: %s (%ld ms)",
                g_config.cn_mirrors[best_mirror], best_time);
    } else {
        log_msg(LOG_INFO, "最快 [Fastest]: GitHub 直连 (%ld ms)", best_time);
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
    if (g_config.region != REGION_CN) {
        strncpy(mirror_url, original_url, size - 1);
        return 0;
    }

    int best = dl_speed_test(original_url);

    if (best >= 0 && best < g_config.cn_mirror_count) {
        snprintf(mirror_url, size, "%s/%s",
                 g_config.cn_mirrors[best], original_url);
        log_msg(LOG_INFO, "使用镜像 [Using mirror]: %s", mirror_url);
    } else {
        strncpy(mirror_url, original_url, size - 1);
        log_msg(LOG_INFO, "使用直连 [Using direct]");
    }

    return 0;
}

/* 带重试的下载 Download with retry */
char *dl_download_with_retry(const char *url, size_t *out_len) {
    int retry_count = g_config.retry_count > 0 ? g_config.retry_count : 3;
    char mirror_url[MAX_URL_LEN];

    dl_get_mirror_url(url, mirror_url, sizeof(mirror_url));

    for (int attempt = 1; attempt <= retry_count; attempt++) {
        log_msg(LOG_INFO, "下载 [%d/%d] [Download attempt]: %s",
                attempt, retry_count, mirror_url);

        size_t len = 0;
        char *data = dl_download(mirror_url, &len);

        if (data && len > 0) {
            log_msg(LOG_INFO, "下载成功 [Download success]: %zu bytes", len);
            if (out_len) *out_len = len;
            return data;
        }

        log_msg(LOG_WARN, "下载失败 [Download failed], 重试中 [retrying]...");

        if (g_config.region == REGION_CN) {
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

        if (attempt < retry_count) {
            sleep(2);
        }
    }

    log_msg(LOG_ERROR, "所有重试失败 [All retries failed]");
    return NULL;
}
