// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_EVIDENCE_H
#define PTG220_EVIDENCE_H

#include <stddef.h>
#include "bootstrap_wire.h"
#include "native_control.h"

#define PTG220_EVIDENCE_VERSION 2u

struct ptg220_native_action {
    int64_t native_return;
    uint64_t before_ns;
    uint64_t after_ns;
    int32_t before_error;
    int32_t after_error;
    int32_t native_errno;
    uint32_t called;
};

struct ptg220_stop_evidence {
    uint64_t observed_ns;
    uint32_t cause;
    uint32_t reserved;
    struct ptg220_native_action kill;
};

enum ptg220_failure_stage {
    PTG220_FAILURE_NONE,
    PTG220_FAILURE_A_INITIAL_TIMER,
    PTG220_FAILURE_A_RESULT_POLL,
    PTG220_FAILURE_A_RESULT_RECEIVE,
    PTG220_FAILURE_A_R_ENVELOPE,
    PTG220_FAILURE_A_R_BIND,
    PTG220_FAILURE_A_R_REARM,
    PTG220_FAILURE_A_R_ACK_SEND,
    PTG220_FAILURE_A_W_FAILURE_ENVELOPE,
    PTG220_FAILURE_A_W_RESULT_ENVELOPE,
    PTG220_FAILURE_A_SEALED_READ,
    PTG220_FAILURE_A_SCOPE_MATCH,
    PTG220_FAILURE_A_TERMINAL_JOIN,
    PTG220_FAILURE_W_R_PEEK,
    PTG220_FAILURE_W_ACK_TIMER,
    PTG220_FAILURE_W_ACK_POLL,
    PTG220_FAILURE_W_ACK_RECEIVE,
    PTG220_FAILURE_W_ACK_BIND,
    PTG220_FAILURE_W_R_CONSUME,
    PTG220_FAILURE_W_SETTLEMENT,
    PTG220_FAILURE_W_RESULT_SEND,
    PTG220_FAILURE_A_R_BIRTH,
};

struct ptg220_settlement_failure {
    uint32_t stage;
    int32_t status;
    int32_t detail;
    uint32_t reserved;
    uint64_t cutoff_ns;
    struct ptg220_receive_observation receive;
};

struct ptg220_caller_retirement {
    uint64_t authenticated_ns;
    uint64_t submit_before_ns;
    uint64_t submit_after_ns;
    struct ptg220_settlement_failure failure;
};

struct ptg220_evidence {
    uint32_t version;
    int32_t final_error;
    int32_t watchdog_error;
    uint32_t reserved;
    uint64_t deadline;
    uint64_t cutoff;
    uint32_t stop_owner;
    uint32_t joined_adoptees;
    uint64_t leaf_inode;
    uint64_t leaf_mount;
    struct ptg220_stop_evidence controller_stop[2];
    struct ptg220_native_action retire;
    struct ptg220_native_action fence_reaper;
    struct ptg220_native_action stop_after_reaper;
    struct ptg220_wait_record bootstrap;
    struct ptg220_wait_record watchdog;
    struct ptg220_wait_record reaper;
    struct ptg220_wait_record payload;
    struct ptg220_wait_record reaper_empty;
    struct ptg220_wait_record watchdog_empty;
    struct ptg220_caller_retirement caller_retirement;
    struct ptg220_settlement_failure watchdog_failure;
    struct ptg220_wait_record reaper_peek;
    uint64_t watchdog_ack_ns;
};

_Static_assert(sizeof(struct ptg220_receive_observation) == 56, "private ancillary readback layout");
_Static_assert(offsetof(struct ptg220_evidence, caller_retirement) == 1488, "prior native evidence prefix");
_Static_assert(sizeof(struct ptg220_evidence) == 1880, "private version-two native evidence layout");

static inline int ptg220_note_settlement_failure(struct ptg220_settlement_failure *failure,
    enum ptg220_failure_stage stage, int status, uint64_t cutoff, int detail,
    const struct ptg220_receive_observation *receive) {
    if (status < 0 && failure->stage == PTG220_FAILURE_NONE) {
        failure->stage = (uint32_t)stage;
        failure->status = status;
        failure->detail = detail;
        failure->cutoff_ns = cutoff;
        if (receive != NULL) failure->receive = *receive;
    }
    return status;
}

int ptg220_evidence_seal(const struct ptg220_evidence *evidence);
int ptg220_evidence_read(int fd, struct ptg220_evidence *evidence);

#endif
