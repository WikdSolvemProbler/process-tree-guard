// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "owners.h"
#include "native_control.h"
#include "owner_watch.h"

#include <errno.h>
#include <fcntl.h>
#include <linux/magic.h>
#include <stdio.h>
#include <string.h>
#include <sys/vfs.h>
#include <unistd.h>

static int native_proc(const char *path, char text[16385]) {
    const int fd = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
    if (fd < 0) return -errno;
    struct statfs filesystem;
    int result = fstatfs(fd, &filesystem) < 0 ? -errno : 0;
    size_t used = 0;
    if (result == 0 && filesystem.f_type != PROC_SUPER_MAGIC) result = -EXDEV;
    while (result == 0) {
        const ssize_t bytes = read(fd, text + used, 16385 - used);
        if (bytes < 0) result = -errno;
        else if (bytes == 0) break;
        else {
            used += (size_t)bytes;
            if (used == 16385) result = -EOVERFLOW;
        }
    }
    (void)close(fd);
    if (result < 0) return result;
    if (used == 0 || memchr(text, '\0', used) != NULL) return -EPROTO;
    text[used] = '\0';
    return 0;
}

static int decimal(const char *start, const char *end, uint64_t *value) {
    if (start == end) return -EPROTO;
    uint64_t now = 0;
    for (; start < end; ++start) {
        if (*start < '0' || *start > '9') return -EPROTO;
        const unsigned int digit = (unsigned int)(*start - '0');
        if (now > (UINT64_MAX - digit) / 10) return -EOVERFLOW;
        now = now * 10 + digit;
    }
    *value = now;
    return 0;
}

int ptg220_pidfd_names(int pidfd, uint32_t expected) {
    char path[80];
    const int n = snprintf(path, sizeof(path), "/proc/thread-self/fdinfo/%d", pidfd);
    if (n <= 0 || (size_t)n >= sizeof(path)) return -EOVERFLOW;
    char text[16385];
    const int readback = native_proc(path, text);
    if (readback < 0) return readback;
    unsigned int seen = 0;
    for (const char *line = text; *line;) {
        const char *ending = strchr(line, '\n');
        if (ending == NULL) return -EPROTO;
        const size_t size = (size_t)(ending - line);
        const unsigned int field = size >= 5 && memcmp(line, "Pid:\t", 5) == 0 ? 1u :
            size >= 7 && memcmp(line, "NSpid:\t", 7) == 0 ? 2u : 0u;
        if (field) {
            uint64_t value;
            if ((seen & field) || decimal(line + (field == 1 ? 5 : 7), ending, &value) < 0 ||
                value != expected) return -EPROTO;
            seen |= field;
        }
        line = ending + 1;
    }
    return seen == 3 ? 0 : -EBADF;
}

static int exact_stat(const struct ptg220_owner_identity *identity) {
    char path[80];
    const int n = snprintf(path, sizeof(path), "/proc/%u/stat", identity->pid);
    if (n <= 0 || (size_t)n >= sizeof(path)) return -EOVERFLOW;
    char text[16385];
    const int readback = native_proc(path, text);
    if (readback < 0) return readback;
    const char *command = strchr(text, '(');
    const char *ending = strrchr(text, ')');
    if (command == NULL || ending == NULL || command <= text || ending <= command ||
        command[-1] != ' ' || strlen(ending) < 5 || ending[1] != ' ' || ending[3] != ' ' ||
        ending[2] == 'Z' || ending[2] == 'X') return -EPROTO;
    uint64_t observed;
    if (decimal(text, command - 1, &observed) < 0 || observed != identity->pid) return -EPROTO;
    const char *p = ending + 4;
    for (unsigned int field = 4; field <= 22; ++field) {
        const char *next = strchr(p, ' ');
        if (next == NULL || next == p) return -EPROTO;
        if (field == 4 && (decimal(p, next, &observed) < 0 || observed != identity->ppid)) return -ESTALE;
        if (field == 22 && (decimal(p, next, &observed) < 0 || observed != identity->start_ticks)) {
            return -ESTALE;
        }
        p = next + 1;
    }
    return 0;
}

int ptg220_bind_pidfds(int original, int reported, uint32_t expected) {
    int status = ptg220_pidfd_names(original, expected);
    if (status == 0) status = ptg220_pidfd_names(reported, expected);
    if (status == 0) status = ptg220_pidfd_names(original, expected);
    return status;
}

int ptg220_verify_owners(const int *pidfds, const struct ptg220_owner_identity *originals,
                         unsigned int count, unsigned int mode) {
    if (count < PTG_OWNER_WATCH_MIN || count > PTG_OWNER_WATCH_MAX || mode > 1 ||
        (mode == 0 && count != 2) || (mode == 1 &&
         (originals[count - 1].pid != 1 || originals[count - 1].ppid != 0))) return -EPROTO;
    for (unsigned int pass = 0; pass < 2; ++pass) {
        for (unsigned int i = 0; i < count; ++i) {
            const struct ptg220_owner_identity *id = &originals[i];
            if (id->pid == 0 || id->pid > INT32_MAX ||
                (i + 1 < count && (id->ppid != originals[i + 1].pid ||
                 id->start_ticks < originals[i + 1].start_ticks))) return -EPROTO;
            for (unsigned int j = 0; j < i; ++j) {
                if (originals[j].pid == id->pid || pidfds[i] == pidfds[j]) return -EPROTO;
            }
            int result = ptg220_pidfd_names(pidfds[i], id->pid);
            if (result == 0) result = ptg220_pidfd_terminal(pidfds[i]);
            if (result > 0) return -ECANCELED;
            if (result == 0) result = exact_stat(id);
            if (result == 0) result = ptg220_pidfd_terminal(pidfds[i]);
            if (result != 0) return result < 0 ? result : -ECANCELED;
        }
    }
    return 0;
}
