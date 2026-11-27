// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "bootstrap.h"
#include "bootstrap_raw.h"
#include "bootstrap_steps.h"
#include "enrollment.h"
#include "native_syscall.h"

#include <errno.h>
#include <signal.h>
#include <stddef.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

static void close_fd(int fd) {
    if (fd >= 0) (void)native(SYS_close, fd, 0, 0, 0, 0);
}

static int send_w(struct ptg220_bootstrap *state, const struct ptg220_bootstrap_packet *packet,
                  int attachment) {
    for (;;) {
        const int sent = ptg220_bootstrap_send(state->w_control, packet, attachment);
        if (sent != -EAGAIN && sent != -EINTR) return sent;
        struct pollfd fds[PTG_OWNER_WATCH_MAX + 3] = {
            {.fd = state->w_control, .events = POLLOUT},
            {.fd = state->w.pidfd, .events = POLLIN}, {.fd = state->deadline, .events = POLLIN},
        };
        for (unsigned int i = 0; i < state->owner_count; ++i) {
            fds[i + 3] = (struct pollfd){.fd = state->owners[i], .events = POLLIN};
        }
        const int ready = ptg220_poll(fds, state->owner_count + 3, 1);
        if (ready == -EINTR) continue;
        if (ready < 0) return ready;
        if (fds[1].revents) return -ECHILD;
        if (fds[2].revents) return -ETIMEDOUT;
        for (unsigned int i = 0; i < state->owner_count; ++i) {
            if (fds[i + 3].revents) return -ECANCELED;
        }
        if (fds[0].revents != POLLOUT) return -EPIPE;
    }
}

/* The newborns cannot depend on the application's stack or TLS mappings. The
   private allocation has a read-only context and separate guarded stacks. */
static int prepare_child(const struct ptg220_bootstrap_child *input, void **allocation,
                         size_t *length, const struct ptg220_bootstrap_child **child) {
    const size_t stack_size = 64 * 1024;
    const long native_page = sysconf(_SC_PAGESIZE);
    if (native_page <= 0 || native_page > (long)stack_size) return -ENOTSUP;
    const size_t page = (size_t)native_page;
    if (sizeof(*input) > page || stack_size % page != 0) return -ENOTSUP;
    const size_t size = 3 * page + 2 * stack_size;
    unsigned char *base = mmap(NULL, size, PROT_NONE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_STACK,
                               -1, 0);
    if (base == MAP_FAILED) return -errno;
    *allocation = base;
    *length = size;
    struct ptg220_bootstrap_child *context = (void *)base;
    unsigned char *b_stack = base + 2 * page;
    unsigned char *w_stack = base + 3 * page + stack_size;
    if (mprotect(base, page, PROT_READ | PROT_WRITE) < 0 ||
        mprotect(b_stack, stack_size, PROT_READ | PROT_WRITE) < 0 ||
        mprotect(w_stack, stack_size, PROT_READ | PROT_WRITE) < 0) return -errno;
    *context = *input;
    context->b_stack = b_stack;
    context->w_stack = w_stack;
    context->stack_size = stack_size;
    if (mprotect(base, page, PROT_READ) < 0) return -errno;
    *child = context;
    return 0;
}

