// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#ifndef PTG_TERMINAL_BRIDGE_H
#define PTG_TERMINAL_BRIDGE_H
#include "waitid_bridge.h"

/* Scalar APIs avoid assuming native sigaction/prctl structure layouts in Rust. */
int32_t ptg_terminal_self(uint32_t which);
int32_t ptg_terminal_self_pidfd(int32_t *error);
int32_t ptg_terminal_sigchld(uint32_t *facts, int32_t *error);
int32_t ptg_terminal_subreaper(uint32_t operation, int32_t *value, int32_t *error);
int32_t ptg_terminal_kill(int32_t pidfd, int32_t *error);
uint32_t ptg_terminal_errno_kind(int32_t error);
int32_t ptg_terminal_poll(void *fds, uint32_t count, int32_t timeout, int32_t *error);
void ptg_terminal_wait_any(uint32_t peek, struct ptg_wait_frame *out,
                         unsigned char *native_bytes, uint32_t native_capacity);
/* Fixed nonblocking exec-report read: 0=bytes,1=EOF,2=again,3=interrupted,4=error. */
uint32_t ptg_terminal_exec_read(int32_t fd, unsigned char *bytes, uint32_t capacity,
                              int32_t *count, int32_t *error);
#endif
