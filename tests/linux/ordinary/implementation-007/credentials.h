// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_CREDENTIALS_H
#define PTG220_CREDENTIALS_H

#include <sys/resource.h>
#include <sys/types.h>

/* From actual inherited IDs, never from a privileged wire request. */
struct ptg220_ids {
    uid_t uid;
    gid_t gid;
};

int ptg220_check_ordinary_ids(void);

int ptg220_restrict_privileged_helper(struct ptg220_ids *original);
int ptg220_restore_soft_stack(const struct rlimit *original);
/* Call only in the gated P child. Any error permanently forbids its exec. */
int ptg220_restore_payload_ids(const struct ptg220_ids *original);

#endif
