// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
#define _GNU_SOURCE 1
#include "payload_bootstrap.h"
#include "owner_poll.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/capability.h>
#include <linux/sched.h>
#include <poll.h>
#include <signal.h>
#include <stdatomic.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#if !defined(__linux__) || !(defined(__aarch64__) || defined(__x86_64__))
#error "payload bootstrap requires Linux LP64 aarch64/x86-64"
#endif
#if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error "payload readiness ABI currently requires little endian"
#endif
_Static_assert(sizeof(void *)==8 && sizeof(unsigned long)==8,"LP64 private ABI");
_Static_assert(sizeof(struct clone_args)==88 && offsetof(struct clone_args,cgroup)==80,"clone3 v2 ABI");
_Static_assert(sizeof(struct pollfd)==8,"native pollfd ABI");
_Static_assert(sizeof(struct ptg_payload_spec)==160,"payload spec ABI");
_Static_assert(offsetof(struct ptg_payload_spec,gate)==144,"payload gate offset");
_Static_assert(offsetof(struct ptg_payload_spec,owners)==16,"payload original owner-set offset");
_Static_assert(offsetof(struct ptg_payload_spec,umask)==92,"explicit caller umask offset");
_Static_assert(offsetof(struct ptg_payload_spec,host_dumpability)==152
    && offsetof(struct ptg_payload_spec,core_reserved)==156,"trusted core policy offsets");
struct ptg_core_limits {uint64_t soft,hard;};
_Static_assert(sizeof(struct ptg_core_limits)==16
    && offsetof(struct ptg_core_limits,soft)==0
    && offsetof(struct ptg_core_limits,hard)==8,"Linux prlimit64 tuple ABI");
_Static_assert(RLIMIT_CORE==4,"Linux core resource ABI");
_Static_assert(sizeof(struct ptg_payload_spawn)==128,"payload spawn ABI");
_Static_assert(sizeof(struct ptg_payload_ready)==96,"payload readiness ABI");
_Static_assert(sizeof(struct ptg_payload_release)==80,"payload release ABI");
_Static_assert(sizeof(uid_t)==4 && sizeof(gid_t)==4 && sizeof(pid_t)==4,"Linux credential/PID scalars");
_Static_assert(ATOMIC_INT_LOCK_FREE==2 && sizeof(unsigned int)==4,"gate atomics must never call a lock helper");
_Static_assert(SIGCHLD==17 && SIGKILL==9 && SIGSTOP==19 && SIG_SETMASK==2,"raw signal constants");
#define PTG_BLOCKED_MASK (UINT64_MAX & ~(UINT64_C(1)<<(SIGKILL-1)) & ~(UINT64_C(1)<<(SIGSTOP-1)))
#define PTG_GATE_MAGIC UINT32_C(0x50475447)
struct ptg_payload_gate {uint32_t magic; _Atomic unsigned int state;};
_Static_assert(sizeof(struct ptg_payload_gate)==8
    && offsetof(struct ptg_payload_gate,state)==4,"opaque shared gate layout");
struct ready_message {uint32_t schema,token;uint64_t mask;};
_Static_assert(sizeof(struct ready_message)==16,"ready wire ABI");
extern uint64_t ptg_arm_kernel_signal_abi(uint32_t word);

