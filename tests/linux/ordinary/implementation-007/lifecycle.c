// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "lifecycle.h"
#include "bootstrap_wire.h"
#include "lease.h"
#include "native_syscall.h"

#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdatomic.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

struct scope {
    struct ptg220_lease lease;
    _Atomic unsigned int stop_owner;
    _Atomic unsigned int finished;
    _Atomic unsigned int exec_stage;
    int exec_errno;
    struct ptg220_result payload_result;
    struct ptg220_evidence evidence;
};

static void close_fd(int fd) {
    if (fd >= 0) (void)native(SYS_close, fd, 0, 0, 0, 0);
}

static _Noreturn void leave(int code) {
    (void)native(SYS_exit_group, code, 0, 0, 0, 0);
    __builtin_trap();
}

static int notify(int channel, const struct ptg220_helper_startup *start,
                  enum ptg220_bootstrap_step step, pid_t pid, uint64_t details, int fd) {
    const struct ptg220_bootstrap_packet packet = {
        .version = 1, .step = step, .pid = (uint32_t)pid,
        .deadline_ns = start->deadline_ns, .payload = details,
    };
    return ptg220_bootstrap_send(channel, &packet, fd);
}

static int next_event(const struct ptg220_helper_startup *start, int channel,
                      int actor_fd, int block) {
    struct pollfd fds[PTG_OWNER_WATCH_MAX + 3];
    for (unsigned int i = 0; i < start->owner_count; ++i) {
        fds[i] = (struct pollfd){.fd = start->owners[i], .events = POLLIN};
    }
    const unsigned int n = start->owner_count;
    fds[n] = (struct pollfd){.fd = start->deadline, .events = POLLIN};
    fds[n + 1] = (struct pollfd){.fd = channel, .events = POLLIN};
    fds[n + 2] = (struct pollfd){.fd = actor_fd, .events = POLLIN};
    int polled;
    do polled = ptg220_poll(fds, n + 3, block); while (polled == -EINTR);
    if (polled < 0) return polled;
    for (unsigned int i = 0; i < n; ++i) {
        if ((fds[i].revents & POLLIN) && !(fds[i].revents & ~(POLLIN | POLLHUP))) return -ECANCELED;
        if (fds[i].revents) return -EPROTO;
    }
    if ((fds[n + 2].revents & POLLIN) && !(fds[n + 2].revents & ~(POLLIN | POLLHUP))) return 2;
    if (fds[n + 2].revents) return -EPROTO;
    uint64_t now;
    const int sampled = ptg220_boottime(&now);
    if (sampled < 0) return sampled;
    if (now >= start->deadline_ns || fds[n].revents == POLLIN) return -ETIMEDOUT;
    if (fds[n].revents) return -EPROTO;
    if (fds[n + 1].revents & POLLIN) return 1;
    if (fds[n + 1].revents) return -EPIPE;
    return 0;
}

static int read_step(const struct ptg220_helper_startup *start, int channel, int actor_fd,
                     pid_t pid, enum ptg220_bootstrap_step step,
                     struct ptg220_bootstrap_message *msg) {
    int result = next_event(start, channel, actor_fd, 1);
    if (result != 1) return result < 0 ? result : -ECHILD;
    result = ptg220_bootstrap_receive(channel, msg);
    if (result < 0) return result;
    if (msg->sender == pid && msg->real_uid == start->original_ids.uid &&
        msg->packet.step == (uint32_t)step && msg->packet.deadline_ns == start->deadline_ns) return 0;
    ptg220_bootstrap_close(msg);
    return -EPROTO;
}

static int expect(const struct ptg220_helper_startup *start, int channel, int actor_fd,
                  pid_t pid, enum ptg220_bootstrap_step step, pid_t subject) {
    struct ptg220_bootstrap_message message;
    int result = read_step(start, channel, actor_fd, pid, step, &message);
    if (result < 0) return result;
    if (message.packet.pid != (uint32_t)subject || message.packet.payload != 0 ||
        message.birth_pidfd >= 0) result = -EPROTO;
    ptg220_bootstrap_close(&message);
    return result;
}


static int wait_creator_terminal(int pidfd, uint64_t cutoff) {
    const int timer = ptg220_deadline_fd(cutoff);
    if (timer < 0) return timer;
    struct pollfd fds[] = {{.fd = timer, .events = POLLIN}, {.fd = pidfd, .events = POLLIN}};
    int result;
    do result = ptg220_poll(fds, 2, 1); while (result == -EINTR);
    if (result >= 0) {
        result = ptg220_pidfd_terminal(pidfd);
        if (result == 1) {
            uint64_t now;
            result = ptg220_boottime(&now);
            if (result == 0 && now >= cutoff) result = -ETIME;
        } else if (result == 0) result = -ETIMEDOUT;
    }
    close_fd(timer);
    return result;
}

