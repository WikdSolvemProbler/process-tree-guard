// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "lifecycle.h"
#include "bootstrap.h"
#include "bootstrap_wire.h"
#include "evidence.h"

#include <errno.h>
#include <signal.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static void close_fd(int fd) {
    if (fd >= 0) (void)close(fd);
}

static int hold_reaper(struct ptg220_bootstrap *state, struct ptg220_birth *r) {
    struct pollfd fds[] = {
        {.fd = state->w_control, .events = POLLIN}, {.fd = state->deadline, .events = POLLIN},
        {.fd = state->w.pidfd, .events = POLLIN},
    };
    int polled;
    do polled = ptg220_poll(fds, 3, 1); while (polled == -EINTR);
    if (polled < 0) return polled;
    if (fds[1].revents) return -ETIMEDOUT;
    if (fds[2].revents || !(fds[0].revents & POLLIN)) return -ECHILD;
    struct ptg220_bootstrap_message msg;
    int result = ptg220_bootstrap_receive(state->w_control, &msg);
    if (result < 0) return result;
    if (msg.sender != state->w.pid || msg.real_uid != state->caller_uid ||
        msg.packet.step != PTG220_BIRTH_R || msg.packet.deadline_ns != state->deadline_ns ||
        msg.packet.pid == 0 || msg.packet.pid > INT32_MAX || msg.packet.payload != 0 ||
        msg.birth_pidfd < 0 || ptg220_pidfd_terminal(msg.birth_pidfd) != 0) {
        result = -EPROTO;
    } else {
        result = ptg220_pidfd_names(msg.birth_pidfd, msg.packet.pid);
        if (result == 0) result = ptg220_pidfd_terminal(msg.birth_pidfd);
        if (result > 0) result = -ECHILD;
    }
    if (result == 0) {
        *r = (struct ptg220_birth){.pid = (pid_t)msg.packet.pid, .pidfd = msg.birth_pidfd};
        msg.birth_pidfd = -1;
        const struct ptg220_bootstrap_packet acknowledgment = {
            .version = 1, .step = PTG220_ACK_R, .pid = (uint32_t)r->pid,
            .deadline_ns = state->deadline_ns,
        };
        result = ptg220_bootstrap_send(state->w_control, &acknowledgment, -1);
    }
    ptg220_bootstrap_close(&msg);
    return result;
}

static int valid_bracket(const struct ptg220_wait_record *record, uint64_t cutoff) {
    return record->frame.called == 1 && record->frame.native_size == 128 &&
        record->before_error == 0 && record->after_error == 0 &&
        record->before_ns <= record->after_ns && record->after_ns < cutoff;
}