uint64_t ptg_payload_abi(uint32_t word) {
    switch(word) {
    case 0:return PTG_PAYLOAD_SCHEMA;
    case 1:return sizeof(struct ptg_payload_spec);
    case 2:return sizeof(struct ptg_payload_spawn);
    case 3:return sizeof(struct ptg_payload_ready);
    case 4:return sizeof(struct ptg_payload_release);
    case 5:return sizeof(struct ready_message);
    case 6:return 8;
    case 7:return CLONE_PIDFD|CLONE_INTO_CGROUP;
    case 8:return SIGCHLD;
    case 9:return SYS_clone3;
    case 10:return CLOCK_BOOTTIME;
    case 11:return PTG_BLOCKED_MASK;
    case 12:return offsetof(struct ptg_payload_spec,gate);
    case 13:return sizeof(struct ptg_owner_watch);
    case 14:return PTG_OWNER_WATCH_MAX;
    case 15:return offsetof(struct ptg_payload_spec,owners);
    case 16:return offsetof(struct ptg_payload_spec,umask);
    case 17:return UINT32_C(0777);
    case 18:return offsetof(struct ptg_payload_spec,host_dumpability);
    case 19:return offsetof(struct ptg_payload_spec,core_reserved);
    case 20:return UINT32_C(5);
    case 21:return sizeof(struct ptg_core_limits);
    case 22:return offsetof(struct ptg_core_limits,soft);
    case 23:return offsetof(struct ptg_core_limits,hard);
    case 24:return RLIMIT_CORE;
    case 25:return SYS_prlimit64;
    case 26:return 1;
    default:return UINT64_MAX;
    }
}
static int gate_valid(const struct ptg_payload_gate *g) {return g && g->magic==PTG_GATE_MAGIC;}
struct ptg_payload_gate *ptg_payload_gate_create(int32_t *error) {
    void *p=mmap(NULL,sizeof(struct ptg_payload_gate),PROT_READ|PROT_WRITE,
                 MAP_SHARED|MAP_ANONYMOUS,-1,0);
    if(p==MAP_FAILED){if(error)*error=errno;return NULL;}
    struct ptg_payload_gate *g=p;g->magic=PTG_GATE_MAGIC;
    atomic_init(&g->state,PTG_PAYLOAD_HELD);
    if(error)*error=0;
    return g;
}
int32_t ptg_payload_gate_destroy(struct ptg_payload_gate *g,int32_t *error) {
    if(!gate_valid(g)){if(error)*error=EINVAL;return -1;}
    /* No state change first: failed unmap leaves the exact owner usable. */
    const int r=munmap(g,sizeof(*g));if(error)*error=r<0?errno:0;return r;
}
static int32_t gate_logical(unsigned int state) {
    switch(state) {
    case PTG_PAYLOAD_HELD:case PTG_PAYLOAD_RELEASED:case PTG_PAYLOAD_CANCELLED:
        return (int32_t)state;
    case PTG_PAYLOAD_CANCELLED|PTG_PAYLOAD_PEER_CANCEL_FLAG:
        return PTG_PAYLOAD_CANCELLED;
    default:return -1;
    }
}
int32_t ptg_payload_gate_state(const struct ptg_payload_gate *g) {
    return gate_valid(g)?gate_logical(atomic_load_explicit(&g->state,memory_order_acquire)):-1;
}
static int32_t gate_cancel(struct ptg_payload_gate *g,unsigned int peer) {
    if(!gate_valid(g))return -1;
    unsigned int observed=atomic_load_explicit(&g->state,memory_order_acquire);
    for(;;) {
        const int32_t previous=gate_logical(observed);
        if(previous<0 || previous==PTG_PAYLOAD_CANCELLED)return previous;
        const unsigned int desired=PTG_PAYLOAD_CANCELLED
            | (peer?PTG_PAYLOAD_PEER_CANCEL_FLAG:0U);
        if(atomic_compare_exchange_strong_explicit(&g->state,&observed,desired,
                memory_order_acq_rel,memory_order_acquire))return previous;
        /* The only valid transitions are HELD->RELEASED->CANCELLED (or direct
         * cancellation). A failed strong CAS therefore cannot spin through
         * repeated cancellation writes or relabel a prior cause. */
    }
}
int32_t ptg_payload_gate_cancel(struct ptg_payload_gate *g) {return gate_cancel(g,0);}
int32_t ptg_payload_gate_cancel_peer(struct ptg_payload_gate *g) {return gate_cancel(g,1);}
int32_t ptg_payload_gate_peer_cancelled(const struct ptg_payload_gate *g) {
    if(!gate_valid(g))return -1;
    const unsigned int value=atomic_load_explicit(&g->state,memory_order_acquire);
    if(gate_logical(value)<0)return -1;
    return value==(PTG_PAYLOAD_CANCELLED|PTG_PAYLOAD_PEER_CANCEL_FLAG)?1:0;
}
int32_t ptg_payload_gate_commit(struct ptg_payload_gate *g) {
    if(!gate_valid(g))return -1;
    unsigned int expected=PTG_PAYLOAD_HELD;
    if(atomic_compare_exchange_strong_explicit(&g->state,&expected,PTG_PAYLOAD_RELEASED,
            memory_order_acq_rel,memory_order_acquire))return PTG_PAYLOAD_HELD;
    return gate_logical(expected);
}
static int kernel_abi_ok(void) {
    return ptg_arm_kernel_signal_abi(0)==8 && ptg_arm_kernel_signal_abi(1)==64
        && ptg_arm_kernel_signal_abi(2)==SIG_SETMASK
        && ptg_arm_kernel_signal_abi(3)==SYS_rt_sigprocmask
        && ptg_arm_kernel_signal_abi(4)==SIGKILL && ptg_arm_kernel_signal_abi(5)==SIGSTOP
        && ptg_arm_kernel_signal_abi(6)==sizeof(unsigned long);
}
static int raw_mask(const uint64_t *set,uint64_t *old) {
    return (int)syscall(SYS_rt_sigprocmask,SIG_SETMASK,set,old,sizeof(uint64_t));
}
static uint64_t now_ns(int *error) {
    struct timespec t;
    if(syscall(SYS_clock_gettime,CLOCK_BOOTTIME,&t)!=0){*error=errno;return 0;}
    if(t.tv_sec<0 || t.tv_nsec<0 || t.tv_nsec>=1000000000L
       || (uint64_t)t.tv_sec>(UINT64_MAX-(uint64_t)t.tv_nsec)/UINT64_C(1000000000)){
        *error=EOVERFLOW;return 0;
    }
    *error=0;return (uint64_t)t.tv_sec*UINT64_C(1000000000)+(uint64_t)t.tv_nsec;
}
static void cancel_kill(const struct ptg_payload_spec *s,int32_t *result,int32_t *error) {
    /* Irreversible shared cancellation always precedes this held kill write. */
    if(s) (void)ptg_payload_gate_cancel(s->gate);
    if(!s || s->kill_fd<0){*result=-1;*error=EBADF;return;}
    const long r=syscall(SYS_write,s->kill_fd,"1",1);
    *result=(int32_t)r;*error=r<0?errno:0;
}
static int vector_bytes(const char *const *v,uint32_t count,size_t *remaining) {
    if(!v || count>PTG_PAYLOAD_MAX_VECTOR || v[count]!=NULL)return EINVAL;
    for(uint32_t i=0;i<count;i++) {
        if(!v[i])return EINVAL;
        size_t n=0;while(n<*remaining && v[i][n]!=0)n++;
        if(n==*remaining)return E2BIG;
        *remaining-=n+1;
    }
    return 0;
}
static int nonblocking_pipe(int fd,int access) {
    struct stat st;
    if(fstat(fd,&st)!=0)return errno;
    if(!S_ISFIFO(st.st_mode))return EINVAL;
    int flags=fcntl(fd,F_GETFL);if(flags<0)return errno;
    if((flags&O_ACCMODE)!=access || !(flags&O_NONBLOCK))return EINVAL;
    flags=fcntl(fd,F_GETFD);if(flags<0)return errno;
    return flags&FD_CLOEXEC?0:EINVAL;
}
/* Permit uses a connected private SEQPACKET socket, never a FIFO: MSG_NOSIGNAL
 * on the parent send preserves EPIPE as an ordinary recorded refusal. */