static int stop_scope(struct scope *scope, const struct ptg220_leaf *leaf,
                      unsigned int lane, enum ptg220_outcome cause, uint64_t selected,
                      uint64_t *cutoff) {
    struct ptg220_stop_evidence *const current = &scope->evidence.controller_stop[lane];
    current->cause = (uint32_t)cause;
    current->observed_ns = selected;
    unsigned int open = 0;
    (void)atomic_compare_exchange_strong_explicit(&scope->stop_owner, &open, lane + 1,
                                                  memory_order_acq_rel, memory_order_acquire);
    ptg220_lease_seal(&scope->lease);
    const int kill = leaf->kill >= 0 ? ptg220_leaf_kill_observed(leaf, selected, &current->kill) : 0;
    const unsigned int first = atomic_load_explicit(&scope->stop_owner, memory_order_acquire);
    if (first == 0 || first > 2) return -EPROTO;
    uint64_t from = scope->evidence.controller_stop[first - 1].observed_ns;
    if (from > scope->evidence.deadline) from = scope->evidence.deadline;
    *cutoff = from + PTG220_SETTLEMENT_NS;
    return kill;
}

static int all_owned_reaped(uint64_t cutoff, int child_events,
                            struct ptg220_wait_record *terminal, uint32_t *joined) {
    const int timer = ptg220_deadline_fd(cutoff);
    if (timer < 0) return timer;
    int result = 0;
    for (;;) {
        uint64_t now;
        result = ptg220_boottime(&now);
        if (result < 0) break;
        if (now >= cutoff) {
            result = -ETIMEDOUT;
            break;
        }
        result = ptg220_record_exclusive_reap(terminal);
        if (result == -ECHILD) {
            result = terminal->before_error != 0 || terminal->after_error != 0 ||
                terminal->after_ns < terminal->before_ns || terminal->after_ns >= cutoff ? -ETIME : 0;
            break;
        }
        if (result == 1) {
            if (*joined == UINT32_MAX) {
                result = -EOVERFLOW;
                break;
            }
            ++*joined;
            continue;
        }
        if (result == -EINTR) continue;
        if (result < 0) break;
        struct pollfd fds[] = {{.fd = timer, .events = POLLIN}, {.fd = child_events, .events = POLLIN}};
        result = ptg220_poll(fds, 2, 1);
        if (result == -EINTR) continue;
        if (result < 0) break;
        if (fds[0].revents || !(fds[1].revents & POLLIN)) {
            result = fds[0].revents == POLLIN ? -ETIMEDOUT : -EPROTO;
            break;
        }
        unsigned char event[128];
        const long consumed = native(SYS_read, child_events, (long)event, sizeof(event), 0, 0);
        if (consumed != (long)sizeof(event) && consumed != -EAGAIN && consumed != -EINTR) {
            result = consumed < 0 ? (int)consumed : -EPROTO;
            break;
        }
    }
    close_fd(timer);
    return result;
}

static int arm_reaper(void) {
    if (prctl(PR_SET_CHILD_SUBREAPER, 1L, 0L, 0L, 0L) < 0) return -errno;
    const uint64_t child_signals = UINT64_C(1) << (SIGCHLD - 1);
    const long blocked = native(SYS_rt_sigprocmask, SIG_BLOCK, (long)&child_signals, 0,
                                sizeof(child_signals), 0);
    if (blocked < 0) return (int)blocked;
    return (int)native(SYS_signalfd4, -1, (long)&child_signals, sizeof(child_signals),
                       O_CLOEXEC | O_NONBLOCK, 0);
}