static int valid_scope_evidence(const struct ptg220_evidence *evidence,
                                const struct ptg220_caller_retirement *retirement) {
    const uint64_t cap = evidence->cutoff;
    const struct ptg220_native_action *kill = &evidence->controller_stop[1].kill;
    const struct ptg220_native_action *retire = &evidence->retire;
    if (evidence->stop_owner < 1 || evidence->stop_owner > 2 ||
        evidence->leaf_inode == 0 || evidence->leaf_mount == 0 ||
        !valid_bracket(&evidence->payload, cap) || !valid_bracket(&evidence->reaper, cap) ||
        !valid_bracket(&evidence->reaper_empty, cap) || !valid_bracket(&evidence->watchdog_empty, cap) ||
        evidence->reaper_empty.frame.result != -1 || evidence->reaper_empty.frame.call_errno != ECHILD ||
        evidence->watchdog_empty.frame.result != -1 || evidence->watchdog_empty.frame.call_errno != ECHILD ||
        evidence->reaper.frame.code != CLD_EXITED || evidence->reaper.frame.status != 0 ||
        kill->called != 1 || kill->native_return != 1 || kill->native_errno != 0 ||
        kill->before_error != 0 || kill->after_error != 0 || kill->before_ns > kill->after_ns ||
        kill->after_ns >= cap || retire->called != 1 || retire->native_return != 0 ||
        retire->native_errno != 0 || retire->before_error != 0 || retire->after_error != 0 ||
        retire->before_ns > retire->after_ns || retire->after_ns >= cap) return 0;
    const struct ptg220_wait_record *const peek = &evidence->reaper_peek;
    const struct ptg220_wait_record *const reaped = &evidence->reaper;
    if (!valid_bracket(peek, cap) || peek->native_options != (WEXITED | WNOHANG | WNOWAIT) ||
        reaped->native_options != (WEXITED | WNOHANG) || peek->expected_pid != reaped->expected_pid ||
        memcmp(&peek->frame, &reaped->frame, sizeof(peek->frame)) != 0 ||
        memcmp(peek->siginfo, reaped->siginfo, sizeof(peek->siginfo)) != 0 ||
        retirement->authenticated_ns == 0 || retirement->authenticated_ns > retirement->submit_before_ns ||
        retirement->submit_before_ns > retirement->submit_after_ns || retirement->submit_after_ns >= cap ||
        retirement->submit_before_ns > evidence->watchdog_ack_ns ||
        peek->after_ns > evidence->watchdog_ack_ns || evidence->watchdog_ack_ns > reaped->before_ns ||
        evidence->watchdog_failure.stage != PTG220_FAILURE_NONE) return 0;
    const uint64_t observed = evidence->controller_stop[evidence->stop_owner - 1].observed_ns;
    const uint64_t earlier = observed < evidence->deadline ? observed : evidence->deadline;
    return observed != 0 && earlier <= UINT64_MAX - PTG220_SETTLEMENT_NS &&
        cap == earlier + PTG220_SETTLEMENT_NS;
}

