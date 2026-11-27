// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "helper_startup.h"
#include "bootstrap_wire.h"
#include "native_control.h"

#include <errno.h>
#include <signal.h>
#include <sys/prctl.h>
#include <unistd.h>

void ptg220_helper_drop_startup(struct ptg220_helper_startup *state) {
    ptg220_descriptor_drop(&state->descriptors);
    ptg220_request_close(&state->request);
    for (unsigned int i = 0; i < state->owner_count; ++i) {
        if (state->owners[i] >= 0) (void)close(state->owners[i]);
    }
    if (state->deadline >= 0) (void)close(state->deadline);
    if (state->enrollment.parent >= 0) (void)close(state->enrollment.parent);
    if (state->enrollment.pid_namespace >= 0) (void)close(state->enrollment.pid_namespace);
    state->owner_count = 0;
    state->deadline = -1;
    state->enrollment = (struct ptg220_enrollment){.parent = -1, .pid_namespace = -1};
}

static int caller_step(struct ptg220_helper_startup *state, uid_t uid,
                       enum ptg220_bootstrap_step step, struct ptg220_bootstrap_message *message) {
    int received;
    for (;;) {
        received = ptg220_bootstrap_receive(state->channel, message);
        if ((received != -EAGAIN && received != -EINTR) || state->deadline < 0) break;
        struct pollfd fds[] = {
            {.fd = state->channel, .events = POLLIN}, {.fd = state->deadline, .events = POLLIN},
            {.fd = state->owners[0], .events = POLLIN},
        };
        const int ready = ptg220_poll(fds, 3, 1);
        if (ready == -EINTR) continue;
        if (ready < 0) return ready;
        if (fds[1].revents) return -ETIMEDOUT;
        if (fds[2].revents) return -ECANCELED;
        if (!(fds[0].revents & POLLIN)) return -EPIPE;
    }
    if (received < 0) return received;
    if (message->sender != state->caller || message->real_uid != uid ||
        message->packet.step != (uint32_t)step || message->packet.pid != (uint32_t)getpid() ||
        (step != PTG220_START_W && message->packet.descriptor_count != 0) ||
        (state->deadline_ns != 0 && message->packet.deadline_ns != state->deadline_ns) ||
        ptg220_pidfd_terminal(state->owners[0]) != 0 ||
        ptg220_pidfd_terminal(message->sender_pidfd) != 0) {
        ptg220_bootstrap_close(message);
        return -EPROTO;
    }
    return 0;
}

static int adopt_ready(const struct ptg220_helper_startup *state) {
    struct pollfd watched[PTG_OWNER_WATCH_MAX + 2];
    watched[0] = (struct pollfd){.fd = state->channel, .events = POLLIN};
    watched[1] = (struct pollfd){.fd = state->deadline, .events = POLLIN};
    for (unsigned int i = 0; i < state->owner_count; ++i) {
        watched[i + 2] = (struct pollfd){.fd = state->owners[i], .events = POLLIN};
    }
    int result;
    do result = ptg220_poll(watched, state->owner_count + 2, 1); while (result == -EINTR);
    if (result < 0) return result;
    uint64_t now;
    result = ptg220_boottime(&now);
    if (result < 0) return result;
    if (watched[1].revents || now >= state->deadline_ns) return -ETIMEDOUT;
    for (unsigned int i = 0; i < state->owner_count; ++i) {
        if (watched[i + 2].revents) return -ECANCELED;
    }
    return watched[0].revents & POLLIN ? 0 : -EPIPE;
}

