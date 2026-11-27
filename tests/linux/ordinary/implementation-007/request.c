// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "request.h"
#include "native_syscall.h"

#include <errno.h>
#include <fcntl.h>
#include <linux/memfd.h>
#include <linux/magic.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/vfs.h>
#include <unistd.h>

struct header {
    uint64_t bytes;
    uint64_t signal_mask;
    uint64_t stack_soft;
    uint64_t stack_hard;
    uint32_t argc;
    uint32_t envc;
    uint32_t closed_stdio;
    uint32_t version;
    uint32_t security_bytes;
    int32_t security_error;
};

static const int seals = F_SEAL_SEAL | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_WRITE;
/* Linux's 3/4 _STK_LIM argument ceiling, plus room for the exec pathname. */
static const size_t max_request_bytes = 6u * 1024u * 1024u + 4096u;

static int security_label(char label[4096], int *unsupported) {
    *unsupported = 0;
    const int fd = open("/proc/thread-self/attr/current", O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
    if (fd < 0) return -errno;
    struct statfs filesystem;
    int result = fstatfs(fd, &filesystem) < 0 ? -errno : 0;
    if (result == 0 && filesystem.f_type != PROC_SUPER_MAGIC) result = -EXDEV;
    if (result == 0) {
        const ssize_t length = read(fd, label, 4096);
        if (length < 0 && errno == EINVAL) *unsupported = EINVAL;
        else if (length < 0) result = -errno;
        else if (length == 0 || length >= 4096) result = -EPROTO;
        else result = (int)length;
    }
    (void)close(fd);
    return result;
}

int ptg220_verify_inherited_security(const struct ptg220_request *request) {
    char label[4096];
    int unsupported;
    const int length = security_label(label, &unsupported);
    if (length < 0) return length;
    if (unsupported != request->security_error || (unsigned int)length != request->security_bytes) {
        return -EXDEV;
    }
    return length == 0 || memcmp(label, request->security_context, (size_t)length) == 0 ? 0 : -EXDEV;
}

static int write_bytes(int fd, const void *bytes, size_t count) {
    const char *p = bytes;
    while (count != 0) {
        const ssize_t n = write(fd, p, count);
        if (n < 0) return -errno;
        if (n == 0) return -EIO;
        p += n;
        count -= (size_t)n;
    }
    return 0;
}

static int count_input(char *const array[], size_t maximum, size_t *bytes, uint32_t *count) {
    *count = 0;
    for (; array[*count] != NULL; ++*count) {
        if ((size_t)*count >= maximum / sizeof(char *) || *bytes >= maximum) return -E2BIG;
        const size_t left = maximum - *bytes;
        const size_t length = strnlen(array[*count], left);
        if (length == left) return -E2BIG;
        *bytes += length + 1;
    }
    return 0;
}

int ptg220_request_capture(const char *path, char *const argv[], char *const envp[]) {
    if (path == NULL || *path == '\0' || argv == NULL || argv[0] == NULL) {
        return -EINVAL;
    }
    if (envp == NULL) envp = environ;
    if (envp == NULL) return -EPROTO;
    const long arg_max = sysconf(_SC_ARG_MAX);
    if (arg_max <= 0) return -EINVAL;
    const size_t maximum = (size_t)arg_max;
    if (maximum > max_request_bytes) return -E2BIG;
    const size_t path_length = strnlen(path, maximum);
    if (path_length == maximum) return -E2BIG;
    size_t bytes = path_length + 1;
    struct header record = {.version = 1};
    char label[4096];
    int unsupported;
    const int security_bytes = security_label(label, &unsupported);
    if (security_bytes < 0) return security_bytes;
    record.security_bytes = (uint32_t)security_bytes;
    record.security_error = unsupported;
    int result = count_input(argv, maximum, &bytes, &record.argc);
    if (result == 0) result = count_input(envp, maximum, &bytes, &record.envc);
    if (result < 0) return result;
    record.bytes = bytes;
    for (int fd = 0; fd < 3; ++fd) {
        const int flags = fcntl(fd, F_GETFD);
        if (flags < 0) {
            if (errno != EBADF) return -errno;
            record.closed_stdio |= 1u << fd;
        }
        if (flags >= 0 && (flags & FD_CLOEXEC)) record.closed_stdio |= 1u << fd;
    }
    struct rlimit stack;
    if (getrlimit(RLIMIT_STACK, &stack) < 0) return -errno;
    record.stack_soft = stack.rlim_cur;
    record.stack_hard = stack.rlim_max;
    const long sampled = native(SYS_rt_sigprocmask, SIG_SETMASK, 0, (long)&record.signal_mask,
                                sizeof(record.signal_mask), 0);
    if (sampled < 0) return (int)sampled;
    const int fd = (int)native(SYS_memfd_create, (long)"ptg-ordinary-request",
                              MFD_CLOEXEC | MFD_ALLOW_SEALING, 0, 0, 0);
    if (fd < 0) return fd;
    result = write_bytes(fd, &record, sizeof(record));
    if (result == 0) result = write_bytes(fd, label, (size_t)security_bytes);
    if (result == 0) result = write_bytes(fd, path, path_length + 1);
    for (uint32_t i = 0; result == 0 && i < record.argc; ++i) {
        result = write_bytes(fd, argv[i], strlen(argv[i]) + 1);
    }
    for (uint32_t i = 0; result == 0 && i < record.envc; ++i) {
        result = write_bytes(fd, envp[i], strlen(envp[i]) + 1);
    }
    if (result == 0 && fcntl(fd, F_ADD_SEALS, seals) < 0) result = -errno;
    if (result == 0) return fd;
    (void)close(fd);
    return result;
}

void ptg220_request_close(struct ptg220_request *request) {
    if (request->mapping != NULL) (void)munmap(request->mapping, request->length);
    free(request->argv);
    free(request->envp);
    *request = (struct ptg220_request){0};
}

static int take_string(char **start, const char *end, char **value) {
    if (*start == end) return -EPROTO;
    const char *terminator = memchr(*start, '\0', (size_t)(end - *start));
    if (terminator == NULL) return -EPROTO;
    *value = *start;
    *start = (char *)terminator + 1;
    return 0;
}

int ptg220_request_read(int fd, struct ptg220_request *request) {
    *request = (struct ptg220_request){0};
    const int flags = fcntl(fd, F_GET_SEALS);
    if (flags < 0) return -errno;
    if ((flags & seals) != seals) return -EPERM;
    struct stat metadata;
    if (fstat(fd, &metadata) < 0) return -errno;
    if (!S_ISREG(metadata.st_mode) || metadata.st_size < (off_t)sizeof(struct header)) return -EPROTO;
    struct header record;
    const ssize_t got = pread(fd, &record, sizeof(record), 0);
    if (got < 0) return -errno;
    if (got != (ssize_t)sizeof(record) || record.version != 1 || record.closed_stdio > 7 ||
        record.argc == 0 || record.bytes > max_request_bytes ||
        record.security_bytes >= 4096 ||
        (record.security_bytes == 0 ? record.security_error != EINVAL : record.security_error != 0) ||
        record.bytes + record.security_bytes != (uint64_t)metadata.st_size - sizeof(record) ||
        (uint64_t)record.argc + record.envc + 1 > record.bytes ||
        ((uint64_t)record.argc + record.envc) > max_request_bytes / sizeof(char *) ||
        record.stack_soft > record.stack_hard) return -EPROTO;
    request->stack = (struct rlimit){.rlim_cur = record.stack_soft, .rlim_max = record.stack_hard};
    request->signal_mask = record.signal_mask;
    request->closed_stdio = record.closed_stdio;
    request->length = (size_t)metadata.st_size;
    void *mapping = mmap(NULL, request->length, PROT_READ, MAP_PRIVATE, fd, 0);
    if (mapping == MAP_FAILED) return -errno;
    request->mapping = mapping;
    request->argv = calloc((size_t)record.argc + 1, sizeof(char *));
    request->envp = calloc((size_t)record.envc + 1, sizeof(char *));
    int result = request->argv != NULL && request->envp != NULL ? 0 : -ENOMEM;
    request->security_context = (const char *)mapping + sizeof(record);
    request->security_bytes = record.security_bytes;
    request->security_error = record.security_error;
    char *start = (char *)mapping + sizeof(record) + record.security_bytes;
    const char *end = (const char *)mapping + request->length;
    char *filename;
    if (result == 0) result = take_string(&start, end, &filename);
    if (result == 0) {
        if (filename[0] == '\0') result = -EPROTO;
        else request->path = filename;
    }
    for (uint32_t i = 0; result == 0 && i < record.argc; ++i) {
        result = take_string(&start, end, &request->argv[i]);
    }
    for (uint32_t i = 0; result == 0 && i < record.envc; ++i) {
        result = take_string(&start, end, &request->envp[i]);
    }
    if (result == 0 && start != end) result = -EPROTO;
    if (result < 0) ptg220_request_close(request);
    return result;
}
