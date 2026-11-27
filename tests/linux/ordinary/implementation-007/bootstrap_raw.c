// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "bootstrap_raw.h"
#include "bootstrap_steps.h"

#if __STDC_HOSTED__ || defined(__SSP__) || defined(__SSP_ALL__) || \
    defined(__SSP_STRONG__) || defined(__SSP_EXPLICIT__)
#error "The caller-derived bootstrap requires its reviewed freestanding flags"
#endif

static _Noreturn void leave(int status) {
    (void)native(SYS_exit_group, status, 0, 0, 0, 0);
    __builtin_trap();
}

static int exec_fixed_helper(int held_image, int control) {
    if (held_image < 0 || control < 3 || held_image == control) return -EBADF;
    const long image_flags = native(SYS_fcntl, held_image, F_GETFD, 0, 0, 0);
    if (image_flags < 0) return (int)image_flags;
    const long control_flags = native(SYS_fcntl, control, F_GETFD, 0, 0, 0);
    if (control_flags < 0) return (int)control_flags;
    if (image_flags != FD_CLOEXEC || control_flags != FD_CLOEXEC) return -EPROTO;
    const long marked = native(SYS_close_range, 0, UINT_MAX, CLOSE_RANGE_CLOEXEC, 0, 0);
    if (marked < 0) return (int)marked;

    char digits[12];
    char *first = digits + sizeof(digits) - 1;
    *first = '\0';
    unsigned int value = (unsigned int)control;
    do {
        *--first = (char)('0' + value % 10);
        value /= 10;
    } while (value != 0);
    char *const arguments[] = {(char *)"ordinary-linux-helper", (char *)"--internal-v1", first, NULL};
    char *const environment[] = {NULL};
    const long inherited = native(SYS_fcntl, control, F_SETFD, 0, 0, 0);
    if (inherited < 0) return (int)inherited;
    const long result = native(SYS_execveat, held_image, (long)"", (long)arguments,
                               (long)environment, AT_EMPTY_PATH);
    /* A returning exec always enters the pre-exec failure path and exits W. */
    (void)native(SYS_fcntl, control, F_SETFD, FD_CLOEXEC, 0, 0);
    return result < 0 ? (int)result : -EPROTO;
}

struct w_stub {
    uint64_t deadline;
    pid_t b;
    uid_t uid;
    int timer;
    int b_pidfd;
    int a_channel;
    int b_wake;
    int w_wake;
    int image;
    int w_channel;
};

long ptg220_raw_clone_b(const struct clone_args *args, size_t size,
                        const struct ptg220_bootstrap_child *child, uintptr_t stack_top);
long ptg220_raw_clone_w(const struct clone_args *args, size_t size, const struct w_stub *child,
                        uintptr_t stack_top);

/* On x86-64 clone3 the child returns with RAX=0 and its new stack. The
   kernel preserves RDX, so the child can pass the copied context to the
   ordinary System V C entry without touching the application's stack.
   A fabricated zero return on the original stack must take the error path. */
#define PTG220_ASM_TEXT1(value) #value
#define PTG220_ASM_TEXT(value) PTG220_ASM_TEXT1(value)
__asm__(
    ".pushsection .text\n"
    ".macro ptg220_clone_branch name, child\n"
    ".p2align 4\n"
    ".globl \\name\n"
    ".hidden \\name\n"
    ".type \\name,@function\n"
    "\\name:\n"
    ".cfi_startproc\n"
    "endbr64\n"
    "mov %rcx, %r8\n"
    "mov $" PTG220_ASM_TEXT(SYS_clone3) ", %eax\n"
    "syscall\n"
    "test %rax, %rax\n"
    "jnz 2f\n"
    "cmp %r8, %rsp\n"
    "je 1f\n"
    "mov $-" PTG220_ASM_TEXT(EPROTO) ", %eax\n"
    "cltq\n"
    "2: ret\n"
    "1:\n"
    ".cfi_undefined 16\n"
    "mov %rdx, %rdi\n"
    "call \\child\n"
    "ud2\n"
    ".cfi_endproc\n"
    ".size \\name, .-\\name\n"
    ".endm\n"
    "ptg220_clone_branch ptg220_raw_clone_b, ptg220_raw_b_child\n"
    "ptg220_clone_branch ptg220_raw_clone_w, ptg220_raw_w_child\n"
    ".purgem ptg220_clone_branch\n"
    ".popsection\n"
);

