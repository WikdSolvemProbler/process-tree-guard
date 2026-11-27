// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_NATIVE_CONTROL_PRIMITIVES_H
#define PTG220_NATIVE_CONTROL_PRIMITIVES_H

#include "native_control.h"
#include "native_syscall.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/close_range.h>
#include <linux/sched.h>
#include <signal.h>
#include <stddef.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/timerfd.h>
#include <sys/wait.h>
#include <time.h>

_Static_assert(sizeof(siginfo_t) == 128 && _Alignof(siginfo_t) == 8, "x86-64 siginfo ABI");
_Static_assert(offsetof(siginfo_t, si_signo) == 0 && offsetof(siginfo_t, si_errno) == 4 &&
               offsetof(siginfo_t, si_code) == 8 && offsetof(siginfo_t, si_pid) == 16 &&
               offsetof(siginfo_t, si_uid) == 20 && offsetof(siginfo_t, si_status) == 24,
               "x86-64 sigchld fields");
_Static_assert(sizeof(struct sigevent) == 64 && offsetof(struct sigevent, sigev_signo) == 8 &&
               offsetof(struct sigevent, sigev_notify) == 12, "x86-64 sigevent ABI");
_Static_assert(sizeof(struct timespec) == 16 && sizeof(struct itimerspec) == 32,
               "x86-64 timer ABI");

static const uint64_t ns_per_second = UINT64_C(1000000000);
/* Linux 6.8 ktime_set saturates at the first ns of KTIME_SEC_MAX. */
static const uint64_t first_saturating_cutoff = UINT64_C(9223372036000000000);

/* Shared native algorithms. Each translation unit keeps its own compiler flags.
   The pre-exec audit includes these bodies whether inline or out of line. */
static inline int control_boottime(uint64_t *now_ns) {
    struct timespec now;
    const long result = native(SYS_clock_gettime, CLOCK_BOOTTIME, (long)&now, 0, 0, 0);
    if (result < INT_MIN || result > 0) return -EPROTO;
    if (result < 0) return (int)result;
    if (now.tv_sec < 0 || now.tv_nsec < 0 || (uint64_t)now.tv_nsec >= ns_per_second ||
        (uint64_t)now.tv_sec > (UINT64_MAX - (uint64_t)now.tv_nsec) / ns_per_second) {
        return -EOVERFLOW;
    }
    *now_ns = (uint64_t)now.tv_sec * ns_per_second + (uint64_t)now.tv_nsec;
    return 0;
}

static inline struct itimerspec control_at(uint64_t deadline_ns) {
    return (struct itimerspec){
        .it_value = {
            .tv_sec = (time_t)(deadline_ns / ns_per_second),
            .tv_nsec = (long)(deadline_ns % ns_per_second),
        },
    };
}

static inline int control_remaining_valid(const struct itimerspec *observed, uint64_t cutoff,
                            uint64_t before, uint64_t after) {
    if (before > after || observed->it_interval.tv_sec != 0 || observed->it_interval.tv_nsec != 0 ||
        observed->it_value.tv_nsec < 0 || (uint64_t)observed->it_value.tv_nsec >= ns_per_second ||
        observed->it_value.tv_sec < 0 || (uint64_t)observed->it_value.tv_sec >
        (UINT64_MAX - (uint64_t)observed->it_value.tv_nsec) / ns_per_second) return -EPROTO;
    const uint64_t remaining = (uint64_t)observed->it_value.tv_sec * ns_per_second +
        (uint64_t)observed->it_value.tv_nsec;
    const uint64_t lower = cutoff > after ? cutoff - after : 0;
    const uint64_t upper = cutoff > before ? cutoff - before : 0;
    return remaining >= lower && remaining <= upper ? 0 : -EPROTO;
}

static inline int control_poll(struct pollfd *fds, unsigned int count, int block) {
    const struct timespec zero = {0, 0};
    const struct timespec *timeout = block ? NULL : &zero;
    return (int)native(SYS_ppoll, (long)fds, count, (long)timeout, 0, sizeof(uint64_t));
}

