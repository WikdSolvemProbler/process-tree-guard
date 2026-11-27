// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_BOOTSTRAP_RAW_H
#define PTG220_BOOTSTRAP_RAW_H

#include "native_control.h"
#include "owner_watch.h"

#include <stddef.h>

struct ptg220_bootstrap_child {
    uint64_t deadline_ns;
    int owners[PTG_OWNER_WATCH_MAX];
    unsigned int owner_count;
    pid_t caller;
    uid_t uid;
    int parent_bootstrap;
    int child_bootstrap;
    int parent_helper;
    int child_helper;
    int parent_timer;
    int image;
    void *b_stack;
    void *w_stack;
    size_t stack_size;
};

/* Only the original A returns. B and its W stub stay in the raw unit. */
int ptg220_bootstrap_spawn(struct ptg220_birth *b, const struct ptg220_bootstrap_child *child);

#endif
