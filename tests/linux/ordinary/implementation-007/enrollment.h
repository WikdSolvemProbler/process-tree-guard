// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_ENROLLMENT_H
#define PTG220_ENROLLMENT_H

#include <stdint.h>

struct ptg220_enrollment {
    int parent;
    int pid_namespace;
};

/* Internal caller-side lookup. No caller-supplied privileged path. */
int ptg220_open_fixed_helper(void);
int ptg220_caller_preflight(void);
/* Internal privileged lookup. Uses actual current credentials and context. */
int ptg220_read_enrollment(uint64_t deadline_ns, struct ptg220_enrollment *enrolled);

#endif
