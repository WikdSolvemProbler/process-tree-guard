// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "lease.h"
#include "native_control.h"

#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <linux/magic.h>
#include <linux/nsfs.h>
#include <linux/sched.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/vfs.h>
#include <unistd.h>

_Static_assert(ATOMIC_INT_LOCK_FREE == 2, "lease publication cannot take a lock");

enum {
    PREPARED = 1u << 0,
    ACCEPTED = 1u << 1,
    CREATE_IN_FLIGHT = 1u << 2,
    IDENTIFIED = 1u << 3,
    SEALED = 1u << 4,
    CREATE_SUCCEEDED = 1u << 5,
    CREATE_FAILED = 1u << 6,
    PAYLOAD_MAY_EXIST = 1u << 7,
};

static int same(const struct ptg220_object *a, const struct ptg220_object *b) {
    return a->inode == b->inode && a->mount == b->mount &&
        a->device_major == b->device_major && a->device_minor == b->device_minor;
}

static int object(int parent, const char *name, int flags, mode_t kind,
                  struct ptg220_object *identity) {
    struct statx metadata;
    const unsigned mask = STATX_TYPE | STATX_MODE | STATX_UID | STATX_INO | STATX_MNT_ID;
    if (statx(parent, name, flags, mask, &metadata) < 0) return -errno;
    if ((metadata.stx_mask & mask) != mask || metadata.stx_uid != 0 ||
        (metadata.stx_mode & S_IFMT) != kind || (metadata.stx_mode & 0022) != 0) {
        return -EPERM;
    }
    *identity = (struct ptg220_object){
        .inode = metadata.stx_ino,
        .mount = metadata.stx_mnt_id,
        .device_major = metadata.stx_dev_major,
        .device_minor = metadata.stx_dev_minor,
    };
    return 0;
}

static int opened(int fd, mode_t kind, struct ptg220_object *identity) {
    struct statfs filesystem;
    if (fstatfs(fd, &filesystem) < 0) return -errno;
    if (filesystem.f_type != CGROUP2_SUPER_MAGIC) return -EXDEV;
    return object(fd, "", AT_EMPTY_PATH | AT_SYMLINK_NOFOLLOW, kind, identity);
}

static int bound_parent(const struct ptg220_lease *lease, int parent) {
    struct ptg220_object identity;
    const int result = opened(parent, S_IFDIR, &identity);
    if (result < 0) return result;
    return same(&identity, &lease->parent) ? 0 : -ESTALE;
}

static int absent(int parent, const char *name) {
    struct stat ignored;
    if (fstatat(parent, name, &ignored, AT_SYMLINK_NOFOLLOW) == 0) return -EEXIST;
    return errno == ENOENT ? 0 : -errno;
}

static int reserved_name(char name[96], int pid_namespace, pid_t w, pid_t r) {
    if (w <= 0 || r <= 0 || w == r) return -EINVAL;
    struct statfs filesystem;
    if (fstatfs(pid_namespace, &filesystem) < 0) return -errno;
    if (filesystem.f_type != NSFS_MAGIC) return -EPROTO;
    const int kind = ioctl(pid_namespace, NS_GET_NSTYPE);
    if (kind < 0) return -errno;
    if (kind != CLONE_NEWPID) return -EPROTO;
    struct stat identity;
    if (fstat(pid_namespace, &identity) < 0) return -errno;
    const int length = snprintf(name, 96, "ptg-ordinary-%jx-%jx-%x-%x",
                                (uintmax_t)identity.st_dev, (uintmax_t)identity.st_ino,
                                (unsigned int)w, (unsigned int)r);
    return length > 0 && length < 96 ? 0 : -EOVERFLOW;
}

void ptg220_lease_init(struct ptg220_lease *lease) {
    /* Called exactly once before cloning R. */
    atomic_init(&lease->phase, 0);
}