static _Noreturn void run_payload(struct ptg220_helper_startup *start, struct scope *scope,
                                  int gate, pid_t r, int r_pidfd) {
    const pid_t p = (pid_t)native(SYS_getpid, 0, 0, 0, 0, 0);
    int result = ptg220_restore_payload_ids(&start->original_ids);
    if (result == 0) result = ptg220_verify_inherited_security(&start->request);
    if (result == 0) result = notify(gate, start, PTG220_P_READY, p, 0, -1);
    if (result == 0) result = expect(start, gate, r_pidfd, r, PTG220_P_RELEASE, p);
    if (result == 0) {
        const int authority = next_event(start, -1, r_pidfd, 0);
        if (authority != 0 || !ptg220_lease_p_can_exec(&scope->lease)) result = -ECANCELED;
    }
    if (result == 0) {
        result = ptg220_descriptor_restore(&start->descriptors);
    }
    if (result == 0) {
        const long mask = native(SYS_rt_sigprocmask, SIG_SETMASK, (long)&start->request.signal_mask,
                                 0, sizeof(start->request.signal_mask), 0);
        if (mask < 0) result = (int)mask;
    }
    if (result == 0) {
        uint64_t now;
        result = ptg220_boottime(&now);
        if (result == 0 && (now >= start->deadline_ns ||
            !ptg220_lease_p_can_exec(&scope->lease))) result = -ECANCELED;
    }
    if (result == 0) {
        atomic_store_explicit(&scope->exec_stage, 1, memory_order_release);
        const long native_result = native(SYS_execve, (long)start->request.path,
                                          (long)start->request.argv, (long)start->request.envp,
                                          0, 0);
        result = native_result < 0 ? (int)native_result : -EPROTO;
    }
    const int failure = -result;
    scope->exec_errno = failure;
    atomic_store_explicit(&scope->exec_stage, 2, memory_order_release);
    leave(125);
}

static enum ptg220_outcome event_cause(int event) {
    if (event == 2) return PTG220_EXITED;
    if (event == -ETIMEDOUT) return PTG220_TIMED_OUT;
    if (event == -ECANCELED) return PTG220_OWNER_ENDED;
    return PTG220_UNCONFIRMED;
}