__attribute__((used, noinline))
static _Noreturn void ptg220_raw_w_child(const struct w_stub *child) {
    if (control_arm_parent_death(child->b, child->b_pidfd, child->deadline) < 0) leave(125);
    wire_close_fd(child->a_channel);
    wire_close_fd(child->b_wake);
    struct ptg220_bootstrap_message permit;
    if (receive_step(child->w_wake, child->timer, &child->b_pidfd, 1, child->b, child->uid,
                     child->deadline, PTG220_COMMIT_W_EXEC, &permit) < 0) leave(125);
    const int valid = permit.birth_pidfd == -1 &&
        permit.packet.pid == (uint32_t)native(SYS_getpid, 0, 0, 0, 0, 0);
    wire_close(&permit);
    if (!valid) leave(125);
    wire_close_fd(child->w_wake);
    wire_close_fd(child->b_pidfd);
    (void)exec_fixed_helper(child->image, child->w_channel);
    leave(125);
}

__attribute__((used, noinline))
static _Noreturn void ptg220_raw_b_child(const struct ptg220_bootstrap_child *child) {
    const uint64_t deadline = child->deadline_ns;
    const int *owners = child->owners;
    int status = control_arm_parent_death(child->caller, owners[0], deadline);
    if (status < 0) leave(125);
    wire_close_fd(child->parent_bootstrap);
    wire_close_fd(child->parent_helper);
    wire_close_fd(child->parent_timer);
    const int timer = control_deadline_fd(deadline);
    if (timer < 0) leave(125);
    const int self = control_open_self_pidfd();
    const pid_t b = (pid_t)native(SYS_getpid, 0, 0, 0, 0, 0);
    int wake[2];
    if (self < 0 || wire_pair(wake) < 0) leave(125);
    status = control_disarm_parent_death(child->caller, owners[0], deadline);
    if (status < 0) leave(125);

    const struct w_stub newborn = {
        .deadline = deadline, .b = b, .uid = child->uid, .timer = timer, .b_pidfd = self,
        .a_channel = child->child_bootstrap, .b_wake = wake[0], .w_wake = wake[1],
        .image = child->image, .w_channel = child->child_helper,
    };
    struct ptg220_birth w = {.pidfd = -1};
    const struct clone_args args = {
        .flags = CLONE_PIDFD, .pidfd = (uintptr_t)&w.pidfd, .exit_signal = SIGCHLD,
        .stack = (uintptr_t)child->w_stack, .stack_size = child->stack_size,
    };
    const long started = ptg220_raw_clone_w(&args, sizeof(args), &newborn,
                                            args.stack + args.stack_size);
    if (started <= 0) leave(125);
    w.pid = (pid_t)started;
    const int channels = child->child_bootstrap;
    const unsigned int count = child->owner_count;
    const pid_t a = child->caller;
    const uid_t uid = child->uid;
    wire_close_fd(wake[1]);
    wire_close_fd(child->child_helper);
    wire_close_fd(child->image);
    status = send_step(channels, deadline, PTG220_BIRTH_W, w.pid, w.pidfd);
    if (status == 0) {
        struct ptg220_bootstrap_message accepted;
        status = receive_step(channels, timer, owners, count, a, uid, deadline,
                              PTG220_COMMIT_W_EXEC, &accepted);
        if (status == 0) {
            if (accepted.packet.pid != (uint32_t)w.pid || accepted.birth_pidfd >= 0) status = -EPROTO;
            wire_close(&accepted);
        }
    }
    if (status == 0) status = send_step(wake[0], deadline, PTG220_COMMIT_W_EXEC, w.pid, -1);
    wire_close_fd(wake[0]);
    if (status == 0) {
        struct ptg220_bootstrap_message release;
        status = receive_step(channels, timer, owners, count, a, uid, deadline, PTG220_B_RETIRE, &release);
        if (status == 0) {
            const int valid = release.packet.pid == (uint32_t)w.pid && release.birth_pidfd == -1;
            wire_close(&release);
            if (valid && control_pidfd_terminal(w.pidfd) == 0) leave(0);
        }
    }
    /* B has first-stop authority until the successful original retirement. */
    (void)control_signal_original(w.pidfd);
    uint64_t selected;
    if (control_boottime(&selected) < 0) leave(125);
    if (selected > deadline) selected = deadline;
    if (selected > UINT64_MAX - PTG220_SETTLEMENT_NS) leave(125);
    struct ptg220_wait_record terminal = {0};
    (void)control_join_original(&w, selected + PTG220_SETTLEMENT_NS, &terminal);
    leave(125);
}

int ptg220_bootstrap_spawn(struct ptg220_birth *b, const struct ptg220_bootstrap_child *child) {
    *b = (struct ptg220_birth){.pidfd = -1};
    const struct clone_args args = {
        .flags = CLONE_PIDFD, .pidfd = (uintptr_t)&b->pidfd, .exit_signal = SIGCHLD,
        .stack = (uintptr_t)child->b_stack, .stack_size = child->stack_size,
    };
    const long started = ptg220_raw_clone_b(&args, sizeof(args), child,
                                            args.stack + args.stack_size);
    if (started > 0) b->pid = (pid_t)started;
    return (int)started;
}