static inline int control_deadline_fd(uint64_t deadline_ns) {
    if (deadline_ns == 0) return -EINVAL;
    if (deadline_ns >= first_saturating_cutoff) return -EOVERFLOW;
    const long fd = native(SYS_timerfd_create, CLOCK_BOOTTIME, TFD_CLOEXEC | TFD_NONBLOCK, 0, 0, 0);
    if (fd < 0) return (int)fd;
    const struct itimerspec expiration = control_at(deadline_ns);
    int result = (int)native(SYS_timerfd_settime, fd, TFD_TIMER_ABSTIME, (long)&expiration, 0, 0);
    uint64_t before = 0, after = 0;
    struct itimerspec observed;
    if (result == 0) result = control_boottime(&before);
    if (result == 0) result = (int)native(SYS_timerfd_gettime, fd, (long)&observed, 0, 0, 0);
    if (result == 0) result = control_boottime(&after);
    if (result == 0) result = control_remaining_valid(&observed, deadline_ns, before, after);
    if (result == 0 && deadline_ns <= after) {
        struct pollfd ready = {.fd = (int)fd, .events = POLLIN};
        const int polled = control_poll(&ready, 1, 0);
        if (polled != 1 || ready.revents != POLLIN) result = polled < 0 ? polled : -EPROTO;
    }
    if (result == 0) return (int)fd;
    (void)native(SYS_close, fd, 0, 0, 0, 0);
    return result;
}

static inline int control_pidfd_terminal(int pidfd) {
    if (pidfd < 0) return -EBADF;
    struct pollfd watched = {.fd = pidfd, .events = POLLIN};
    const int result = control_poll(&watched, 1, 0);
    if (result < 0) return result;
    if (result == 0 && watched.revents == 0) return 0;
    if (result == 1 && (watched.revents & POLLIN) &&
        !(watched.revents & ~(POLLIN | POLLHUP))) return 1;
    return -EPROTO;
}

static inline int control_open_self_pidfd(void) {
    const long self = native(SYS_getpid, 0, 0, 0, 0, 0);
    return (int)native(SYS_pidfd_open, self, 0, 0, 0, 0);
}

static inline int control_signal_original(int pidfd) {
    if (pidfd < 0) return -EBADF;
    return (int)native(SYS_pidfd_send_signal, pidfd, SIGKILL, 0, 0, 0);
}

static inline int control_parent_death_signal(int expected) {
    int observed = -1;
    const long result = native(SYS_prctl, PR_GET_PDEATHSIG, (long)&observed, 0, 0, 0);
    if (result < 0) return (int)result;
    return observed == expected ? 0 : -EPROTO;
}

static inline int control_verify_parent(pid_t parent, int parent_pidfd, uint64_t deadline_ns) {
    if (parent <= 0) return -EINVAL;
    if (native(SYS_getppid, 0, 0, 0, 0, 0) != parent) return -ECHILD;
    const int terminal = control_pidfd_terminal(parent_pidfd);
    if (terminal != 0) return terminal < 0 ? terminal : -ECHILD;
    uint64_t now;
    const int sampled = control_boottime(&now);
    if (sampled < 0) return sampled;
    return now < deadline_ns ? 0 : -ETIMEDOUT;
}

static inline int control_arm_parent_death(pid_t parent, int parent_pidfd, uint64_t deadline_ns) {
    const long armed = native(SYS_prctl, PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0);
    if (armed < 0) return (int)armed;
    const int verified = control_parent_death_signal(SIGKILL);
    if (verified < 0) return verified;
    return control_verify_parent(parent, parent_pidfd, deadline_ns);
}

static inline int control_disarm_parent_death(pid_t parent, int parent_pidfd, uint64_t deadline_ns) {
    const long disarmed = native(SYS_prctl, PR_SET_PDEATHSIG, 0, 0, 0, 0);
    if (disarmed < 0) return (int)disarmed;
    const int verified = control_parent_death_signal(0);
    if (verified < 0) return verified;
    return control_verify_parent(parent, parent_pidfd, deadline_ns);
}

