// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_BOOTSTRAP_WIRE_H
#define PTG220_BOOTSTRAP_WIRE_H

#include <stdint.h>
#include <sys/types.h>
#include "waitid_bridge.h"
#include "owners.h"

enum ptg220_bootstrap_step {
    PTG220_BIRTH_W = 1,
    PTG220_COMMIT_W_EXEC,
    PTG220_START_W,
    PTG220_W_OWNER,
    PTG220_W_REQUEST,
    PTG220_W_DESCRIPTOR,
    PTG220_W_READY,
    PTG220_B_RETIRE,
    PTG220_ADOPT_W,
    PTG220_BIRTH_R,
    PTG220_ACK_R,
    PTG220_R_READY,
    PTG220_G_OFFER,
    PTG220_G_ACCEPTED,
    PTG220_G_CREATED,
    PTG220_P_READY,
    PTG220_P_BIRTH,
    PTG220_P_HELD,
    PTG220_P_RELEASE,
    PTG220_R_RESULT,
    PTG220_R_SETTLED,
    PTG220_W_RESULT,
    PTG220_W_FAILURE,
    PTG220_ACK_R_SETTLED,
};

struct ptg220_bootstrap_packet {
    uint64_t deadline_ns;
    uint64_t payload;
    uint32_t version;
    uint32_t step;
    uint32_t pid;
    uint32_t reserved;
    uint64_t cutoff_ns;
    struct ptg_wait_frame native_wait;
    struct ptg220_owner_identity owner;
    uint64_t descriptor_count;
};

struct ptg220_bootstrap_message {
    struct ptg220_bootstrap_packet packet;
    pid_t sender;
    uid_t real_uid;
    gid_t real_gid;
    int sender_pidfd;
    int birth_pidfd;
};

struct ptg220_receive_observation {
    int64_t native_count;
    uint64_t control_bytes;
    int32_t msg_flags;
    int32_t pidfd_value;
    uint32_t called;
    uint32_t complete_packet;
    uint32_t step;
    uint32_t credential_count;
    uint32_t pidfd_count;
    uint32_t rights_count;
    uint32_t pidfd_int_count;
    uint32_t bad_cmsg_length;
};

int ptg220_bootstrap_pair(int pair[2]);
int ptg220_bootstrap_send(int fd, const struct ptg220_bootstrap_packet *packet, int birth_pidfd);
/* Each received descriptor belongs to the message on positive return only. */
int ptg220_bootstrap_receive(int fd, struct ptg220_bootstrap_message *message);
int ptg220_bootstrap_receive_observed(int fd, struct ptg220_bootstrap_message *message,
                                     struct ptg220_receive_observation *observation);
/* Uses the authenticated packet sender and keeps specific application proof separate. */
int ptg220_settlement_packet(const struct ptg220_bootstrap_message *message,
                             enum ptg220_bootstrap_step step, pid_t subject, uint64_t deadline, uint64_t cutoff);
void ptg220_bootstrap_close(struct ptg220_bootstrap_message *message);
/* Pins socketpair creator A through the kernel even when B inherited an end. */
int ptg220_bootstrap_creator(int fd, pid_t *pid, uid_t *uid);

#endif