int ptg220_lease_prepare(struct ptg220_lease *lease, int parent, int pid_namespace,
                         pid_t creator_w, pid_t recovery_r) {
    if (atomic_load_explicit(&lease->phase, memory_order_acquire) != 0) return -ECANCELED;
    int result = opened(parent, S_IFDIR, &lease->parent);
    if (result == 0) result = reserved_name(lease->basename, pid_namespace, creator_w, recovery_r);
    if (result == 0) result = absent(parent, lease->basename);
    if (result < 0) return result;
    unsigned int expected = 0;
    return atomic_compare_exchange_strong_explicit(&lease->phase, &expected, PREPARED,
                                                   memory_order_acq_rel,
                                                   memory_order_acquire) ? 0 : -ECANCELED;
}

int ptg220_lease_accept(struct ptg220_lease *lease, int parent, int pid_namespace,
                        pid_t creator_w, pid_t recovery_r) {
    unsigned int expected = PREPARED;
    if (atomic_load_explicit(&lease->phase, memory_order_acquire) != expected) return -ECANCELED;
    char name[96];
    int result = reserved_name(name, pid_namespace, creator_w, recovery_r);
    if (result == 0 && strcmp(name, lease->basename) != 0) result = -ESTALE;
    if (result == 0) result = bound_parent(lease, parent);
    if (result == 0) result = absent(parent, name);
    if (result < 0) return result;
    return atomic_compare_exchange_strong_explicit(&lease->phase, &expected,
                                                   PREPARED | ACCEPTED,
                                                   memory_order_acq_rel,
                                                   memory_order_acquire) ? 0 : -ECANCELED;
}

void ptg220_lease_seal(struct ptg220_lease *lease) {
    (void)atomic_fetch_or_explicit(&lease->phase, SEALED, memory_order_acq_rel);
}

int ptg220_lease_sealed_without_birth(const struct ptg220_lease *lease) {
    const unsigned int phase = atomic_load_explicit(&lease->phase, memory_order_acquire);
    return (phase & (SEALED | PAYLOAD_MAY_EXIST)) == SEALED;
}

static int control_file(int directory, const char *name, int flags, uint64_t mount) {
    const int fd = openat(directory, name, flags | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC);
    if (fd < 0) return -errno;
    struct ptg220_object identity;
    int result = opened(fd, S_IFREG, &identity);
    if (result == 0 && identity.mount != mount) result = -EXDEV;
    if (result == 0) return fd;
    (void)close(fd);
    return result;
}

void ptg220_leaf_close(struct ptg220_leaf *leaf) {
    if (leaf->events >= 0) (void)close(leaf->events);
    if (leaf->kill >= 0) (void)close(leaf->kill);
    if (leaf->directory >= 0) (void)close(leaf->directory);
    *leaf = (struct ptg220_leaf){.directory = -1, .kill = -1, .events = -1};
}

static int open_leaf(const struct ptg220_lease *lease, int parent, struct ptg220_leaf *leaf) {
    *leaf = (struct ptg220_leaf){.directory = -1, .kill = -1, .events = -1};
    int result = bound_parent(lease, parent);
    if (result < 0) return result;
    leaf->directory = openat(parent, lease->basename,
                             O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC);
    if (leaf->directory < 0) return -errno;
    result = opened(leaf->directory, S_IFDIR, &leaf->identity);
    if (result == 0 && leaf->identity.mount != lease->parent.mount) result = -EXDEV;
    if (result == 0) {
        const int type = control_file(leaf->directory, "cgroup.type", O_RDONLY, leaf->identity.mount);
        if (type < 0) {
            result = type;
        } else {
            char value[16];
            const ssize_t size = pread(type, value, sizeof(value), 0);
            if (size < 0) result = -errno;
            else if (size != 7 || memcmp(value, "domain\n", 7) != 0) result = -EPROTO;
            (void)close(type);
        }
    }
    if (result == 0) {
        leaf->events = control_file(leaf->directory, "cgroup.events", O_RDONLY, leaf->identity.mount);
        if (leaf->events < 0) result = leaf->events;
    }
    if (result == 0) {
        leaf->kill = control_file(leaf->directory, "cgroup.kill", O_WRONLY, leaf->identity.mount);
        if (leaf->kill < 0) result = leaf->kill;
    }
    if (result < 0) ptg220_leaf_close(leaf);
    return result;
}

int ptg220_leaf_kill(const struct ptg220_leaf *leaf) {
    return ptg220_leaf_kill_observed(leaf, 0, NULL);
}

