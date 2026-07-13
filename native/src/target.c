#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>

/* ===== 包列表解析器 Package List Parser ===== */
/*
 * 使用 'cmd package list packages -f' 获取已安装包列表
 * Uses 'cmd package list packages -f' to get installed packages.
 * 输出格式 Output format: package:/path/to/base.apk=package.name
 * 用户安装应用 User-installed apps: path starts with /data/app/
 * 系统应用 System apps: /system/, /vendor/, /product/, /apex/, etc.
 */

/* 检查是否为用户安装应用 Check if codePath is a user-installed app */
static int is_user_app(const char *code_path) {
    /* 用户应用路径 User apps path: /data/app/... */
    if (str_starts_with(code_path, "/data/app/")) {
        return 1;
    }
    return 0;
}

int target_generate(void) {
    log_msg(LOG_INFO, "正在获取已安装包列表... [Listing installed packages...]");

    /* 运行包管理命令 Run package list command */
    FILE *fp = popen("cmd package list packages -f", "r");
    if (!fp) {
        log_msg(LOG_ERROR, "运行 'cmd package list' 失败 [Failed to run 'cmd package list']");
        return -1;
    }

    /* 收集包名 Collect package names */
    char packages[MAX_PACKAGES][MAX_PKG_NAME];
    int count = 0;

    char line[MAX_LINE];
    while (fgets(line, sizeof(line), fp) && count < MAX_PACKAGES) {
        /* 移除末尾换行符 Remove trailing newline */
        char *nl = strchr(line, '\n');
        if (nl) *nl = '\0';

        /* 检查是否为包记录 Check if this is a package record */
        if (!str_starts_with(line, "package:")) continue;

        /* 查找最后一个 '=' 以处理路径中的 '=' 号 */
        /* Find LAST '=' to handle paths with '=' in them */
        char *last_eq = strrchr(line, '=');
        if (!last_eq || last_eq == line) continue;

        /* 提取代码路径 Extract codePath */
        char code_path[MAX_PATH_LEN];
        size_t path_len = last_eq - line - 8;  /* 8 = strlen("package:") */
        if (path_len >= MAX_PATH_LEN) path_len = MAX_PATH_LEN - 1;
        strncpy(code_path, line + 8, path_len);
        code_path[path_len] = '\0';

        if (!is_user_app(code_path)) continue;

        /* 提取包名 Extract package name */
        const char *pkg = last_eq + 1;
        if (strlen(pkg) == 0) continue;

        strncpy(packages[count], pkg, MAX_PKG_NAME - 1);
        packages[count][MAX_PKG_NAME - 1] = '\0';
        count++;

        log_msg(LOG_DEBUG, "  + %s (%s)", pkg, code_path);
    }

    int ret = pclose(fp);
    if (ret != 0) {
        log_msg(LOG_WARN, "'cmd package list' 返回 %d ['cmd package list' returned %d]", ret, ret);
    }

    log_msg(LOG_INFO, "发现 %d 个用户安装应用 [Found %d user-installed packages]", count, count);

    /* 确保输出目录存在 Ensure output directory exists */
    if (!dir_exists("/data/adb/tricky_store/")) {
        log_msg(LOG_WARN, "Tricky Store 目录不存在: /data/adb/tricky_store/ [Tricky Store directory not found]");
    }

    /* 写入 target.txt Write target.txt */
    FILE *f = fopen(g_config.target_txt, "w");
    if (!f) {
        log_msg(LOG_ERROR, "无法写入 %s (%s) [Cannot write %s (%s)]", g_config.target_txt, strerror(errno), g_config.target_txt, strerror(errno));
        return -1;
    }

    for (int i = 0; i < count; i++) {
        fprintf(f, "%s\n", packages[i]);
    }

    fclose(f);

    log_msg(LOG_INFO, "已写入 %d 个包到 %s [Wrote %d packages to %s]", count, g_config.target_txt, count, g_config.target_txt);
    return 0;
}