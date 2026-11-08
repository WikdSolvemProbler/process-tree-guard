// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Owned one-shot supervisor; no fixture clients or child drivers.
use super::*;
use std::sync::Arc;
use super::invocation::{VerifiedInvocation,OwnerWatch,MAX_WATCHERS};
use std::ffi::c_char;
const POLICY_REFUSAL: &str = "verified identity refused by activation policy";
type Probe<T> = std::result::Result<T, String>;
fn native<T>(result: super::Result<T>) -> Probe<T> {
    result.map_err(|error| format!("{error:?}"))
}

fn io_result<T>(result: io::Result<T>) -> Probe<T> {
    result.map_err(|error| error.to_string())
}

fn check(condition: bool, message: &str) -> Probe<()> {
    if !condition {
        return Err(message.into());
    }
    Ok(())
}

fn fd_from_call(value: c_int) -> Probe<OwnedFd> {
    let fd = native(os_result(value as isize))? as RawFd;
    // The successful socket or accept syscall returns a newly owned fd.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

const CLOCK_BOOTTIME: c_int = 7;
const TFD_TIMER_ABSTIME: c_int = 1;
const POLLHUP: c_short = 0x10;
const F_DUPFD_CLOEXEC: c_int = 1030;
const NS_PER_SECOND: u64 = 1_000_000_000;

#[repr(C)]
#[derive(Default)]
struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}

#[repr(C)]
struct ItimerSpec {
    interval: Timespec,
    value: Timespec,
}

unsafe extern "C" {
    fn clock_gettime(clock: c_int, time: *mut Timespec) -> c_int;
    fn timerfd_create(clock: c_int, flags: c_int) -> c_int;
    fn timerfd_settime(
        fd: c_int,
        flags: c_int,
        value: *const ItimerSpec,
        old: *mut ItimerSpec,
    ) -> c_int;
    fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;
}

pub(super) fn boottime_ns() -> Probe<u64> {
    let mut time = Timespec::default();
    native(os_result(
        unsafe { clock_gettime(CLOCK_BOOTTIME, &mut time) } as isize,
    ))?;
    let seconds = u64::try_from(time.seconds).map_err(|_| "negative boot time")?;
    let nanoseconds = u64::try_from(time.nanoseconds).map_err(|_| "negative boot fraction")?;
    check(nanoseconds < NS_PER_SECOND, "invalid boot fraction")?;
    seconds
        .checked_mul(NS_PER_SECOND)
        .and_then(|value| value.checked_add(nanoseconds))
        .ok_or_else(|| "boot time overflow".into())
}

// Own a receiver-private, absolute timer. The sender never supplies timer authority.
struct RetainedLease {
    invocation: Arc<VerifiedInvocation>, timer:OwnedFd,
    captured_ns:u64, deadline_ns:u64,
    observed:std::cell::Cell<Option<Decision>>,
}
impl RetainedLease {
    fn from_invocation(invocation:Arc<VerifiedInvocation>)->Probe<Self> {
        let captured_ns=invocation.metadata().captured_ns;
        let deadline_ns=invocation.metadata().deadline_ns;
        let timer=fd_from_call(unsafe{timerfd_create(CLOCK_BOOTTIME,SOCK_NONBLOCK|SOCK_CLOEXEC)})?;
        native(close_on_exec(timer.as_raw_fd()))?;
        let value=ItimerSpec{interval:Timespec::default(),value:Timespec{
            seconds:(deadline_ns/NS_PER_SECOND) as i64,nanoseconds:(deadline_ns%NS_PER_SECOND) as i64}};
        native(os_result(unsafe{timerfd_settime(timer.as_raw_fd(),TFD_TIMER_ABSTIME,&value,ptr::null_mut())} as isize))?;
        Ok(Self{invocation,timer,captured_ns,deadline_ns,observed:std::cell::Cell::new(None)})
    }
    fn sample(&self)->Probe<DecisionReceipt> {
        let owners=self.invocation.numbers();
        let mut watched=[PollFd{fd:-1,events:POLLIN,revents:0};MAX_WATCHERS+1];
        for (entry,fd) in watched.iter_mut().zip(owners){entry.fd=*fd;}
        watched[owners.len()].fd=self.timer.as_raw_fd();
        let watched=&mut watched[..owners.len()+1];
        let count=native(os_result(unsafe{poll(watched.as_mut_ptr(),watched.len(),0)} as isize))?;
        let sampled_ns=boottime_ns()?;
        let owner_events=watched[..owners.len()].iter().fold(0,|bits,p|bits|p.revents);
        let timer_events=watched[owners.len()].revents;
        check(count==watched.iter().filter(|fd|fd.revents!=0).count()
            && owner_events & !(POLLIN|POLLHUP)==0 && timer_events & !POLLIN==0,
            "unexpected original-owner sample poll result")?;
        let decision=if owner_events!=0 {Decision::OwnerLost}
            else if timer_events!=0||sampled_ns>=self.deadline_ns {Decision::Deadline}else{Decision::Permit};
        // An observed owner loss/deadline is irreversible, even if cleanup runs later.
        let latched=match (self.observed.get(),decision){
            (Some(Decision::OwnerLost),_)|(_,Decision::OwnerLost)=>Decision::OwnerLost,
            (Some(Decision::Deadline),_)|(_,Decision::Deadline)=>Decision::Deadline,
            _=>Decision::Permit,
        };
        self.observed.set(Some(latched));
        Ok(DecisionReceipt{decision:latched,owner_events,timer_events,sampled_ns})
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Permit,
    OwnerLost,
    Deadline,
}

#[derive(Debug)]
struct DecisionReceipt {
    decision: Decision,
    owner_events: c_short,
    timer_events: c_short,
    sampled_ns: u64,
}
fn pipe() -> Probe<(OwnedFd, OwnedFd)> {
    let mut fds = [-1; 2];
    native(os_result(
        unsafe { pipe2(fds.as_mut_ptr(), SOCK_CLOEXEC | SOCK_NONBLOCK) } as isize,
    ))?;
    // Successful pipe2 installs two newly owned descriptors.
    let [reader, writer] = fds.map(|fd| unsafe { OwnedFd::from_raw_fd(fd) });
    native(close_on_exec(reader.as_raw_fd()))?;
    native(close_on_exec(writer.as_raw_fd()))?;
    Ok((reader, writer))
}

#[derive(Debug, PartialEq, Eq)]
struct VerifiedIdentity {
    uid: u32,
    gid: u32,
    groups: Vec<u32>,
}

struct PreparedRun {
    owner_pid: c_int,
    identity: VerifiedIdentity,
    plan: request::ExecPlan,
    lease: RetainedLease,
}

fn status_field<'a>(status: &'a str, name: &str) -> Probe<&'a str> {
    let mut fields = status.lines().filter_map(|line| line.strip_prefix(name));
    let value = fields
        .next()
        .ok_or_else(|| format!("missing process status field {name}"))?;
    check(fields.next().is_none(), "duplicate process status field")?;
    Ok(value.trim())
}

