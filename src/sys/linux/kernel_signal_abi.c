// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
// Isolated installed kernel-UAPI signal ABI, never libc sigaction/sigset_t.
#define _GNU_SOURCE 1
#include "native_arming.h"
#include "payload_bootstrap.h"
#include <asm/signal.h>
#include <asm/unistd.h>
#include <errno.h>
#include <unistd.h>
#if !defined(__linux__) || !(defined(__aarch64__) || defined(__x86_64__))
#error "kernel signal ABI is restricted to Linux LP64 ARM64/x86-64"
#endif
_Static_assert(sizeof(unsigned long)==8 && sizeof(uint64_t)==8,"LP64 kernel word");
_Static_assert(sizeof(sigset_t)==8,"kernel rt_sigprocmask set size, not libc sigset_t");
_Static_assert(SIG_SETMASK==2 && SIGKILL==9 && SIGSTOP==19,"kernel signal constants");
_Static_assert(sizeof(struct sigaction)==32,"target kernel LP64 sigaction");
_Static_assert(offsetof(struct sigaction,sa_handler)==0 && offsetof(struct sigaction,sa_flags)==8
    && offsetof(struct sigaction,sa_restorer)==16 && offsetof(struct sigaction,sa_mask)==24,
    "installed target rt_sigaction offsets");
uint64_t ptg_arm_kernel_signal_abi(uint32_t word) {
    switch(word) {
    case 0:return sizeof(sigset_t);
    case 1:return 8*sizeof(sigset_t); /* kernel mask width, not x86 legacy NSIG32 */
    case 2:return SIG_SETMASK;
    case 3:return __NR_rt_sigprocmask;
    case 4:return SIGKILL;
    case 5:return SIGSTOP;
    case 6:return sizeof(unsigned long);
    default:return UINT64_MAX;
    }
}
/* Linux exec resets caught dispositions while preserving SIG_IGN. Do this
 * while every maskable kernel signal is blocked, before restoring the original
 * mask and execve. Raw UAPI includes libc-reserved signals32/33. No handler,
 * allocator, stdio, or libc cancellation wrapper is invoked after clone.
 * Source references: Linuxv5.14 arch/{x86,arm64}/include/uapi/asm/signal.h,
 * include/uapi/asm-generic/signal.h; installed-header assertions still need
 * actual target build qualification before backend activation.
 */
int ptg_payload_reset_dispositions(int32_t *failed_signal,int32_t *error) {
    *failed_signal=0; *error=0;
    for(int signo=1;signo<=64;signo++) {
        if(signo==SIGKILL || signo==SIGSTOP)continue;
        struct sigaction current={0};
        if(syscall(__NR_rt_sigaction,signo,NULL,&current,sizeof(sigset_t))<0) {
            *failed_signal=signo;*error=errno;return -1;
        }
        if(current.sa_handler==SIG_DFL || current.sa_handler==SIG_IGN)continue;
        struct sigaction reset={0};
        reset.sa_handler=SIG_DFL;
        if(syscall(__NR_rt_sigaction,signo,&reset,NULL,sizeof(sigset_t))<0) {
            *failed_signal=signo;*error=errno;return -1;
        }
        struct sigaction observed={0};
        if(syscall(__NR_rt_sigaction,signo,NULL,&observed,sizeof(sigset_t))<0) {
            *failed_signal=signo;*error=errno;return -1;
        }
        if(observed.sa_handler!=SIG_DFL) {*failed_signal=signo;*error=EPROTO;return -1;}
    }
    return 0;
}
