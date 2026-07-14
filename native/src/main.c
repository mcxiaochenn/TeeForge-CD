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
    printf("  --volume SEC  音量键监听 [Volume key listen] (输出 1/0)\n");
    printf("  --verbose     启用调试日志 [Enable debug logging]\n");
    printf("  --config FILE 使用自定义配置文件 [Use custom config file]\n");
    printf("  --help        显示帮助 [Show this help]\n");
}

int main(int argc, char *argv[]) {
    int do_generate = 0;
    int do_hide_bl = 0;
    int do_keybox = 0;
    int do_volume = 0;
    int volume_timeout = 10;
    int verbose = 0;
    const char *config_file = CONFIG_FILE;

    /* 解析参数 Parse arguments */
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--generate") == 0) {
            do_generate = 1;
        } else if (strcmp(argv[i], "--hide-bl") == 0) {
            do_hide_bl = 1;
        } else if (strcmp(argv[i], "--keybox") == 0) {
            do_keybox = 1;
        } else if (strcmp(argv[i], "--volume") == 0) {
            do_volume = 1;
            if (i + 1 < argc && argv[i + 1][0] != '-') {
                volume_timeout = atoi(argv[++i]);
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
    if (!do_generate && !do_hide_bl && !do_keybox && !do_volume && argc == 1) {
        do_generate = 1;
    }

    /* 设置日志级别 Setup logging */
    log_set_level(verbose ? LOG_DEBUG : LOG_INFO);
    log_msg(LOG_INFO, "TeeForge-CD v%s 启动中... [starting...]", TEEFORGE_VERSION);

    /* 加载配置 Load config */
    config_load(config_file);

    /* 检查 root 权限 Check if running as root */
    if (getuid() != 0) {
        log_msg(LOG_WARN, "未以 root 运行 [Not running as root]");
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

    if (do_volume) {
        int result = volume_listen(volume_timeout);
        printf("%d\n", result);
        return (result >= 0) ? 0 : 1;
    }

    log_msg(LOG_INFO, "");
    if (ret == 0) {
        log_msg(LOG_INFO, "完成 [Done]");
    } else {
        log_msg(LOG_ERROR, "失败，错误码 %d [Failed with code %d]", ret, ret);
    }

    return ret;
}
