// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#define _GNU_SOURCE 1
#include "native_arming.h"
#include "payload_bootstrap.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/sched.h>
#include <poll.h>
#include <signal.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

_Static_assert(sizeof(pid_t)==4 && (pid_t)-1<0,"Linux PID scalar");
_Static_assert(sizeof(struct clone_args)==88,"installed clone3 structure");
_Static_assert(offsetof(struct clone_args,pidfd)==8,"installed pidfd offset");
_Static_assert(sizeof(struct pollfd)==8,"installed pollfd shape");
#if !defined(__linux__) || !(defined(__aarch64__) || defined(__x86_64__))
#error "raw kernel mask requires Linux LP64 ARM64/x86-64"
#endif
_Static_assert(sizeof(unsigned long)==8 && sizeof(uint64_t)==8,"native64-bit kernel signal word");
_Static_assert(SIG_SETMASK==2 && SIGKILL==9 && SIGSTOP==19,"native signal numbers");
#define PTG_BLOCKED_MASK (UINT64_MAX & ~(UINT64_C(1)<<(SIGKILL-1)) & ~(UINT64_C(1)<<(SIGSTOP-1)))
struct ready_message {uint32_t schema,token;uint64_t mask;};
_Static_assert(sizeof(struct ready_message)==16 && offsetof(struct ready_message,mask)==8,"readiness includes actual child mask readback");
static int kernel_abi_ok(void) {
    return ptg_arm_kernel_signal_abi(0)==8 && ptg_arm_kernel_signal_abi(1)==64
        && ptg_arm_kernel_signal_abi(2)==SIG_SETMASK
        && ptg_arm_kernel_signal_abi(3)==SYS_rt_sigprocmask
        && ptg_arm_kernel_signal_abi(4)==SIGKILL && ptg_arm_kernel_signal_abi(5)==SIGSTOP
        && ptg_arm_kernel_signal_abi(6)==sizeof(unsigned long);
}
static int kernel_mask(const uint64_t *set,uint64_t *old) {
    return (int)syscall(SYS_rt_sigprocmask,SIG_SETMASK,set,old,sizeof(uint64_t));
}
_Static_assert(sizeof(struct ptg_arm_spawn)==128 && offsetof(struct ptg_arm_spawn,original_signal_mask)==56,"spawn bridge ABI");
_Static_assert(sizeof(struct ptg_arm_ready)==104 && offsetof(struct ptg_arm_ready,before_ns)==72,"readiness bridge ABI");
_Static_assert(sizeof(struct ptg_arm_kill)==8,"kill bridge ABI");
_Static_assert(offsetof(struct ptg_arm_spawn,blocked_mask)==112,"kernel mask observation offset");
_Static_assert(offsetof(struct ptg_arm_ready,child_mask)==96,"child mask observation offset");

