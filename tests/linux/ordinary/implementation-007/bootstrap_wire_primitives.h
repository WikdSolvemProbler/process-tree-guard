// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_BOOTSTRAP_WIRE_PRIMITIVES_H
#define PTG220_BOOTSTRAP_WIRE_PRIMITIVES_H

#include "bootstrap_wire.h"
#include "native_syscall.h"

#include <errno.h>
#include <stddef.h>
#include <sys/socket.h>
#include <sys/syscall.h>

#ifndef SO_PASSPIDFD
#define SO_PASSPIDFD 76
#endif
#ifndef SCM_PIDFD
#define SCM_PIDFD 0x04
#endif

_Static_assert(sizeof(struct ptg220_bootstrap_packet) == 104, "shared bootstrap packet size");

static inline void wire_close_fd(int fd) {
    if (fd >= 0) (void)native(SYS_close, fd, 0, 0, 0, 0);
}

static inline int wire_pair(int pair[2]) {
    const long opened = native(SYS_socketpair, AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC | SOCK_NONBLOCK,
                               0, (long)pair, 0);
    if (opened < 0) return (int)opened;
    const int enabled = 1;
    for (unsigned int i = 0; i < 2; ++i) {
        long result = native(SYS_setsockopt, pair[i], SOL_SOCKET, SO_PASSCRED,
                             (long)&enabled, sizeof(enabled));
        if (result == 0) result = native(SYS_setsockopt, pair[i], SOL_SOCKET, SO_PASSPIDFD,
                                         (long)&enabled, sizeof(enabled));
        if (result < 0) {
            wire_close_fd(pair[0]);
            wire_close_fd(pair[1]);
            pair[0] = -1;
            pair[1] = -1;
            return (int)result;
        }
    }
    return 0;
}

static inline int wire_send(int fd, const struct ptg220_bootstrap_packet *packet, int birth_pidfd) {
    struct iovec data = {.iov_base = (void *)packet, .iov_len = sizeof(*packet)};
    struct msghdr message = {.msg_iov = &data, .msg_iovlen = 1};
    union { struct cmsghdr aligned; unsigned char bytes[CMSG_SPACE(sizeof(int))]; } rights;
    if (birth_pidfd >= 0) {
        rights.aligned.cmsg_len = CMSG_LEN(sizeof(int));
        rights.aligned.cmsg_level = SOL_SOCKET;
        rights.aligned.cmsg_type = SCM_RIGHTS;
        *(int *)CMSG_DATA(&rights.aligned) = birth_pidfd;
        message.msg_control = rights.bytes;
        message.msg_controllen = sizeof(rights.bytes);
    }
    const long sent = native(SYS_sendmsg, fd, (long)&message, MSG_NOSIGNAL | MSG_DONTWAIT, 0, 0);
    return sent == (long)sizeof(*packet) ? 0 : sent < 0 ? (int)sent : -EPROTO;
}

static inline void wire_close(struct ptg220_bootstrap_message *message) {
    wire_close_fd(message->sender_pidfd);
    wire_close_fd(message->birth_pidfd);
    message->sender_pidfd = -1;
    message->birth_pidfd = -1;
}

static inline __attribute__((always_inline))
int wire_receive_impl(int fd, struct ptg220_bootstrap_message *message,
                      struct ptg220_receive_observation *observation, uint32_t last_step) {
    struct iovec data = {.iov_base = &message->packet, .iov_len = sizeof(message->packet)};
    union {
        struct cmsghdr aligned;
        unsigned char bytes[CMSG_SPACE(sizeof(struct ucred)) + 2 * CMSG_SPACE(sizeof(int))];
    } control;
    struct msghdr envelope = {
        .msg_iov = &data, .msg_iovlen = 1,
        .msg_control = control.bytes, .msg_controllen = sizeof(control.bytes),
    };
    message->sender_pidfd = -1;
    message->birth_pidfd = -1;
    const long count = native(SYS_recvmsg, fd, (long)&envelope,
                              MSG_CMSG_CLOEXEC | MSG_DONTWAIT, 0, 0);
    if (observation != NULL) {
        *observation = (struct ptg220_receive_observation){.called = 1, .native_count = count};
        if (count >= 0) {
            observation->control_bytes = envelope.msg_controllen;
            observation->msg_flags = envelope.msg_flags;
            if (count == (long)sizeof(message->packet) && !(envelope.msg_flags & MSG_TRUNC)) {
                observation->complete_packet = 1;
                observation->step = message->packet.step;
            }
        }
    }
    if (count < 0) return (int)count;
    int bad = count != (long)sizeof(message->packet) || (envelope.msg_flags & (MSG_TRUNC | MSG_CTRUNC));
    int credential_count = 0;
    for (size_t offset = 0; offset + sizeof(struct cmsghdr) <= envelope.msg_controllen;) {
        const struct cmsghdr *entry = (const struct cmsghdr *)(control.bytes + offset);
        if (entry->cmsg_len < CMSG_LEN(0) || entry->cmsg_len > envelope.msg_controllen - offset) {
            if (observation != NULL) observation->bad_cmsg_length = 1;
            bad = 1;
            break;
        }
        if (observation != NULL && entry->cmsg_level == SOL_SOCKET) {
            if (entry->cmsg_type == SCM_CREDENTIALS) ++observation->credential_count;
            if (entry->cmsg_type == SCM_RIGHTS) ++observation->rights_count;
            if (entry->cmsg_type == SCM_PIDFD) {
                ++observation->pidfd_count;
                if (entry->cmsg_len == CMSG_LEN(sizeof(int))) {
                    if (observation->pidfd_int_count == 0) {
                        observation->pidfd_value = *(const int *)CMSG_DATA(entry);
                    }
                    ++observation->pidfd_int_count;
                }
            }
        }
        if (entry->cmsg_level == SOL_SOCKET && entry->cmsg_type == SCM_CREDENTIALS &&
            entry->cmsg_len == CMSG_LEN(sizeof(struct ucred))) {
            const struct ucred *credential = (const struct ucred *)CMSG_DATA(entry);
            ++credential_count;
            message->sender = credential->pid;
            message->real_uid = credential->uid;
            message->real_gid = credential->gid;
        } else if (entry->cmsg_level == SOL_SOCKET &&
                   (entry->cmsg_type == SCM_PIDFD || entry->cmsg_type == SCM_RIGHTS)) {
            int *held = entry->cmsg_type == SCM_PIDFD ? &message->sender_pidfd : &message->birth_pidfd;
            const size_t bytes = entry->cmsg_len - CMSG_LEN(0);
            const int *passed = (const int *)CMSG_DATA(entry);
            if (*held < 0 && bytes == sizeof(int) && passed[0] >= 0) {
                *held = passed[0];
            } else {
                bad = 1;
                for (size_t i = 0; i < bytes / sizeof(int); ++i) wire_close_fd(passed[i]);
            }
        } else {
            bad = 1;
        }
        offset += CMSG_ALIGN(entry->cmsg_len);
    }
    if (!bad && credential_count == 1 && message->sender > 0 && message->sender_pidfd >= 0 &&
        message->packet.version == 1 && message->packet.reserved == 0 &&
        message->packet.deadline_ns != 0 && message->packet.step >= PTG220_BIRTH_W &&
        message->packet.step <= last_step) return 1;
    wire_close(message);
    return count == 0 ? -EPIPE : -EPROTO;
}

static inline int wire_receive(int fd, struct ptg220_bootstrap_message *message) {
    return wire_receive_impl(fd, message, NULL, PTG220_W_FAILURE);
}

#endif
