// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_LEASE_H
#define PTG220_LEASE_H

#include <stdatomic.h>
#include <stdint.h>
#include <sys/types.h>
#include "evidence.h"

struct ptg220_object {
    uint64_t inode;
    uint64_t mount;
    uint32_t device_major;
    uint32_t device_minor;
};

/* Root-private MAP_SHARED state. Only original W and R may write it. */
struct ptg220_lease {
    _Atomic unsigned int phase;
    struct ptg220_object parent;
    struct ptg220_object leaf;
    char basename[96];
};

struct ptg220_leaf {
    struct ptg220_object identity;
    int directory;
    int kill;
    int events;
};

/* These operations require the independently enrolled C and original W/R. */
void ptg220_lease_init(struct ptg220_lease *lease);
int ptg220_lease_prepare(struct ptg220_lease *lease, int parent, int pid_namespace,
                         pid_t creator_w, pid_t recovery_r);
int ptg220_lease_accept(struct ptg220_lease *lease, int parent, int pid_namespace,
                        pid_t creator_w, pid_t recovery_r);
/* After successful mkdir/open, a concurrent seal returns -ECANCELED and the
   returned leaf still owns its descriptors for immediate retirement. */
int ptg220_lease_create(struct ptg220_lease *lease, int parent, struct ptg220_leaf *leaf);
int ptg220_lease_join(const struct ptg220_lease *lease, int parent, struct ptg220_leaf *leaf);
/* R only, after both independent G endpoint closures are confirmed. */
int ptg220_lease_before_birth(struct ptg220_lease *lease);
int ptg220_lease_p_can_exec(const struct ptg220_lease *lease);
void ptg220_lease_seal(struct ptg220_lease *lease);
int ptg220_lease_sealed_without_birth(const struct ptg220_lease *lease);

int ptg220_leaf_kill(const struct ptg220_leaf *leaf);
int ptg220_leaf_kill_observed(const struct ptg220_leaf *leaf, uint64_t selected,
                               struct ptg220_native_action *action);
int ptg220_leaf_populated(const struct ptg220_leaf *leaf);
void ptg220_leaf_close(struct ptg220_leaf *leaf);

/* Retirement requires the unique administrative removal role and a sealed gate. */
int ptg220_lease_remove_sealed(const struct ptg220_lease *lease, int parent,
                               const struct ptg220_leaf *leaf);
int ptg220_lease_remove_observed(const struct ptg220_lease *lease, int parent,
                                 const struct ptg220_leaf *leaf, struct ptg220_native_action *action);
/* Pre-activation only. R holds the exclusive lease and W's original pidfd. */
int ptg220_lease_recover_sealed(const struct ptg220_lease *lease, int parent,
                                int original_w_pidfd);
/* Only W, after its successful mkdir returned and while R cannot remove. */
int ptg220_lease_recover_own_create(const struct ptg220_lease *lease, int parent);

#endif
