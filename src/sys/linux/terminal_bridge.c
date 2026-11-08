// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#define _GNU_SOURCE 1
#include "terminal_bridge.h"
#include "owner_watch.h"
#include <errno.h>
#include <poll.h>
#include <signal.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

_Static_assert(sizeof(siginfo_t) <= PTG_WAIT_NATIVE_CAPACITY, "native wait capacity");
_Static_assert(sizeof(pid_t) == 4 && (pid_t)-1 < 0, "native PID ABI");
_Static_assert(sizeof(uid_t) == 4 && (uid_t)-1 > 0, "native UID ABI");
_Static_assert(sizeof(struct pollfd) == 8 && offsetof(struct pollfd, events) == 4
               && offsetof(struct pollfd, revents) == 6, "native poll frame ABI");

int32_t ptg_terminal_self(uint32_t which) {
    if (which == 0) return (int32_t)getpid();
    if (which == 1) return (int32_t)syscall(SYS_gettid);
    return -1;
}

int32_t ptg_terminal_self_pidfd(int32_t *error) {
    const int result = (int)syscall(SYS_pidfd_open, getpid(), 0);
    *error = result < 0 ? errno : 0;
    return result;
}

int32_t ptg_terminal_sigchld(uint32_t *facts, int32_t *error) {
    struct sigaction action;
    sigset_t mask;
    *facts = 0;
    if (sigaction(SIGCHLD, NULL, &action) != 0 || sigprocmask(SIG_BLOCK, NULL, &mask) != 0) {
        *error = errno;
        return -1;
    }
    if (action.sa_handler == SIG_DFL) *facts |= 1;
    if ((action.sa_flags & SA_NOCLDWAIT) == 0) *facts |= 2;
    if (sigismember(&mask, SIGCHLD) == 0) *facts |= 4;
    *error = 0;
    return 0;
}

int32_t ptg_terminal_subreaper(uint32_t operation, int32_t *value, int32_t *error) {
    int result;
    if (operation == 0) {
        result = prctl(PR_GET_CHILD_SUBREAPER, value, 0L, 0L, 0L);
    } else if (operation == 1 || operation == 2) {
        result = prctl(PR_SET_CHILD_SUBREAPER, operation == 1 ? 1L : 0L, 0L, 0L, 0L);
    } else {
        *error = EINVAL;
        return -1;
    }
    *error = result < 0 ? errno : 0;
    return result;
}

int32_t ptg_terminal_kill(int32_t pidfd, int32_t *error) {
    const int result = (int)syscall(SYS_pidfd_send_signal, pidfd, SIGKILL, NULL, 0);
    *error = result < 0 ? errno : 0;
    return result;
}

uint32_t ptg_terminal_errno_kind(int32_t error) {
    if (error == EINTR) return 1;
    if (error == EAGAIN || error == EWOULDBLOCK) return 2;
    if (error == ESRCH) return 3;
    return 0;
}

int32_t ptg_terminal_poll(void *fds, uint32_t count, int32_t timeout, int32_t *error) {
    if (fds == NULL || count == 0 || count > PTG_OWNER_WATCH_MAX + 5 || timeout < -1) {
        *error = EINVAL;
        return -1;
    }
    const int result = poll((struct pollfd *)fds, (nfds_t)count, timeout);
    *error = result < 0 ? errno : 0;
    return result;
}

void ptg_terminal_wait_any(uint32_t peek, struct ptg_wait_frame *out,
                         unsigned char *native_bytes, uint32_t native_capacity) {
    siginfo_t info;
    memset(out, 0, sizeof(*out));
    out->result = -2;
    out->call_errno = EINVAL;
    out->native_size = (uint32_t)sizeof(info);
    if (peek > 1 || native_bytes == NULL || native_capacity < sizeof(info)) return;
    memset(&info, 0, sizeof(info));
    errno = 0;
    // Include clone children with non-SIGCHLD exit signals. Without __WALL an
    // initial ECHILD would not establish that this dedicated process is empty.
    const int result = waitid(P_ALL, 0, &info, WEXITED | WNOHANG | __WALL | (peek ? WNOWAIT : 0));
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

uint32_t ptg_terminal_exec_read(int32_t fd, unsigned char *bytes, uint32_t capacity,
                              int32_t *count, int32_t *error) {
    if (capacity == 0 || capacity > 9) {
        *count = -1; *error = EINVAL; return 4;
    }
    const ssize_t result = read(fd, bytes, capacity);
    *count = (int32_t)result;
    *error = result < 0 ? errno : 0;
    if (result > 0) return 0;
    if (result == 0) return 1;
    if (*error == EAGAIN || *error == EWOULDBLOCK) return 2;
    if (*error == EINTR) return 3;
    return 4;
}
