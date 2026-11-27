// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "native_control_primitives.h"

/* The existing crate::deadline::MAX_SECONDS (86,400), not a new profile. */
static const uint64_t invocation_limit_ns = UINT64_C(86400000000000);

int ptg220_boottime(uint64_t *now_ns) {
    return control_boottime(now_ns);
}

int ptg220_validate_invocation_deadline(uint64_t deadline_ns) {
    if (deadline_ns >= first_saturating_cutoff - PTG220_SETTLEMENT_NS) return -EOVERFLOW;
    uint64_t now;
    const int sampled = ptg220_boottime(&now);
    if (sampled < 0) return sampled;
    if (deadline_ns <= now) return -ETIMEDOUT;
    if (deadline_ns - now > invocation_limit_ns) return -EINVAL;
    /* Host conversion applies the current BOOTTIME namespace offset.
       Prove the entire existing horizon with this kernel before admission. */
    const int limit = ptg220_deadline_fd(deadline_ns + PTG220_SETTLEMENT_NS);
    if (limit < 0) return limit;
    const long closed = native(SYS_close, limit, 0, 0, 0, 0);
    return closed < 0 ? (int)closed : 0;
}

int ptg220_deadline_fd(uint64_t deadline_ns) {
    return control_deadline_fd(deadline_ns);
}

int ptg220_poll(struct pollfd *fds, unsigned int count, int block) {
    return control_poll(fds, count, block);
}

int ptg220_pidfd_terminal(int pidfd) {
    return control_pidfd_terminal(pidfd);
}

int ptg220_open_self_pidfd(void) {
    return control_open_self_pidfd();
}

int ptg220_signal_original(int pidfd) {
    return control_signal_original(pidfd);
}

int ptg220_arm_parent_death(pid_t parent, int parent_pidfd, uint64_t deadline_ns) {
    return control_arm_parent_death(parent, parent_pidfd, deadline_ns);
}

int ptg220_disarm_parent_death(pid_t parent, int parent_pidfd, uint64_t deadline_ns) {
    return control_disarm_parent_death(parent, parent_pidfd, deadline_ns);
}

int ptg220_clone_controller(struct ptg220_birth *birth) {
    return control_original_clone(-1, birth);
}

int ptg220_clone_payload(int held_leaf, struct ptg220_birth *birth) {
    if (held_leaf < 0) return -EBADF;
    return control_original_clone(held_leaf, birth);
}

int ptg220_wait_original(const struct ptg220_birth *birth, struct ptg_wait_frame *frame,
                         unsigned char evidence[PTG_WAIT_NATIVE_CAPACITY]) {
    if (birth->pid <= 0 || birth->pidfd < 0) return -EINVAL;
    return control_wait_native(P_PIDFD, birth->pidfd, WEXITED | WNOHANG, birth->pid, frame, evidence);
}

int ptg220_peek_original(const struct ptg220_birth *birth, struct ptg_wait_frame *frame,
                         unsigned char evidence[PTG_WAIT_NATIVE_CAPACITY]) {
    if (birth->pid <= 0 || birth->pidfd < 0) return -EINVAL;
    return control_wait_native(P_PIDFD, birth->pidfd, WEXITED | WNOHANG | WNOWAIT, birth->pid, frame, evidence);
}

int ptg220_wait_adopted(int peek, struct ptg_wait_frame *frame,
                        unsigned char evidence[PTG_WAIT_NATIVE_CAPACITY]) {
    if (peek != 0 && peek != 1) return -EINVAL;
    return control_wait_native(P_ALL, 0, WEXITED | WNOHANG | __WALL | (peek ? WNOWAIT : 0),
                       0, frame, evidence);
}

int ptg220_record_exclusive_reap(struct ptg220_wait_record *record) {
    return control_record_wait(NULL, record);
}

int ptg220_record_original_peek(const struct ptg220_birth *birth,
                               struct ptg220_wait_record *record) {
    if (birth->pidfd < 0 || birth->pid <= 0) return -EINVAL;
    *record = (struct ptg220_wait_record){.expected_pid = birth->pid,
                                        .native_options = WEXITED | WNOHANG | WNOWAIT};
    record->before_error = control_boottime(&record->before_ns);
    const int result = control_wait_native(P_PIDFD, birth->pidfd, record->native_options,
        record->expected_pid, &record->frame, record->siginfo);
    record->after_error = control_boottime(&record->after_ns);
    return result;
}

int ptg220_record_original_reap(const struct ptg220_birth *birth,
                               struct ptg220_wait_record *record) {
    if (birth->pidfd < 0 || birth->pid <= 0) return -EINVAL;
    return control_record_wait(birth, record);
}

