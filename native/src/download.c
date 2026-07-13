#include "teeforge.h"
#include <unistd.h>
#include <sys/wait.h>

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

    log_msg(LOG_INFO, "");
    log_msg(LOG_INFO, "请选择地区 [Please select region]:");
    log_msg(LOG_INFO, "  音量+ = 中国大陆 [Volume+ = China]");
    log_msg(LOG_INFO, "  音量- = 海外直连 [Volume- = Global]");

    /* 音量键选择，输出 1/0 Volume key selection, output 1/0 */
    int result = volume_listen(10);

    if (result == 1) {
        g_config.region = REGION_CN;
        log_msg(LOG_INFO, "已选择 [Selected]: 中国大陆 [China]");
    } else if (result == 0) {
        g_config.region = REGION_GLOBAL;
        log_msg(LOG_INFO, "已选择 [Selected]: 海外 [Global]");
    } else {
        g_config.region = REGION_GLOBAL;
        log_msg(LOG_INFO, "超时，默认海外 [Timeout, defaulting to Global]");
    }
    log_msg(LOG_INFO, "");

    return g_config.region;
}

/* 并行测速结果 Parallel speed test result */
typedef struct {
    char name[128];
    long time_ms;
    int index; /* -1 = direct, 0+ = mirror index */
} speed_result_t;

/* 测速子进程 Speed test child process */
static void speed_test_child(const char *url, int write_fd) {
    char cmd[1024];
    char result[64] = {0};

    snprintf(cmd, sizeof(cmd),
        "curl -sL --connect-timeout 3 --max-time 8 "
        "-o /dev/null -w '%%{time_total}' '%s' 2>/dev/null",
        url);

    FILE *fp = popen(cmd, "r");
    if (fp) {
        fgets(result, sizeof(result), fp);
        pclose(fp);
    }

    /* 写入结果 Write result */
    long ms = (long)(atof(result) * 1000);
    write(write_fd, &ms, sizeof(ms));
    close(write_fd);
    _exit(0);
}

/* 测速并选择最快镜像（并行） Speed test and select fastest mirror (parallel) */
int dl_speed_test(const char *test_url) {
    log_msg(LOG_INFO, "");
    log_msg(LOG_INFO, "测速中 [Testing speed] (parallel)...");

    if (g_config.region != REGION_CN) {
        log_msg(LOG_INFO, "海外用户，跳过镜像测速 [Global user, skip mirror test]");
        return -1;
    }

    /* 准备所有测试 URL Prepare all test URLs */
    char urls[MAX_MIRRORS + 1][MAX_URL_LEN + 128];
    char names[MAX_MIRRORS + 1][128];
    int indices[MAX_MIRRORS + 1]; /* -1=direct, 0+=mirror */
    int total = 0;

    /* 直连 Direct */
    strncpy(urls[total], test_url, sizeof(urls[0]) - 1);
    strncpy(names[total], "GitHub 直连 [Direct]", sizeof(names[0]) - 1);
    indices[total] = -1;
    total++;

    /* 镜像 Mirrors */
    for (int i = 0; i < g_config.cn_mirror_count && i < MAX_MIRRORS; i++) {
        if (g_config.cn_mirrors[i][0] == '\0') continue;
        snprintf(urls[total], sizeof(urls[0]), "%s/%s",
                 g_config.cn_mirrors[i], test_url);
        strncpy(names[total], g_config.cn_mirrors[i], sizeof(names[0]) - 1);
        indices[total] = i;
        total++;
    }

    /* 创建管道和子进程 Create pipes and child processes */
    int pipes[MAX_MIRRORS + 1][2];
    pid_t pids[MAX_MIRRORS + 1];

    for (int i = 0; i < total; i++) {
        if (pipe(pipes[i]) != 0) {
            pids[i] = -1;
            continue;
        }
        pid_t pid = fork();
        if (pid == 0) {
            /* 子进程 Child */
            close(pipes[i][0]);
            speed_test_child(urls[i], pipes[i][1]);
        } else {
            /* 父进程 Parent */
            close(pipes[i][1]);
            pids[i] = pid;
        }
    }

    /* 收集结果 Collect results */
    speed_result_t results[MAX_MIRRORS + 1];
    int result_count = 0;

    for (int i = 0; i < total; i++) {
        long ms = -1;
        if (pids[i] > 0) {
            read(pipes[i][0], &ms, sizeof(ms));
            close(pipes[i][0]);
            waitpid(pids[i], NULL, 0);
        }

        if (ms > 0) {
            strncpy(results[result_count].name, names[i], sizeof(results[0].name) - 1);
            results[result_count].time_ms = ms;
            results[result_count].index = indices[i];
            result_count++;
            log_msg(LOG_INFO, "  %s: %ld ms", names[i], ms);
        } else {
            log_msg(LOG_INFO, "  %s: 超时 [timeout]", names[i]);
        }
    }

    /* 找最快 Find fastest */
    int best_mirror = -1;
    long best_time = -1;

    for (int i = 0; i < result_count; i++) {
        if (best_time < 0 || results[i].time_ms < best_time) {
            best_time = results[i].time_ms;
            best_mirror = results[i].index;
        }
    }

    if (best_mirror >= 0) {
        log_msg(LOG_INFO, "最快 [Fastest]: %s (%ld ms)",
                g_config.cn_mirrors[best_mirror], best_time);
    } else if (best_time > 0) {
        log_msg(LOG_INFO, "最快 [Fastest]: GitHub 直连 (%ld ms)", best_time);
    } else {
        log_msg(LOG_INFO, "最快 [Fastest]: 全部超时 [All timeout]");
    }
    log_msg(LOG_INFO, "");

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
    log_msg(LOG_INFO, "");
    log_msg(LOG_INFO, "开始下载 [Starting download]...");

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
            log_msg(LOG_INFO, "");
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