int ptg220_bootstrap_begin(uint64_t deadline_ns, const int *owner_pidfds, unsigned int owner_count,
                           const struct ptg220_owner_identity *owners, unsigned int owner_mode,
                           int immutable_request, const struct ptg220_descriptors *descriptors,
                           struct ptg220_bootstrap *state) {
    *state = (struct ptg220_bootstrap){
        .deadline_ns = deadline_ns, .owners = owner_pidfds, .owner_count = owner_count,
        .b = {.pidfd = -1}, .w = {.pidfd = -1}, .b_control = -1, .w_control = -1, .deadline = -1,
    };
    if (owner_pidfds == NULL || owners == NULL || owner_count < PTG_OWNER_WATCH_MIN ||
        owner_count > PTG_OWNER_WATCH_MAX || descriptors == NULL ||
        deadline_ns > UINT64_MAX - PTG220_SETTLEMENT_NS || immutable_request < 0) return -EINVAL;
    const int caller = ptg220_caller_preflight();
    if (caller < 0) return caller;
    const int verified = ptg220_verify_owners(owner_pidfds, owners, owner_count, owner_mode);
    if (verified < 0) return verified;
    for (unsigned int i = 0; i < owner_count; ++i) {
        const int terminal = ptg220_pidfd_terminal(owner_pidfds[i]);
        if (terminal != 0) return terminal < 0 ? terminal : -ECANCELED;
    }
    int status = ptg220_validate_invocation_deadline(deadline_ns);
    if (status < 0) return status;
    const pid_t a = (pid_t)native(SYS_getpid, 0, 0, 0, 0, 0);
    if ((uint32_t)a != owners[0].pid) return -EPROTO;
    const uid_t uid = (uid_t)native(SYS_getuid, 0, 0, 0, 0, 0);
    int a_b[2] = {-1, -1}, a_w[2] = {-1, -1};
    const int image = ptg220_open_fixed_helper();
    if (image < 0) return image;
    const int timer = ptg220_deadline_fd(deadline_ns);
    status = timer;
    if (timer >= 0) status = ptg220_bootstrap_pair(a_b);
    if (status == 0) status = ptg220_bootstrap_pair(a_w);
    if (status == 0 && a_w[1] < 3) status = -ENOTSUP;
    void *child_allocation = NULL;
    size_t child_bytes = 0;
    const struct ptg220_bootstrap_child *child = NULL;
    if (status == 0) {
        struct ptg220_bootstrap_child context = {
            .deadline_ns = deadline_ns, .owner_count = owner_count,
            .caller = a, .uid = uid, .image = image, .parent_timer = timer,
            .parent_bootstrap = a_b[0], .child_bootstrap = a_b[1],
            .parent_helper = a_w[0], .child_helper = a_w[1],
        };
        for (unsigned int i = 0; i < owner_count; ++i) context.owners[i] = owner_pidfds[i];
        status = prepare_child(&context, &child_allocation, &child_bytes, &child);
    }
    const uint64_t blocked = UINT64_MAX & ~(UINT64_C(1) << (SIGKILL - 1)) &
        ~(UINT64_C(1) << (SIGSTOP - 1));
    uint64_t original_mask = 0;
    int masked = 0;
    if (status == 0) {
        const long changed = native(SYS_rt_sigprocmask, SIG_SETMASK, (long)&blocked,
                                     (long)&original_mask, sizeof(blocked), 0);
        if (changed < 0) status = (int)changed;
        else masked = 1;
    }
    if (status == 0) status = ptg220_bootstrap_spawn(&state->b, child);
    long restoration = 0;
    if (masked) restoration = native(SYS_rt_sigprocmask, SIG_SETMASK, (long)&original_mask,
                                     0, sizeof(original_mask), 0);
    const int unmap_error = child_bytes != 0 && munmap(child_allocation, child_bytes) < 0 ? errno : 0;
    close_fd(image);
    if (status < 0) {
        close_fd(timer);
        if (a_b[0] >= 0) {
            close_fd(a_b[0]);
            close_fd(a_b[1]);
        }
        if (a_w[0] >= 0) {
            close_fd(a_w[0]);
            close_fd(a_w[1]);
        }
        return status;
    }
    close_fd(a_b[1]);
    close_fd(a_w[1]);
    state->b_control = a_b[0];
    state->w_control = a_w[0];
    state->deadline = timer;
    state->caller_uid = uid;
    if (restoration < 0) return (int)restoration;
    if (unmap_error != 0) return -unmap_error;

    struct ptg220_bootstrap_message birth;
    status = receive_step(state->b_control, timer, owner_pidfds, owner_count, state->b.pid,
                          uid, deadline_ns, PTG220_BIRTH_W, &birth);
    if (status < 0) return status;
    if (birth.packet.pid == 0 || birth.packet.pid > INT32_MAX || birth.birth_pidfd < 0 ||
        ptg220_pidfd_terminal(birth.birth_pidfd) != 0) {
        ptg220_bootstrap_close(&birth);
        return -EPROTO;
    }
    state->w = (struct ptg220_birth){.pid = (pid_t)birth.packet.pid, .pidfd = birth.birth_pidfd};
    birth.birth_pidfd = -1;
    ptg220_bootstrap_close(&birth);
    struct ptg220_bootstrap_packet start = {
        .version = 1, .step = PTG220_START_W, .pid = (uint32_t)state->w.pid,
        .deadline_ns = deadline_ns,
        .payload = ((uint64_t)(uint32_t)state->b.pid << 32) | ((uint64_t)owner_mode << 16) | owner_count,
        .owner = owners[0],
        .descriptor_count = descriptors->count,
    };
    status = send_w(state, &start, -1);
    if (status == 0) status = send_step(state->b_control, deadline_ns, PTG220_COMMIT_W_EXEC, state->w.pid, -1);
    start.descriptor_count = 0;
    for (unsigned int i = 1; status == 0 && i < owner_count; ++i) {
        start.step = PTG220_W_OWNER;
        start.payload = i;
        start.owner = owners[i];
        status = send_w(state, &start, owner_pidfds[i]);
    }
    if (status == 0) {
        start.step = PTG220_W_REQUEST;
        start.payload = 0;
        start.owner = (struct ptg220_owner_identity){0};
        status = send_w(state, &start, immutable_request);
    }
    for (size_t i = 0; status == 0 && i < descriptors->count; ++i) {
        start.step = PTG220_W_DESCRIPTOR;
        start.payload = (unsigned int)descriptors->entries[i].target;
        status = send_w(state, &start, descriptors->entries[i].held);
    }
    return status;
}

