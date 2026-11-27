// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_OWNERS_H
#define PTG220_OWNERS_H

#include <stdint.h>

struct ptg220_owner_identity {
    uint32_t pid;
    uint32_t ppid;
    uint64_t start_ticks;
};

/* Verify a held pidfd in the already qualified procfs/PID namespace. Allows zombies. */
int ptg220_pidfd_names(int pidfd, uint32_t expected);
/* Both held pidfds name the still-attached original in the same PID namespace. */
int ptg220_bind_pidfds(int original, int reported, uint32_t expected);
/* Existing Manual=0 and Auto=1 selection. No numeric PID is reopened for control. */
int ptg220_verify_owners(const int *pidfds, const struct ptg220_owner_identity *originals,
                         unsigned int count, unsigned int mode);

#endif