uint64_t ptg_arm_abi(uint32_t word) {
    switch(word) {
    case 0:return 2;
    case 1:return sizeof(struct ptg_arm_spawn);
    case 2:return sizeof(struct ptg_arm_ready);
    case 3:return sizeof(struct ptg_arm_kill);
    case 4:return CLONE_PIDFD;
    case 5:return SIGCHLD;
    case 6:return SYS_clone3;
    case 7:return CLOCK_BOOTTIME;
    case 8:return sizeof(struct pollfd);
    case 9:return ptg_arm_kernel_signal_abi(0);
    case 10:return ptg_arm_kernel_signal_abi(1);
    case 11:return ptg_arm_kernel_signal_abi(2);
    case 12:return ptg_arm_kernel_signal_abi(3);
    case 13:return ptg_arm_kernel_signal_abi(4);
    case 14:return ptg_arm_kernel_signal_abi(5);
    case 15:return ptg_arm_kernel_signal_abi(6);
    case 16:return PTG_BLOCKED_MASK;
    default:return UINT64_MAX;
    }
}
static uint64_t now_ns(int *error) {
    struct timespec now;
    if(clock_gettime(CLOCK_BOOTTIME,&now)!=0){*error=errno;return 0;}
    if(now.tv_sec<0 || now.tv_nsec<0 || now.tv_nsec>=1000000000L
       || (uint64_t)now.tv_sec>(UINT64_MAX-(uint64_t)now.tv_nsec)/UINT64_C(1000000000)){
        *error=EOVERFLOW;return 0;
    }
    *error=0;return (uint64_t)now.tv_sec*UINT64_C(1000000000)+(uint64_t)now.tv_nsec;
}
void ptg_arm_kill_held(int32_t fd,struct ptg_arm_kill *out) {
    // First action is the held write. No clock, allocation, formatting or reopen.
    const ssize_t result=syscall(SYS_write,fd,"1",1);
    out->result=(int32_t)result;
    out->call_errno=result<0?errno:0;
}
static _Noreturn void cancel_exit(struct ptg_payload_gate *gate,int kill_fd) {
    (void)ptg_payload_gate_cancel(gate);
    const ssize_t result=syscall(SYS_write,kill_fd,"1",1);
    _exit(result==1?124:125); // attempt result only; never an empty-domain receipt.
}
static _Noreturn void watchdog_child(struct ptg_payload_gate *gate,int owner,int timer,int supervisor,int kill_fd,int ready) {
    // Raw kernel readback BEFORE clone covered signals1..64 except KILL/STOP,
    // including the two signals libc reserves. Recheck this child's inheritance.
    // No libc cancellation-point wrappers, Rust, heap, stdio or exec path.
    uint64_t inherited_mask=0;
    if(kernel_mask(NULL,&inherited_mask)!=0 || inherited_mask!=PTG_BLOCKED_MASK)cancel_exit(gate,kill_fd);
    const int source[5]={owner,timer,supervisor,kill_fd,ready};
    int staged[5]={-1,-1,-1,-1,-1};
    for(unsigned i=0;i<5;i++) {
        staged[i]=(int)syscall(SYS_fcntl,source[i],F_DUPFD_CLOEXEC,64);
        if(staged[i]<0)cancel_exit(gate,staged[3]>=0?staged[3]:kill_fd);
    }
    for(unsigned i=0;i<5;i++)if(syscall(SYS_dup3,staged[i],(int)i+3,O_CLOEXEC)<0)cancel_exit(gate,staged[3]);
    // Exact child-local retained layout:3=owner,4=original timer,5=exact
    // supervisor pidfd,6=held cgroup.kill,7=sole readiness writer.
    if(syscall(SYS_close_range,8U,UINT_MAX,0U)!=0)cancel_exit(gate,6);
    if(syscall(SYS_close_range,0U,2U,0U)!=0)cancel_exit(gate,6);
    struct pollfd watched[3]={{.fd=3,.events=POLLIN},{.fd=4,.events=POLLIN},{.fd=5,.events=POLLIN}};
    struct timespec zero={0,0};
    const int initial=(int)syscall(SYS_ppoll,watched,3,&zero,NULL,sizeof(uint64_t));
    if(initial!=0 || watched[0].revents || watched[1].revents || watched[2].revents)cancel_exit(gate,6);
    uint64_t ready_mask=0;
    if(kernel_mask(NULL,&ready_mask)!=0 || ready_mask!=PTG_BLOCKED_MASK)cancel_exit(gate,6);
    const struct ready_message message={.schema=2,.token='R',.mask=ready_mask};
    if(syscall(SYS_write,7,&message,sizeof(message))!=(long)sizeof(message))cancel_exit(gate,6);
    if(syscall(SYS_close,7)!=0)cancel_exit(gate,6);
    // Never read/disarm/retarget the execution timer. Any owner/supervisor
    // readiness, expiry, unexpected poll state or EINTR cancels immediately.
    // The original finite BOOTTIME timer bounds the otherwise blocking poll.
    for(unsigned i=0;i<3;i++)watched[i].revents=0;
    (void)syscall(SYS_ppoll,watched,3,NULL,NULL,sizeof(uint64_t));
    cancel_exit(gate,6);
}
static void failure_kill(struct ptg_payload_gate *gate,int fd,struct ptg_arm_spawn *out) {
    (void)ptg_payload_gate_cancel(gate);
    if(out->failure_kill_result!=INT32_MIN)return;
    const long result=syscall(SYS_write,fd,"1",1);
    out->failure_kill_result=(int32_t)result;out->failure_kill_errno=result<0?errno:0;
}
static void restore_parent_mask(struct ptg_payload_gate *gate,int kill_fd,struct ptg_arm_spawn *out) {
    out->mask_restore_result=kernel_mask(&out->original_signal_mask,NULL);
    out->mask_restore_errno=out->mask_restore_result<0?errno:0;
    if(out->mask_restore_result!=0)failure_kill(gate,kill_fd,out);
    out->mask_restore_query_result=kernel_mask(NULL,&out->restored_mask);
    out->mask_restore_query_errno=out->mask_restore_query_result<0?errno:0;
    if(out->mask_restore_query_result!=0 || out->restored_mask!=out->original_signal_mask)
        failure_kill(gate,kill_fd,out);
}
void ptg_arm_spawn_watchdog(struct ptg_payload_gate *gate,int32_t owner,int32_t timer,int32_t supervisor,
    int32_t kill_fd,int32_t ready,uint64_t until,struct ptg_arm_spawn *out) {
    memset(out,0,sizeof(*out));out->schema=2;out->pid=out->pidfd=-1;
    out->mask_restore_result=out->initial_poll_result=out->clone_result=INT32_MIN;
    out->mask_set_result=out->mask_query_result=out->mask_restore_query_result=INT32_MIN;
    out->failure_kill_result=INT32_MIN;out->stage=1;
    if(!kernel_abi_ok()){out->call_errno=EPROTO;return;}
    if(gate==NULL||ptg_payload_gate_state(gate)!=PTG_PAYLOAD_HELD||owner<0||timer<0||supervisor<0||kill_fd<0||ready<0){out->call_errno=EBADF;return;}
    struct pollfd initial[3]={{.fd=owner,.events=POLLIN},{.fd=timer,.events=POLLIN},{.fd=supervisor,.events=POLLIN}};
    const int sampled=poll(initial,3,0);out->initial_poll_result=sampled;
    for(unsigned i=0;i<3;i++)out->initial_revents[i]=initial[i].revents;
    if(sampled!=0){out->call_errno=sampled<0?errno:ECANCELED;return;}
    int error=0;out->before_clone_ns=now_ns(&error);
    if(error||out->before_clone_ns>=until){out->call_errno=error?error:ETIMEDOUT;return;}
    const uint64_t block=PTG_BLOCKED_MASK;
    // Atomic kernel exchange captures the exact original kernel mask. No libc
    // sigset_t, sigfillset, pthread_sigmask or NPTL filtering is involved.
    out->mask_set_result=kernel_mask(&block,&out->original_signal_mask);
    out->mask_set_errno=out->mask_set_result<0?errno:0;
    if(out->mask_set_result!=0){out->call_errno=out->mask_set_errno;failure_kill(gate,kill_fd,out);return;}
    out->mask_query_result=kernel_mask(NULL,&out->blocked_mask);
    out->mask_query_errno=out->mask_query_result<0?errno:0;
    if(out->mask_query_result!=0 || out->blocked_mask!=block){
        out->call_errno=out->mask_query_errno?out->mask_query_errno:EPROTO;
        failure_kill(gate,kill_fd,out);restore_parent_mask(gate,kill_fd,out);return;
    }
    struct clone_args args;memset(&args,0,sizeof(args));
    args.flags=CLONE_PIDFD;args.pidfd=(uint64_t)(uintptr_t)&out->pidfd;args.exit_signal=SIGCHLD;
    out->stage=2;
    const long result=syscall(SYS_clone3,&args,sizeof(args));
    if(result==0)watchdog_child(gate,owner,timer,supervisor,kill_fd,ready);
    const int clone_error=result<0?errno:0;out->clone_result=(int32_t)result;
    // Publish positive-clone custody BEFORE restoration or clock operations.
    if(result>0){out->pid=(int32_t)result;out->child_created=1;}
    if(result<0)failure_kill(gate,kill_fd,out);
    restore_parent_mask(gate,kill_fd,out);
    if(result<0){out->call_errno=clone_error;return;}
    if(out->mask_restore_result!=0 || out->mask_restore_query_result!=0
       || out->restored_mask!=out->original_signal_mask){
        out->call_errno=out->mask_restore_errno?out->mask_restore_errno:
            (out->mask_restore_query_errno?out->mask_restore_query_errno:EPROTO);return;
    }
    out->stage=3;out->after_clone_ns=now_ns(&error);out->call_errno=error;
    if(error){failure_kill(gate,kill_fd,out);return;}
    out->stage=4;
}
void ptg_arm_wait_ready(int32_t ready,int32_t child,int32_t owner,int32_t timer,
    int32_t supervisor,int32_t setup_timer,uint64_t until,struct ptg_arm_ready *out) {
    memset(out,0,sizeof(*out));out->result=-1;out->read_count=-1;out->poll_result=out->after_poll_result=INT32_MIN;
    int error=0;out->before_ns=now_ns(&error);
    if(error||out->before_ns>=until){out->call_errno=error?error:ETIMEDOUT;return;}
    struct pollfd watched[6]={{.fd=ready,.events=POLLIN},{.fd=child,.events=POLLIN},
        {.fd=owner,.events=POLLIN},{.fd=timer,.events=POLLIN},{.fd=supervisor,.events=POLLIN},
        {.fd=setup_timer,.events=POLLIN}};
    int result=poll(watched,6,-1);out->poll_result=result;
    for(unsigned i=0;i<6;i++)out->revents[i]=watched[i].revents;
    if(result<=0){out->call_errno=result<0?errno:EIO;return;}
    for(unsigned i=1;i<6;i++)if(watched[i].revents){out->call_errno=ECANCELED;return;}
    if(!(watched[0].revents&POLLIN)||watched[0].revents&~(POLLIN|POLLHUP)){out->call_errno=EPROTO;return;}
    struct ready_message message;memset(&message,0,sizeof(message));
    const ssize_t count=read(ready,&message,sizeof(message));
    out->read_count=(int32_t)count;out->token=message.token;
    out->message_schema=message.schema;out->child_mask=message.mask;
    if(count!=(ssize_t)sizeof(message)||message.schema!=2||message.token!='R'||message.mask!=PTG_BLOCKED_MASK){
        out->call_errno=count<0?errno:EPROTO;return;
    }
    for(unsigned i=0;i<6;i++)watched[i].revents=0;
    result=poll(watched,6,0);out->after_poll_result=result;
    for(unsigned i=0;i<6;i++)out->after_revents[i]=watched[i].revents;
    if(result<0){out->call_errno=errno;return;}
    for(unsigned i=1;i<6;i++)if(watched[i].revents){out->call_errno=ECANCELED;return;}
    out->after_ns=now_ns(&error);
    if(error||out->after_ns<out->before_ns||out->after_ns>=until){out->call_errno=error?error:ETIMEDOUT;return;}
    out->result=0;
}