int ptg220_bootstrap_adopt(struct ptg220_bootstrap *state) {
    if (state->w.pidfd < 0 || state->b.pidfd < 0 || state->b_consumed || state->adopted) return -EINVAL;
    struct ptg220_bootstrap_message ready;
    int result = receive_step(state->w_control, state->deadline, state->owners, state->owner_count,
                              state->w.pid, state->caller_uid, state->deadline_ns,
                              PTG220_W_READY, &ready);
    if (result < 0) return result;
    const int original = ready.birth_pidfd == -1 && ready.packet.pid == (uint32_t)state->w.pid &&
        ptg220_pidfd_terminal(state->w.pidfd) == 0 && ptg220_pidfd_terminal(ready.sender_pidfd) == 0;
    ptg220_bootstrap_close(&ready);
    if (!original) return -EPROTO;
    result = send_step(state->b_control, state->deadline_ns, PTG220_B_RETIRE, state->w.pid, -1);
    if (result < 0) return result;
    result = ptg220_join_original(&state->b, state->deadline_ns, &state->b_wait);
    state->b_consumed = state->b_wait.frame.called == 1 && state->b_wait.frame.result == 0 &&
        state->b_wait.frame.pid == state->b.pid;
    if (result < 0) return result;
    if (state->b_wait.frame.code != CLD_EXITED || state->b_wait.frame.status != 0) return -EPROTO;
    unsigned char evidence[PTG_WAIT_NATIVE_CAPACITY];
    result = ptg220_peek_original(&state->w, &state->w_wait.frame, evidence);
    if (result != 0) return result < 0 ? result : -ECHILD;
    result = send_step(state->w_control, state->deadline_ns, PTG220_ADOPT_W, state->w.pid, -1);
    if (result == 0) state->adopted = 1;
    return result;
}

int ptg220_bootstrap_rollback(struct ptg220_bootstrap *state) {
    if (state->adopted) return -EPERM;
    const int had_w = state->w.pidfd >= 0;
    /* Stop original W before any wait or status collection. B may do the same. */
    const int signal = had_w ? ptg220_signal_original(state->w.pidfd) : 0;
    close_fd(state->b_control);
    close_fd(state->w_control);
    state->b_control = -1;
    state->w_control = -1;
    close_fd(state->deadline);
    state->deadline = -1;
    uint64_t selected_ns = state->deadline_ns;
    const int sampled = ptg220_boottime(&selected_ns);
    if (sampled < 0 || selected_ns > state->deadline_ns) selected_ns = state->deadline_ns;
    if (selected_ns > UINT64_MAX - PTG220_SETTLEMENT_NS) return -EOVERFLOW;
    const uint64_t cutoff = selected_ns + PTG220_SETTLEMENT_NS;
    int result = signal < 0 && signal != -ESRCH ? signal : sampled < 0 ? sampled : 0;
    if (state->b.pidfd >= 0 && !state->b_consumed) {
        const int settled = ptg220_join_original(&state->b, cutoff, &state->b_wait);
        state->b_consumed = state->b_wait.frame.called == 1 && state->b_wait.frame.result == 0 &&
            state->b_wait.frame.pid == state->b.pid;
        /* With no W receipt, only B can stop its original W by pidfd.
           Closing A's channels denies exec and wakes B to request that stop. */
        if (settled == -ETIMEDOUT && had_w && (signal == 0 || signal == -ESRCH)) {
            (void)ptg220_signal_original(state->b.pidfd);
        }
        if (result == 0) result = settled;
    }
    if (had_w) {
        const int settled = ptg220_join_original(&state->w, cutoff, &state->w_wait);
        if (result == 0) result = settled;
    } else if (state->b.pidfd >= 0 && state->b_consumed) {
        /* The closed birth gate excludes privileged work without the original
           receipt. A may join an adopted stub but cannot invent birth proof. */
        struct ptg_wait_frame frame;
        unsigned char evidence[PTG_WAIT_NATIVE_CAPACITY];
        const int adopted = ptg220_wait_adopted(0, &frame, evidence);
        if (adopted != -ECHILD && result == 0) result = -EUCLEAN;
    }
    if (result != 0) return result;
    close_fd(state->b.pidfd);
    close_fd(state->w.pidfd);
    state->b.pidfd = -1;
    state->w.pidfd = -1;
    return 0;
}
