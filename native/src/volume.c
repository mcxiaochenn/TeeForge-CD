#include "teeforge.h"
#include <unistd.h>
#include <fcntl.h>
#include <linux/input.h>
#include <stdio.h>
#include <time.h>

/* ===== 音量键监听 Volume Key Listener ===== */
/* 独立模块，方便复用 Standalone module for reuse */
/* 返回 Return: 1=音量+ / 0=音量- / -1=超时 */

int volume_listen(int timeout_sec) {
    const char *devices[] = {
        "/dev/input/event0", "/dev/input/event1",
        "/dev/input/event2", "/dev/input/event3",
        "/dev/input/event4", "/dev/input/event5",
        NULL
    };

    int fds[16];
    int nfds = 0;

    for (int i = 0; devices[i] != NULL && nfds < 16; i++) {
        int fd = open(devices[i], O_RDONLY | O_NONBLOCK);
        if (fd >= 0) {
            fds[nfds++] = fd;
        }
    }

    if (nfds == 0) {
        log_msg(LOG_WARN, "无法打开输入设备 [Cannot open input devices]");
        return -1;
    }

    time_t start = time(NULL);
    struct input_event ev;

    while (time(NULL) - start < timeout_sec) {
        for (int i = 0; i < nfds; i++) {
            ssize_t n = read(fds[i], &ev, sizeof(ev));
            if (n == sizeof(ev)) {
                if (ev.type == EV_KEY && ev.value == 1) {
                    if (ev.code == KEY_VOLUMEUP) {
                        for (int j = 0; j < nfds; j++) close(fds[j]);
                        return 1;
                    } else if (ev.code == KEY_VOLUMEDOWN) {
                        for (int j = 0; j < nfds; j++) close(fds[j]);
                        return 0;
                    }
                }
            }
        }
        usleep(50000);
    }

    for (int j = 0; j < nfds; j++) close(fds[j]);
    return -1;
}