static int nonblocking_permit(int fd) {
    struct stat st;if(fstat(fd,&st)!=0)return errno;
    if(!S_ISSOCK(st.st_mode))return EINVAL;
    int flags=fcntl(fd,F_GETFL);if(flags<0)return errno;
    if(!(flags&O_NONBLOCK))return EINVAL;
    flags=fcntl(fd,F_GETFD);if(flags<0)return errno;
    if(!(flags&FD_CLOEXEC))return EINVAL;
    const int options[3]={SO_DOMAIN,SO_TYPE,SO_PROTOCOL};
    const int expected[3]={AF_UNIX,SOCK_SEQPACKET,0};
    for(unsigned int i=0;i<3;i++){
        int value=-1;socklen_t length=sizeof(value);
        if(getsockopt(fd,SOL_SOCKET,options[i],&value,&length)!=0)return errno;
        if(length!=sizeof(value) || value!=expected[i])return EINVAL;
    }
    struct sockaddr peer;socklen_t length=sizeof(peer);
    if(getpeername(fd,&peer,&length)!=0)return errno;
    return length>=sizeof(peer.sa_family) && peer.sa_family==AF_UNIX?0:EINVAL;
}
static int watch_spec_valid(const struct ptg_payload_spec *s) {
    if(!s || s->schema!=PTG_PAYLOAD_SCHEMA || s->bytes!=sizeof(*s)
       || !gate_valid(s->gate) || !s->deadline_ns || (s->umask&~UINT32_C(0777))
       || (s->host_dumpability!=0 && s->host_dumpability!=2) || s->core_reserved)return EINVAL;
    const int32_t other[4]={s->kill_fd,s->timer_fd,s->supervisor_fd,s->watchdog_fd};
    return ptg_owner_watch_disjoint(&s->owners,other,4)?0:EINVAL;
}
static int spec_valid(const struct ptg_payload_spec *s) {
    if(watch_spec_valid(s) || s->uid==0 || s->gid==0
       || s->uid==UINT32_MAX || s->gid==UINT32_MAX || s->deadline_ns==0
       || s->group_count>PTG_PAYLOAD_MAX_GROUPS || (s->group_count && !s->groups)
       || s->argc==0 || !s->program || s->program[0]==0)return EINVAL;
    const int32_t controls[8]={s->cgroup_fd,s->kill_fd,s->timer_fd,
        s->supervisor_fd,s->watchdog_fd,s->permit_reader,s->ready_writer,s->exec_writer};
    if(!ptg_owner_watch_disjoint(&s->owners,controls,8))return EINVAL;
    for(uint32_t i=0;i<s->owners.count;i++) {
        const int fd=s->owners.fds[i];
        const int flags=fcntl(fd,F_GETFD);
        if(flags<0)return EBADF;
        if(!(flags&FD_CLOEXEC) || s->cwd_fd==fd)return EINVAL;
        for(unsigned j=0;j<3;j++)if(s->stdio_fd[j]==fd)return EINVAL;
    }
    for(unsigned int i=0;i<8;i++) {
        if(controls[i]<0 || fcntl(controls[i],F_GETFD)<0)return EBADF;
        for(unsigned int j=0;j<i;j++)if(controls[i]==controls[j])return EINVAL;
        if(s->cwd_fd==controls[i])return EINVAL;
        for(unsigned int j=0;j<3;j++)if(s->stdio_fd[j]==controls[i])return EINVAL;
    }
    if(s->cwd_fd<0 || fcntl(s->cwd_fd,F_GETFD)<0)return EBADF;
    for(unsigned int i=0;i<3;i++)if(s->stdio_fd[i]<0 || fcntl(s->stdio_fd[i],F_GETFD)<0)return EBADF;
    for(uint32_t i=0;i<s->group_count;i++)if(s->groups[i]==0 || s->groups[i]==UINT32_MAX
        || (i && s->groups[i]<s->groups[i-1]))return EINVAL;
    size_t remaining=PTG_PAYLOAD_MAX_STRING_BYTES;
    size_t n=0;while(n<remaining && s->program[n]!=0)n++;
    if(n==remaining)return E2BIG;
    remaining-=n+1;
    int e=vector_bytes(s->argv,s->argc,&remaining);if(e)return e;
    e=vector_bytes(s->envp,s->envc,&remaining);if(e)return e;
    e=nonblocking_permit(s->permit_reader);if(e)return e;
    e=nonblocking_pipe(s->ready_writer,O_WRONLY);if(e)return e;
    return nonblocking_pipe(s->exec_writer,O_WRONLY);
}
static _Noreturn void child_fail(int report,int error,int code) {
    const uint32_t e=(uint32_t)(error>0?error:EIO);
    const unsigned char bytes[8]={'E','X','E','1',(unsigned char)e,
        (unsigned char)(e>>8),(unsigned char)(e>>16),(unsigned char)(e>>24)};
    /* Best effort only. A missing/failed report is never an execution proof. */
    (void)syscall(SYS_write,report,bytes,sizeof(bytes));
    (void)syscall(SYS_exit_group,code);_exit(code);
}
static int child_live(const struct ptg_payload_spec *s) {
    if(ptg_payload_gate_state(s->gate)!=PTG_PAYLOAD_RELEASED)return ECANCELED;
    struct pollfd fds[4]={{.fd=-1,.events=POLLIN},{.fd=s->timer_fd,.events=POLLIN},
        {.fd=s->supervisor_fd,.events=POLLIN},{.fd=s->watchdog_fd,.events=POLLIN}};
    const int r=ptg_owner_poll(&s->owners,fds,4,0,0);if(r!=0)return r<0?errno:ECANCELED;
    int e=0;const uint64_t now=now_ns(&e);
    return e?e:(now>=s->deadline_ns?ETIMEDOUT:0);
}
static int core_limits_read(struct ptg_core_limits *out) {
    if(syscall(SYS_prlimit64,0,RLIMIT_CORE,NULL,out)!=0)return errno;
    return out->soft<=out->hard?0:EPROTO;
}
/* Linux 5.14/6.8 abort pipe cores at literal1 before starting a helper, and
 * ELF file cores below ELF_EXEC_PAGESIZE. This is a reviewed kernel contract,
 * not a portable setrlimit API promise. Never raise the original hard limit.
 * Real/saved UID0 MUST remain until this tuple has been restored at dumpable0:
 * dumpability alone does not stop a same-credential peer's remote prlimit.
 */