static inline int control_original_clone(int held_leaf, struct ptg220_birth *birth) {
    birth->pid = 0;
    birth->pidfd = -1;
    struct clone_args args = {
        .flags = CLONE_PIDFD,
        .pidfd = (uintptr_t)&birth->pidfd,
        .exit_signal = SIGCHLD,
    };
    if (held_leaf >= 0) {
        args.flags |= CLONE_INTO_CGROUP;
        args.cgroup = (unsigned int)held_leaf;
    }
    const long status = native(SYS_clone3, (long)&args, sizeof(args), 0, 0, 0);
    /* Publish original-parent custody before any follow-up operation. */
    if (status > 0) birth->pid = (pid_t)status;
    return (int)status;
}

static inline int control_wait_native(int type, int id, int options, pid_t expected,
                       struct ptg_wait_frame *frame, unsigned char *evidence) {
    if (frame == NULL || evidence == NULL) return -EINVAL;
    siginfo_t signal_info;
    volatile unsigned char *const raw = (volatile unsigned char *)&signal_info;
    for (size_t i = 0; i < sizeof(signal_info); ++i) raw[i] = 0;
    const long result = native(SYS_waitid, type, id, (long)&signal_info, options, 0);
    for (size_t i = 0; i < sizeof(signal_info); ++i) evidence[i] = raw[i];
    frame->called = 1;
    frame->result = result < 0 ? -1 : (int32_t)result;
    frame->call_errno = result < 0 ? (int32_t)-result : 0;
    frame->signo = signal_info.si_signo;
    frame->signal_errno = signal_info.si_errno;
    frame->code = signal_info.si_code;
    frame->pid = signal_info.si_pid;
    frame->uid = signal_info.si_uid;
    frame->status = signal_info.si_status;
    frame->native_size = sizeof(signal_info);
    if (result != 0) return result < 0 ? (int)result : -EPROTO;
    if (frame->pid == 0 && frame->signo == 0 && frame->signal_errno == 0 &&
        frame->code == 0 && frame->uid == 0 && frame->status == 0) return 0;
    if (frame->pid <= 0 || (expected > 0 && frame->pid != expected) ||
        frame->signo != SIGCHLD || frame->signal_errno != 0) return -EPROTO;
    if (frame->code == CLD_EXITED && frame->status >= 0 && frame->status <= 255) return 1;
    if ((frame->code == CLD_KILLED || frame->code == CLD_DUMPED) &&
        frame->status > 0 && frame->status < NSIG) return 1;
    return -EPROTO;
}

static inline int control_record_wait(const struct ptg220_birth *birth, struct ptg220_wait_record *record) {
    record->frame.called = 0;
    record->before_ns = 0;
    record->after_ns = 0;
    record->before_error = control_boottime(&record->before_ns);
    record->expected_pid = birth != NULL ? birth->pid : 0;
    record->native_options = WEXITED | WNOHANG | (birth != NULL ? 0 : __WALL);
    const int native_result = control_wait_native(birth != NULL ? P_PIDFD : P_ALL,
        birth != NULL ? birth->pidfd : 0, record->native_options, record->expected_pid,
        &record->frame, record->siginfo);
    record->after_error = control_boottime(&record->after_ns);
    return native_result;
}

static inline int control_join_original(const struct ptg220_birth *birth, uint64_t cutoff,
                         struct ptg220_wait_record *record) {
    if (birth->pidfd < 0 || birth->pid <= 0) return -EINVAL;
    const int timer = control_deadline_fd(cutoff);
    if (timer < 0) return timer;
    struct pollfd fds[] = {{.fd = timer, .events = POLLIN}, {.fd = birth->pidfd, .events = POLLIN}};
    int result;
    do result = control_poll(fds, 2, 1); while (result == -EINTR);
    (void)native(SYS_close, timer, 0, 0, 0, 0);
    if (result < 0) return result;
    if (!(fds[1].revents & POLLIN) || (fds[1].revents & ~(POLLIN | POLLHUP))) {
        return fds[0].revents == POLLIN ? -ETIMEDOUT : -EPROTO;
    }
    result = control_record_wait(birth, record);
    if (result != 1) return result < 0 ? result : -EPROTO;
    if (record->before_error < 0 || record->after_error < 0 ||
        record->after_ns < record->before_ns || record->after_ns >= cutoff) return -ETIME;
    return 0;
}

#endif
