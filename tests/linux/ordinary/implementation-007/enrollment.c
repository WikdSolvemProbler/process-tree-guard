// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "enrollment.h"
#include "credentials.h"
#include "native_control.h"

#include <elf.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/magic.h>
#include <linux/nsfs.h>
#include <linux/sched.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/vfs.h>
#include <unistd.h>

/* Reviewed initial nsfs inode assignments for the selected Linux kernel. */
#define INITIAL_USER_NS UINT64_C(0xeffffffd)
#define INITIAL_PID_NS UINT64_C(0xeffffffc)
#define INITIAL_CGROUP_NS UINT64_C(0xeffffffb)

static int checked_root_object(int fd, mode_t kind, struct stat *metadata) {
    if (fstat(fd, metadata) < 0) return -errno;
    if (metadata->st_uid != 0 || metadata->st_gid != 0 ||
        (metadata->st_mode & S_IFMT) != kind || (metadata->st_mode & 0022) != 0) return -EPERM;
    return 0;
}

static int opened_root(int parent, const char *name, mode_t kind, struct stat *metadata) {
    const int flags = O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK;
    const int fd = openat(parent, name, flags | (kind == S_IFDIR ? O_DIRECTORY : 0));
    if (fd < 0) return -errno;
    const int checked = checked_root_object(fd, kind, metadata);
    if (checked == 0) return fd;
    (void)close(fd);
    return checked;
}

static int root_child(int parent, const char *name) {
    struct stat metadata;
    return opened_root(parent, name, S_IFDIR, &metadata);
}

static int bounded_read(int fd, char *bytes, size_t capacity) {
    size_t used = 0;
    while (used < capacity) {
        const ssize_t size = read(fd, bytes + used, capacity - used);
        if (size < 0) return -errno;
        if (size == 0) {
            bytes[used] = '\0';
            return (int)used;
        }
        used += (size_t)size;
    }
    return -EOVERFLOW;
}

static int filesystem_is(int fd, long expected) {
    struct statfs filesystem;
    if (fstatfs(fd, &filesystem) < 0) return -errno;
    return filesystem.f_type == expected ? 0 : -EXDEV;
}

static int zero_suid_dumpable(int root) {
    const int proc = root_child(root, "proc");
    if (proc < 0) return proc;
    int result = filesystem_is(proc, PROC_SUPER_MAGIC);
    const int sys = result == 0 ? root_child(proc, "sys") : result;
    (void)close(proc);
    if (sys < 0) return sys;
    const int fs = root_child(sys, "fs");
    (void)close(sys);
    if (fs < 0) return fs;
    struct stat metadata;
    const int file = opened_root(fs, "suid_dumpable", S_IFREG, &metadata);
    (void)close(fs);
    if (file < 0) return file;
    result = filesystem_is(file, PROC_SUPER_MAGIC);
    if (result == 0) {
        char value[8];
        const int size = bounded_read(file, value, sizeof(value));
        result = size < 0 ? size : size == 2 && memcmp(value, "0\n", 2) == 0 ? 0 : -EPERM;
    }
    (void)close(file);
    return result;
}

static int read_exact_at(int fd, void *bytes, size_t length, off_t offset) {
    const ssize_t got = pread(fd, bytes, length, offset);
    return got == (ssize_t)length ? 0 : got < 0 ? -errno : -EPROTO;
}