static int read_result(const struct ptg220_bootstrap *state, const struct ptg220_birth *r,
                       int *hard, struct ptg220_result *result, uint64_t *cutoff,
                       struct ptg220_evidence *evidence, int *r_retired,
                       struct ptg220_caller_retirement *retirement) {
    struct ptg220_settlement_failure *const failure = &retirement->failure;
again: ;
    struct pollfd fds[] = {
        {.fd = state->w_control, .events = POLLIN}, {.fd = *hard, .events = POLLIN},
        {.fd = state->w.pidfd, .events = POLLIN},
    };
    int polled;
    do polled = ptg220_poll(fds, 3, 1); while (polled == -EINTR);
    if (polled < 0) return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_RESULT_POLL,
                                                        polled, *cutoff, 0, NULL);
    const int expired = fds[1].revents == POLLIN;
    if (fds[1].revents && !expired) {
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_RESULT_POLL,
                                               -EPROTO, *cutoff, fds[1].revents, NULL);
    }
    if (!(fds[0].revents & POLLIN)) {
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_RESULT_POLL,
            expired ? -ETIMEDOUT : -ECHILD, *cutoff, fds[0].revents, NULL);
    }

    struct ptg220_bootstrap_message msg;
    struct ptg220_receive_observation observed_receive;
    int outcome = ptg220_bootstrap_receive_observed(state->w_control, &msg, &observed_receive);
    if (outcome < 0) {
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_RESULT_RECEIVE,
            expired ? -ETIMEDOUT : outcome, *cutoff, outcome, &observed_receive);
    }
    if (msg.packet.step == PTG220_W_FAILURE) {
        const uint64_t error = msg.packet.payload;
        if (msg.sender != state->w.pid || msg.real_uid != state->caller_uid ||
            msg.packet.pid != (uint32_t)state->w.pid || msg.packet.deadline_ns != state->deadline_ns ||
            msg.birth_pidfd < 0 || error == 0 || error > INT32_MAX ||
            msg.packet.cutoff_ns == 0 || msg.packet.cutoff_ns > *cutoff) {
            ptg220_bootstrap_close(&msg);
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_W_FAILURE_ENVELOPE,
                -EPROTO, *cutoff, 0, &observed_receive);
        }
        struct ptg220_evidence observed;
        outcome = ptg220_evidence_read(msg.birth_pidfd, &observed);
        ptg220_bootstrap_close(&msg);
        if (outcome < 0) {
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_SEALED_READ,
                outcome, *cutoff, 0, &observed_receive);
        }
        if (observed.deadline != state->deadline_ns ||
            observed.cutoff != msg.packet.cutoff_ns || observed.watchdog_error != (int)error) {
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_SCOPE_MATCH,
                -EPROTO, *cutoff, 0, &observed_receive);
        }
        *evidence = observed;
        *cutoff = observed.cutoff;
        return expired ? ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_RESULT_POLL,
            -ETIMEDOUT, *cutoff, fds[1].revents, &observed_receive) : -(int)error;
    }
    if (msg.packet.step == PTG220_R_SETTLED) {
        const uint64_t accepted_cutoff = msg.packet.cutoff_ns;
        if (*r_retired || msg.sender != r->pid || msg.real_uid != state->caller_uid ||
            msg.real_gid != getgid() || accepted_cutoff == 0 || accepted_cutoff > *cutoff ||
            ptg220_settlement_packet(&msg, PTG220_R_SETTLED, r->pid, state->deadline_ns, accepted_cutoff) != 0) {
            ptg220_bootstrap_close(&msg);
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_R_ENVELOPE,
                -EPROTO, *cutoff, 0, &observed_receive);
        }
        outcome = ptg220_bind_pidfds(r->pidfd, msg.sender_pidfd, (uint32_t)r->pid);
        ptg220_bootstrap_close(&msg);
        if (outcome < 0) {
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_R_BIND,
                outcome, *cutoff, 0, &observed_receive);
        }
        *r_retired = 1;
        const uint64_t previous_cutoff = *cutoff;
        *cutoff = accepted_cutoff;
        if (expired) {
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_R_ACK_SEND,
                -ETIMEDOUT, *cutoff, 0, &observed_receive);
        }
        outcome = ptg220_boottime(&retirement->authenticated_ns);
        if (outcome == 0 && retirement->authenticated_ns >= *cutoff) outcome = -ETIMEDOUT;
        if (outcome < 0) {
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_R_ACK_SEND,
                outcome, *cutoff, 1, &observed_receive);
        }
        if (*cutoff != previous_cutoff) {
            const int shorter = ptg220_deadline_fd(*cutoff);
            if (shorter < 0) {
                return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_R_REARM,
                    shorter, *cutoff, 0, &observed_receive);
            }
            close_fd(*hard);
            *hard = shorter;
        }
        const struct ptg220_bootstrap_packet ack = {
            .version = 1, .step = PTG220_ACK_R_SETTLED, .pid = (uint32_t)r->pid,
            .deadline_ns = state->deadline_ns, .cutoff_ns = *cutoff,
        };
        outcome = ptg220_boottime(&retirement->submit_before_ns);
        if (outcome == 0 && retirement->submit_before_ns >= *cutoff) outcome = -ETIMEDOUT;
        if (outcome < 0) {
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_R_ACK_SEND,
                outcome, *cutoff, 2, &observed_receive);
        }
        outcome = ptg220_bootstrap_send(state->w_control, &ack, -1);
        if (outcome < 0) {
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_R_ACK_SEND,
                outcome, *cutoff, 3, &observed_receive);
        }
        outcome = ptg220_boottime(&retirement->submit_after_ns);
        if (outcome == 0 && retirement->submit_after_ns < retirement->submit_before_ns) outcome = -EPROTO;
        if (outcome == 0 && retirement->submit_after_ns >= *cutoff) outcome = -ETIMEDOUT;
        if (outcome < 0) {
            return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_R_ACK_SEND,
                outcome, *cutoff, 4, &observed_receive);
        }
        goto again;
    }
    const uint64_t code = msg.packet.payload >> 32;
    const uint32_t value = (uint32_t)msg.packet.payload;
    const struct ptg_wait_frame wait = msg.packet.native_wait;
    if (!*r_retired || msg.sender != state->w.pid || msg.real_uid != state->caller_uid ||
        msg.packet.pid != (uint32_t)state->w.pid || msg.packet.step != PTG220_W_RESULT ||
        msg.packet.deadline_ns != state->deadline_ns || msg.birth_pidfd < 0 ||
        msg.packet.cutoff_ns == 0 || msg.packet.cutoff_ns != *cutoff ||
        code < PTG220_EXITED || code > PTG220_EXEC_REFUSED ||
        wait.called != 1 || wait.result != 0 || wait.pid <= 0 || wait.signo != SIGCHLD ||
        (code == PTG220_EXITED && (wait.code != CLD_EXITED || value != (uint32_t)wait.status ||
                                  value > 255)) ||
        (code == PTG220_SIGNALED && ((wait.code != CLD_KILLED && wait.code != CLD_DUMPED) ||
                                   value != (uint32_t)wait.status || value == 0 || value >= NSIG)) ||
        (code == PTG220_EXEC_REFUSED && (value == 0 || value > INT32_MAX))) {
        ptg220_bootstrap_close(&msg);
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_W_RESULT_ENVELOPE,
            -EPROTO, *cutoff, 0, &observed_receive);
    }
    struct ptg220_evidence observed;
    outcome = ptg220_evidence_read(msg.birth_pidfd, &observed);
    ptg220_bootstrap_close(&msg);
    if (outcome < 0) {
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_SEALED_READ,
            outcome, *cutoff, 0, &observed_receive);
    }
    if (observed.deadline != state->deadline_ns || observed.cutoff != *cutoff ||
        observed.reaper.expected_pid != r->pid || !valid_scope_evidence(&observed, retirement) ||
        memcmp(&observed.payload.frame, &wait, sizeof(wait)) != 0) {
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_SCOPE_MATCH,
            -EPROTO, *cutoff, 0, &observed_receive);
    }
    *evidence = observed;
    *result = (struct ptg220_result){.outcome = (enum ptg220_outcome)code, .value = (int)value,
                                   .worker_wait = wait, .evidence_fd = -1};
    return ptg220_note_settlement_failure(failure, PTG220_FAILURE_A_RESULT_POLL,
        expired ? -ETIMEDOUT : 0, *cutoff, fds[1].revents, &observed_receive);
}

