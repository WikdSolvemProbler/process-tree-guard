// SPDX-License-Identifier: Apache-2.0
#ifndef PTG220_NATIVE_CONTROL_H
#define PTG220_NATIVE_CONTROL_H

#include <poll.h>
#include <stdint.h>
#include <sys/types.h>
#include "waitid_bridge.h"

#define PTG220_SETTLEMENT_NS UINT64_C(5000000000)

struct ptg220_birth {
    pid_t pid;
    int pidfd;
};

struct ptg220_wait_record {
    struct ptg_wait_frame frame;
    unsigned char siginfo[128];
    uint64_t before_ns;
    uint64_t after_ns;
    int32_t before_error;
    int32_t after_error;
    int32_t expected_pid;
    int32_t native_options;
};

_Static_assert(sizeof(struct ptg220_wait_record) == 200, "private original wait record layout");

enum ptg220_timer_state { PTG220_TIMER_EMPTY, PTG220_TIMER_ARMED, PTG220_TIMER_UNCERTAIN };

struct ptg220_terminal_timer {
    int native_id;
    enum ptg220_timer_state state;
    uint64_t cutoff_ns;
    int disposal_error;
};

#define PTG220_TERMINAL_TIMER_INIT {.native_id = -1, .state = PTG220_TIMER_EMPTY}

/* All control operations except the inherited wait bridge return negative errno. */
int ptg220_boottime(uint64_t *now_ns);
int ptg220_validate_invocation_deadline(uint64_t deadline_ns);
int ptg220_deadline_fd(uint64_t deadline_ns);
int ptg220_poll(struct pollfd *fds, unsigned int count, int block);
int ptg220_pidfd_terminal(int pidfd);
int ptg220_open_self_pidfd(void);
int ptg220_signal_original(int pidfd);
int ptg220_arm_parent_death(pid_t parent, int parent_pidfd, uint64_t deadline_ns);
int ptg220_disarm_parent_death(pid_t parent, int parent_pidfd, uint64_t deadline_ns);
int ptg220_clone_controller(struct ptg220_birth *birth);
int ptg220_clone_payload(int held_leaf, struct ptg220_birth *birth);
int ptg220_wait_original(const struct ptg220_birth *birth, struct ptg_wait_frame *frame,
                         unsigned char evidence[PTG_WAIT_NATIVE_CAPACITY]);
int ptg220_peek_original(const struct ptg220_birth *birth, struct ptg_wait_frame *frame,
                         unsigned char evidence[PTG_WAIT_NATIVE_CAPACITY]);
/* Exclusive original subreaper only. A positive peek never consumes a status. */
int ptg220_wait_adopted(int peek, struct ptg_wait_frame *frame,
                        unsigned char evidence[PTG_WAIT_NATIVE_CAPACITY]);
int ptg220_record_exclusive_reap(struct ptg220_wait_record *record);
int ptg220_record_original_peek(const struct ptg220_birth *birth,
                               struct ptg220_wait_record *record);
int ptg220_record_original_reap(const struct ptg220_birth *birth,
                               struct ptg220_wait_record *record);
int ptg220_original_terminal(const struct ptg220_birth *birth,
                             const struct ptg220_wait_record *record);
int ptg220_join_original(const struct ptg220_birth *birth, uint64_t cutoff,
                         struct ptg220_wait_record *record);

/* Call only after borrowing nothing whose retirement still needs this actor. */
int ptg220_arm_terminal_timer(struct ptg220_terminal_timer *timer, uint64_t cutoff_ns);
int ptg220_tighten_terminal_timer(struct ptg220_terminal_timer *timer, uint64_t cutoff_ns);
int ptg220_delete_terminal_timer(struct ptg220_terminal_timer *timer);

#endif
