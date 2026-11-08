// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#ifndef PTG_OWNER_POLL_H
#define PTG_OWNER_POLL_H
// Keep libc poll/time helpers out of the isolated kernel-signal ABI headers.
#include "owner_watch.h"
#include <errno.h>
#include <poll.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

static inline int ptg_owner_watch_valid(const struct ptg_owner_watch *owners) {
    if(!owners || !owners->fds || owners->reserved
       || owners->count<PTG_OWNER_WATCH_MIN || owners->count>PTG_OWNER_WATCH_MAX)return 0;
    for(uint32_t i=0;i<owners->count;i++) {
        if(owners->fds[i]<0)return 0;
        for(uint32_t j=0;j<i;j++)if(owners->fds[i]==owners->fds[j])return 0;
    }
    return 1;
}
static inline int ptg_owner_watch_disjoint(const struct ptg_owner_watch *owners,
    const int32_t *other,uint32_t count) {
    if(!ptg_owner_watch_valid(owners) || (count && !other))return 0;
    for(uint32_t i=0;i<count;i++) {
        if(other[i]<0)return 0;
        for(uint32_t j=0;j<owners->count;j++)if(other[i]==owners->fds[j])return 0;
    }
    return 1;
}

/* Expand one logical owner slot into EVERY original pidfd. The receipt's owner
 * event is their bitwise OR, never a sample of just one owner. Any owner event
 * remains a refusal. All other slots retain their original event observations.
 * Uses bounded C stack and the raw syscall, including in post-clone children.
 * It never reads, consumes, rearms or replaces any pidfd or original timer.
 */
static inline int ptg_owner_poll(const struct ptg_owner_watch *owners,
    struct pollfd *logical,uint32_t count,uint32_t owner_slot,int block) {
    if(!logical || !count || count>7 || owner_slot>=count){errno=EINVAL;return -1;}
    for(uint32_t i=0;i<count;i++)logical[i].revents=0;
    if(!ptg_owner_watch_valid(owners)){errno=EINVAL;return -1;}
    struct pollfd expanded[PTG_OWNER_WATCH_MAX+6];
    uint32_t n=0;
    for(uint32_t i=0;i<count;i++) {
        if(i==owner_slot) {
            for(uint32_t j=0;j<owners->count;j++)
                expanded[n++]=(struct pollfd){.fd=owners->fds[j],.events=POLLIN};
        } else {
            if(logical[i].fd<0){errno=EBADF;return -1;}
            for(uint32_t j=0;j<owners->count;j++)
                if(logical[i].fd==owners->fds[j]){errno=EINVAL;return -1;}
            expanded[n]=logical[i];expanded[n++].revents=0;
        }
    }
    struct timespec zero={0,0};
    const int result=(int)syscall(SYS_ppoll,expanded,n,block?NULL:&zero,NULL,sizeof(uint64_t));
    n=0;
    for(uint32_t i=0;i<count;i++) {
        if(i==owner_slot) {
            for(uint32_t j=0;j<owners->count;j++)logical[i].revents|=expanded[n++].revents;
        } else logical[i].revents=expanded[n++].revents;
    }
    return result;
}
#endif