int ptg220_leaf_kill_observed(const struct ptg220_leaf *leaf, uint64_t selected,
                               struct ptg220_native_action *action) {
    const ssize_t result = write(leaf->kill, "1", 1);
    const int error = result < 0 ? errno : 0;
    if (action != NULL) {
        action->called = 1;
        action->native_return = result;
        action->native_errno = error;
        action->before_ns = selected;
        action->after_error = ptg220_boottime(&action->after_ns);
    }
    return result == 1 ? 0 : result < 0 ? -error : -EIO;
}

int ptg220_leaf_populated(const struct ptg220_leaf *leaf) {
    char value[128];
    const ssize_t size = pread(leaf->events, value, sizeof(value), 0);
    if (size < 0) return -errno;
    if (size == 0 || (size_t)size == sizeof(value) || value[size - 1] != '\n') return -EPROTO;
    int populated = -EPROTO;
    for (const char *line = value, *end = value + size; line < end;) {
        const char *newline = memchr(line, '\n', (size_t)(end - line));
        if (newline == NULL) return -EPROTO;
        if (newline - line >= 10 && memcmp(line, "populated ", 10) == 0) {
            if (populated >= 0 || newline - line != 11 || (line[10] != '0' && line[10] != '1')) {
                return -EPROTO;
            }
            populated = line[10] - '0';
        }
        line = newline + 1;
    }
    return populated;
}

int ptg220_lease_create(struct ptg220_lease *lease, int parent, struct ptg220_leaf *leaf) {
    *leaf = (struct ptg220_leaf){.directory = -1, .kill = -1, .events = -1};
    int result = bound_parent(lease, parent);
    if (result < 0) return result;
    unsigned int expected = PREPARED | ACCEPTED;
    const unsigned int creating = expected | CREATE_IN_FLIGHT;
    if (!atomic_compare_exchange_strong_explicit(&lease->phase, &expected, creating,
                                                 memory_order_acq_rel, memory_order_acquire)) {
        return -ECANCELED;
    }
    if (atomic_load_explicit(&lease->phase, memory_order_acquire) & SEALED) return -ECANCELED;
    /* W is single-threaded and R has already inherited the original umask. */
    const mode_t saved_mask = umask(0);
    const int made = mkdirat(parent, lease->basename, 0755);
    const int error = made < 0 ? errno : 0;
    (void)umask(saved_mask);
    if (made < 0) {
        (void)atomic_fetch_or_explicit(&lease->phase, CREATE_FAILED, memory_order_release);
        return -error;
    }
    (void)atomic_fetch_or_explicit(&lease->phase, CREATE_SUCCEEDED, memory_order_release);
    result = open_leaf(lease, parent, leaf);
    if (result < 0) return result;
    lease->leaf = leaf->identity;
    const unsigned int prior = atomic_fetch_or_explicit(&lease->phase, IDENTIFIED, memory_order_release);
    return (prior & SEALED) != 0 ? -ECANCELED : 0;
}

int ptg220_lease_join(const struct ptg220_lease *lease, int parent, struct ptg220_leaf *leaf) {
    *leaf = (struct ptg220_leaf){.directory = -1, .kill = -1, .events = -1};
    const unsigned int phase = atomic_load_explicit(&lease->phase, memory_order_acquire);
    if ((phase & (IDENTIFIED | SEALED)) != IDENTIFIED) return -ECANCELED;
    const int result = open_leaf(lease, parent, leaf);
    if (result < 0) return result;
    if (same(&lease->leaf, &leaf->identity)) return 0;
    ptg220_leaf_close(leaf);
    return -ESTALE;
}

int ptg220_lease_before_birth(struct ptg220_lease *lease) {
    unsigned int expected = PREPARED | ACCEPTED | CREATE_IN_FLIGHT | CREATE_SUCCEEDED | IDENTIFIED;
    const unsigned int born = expected | PAYLOAD_MAY_EXIST;
    return atomic_compare_exchange_strong_explicit(&lease->phase, &expected, born,
                                                   memory_order_acq_rel,
                                                   memory_order_acquire) ? 0 : -ECANCELED;
}

