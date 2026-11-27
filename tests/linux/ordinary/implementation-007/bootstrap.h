// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_BOOTSTRAP_H
#define PTG220_BOOTSTRAP_H

#include "native_control.h"
#include "owners.h"
#include "descriptors.h"

struct ptg220_bootstrap {
    uint64_t deadline_ns;
    struct ptg220_birth b;
    struct ptg220_birth w;
    int b_control;
    int w_control;
    int deadline;
    uid_t caller_uid;
    const int *owners;
    unsigned int owner_count;
    int b_consumed;
    int adopted;
    struct ptg220_wait_record b_wait;
    struct ptg220_wait_record w_wait;
};

/* Private caller role. Caller context checks and owner capture precede this call. */
int ptg220_bootstrap_begin(uint64_t deadline_ns, const int *owner_pidfds, unsigned int owner_count,
                           const struct ptg220_owner_identity *owners, unsigned int owner_mode,
                           int immutable_request, const struct ptg220_descriptors *descriptors,
                           struct ptg220_bootstrap *state);
/* Finishes the bootstrap handshake only. This grants no payload admission. */
int ptg220_bootstrap_adopt(struct ptg220_bootstrap *state);
/* An uncertain rollback retains the state and original descriptors to report. */
int ptg220_bootstrap_rollback(struct ptg220_bootstrap *state);

#endif