static _Noreturn void run_reaper(struct ptg220_helper_startup *start, struct scope *scope,
                                 int peer, pid_t w, int w_pidfd) {
    if (ptg220_arm_parent_death(w, w_pidfd, start->deadline_ns) < 0) leave(125);
    const int events = arm_reaper();
    const int self = ptg220_open_self_pidfd();
    const pid_t r = getpid();
    if (events < 0 || self < 0 || next_event(start, -1, w_pidfd, 0) != 0 ||
        ptg220_disarm_parent_death(w, w_pidfd, start->deadline_ns) < 0) leave(125);

    int status = notify(peer, start, PTG220_R_READY, r, 0, -1);
    if (status == 0) status = expect(start, peer, w_pidfd, w, PTG220_G_OFFER, r);
    if (status == 0) status = ptg220_lease_accept(&scope->lease, start->enrollment.parent,
                                                start->enrollment.pid_namespace, w, r);
    const int accepted_reservation = status == 0;
    if (status == 0) status = notify(peer, start, PTG220_G_ACCEPTED, r, 0, -1);
    if (status == 0) status = expect(start, peer, w_pidfd, w, PTG220_G_CREATED, r);
    struct ptg220_leaf leaf = {.directory = -1, .kill = -1, .events = -1};
    if (status == 0) status = ptg220_lease_join(&scope->lease, start->enrollment.parent, &leaf);
    if (status < 0) {
        ptg220_lease_seal(&scope->lease);
        if (accepted_reservation) {
            uint64_t now;
            const int clock = ptg220_boottime(&now);
            (void)ptg220_signal_original(w_pidfd);
            if (clock == 0) {
                if (now > start->deadline_ns) now = start->deadline_ns;
                if (now <= UINT64_MAX - PTG220_SETTLEMENT_NS &&
                    wait_creator_terminal(w_pidfd, now + PTG220_SETTLEMENT_NS) == 0) {
                    (void)ptg220_lease_recover_sealed(&scope->lease, start->enrollment.parent, w_pidfd);
                }
            }
        }
        leave(125);
    }

    int gate[2] = {-1, -1};
    struct ptg220_birth p = {.pidfd = -1};
    status = ptg220_bootstrap_pair(gate);
    if (status == 0 && next_event(start, -1, w_pidfd, 0) != 0) status = -ECANCELED;
    if (status == 0) status = ptg220_lease_before_birth(&scope->lease);
    if (status == 0) {
        status = ptg220_clone_payload(leaf.directory, &p);
        if (status == 0) {
            close_fd(gate[0]);
            close_fd(peer);
            run_payload(start, scope, gate[1], r, self);
        }
        if (status > 0) status = 0;
    }
    close_fd(gate[1]);
    if (status == 0) status = expect(start, gate[0], p.pidfd, p.pid, PTG220_P_READY, p.pid);
    if (status == 0) status = notify(peer, start, PTG220_P_BIRTH, p.pid, 0, p.pidfd);
    if (status == 0) status = expect(start, peer, w_pidfd, w, PTG220_P_HELD, p.pid);
    if (status == 0) status = notify(gate[0], start, PTG220_P_RELEASE, p.pid, 0, -1);
    if (status == 0) status = next_event(start, w_pidfd, p.pidfd, 1);

    uint64_t selected = 0;
    const int selection_clock = ptg220_boottime(&selected);
    if (selection_clock < 0) {
        ptg220_lease_seal(&scope->lease);
        struct ptg220_native_action *const stop = &scope->evidence.controller_stop[1].kill;
        (void)ptg220_leaf_kill_observed(&leaf, 0, stop);
        stop->before_error = selection_clock;
        leave(125);
    }
    const enum ptg220_outcome local_cause = event_cause(status);
    uint64_t cutoff = 0;
    const int first_kill = stop_scope(scope, &leaf, 1, local_cause, selected, &cutoff);
    close_fd(gate[0]);
    struct ptg220_result result = {0};
    const int original = p.pidfd >= 0 ? ptg220_join_original(&p, cutoff, &scope->evidence.payload) : -ECHILD;
    result.worker_wait = scope->evidence.payload.frame;
    const int rest = all_owned_reaped(cutoff, events, &scope->evidence.reaper_empty,
                                      &scope->evidence.joined_adoptees);
    const int removed = rest == 0 && (original == 0 || p.pidfd < 0)
        ? ptg220_lease_remove_observed(&scope->lease, start->enrollment.parent, &leaf,
                                       &scope->evidence.retire) : -EBUSY;
    if (first_kill < 0 || original < 0 || rest < 0 || removed < 0) leave(125);
    uint64_t done;
    if (ptg220_boottime(&done) < 0 || done >= cutoff) leave(125);
    const unsigned int winner = atomic_load_explicit(&scope->stop_owner, memory_order_acquire);
    if (winner == 0 || winner > 2) leave(125);
    result.outcome = (enum ptg220_outcome)scope->evidence.controller_stop[winner - 1].cause;
    if (result.outcome == PTG220_EXITED) {
        const unsigned int exec_stage = atomic_load_explicit(&scope->exec_stage, memory_order_acquire);
        if (exec_stage == 2 && scope->exec_errno > 0) {
            result.outcome = PTG220_EXEC_REFUSED;
            result.value = scope->exec_errno;
        } else if (exec_stage == 1) {
            result.value = result.worker_wait.status;
            result.outcome = result.worker_wait.code == CLD_EXITED ? PTG220_EXITED : PTG220_SIGNALED;
        } else {
            leave(125);
        }
    }
    if (result.outcome == PTG220_UNCONFIRMED) leave(125);
    scope->payload_result = result;
    atomic_store_explicit(&scope->finished, 1, memory_order_release);
    /* Only after native G retirement can A fence a stalled W without
       discarding the last live authority over a running payload. */
    const struct ptg220_bootstrap_packet retired = {
        .version = 1, .step = PTG220_R_SETTLED, .pid = (uint32_t)r,
        .deadline_ns = start->deadline_ns, .cutoff_ns = cutoff,
    };
    (void)ptg220_bootstrap_send(start->channel, &retired, -1);
    leave(0);
}

static int hold_payload(struct ptg220_helper_startup *start, int peer,
                         const struct ptg220_birth *r, struct ptg220_birth *p) {
    struct ptg220_bootstrap_message birth;
    int result = read_step(start, peer, r->pidfd, r->pid, PTG220_P_BIRTH, &birth);
    if (result < 0) return result;
    if (birth.packet.pid == 0 || birth.packet.pid > INT32_MAX || birth.packet.payload != 0 ||
        birth.birth_pidfd < 0 || ptg220_pidfd_terminal(birth.birth_pidfd) != 0) {
        result = -EPROTO;
    } else {
        *p = (struct ptg220_birth){.pid = (pid_t)birth.packet.pid, .pidfd = birth.birth_pidfd};
        birth.birth_pidfd = -1;
    }
    ptg220_bootstrap_close(&birth);
    return result == 0 ? notify(peer, start, PTG220_P_HELD, p->pid, 0, -1) : result;
}