static int core_fence_begin(struct ptg_core_limits *original) {
    int error=core_limits_read(original);if(error)return error;
    if(original->hard<1)return EPERM;
    const struct ptg_core_limits guarded={.soft=1,.hard=original->hard};
    if(syscall(SYS_prlimit64,0,RLIMIT_CORE,&guarded,NULL)!=0)return errno;
    struct ptg_core_limits observed={0,0};
    error=core_limits_read(&observed);if(error)return error;
    return observed.soft==1 && observed.hard==original->hard?0:EPROTO;
}
static int core_fence_restore(const struct ptg_core_limits *original) {
    struct ptg_core_limits observed={0,0};
    int error=core_limits_read(&observed);if(error)return error;
    if(observed.soft!=1 || observed.hard!=original->hard)return EPROTO;
    if(syscall(SYS_prlimit64,0,RLIMIT_CORE,original,NULL)!=0)return errno;
    error=core_limits_read(&observed);if(error)return error;
    return observed.soft==original->soft && observed.hard==original->hard?0:EPROTO;
}
static int zero_capabilities_confirmed(void) {
    const struct __user_cap_header_struct header={.version=_LINUX_CAPABILITY_VERSION_3,.pid=0};
    struct __user_cap_data_struct caps[2]={{0,0,0},{0,0,0}};
    if(syscall(SYS_capget,&header,caps)!=0)return errno;
    for(unsigned i=0;i<2;i++)
        if(caps[i].effective || caps[i].permitted || caps[i].inheritable)return EPERM;
    const long keep=syscall(SYS_prctl,PR_GET_KEEPCAPS,0,0,0,0);
    if(keep<0)return errno;
    if(keep!=0)return EPERM;
    for(unsigned cap=0;cap<=CAP_LAST_CAP;cap++) {
        const long ambient=syscall(SYS_prctl,PR_CAP_AMBIENT,PR_CAP_AMBIENT_IS_SET,cap,0,0);
        if(ambient<0)return errno;
        if(ambient!=0)return EPERM;
    }
    return 0;
}
static _Noreturn void payload_child(const struct ptg_payload_spec *s) {
    /* All buffers below are fixed C stack/storage. No Rust, allocator, stdio,
     * environment builder, path reopen, libc credential wrapper or exec wrapper.
     * Stack exhaustion is a failed bootstrap, never admission or cleanup proof.
     */
    uint32_t readback_groups[PTG_PAYLOAD_MAX_GROUPS];
    uint64_t mask=0;
    if(raw_mask(NULL,&mask)!=0 || mask!=PTG_BLOCKED_MASK)child_fail(s->exec_writer,EPROTO,125);
    if(ptg_payload_gate_state(s->gate)!=PTG_PAYLOAD_HELD)child_fail(s->exec_writer,ECANCELED,125);
    if(!ptg_owner_watch_valid(&s->owners))child_fail(s->exec_writer,EINVAL,125);
    /* Duplicate all sources high BEFORE replacing low descriptors. */
    const uint32_t owner_count=s->owners.count,base=owner_count+3,count=owner_count+10;
    const uint32_t report_index=base+5;
    const int floor=count>64?(int)count:64;
    int source[PTG_OWNER_WATCH_MAX+10],staged[PTG_OWNER_WATCH_MAX+10];
    for(uint32_t i=0;i<3;i++)source[i]=s->stdio_fd[i];
    for(uint32_t i=0;i<owner_count;i++)source[i+3]=s->owners.fds[i];
    source[base]=s->timer_fd;source[base+1]=s->supervisor_fd;
    source[base+2]=s->watchdog_fd;source[base+3]=s->permit_reader;
    source[base+4]=s->ready_writer;source[base+5]=s->exec_writer;source[base+6]=s->cwd_fd;
    for(uint32_t i=0;i<count;i++)staged[i]=-1;
    staged[report_index]=(int)syscall(SYS_fcntl,source[report_index],F_DUPFD_CLOEXEC,floor);
    if(staged[report_index]<0)child_fail(s->exec_writer,errno,125);
    for(uint32_t i=0;i<count;i++)if(i!=report_index) {
        staged[i]=(int)syscall(SYS_fcntl,source[i],F_DUPFD_CLOEXEC,floor);
        if(staged[i]<0)child_fail(staged[report_index],errno,125);
    }
    for(uint32_t i=0;i<count;i++)if(syscall(SYS_dup3,staged[i],(int)i,i<3?0:O_CLOEXEC)<0)
        child_fail(staged[report_index],errno,125);
    if(syscall(SYS_close_range,count,UINT_MAX,0U)!=0)child_fail((int)report_index,errno,125);
    int32_t local_owners[PTG_OWNER_WATCH_MAX];
    for(uint32_t i=0;i<owner_count;i++)local_owners[i]=(int32_t)i+3;
    struct ptg_payload_spec remapped=*s;
    remapped.owners=(struct ptg_owner_watch){.fds=local_owners,.count=owner_count,.reserved=0};
    remapped.timer_fd=(int32_t)base;remapped.supervisor_fd=(int32_t)base+1;
    remapped.watchdog_fd=(int32_t)base+2;remapped.permit_reader=(int32_t)base+3;
    remapped.ready_writer=(int32_t)base+4;remapped.exec_writer=(int32_t)report_index;
    remapped.cwd_fd=(int32_t)base+6;remapped.cgroup_fd=remapped.kill_fd=-1;
    s=&remapped;
    /* Parent/watchdog retain RW. Only THIS child's VMA becomes read-only. */
    if(syscall(SYS_mprotect,s->gate,sizeof(*s->gate),PROT_READ)!=0)child_fail(s->exec_writer,errno,125);
    /* Irreversible exec privilege barrier precedes every credential change. */
    if(syscall(SYS_prctl,PR_SET_NO_NEW_PRIVS,1,0,0,0)!=0)child_fail(s->exec_writer,errno,125);
    if(syscall(SYS_prctl,PR_GET_NO_NEW_PRIVS,0,0,0,0)!=1)child_fail(s->exec_writer,EPERM,125);
    struct ptg_core_limits original_core={0,0};
    uint32_t r=0,e=0,saved=0;
    if(s->host_dumpability==2) {
        if(syscall(SYS_getresuid,&r,&e,&saved)!=0)child_fail(s->exec_writer,errno,125);
        if(r!=0 || e!=0 || saved!=0 || (uint32_t)syscall(SYS_setfsuid,UINT32_MAX)!=0)
            child_fail(s->exec_writer,EPERM,125);
        const int error=core_fence_begin(&original_core);
        if(error)child_fail(s->exec_writer,error,125);
    }
    if(syscall(SYS_prctl,PR_CAP_AMBIENT,PR_CAP_AMBIENT_CLEAR_ALL,0,0,0)!=0)child_fail(s->exec_writer,errno,125);
    if(syscall(SYS_prctl,PR_SET_KEEPCAPS,0,0,0,0)!=0)child_fail(s->exec_writer,errno,125);
    if(syscall(SYS_setgroups,s->group_count,s->groups)!=0)child_fail(s->exec_writer,errno,125);
    if(syscall(SYS_setresgid,s->gid,s->gid,s->gid)!=0)child_fail(s->exec_writer,errno,125);
    /* The mode2 intermediate real/saved IDs keep external prlimit UID matching
     * false while effective/fs UID changes can reset dumpability to host policy.
     * No untrusted code, callback or READY is reachable with these saved IDs. */
    if(syscall(SYS_setresuid,s->host_dumpability==2?0:s->uid,s->uid,
               s->host_dumpability==2?0:s->uid)!=0)child_fail(s->exec_writer,errno,125);
    const struct __user_cap_header_struct cap_header={.version=_LINUX_CAPABILITY_VERSION_3,.pid=0};
    struct __user_cap_data_struct caps[2]={{0,0,0},{0,0,0}};
    if(syscall(SYS_capset,&cap_header,caps)!=0 || syscall(SYS_capget,&cap_header,caps)!=0)child_fail(s->exec_writer,errno,125);
    for(unsigned int i=0;i<2;i++)if(caps[i].effective || caps[i].permitted || caps[i].inheritable)child_fail(s->exec_writer,EPERM,125);
    if(syscall(SYS_getresgid,&r,&e,&saved)!=0)child_fail(s->exec_writer,errno,125);
    if(r!=s->gid || e!=s->gid || saved!=s->gid || (uint32_t)syscall(SYS_setfsgid,UINT32_MAX)!=s->gid)child_fail(s->exec_writer,EPERM,125);
    if(syscall(SYS_getresuid,&r,&e,&saved)!=0)child_fail(s->exec_writer,errno,125);
    const uint32_t protected_uid=s->host_dumpability==2?0:s->uid;
    if(r!=protected_uid || e!=s->uid || saved!=protected_uid || (uint32_t)syscall(SYS_setfsuid,UINT32_MAX)!=s->uid)child_fail(s->exec_writer,EPERM,125);
    const long group_count=syscall(SYS_getgroups,s->group_count,readback_groups);
    if(group_count<0)child_fail(s->exec_writer,errno,125);
    if((uint64_t)group_count!=s->group_count)child_fail(s->exec_writer,EPERM,125);
    for(uint32_t i=0;i<s->group_count;i++)if(readback_groups[i]!=s->groups[i])child_fail(s->exec_writer,EPERM,125);
    if(syscall(SYS_prctl,PR_GET_NO_NEW_PRIVS,0,0,0,0)!=1)child_fail(s->exec_writer,EPERM,125);
    if(s->host_dumpability==2) {
        const int error=zero_capabilities_confirmed();
        if(error)child_fail(s->exec_writer,error,125);
    }
    /* ID/capability changes can reset dumpability to host policy: explicitly
     * deny unprivileged ptrace before declaring preparation. Activation must
     * separately exclude an already attached tracer/hostile same-UID bootstrap
     * interference; this check cannot retroactively exclude such a tracer. */
    if(syscall(SYS_prctl,PR_SET_DUMPABLE,0,0,0,0)!=0)child_fail(s->exec_writer,errno,125);
    if(syscall(SYS_prctl,PR_GET_DUMPABLE,0,0,0,0)!=0)child_fail(s->exec_writer,EPERM,125);
    if(s->host_dumpability==2) {
        const int error=core_fence_restore(&original_core);
        if(error)child_fail(s->exec_writer,error,125);
        /* Only real/saved IDs change now: effective/fs IDs and all capability
         * sets already match. Reviewed setresuid/commoncap/commit_creds leaves
         * dumpable0 unchanged. Full peer-credential matching begins only here. */
        if(syscall(SYS_setresuid,s->uid,s->uid,s->uid)!=0)child_fail(s->exec_writer,errno,125);
        if(syscall(SYS_getresuid,&r,&e,&saved)!=0)child_fail(s->exec_writer,errno,125);
        if(r!=s->uid || e!=s->uid || saved!=s->uid || (uint32_t)syscall(SYS_setfsuid,UINT32_MAX)!=s->uid)
            child_fail(s->exec_writer,EPERM,125);
        if(syscall(SYS_getresgid,&r,&e,&saved)!=0)child_fail(s->exec_writer,errno,125);
        if(r!=s->gid || e!=s->gid || saved!=s->gid || (uint32_t)syscall(SYS_setfsgid,UINT32_MAX)!=s->gid)
            child_fail(s->exec_writer,EPERM,125);
        const long final_groups=syscall(SYS_getgroups,s->group_count,readback_groups);
        if(final_groups<0)child_fail(s->exec_writer,errno,125);
        if((uint64_t)final_groups!=s->group_count)child_fail(s->exec_writer,EPERM,125);
        for(uint32_t i=0;i<s->group_count;i++)
            if(readback_groups[i]!=s->groups[i])child_fail(s->exec_writer,EPERM,125);
        const int cap_error=zero_capabilities_confirmed();
        if(cap_error)child_fail(s->exec_writer,cap_error,125);
        if(syscall(SYS_prctl,PR_GET_NO_NEW_PRIVS,0,0,0,0)!=1
           || syscall(SYS_prctl,PR_GET_DUMPABLE,0,0,0,0)!=0)child_fail(s->exec_writer,EPERM,125);
    }
    /* Caller credentials, not root, must be able to enter the trusted CWD. */
    if(syscall(SYS_fchdir,s->cwd_fd)!=0 || syscall(SYS_close,s->cwd_fd)!=0)child_fail(s->exec_writer,errno,125);
    /* clone3 did not share fs_struct. Change only this already-contained child,
     * never the caller or root service, and confirm by setting the same mask. */
    const long previous_mask=syscall(SYS_umask,s->umask);
    if(previous_mask<0)child_fail(s->exec_writer,errno,125);
    if((uint64_t)previous_mask>UINT32_C(0777))child_fail(s->exec_writer,EPROTO,125);
    const long confirmed_mask=syscall(SYS_umask,s->umask);
    if(confirmed_mask<0)child_fail(s->exec_writer,errno,125);
    if((uint64_t)confirmed_mask!=s->umask)child_fail(s->exec_writer,EPROTO,125);
    if(raw_mask(NULL,&mask)!=0 || mask!=PTG_BLOCKED_MASK)child_fail(s->exec_writer,EPROTO,125);
    if(ptg_payload_gate_state(s->gate)!=PTG_PAYLOAD_HELD)child_fail(s->exec_writer,ECANCELED,125);
    const struct ready_message ready={.schema=PTG_PAYLOAD_SCHEMA,.token='R',.mask=mask};
    const long ready_sent=syscall(SYS_write,s->ready_writer,&ready,sizeof(ready));
    if(ready_sent!=(long)sizeof(ready))child_fail(s->exec_writer,ready_sent<0?errno:EIO,125);
    if(syscall(SYS_close,s->ready_writer)!=0)child_fail(s->exec_writer,errno,125);
    struct pollfd fds[5]={{.fd=-1,.events=POLLIN},{.fd=s->timer_fd,.events=POLLIN},
        {.fd=s->supervisor_fd,.events=POLLIN},{.fd=s->watchdog_fd,.events=POLLIN},{.fd=s->permit_reader,.events=POLLIN}};
    const int polled=ptg_owner_poll(&s->owners,fds,5,0,1);
    if(polled<=0)child_fail(s->exec_writer,polled<0?errno:EIO,125);
    for(unsigned int i=0;i<4;i++)if(fds[i].revents)child_fail(s->exec_writer,ECANCELED,125);
    if(!(fds[4].revents&POLLIN) || fds[4].revents&~(POLLIN|POLLHUP))child_fail(s->exec_writer,EPROTO,125);
    unsigned char permit=0;
    const long permit_count=syscall(SYS_recvfrom,s->permit_reader,&permit,1,MSG_TRUNC|MSG_DONTWAIT,NULL,0);
    if(permit_count!=1 || permit!='G')child_fail(s->exec_writer,permit_count<0?errno:EPROTO,125);
    if(syscall(SYS_close,s->permit_reader)!=0)child_fail(s->exec_writer,errno,125);
    int live=child_live(s);if(live)child_fail(s->exec_writer,live,125);
    int32_t failed_signal=0,signal_error=0;
    if(ptg_payload_reset_dispositions(&failed_signal,&signal_error)!=0)
        child_fail(s->exec_writer,signal_error>0?signal_error:EPROTO,125);
    if(raw_mask(&s->original_mask,NULL)!=0 || raw_mask(NULL,&mask)!=0)child_fail(s->exec_writer,errno,125);
    if(mask!=s->original_mask)child_fail(s->exec_writer,EPROTO,125);
    /* The live independent watchdog covers the unavoidable poll->exec race. */
    live=child_live(s);if(live)child_fail(s->exec_writer,live,125);
    (void)syscall(SYS_execve,s->program,s->argv,s->envp);
    child_fail(s->exec_writer,errno,127);
}
static void restore_mask(const struct ptg_payload_spec *s,struct ptg_payload_spawn *out) {
    out->mask_restore_result=raw_mask(&out->original_mask,NULL);
    out->mask_restore_error=out->mask_restore_result<0?errno:0;
    if(out->mask_restore_result==0) {
        if(raw_mask(NULL,&out->restored_mask)!=0)out->mask_restore_error=errno;
        else if(out->restored_mask!=out->original_mask)out->mask_restore_error=EPROTO;
    }
    if(out->mask_restore_result!=0 || out->mask_restore_error)
        cancel_kill(s,&out->kill_result,&out->kill_error);
}
void ptg_payload_spawn(const struct ptg_payload_spec *s,struct ptg_payload_spawn *out) {
    if(!out)return;
    memset(out,0,sizeof(*out));out->schema=PTG_PAYLOAD_SCHEMA;out->stage=1;
    out->pid=out->pidfd=-1;out->clone_result=out->initial_poll_result=INT32_MIN;
    out->mask_set_result=out->mask_restore_result=out->kill_result=INT32_MIN;
    int error=spec_valid(s);
    if(!error && !kernel_abi_ok())error=EPROTO;
    if(error){out->error=error;cancel_kill(s,&out->kill_result,&out->kill_error);return;}
    out->gate_state=ptg_payload_gate_state(s->gate);
    if(out->gate_state!=PTG_PAYLOAD_HELD){out->error=ECANCELED;cancel_kill(s,&out->kill_result,&out->kill_error);return;}
    struct pollfd fds[4]={{.fd=-1,.events=POLLIN},{.fd=s->timer_fd,.events=POLLIN},
        {.fd=s->supervisor_fd,.events=POLLIN},{.fd=s->watchdog_fd,.events=POLLIN}};
    const int polled=ptg_owner_poll(&s->owners,fds,4,0,0);const int poll_error=polled<0?errno:0;
    out->initial_poll_result=polled;
    for(unsigned int i=0;i<4;i++)out->initial_revents[i]=fds[i].revents;
    out->before_clone_ns=now_ns(&error);
    if(polled!=0 || error || out->before_clone_ns>=s->deadline_ns){
        out->error=error?error:(polled<0?poll_error:(polled?ECANCELED:ETIMEDOUT));
        cancel_kill(s,&out->kill_result,&out->kill_error);return;
    }
    const uint64_t blocked=PTG_BLOCKED_MASK;
    out->mask_set_result=raw_mask(&blocked,&out->original_mask);
    out->mask_set_error=out->mask_set_result<0?errno:0;
    if(out->mask_set_result!=0){out->error=out->mask_set_error;cancel_kill(s,&out->kill_result,&out->kill_error);return;}
    if(raw_mask(NULL,&out->blocked_mask)!=0 || out->blocked_mask!=blocked || out->original_mask!=s->original_mask){
        out->error=EPROTO;cancel_kill(s,&out->kill_result,&out->kill_error);restore_mask(s,out);return;
    }
    struct clone_args args;memset(&args,0,sizeof(args));
    args.flags=CLONE_PIDFD|CLONE_INTO_CGROUP;args.pidfd=(uint64_t)(uintptr_t)&out->pidfd;
    args.exit_signal=SIGCHLD;args.cgroup=(uint64_t)s->cgroup_fd;out->stage=2;
    const long cloned=syscall(SYS_clone3,&args,sizeof(args));
    if(cloned==0)payload_child(s);
    const int clone_error=cloned<0?errno:0;
    /* Publish successful clone custody before EVERY later operation. */
    if(cloned>0){out->child_created=1;out->pid=(int32_t)cloned;}
    out->clone_result=(int32_t)cloned;
    if(cloned<0)cancel_kill(s,&out->kill_result,&out->kill_error);
    restore_mask(s,out);
    if(cloned<0){out->error=clone_error;return;}
    if(out->mask_restore_result!=0 || out->mask_restore_error){out->error=out->mask_restore_error?out->mask_restore_error:EPROTO;return;}
    if(out->pid<=0 || out->pidfd<0){out->error=EPROTO;cancel_kill(s,&out->kill_result,&out->kill_error);return;}
    out->stage=3;out->after_clone_ns=now_ns(&error);
    out->gate_state=ptg_payload_gate_state(s->gate);
    if(error || out->after_clone_ns<out->before_clone_ns || out->after_clone_ns>=s->deadline_ns
       || out->gate_state!=PTG_PAYLOAD_HELD){
        out->error=error?error:(out->gate_state!=PTG_PAYLOAD_HELD?ECANCELED:ETIMEDOUT);
        cancel_kill(s,&out->kill_result,&out->kill_error);return;
    }
    /* scratch_unmap fields stay0: child uses fixed C stack, no scratch mapping. */
    out->stage=4;
}
void ptg_payload_wait_ready(const struct ptg_payload_spec *s,int32_t reader,int32_t child,
    int32_t setup_timer,uint64_t until,struct ptg_payload_ready *out) {
    if(!out)return;
    memset(out,0,sizeof(*out));out->schema=PTG_PAYLOAD_SCHEMA;out->stage=1;
    out->result=out->read_count=out->kill_result=INT32_MIN;
    if(watch_spec_valid(s) || reader<0 || child<0 || setup_timer<0 || until==0 || until>s->deadline_ns){out->error=EINVAL;goto refused;}
    out->error=nonblocking_pipe(reader,O_RDONLY);if(out->error)goto refused;
    int error=0;out->before_ns=now_ns(&error);
    if(error || out->before_ns>=until){out->error=error?error:ETIMEDOUT;goto refused;}
    struct pollfd fds[7]={{.fd=reader,.events=POLLIN},{.fd=child,.events=POLLIN},
        {.fd=-1,.events=POLLIN},{.fd=s->timer_fd,.events=POLLIN},
        {.fd=s->supervisor_fd,.events=POLLIN},{.fd=s->watchdog_fd,.events=POLLIN},
        {.fd=setup_timer,.events=POLLIN}};
    out->stage=2;out->poll_result=ptg_owner_poll(&s->owners,fds,7,2,1);
    for(unsigned int i=0;i<7;i++)out->revents[i]=fds[i].revents;
    if(out->poll_result<=0){out->error=out->poll_result<0?errno:EIO;goto refused;}
    for(unsigned int i=1;i<7;i++)if(fds[i].revents){out->error=ECANCELED;goto refused;}
    if(!(fds[0].revents&POLLIN) || fds[0].revents&~(POLLIN|POLLHUP)){out->error=EPROTO;goto refused;}
    struct ready_message message={0,0,0};
    const long count=syscall(SYS_read,reader,&message,sizeof(message));
    out->read_count=(int32_t)count;out->message_schema=message.schema;
    out->message_token=message.token;out->child_mask=message.mask;
    if(count!=(long)sizeof(message) || message.schema!=PTG_PAYLOAD_SCHEMA || message.token!='R'
       || message.mask!=PTG_BLOCKED_MASK){out->error=count<0?errno:EPROTO;goto refused;}
    const int after=ptg_owner_poll(&s->owners,fds,7,2,0);
    if(after<0){out->error=errno;goto refused;}
    for(unsigned int i=1;i<7;i++)if(fds[i].revents){out->error=ECANCELED;goto refused;}
    out->after_ns=now_ns(&error);
    if(error || out->after_ns<out->before_ns || out->after_ns>=until
       || ptg_payload_gate_state(s->gate)!=PTG_PAYLOAD_HELD){out->error=error?error:ECANCELED;goto refused;}
    out->stage=3;out->result=0;return;
refused:
    cancel_kill(s,&out->kill_result,&out->kill_error);
}
void ptg_payload_commit(const struct ptg_payload_spec *s,int32_t child,int32_t writer,
    struct ptg_payload_release *out) {
    if(!out)return;
    memset(out,0,sizeof(*out));out->schema=PTG_PAYLOAD_SCHEMA;out->stage=1;
    out->result=out->write_result=out->kill_result=INT32_MIN;
    if(watch_spec_valid(s) || child<0 || writer<0){out->error=EINVAL;goto refused;}
    out->error=nonblocking_permit(writer);if(out->error)goto refused;
    out->gate_before=ptg_payload_gate_state(s->gate);
    if(out->gate_before!=PTG_PAYLOAD_HELD){out->error=ECANCELED;goto refused;}
    struct pollfd fds[5]={{.fd=-1,.events=POLLIN},{.fd=s->timer_fd,.events=POLLIN},
        {.fd=s->supervisor_fd,.events=POLLIN},{.fd=s->watchdog_fd,.events=POLLIN},{.fd=child,.events=POLLIN}};
    out->poll_result=ptg_owner_poll(&s->owners,fds,5,0,0);
    for(unsigned int i=0;i<5;i++)out->revents[i]=fds[i].revents;
    if(out->poll_result!=0){out->error=out->poll_result<0?errno:ECANCELED;goto refused;}
    int error=0;out->before_ns=now_ns(&error);
    if(error || out->before_ns>=s->deadline_ns){out->error=error?error:ETIMEDOUT;goto refused;}
    if(ptg_payload_gate_commit(s->gate)!=PTG_PAYLOAD_HELD){out->error=ECANCELED;goto refused;}
    out->gate_after=ptg_payload_gate_state(s->gate);
    if(out->gate_after!=PTG_PAYLOAD_RELEASED){out->error=ECANCELED;goto refused;}
    out->stage=2;
    const long sent=syscall(SYS_sendto,writer,"G",1,MSG_NOSIGNAL|MSG_DONTWAIT,NULL,0);
    out->write_result=(int32_t)sent;out->write_error=sent<0?errno:0;
    if(sent!=1){out->error=out->write_error?out->write_error:EIO;goto refused;}
    out->after_ns=now_ns(&error);out->gate_after=ptg_payload_gate_state(s->gate);
    if(error || out->after_ns<out->before_ns || out->after_ns>=s->deadline_ns
       || out->gate_after!=PTG_PAYLOAD_RELEASED){out->error=error?error:ECANCELED;goto refused;}
    out->stage=3;out->result=0;return;
refused:
    cancel_kill(s,&out->kill_result,&out->kill_error);
    out->gate_after=s?ptg_payload_gate_state(s->gate):-1;
}