static int archive_result(struct ptg220_result *result, struct ptg220_evidence *evidence,
                          const struct ptg220_bootstrap *state, int status) {
    evidence->final_error = status < 0 ? -status : 0;
    evidence->bootstrap = state->b_wait;
    evidence->watchdog = state->w_wait;
    const int archived = ptg220_evidence_seal(evidence);
    if (archived >= 0) result->evidence_fd = archived;
    return status == 0 && archived < 0 ? archived : status;
}

int ptg220_run(const char *path, char *const argv[], char *const envp[],
               uint64_t deadline_ns, const int *original_owner_pidfds, unsigned int owner_count,
               const struct ptg220_owner_identity *originals, unsigned int owner_mode,
               struct ptg220_result *result) {
    if (result == NULL) return -EINVAL;
    *result = (struct ptg220_result){.outcome = PTG220_UNCONFIRMED, .evidence_fd = -1};
    int status = ptg220_caller_preflight();
    if (deadline_ns == 0 || deadline_ns > UINT64_MAX - PTG220_SETTLEMENT_NS) {
        status = -EOVERFLOW;
    }
    if (status < 0) {
        result->guard_errno = -status;
        return status;
    }
    struct ptg220_descriptors descriptors;
    status = ptg220_descriptor_capture(&descriptors);
    if (status < 0) {
        result->guard_errno = -status;
        return status;
    }
    const int request = ptg220_request_capture(path, argv, envp);
    if (request < 0) {
        ptg220_descriptor_drop(&descriptors);
        result->guard_errno = -request;
        return request;
    }
    struct ptg220_bootstrap state;
    uint64_t cutoff = deadline_ns + PTG220_SETTLEMENT_NS;
    struct ptg220_evidence evidence = {
        .version = PTG220_EVIDENCE_VERSION, .deadline = deadline_ns, .cutoff = cutoff,
    };
    status = ptg220_bootstrap_begin(deadline_ns, original_owner_pidfds, owner_count, originals,
                                    owner_mode, request, &descriptors, &state);
    close_fd(request);
    ptg220_descriptor_drop(&descriptors);
    if (status == 0) status = ptg220_bootstrap_adopt(&state);

    if (!state.adopted) {
        const int retired = ptg220_bootstrap_rollback(&state);
        if (retired < 0) status = retired;
        result->guard_errno = status < 0 ? -status : EPROTO;
        (void)archive_result(result, &evidence, &state, -result->guard_errno);
        close_fd(state.b.pidfd);
        close_fd(state.w.pidfd);
        return -result->guard_errno;
    }

    struct ptg220_birth r = {.pidfd = -1};
    status = hold_reaper(&state, &r);
    int hard = ptg220_deadline_fd(cutoff);
    int r_retired = 0;
    struct ptg220_caller_retirement retirement = {0};
    if (status < 0) {
        (void)ptg220_note_settlement_failure(&retirement.failure, PTG220_FAILURE_A_R_BIRTH,
                                            status, cutoff, 0, NULL);
    }
    if (status == 0 && hard < 0) {
        status = ptg220_note_settlement_failure(&retirement.failure, PTG220_FAILURE_A_INITIAL_TIMER,
            hard, cutoff, 0, NULL);
    }
    if (status == 0) status = read_result(&state, &r, &hard, result, &cutoff, &evidence, &r_retired, &retirement);
    if (status == -ETIMEDOUT && r_retired) (void)ptg220_signal_original(state.w.pidfd);
    close_fd(hard);
    close_fd(state.b_control);
    close_fd(state.w_control);
    close_fd(state.deadline);

    /* W and R retain their own original stop authority past the adoption
       boundary. A must not kill one on a read failure before its first stop. */
    const int w_terminal = ptg220_join_original(&state.w, cutoff, &state.w_wait);
    if (status == 0 && w_terminal < 0) status = w_terminal;
    if (status == 0 && (state.w_wait.frame.code != CLD_EXITED || state.w_wait.frame.status != 0 ||
                        r.pidfd < 0 || ptg220_pidfd_terminal(r.pidfd) != 1)) status = -EUCLEAN;
    if (ptg220_original_terminal(&state.w, &state.w_wait) && r.pidfd >= 0) {
        struct ptg_wait_frame r_wait;
        unsigned char peek_bytes[PTG_WAIT_NATIVE_CAPACITY];
        const int r_owned = ptg220_peek_original(&r, &r_wait, peek_bytes);
        if (r_owned >= 0) {
            if (w_terminal == 0) (void)ptg220_join_original(&r, cutoff, &evidence.reaper);
            else (void)ptg220_record_original_reap(&r, &evidence.reaper);
            if (status == 0) status = -EUCLEAN;
        } else if (r_owned != -ECHILD && status == 0) status = r_owned;
    }
    struct ptg_wait_frame remaining;
    unsigned char raw[PTG_WAIT_NATIVE_CAPACITY];
    if (ptg220_wait_adopted(1, &remaining, raw) != -ECHILD && status == 0) status = -EUCLEAN;
    close_fd(state.b.pidfd);
    close_fd(state.w.pidfd);
    close_fd(r.pidfd);
    evidence.cutoff = cutoff;
    if (status < 0 && retirement.failure.stage == PTG220_FAILURE_NONE && evidence.watchdog_error == 0) {
        (void)ptg220_note_settlement_failure(&retirement.failure, PTG220_FAILURE_A_TERMINAL_JOIN,
                                            status, cutoff, 0, NULL);
    }
    evidence.caller_retirement = retirement;
    status = archive_result(result, &evidence, &state, status);
    if (status < 0) {
        result->outcome = PTG220_UNCONFIRMED;
        result->guard_errno = -status;
    }
    return status;
}
