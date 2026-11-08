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
    initial_poll_result: c_int, initial_revents: [c_int;3], clone_result: c_int, cancellation_revents: c_int,
    original_signal_mask: u64, before_clone_ns: u64, after_clone_ns: u64,
    mask_set_result:c_int, mask_set_errno:c_int, mask_query_result:c_int, mask_query_errno:c_int,
    mask_restore_query_result:c_int, mask_restore_query_errno:c_int, failure_kill_result:c_int, failure_kill_errno:c_int,
    blocked_mask:u64, restored_mask:u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ReadinessObservation {
    result: c_int, call_errno: c_int, read_count: c_int, token: u32,
    revents: [c_int;7], after_revents: [c_int;7], poll_result: c_int, after_poll_result: c_int,
    before_ns: u64, after_ns: u64,
    message_schema:u32, reserved:u32, child_mask:u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct KillObservation { result: c_int, call_errno: c_int }
const _: () = {
    assert!(std::mem::size_of::<SpawnObservation>()==128);
    assert!(std::mem::offset_of!(SpawnObservation,original_signal_mask)==56);
    assert!(std::mem::offset_of!(SpawnObservation,cancellation_revents)==52);
    assert!(std::mem::size_of::<ReadinessObservation>()==112);
    assert!(std::mem::offset_of!(ReadinessObservation,before_ns)==80);
    assert!(std::mem::size_of::<KillObservation>()==8);
    assert!(std::mem::offset_of!(SpawnObservation,blocked_mask)==112);
    assert!(std::mem::offset_of!(ReadinessObservation,child_mask)==104);
};
unsafe extern "C" {
    fn ptg_arm_abi(word:u32)->u64;
    fn ptg_arm_spawn_watchdog(gate:*mut c_void,owners:*const OwnerWatch,timer:c_int,supervisor:c_int,cancellation:c_int,kill:c_int,
        ready:c_int,deadline_ns:u64,out:*mut SpawnObservation);
    fn ptg_arm_wait_ready(ready:c_int,child:c_int,owners:*const OwnerWatch,timer:c_int,supervisor:c_int,cancellation:c_int,
        setup_timer:c_int,setup_until_ns:u64,out:*mut ReadinessObservation);
    fn ptg_arm_kill_held(kill:c_int,out:*mut KillObservation);
}

impl ReadinessObservation{
 fn private_only_cancel(&self)->bool{
  let recognized=|events:&[c_int;7]|events[6]!=0&&events[6]&!(1|0x2000|0x10)==0
   &&events[0]&!(1|0x10)==0&&events[1..6].iter().all(|value|*value==0);
  self.result<0&&self.call_errno==125&&((self.poll_result>0&&recognized(&self.revents))||(self.after_poll_result>0&&recognized(&self.after_revents)))
 }
}


#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum DeadlineSite {PrepareClock,SpawnTimer,SpawnBeforeClock,SpawnAfterClock,ReadyBeforeClock,ReadyFirstTimer,ReadyAfterTimer,ReadyAfterClock}
#[derive(Debug,PartialEq,Eq)]
struct OriginalDeadlineProof {site:DeadlineSite,cutoff:u64,sampled_ns:Option<u64>}
fn prepare_deadline_proof(now:u64,cutoff:u64)->Option<OriginalDeadlineProof>{
 (cutoff>0&&now>=cutoff).then_some(OriginalDeadlineProof{site:DeadlineSite::PrepareClock,cutoff,sampled_ns:Some(now)})
}
fn expected_abi()->[u64;21]{[4,128,112,8,CLONE_PIDFD,SIGCHLD,435,7,8,8,64,2,RT_SIGPROCMASK_NUMBER,9,19,8,BLOCKED_KERNEL_MASK,16,249,8193,256]}
impl SpawnObservation {
 fn untouched_mask(&self)->bool{
  self.mask_set_result==c_int::MIN&&self.mask_set_errno==0&&self.mask_query_result==c_int::MIN&&self.mask_query_errno==0
   &&self.mask_restore_result==c_int::MIN&&self.mask_restore_errno==0&&self.mask_restore_query_result==c_int::MIN&&self.mask_restore_query_errno==0
   &&self.original_signal_mask==0&&self.blocked_mask==0&&self.restored_mask==0
 }
 fn no_clone(&self)->bool{self.schema==4&&self.stage==1&&self.child_created==0&&self.pid== -1&&self.pidfd== -1
  &&self.clone_result==c_int::MIN&&self.cancellation_revents==0&&self.after_clone_ns==0&&self.untouched_mask()&&self.failure_kill_errno==0}
 fn complete(&self,cutoff:u64)->bool{
  cutoff>0&&self.schema==4&&self.stage==4&&self.child_created==1&&self.pid>0&&self.pidfd>=0&&self.clone_result==self.pid&&self.call_errno==0
   &&self.initial_poll_result==0&&self.initial_revents==[0;3]&&self.cancellation_revents==0
   &&self.mask_set_result==0&&self.mask_set_errno==0&&self.mask_query_result==0&&self.mask_query_errno==0
   &&self.mask_restore_result==0&&self.mask_restore_errno==0&&self.mask_restore_query_result==0&&self.mask_restore_query_errno==0
   &&self.blocked_mask==BLOCKED_KERNEL_MASK&&self.original_signal_mask&!BLOCKED_KERNEL_MASK==0&&self.restored_mask==self.original_signal_mask
   &&self.failure_kill_result==c_int::MIN&&self.failure_kill_errno==0&&self.before_clone_ns>0&&self.before_clone_ns<cutoff&&self.after_clone_ns>=self.before_clone_ns
 }
 fn deadline_proof(&self,cutoff:u64)->Option<OriginalDeadlineProof>{
  if cutoff==0{return None;}
  let (site,sampled_ns)=if self.no_clone()&&self.call_errno==125&&self.initial_poll_result==1&&self.initial_revents==[0,1,0]
    &&self.before_clone_ns==0&&self.failure_kill_result==1{(DeadlineSite::SpawnTimer,None)}
   else if self.no_clone()&&self.call_errno==110&&self.initial_poll_result==0&&self.initial_revents==[0;3]
    &&self.before_clone_ns>=cutoff&&self.failure_kill_result==c_int::MIN{(DeadlineSite::SpawnBeforeClock,Some(self.before_clone_ns))}
   else if self.complete(cutoff)&&self.after_clone_ns>=cutoff{(DeadlineSite::SpawnAfterClock,Some(self.after_clone_ns))}
   else{return None;};
  Some(OriginalDeadlineProof{site,cutoff,sampled_ns})
 }
}
impl ReadinessObservation {
 fn empty_message(&self)->bool{self.read_count== -1&&self.token==0&&self.message_schema==0&&self.reserved==0&&self.child_mask==0}
 fn complete_message(&self)->bool{self.read_count==16&&self.token==u32::from(b'R')&&self.message_schema==4&&self.reserved==0&&self.child_mask==BLOCKED_KERNEL_MASK}
 fn first_ready_only(&self)->bool{self.poll_result==1&&matches!(self.revents[0],1|17)&&self.revents[1..].iter().all(|event|*event==0)}
 fn exact_count(result:c_int,events:&[c_int;7])->bool{events[2]==0&&result>=0&&result as usize==events.iter().filter(|event|**event!=0).count()}
 fn timer_events(events:&[c_int;7],setup:u64,cutoff:u64,allow_ready:bool)->bool{
  (if allow_ready{matches!(events[0],0|1|16|17)}else{events[0]==0})
   &&events[1]==0&&events[2]==0&&events[3]==1&&events[4]==0&&events[6]==0
   &&(events[5]==0||(events[5]==1&&setup==cutoff))
 }
 fn deadline_proof(&self,cutoff:u64,setup:u64,spawn:&SpawnObservation)->Option<OriginalDeadlineProof>{
  if cutoff==0||setup==0||setup>cutoff||!spawn.complete(cutoff)||spawn.after_clone_ns>=cutoff
   ||self.result!= -1||self.reserved!=0{return None;}
  let before_valid=self.before_ns>=spawn.after_clone_ns&&self.before_ns>0;
  let before_live=before_valid&&self.before_ns<setup;
  let (site,sampled_ns)=if setup==cutoff&&before_valid&&self.before_ns>=cutoff&&self.call_errno==110&&self.empty_message()
   &&self.poll_result==c_int::MIN&&self.after_poll_result==c_int::MIN&&self.revents==[0;7]&&self.after_revents==[0;7]&&self.after_ns==0{
    (DeadlineSite::ReadyBeforeClock,Some(self.before_ns))
   }else if before_live&&self.call_errno==125&&self.empty_message()&&Self::timer_events(&self.revents,setup,cutoff,false)
    &&Self::exact_count(self.poll_result,&self.revents)&&self.after_poll_result==c_int::MIN&&self.after_revents==[0;7]&&self.after_ns==0{
    (DeadlineSite::ReadyFirstTimer,None)
   }else if before_live&&self.call_errno==125&&self.complete_message()&&self.first_ready_only()
    &&Self::timer_events(&self.after_revents,setup,cutoff,true)&&Self::exact_count(self.after_poll_result,&self.after_revents)&&self.after_ns==0{
    (DeadlineSite::ReadyAfterTimer,None)
   }else if setup==cutoff&&before_live&&self.call_errno==110&&self.complete_message()&&self.first_ready_only()
    &&matches!(self.after_revents[0],0|1|16|17)&&self.after_revents[1..].iter().all(|event|*event==0)
    &&Self::exact_count(self.after_poll_result,&self.after_revents)&&self.after_ns>=self.before_ns&&self.after_ns>=cutoff{
    (DeadlineSite::ReadyAfterClock,Some(self.after_ns))
   }else{return None;};
  Some(OriginalDeadlineProof{site,cutoff,sampled_ns})
 }
}

// The caller retains this real receipt alongside success OR ArmFailure through
// the existing014 consuming controller. None means no native call occurred.
// It contains an owned, pre-clone short readiness timer, not a replacement
// execution lease or a claim that dropping it settles any child/domain.
#[derive(Default)]
pub(super) struct ArmingReceipt {
    started: bool,
    abi: Option<[u64;21]>,
    original_deadline_ns: Option<u64>,
    setup: Option<Deadline>,
    spawn: Option<SpawnObservation>,
    readiness: Option<ReadinessObservation>,
    refused_kill: Option<KillObservation>,
    ready: bool,
    gate_error: Option<String>,
    ready_peer_cancel:Option<(c_int,c_int)>,
    primary_deadline:Option<OriginalDeadlineProof>,
}

impl ArmingReceipt {
    pub(super) fn has_primary_deadline(&self)->bool{self.primary_deadline.is_some()}
    pub(super) fn permits_timeout(&self)->bool{
        let Some(primary)=self.primary_deadline.as_ref() else{return false;};
        if !self.started||self.abi!=Some(expected_abi())||self.original_deadline_ns!=Some(primary.cutoff)||primary.cutoff==0
            ||self.ready||self.gate_error.is_some()||self.ready_peer_cancel.is_some()||self.parent_mask_uncertain(){return false;}
        if primary.site==DeadlineSite::PrepareClock{return self.setup.is_none()&&self.spawn.is_none()&&self.readiness.is_none()&&self.refused_kill.is_none()
            &&primary.sampled_ns.is_some_and(|sample|sample>=primary.cutoff);}
        let Some(setup)=self.setup.as_ref()else{return false;};
        if setup.effective_until_ns()==0||setup.effective_until_ns()>primary.cutoff||setup.armed_ceiling_ns()!=setup.effective_until_ns()||setup.narrow_observation().is_some()
            ||!self.refused_kill.is_some_and(|raw|raw.result==1&&raw.call_errno==0){return false;}
        let Some(spawn)=self.spawn.as_ref()else{return false;};
        let observed=match self.readiness.as_ref(){Some(ready)=>ready.deadline_proof(primary.cutoff,setup.effective_until_ns(),spawn),None=>spawn.deadline_proof(primary.cutoff)};
        observed.as_ref()==Some(primary)
    }
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
    if receipt.readiness.as_ref().is_some_and(ReadinessObservation::private_only_cancel){receipt.ready_peer_cancel=Some(controls.cancel_release_peer());}
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
    check(abi==expected_abi(),"native watchdog bridge ABI differs")?;
    check(watch.owner_pid>0,"native armer lacks original owner identity")?;
    receipt.original_deadline_ns=Some(watch.deadline_ns);
    check(watch.deadline_ns>0,"original arming cutoff is missing")?;
    let now=boottime_ns()?;
    if let Some(primary)=prepare_deadline_proof(now,watch.deadline_ns){
        receipt.primary_deadline=Some(primary);
        return Err("original deadline expired before native setup".into());
    }
    let setup_until=now.checked_add(READINESS_BOUND_NS).ok_or("readiness deadline overflow")?
        .min(watch.deadline_ns);
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
    controls.cancellation=Some(Arc::clone(watch.cancellation));
    controls.gate=match native_port::SharedGate::create() {
        Ok(value)=>Some(value),
        Err(error)=>{receipt.gate_error=Some(error);return Err(refuse_verified(controls,None,receipt,"shared cancellation allocation refused"));},
    };
    let mut raw=SpawnObservation::default();
    unsafe { ptg_arm_spawn_watchdog(controls.gate.as_ref().expect("installed shared gate").as_ptr(),&watch.owners.watch(),watch.timer.as_raw_fd(),
        watch.supervisor.as_raw_fd(),watch.cancellation.fd(),controls.watchdog_kill.as_raw_fd(),writer.as_raw_fd(),
        watch.deadline_ns,&mut raw) };
    // Trusted C's positive-clone contract: CLONE_PIDFD installed this new fd
    // with the same positive direct-child PID. Capture ownership immediately,
    // BEFORE interpreting any post-clone clock/mask/readiness error. No numeric
    // pidfd_open or wait can reopen/reassign that child's identity.
    let watchdog=if raw.child_created==1 {
        Some(unsafe {DirectChild::from_successful_clone(raw.pid,OwnedFd::from_raw_fd(raw.pidfd))})
    } else {None};
    receipt.spawn=Some(raw);
    receipt.primary_deadline=raw.deadline_proof(watch.deadline_ns);
    drop(writer); // child owns the only remaining readiness writer after scrub.
    if raw.child_created!=1 {
        return Err(refuse_verified(controls,watchdog,receipt,"native watchdog clone refused"));
    }
    if raw.schema!=4 || raw.stage!=4 || raw.pid<=0 || raw.pidfd<0 || raw.clone_result!=raw.pid
        || raw.call_errno!=0 || raw.mask_restore_result!=0 || raw.mask_restore_errno!=0
        || raw.mask_set_result!=0 || raw.mask_set_errno!=0 || raw.mask_query_result!=0 || raw.mask_query_errno!=0
        || raw.blocked_mask!=BLOCKED_KERNEL_MASK || raw.mask_restore_query_result!=0 || raw.mask_restore_query_errno!=0
        || raw.restored_mask!=raw.original_signal_mask || raw.failure_kill_result!=c_int::MIN
        || raw.initial_poll_result!=0 || raw.initial_revents!=[0;3] || raw.cancellation_revents!=0
        || raw.before_clone_ns==0 || raw.after_clone_ns<raw.before_clone_ns
        || raw.after_clone_ns>=watch.deadline_ns {
        return Err(refuse_verified(controls,watchdog,receipt,"native watchdog post-clone state refused"));
    }
    let watchdog=watchdog.expect("positive native clone was retained");
    let setup=receipt.setup.as_ref().expect("readiness timer precedes provision");
    let mut ready=ReadinessObservation::default();
    unsafe {ptg_arm_wait_ready(reader.as_raw_fd(),watchdog.pidfd.as_raw_fd(),
        &watch.owners.watch(),watch.timer.as_raw_fd(),watch.supervisor.as_raw_fd(),watch.cancellation.fd(),
        setup.timer_fd(),setup.effective_until_ns(),&mut ready)};
    receipt.readiness=Some(ready);
    receipt.primary_deadline=ready.deadline_proof(watch.deadline_ns,setup.effective_until_ns(),&raw);
    if ready.result!=0 || ready.call_errno!=0 || ready.read_count!=16 || ready.token!=u32::from(b'R')
        || ready.message_schema!=4 || ready.child_mask!=BLOCKED_KERNEL_MASK
        || ready.before_ns<raw.after_clone_ns || ready.after_ns<ready.before_ns
        || ready.after_ns>=setup.effective_until_ns() {
        return Err(refuse_verified(controls,Some(watchdog),receipt,"native watchdog readiness refused"));
    }
    receipt.ready=true;
    Ok(ArmedDomain{controls,watchdog,arming:receipt})
}

pub(super) unsafe fn kill_held(fd:c_int,out:*mut KillObservation) {unsafe{ptg_arm_kill_held(fd,out)}}

#[cfg(test)]pub(super) fn deadline_refusal_regression(){
 const CUTOFF:u64=100;
 assert!(prepare_deadline_proof(100,CUTOFF).is_some());assert!(prepare_deadline_proof(99,CUTOFF).is_none());assert!(prepare_deadline_proof(100,0).is_none());
 let untouched=SpawnObservation{schema:4,stage:1,pid:-1,pidfd:-1,clone_result:c_int::MIN,mask_set_result:c_int::MIN,mask_query_result:c_int::MIN,
  mask_restore_result:c_int::MIN,mask_restore_query_result:c_int::MIN,failure_kill_result:c_int::MIN,..SpawnObservation::default()};
 let timer=SpawnObservation{call_errno:125,initial_poll_result:1,initial_revents:[0,1,0],failure_kill_result:1,..untouched};
 let before=SpawnObservation{call_errno:110,before_clone_ns:CUTOFF,..untouched};
 let after=SpawnObservation{schema:4,stage:4,child_created:1,pid:123,pidfd:9,clone_result:123,original_signal_mask:2,blocked_mask:BLOCKED_KERNEL_MASK,restored_mask:2,
  before_clone_ns:10,after_clone_ns:CUTOFF,failure_kill_result:c_int::MIN,..SpawnObservation::default()};
 for raw in [timer,before,after]{assert!(raw.deadline_proof(CUTOFF).is_some());}
 macro_rules! reject_spawn{($base:expr,$field:ident,$value:expr)=>{{let mut raw=$base;raw.$field=$value;assert!(raw.deadline_proof(CUTOFF).is_none(),stringify!($field));}}}
 reject_spawn!(timer,schema,5);reject_spawn!(timer,stage,2);reject_spawn!(timer,child_created,1);reject_spawn!(timer,pid,1);reject_spawn!(timer,pidfd,0);
 reject_spawn!(timer,call_errno,110);reject_spawn!(timer,initial_poll_result,2);reject_spawn!(timer,clone_result,-1);reject_spawn!(timer,cancellation_revents,1);
 reject_spawn!(timer,mask_set_result,0);reject_spawn!(timer,mask_set_errno,4);reject_spawn!(timer,mask_query_result,0);reject_spawn!(timer,mask_query_errno,4);
 reject_spawn!(timer,mask_restore_result,0);reject_spawn!(timer,mask_restore_errno,4);reject_spawn!(timer,mask_restore_query_result,0);reject_spawn!(timer,mask_restore_query_errno,4);
 reject_spawn!(timer,original_signal_mask,1);reject_spawn!(timer,blocked_mask,1);reject_spawn!(timer,restored_mask,1);reject_spawn!(timer,before_clone_ns,1);reject_spawn!(timer,after_clone_ns,1);
 reject_spawn!(timer,failure_kill_result,0);reject_spawn!(timer,failure_kill_errno,5);
 for i in [0,2]{let mut raw=timer;raw.initial_revents[i]=1;raw.initial_poll_result=2;assert!(raw.deadline_proof(CUTOFF).is_none());}
 reject_spawn!(before,before_clone_ns,0);reject_spawn!(before,before_clone_ns,99);reject_spawn!(before,stage,2);reject_spawn!(before,clone_result,-1);
 reject_spawn!(before,call_errno,5);reject_spawn!(before,initial_poll_result,1);reject_spawn!(before,failure_kill_result,1);
 reject_spawn!(after,schema,5);reject_spawn!(after,stage,3);reject_spawn!(after,child_created,0);reject_spawn!(after,pid,0);reject_spawn!(after,pidfd,-1);
 reject_spawn!(after,clone_result,-1);reject_spawn!(after,call_errno,110);reject_spawn!(after,mask_set_result,-1);reject_spawn!(after,mask_set_errno,4);
 reject_spawn!(after,mask_query_result,-1);reject_spawn!(after,mask_query_errno,4);reject_spawn!(after,mask_restore_result,-1);reject_spawn!(after,mask_restore_errno,4);
 reject_spawn!(after,mask_restore_query_result,-1);reject_spawn!(after,mask_restore_query_errno,4);reject_spawn!(after,blocked_mask,0);reject_spawn!(after,restored_mask,0);
 reject_spawn!(after,original_signal_mask,u64::MAX);reject_spawn!(after,failure_kill_result,1);reject_spawn!(after,failure_kill_errno,5);
 reject_spawn!(after,initial_poll_result,1);reject_spawn!(after,cancellation_revents,1);reject_spawn!(after,before_clone_ns,CUTOFF);reject_spawn!(after,after_clone_ns,9);
 let spawn=SpawnObservation{after_clone_ns:20,..after};
 let pre=ReadinessObservation{result:-1,call_errno:110,read_count:-1,poll_result:c_int::MIN,after_poll_result:c_int::MIN,before_ns:CUTOFF,..ReadinessObservation::default()};
 let first=ReadinessObservation{call_errno:125,poll_result:2,revents:[0,0,0,1,0,1,0],before_ns:30,..pre};
 let post=ReadinessObservation{result:-1,call_errno:125,read_count:16,token:82,poll_result:1,revents:[17,0,0,0,0,0,0],after_poll_result:3,after_revents:[16,0,0,1,0,1,0],before_ns:30,message_schema:4,child_mask:BLOCKED_KERNEL_MASK,..ReadinessObservation::default()};
 let final_clock=ReadinessObservation{call_errno:110,after_poll_result:1,after_revents:[16,0,0,0,0,0,0],after_ns:CUTOFF,..post};
 for raw in [pre,first,post,final_clock]{assert!(raw.deadline_proof(CUTOFF,CUTOFF,&spawn).is_some());}
 assert!(pre.deadline_proof(CUTOFF,90,&spawn).is_none());assert!(final_clock.deadline_proof(CUTOFF,90,&spawn).is_none());
 macro_rules! reject_ready{($base:expr,$field:ident,$value:expr)=>{{let mut raw=$base;raw.$field=$value;assert!(raw.deadline_proof(CUTOFF,CUTOFF,&spawn).is_none(),stringify!($field));}}}
 reject_ready!(pre,result,0);reject_ready!(pre,call_errno,5);reject_ready!(pre,read_count,0);reject_ready!(pre,token,82);reject_ready!(pre,message_schema,4);
 reject_ready!(pre,reserved,1);reject_ready!(pre,child_mask,BLOCKED_KERNEL_MASK);reject_ready!(pre,poll_result,0);reject_ready!(pre,after_poll_result,0);reject_ready!(pre,before_ns,19);reject_ready!(pre,after_ns,1);
 reject_ready!(first,poll_result,1);reject_ready!(first,after_poll_result,0);reject_ready!(first,read_count,16);reject_ready!(first,after_ns,1);reject_ready!(first,before_ns,CUTOFF);
 let setup_only=ReadinessObservation{poll_result:1,revents:[0,0,0,0,0,1,0],..first};assert!(setup_only.deadline_proof(CUTOFF,CUTOFF,&spawn).is_none());
 for i in [0,1,2,4,6]{let mut raw=first;raw.revents[i]=1;raw.poll_result+=1;assert!(raw.deadline_proof(CUTOFF,CUTOFF,&spawn).is_none());}
 for i in 0..7{let mut raw=pre;raw.revents[i]=1;assert!(raw.deadline_proof(CUTOFF,CUTOFF,&spawn).is_none());raw=pre;raw.after_revents[i]=1;assert!(raw.deadline_proof(CUTOFF,CUTOFF,&spawn).is_none());}
 reject_ready!(post,poll_result,2);reject_ready!(post,after_poll_result,2);reject_ready!(post,read_count,8);reject_ready!(post,token,0);reject_ready!(post,message_schema,3);
 reject_ready!(post,child_mask,0);reject_ready!(post,reserved,1);reject_ready!(post,after_ns,1);reject_ready!(post,call_errno,110);
 for i in [1,2,4,6]{let mut raw=post;raw.after_revents[i]=1;raw.after_poll_result+=1;assert!(raw.deadline_proof(CUTOFF,CUTOFF,&spawn).is_none());}
 for value in [0,1,16,17]{let raw=ReadinessObservation{after_poll_result:1+c_int::from(value!=0),after_revents:[value,0,0,1,0,0,0],..post};assert!(raw.deadline_proof(CUTOFF,CUTOFF,&spawn).is_some());}
 reject_ready!(final_clock,after_ns,0);reject_ready!(final_clock,after_ns,99);reject_ready!(final_clock,after_poll_result,0);reject_ready!(final_clock,before_ns,19);
 for i in 1..7{let mut raw=final_clock;raw.after_revents[i]=1;raw.after_poll_result+=1;assert!(raw.deadline_proof(CUTOFF,CUTOFF,&spawn).is_none());}
 let native_fault=SpawnObservation{call_errno:5,..spawn};for raw in [pre,first,post,final_clock]{assert!(raw.deadline_proof(CUTOFF,CUTOFF,&native_fault).is_none());}
 println!("arming_original_deadline exact_rows=8 raw_shape_count_mask_message_clock_vetoes=true no_native_calls=true");
}
