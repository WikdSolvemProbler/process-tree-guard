// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#define _GNU_SOURCE 1
#include "waitid_bridge.h"
#include <errno.h>
#include <signal.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>

_Static_assert(sizeof(int) == sizeof(int32_t), "native int width");
_Static_assert(sizeof(pid_t) == sizeof(int32_t), "native PID width");
_Static_assert((pid_t)-1 < 0, "native PID signedness");
_Static_assert(sizeof(uid_t) == sizeof(uint32_t), "native UID width");
_Static_assert((uid_t)-1 > 0, "native UID signedness");
_Static_assert(sizeof(id_t) == sizeof(uint32_t), "native wait ID width");
_Static_assert(sizeof(((siginfo_t *)0)->si_signo) == sizeof(int32_t), "native signo width");
_Static_assert(sizeof(((siginfo_t *)0)->si_errno) == sizeof(int32_t), "native signal errno width");
_Static_assert(sizeof(((siginfo_t *)0)->si_code) == sizeof(int32_t), "native code width");
_Static_assert(sizeof(((siginfo_t *)0)->si_pid) == sizeof(int32_t), "native signal PID width");
_Static_assert(sizeof(((siginfo_t *)0)->si_uid) == sizeof(uint32_t), "native signal UID width");
_Static_assert(sizeof(((siginfo_t *)0)->si_status) == sizeof(int32_t), "native status width");
_Static_assert(sizeof(siginfo_t) <= PTG_WAIT_NATIVE_CAPACITY, "native raw capacity");

uint64_t ptg_waitid_abi_word(uint32_t field) {
    switch (field) {
    case PTG_ABI_SCHEMA: return PTG_WAIT_ABI_SCHEMA;
    case PTG_ABI_FRAME_SIZE: return sizeof(struct ptg_wait_frame);
    case PTG_ABI_FRAME_ALIGN: return _Alignof(struct ptg_wait_frame);
    case PTG_ABI_CALLED_OFFSET: return offsetof(struct ptg_wait_frame, called);
    case PTG_ABI_RESULT_OFFSET: return offsetof(struct ptg_wait_frame, result);
    case PTG_ABI_CALL_ERRNO_OFFSET: return offsetof(struct ptg_wait_frame, call_errno);
    case PTG_ABI_SIGNO_OFFSET: return offsetof(struct ptg_wait_frame, signo);
    case PTG_ABI_SIGNAL_ERRNO_OFFSET: return offsetof(struct ptg_wait_frame, signal_errno);
    case PTG_ABI_CODE_OFFSET: return offsetof(struct ptg_wait_frame, code);
    case PTG_ABI_PID_OFFSET: return offsetof(struct ptg_wait_frame, pid);
    case PTG_ABI_UID_OFFSET: return offsetof(struct ptg_wait_frame, uid);
    case PTG_ABI_STATUS_OFFSET: return offsetof(struct ptg_wait_frame, status);
    case PTG_ABI_NATIVE_SIZE_OFFSET: return offsetof(struct ptg_wait_frame, native_size);
    case PTG_ABI_SIGINFO_SIZE: return sizeof(siginfo_t);
    case PTG_ABI_SIGINFO_ALIGN: return _Alignof(siginfo_t);
    case PTG_ABI_SIGINFO_SIGNO_OFFSET: return offsetof(siginfo_t, si_signo);
    case PTG_ABI_SIGINFO_ERRNO_OFFSET: return offsetof(siginfo_t, si_errno);
    case PTG_ABI_SIGINFO_CODE_OFFSET: return offsetof(siginfo_t, si_code);
    case PTG_ABI_SIGINFO_PID_OFFSET: return offsetof(siginfo_t, si_pid);
    case PTG_ABI_SIGINFO_UID_OFFSET: return offsetof(siginfo_t, si_uid);
    case PTG_ABI_SIGINFO_STATUS_OFFSET: return offsetof(siginfo_t, si_status);
    case PTG_ABI_P_PIDFD: return P_PIDFD;
    case PTG_ABI_WEXITED: return WEXITED;
    case PTG_ABI_WNOHANG: return WNOHANG;
    case PTG_ABI_SIGCHLD: return SIGCHLD;
    case PTG_ABI_CLD_EXITED: return CLD_EXITED;
    case PTG_ABI_CLD_KILLED: return CLD_KILLED;
    case PTG_ABI_CLD_DUMPED: return CLD_DUMPED;
    case PTG_ABI_NSIG: return NSIG;
    case PTG_ABI_ECHILD: return ECHILD;
    case PTG_ABI_EINTR: return EINTR;
    case PTG_ABI_EBADF: return EBADF;
    case PTG_ABI_EFAULT: return EFAULT;
    case PTG_ABI_EINVAL: return EINVAL;
    default: return UINT64_MAX;
    }
}

void ptg_waitid_once(int32_t pidfd, struct ptg_wait_frame *out,
                     unsigned char *native_bytes, uint32_t native_capacity) {
    siginfo_t info;
    memset(out, 0, sizeof(*out));
    out->result = -2; /* bridge refusal, not a fabricated kernel result */
    out->call_errno = EINVAL;
    out->native_size = (uint32_t)sizeof(info);
    if (native_bytes == NULL || native_capacity < sizeof(info)) return;
    memset(&info, 0, sizeof(info));
    errno = 0;
    const int result = waitid(P_PIDFD, (id_t)pidfd, &info, WEXITED | WNOHANG);
    const int saved_errno = errno;
    out->called = 1;
    out->result = result;
    out->call_errno = saved_errno;
    out->signo = info.si_signo;
    out->signal_errno = info.si_errno;
    out->code = info.si_code;
    out->pid = info.si_pid;
    out->uid = info.si_uid;
    out->status = info.si_status;
    memcpy(native_bytes, &info, sizeof(info));
}
