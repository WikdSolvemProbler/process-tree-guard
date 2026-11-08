// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#define _GNU_SOURCE 1
#include "entry_timer.h"
#include <errno.h>
#include <limits.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#if !defined(__linux__) || !(defined(__aarch64__) || defined(__x86_64__))
#error "entry timers require Linux LP64 ARM64/x86-64"
#endif
_Static_assert(sizeof(void *)==8 && sizeof(time_t)==8,"entry timer LP64 ABI");
_Static_assert(sizeof(struct ptg_entry_timer_observation)==96
    && offsetof(struct ptg_entry_timer_observation,deadline_ns)==56,
    "entry timer observation ABI");
#define PTG_ENTRY_TIMER_MAGIC UINT32_C(0x50544754)
#define NS_PER_SECOND UINT64_C(1000000000)
struct ptg_entry_timer {
    uint32_t magic;
    int armed,sticky;
    timer_t timer;
    uint64_t deadline_ns;
};
static void initialize(struct ptg_entry_timer_observation *out,uint64_t deadline) {
    memset(out,0,sizeof(*out));
    out->schema=1;out->stage=1;out->result=-1;out->deadline_ns=deadline;
    out->create_result=out->set_result=out->get_result=out->delete_result=INT32_MIN;
}
static int normalized_ns(const struct timespec *value,uint64_t *ns) {
    if(value->tv_sec<0 || value->tv_nsec<0 || value->tv_nsec>=1000000000L
       || (uint64_t)value->tv_sec>(UINT64_MAX-(uint64_t)value->tv_nsec)/NS_PER_SECOND)
        return EOVERFLOW;
    *ns=(uint64_t)value->tv_sec*NS_PER_SECOND+(uint64_t)value->tv_nsec;
    return 0;
}
static int boottime(uint64_t *ns) {
    struct timespec now;
    if(clock_gettime(CLOCK_BOOTTIME,&now)!=0)return errno;
    return normalized_ns(&now,ns);
}
static int valid(const struct ptg_entry_timer *owner) {
    return owner && owner->magic==PTG_ENTRY_TIMER_MAGIC;
}
static void observe(const struct ptg_entry_timer *owner,
    struct ptg_entry_timer_observation *out) {
    if(!valid(owner)){out->call_errno=EINVAL;return;}
    out->created=1;out->armed=owner->armed;out->deadline_ns=owner->deadline_ns;
    int error=boottime(&out->before_ns);
    if(error){out->call_errno=error;return;}
    struct itimerspec state;
    memset(&state,0,sizeof(state));
    out->get_result=timer_gettime(owner->timer,&state);
    out->get_errno=out->get_result<0?errno:0;
    if(out->get_result!=0){out->call_errno=out->get_errno;return;}
    error=normalized_ns(&state.it_value,&out->remaining_ns);
    if(!error)error=normalized_ns(&state.it_interval,&out->interval_ns);
    if(error){out->call_errno=error;return;}
    error=boottime(&out->observed_ns);
    if(error){out->call_errno=error;return;}
    if(!owner->armed || out->interval_ns || out->observed_ns<out->before_ns){
        out->call_errno=EPROTO;return;
    }
    if(!out->remaining_ns || out->observed_ns>=owner->deadline_ns){
        out->call_errno=ETIMEDOUT;return;
    }
    /* gettime's inferred kernel sample must lie within the two same-clock
     * observations. No deadline retarget or caller-controlled timer is accepted.
     */
    if(out->remaining_ns>owner->deadline_ns){
        out->call_errno=EPROTO;return;
    }
    const uint64_t inferred=owner->deadline_ns-out->remaining_ns;
    if(inferred<out->before_ns || inferred>out->observed_ns){out->call_errno=EPROTO;return;}
    out->stage=4;out->result=0;
}
static void create(uint64_t deadline,struct ptg_entry_timer **returned,
    struct ptg_entry_timer_observation *out,int sticky) {
    if(!out)return;
    initialize(out,deadline);
    if(!returned || *returned || !deadline){out->call_errno=EINVAL;return;}
    if(getpid()<=1){out->call_errno=EPERM;return;}
    int error=boottime(&out->observed_ns);
    if(error){out->call_errno=error;return;}
    if(out->observed_ns>=deadline){out->call_errno=ETIMEDOUT;return;}
    struct ptg_entry_timer *owner=calloc(1,sizeof(*owner));
    if(!owner){out->call_errno=ENOMEM;return;}
    owner->magic=PTG_ENTRY_TIMER_MAGIC;owner->deadline_ns=deadline;owner->sticky=sticky;
    struct sigevent notification;
    memset(&notification,0,sizeof(notification));
    notification.sigev_notify=SIGEV_SIGNAL;notification.sigev_signo=SIGKILL;
    out->create_result=timer_create(CLOCK_BOOTTIME,&notification,&owner->timer);
    out->create_errno=out->create_result<0?errno:0;
    if(out->create_result!=0){
        out->call_errno=out->create_errno;owner->magic=0;free(owner);return;
    }
    /* Ownership is observable even if every following operation fails. */
    *returned=owner;out->created=1;out->stage=2;
    struct itimerspec setting;
    memset(&setting,0,sizeof(setting));
    setting.it_value.tv_sec=(time_t)(deadline/NS_PER_SECOND);
    setting.it_value.tv_nsec=(long)(deadline%NS_PER_SECOND);
    out->set_result=timer_settime(owner->timer,TIMER_ABSTIME,&setting,NULL);
    out->set_errno=out->set_result<0?errno:0;
    if(out->set_result!=0){out->call_errno=out->set_errno;return;}
    owner->armed=1;out->armed=1;out->stage=3;
    observe(owner,out);
}
void ptg_entry_timer_create(uint64_t deadline,struct ptg_entry_timer **owner,
    struct ptg_entry_timer_observation *out) {create(deadline,owner,out,0);}
void ptg_entry_cleanup_timer_create(uint64_t deadline,struct ptg_entry_timer **owner,
    struct ptg_entry_timer_observation *out) {create(deadline,owner,out,1);}
void ptg_entry_timer_observe(const struct ptg_entry_timer *owner,
    struct ptg_entry_timer_observation *out) {
    if(!out)return;
    initialize(out,valid(owner)?owner->deadline_ns:0);
    observe(owner,out);
}
void ptg_entry_timer_delete(struct ptg_entry_timer *owner,
    struct ptg_entry_timer_observation *out) {
    if(!out)return;
    initialize(out,valid(owner)?owner->deadline_ns:0);
    if(!valid(owner)){out->call_errno=EINVAL;return;}
    out->created=1;out->armed=owner->armed;
    if(owner->sticky){out->call_errno=EPERM;return;}
    out->delete_result=timer_delete(owner->timer);
    out->delete_errno=out->delete_result<0?errno:0;
    if(out->delete_result!=0){out->call_errno=out->delete_errno;return;}
    out->stage=5;out->result=0;out->armed=0;owner->magic=0;free(owner);
}
uint64_t ptg_entry_timer_abi(uint32_t word) {
    switch(word) {
    case 0:return 1;
    case 1:return sizeof(struct ptg_entry_timer_observation);
    case 2:return CLOCK_BOOTTIME;
    case 3:return SIGKILL;
    case 4:return SIGEV_SIGNAL;
    case 5:return TIMER_ABSTIME;
    case 6:return sizeof(struct sigevent);
    case 7:return sizeof(struct itimerspec);
    case 8:return offsetof(struct ptg_entry_timer_observation,deadline_ns);
    default:return UINT64_MAX;
    }
}