int ptg220_helper_take_adopted(int channel, struct ptg220_helper_startup *state) {
    *state = (struct ptg220_helper_startup){
        .channel = channel, .deadline = -1, .enrollment = {.parent = -1, .pid_namespace = -1},
    };
    uid_t uid;
    const int caller = ptg220_bootstrap_creator(channel, &state->caller, &uid);
    if (caller < 0) return caller;
    state->owners[0] = caller;
    state->owner_count = 1;
    struct ptg220_bootstrap_message start;
    int result = caller_step(state, uid, PTG220_START_W, &start);
    if (result < 0) goto done;
    const uint32_t offered_count = (uint32_t)start.packet.payload & UINT32_C(0xffff);
    const uint32_t owner_mode = ((uint32_t)start.packet.payload >> 16) & UINT32_C(0xffff);
    const uint32_t expected_b = (uint32_t)(start.packet.payload >> 32);
    const uint64_t descriptor_count = start.packet.descriptor_count;
    struct ptg220_owner_identity originals[PTG_OWNER_WATCH_MAX];
    originals[0] = start.packet.owner;
    state->deadline_ns = start.packet.deadline_ns;
    if (offered_count < PTG_OWNER_WATCH_MIN || offered_count > PTG_OWNER_WATCH_MAX || owner_mode > 1 ||
        originals[0].pid != (uint32_t)state->caller ||
        expected_b != (uint32_t)getppid() || start.birth_pidfd >= 0 ||
        uid != getuid() || uid == 0 || expected_b == (uint32_t)state->caller) result = -EPROTO;
    ptg220_bootstrap_close(&start);
    if (result < 0) goto done;
    result = ptg220_validate_invocation_deadline(state->deadline_ns);
    if (result < 0) goto done;
    state->deadline = ptg220_deadline_fd(state->deadline_ns);
    if (state->deadline < 0) {
        result = state->deadline;
        goto done;
    }
    struct rlimit descriptor_limit;
    if (getrlimit(RLIMIT_NOFILE, &descriptor_limit) < 0) {
        result = -errno;
        goto done;
    }
    if (descriptor_count > descriptor_limit.rlim_cur) {
        result = -EOVERFLOW;
        goto done;
    }

    for (unsigned int i = 1; i < offered_count; ++i) {
        struct ptg220_bootstrap_message authority;
        result = caller_step(state, uid, PTG220_W_OWNER, &authority);
        if (result < 0) goto done;
        if (authority.packet.payload != i || authority.birth_pidfd < 0 ||
            ptg220_pidfd_terminal(authority.birth_pidfd) != 0) result = -EPROTO;
        if (result == 0) {
            state->owners[state->owner_count++] = authority.birth_pidfd;
            originals[i] = authority.packet.owner;
            authority.birth_pidfd = -1;
        }
        ptg220_bootstrap_close(&authority);
        if (result < 0) goto done;
    }
    result = ptg220_verify_owners(state->owners, originals, state->owner_count, owner_mode);
    if (result < 0) goto done;

    struct ptg220_bootstrap_message immutable;
    result = caller_step(state, uid, PTG220_W_REQUEST, &immutable);
    if (result < 0) goto done;
    result = immutable.birth_pidfd >= 0 && immutable.packet.payload == 0
        ? ptg220_request_read(immutable.birth_pidfd, &state->request) : -EPROTO;
    ptg220_bootstrap_close(&immutable);
    if (result < 0) goto done;
    unsigned int open_stdio = 0;
    for (uint64_t i = 0; i < descriptor_count; ++i) {
        struct ptg220_bootstrap_message descriptor;
        result = caller_step(state, uid, PTG220_W_DESCRIPTOR, &descriptor);
        if (result < 0) goto done;
        if (descriptor.packet.payload > INT32_MAX || descriptor.birth_pidfd < 0) result = -EPROTO;
        else result = ptg220_descriptor_take(&state->descriptors, (int)descriptor.packet.payload,
                                               descriptor.birth_pidfd);
        if (result == 0) {
            descriptor.birth_pidfd = -1;
            if (descriptor.packet.payload <= 2) open_stdio |= 1U << descriptor.packet.payload;
        }
        ptg220_bootstrap_close(&descriptor);
        if (result < 0) goto done;
    }
    if ((7U ^ open_stdio) != state->request.closed_stdio) {
        result = -ESTALE;
        goto done;
    }

    uint64_t now;
    result = ptg220_boottime(&now);
    if (result < 0) goto done;
    if (now >= state->deadline_ns) {
        result = -ETIMEDOUT;
        goto done;
    }
    int parent_signal = -1;
    if (prctl(PR_GET_PDEATHSIG, &parent_signal, 0L, 0L, 0L) < 0) {
        result = -errno;
        goto done;
    }
    if (parent_signal != 0) {
        result = -EPERM;
        goto done;
    }
    result = ptg220_read_enrollment(state->deadline_ns, &state->enrollment);
    if (result == 0) result = ptg220_verify_inherited_security(&state->request);
    if (result == 0) result = ptg220_restrict_privileged_helper(&state->original_ids);
    if (result == 0) result = ptg220_restore_soft_stack(&state->request.stack);
    if (result < 0) goto done;
    struct ptg220_bootstrap_packet ready = {
        .version = 1, .step = PTG220_W_READY, .pid = (uint32_t)getpid(),
        .deadline_ns = state->deadline_ns,
    };
    result = ptg220_bootstrap_send(channel, &ready, -1);
    if (result == 0) result = adopt_ready(state);
    if (result == 0) {
        struct ptg220_bootstrap_message adopted;
        result = caller_step(state, uid, PTG220_ADOPT_W, &adopted);
        if (result == 0) {
            if (adopted.birth_pidfd >= 0 || adopted.packet.payload != 0 ||
                getppid() != state->caller) result = -EPROTO;
            ptg220_bootstrap_close(&adopted);
        }
    }

done:
    if (result < 0) ptg220_helper_drop_startup(state);
    return result;
}
