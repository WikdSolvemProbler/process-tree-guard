// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "descriptors.h"
#include "native_syscall.h"

#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/magic.h>
#include <stdlib.h>
#include <stdint.h>
#include <sys/syscall.h>
#include <sys/vfs.h>
#include <unistd.h>

static int authority(int fd) {
    const int flags = fcntl(fd, F_GETFL);
    if (flags < 0) return -errno;
    struct statfs filesystem;
    if (fstatfs(fd, &filesystem) < 0) return -errno;
    if ((filesystem.f_type == CGROUP_SUPER_MAGIC || filesystem.f_type == CGROUP2_SUPER_MAGIC) &&
        (flags & O_ACCMODE) != O_RDONLY) return -EPERM;
    return 0;
}

void ptg220_descriptor_drop(struct ptg220_descriptors *set) {
    for (size_t i = 0; i < set->count; ++i) (void)close(set->entries[i].held);
    free(set->entries);
    *set = (struct ptg220_descriptors){0};
}

static int append(struct ptg220_descriptors *set, int target, int held) {
    if (set->count == set->capacity) {
        if (set->capacity > SIZE_MAX / (2 * sizeof(*set->entries))) return -EOVERFLOW;
        const size_t capacity = set->capacity ? 2 * set->capacity : 8;
        void *entries = realloc(set->entries, capacity * sizeof(*set->entries));
        if (entries == NULL) return -ENOMEM;
        set->capacity = capacity;
        set->entries = entries;
    }
    set->entries[set->count++] = (struct ptg220_descriptor){.target = target, .held = held};
    return 0;
}

int ptg220_descriptor_take(struct ptg220_descriptors *set, int target, int held) {
    if (target < 0 || held < 0 || (set->count && target <= set->entries[set->count - 1].target)) {
        return -EPROTO;
    }
    const int flags = fcntl(held, F_GETFD);
    if (flags < 0) return -errno;
    if (flags != FD_CLOEXEC) return -EPROTO;
    const int permitted = authority(held);
    return permitted < 0 ? permitted : append(set, target, held);
}

static int compare_target(const void *left, const void *right) {
    const struct ptg220_descriptor *a = left;
    const struct ptg220_descriptor *b = right;
    return (a->target > b->target) - (a->target < b->target);
}

static int descriptor_name(const char *p) {
    if (*p < '0' || *p > '9') return -1;
    int fd = 0;
    for (; *p; ++p) {
        if (*p < '0' || *p > '9' || fd > (INT_MAX - (*p - '0')) / 10) return -1;
        fd = fd * 10 + (*p - '0');
    }
    return fd;
}

int ptg220_descriptor_capture(struct ptg220_descriptors *set) {
    *set = (struct ptg220_descriptors){0};
    DIR *const table = opendir("/proc/thread-self/fd");
    if (table == NULL) return -errno;
    const int scanner = dirfd(table);
    struct statfs filesystem;
    int result = fstatfs(scanner, &filesystem) < 0 ? -errno : 0;
    if (result == 0 && filesystem.f_type != PROC_SUPER_MAGIC) result = -EXDEV;
    while (result == 0) {
        errno = 0;
        const struct dirent *row = readdir(table);
        if (row == NULL) {
            result = -errno;
            break;
        }
        const int original = descriptor_name(row->d_name);
        if (original < 0 || original == scanner) continue;
        const int flags = fcntl(original, F_GETFD);
        if (flags < 0) {
            result = -errno;
            break;
        }
        if (flags & FD_CLOEXEC) continue;
        result = authority(original);
        if (result < 0) break;
        const int held = fcntl(original, F_DUPFD_CLOEXEC, 3);
        if (held < 0) {
            result = -errno;
            break;
        }
        result = append(set, original, held);
        if (result < 0) (void)close(held);
    }
    (void)closedir(table);
    if (result < 0) {
        ptg220_descriptor_drop(set);
        return result;
    }
    if (set->count > 1) qsort(set->entries, set->count, sizeof(*set->entries), compare_target);
    return 0;
}

static int compare_fd(const void *left, const void *right) {
    const int a = *(const int *)left;
    const int b = *(const int *)right;
    return (a > b) - (a < b);
}