static int fixed_static_elf(int fd, const struct stat *metadata) {
    if ((metadata->st_mode & 07777) != 04755 || metadata->st_size < (off_t)sizeof(Elf64_Ehdr)) {
        return -EPERM;
    }
    Elf64_Ehdr elf;
    int result = read_exact_at(fd, &elf, sizeof(elf), 0);
    if (result < 0) return result;
    if (memcmp(elf.e_ident, ELFMAG, SELFMAG) != 0 || elf.e_ident[EI_CLASS] != ELFCLASS64 ||
        elf.e_ident[EI_DATA] != ELFDATA2LSB || elf.e_ident[EI_VERSION] != EV_CURRENT ||
        elf.e_machine != EM_X86_64 || (elf.e_type != ET_EXEC && elf.e_type != ET_DYN) ||
        elf.e_version != EV_CURRENT || elf.e_ehsize != sizeof(Elf64_Ehdr) ||
        elf.e_phentsize != sizeof(Elf64_Phdr) || elf.e_phnum == 0 || elf.e_phnum == PN_XNUM ||
        elf.e_phoff > (uint64_t)metadata->st_size ||
        (uint64_t)elf.e_phnum * sizeof(Elf64_Phdr) > (uint64_t)metadata->st_size - elf.e_phoff) {
        return -ENOEXEC;
    }
    int loadable = 0;
    for (unsigned int i = 0; i < elf.e_phnum; ++i) {
        Elf64_Phdr segment;
        const off_t offset = (off_t)(elf.e_phoff + i * sizeof(segment));
        result = read_exact_at(fd, &segment, sizeof(segment), offset);
        if (result < 0) return result;
        if (segment.p_type == PT_INTERP) return -ENOEXEC;
        if (segment.p_type == PT_LOAD) loadable = 1;
    }
    return loadable ? 0 : -ENOEXEC;
}

int ptg220_open_fixed_helper(void) {
    int parent = root_child(AT_FDCWD, "/");
    if (parent < 0) return parent;
    const int dumpable = zero_suid_dumpable(parent);
    if (dumpable < 0) {
        (void)close(parent);
        return dumpable;
    }
    const char *const components[] = {"usr", "libexec", "process-tree-guard"};
    for (unsigned int i = 0; i < sizeof(components) / sizeof(components[0]); ++i) {
        const int next = root_child(parent, components[i]);
        (void)close(parent);
        if (next < 0) return next;
        parent = next;
    }
    struct stat metadata;
    const int image = opened_root(parent, "ordinary-linux-helper", S_IFREG, &metadata);
    (void)close(parent);
    if (image < 0) return image;
    const int result = fixed_static_elf(image, &metadata);
    if (result == 0) return image;
    (void)close(image);
    return result;
}

static int initial_namespace(int thread, const char *name, int expected_kind, uint64_t inode) {
    /* These fixed leaves are procfs kernel magic links, checked as nsfs below. */
    const int fd = openat(thread, name, O_RDONLY | O_CLOEXEC);
    if (fd < 0) return -errno;
    int result = filesystem_is(fd, NSFS_MAGIC);
    if (result == 0) {
        const int kind = ioctl(fd, NS_GET_NSTYPE);
        if (kind < 0) result = -errno;
        else if (kind != expected_kind) result = -EPROTO;
    }
    struct stat metadata;
    if (result == 0 && fstat(fd, &metadata) < 0) result = -errno;
    if (result == 0 && (uint64_t)metadata.st_ino != inode) result = -EPERM;
    if (result == 0) return fd;
    (void)close(fd);
    return result;
}

static int open_task(int root) {
    const int proc = root_child(root, "proc");
    if (proc < 0) return proc;
    int result = filesystem_is(proc, PROC_SUPER_MAGIC);
    if (result < 0) {
        (void)close(proc);
        return result;
    }
    char path[64];
    const long tid = syscall(SYS_gettid);
    const pid_t pid = getpid();
    if (tid < 1 || pid < 1) {
        (void)close(proc);
        return -EPROTO;
    }
    const int length = snprintf(path, sizeof(path), "%ld/task/%ld", (long)pid, tid);
    if (length <= 0 || (size_t)length >= sizeof(path)) {
        (void)close(proc);
        return -EOVERFLOW;
    }
    const int task = openat(proc, path, O_RDONLY | O_DIRECTORY | O_CLOEXEC | O_NOFOLLOW);
    const int error = task < 0 ? -errno : 0;
    (void)close(proc);
    if (error < 0) return error;
    result = filesystem_is(task, PROC_SUPER_MAGIC);
    if (result == 0) return task;
    (void)close(task);
    return result;
}

