// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#ifndef PTG_ENTRY_TIMER_H
#define PTG_ENTRY_TIMER_H
#include <stddef.h>
#include <stdint.h>

/* Private root-service ABI. No timer_t representation crosses into Rust.
 * These timers signal SIGKILL to THIS process only. Entry may create NO children
 * until a verified PID1 service's recursive-kill policy supplies whole-unit
 * coverage on MainPID exit. Fixed PID1 startup/runtime limits cover pre-main
 * and pre-arm code; those facts must be checked by entry, not assumed here.
 */
struct ptg_entry_timer;
struct ptg_entry_timer_observation {
    uint32_t schema, stage;
    int32_t result, call_errno, created, armed;
    int32_t create_result, create_errno, set_result, set_errno;
    int32_t get_result, get_errno, delete_result, delete_errno;
    uint64_t deadline_ns, observed_ns, remaining_ns, interval_ns, before_ns;
};
/* *owner must initially be NULL. After timer_create succeeds its actual owner
 * is published BEFORE arming/readback. A failed result can therefore still
 * return a live owner; never drop that custody or infer no timer was created.
 * Uncalled raw results are INT32_MIN. Create success requires stage4, result0, all
 * create/set/get results0 and errno0, a future deadline and nonzero readback.
 */
void ptg_entry_timer_create(uint64_t absolute_boottime_ns,
    struct ptg_entry_timer **owner,struct ptg_entry_timer_observation *out);
/* The final cleanup bound is sticky: deletion is forbidden even if requested
 * accidentally. It remains armed until this root-service process actually exits.
 */
void ptg_entry_cleanup_timer_create(uint64_t absolute_boottime_ns,
    struct ptg_entry_timer **owner,struct ptg_entry_timer_observation *out);
void ptg_entry_timer_observe(const struct ptg_entry_timer *owner,
    struct ptg_entry_timer_observation *out);
/* An observe-only receipt leaves create/set/delete results INT32_MIN. */
/* Bootstrap-only. Call ONLY after an independent sticky cleanup timer has been
 * armed and verified. Success consumes/frees the owner; failure retains it.
 * There is no automatic Drop/disarm promise and no rearm/extension function.
 */
void ptg_entry_timer_delete(struct ptg_entry_timer *owner,
    struct ptg_entry_timer_observation *out);
uint64_t ptg_entry_timer_abi(uint32_t word);
#endif