static void fence_reaper(struct scope *scope, const struct ptg220_birth *r,
                          uint64_t selected, uint64_t cutoff) {
    const int signaled = ptg220_signal_original(r->pidfd);
    struct ptg220_native_action *const action = &scope->evidence.fence_reaper;
    action->called = 1;
    action->native_return = signaled < 0 ? -1 : signaled;
    action->native_errno = signaled < 0 ? -signaled : 0;
    action->before_ns = selected;
    action->after_error = ptg220_boottime(&action->after_ns);

    if (action->after_error == 0 && action->after_ns < cutoff) {
        (void)ptg220_join_original(r, cutoff, &scope->evidence.reaper);
    }
    if (!ptg220_original_terminal(r, &scope->evidence.reaper)) {
        /* One nonblocking terminal observation remains diagnostic if late.
           A signal success, pidfd readability or ECHILD is not this barrier. */
        (void)ptg220_record_original_reap(r, &scope->evidence.reaper);
    }
}

static int observe_reaper(const struct ptg220_birth *r, uint64_t cutoff,
                          struct ptg220_wait_record *peek) {
    const int timer = ptg220_deadline_fd(cutoff);
    if (timer < 0) return timer;
    struct pollfd fds[] = {{.fd = timer, .events = POLLIN}, {.fd = r->pidfd, .events = POLLIN}};
    int status;
    do status = ptg220_poll(fds, 2, 1); while (status == -EINTR);
    close_fd(timer);
    if (status < 0) return status;
    if (!(fds[1].revents & POLLIN) || (fds[1].revents & ~(POLLIN | POLLHUP))) {
        return fds[0].revents == POLLIN ? -ETIMEDOUT : -EPROTO;
    }
    status = ptg220_record_original_peek(r, peek);
    if (status != 1) return status < 0 ? status : -EPROTO;
    if (peek->before_error != 0 || peek->after_error != 0 ||
        peek->after_ns < peek->before_ns || peek->after_ns >= cutoff) return -ETIME;
    return 0;
}

static int receive_retirement_ack(const struct ptg220_helper_startup *start,
                                 const struct ptg220_birth *r, uint64_t cutoff,
                                 struct ptg220_evidence *evidence) {
    struct ptg220_settlement_failure *const failure = &evidence->watchdog_failure;
    const int timer = ptg220_deadline_fd(cutoff);
    if (timer < 0) {
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_W_ACK_TIMER,
                                            timer, cutoff, 0, NULL);
    }
    struct pollfd fds[] = {
        {.fd = start->owners[0], .events = POLLIN},
        {.fd = timer, .events = POLLIN},
        {.fd = start->channel, .events = POLLIN},
    };
    int status;
    do status = ptg220_poll(fds, 3, 1); while (status == -EINTR);
    close_fd(timer);
    int detail = 0;
    if (status >= 0) {
        if (fds[0].revents) {
            detail = fds[0].revents;
            status = (detail & POLLIN) && !(detail & ~(POLLIN | POLLHUP)) ? -ECANCELED : -EPROTO;
        } else if (fds[1].revents) {
            detail = fds[1].revents;
            status = detail == POLLIN ? -ETIMEDOUT : -EPROTO;
        } else if (fds[2].revents != POLLIN) {
            detail = fds[2].revents;
            status = detail & POLLHUP ? -EPIPE : -EPROTO;
        }
    }
    if (status < 0) {
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_W_ACK_POLL,
                                             status, cutoff, detail, NULL);
    }
    struct ptg220_bootstrap_message ack;
    struct ptg220_receive_observation seen;
    status = ptg220_bootstrap_receive_observed(start->channel, &ack, &seen);
    if (status < 0) {
        return ptg220_note_settlement_failure(failure, PTG220_FAILURE_W_ACK_RECEIVE,
                                             status, cutoff, 0, &seen);
    }
    if (ack.sender != start->caller || ack.real_uid != start->original_ids.uid ||
        ack.real_gid != start->original_ids.gid ||
        ptg220_settlement_packet(&ack, PTG220_ACK_R_SETTLED, r->pid, start->deadline_ns, cutoff) != 0) {
        status = -EPROTO;
    } else {
        status = ptg220_bind_pidfds(start->owners[0], ack.sender_pidfd, (uint32_t)start->caller);
        if (status == 0) {
            status = ptg220_pidfd_terminal(start->owners[0]);
            if (status > 0) status = -ECANCELED;
        }
    }
    ptg220_bootstrap_close(&ack);
    if (status == 0) {
        uint64_t observed = 0;
        status = ptg220_boottime(&observed);
        if (status == 0 && (observed == 0 || observed < evidence->reaper_peek.after_ns)) status = -EPROTO;
        if (status == 0 && observed >= cutoff) status = -ETIMEDOUT;
        if (status == 0) evidence->watchdog_ack_ns = observed;
    }
    return ptg220_note_settlement_failure(failure, PTG220_FAILURE_W_ACK_BIND, status, cutoff, 0, &seen);
}

