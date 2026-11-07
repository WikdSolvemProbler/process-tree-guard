// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Native factory/watchdog/readiness edge used by the concrete NativePort.
use super::*;
use super::cgroup_authority::settlement::Deadline;

const READINESS_BOUND_NS: u64 = 1_000_000_000;
const BLOCKED_KERNEL_MASK:u64=u64::MAX & !(1u64<<8) & !(1u64<<18);
#[cfg(target_arch="aarch64")]
const RT_SIGPROCMASK_NUMBER:u64=135;
#[cfg(target_arch="x86_64")]
const RT_SIGPROCMASK_NUMBER:u64=14;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SpawnObservation {
    schema: u32, stage: u32,
    child_created: c_int, pid: c_int, pidfd: c_int, call_errno: c_int,
    mask_restore_result: c_int, mask_restore_errno: c_int,
    initial_poll_result: c_int, initial_revents: [c_int;3], clone_result: c_int, reserved: c_int,
    original_signal_mask: u64, before_clone_ns: u64, after_clone_ns: u64,
    mask_set_result:c_int, mask_set_errno:c_int, mask_query_result:c_int, mask_query_errno:c_int,
    mask_restore_query_result:c_int, mask_restore_query_errno:c_int, failure_kill_result:c_int, failure_kill_errno:c_int,
    blocked_mask:u64, restored_mask:u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ReadinessObservation {
    result: c_int, call_errno: c_int, read_count: c_int, token: u32,
    revents: [c_int;6], after_revents: [c_int;6], poll_result: c_int, after_poll_result: c_int,
    before_ns: u64, after_ns: u64,
    message_schema:u32, reserved:u32, child_mask:u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct KillObservation { result: c_int, call_errno: c_int }
const _: () = {
    assert!(std::mem::size_of::<SpawnObservation>()==128);
    assert!(std::mem::offset_of!(SpawnObservation,original_signal_mask)==56);
    assert!(std::mem::size_of::<ReadinessObservation>()==104);
    assert!(std::mem::offset_of!(ReadinessObservation,before_ns)==72);
    assert!(std::mem::size_of::<KillObservation>()==8);
    assert!(std::mem::offset_of!(SpawnObservation,blocked_mask)==112);
    assert!(std::mem::offset_of!(ReadinessObservation,child_mask)==96);
};
unsafe extern "C" {
    fn ptg_arm_abi(word:u32)->u64;
    fn ptg_arm_spawn_watchdog(gate:*mut c_void,owner:c_int,timer:c_int,supervisor:c_int,kill:c_int,
        ready:c_int,deadline_ns:u64,out:*mut SpawnObservation);
    fn ptg_arm_wait_ready(ready:c_int,child:c_int,owner:c_int,timer:c_int,supervisor:c_int,
        setup_timer:c_int,setup_until_ns:u64,out:*mut ReadinessObservation);
    fn ptg_arm_kill_held(kill:c_int,out:*mut KillObservation);
}

// The caller retains this real receipt alongside success OR ArmFailure through
// the existing014 consuming controller. None means no native call occurred.
// It contains an owned, pre-clone short readiness timer, not a replacement
// execution lease or a claim that dropping it settles any child/domain.
#[derive(Default)]
pub(super) struct ArmingReceipt {
    started: bool,
    abi: Option<[u64;17]>,
    original_deadline_ns: Option<u64>,
    setup: Option<Deadline>,
    spawn: Option<SpawnObservation>,
    readiness: Option<ReadinessObservation>,
    refused_kill: Option<KillObservation>,
    ready: bool,
    gate_error: Option<String>,
}

impl ArmingReceipt {
    pub(super) fn original_mask(&self)->Option<u64> {
        self.spawn.filter(|_|self.parent_mask_confirmed()).map(|raw|raw.original_signal_mask)
    }
    // A caller may not infer whole original-mask restoration from014's narrower
    // SIGCHLD/subreaper checks. A failed or absent raw proof remains unconfirmed.
    pub(super) fn parent_mask_uncertain(&self)->bool {
        self.spawn.is_some_and(|raw| raw.mask_set_result != c_int::MIN) && !self.parent_mask_confirmed()
    }
    pub(super) fn parent_mask_confirmed(&self)->bool {
        self.spawn.is_some_and(|raw|raw.mask_set_result==0 && raw.mask_set_errno==0
            && raw.mask_query_result==0 && raw.mask_query_errno==0 && raw.blocked_mask==BLOCKED_KERNEL_MASK
            && raw.mask_restore_result==0 && raw.mask_restore_errno==0
            && raw.mask_restore_query_result==0 && raw.mask_restore_query_errno==0
            && raw.original_signal_mask==raw.restored_mask)
    }
}

fn refuse_verified(controls:DomainControls, watchdog:Option<DirectChild>,
    mut receipt:ArmingReceipt, reason:&'static str)->ArmFailure {
    let mut raw=KillObservation::default();
    // Trust is in the original factory-held FD; no current name, clock,
    // format or allocation is allowed to gate this cancellation attempt.
    controls.cancel_release();
    unsafe { ptg_arm_kill_held(controls.supervisor_kill.as_raw_fd(),&mut raw) };
    receipt.refused_kill=Some(raw);
    ArmFailure::VerifiedDomain { controls,watchdog,error:reason.into(),arming:receipt }
}

fn prepare(watch:&LeaseWatch<'_>, receipt:&mut ArmingReceipt)->Probe<(OwnedFd,OwnedFd)> {
    check(!receipt.started,"native arming receipt cannot be reused")?;
    receipt.started=true;
    let abi=std::array::from_fn(|i|unsafe{ptg_arm_abi(i as u32)});
    receipt.abi=Some(abi);
    check(abi==[2,128,104,8,CLONE_PIDFD,SIGCHLD,435,7,8,8,64,2,RT_SIGPROCMASK_NUMBER,9,19,8,BLOCKED_KERNEL_MASK],"native watchdog bridge ABI differs")?;
    check(watch.owner_pid>0,"native armer lacks original owner identity")?;
    let now=boottime_ns()?;
    let setup_until=now.checked_add(READINESS_BOUND_NS).ok_or("readiness deadline overflow")?
        .min(watch.deadline_ns);
    receipt.original_deadline_ns=Some(watch.deadline_ns);
    // Store the actual armed timer before any post-arm validation. Original
    // owner/timer/deadline remain unchanged and separately retained.
    receipt.setup=Some(Deadline::arm_before_placement(setup_until)?);
    receipt.setup.as_ref().expect("stored deadline").check()?;
    pipe()
}

/// # Safety
/// Called only by the dedicated one-shot supervisor established by014, with
/// exclusive clone/wait ownership and an independently finite enclosing process
/// authority covering setup, native filesystem/syscall stalls, and teardown.
/// Inputs are original private LeaseWatch borrows, never request-controlled FDs.
/// No payload exists or may be created by this component. The caller must pass
/// every returned ArmFailure into the real014 failure controller, and must keep
/// this receipt with that outcome. Successful ArmedDomain still requires actual
/// placement, shared release cancellation and terminal-controller integration.
pub(super) unsafe fn arm(watch:LeaseWatch<'_>,scope:&cgroup_authority::service_scope::VerifiedServiceScope)
    ->std::result::Result<ArmedDomain,ArmFailure> {
    let mut receipt=ArmingReceipt::default();
    let (reader,writer)=match prepare(&watch,&mut receipt) {
        Ok(value)=>value, Err(error)=>return Err(ArmFailure::BeforeProvision{error,arming:receipt}),
    };
    let mut controls=match cgroup_authority::provision(scope) {
        Ok(value)=>value, Err(failure)=>return Err(ArmFailure::Provision{failure,arming:receipt}),
    };
    controls.gate=match native_port::SharedGate::create() {
        Ok(value)=>Some(value),
        Err(error)=>{receipt.gate_error=Some(error);return Err(refuse_verified(controls,None,receipt,"shared cancellation allocation refused"));},
    };
    let mut raw=SpawnObservation::default();
    unsafe { ptg_arm_spawn_watchdog(controls.gate.as_ref().expect("installed shared gate").as_ptr(),watch.owner.as_raw_fd(),watch.timer.as_raw_fd(),
        watch.supervisor.as_raw_fd(),controls.watchdog_kill.as_raw_fd(),writer.as_raw_fd(),
        watch.deadline_ns,&mut raw) };
    // Trusted C's positive-clone contract: CLONE_PIDFD installed this new fd
    // with the same positive direct-child PID. Capture ownership immediately,
    // BEFORE interpreting any post-clone clock/mask/readiness error. No numeric
    // pidfd_open or wait can reopen/reassign that child's identity.
    let watchdog=if raw.child_created==1 {
        Some(unsafe {DirectChild::from_successful_clone(raw.pid,OwnedFd::from_raw_fd(raw.pidfd))})
    } else {None};
    receipt.spawn=Some(raw);
    drop(writer); // child owns the only remaining readiness writer after scrub.
    if raw.child_created!=1 {
        return Err(refuse_verified(controls,watchdog,receipt,"native watchdog clone refused"));
    }
    if raw.schema!=2 || raw.stage!=4 || raw.pid<=0 || raw.pidfd<0 || raw.clone_result!=raw.pid
        || raw.call_errno!=0 || raw.mask_restore_result!=0 || raw.mask_restore_errno!=0
        || raw.mask_set_result!=0 || raw.mask_set_errno!=0 || raw.mask_query_result!=0 || raw.mask_query_errno!=0
        || raw.blocked_mask!=BLOCKED_KERNEL_MASK || raw.mask_restore_query_result!=0 || raw.mask_restore_query_errno!=0
        || raw.restored_mask!=raw.original_signal_mask || raw.failure_kill_result!=c_int::MIN
        || raw.initial_poll_result!=0 || raw.initial_revents!=[0;3]
        || raw.before_clone_ns==0 || raw.after_clone_ns<raw.before_clone_ns
        || raw.after_clone_ns>=watch.deadline_ns {
        return Err(refuse_verified(controls,watchdog,receipt,"native watchdog post-clone state refused"));
    }
    let watchdog=watchdog.expect("positive native clone was retained");
    let setup=receipt.setup.as_ref().expect("readiness timer precedes provision");
    let mut ready=ReadinessObservation::default();
    unsafe {ptg_arm_wait_ready(reader.as_raw_fd(),watchdog.pidfd.as_raw_fd(),
        watch.owner.as_raw_fd(),watch.timer.as_raw_fd(),watch.supervisor.as_raw_fd(),
        setup.timer_fd(),setup.effective_until_ns(),&mut ready)};
    receipt.readiness=Some(ready);
    if ready.result!=0 || ready.call_errno!=0 || ready.read_count!=16 || ready.token!=u32::from(b'R')
        || ready.message_schema!=2 || ready.child_mask!=BLOCKED_KERNEL_MASK
        || ready.before_ns<raw.after_clone_ns || ready.after_ns<ready.before_ns
        || ready.after_ns>=setup.effective_until_ns() {
        return Err(refuse_verified(controls,Some(watchdog),receipt,"native watchdog readiness refused"));
    }
    receipt.ready=true;
    Ok(ArmedDomain{controls,watchdog,arming:receipt})
}

pub(super) unsafe fn kill_held(fd:c_int,out:*mut KillObservation) {unsafe{ptg_arm_kill_held(fd,out)}}
