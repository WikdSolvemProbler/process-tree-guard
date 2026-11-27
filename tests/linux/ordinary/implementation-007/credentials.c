// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "credentials.h"

#include <errno.h>
#include <linux/capability.h>
#include <sys/fsuid.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <unistd.h>

static int exact_capabilities(unsigned int permitted) {
    struct __user_cap_header_struct header = {.version = _LINUX_CAPABILITY_VERSION_3};
    struct __user_cap_data_struct values[2] = {0};
    if (syscall(SYS_capget, &header, values) < 0) return -errno;
    if (header.version != _LINUX_CAPABILITY_VERSION_3 ||
        values[0].effective != permitted || values[0].permitted != permitted ||
        values[0].inheritable != 0 || values[1].effective != 0 ||
        values[1].permitted != 0 || values[1].inheritable != 0) return -EPERM;
    return 0;
}

int ptg220_check_ordinary_ids(void) {
    uid_t uid, euid, suid;
    gid_t gid, egid, sgid;
    if (getresuid(&uid, &euid, &suid) < 0 || getresgid(&gid, &egid, &sgid) < 0) return -errno;
    if (uid == 0 || euid != uid || suid != uid || gid != egid || gid != sgid ||
        (uid_t)setfsuid((uid_t)-1) != uid || (gid_t)setfsgid((gid_t)-1) != gid) return -EPERM;
    const int securebits = prctl(PR_GET_SECUREBITS, 0L, 0L, 0L, 0L);
    if (securebits < 0) return -errno;
    if (securebits != 0) return -EPERM;
    const int nnp = prctl(PR_GET_NO_NEW_PRIVS, 0L, 0L, 0L, 0L);
    if (nnp < 0) return -errno;
    if (nnp != 0) return -EPERM;
    return exact_capabilities(0);
}

int ptg220_restrict_privileged_helper(struct ptg220_ids *original) {
    uid_t uid, euid, suid;
    gid_t gid, egid, sgid;
    if (getresuid(&uid, &euid, &suid) < 0 || getresgid(&gid, &egid, &sgid) < 0) return -errno;
    if (uid == 0 || euid != 0 || suid != 0 || gid != egid || gid != sgid ||
        (uid_t)setfsuid((uid_t)-1) != 0 || (gid_t)setfsgid((gid_t)-1) != gid) return -EPERM;

    const int flags = prctl(PR_GET_SECUREBITS, 0L, 0L, 0L, 0L);
    if (flags < 0) return -errno;
    if (flags != 0) return -EPERM;
    if (prctl(PR_SET_DUMPABLE, 0L, 0L, 0L, 0L) < 0) return -errno;
    const int dumpable = prctl(PR_GET_DUMPABLE, 0L, 0L, 0L, 0L);
    if (dumpable < 0) return -errno;
    if (dumpable != 0) return -EPERM;
    if (prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_CLEAR_ALL, 0L, 0L, 0L) < 0) return -errno;

    const unsigned int permitted = 1u << CAP_SETUID;
    struct __user_cap_header_struct header = {.version = _LINUX_CAPABILITY_VERSION_3};
    const struct __user_cap_data_struct restricted[2] = {
        {.effective = permitted, .permitted = permitted}, {0},
    };
    if (syscall(SYS_capset, &header, restricted) < 0) return -errno;
    const int verified = exact_capabilities(permitted);
    if (verified < 0) return verified;
    *original = (struct ptg220_ids){.uid = uid, .gid = gid};
    return 0;
}

int ptg220_restore_soft_stack(const struct rlimit *original) {
    struct rlimit stack;
    if (getrlimit(RLIMIT_STACK, &stack) < 0) return -errno;
    if (stack.rlim_max != original->rlim_max || original->rlim_cur > original->rlim_max) return -EPERM;
    if (stack.rlim_cur != original->rlim_cur) {
        stack.rlim_cur = original->rlim_cur;
        if (setrlimit(RLIMIT_STACK, &stack) < 0) return -errno;
        if (getrlimit(RLIMIT_STACK, &stack) < 0) return -errno;
    }
    return stack.rlim_cur == original->rlim_cur && stack.rlim_max == original->rlim_max ? 0 : -EPERM;
}

int ptg220_restore_payload_ids(const struct ptg220_ids *original) {
    uid_t uid, euid, suid;
    gid_t gid, egid, sgid;
    if (getresuid(&uid, &euid, &suid) < 0 || getresgid(&gid, &egid, &sgid) < 0) return -errno;
    if (uid == 0 || uid != original->uid || gid != original->gid ||
        euid != 0 || suid != 0 || gid != egid || gid != sgid) return -EPERM;
    /* The fixed helper is setuid only. GIDs and supplementary groups were inherited. */
    if (prctl(PR_SET_KEEPCAPS, 0L, 0L, 0L, 0L) < 0) return -errno;
    if (setresuid(uid, uid, uid) < 0) return -errno;
    if (getresuid(&uid, &euid, &suid) < 0 || getresgid(&gid, &egid, &sgid) < 0) return -errno;
    if (uid != original->uid || euid != uid || suid != uid || (uid_t)setfsuid((uid_t)-1) != uid ||
        gid != original->gid || egid != gid || sgid != gid || (gid_t)setfsgid((gid_t)-1) != gid) {
        return -EPERM;
    }
    const int dumpable = prctl(PR_GET_DUMPABLE, 0L, 0L, 0L, 0L);
    if (dumpable < 0) return -errno;
    if (dumpable != 0) return -EPERM;
    return exact_capabilities(0);
}