static int finish_scope(struct ptg220_helper_startup *start, struct scope *scope,
                        struct ptg220_birth *r, struct ptg220_birth *p,
                        struct ptg220_leaf *leaf, int child_events, int event) {
    uint64_t selected = 0;
    const int selection_clock = ptg220_boottime(&selected);
    if (selection_clock < 0) {
        ptg220_lease_seal(&scope->lease);
        if (leaf->kill >= 0) {
            struct ptg220_native_action *const stop = &scope->evidence.controller_stop[0].kill;
            (void)ptg220_leaf_kill_observed(leaf, 0, stop);
            stop->before_error = selection_clock;
        }
        return selection_clock;
    }
    const enum ptg220_outcome local = event_cause(event == 2 ? -ECHILD : event);
    uint64_t cutoff = 0;
    int first_kill = 0;
    const unsigned int first = atomic_load_explicit(&scope->stop_owner, memory_order_acquire);
    if (first != 0 && first <= 2 &&
        atomic_load_explicit(&scope->finished, memory_order_acquire) == 1) {
        uint64_t from = scope->evidence.controller_stop[first - 1].observed_ns;
        if (from > start->deadline_ns) from = start->deadline_ns;
        cutoff = from + PTG220_SETTLEMENT_NS;
    } else {
        first_kill = stop_scope(scope, leaf, 0, local, selected, &cutoff);
    }
    scope->evidence.cutoff = cutoff;
    const struct ptg220_native_action *request = &scope->evidence.controller_stop[0].kill;
    const int first_stop = request->called && request->native_return == 1 && request->native_errno == 0;
    const int unopened = ptg220_lease_sealed_without_birth(&scope->lease);
    /* Where R acquired the one-shot birth slot but never transferred P, W
       has not permitted ordinary exec. Don't exhaust the budget waiting for
       that producer before fencing it and doing the post-adoption stop. */
    const int pending_birth = p->pidfd < 0 && !unopened && first_stop &&
        atomic_load_explicit(&scope->finished, memory_order_acquire) != 1;
    int joined;
    if (pending_birth) {
        /* This existing fence irrevocably abandons a positive result. */
        fence_reaper(scope, r, selected, cutoff);
        joined = -EUCLEAN;
    } else {
        struct ptg220_wait_record *const peek = &scope->evidence.reaper_peek;
        struct ptg220_settlement_failure *const failure = &scope->evidence.watchdog_failure;
        joined = observe_reaper(r, cutoff, peek);
        if (joined < 0) {
            (void)ptg220_note_settlement_failure(failure, PTG220_FAILURE_W_R_PEEK,
                                                joined, cutoff, 0, NULL);
        }
        if (ptg220_original_terminal(r, peek)) {
            /* R may publish while W sleeps. Select only after a native peek.
               Until the ACK, leave the original zombie attached for recvmsg. */
            if (joined == 0 && first_kill == 0 && peek->frame.code == CLD_EXITED &&
                peek->frame.status == 0 &&
                atomic_load_explicit(&scope->finished, memory_order_acquire) == 1) {
                joined = receive_retirement_ack(start, r, cutoff, &scope->evidence);
            } else if (joined == 0) {
                joined = first_kill < 0 ? first_kill : -EUCLEAN;
            }
            /* A nonzero joined irrevocably forbids W_RESULT. The original
               child is terminal, so an ACK failure cannot leave live work. */
            struct ptg220_wait_record *const reaped = &scope->evidence.reaper;
            const int consumed = ptg220_record_original_reap(r, reaped);
            if (joined == 0) {
                if (consumed != 1 || !ptg220_original_terminal(r, reaped) ||
                    memcmp(&peek->frame, &reaped->frame, sizeof(peek->frame)) != 0 ||
                    memcmp(peek->siginfo, reaped->siginfo, sizeof(peek->siginfo)) != 0) {
                    joined = consumed < 0 ? consumed : -EPROTO;
                } else if (reaped->before_error != 0 || reaped->after_error != 0 ||
                    reaped->before_ns < scope->evidence.watchdog_ack_ns ||
                    reaped->after_ns < reaped->before_ns || reaped->after_ns >= cutoff) {
                    joined = -ETIME;
                }
                (void)ptg220_note_settlement_failure(failure, PTG220_FAILURE_W_R_CONSUME,
                                                     joined, cutoff, consumed, NULL);
            }
        }
    }
    if (!pending_birth && joined < 0 && !ptg220_original_terminal(r, &scope->evidence.reaper)) {
        const int retired = atomic_load_explicit(&scope->finished, memory_order_acquire) == 1;
        if (first_stop || unopened || retired) {
            fence_reaper(scope, r, selected, cutoff);
        }
    }
    if (!ptg220_original_terminal(r, &scope->evidence.reaper)) return joined < 0 ? joined : -EUCLEAN;
    const struct ptg_wait_frame *const original_r = &scope->evidence.reaper.frame;
    if (joined == 0 && scope->evidence.watchdog_ack_ns != 0 &&
        original_r->code == CLD_EXITED && original_r->status == 0 &&
        atomic_load_explicit(&scope->finished, memory_order_acquire) == 1) {
        const int adopted = all_owned_reaped(cutoff, child_events, &scope->evidence.watchdog_empty,
                                             &scope->evidence.joined_adoptees);
        if (first_kill < 0 || adopted < 0) return first_kill < 0 ? first_kill : adopted;
        scope->evidence.stop_owner = atomic_load_explicit(&scope->stop_owner, memory_order_acquire);
        scope->evidence.cutoff = cutoff;
        const int sealed = ptg220_evidence_seal(&scope->evidence);
        if (sealed < 0) return sealed;
        const uint64_t result = ((uint64_t)scope->payload_result.outcome << 32) |
            (uint32_t)scope->payload_result.value;
        const struct ptg220_bootstrap_packet reply = {
            .version = 1, .step = PTG220_W_RESULT, .pid = (uint32_t)getpid(),
            .deadline_ns = start->deadline_ns, .payload = result, .cutoff_ns = cutoff,
            .native_wait = scope->payload_result.worker_wait,
        };
        const int delivered = ptg220_bootstrap_send(start->channel, &reply, sealed);
        close_fd(sealed);
        return ptg220_note_settlement_failure(&scope->evidence.watchdog_failure,
            PTG220_FAILURE_W_RESULT_SEND, delivered, cutoff, 0, NULL);
    }

    /* R's original native wait follows kernel adoption and fences any last
       clone that could have entered G after W's earlier stop. A prior
       positive retirement remains evidence, never a late success. */
    if (atomic_load_explicit(&scope->finished, memory_order_acquire) == 1) {
        return joined < 0 ? joined : -EUCLEAN;
    }
    int second_stop = 0;
    if (leaf->kill >= 0) {
        second_stop = ptg220_leaf_kill_observed(leaf, scope->evidence.reaper.after_ns,
                                                &scope->evidence.stop_after_reaper);
        scope->evidence.stop_after_reaper.before_error = scope->evidence.reaper.after_error;
    } else if (!ptg220_lease_sealed_without_birth(&scope->lease)) {
        return -EUCLEAN;
    }
    if (p->pidfd >= 0) {
        if (!ptg220_original_terminal(p, &scope->evidence.payload)) {
            (void)ptg220_join_original(p, cutoff, &scope->evidence.payload);
        }
    }
    const int rest = all_owned_reaped(cutoff, child_events, &scope->evidence.watchdog_empty,
                                      &scope->evidence.joined_adoptees);
    if (second_stop == 0 && rest == 0) {
        if (leaf->directory >= 0) {
            (void)ptg220_lease_remove_observed(&scope->lease, start->enrollment.parent, leaf,
                                               &scope->evidence.retire);
        } else {
            (void)ptg220_lease_recover_own_create(&scope->lease, start->enrollment.parent);
        }
    }
    return -EUCLEAN;
}

