// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#ifndef PTG_PAYLOAD_BOOTSTRAP_H
#define PTG_PAYLOAD_BOOTSTRAP_H

#include <stddef.h>
#include <stdint.h>

/* Product integration in progress: no backend activation or runtime qualification.
 * Linux LP64 aarch64/x86-64 only. This is a PRIVATE trusted-supervisor ABI,
 * never a request packet. No raw request-controlled descriptor/pointer is valid.
 */
#define PTG_PAYLOAD_SCHEMA 1u
#define PTG_PAYLOAD_HELD 0
#define PTG_PAYLOAD_RELEASED 1
#define PTG_PAYLOAD_CANCELLED 2
#define PTG_PAYLOAD_MAX_GROUPS 65536u
#define PTG_PAYLOAD_MAX_VECTOR 65536u
#define PTG_PAYLOAD_MAX_STRING_BYTES 131072u

struct ptg_payload_gate;
/* Allocate before any clone. Shared RW parent/watchdog mapping; child makes its
 * own mapping read-only. All gate accesses are lock-free C atomics. Cancellation
 * is irreversible from HELD or RELEASED and must precede EVERY held kill write.
 * destroy is parent-only, after every child is settled; uncertainty retains it.
 */
struct ptg_payload_gate *ptg_payload_gate_create(int32_t *error);
int32_t ptg_payload_gate_destroy(struct ptg_payload_gate *gate, int32_t *error);
int32_t ptg_payload_gate_state(const struct ptg_payload_gate *gate);
int32_t ptg_payload_gate_cancel(struct ptg_payload_gate *gate); /* previous state */
/* 0=HELD->RELEASED performed, 1=already RELEASED, 2=CANCELLED, -1=null.
 * Does not deliver a permit. Duplicate commit is a refusal, not success. */
int32_t ptg_payload_gate_commit(struct ptg_payload_gate *gate);

struct ptg_payload_spec {
    uint32_t schema, bytes;
    int32_t cgroup_fd, kill_fd, owner_fd, timer_fd, supervisor_fd, watchdog_fd;
    int32_t cwd_fd, stdio_fd[3], permit_reader, ready_writer, exec_writer;
    uint32_t uid, gid, group_count, argc, envc;
    uint64_t deadline_ns, original_mask;
    const uint32_t *groups;
    const char *program;
    const char *const *argv;
    const char *const *envp;
    struct ptg_payload_gate *gate;
};
/* All FDs are BORROWED by C. Parent Rust owns every endpoint and closes its own
 * child-only writers/readers immediately after spawn. No C function closes a
 * parent FD. stdio/CWD and immutable argv/envp/group buffers must be prepared
 * before clone; vectors include their final NULL. UID/GID/groups are verified
 * non-root caller credentials. groups are in nondecreasing kernel readback order.
 * READY and EXE1 use private CLOEXEC nonblocking pipes. Permit uses a private
 * connected AF_UNIX SOCK_SEQPACKET|SOCK_CLOEXEC|SOCK_NONBLOCK socketpair;
 * permit_reader is its child endpoint, and the parent alone sends exactly one
 * one-byte G packet using MSG_NOSIGNAL. No global SIGPIPE policy is changed.
 * All child control endpoints and the parent READY reader are nonblocking. Exec reader
 * belongs to the existing EXE1 decoder. The timer is the ORIGINAL armed absolute
 * CLOCK_BOOTTIME lease; it is never read, consumed, disarmed or retargeted here.
 * An independent live watchdog covers the whole operation and shares gate.
 */
struct ptg_payload_spawn {
    uint32_t schema, stage;
    int32_t child_created, pid, pidfd, error;
    int32_t clone_result, initial_poll_result, initial_revents[4];
    int32_t mask_set_result, mask_set_error, mask_restore_result, mask_restore_error;
    int32_t kill_result, kill_error, scratch_unmap_result, scratch_unmap_error;
    uint64_t original_mask, blocked_mask, restored_mask, before_clone_ns, after_clone_ns;
    int32_t gate_state, reserved;
};
/* A successful clone ALWAYS publishes child_created=1 and the exact positive
 * PID/kernel-installed pidfd before mask restoration, clocks, or later errors.
 * Parent MUST own/reap that child even if stage/error subsequently refuse.
 * No successful clone can be translated into a no-child error. The child branch
 * NEVER returns to Rust, allocates Rust memory, formats, or invokes exec wrappers.
 */
void ptg_payload_spawn(const struct ptg_payload_spec *spec, struct ptg_payload_spawn *out);

struct ptg_payload_ready {
    uint32_t schema, stage;
    int32_t result, error, read_count, poll_result, revents[7];
    uint32_t message_schema, message_token;
    uint64_t child_mask, before_ns, after_ns;
    int32_t kill_result, kill_error;
};
/* READY wire is exactly16 bytes: schema:u32=1, token:u32='R', blocked_mask:u64.
 * It follows FD scrub and verified groups/gid/uid/zero caps/NNP/CWD. It certifies
 * bootstrap preparation (including verified dumpable0), NOT exec or terminal completion. setup_timer is an
 * independently armed finite readiness timer; no C-owned timer is introduced.
 */
void ptg_payload_wait_ready(const struct ptg_payload_spec *spec,
    int32_t ready_reader, int32_t payload_pidfd, int32_t setup_timer,
    uint64_t setup_deadline_ns, struct ptg_payload_ready *out);

struct ptg_payload_release {
    uint32_t schema, stage;
    int32_t result, error, poll_result, revents[5];
    int32_t gate_before, gate_after, write_result, write_error;
    int32_t kill_result, kill_error;
    uint64_t before_ns, after_ns;
};
/* Final poll/clock observation, HELD->RELEASED CAS, and exactly one nonblocking
 * 'G' packet through MSG_NOSIGNAL. Every refusal latches CANCELLED before attempting held cgroup.kill.
 * A racing cancellation can win after the observation; the watchdog must then
 * kill/drain. Neither successful send nor pipe EOF proves exec or settlement.
 */
void ptg_payload_commit(const struct ptg_payload_spec *spec,
    int32_t payload_pidfd, int32_t permit_writer, struct ptg_payload_release *out);

/* Existing exec report is exactly8 bytes: ASCII EXE1 + positive errno LE32.
 * Bootstrap failure attempts that report then exits125; execve failure exits127.
 * EOF alone remains NoExecErrorReport, never an exec/cleanup proof.
 * Supplied by the separately owned kernel-UAPI translation unit; no libc
 * sigaction layout/reserved-signal filtering is invented in this bridge.
 */
int ptg_payload_reset_dispositions(int32_t *failed_signal, int32_t *error);

/* Scalar layout/constants reporter: 0=schema,1=spec,2=spawn,3=ready,4=release,
 * 5=READY bytes,6=exec bytes,7=clone flags,8=SIGCHLD,9=clone3 syscall,
 * 10=CLOCK_BOOTTIME,11=kernel blocked mask,12=spec.gate offset.
 */
uint64_t ptg_payload_abi(uint32_t word);
#endif
