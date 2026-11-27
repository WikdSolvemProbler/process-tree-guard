// SPDX-License-Identifier: Apache-2.0
#define _GNU_SOURCE
#include "bootstrap_wire_primitives.h"

#ifndef SO_PEERPIDFD
#define SO_PEERPIDFD 77
#endif

int ptg220_bootstrap_pair(int pair[2]) {
    return wire_pair(pair);
}

int ptg220_bootstrap_send(int fd, const struct ptg220_bootstrap_packet *packet, int birth_pidfd) {
    return wire_send(fd, packet, birth_pidfd);
}

void ptg220_bootstrap_close(struct ptg220_bootstrap_message *message) {
    wire_close(message);
}

int ptg220_bootstrap_receive(int fd, struct ptg220_bootstrap_message *message) {
    return wire_receive(fd, message);
}

int ptg220_bootstrap_receive_observed(int fd, struct ptg220_bootstrap_message *message,
                                     struct ptg220_receive_observation *observation) {
    return wire_receive_impl(fd, message, observation, PTG220_ACK_R_SETTLED);
}

int ptg220_settlement_packet(const struct ptg220_bootstrap_message *message,
                             enum ptg220_bootstrap_step step, pid_t subject, uint64_t deadline, uint64_t cutoff) {
    const struct ptg220_bootstrap_packet *const packet = &message->packet;
    const unsigned char *const actual = (const unsigned char *)&packet->native_wait;
    if (message->birth_pidfd >= 0 || message->sender_pidfd < 0 ||
        packet->step != (uint32_t)step || packet->pid != (uint32_t)subject ||
        packet->deadline_ns != deadline || packet->cutoff_ns != cutoff ||
        packet->payload || packet->owner.pid || packet->owner.ppid ||
        packet->owner.start_ticks || packet->descriptor_count) return -EPROTO;
    for (size_t i = 0; i < sizeof(packet->native_wait); ++i) {
        if (actual[i] != 0) return -EPROTO;
    }
    return 0;
}

int ptg220_bootstrap_creator(int fd, pid_t *pid, uid_t *uid) {
    struct ucred credential;
    socklen_t count = sizeof(credential);
    long result = native(SYS_getsockopt, fd, SOL_SOCKET, SO_PEERCRED,
                         (long)&credential, (long)&count);
    if (result < 0) return (int)result;
    if (count != sizeof(credential) || credential.pid <= 0) return -EPROTO;
    int creator_pidfd = -1;
    count = sizeof(creator_pidfd);
    result = native(SYS_getsockopt, fd, SOL_SOCKET, SO_PEERPIDFD,
                    (long)&creator_pidfd, (long)&count);
    if (result < 0) return (int)result;
    if (count != sizeof(creator_pidfd) || creator_pidfd < 0) {
        wire_close_fd(creator_pidfd);
        return -EPROTO;
    }
    *pid = credential.pid;
    *uid = credential.uid;
    return creator_pidfd;
}