int ptg220_lease_p_can_exec(const struct ptg220_lease *lease) {
    const unsigned int phase = atomic_load_explicit(&lease->phase, memory_order_acquire);
    return (phase & (SEALED | PAYLOAD_MAY_EXIST)) == PAYLOAD_MAY_EXIST;
}

int ptg220_lease_remove_sealed(const struct ptg220_lease *lease, int parent,
                               const struct ptg220_leaf *leaf) {
    return ptg220_lease_remove_observed(lease, parent, leaf, NULL);
}

int ptg220_lease_remove_observed(const struct ptg220_lease *lease, int parent,
                                 const struct ptg220_leaf *leaf, struct ptg220_native_action *action) {
    const unsigned int phase = atomic_load_explicit(&lease->phase, memory_order_acquire);
    if ((phase & (CREATE_IN_FLIGHT | IDENTIFIED | SEALED | CREATE_FAILED)) !=
        (CREATE_IN_FLIGHT | IDENTIFIED | SEALED)) return -EPERM;
    int result = bound_parent(lease, parent);
    if (result < 0) return result;
    struct ptg220_object current;
    result = opened(leaf->directory, S_IFDIR, &current);
    if (result < 0) return result;
    if (!same(&current, &leaf->identity) || !same(&current, &lease->leaf)) {
        return -ESTALE;
    }
    result = object(parent, lease->basename, AT_SYMLINK_NOFOLLOW, S_IFDIR, &current);
    if (result < 0) return result;
    if (!same(&current, &leaf->identity)) return -ESTALE;
    result = ptg220_leaf_populated(leaf);
    if (result != 0) return result < 0 ? result : -EBUSY;
    if (action != NULL) action->before_error = ptg220_boottime(&action->before_ns);
    const int removed = unlinkat(parent, lease->basename, AT_REMOVEDIR);
    const int error = removed < 0 ? errno : 0;
    if (action != NULL) {
        action->called = 1;
        action->native_return = removed;
        action->native_errno = error;
        action->after_error = ptg220_boottime(&action->after_ns);
    }
    if (removed < 0) return -error;
    return absent(parent, lease->basename);
}

static int recover_empty(const struct ptg220_lease *lease, int parent, unsigned int phase) {
    if ((phase & (PREPARED | ACCEPTED | SEALED)) != (PREPARED | ACCEPTED | SEALED)) return -EPERM;
    if (phase & PAYLOAD_MAY_EXIST) return -EPERM;
    int result = bound_parent(lease, parent);
    if (result == 0) result = absent(parent, lease->basename);
    if (result != -EEXIST) return result;
    if (!(phase & CREATE_IN_FLIGHT) || (phase & CREATE_FAILED)) return -ESTALE;
    /* This is pre-activation recovery only. No process or descendant may
       have received admission. Use the kernel's atomic empty rmdir check;
       recovery must not depend on allocating a fresh kill/events descriptor. */
    struct ptg220_object current;
    result = object(parent, lease->basename, AT_SYMLINK_NOFOLLOW, S_IFDIR, &current);
    if (result < 0) return result;
    if (current.mount != lease->parent.mount ||
        ((phase & IDENTIFIED) && !same(&current, &lease->leaf))) return -ESTALE;
    if (unlinkat(parent, lease->basename, AT_REMOVEDIR) < 0) return -errno;
    return absent(parent, lease->basename);
}

int ptg220_lease_recover_sealed(const struct ptg220_lease *lease, int parent,
                                int original_w_pidfd) {
    const int terminal = ptg220_pidfd_terminal(original_w_pidfd);
    if (terminal != 1) return terminal < 0 ? terminal : -EAGAIN;
    const unsigned int phase = atomic_load_explicit(&lease->phase, memory_order_acquire);
    return recover_empty(lease, parent, phase);
}

int ptg220_lease_recover_own_create(const struct ptg220_lease *lease, int parent) {
    const unsigned int phase = atomic_load_explicit(&lease->phase, memory_order_acquire);
    if (!(phase & CREATE_SUCCEEDED)) return -EPERM;
    return recover_empty(lease, parent, phase);
}