fn status_ids(status: &str, name: &str) -> Probe<Vec<u32>> {
    status_field(status, name)?
        .split_whitespace()
        .map(|id| {
            id.parse()
                .map_err(|_| format!("invalid process ID field {name}"))
        })
        .collect()
}

fn verify_identity(owner: &OwnedFd, peer: Credentials) -> Probe<VerifiedIdentity> {
    native(validate_pidfd(owner, peer.pid, &mut NativeReceipt::default()))?;
    let status = io_result(fs::read_to_string(format!("/proc/{}/status", peer.pid)))?;
    check(
        peer.uid != 0
            && peer.gid != 0
            && status_ids(&status, "Uid:")? == [peer.uid; 4]
            && status_ids(&status, "Gid:")? == [peer.gid; 4],
        "owner credentials differ from the kernel transport identity",
    )?;
    for name in ["CapEff:", "CapPrm:", "CapInh:", "CapAmb:"] {
        check(
            status_field(&status, name)? == "0000000000000000",
            "owner retained capabilities",
        )?;
    }
    check(
        status_field(&status, "NoNewPrivs:")? == "1",
        "owner lacks NoNewPrivs",
    )?;
    let groups = status_ids(&status, "Groups:")?;
    check(!groups.contains(&0), "owner has a root supplementary group")?;
    // A PID exit or replacement during the numeric procfs lookup refuses preparation.
    native(validate_pidfd(owner, peer.pid, &mut NativeReceipt::default()))?;
    Ok(VerifiedIdentity {
        uid: peer.uid,
        gid: peer.gid,
        groups,
    })
}

// Retain the actual original objects/plan on preparation refusal. No child exists.
struct PreparationFailure {
    invocation:Arc<VerifiedInvocation>,plan:request::ExecPlan,
    lease:Option<RetainedLease>,error:String,code:i32,
}
fn prepare(peer:Credentials,invocation:VerifiedInvocation,plan:request::ExecPlan,
    authorizes:impl FnOnce(&VerifiedIdentity)->bool)->std::result::Result<PreparedRun,PreparationFailure> {
    let invocation=Arc::new(invocation);
    let mut lease=None;
    let result=(||->std::result::Result<VerifiedIdentity,(String,i32)>{
        invocation.check_live().map_err(|e|(format!("{e:?}"),e.exit_code()))?;
        invocation.check_time().map_err(|e|(format!("{e:?}"),e.exit_code()))?;
        let identity=verify_identity(invocation.requester(),peer).map_err(|e|(e,125))?;
        check(authorizes(&identity),POLICY_REFUSAL).map_err(|e|(e,125))?;
        lease=Some(RetainedLease::from_invocation(Arc::clone(&invocation)).map_err(|e|(e,125))?);
        let decision=lease.as_ref().expect("stored private timer").sample().map_err(|e|(e,125))?.decision;
        match decision {Decision::Permit=>Ok(identity),Decision::Deadline=>Err(("original invocation expired during preparation".into(),124)),
            Decision::OwnerLost=>Err(("original invocation owner lost during preparation".into(),125))}
    })();
    match result {
        Ok(identity)=>Ok(PreparedRun{owner_pid:peer.pid,identity,plan,lease:lease.expect("successful preparation armed timer")}),
        Err((error,code))=>Err(PreparationFailure{invocation,plan,lease,error,code}),
    }
}


