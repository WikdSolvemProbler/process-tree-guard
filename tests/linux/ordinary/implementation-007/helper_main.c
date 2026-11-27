// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "lifecycle.h"

#include <fcntl.h>
#include <limits.h>
#include <string.h>
#include <unistd.h>

static int protocol_fd(const char *text) {
    if (text[0] < '1' || text[0] > '9') return -1;
    int fd = 0;
    for (; *text; ++text) {
        if (*text < '0' || *text > '9' || fd > (INT_MAX - (*text - '0')) / 10) return -1;
        fd = fd * 10 + (*text - '0');
    }
    return fd >= 3 ? fd : -1;
}

int main(int argc, char **argv) {
    if (argc != 3 || strcmp(argv[1], "--internal-v1") != 0) return 125;
    const int channel = protocol_fd(argv[2]);
    if (channel < 0 || fcntl(channel, F_SETFD, FD_CLOEXEC) < 0) return 125;
    struct ptg220_helper_startup start;
    int result = ptg220_helper_take_adopted(channel, &start);
    if (result == 0) {
        result = ptg220_supervise(&start);
        ptg220_helper_drop_startup(&start);
    }
    (void)close(channel);
    return result == 0 ? 0 : 125;
}