static size_t target_index(const struct ptg220_descriptors *set, int fd) {
    size_t low = 0, high = set->count;
    while (low < high) {
        const size_t middle = low + (high - low) / 2;
        if (set->entries[middle].target < fd) low = middle + 1;
        else high = middle;
    }
    return low < set->count && set->entries[low].target == fd ? low : SIZE_MAX;
}

static int close_gaps(const int *sources, size_t count) {
    unsigned int first = 0;
    for (size_t i = 0; i < count; ++i) {
        const unsigned int retained = (unsigned int)sources[i];
        if (retained > first) {
            const long closed = native(SYS_close_range, first, retained - 1, 0, 0, 0);
            if (closed < 0) return (int)closed;
        }
        first = retained + 1;
    }
    return (int)native(SYS_close_range, first, UINT_MAX, 0, 0, 0);
}

static int restore_chain(struct ptg220_descriptors *set, size_t *incoming, size_t index) {
    while (index != SIZE_MAX && set->entries[index].held >= 0 &&
           (incoming[index] == SIZE_MAX || incoming[index] == index)) {
        struct ptg220_descriptor *const entry = &set->entries[index];
        const int old = entry->held;
        const int target = entry->target;
        const long restored = old == target
            ? native(SYS_fcntl, target, F_SETFD, 0, 0, 0)
            : native(SYS_dup3, old, target, 0, 0, 0);
        if (restored < 0) return (int)restored;
        if (restored != (old == target ? 0 : target)) return -EPROTO;
        const long flags = native(SYS_fcntl, target, F_GETFD, 0, 0, 0);
        if (flags != 0) return flags < 0 ? (int)flags : -EPROTO;
        entry->held = -1;
        if (old == target) break;
        const long closed = native(SYS_close, old, 0, 0, 0, 0);
        if (closed < 0) return (int)closed;
        index = target_index(set, old);
        if (index != SIZE_MAX) incoming[index] = SIZE_MAX;
    }
    return 0;
}

int ptg220_descriptor_restore(struct ptg220_descriptors *set) {
    if (set->count == 0) return close_gaps(NULL, 0);
    if (set->count > SIZE_MAX / sizeof(size_t)) return -EOVERFLOW;
    size_t *const incoming = malloc(set->count * sizeof(*incoming));
    int *const sources = malloc(set->count * sizeof(*sources));
    int result = incoming != NULL && sources != NULL ? 0 : -ENOMEM;
    if (result < 0) goto done;
    for (size_t i = 0; i < set->count; ++i) {
        incoming[i] = SIZE_MAX;
        sources[i] = set->entries[i].held;
    }
    qsort(sources, set->count, sizeof(*sources), compare_fd);
    for (size_t i = 0; i < set->count; ++i) {
        if (sources[i] < 0 || (i && sources[i] == sources[i - 1])) {
            result = -EPROTO;
            goto done;
        }
        const size_t target = target_index(set, set->entries[i].held);
        if (target != SIZE_MAX) incoming[target] = i;
    }
    /* P has completed its release handshake and dropped privilege. Nothing
       except the authenticated donations belongs in its new image. */
    result = close_gaps(sources, set->count);
    for (size_t i = 0; result == 0 && i < set->count; ++i) {
        result = restore_chain(set, incoming, i);
    }
    for (size_t i = 0; result == 0 && i < set->count; ++i) {
        if (set->entries[i].held < 0) continue;
        const int old = set->entries[i].held;
        const size_t vacated = target_index(set, old);
        if (vacated == SIZE_MAX || incoming[vacated] != i) {
            result = -EPROTO;
            break;
        }
        /* Only cycles remain. Every pending target is occupied, so any
           free native slot can break this cycle without hiding a vacancy. */
        const long scratch = native(SYS_fcntl, old, F_DUPFD_CLOEXEC, 0, 0, 0);
        if (scratch < 0) {
            result = (int)scratch;
            break;
        }
        set->entries[i].held = (int)scratch;
        const long closed = native(SYS_close, old, 0, 0, 0, 0);
        if (closed < 0) {
            result = (int)closed;
            break;
        }
        incoming[vacated] = SIZE_MAX;
        result = restore_chain(set, incoming, vacated);
        if (result == 0 && set->entries[i].held >= 0) result = -EPROTO;
    }
done:
    free(sources);
    free(incoming);
    return result;
}