int ptg220_original_terminal(const struct ptg220_birth *birth,
                             const struct ptg220_wait_record *record) {
    const struct ptg_wait_frame *const frame = &record->frame;
    return frame->called == 1 && frame->native_size == 128 && frame->result == 0 &&
        frame->call_errno == 0 && frame->signal_errno == 0 && frame->signo == SIGCHLD &&
        frame->pid == birth->pid && record->expected_pid == birth->pid &&
        ((frame->code == CLD_EXITED && frame->status >= 0 && frame->status <= 255) ||
         ((frame->code == CLD_KILLED || frame->code == CLD_DUMPED) &&
          frame->status > 0 && frame->status < NSIG));
}

int ptg220_join_original(const struct ptg220_birth *birth, uint64_t cutoff,
                         struct ptg220_wait_record *record) {
    return control_join_original(birth, cutoff, record);
}

int ptg220_delete_terminal_timer(struct ptg220_terminal_timer *timer) {
    if (timer->state == PTG220_TIMER_EMPTY || timer->native_id < 0) return -EINVAL;
    const long result = native(SYS_timer_delete, timer->native_id, 0, 0, 0, 0);
    timer->disposal_error = result < 0 ? (int)-result : 0;
    if (result < 0) {
        timer->state = PTG220_TIMER_UNCERTAIN;
        return (int)result;
    }
    timer->native_id = -1;
    timer->state = PTG220_TIMER_EMPTY;
    timer->cutoff_ns = 0;
    return 0;
}

static int verify_timer_bracket(const struct ptg220_terminal_timer *timer) {
    uint64_t before, after;
    int error = ptg220_boottime(&before);
    if (error < 0) return error;
    struct itimerspec observed;
    const long readback = native(SYS_timer_gettime, timer->native_id, (long)&observed, 0, 0, 0);
    if (readback < 0) return (int)readback;
    error = ptg220_boottime(&after);
    if (error < 0) return error;
    if (after >= timer->cutoff_ns) return -ETIME;
    return control_remaining_valid(&observed, timer->cutoff_ns, before, after);
}

static int program_terminal_timer(struct ptg220_terminal_timer *timer, uint64_t cutoff_ns) {
    if (cutoff_ns >= first_saturating_cutoff) return -EOVERFLOW;
    uint64_t now;
    const int sampled = ptg220_boottime(&now);
    if (sampled < 0) return sampled;
    if (now >= cutoff_ns) return -ETIMEDOUT;
    const struct itimerspec expiration = control_at(cutoff_ns);
    const long armed = native(SYS_timer_settime, timer->native_id, TIMER_ABSTIME,
                              (long)&expiration, 0, 0);
    if (armed < 0) return (int)armed;
    const int replacing = timer->state == PTG220_TIMER_ARMED;
    timer->cutoff_ns = cutoff_ns;
    timer->state = PTG220_TIMER_UNCERTAIN;
    const int verified = verify_timer_bracket(timer);
    if (verified < 0) {
        if (!replacing) (void)ptg220_delete_terminal_timer(timer);
        return verified;
    }
    timer->state = PTG220_TIMER_ARMED;
    return 0;
}

int ptg220_arm_terminal_timer(struct ptg220_terminal_timer *timer, uint64_t cutoff_ns) {
    if (timer->state != PTG220_TIMER_EMPTY || timer->native_id != -1 || cutoff_ns == 0) return -EINVAL;
    struct sigevent signal_event = {.sigev_notify = SIGEV_SIGNAL, .sigev_signo = SIGKILL};
    int timer_id = -1;
    const long created = native(SYS_timer_create, CLOCK_BOOTTIME, (long)&signal_event,
                                (long)&timer_id, 0, 0);
    if (created < 0) return (int)created;
    timer->native_id = timer_id;
    timer->state = PTG220_TIMER_UNCERTAIN;
    const int result = program_terminal_timer(timer, cutoff_ns);
    if (result < 0 && timer->state == PTG220_TIMER_UNCERTAIN && timer->cutoff_ns == 0) {
        (void)ptg220_delete_terminal_timer(timer);
    }
    return result;
}

int ptg220_tighten_terminal_timer(struct ptg220_terminal_timer *timer, uint64_t cutoff_ns) {
    if (timer->state != PTG220_TIMER_ARMED || timer->native_id < 0 || cutoff_ns == 0 ||
        cutoff_ns > timer->cutoff_ns) return -EINVAL;
    return program_terminal_timer(timer, cutoff_ns);
}
