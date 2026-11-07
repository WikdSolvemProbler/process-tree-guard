// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#ifndef PTG_WAITID_BRIDGE_H
#define PTG_WAITID_BRIDGE_H

#include <stddef.h>
#include <stdint.h>

#define PTG_WAIT_ABI_SCHEMA 1u
#define PTG_WAIT_NATIVE_CAPACITY 512u

/* A deliberately fixed wire frame, not a copy of native siginfo_t layout. */
struct ptg_wait_frame {
    uint32_t called;
    int32_t result;
    int32_t call_errno;
    int32_t signo;
    int32_t signal_errno;
    int32_t code;
    int32_t pid;
    uint32_t uid;
    int32_t status;
    uint32_t native_size;
};

_Static_assert(sizeof(struct ptg_wait_frame) == 40, "wait frame size");
_Static_assert(_Alignof(struct ptg_wait_frame) == 4, "wait frame alignment");
_Static_assert(offsetof(struct ptg_wait_frame, called) == 0, "called offset");
_Static_assert(offsetof(struct ptg_wait_frame, result) == 4, "result offset");
_Static_assert(offsetof(struct ptg_wait_frame, call_errno) == 8, "errno offset");
_Static_assert(offsetof(struct ptg_wait_frame, signo) == 12, "signo offset");
_Static_assert(offsetof(struct ptg_wait_frame, signal_errno) == 16, "signal errno offset");
_Static_assert(offsetof(struct ptg_wait_frame, code) == 20, "code offset");
_Static_assert(offsetof(struct ptg_wait_frame, pid) == 24, "pid offset");
_Static_assert(offsetof(struct ptg_wait_frame, uid) == 28, "uid offset");
_Static_assert(offsetof(struct ptg_wait_frame, status) == 32, "status offset");
_Static_assert(offsetof(struct ptg_wait_frame, native_size) == 36, "native size offset");

enum ptg_wait_abi_field {
    PTG_ABI_SCHEMA = 0,
    PTG_ABI_FRAME_SIZE = 1,
    PTG_ABI_FRAME_ALIGN = 2,
    PTG_ABI_CALLED_OFFSET = 3,
    PTG_ABI_RESULT_OFFSET = 4,
    PTG_ABI_CALL_ERRNO_OFFSET = 5,
    PTG_ABI_SIGNO_OFFSET = 6,
    PTG_ABI_SIGNAL_ERRNO_OFFSET = 7,
    PTG_ABI_CODE_OFFSET = 8,
    PTG_ABI_PID_OFFSET = 9,
    PTG_ABI_UID_OFFSET = 10,
    PTG_ABI_STATUS_OFFSET = 11,
    PTG_ABI_NATIVE_SIZE_OFFSET = 12,
    PTG_ABI_SIGINFO_SIZE = 13,
    PTG_ABI_SIGINFO_ALIGN = 14,
    PTG_ABI_SIGINFO_SIGNO_OFFSET = 15,
    PTG_ABI_SIGINFO_ERRNO_OFFSET = 16,
    PTG_ABI_SIGINFO_CODE_OFFSET = 17,
    PTG_ABI_SIGINFO_PID_OFFSET = 18,
    PTG_ABI_SIGINFO_UID_OFFSET = 19,
    PTG_ABI_SIGINFO_STATUS_OFFSET = 20,
    PTG_ABI_P_PIDFD = 21,
    PTG_ABI_WEXITED = 22,
    PTG_ABI_WNOHANG = 23,
    PTG_ABI_SIGCHLD = 24,
    PTG_ABI_CLD_EXITED = 25,
    PTG_ABI_CLD_KILLED = 26,
    PTG_ABI_CLD_DUMPED = 27,
    PTG_ABI_NSIG = 28,
    PTG_ABI_ECHILD = 29,
    PTG_ABI_EINTR = 30,
    PTG_ABI_EBADF = 31,
    PTG_ABI_EFAULT = 32,
    PTG_ABI_EINVAL = 33,
    PTG_ABI_WORD_COUNT = 34
};

uint64_t ptg_waitid_abi_word(uint32_t field);

/* The caller provides one valid frame and native_capacity writable bytes.
 * No caller-supplied PID or wait flags are accepted. Exactly one waitid call
 * is made when the raw buffer is sufficient; called=0 reports no call.
 */
void ptg_waitid_once(int32_t pidfd, struct ptg_wait_frame *out,
                     unsigned char *native_bytes, uint32_t native_capacity);

#endif
