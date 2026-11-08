// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#ifndef PTG_NATIVE_ARMING_H
#define PTG_NATIVE_ARMING_H
#include <stdint.h>
#include <stddef.h>
#include "owner_watch.h"
struct ptg_arm_spawn {
    uint32_t schema, stage;
    int32_t child_created, pid, pidfd, call_errno;
    int32_t mask_restore_result, mask_restore_errno;
    int32_t initial_poll_result, initial_revents[3], clone_result, cancellation_revents;
    uint64_t original_signal_mask, before_clone_ns, after_clone_ns;
    int32_t mask_set_result, mask_set_errno, mask_query_result, mask_query_errno;
    int32_t mask_restore_query_result, mask_restore_query_errno, failure_kill_result, failure_kill_errno;
    uint64_t blocked_mask, restored_mask;
};
struct ptg_arm_ready {
    int32_t result, call_errno, read_count;
    uint32_t token;
    int32_t revents[7], after_revents[7], poll_result, after_poll_result;
    uint64_t before_ns, after_ns;
    uint32_t message_schema, reserved;
    uint64_t child_mask;
};
struct ptg_arm_kill { int32_t result, call_errno; };
uint64_t ptg_arm_abi(uint32_t word);
uint64_t ptg_arm_kernel_signal_abi(uint32_t word);
struct ptg_payload_gate;
void ptg_arm_spawn_watchdog(struct ptg_payload_gate *gate,const struct ptg_owner_watch *owners, int32_t timer, int32_t supervisor,
    int32_t cancellation, int32_t kill_fd, int32_t ready_writer, uint64_t original_deadline_ns,
    struct ptg_arm_spawn *out);
void ptg_arm_wait_ready(int32_t ready, int32_t child, const struct ptg_owner_watch *owners, int32_t timer,
    int32_t supervisor, int32_t cancellation, int32_t setup_timer, uint64_t setup_deadline_ns,
    struct ptg_arm_ready *out);
void ptg_arm_kill_held(int32_t kill_fd, struct ptg_arm_kill *out);
#endif
