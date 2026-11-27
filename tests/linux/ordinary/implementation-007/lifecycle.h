// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_LIFECYCLE_H
#define PTG220_LIFECYCLE_H

#include "helper_startup.h"
#include "native_control.h"
#include "owners.h"

enum ptg220_outcome {
    PTG220_UNCONFIRMED = 0,
    PTG220_EXITED = 1,
    PTG220_SIGNALED = 2,
    PTG220_TIMED_OUT = 3,
    PTG220_OWNER_ENDED = 4,
    PTG220_EXEC_REFUSED = 5,
};

struct ptg220_result {
    enum ptg220_outcome outcome;
    int value;
    int guard_errno;
    struct ptg_wait_frame worker_wait;
    int evidence_fd;
};

_Static_assert(sizeof(struct ptg220_result) == 56, "private Rust/C result ABI");

/* Only called by the fixed helper after authenticated startup and adoption. */
int ptg220_supervise(struct ptg220_helper_startup *startup);

/* Native private entry consumed after OriginalInvocation capture. */
int ptg220_run(const char *path, char *const argv[], char *const envp[],
               uint64_t deadline_ns, const int *original_owner_pidfds, unsigned int owner_count,
               const struct ptg220_owner_identity *originals, unsigned int owner_mode,
               struct ptg220_result *result);

#endif