static void report_failure(const struct ptg220_helper_startup *start, const struct scope *scope,
                           const struct ptg220_birth *r, int error) {
    struct ptg220_evidence snapshot = {
        .version = PTG220_EVIDENCE_VERSION, .deadline = start->deadline_ns, .cutoff = scope->evidence.cutoff,
        .leaf_inode = scope->evidence.leaf_inode, .leaf_mount = scope->evidence.leaf_mount,
        .reaper = scope->evidence.reaper, .fence_reaper = scope->evidence.fence_reaper,
        .stop_after_reaper = scope->evidence.stop_after_reaper,
        .watchdog_failure = scope->evidence.watchdog_failure, .reaper_peek = scope->evidence.reaper_peek,
        .watchdog_ack_ns = scope->evidence.watchdog_ack_ns,
    };
    if (r->pidfd < 0 || ptg220_original_terminal(r, &scope->evidence.reaper)) {
        snapshot = scope->evidence;
    } else {
        /* R is unjoined and may still own its half of the shared record. */
        snapshot.controller_stop[0] = scope->evidence.controller_stop[0];
    }
    snapshot.watchdog_error = -error;
    if (snapshot.cutoff == 0) snapshot.cutoff = start->deadline_ns + PTG220_SETTLEMENT_NS;
    const int sealed = ptg220_evidence_seal(&snapshot);
    if (sealed < 0) return;
    const struct ptg220_bootstrap_packet failed = {
        .version = 1, .step = PTG220_W_FAILURE, .pid = (uint32_t)getpid(),
        .deadline_ns = start->deadline_ns, .payload = (uint32_t)-error,
        .cutoff_ns = snapshot.cutoff,
    };
    (void)ptg220_bootstrap_send(start->channel, &failed, sealed);
    close_fd(sealed);
}

