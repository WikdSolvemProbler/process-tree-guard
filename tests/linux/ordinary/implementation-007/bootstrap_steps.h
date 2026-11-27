// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_BOOTSTRAP_STEPS_H
#define PTG220_BOOTSTRAP_STEPS_H

#include "bootstrap_wire_primitives.h"
#include "native_control_primitives.h"
#include "owner_watch.h"

static inline int send_step(int fd, uint64_t deadline, enum ptg220_bootstrap_step step,
                     pid_t pid, int birth_fd) {
    const struct ptg220_bootstrap_packet packet = {
        .version = 1, .step = step, .pid = (uint32_t)pid, .deadline_ns = deadline,
    };
    return wire_send(fd, &packet, birth_fd);
}

static inline int receive_step(int channel, int timer, const int *owners, unsigned int count,
                        pid_t sender, uid_t uid, uint64_t deadline, enum ptg220_bootstrap_step step,
                        struct ptg220_bootstrap_message *message) {
    if (count > PTG_OWNER_WATCH_MAX) return -EINVAL;
    struct pollfd watched[PTG_OWNER_WATCH_MAX + 2];
    watched[0] = (struct pollfd){.fd = channel, .events = POLLIN};
    watched[1] = (struct pollfd){.fd = timer, .events = POLLIN};
    for (unsigned int i = 0; i < count; ++i) {
        if (owners[i] < 0) return -EBADF;
        watched[i + 2] = (struct pollfd){.fd = owners[i], .events = POLLIN};
    }
    for (;;) {
        const int polled = control_poll(watched, count + 2, 1);
        if (polled == -EINTR) continue;
        if (polled < 0) return polled;
        uint64_t now;
        int result = control_boottime(&now);
        if (result < 0) return result;
        if (now >= deadline || watched[1].revents == POLLIN) return -ETIMEDOUT;
        if (watched[1].revents) return -EPROTO;
        for (unsigned int i = 0; i < count; ++i) {
            const short ready = watched[i + 2].revents;
            if ((ready & POLLIN) && !(ready & ~(POLLIN | POLLHUP))) return -ECANCELED;
            if (ready) return -EPROTO;
        }
        if (!(watched[0].revents & POLLIN)) return -EPIPE;
        result = wire_receive(channel, message);
        if (result == -EAGAIN || result == -EINTR) continue;
        if (result < 0) return result;
        if (message->sender == sender && message->real_uid == uid &&
            message->packet.deadline_ns == deadline && message->packet.step == (uint32_t)step &&
            message->packet.payload == 0) return 0;
        wire_close(message);
        return -EPROTO;
    }
}

#endif
