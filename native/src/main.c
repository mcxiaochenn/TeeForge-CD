#include "teeforge.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void print_usage(const char *prog) {
    printf("TeeForge-CD v%s\n", TEEFORGE_VERSION);
    printf("用法 Usage: %s [options]\n", prog);
    printf("\n选项 Options:\n");
    printf("  --generate    生成 target.txt [Generate target.txt]\n");
    printf("  --hide-bl     弱隐 bootloader [Weak bootloader hiding]\n");
    printf("  --keybox      获取并更新 keybox [Fetch and update keybox]\n");
    printf("  --download URL OUTPUT  下载文件 [Download file]\n");
    printf("  --verbose     启用调试日志 [Enable debug logging]\n");
    printf("  --config FILE 使用自定义配置文件 [Use custom config file] (default: %s)\n", CONFIG_FILE);
    printf("  --help        显示帮助 [Show this help]\n");
    printf("\n");
    printf("路径 Paths (from config):\n");
    printf("  输入 Input:   %s\n", g_config.packages_xml);
    printf("  输出 Output:  %s\n", g_config.target_txt);
}

int main(int argc, char *argv[]) {
    int do_generate = 0;
    int do_hide_bl = 0;
    int do_keybox = 0;
    int do_download = 0;
    int verbose = 0;
    const char *config_file = CONFIG_FILE;
    const char *dl_url = NULL;
    const char *dl_output = NULL;

    /* 解析参数 Parse arguments */
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--generate") == 0) {
            do_generate = 1;
        } else if (strcmp(argv[i], "--hide-bl") == 0) {
            do_hide_bl = 1;
        } else if (strcmp(argv[i], "--keybox") == 0) {
            do_keybox = 1;
        } else if (strcmp(argv[i], "--download") == 0) {
            do_download = 1;
            if (i + 2 < argc) {
                dl_url = argv[++i];
                dl_output = argv[++i];
            } else {
                fprintf(stderr, "--download 需要 URL 和输出路径 [--download requires URL and output path]\n");
                return 1;
            }
        } else if (strcmp(argv[i], "--verbose") == 0) {
            verbose = 1;
        } else if (strcmp(argv[i], "--config") == 0) {
            if (i + 1 < argc) {
                config_file = argv[++i];
            } else {
                fprintf(stderr, "--config 需要文件路径 [--config requires a file path]\n");
                return 1;
            }
        } else if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            print_usage(argv[0]);
            return 0;
        } else {
            fprintf(stderr, "未知选项 Unknown option: %s\n", argv[i]);
            print_usage(argv[0]);
            return 1;
        }
    }

    /* 默认：无参数时执行生成 Default: generate if no args */
    if (!do_generate && !do_hide_bl && !do_keybox && !do_download && argc == 1) {
        do_generate = 1;
    }

    /* 设置日志级别 Setup logging */
    log_set_level(verbose ? LOG_DEBUG : LOG_INFO);
    log_msg(LOG_INFO, "TeeForge-CD v%s 启动中... [starting...]", TEEFORGE_VERSION);

    /* 加载配置 Load config */
    config_load(config_file);

    /* 检测地区 Detect region */
    dl_detect_region();

    /* 检查 root 权限 Check if running as root */
    if (getuid() != 0) {
        log_msg(LOG_WARN, "未以 root 运行，可能无法读取系统文件 [Not running as root, may fail to read system files]");
    }

    /* 执行任务 Execute */
    int ret = 0;

    if (do_generate) {
        log_msg(LOG_INFO, "");
        ret = target_generate();
    }

    if (do_hide_bl) {
        log_msg(LOG_INFO, "");
        ret = bl_hide();
    }

    if (do_keybox) {
        log_msg(LOG_INFO, "");
        ret = keybox_fetch();
    }

    if (do_download && dl_url && dl_output) {
        size_t len = 0;
        char *data = dl_download_with_retry(dl_url, &len);
        if (data && len > 0) {
            ret = write_file(dl_output, data, len);
            free(data);
            if (ret == 0) {
                log_msg(LOG_INFO, "下载完成 [Download complete]: %s (%zu bytes)", dl_output, len);
            }
        } else {
            ret = -1;
        }
    }

    log_msg(LOG_INFO, "");
    if (ret == 0) {
        log_msg(LOG_INFO, "完成 [Done]");
    } else {
        log_msg(LOG_ERROR, "失败，错误码 %d [Failed with code %d]", ret, ret);
    }

    return ret;
}