#[path="native_activation.rs"]
pub(super) mod native_activation;
use native_activation::LaunchContext;
pub(crate) use native_activation::{RunOutcome,VerifiedServiceScope};

/// # Safety
/// The privileged launcher must establish a fresh trusted one-shot main process,
/// initial trusted namespaces/bootstrap environment, exclusive wait ownership,
/// no unrelated children or threads, and independent finite authority covering
/// BOTH the supervisor/watchdog and the exact payload domain. Killing only a
/// service-manager cgroup is insufficient if placement migrates payload outside
/// it. Those launcher preconditions remain unqualified; selector stays disabled.
/// The received kernel-authenticated request supplies all caller IO/CWD/env;
/// no privileged ambient or externally fabricated caller context is accepted.
pub(crate) unsafe fn run_received(
    connection:OwnedFd,cancellation:super::entry::AuthenticatedCancellation,
    authorizes:impl FnOnce(u32,u32,&[u32],&std::ffi::CStr)->bool,
)->RunOutcome {
    let cancellation=Arc::new(cancellation);
    let scope=match native_activation::capture_service_scope(){
        Ok(scope)=>scope,
        Err(error)=>return native_activation::before_admission(None,Arc::clone(&cancellation),NativeReceipt::default(),error),
    };
    unsafe{run_in_service_arc(scope,connection,cancellation,authorizes)}
}

/// Typed internal consuming path, exported only so the actual checked entry and
/// compiler anchor can reach the complete NativePort/controller graph. A type
/// export grants no construction/mint authority. No fake token is provided.
/// # Safety
/// Real entry must already establish the full public original owner set and
/// pre-manager absolute lease as well as all dedicated supervisor invariants.
/// OriginalInvocation capture and PTGEX4 import preserve that set/cutoff here;
/// actual public dispatch/manager bootstrap still must call them before helpers.
pub(crate) unsafe fn run_in_service(
    scope:VerifiedServiceScope,
    connection:OwnedFd,cancellation:super::entry::AuthenticatedCancellation,
    authorizes:impl FnOnce(u32,u32,&[u32],&std::ffi::CStr)->bool,
)->RunOutcome {unsafe{run_in_service_arc(scope,connection,Arc::new(cancellation),authorizes)}}
unsafe fn run_in_service_arc(scope:VerifiedServiceScope,connection:OwnedFd,cancellation:Arc<super::entry::AuthenticatedCancellation>,
 authorizes:impl FnOnce(u32,u32,&[u32],&std::ffi::CStr)->bool)->RunOutcome{
    let mut admission=NativeReceipt::default();
    if let Err(error)=scope.revalidate(){
        return native_activation::before_admission(Some(scope),Arc::clone(&cancellation),admission,error);
    }
    let admitted=match receive_observed(connection.as_raw_fd(),&mut admission){
        Ok(value)=>value,Err(error)=>{let code=match &error {Refusal::Invocation(e)=>e.exit_code(),_=>125};
            return native_activation::before_admission_code(Some(scope),Arc::clone(&cancellation),admission,format!("admission refused: {error:?}"),code);},
    };
    unsafe{run_admitted_in_service(scope,admitted,admission,cancellation,authorizes)}
}
pub(super) unsafe fn run_admitted_in_service(scope:VerifiedServiceScope,admitted:Admitted,admission:NativeReceipt,cancellation:Arc<super::entry::AuthenticatedCancellation>,
    authorizes:impl FnOnce(u32,u32,&[u32],&std::ffi::CStr)->bool)->RunOutcome {
    let Admitted{peer,invocation,plan,io}=admitted;
    let launch=LaunchContext::from_received(peer,io,plan.environment().to_vec(),plan.invocation().umask,cancellation);
    let program=plan.program().to_owned();
    let prepared=match prepare(peer,invocation,plan,
        |identity|authorizes(identity.uid,identity.gid,&identity.groups,&program)) {
        Ok(value)=>value,Err(error)=>return native_activation::before_activation(launch,scope,admission,error),
    };
    unsafe{native_activation::run_admitted(prepared,launch,scope,admission)}
}

pub(super) fn validate_received_identity(admitted:&Admitted)->Probe<()>{verify_identity(admitted.invocation.requester(),admitted.peer).map(|_|())}
pub(super) fn monotonic_ns()->Probe<u64>{let mut time=Timespec::default();native(os_result(unsafe{clock_gettime(1,&mut time)} as isize))?;let seconds=u64::try_from(time.seconds).map_err(|_|"negative monotonic time")?;let nanos=u64::try_from(time.nanoseconds).map_err(|_|"negative monotonic fraction")?;check(nanos<NS_PER_SECOND,"invalid monotonic fraction")?;seconds.checked_mul(NS_PER_SECOND).and_then(|n|n.checked_add(nanos)).ok_or("monotonic overflow".into())}
