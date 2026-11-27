// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_HELPER_STARTUP_H
#define PTG220_HELPER_STARTUP_H

#include "credentials.h"
#include "enrollment.h"
#include "owner_watch.h"
#include "request.h"
#include "descriptors.h"
#include <stdint.h>

struct ptg220_helper_startup {
    int channel;
    int owners[PTG_OWNER_WATCH_MAX];
    unsigned int owner_count;
    pid_t caller;
    uint64_t deadline_ns;
    int deadline;
    struct ptg220_enrollment enrollment;
    struct ptg220_ids original_ids;
    struct ptg220_request request;
    struct ptg220_descriptors descriptors;
};

/* Called only by the fixed trusted helper; success denotes custody, no G or P. */
int ptg220_helper_take_adopted(int channel, struct ptg220_helper_startup *state);
void ptg220_helper_drop_startup(struct ptg220_helper_startup *state);

#endif