static int namespace_pair(int task, const char *current, const char *children, int kind) {
    const int here = openat(task, current, O_RDONLY | O_CLOEXEC);
    if (here < 0) return -errno;
    const int next = openat(task, children, O_RDONLY | O_CLOEXEC);
    int result = next < 0 ? -errno : filesystem_is(here, NSFS_MAGIC);
    if (result == 0) result = filesystem_is(next, NSFS_MAGIC);
    if (result == 0 && (ioctl(here, NS_GET_NSTYPE) != kind || ioctl(next, NS_GET_NSTYPE) != kind)) {
        result = -EPROTO;
    }
    struct stat a, b;
    if (result == 0 && (fstat(here, &a) < 0 || fstat(next, &b) < 0)) result = -errno;
    if (result == 0 && (a.st_dev != b.st_dev || a.st_ino != b.st_ino)) result = -EXDEV;
    if (next >= 0) (void)close(next);
    (void)close(here);
    return result;
}

static int one_thread(int task) {
    const int fd = openat(task, "status", O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
    if (fd < 0) return -errno;
    char text[4096];
    int size = filesystem_is(fd, PROC_SUPER_MAGIC);
    if (size == 0) size = bounded_read(fd, text, sizeof(text));
    (void)close(fd);
    if (size < 0) return size;
    const char *row = strstr(text, "\nThreads:\t1\n");
    return row != NULL ? 0 : -ENOTSUP;
}

int ptg220_caller_preflight(void) {
    const int identity = ptg220_check_ordinary_ids();
    if (identity < 0) return identity;
    int reaper = 0;
    if (prctl(PR_GET_CHILD_SUBREAPER, &reaper, 0L, 0L, 0L) < 0) return -errno;
    if (reaper != 1) return -ENOTSUP;
    struct sigaction action;
    if (sigaction(SIGCHLD, NULL, &action) < 0) return -errno;
    if (action.sa_handler != SIG_DFL || (action.sa_flags & SA_NOCLDWAIT)) return -ENOTSUP;
    struct ptg_wait_frame receipt;
    unsigned char bytes[PTG_WAIT_NATIVE_CAPACITY];
    if (ptg220_wait_adopted(1, &receipt, bytes) != -ECHILD) return -EBUSY;

    const int root = root_child(AT_FDCWD, "/");
    if (root < 0) return root;
    const int task = open_task(root);
    (void)close(root);
    if (task < 0) return task;
    int result = one_thread(task);
    if (result == 0) result = namespace_pair(task, "ns/pid", "ns/pid_for_children", CLONE_NEWPID);
    if (result == 0) result = namespace_pair(task, "ns/time", "ns/time_for_children", CLONE_NEWTIME);
    const int pid_ns = result == 0 ? initial_namespace(task, "ns/pid", CLONE_NEWPID, INITIAL_PID_NS) : result;
    if (pid_ns >= 0) (void)close(pid_ns);
    else result = pid_ns;
    (void)close(task);
    return result;
}

static int match_enrollment(int root, int task, uid_t uid, char path[4096]) {
    const int etc = root_child(root, "etc");
    if (etc < 0) return etc;
    const int configuration = root_child(etc, "process-tree-guard");
    (void)close(etc);
    if (configuration < 0) return configuration;
    struct stat metadata;
    const int file = opened_root(configuration, "ordinary-linux.v1", S_IFREG, &metadata);
    (void)close(configuration);
    if (file < 0) return file;
    char match[4096];
    const int bytes = bounded_read(file, match, sizeof(match));
    (void)close(file);
    if (bytes < 0) return bytes;
    char prefix[80];
    const int size = snprintf(prefix, sizeof(prefix), "PTG-ORDINARY-LINUX-V1\nuid=%lu\ncgroup=",
                              (unsigned long)uid);
    if (size <= 0 || (size_t)size >= sizeof(prefix) || bytes < size + 3 ||
        memcmp(match, prefix, (size_t)size) != 0 || match[bytes - 1] != '\n') return -EPERM;
    const char *bound = match + size;
    const size_t bound_length = (size_t)(bytes - size - 1);
    if (bound[0] != '/' || memchr(bound, '\0', bound_length) != NULL) return -EPROTO;

    const int actual = openat(task, "cgroup", O_RDONLY | O_NONBLOCK | O_CLOEXEC | O_NOFOLLOW);
    if (actual < 0) return -errno;
    const int observed = bounded_read(actual, path, 4096);
    (void)close(actual);
    if (observed < 0) return observed;
    if ((size_t)observed != bound_length + 4 || memcmp(path, "0::", 3) != 0 ||
        path[observed - 1] != '\n' || memcmp(path + 3, bound, bound_length) != 0) return -EXDEV;
    memmove(path, path + 3, bound_length);
    path[bound_length] = '\0';
    return 0;
}

static int secure_migration_file(int parent, const char *name) {
    struct stat metadata;
    const int fd = opened_root(parent, name, S_IFREG, &metadata);
    if (fd < 0) return fd;
    const int result = filesystem_is(fd, CGROUP2_SUPER_MAGIC);
    (void)close(fd);
    return result;
}

static int secure_migration_boundary(int parent) {
    int result = filesystem_is(parent, CGROUP2_SUPER_MAGIC);
    if (result == 0) result = secure_migration_file(parent, "cgroup.procs");
    if (result == 0) result = secure_migration_file(parent, "cgroup.threads");
    return result;
}

static int control_equals(int parent, const char *name, const char *expected) {
    struct stat metadata;
    const int fd = opened_root(parent, name, S_IFREG, &metadata);
    if (fd < 0) return fd;
    int result = filesystem_is(fd, CGROUP2_SUPER_MAGIC);
    if (result == 0) {
        char value[64];
        const int bytes = bounded_read(fd, value, sizeof(value));
        if (bytes < 0) result = bytes;
        else if ((size_t)bytes != strlen(expected) || memcmp(value, expected, (size_t)bytes) != 0) {
            result = -EPERM;
        }
    }
    (void)close(fd);
    return result;
}

static int open_cgroup(int root, char *path) {
    const int sys = root_child(root, "sys");
    if (sys < 0) return sys;
    const int fs = root_child(sys, "fs");
    (void)close(sys);
    if (fs < 0) return fs;
    int parent = root_child(fs, "cgroup");
    (void)close(fs);
    if (parent < 0) return parent;

    struct statx mount;
    int result = 0;
    if (statx(parent, "", AT_EMPTY_PATH, STATX_MNT_ID, &mount) < 0) result = -errno;
    else if (!(mount.stx_mask & STATX_MNT_ID)) result = -EPROTO;
    if (result == 0) result = secure_migration_boundary(parent);
    for (char *name = path + 1; result == 0;) {
        char *end = strchr(name, '/');
        if (end != NULL) *end = '\0';
        if (!name[0] || strcmp(name, ".") == 0 || strcmp(name, "..") == 0 ||
            strspn(name, "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.-") !=
            strlen(name)) {
            result = -EPROTO;
            break;
        }
        const int next = root_child(parent, name);
        if (next < 0) {
            result = next;
            break;
        }
        (void)close(parent);
        parent = next;
        struct statx identity;
        if (statx(parent, "", AT_EMPTY_PATH, STATX_MNT_ID, &identity) < 0) result = -errno;
        else if (!(identity.stx_mask & STATX_MNT_ID) || identity.stx_mnt_id != mount.stx_mnt_id) {
            result = -EXDEV;
        }
        if (result == 0) result = secure_migration_boundary(parent);
        if (end == NULL) break;
        name = end + 1;
    }
    if (result == 0) return parent;
    (void)close(parent);
    return result;
}

static int verify_current_thread(int parent, uint64_t deadline_ns) {
    struct stat metadata;
    const int threads = opened_root(parent, "cgroup.threads", S_IFREG, &metadata);
    if (threads < 0) return threads;
    const long tid = syscall(SYS_gettid);
    uint64_t entry = 0;
    int have_digit = 0;
    int result = tid > 0 ? filesystem_is(threads, CGROUP2_SUPER_MAGIC) : -ESRCH;
    while (result == 0) {
        uint64_t now;
        result = ptg220_boottime(&now);
        if (result < 0) break;
        if (now >= deadline_ns) {
            result = -ETIMEDOUT;
            break;
        }
        char bytes[256];
        const ssize_t length = read(threads, bytes, sizeof(bytes));
        if (length <= 0) {
            result = length < 0 ? -errno : have_digit ? -EPROTO : -EXDEV;
            break;
        }
        for (ssize_t i = 0; i < length; ++i) {
            const char byte = bytes[i];
            if (byte == '\n' && have_digit && entry > 0) {
                if (entry == (uint64_t)tid) {
                    result = 1;
                    break;
                }
                entry = 0;
                have_digit = 0;
            } else if (byte >= '0' && byte <= '9' && entry <= (UINT32_MAX - 9) / 10) {
                entry = entry * 10 + (unsigned int)(byte - '0');
                have_digit = 1;
            } else {
                result = -EPROTO;
                break;
            }
        }
    }
    (void)close(threads);
    if (result != 1) return result;
    uint64_t now;
    result = ptg220_boottime(&now);
    if (result < 0) return result;
    return now < deadline_ns ? 0 : -ETIMEDOUT;
}

int ptg220_read_enrollment(uint64_t deadline_ns, struct ptg220_enrollment *enrolled) {
    *enrolled = (struct ptg220_enrollment){.parent = -1, .pid_namespace = -1};
    uid_t real, effective, saved;
    if (getresuid(&real, &effective, &saved) < 0) return -errno;
    if (real == 0 || effective != 0 || saved != 0) return -EPERM;
    const int nnp = prctl(PR_GET_NO_NEW_PRIVS, 0L, 0L, 0L, 0L);
    if (nnp < 0) return -errno;
    if (nnp != 0) return -EPERM;
    const int root = root_child(AT_FDCWD, "/");
    if (root < 0) return root;
    const int dumpable = zero_suid_dumpable(root);
    if (dumpable < 0) {
        (void)close(root);
        return dumpable;
    }
    const int task = open_task(root);
    if (task < 0) {
        (void)close(root);
        return task;
    }
    const struct { const char *name; int kind; uint64_t inode; } checks[] = {
        {"ns/user", CLONE_NEWUSER, INITIAL_USER_NS}, {"ns/pid", CLONE_NEWPID, INITIAL_PID_NS},
        {"ns/cgroup", CLONE_NEWCGROUP, INITIAL_CGROUP_NS},
    };
    int result = 0;
    for (unsigned int i = 0; i < sizeof(checks) / sizeof(checks[0]); ++i) {
        const int ns = initial_namespace(task, checks[i].name, checks[i].kind, checks[i].inode);
        if (ns < 0) {
            result = ns;
            break;
        }
        if (checks[i].kind == CLONE_NEWPID) enrolled->pid_namespace = ns;
        else (void)close(ns);
    }
    char path[4096];
    if (result == 0) result = match_enrollment(root, task, real, path);
    if (result == 0) {
        enrolled->parent = open_cgroup(root, path);
        if (enrolled->parent < 0) result = enrolled->parent;
    }
    if (result == 0) result = control_equals(enrolled->parent, "cgroup.type", "domain\n");
    /* Selected initialized, populated domain with no child controllers enabled. */
    if (result == 0) result = control_equals(enrolled->parent, "cgroup.subtree_control", "");
    if (result == 0) result = verify_current_thread(enrolled->parent, deadline_ns);
    (void)close(task);
    (void)close(root);
    if (result < 0) {
        if (enrolled->parent >= 0) (void)close(enrolled->parent);
        if (enrolled->pid_namespace >= 0) (void)close(enrolled->pid_namespace);
        enrolled->pid_namespace = -1;
        enrolled->parent = -1;
    }
    return result;
}
