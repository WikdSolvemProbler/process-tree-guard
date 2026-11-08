// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#ifndef PTG_OWNER_WATCH_H
#define PTG_OWNER_WATCH_H
#include <stddef.h>
#include <stdint.h>

/* Requester plus every original required owner. The single atomic ancillary
 * packet reserves four additional rights for stdio/CWD (Linux SCM_MAX_FD253).
 * Overflow refuses the entire invocation; no owner is truncated or substituted.
 * Rust owns these pidfds and their immutable number array through all C calls.
 * A raw-clone child inherits its own COW array and kernel descriptor references.
 */
#define PTG_OWNER_WATCH_MIN 2u
#define PTG_OWNER_WATCH_MAX 249u
struct ptg_owner_watch {
    const int32_t *fds;
    uint32_t count, reserved;
};
_Static_assert(sizeof(struct ptg_owner_watch)==16
    && offsetof(struct ptg_owner_watch,count)==8,"LP64 owner watch ABI");

#endif