int ptg220_supervise(struct ptg220_helper_startup *start) {
    const int events = arm_reaper();
    if (events < 0) return events;
    struct scope *scope = mmap(NULL, sizeof(*scope), PROT_READ | PROT_WRITE,
                               MAP_ANONYMOUS | MAP_SHARED, -1, 0);
    if (scope == MAP_FAILED) {
        const int failure = errno;
        close_fd(events);
        return -failure;
    }
    ptg220_lease_init(&scope->lease);
    atomic_init(&scope->stop_owner, 0);
    atomic_init(&scope->finished, 0);
    atomic_init(&scope->exec_stage, 0);
    scope->evidence.version = PTG220_EVIDENCE_VERSION;
    scope->evidence.deadline = start->deadline_ns;
    const pid_t w = getpid();
    const int w_pidfd = ptg220_open_self_pidfd();
    int peer[2] = {-1, -1};
    int status = w_pidfd < 0 ? w_pidfd : ptg220_bootstrap_pair(peer);
    if (status == 0 && next_event(start, -1, -1, 0) != 0) status = -ECANCELED;
    struct ptg220_birth r = {.pidfd = -1}, p = {.pidfd = -1};
    if (status == 0) {
        status = ptg220_clone_controller(&r);
        if (status == 0) {
            close_fd(peer[0]);
            close_fd(events);
            run_reaper(start, scope, peer[1], w, w_pidfd);
        }
        if (status > 0) status = 0;
    }
    close_fd(peer[1]);
    if (status == 0) status = notify(start->channel, start, PTG220_BIRTH_R, r.pid, 0, r.pidfd);
    if (status == 0) status = expect(start, start->channel, -1, start->caller, PTG220_ACK_R, r.pid);
    if (status == 0) status = expect(start, peer[0], r.pidfd, r.pid, PTG220_R_READY, r.pid);
    if (status == 0) status = ptg220_lease_prepare(&scope->lease, start->enrollment.parent,
                                                 start->enrollment.pid_namespace, w, r.pid);
    if (status == 0) status = notify(peer[0], start, PTG220_G_OFFER, r.pid, 0, -1);
    if (status == 0) status = expect(start, peer[0], r.pidfd, r.pid, PTG220_G_ACCEPTED, r.pid);
    struct ptg220_leaf leaf = {.directory = -1, .kill = -1, .events = -1};
    if (status == 0 && next_event(start, -1, r.pidfd, 0) != 0) status = -ECANCELED;
    if (status == 0) status = ptg220_lease_create(&scope->lease, start->enrollment.parent, &leaf);
    if (status == 0) {
        scope->evidence.leaf_inode = leaf.identity.inode;
        scope->evidence.leaf_mount = leaf.identity.mount;
    }
    if (status == 0) status = notify(peer[0], start, PTG220_G_CREATED, r.pid, 0, -1);
    if (status == 0) status = hold_payload(start, peer[0], &r, &p);
    if (status == 0) status = next_event(start, -1, r.pidfd, 1);
    close_fd(peer[0]);
    if (r.pidfd >= 0) status = finish_scope(start, scope, &r, &p, &leaf, events, status);
    if (status < 0) {
        const uint64_t cutoff = scope->evidence.cutoff != 0
            ? scope->evidence.cutoff : start->deadline_ns + PTG220_SETTLEMENT_NS;
        (void)ptg220_note_settlement_failure(&scope->evidence.watchdog_failure,
            PTG220_FAILURE_W_SETTLEMENT, status, cutoff, 0, NULL);
        report_failure(start, scope, &r, status);
    }
    ptg220_leaf_close(&leaf);
    close_fd(p.pidfd);
    close_fd(r.pidfd);
    close_fd(w_pidfd);
    close_fd(events);
    (void)munmap(scope, sizeof(*scope));
    return status;
}
