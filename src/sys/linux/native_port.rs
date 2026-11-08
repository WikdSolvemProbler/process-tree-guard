// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Concrete native one-shot port. Product selector deliberately remains disabled.
use super::*;
use std::ffi::{c_char, CString};
use std::ptr::NonNull;
use super::cgroup_authority::settlement::Deadline;

const BLOCKED_MASK:u64=u64::MAX & !(1u64<<8) & !(1u64<<18);
#[repr(C)]
struct PayloadSpec {
    schema:u32, bytes:u32,
    cgroup_fd:c_int, kill_fd:c_int, owners:OwnerWatch, timer_fd:c_int, supervisor_fd:c_int, watchdog_fd:c_int,
    cwd_fd:c_int, stdio_fd:[c_int;3], permit_reader:c_int, ready_writer:c_int, exec_writer:c_int,
    uid:u32, gid:u32, group_count:u32, argc:u32, envc:u32,umask:u32,
    deadline_ns:u64, original_mask:u64,
    groups:*const u32, program:*const c_char, argv:*const *const c_char, envp:*const *const c_char,
    gate:*mut c_void,
}
#[repr(C)]
#[derive(Clone,Copy,Debug,Default)]
struct Spawn {
    schema:u32,stage:u32,child_created:c_int,pid:c_int,pidfd:c_int,error:c_int,
    clone_result:c_int,initial_poll_result:c_int,initial_revents:[c_int;4],
    mask_set_result:c_int,mask_set_error:c_int,mask_restore_result:c_int,mask_restore_error:c_int,
    kill_result:c_int,kill_error:c_int,scratch_unmap_result:c_int,scratch_unmap_error:c_int,
    original_mask:u64,blocked_mask:u64,restored_mask:u64,before_clone_ns:u64,after_clone_ns:u64,
    gate_state:c_int,reserved:c_int,
}
#[derive(Clone,Copy,Debug)]
pub(super) struct ParentMaskObservation {
    set_result:c_int,set_error:c_int,restore_result:c_int,restore_error:c_int,
    expected:u64,original:u64,blocked:u64,restored:u64,
}
impl ParentMaskObservation {
    fn from_spawn(raw:Spawn,expected:u64)->Self {Self{set_result:raw.mask_set_result,set_error:raw.mask_set_error,
        restore_result:raw.mask_restore_result,restore_error:raw.mask_restore_error,expected,
        original:raw.original_mask,blocked:raw.blocked_mask,restored:raw.restored_mask}}
    pub(super) fn uncertain(&self)->bool {self.set_result!=c_int::MIN && !self.confirmed()}
    pub(super) fn confirmed(&self)->bool {
        self.set_result==0 && self.set_error==0 && self.restore_result==0 && self.restore_error==0
            && self.blocked==BLOCKED_MASK && self.original==self.expected && self.restored==self.original
    }
}
#[repr(C)]
#[derive(Clone,Copy,Debug,Default)]
struct Ready {
    schema:u32,stage:u32,result:c_int,error:c_int,read_count:c_int,poll_result:c_int,revents:[c_int;7],
    message_schema:u32,message_token:u32,child_mask:u64,before_ns:u64,after_ns:u64,
    kill_result:c_int,kill_error:c_int,
}
#[repr(C)]
#[derive(Clone,Copy,Debug,Default)]
struct Release {
    schema:u32,stage:u32,result:c_int,error:c_int,poll_result:c_int,revents:[c_int;5],
    gate_before:c_int,gate_after:c_int,write_result:c_int,write_error:c_int,kill_result:c_int,kill_error:c_int,
    before_ns:u64,after_ns:u64,
}
const _:()={
    assert!(std::mem::offset_of!(PayloadSpec,umask)==92);
    assert!(size_of::<PayloadSpec>()==152 && std::mem::offset_of!(PayloadSpec,gate)==144 && std::mem::offset_of!(PayloadSpec,owners)==16);
    assert!(size_of::<Spawn>()==128 && size_of::<Ready>()==96 && size_of::<Release>()==80);
};
unsafe extern "C" {
    #[link_name="socketpair"]
    fn permit_socketpair(domain:c_int,kind:c_int,protocol:c_int,fds:*mut c_int)->c_int;
    fn ptg_payload_abi(word:u32)->u64;
    fn ptg_payload_gate_create(error:*mut c_int)->*mut c_void;
    fn ptg_payload_gate_state(gate:*const c_void)->c_int;
    fn ptg_payload_gate_cancel(gate:*mut c_void)->c_int;
    fn ptg_payload_gate_cancel_peer(gate:*mut c_void)->c_int;
    #[cfg(test)]fn ptg_payload_gate_commit(gate:*mut c_void)->c_int;
    fn ptg_payload_gate_peer_cancelled(gate:*const c_void)->c_int;
    fn ptg_payload_gate_destroy(gate:*mut c_void,error:*mut c_int)->c_int;
    fn ptg_payload_spawn(spec:*const PayloadSpec,out:*mut Spawn);
    fn ptg_payload_wait_ready(spec:*const PayloadSpec,reader:c_int,child:c_int,setup:c_int,until:u64,out:*mut Ready);
    fn ptg_payload_commit(spec:*const PayloadSpec,child:c_int,permit:c_int,out:*mut Release);
}

// No Drop cleanup promise: the actual mapping stays with controls/quarantine,
// or the terminal receipt after all direct/adopted children are reaped.
#[must_use="shared cancellation mapping must remain owned through all child settlement"]
pub(super) struct SharedGate { pointer:NonNull<c_void> }
impl SharedGate {
    pub(super) fn create()->Probe<Self> {
        let mut error=0;
        let pointer=unsafe{ptg_payload_gate_create(&mut error)};
        match NonNull::new(pointer) {
            Some(pointer) if error==0=>Ok(Self{pointer}),
            // C create contract: failure returns NULL and no live mapping.
            _=>Err(format!("shared gate allocation refused: errno={error}")),
        }
    }
    pub(super) fn as_ptr(&self)->*mut c_void {self.pointer.as_ptr()}
    fn state(&self)->c_int {unsafe{ptg_payload_gate_state(self.as_ptr())}}
    pub(super) fn cancel(&self)->c_int {unsafe{ptg_payload_gate_cancel(self.as_ptr())}}
    pub(super) fn cancel_peer(&self)->c_int{unsafe{ptg_payload_gate_cancel_peer(self.as_ptr())}}
    pub(super) fn peer_cancelled(&self)->c_int{unsafe{ptg_payload_gate_peer_cancelled(self.as_ptr())}}
    // Only the consuming terminal controller may call after exact reaping.
    pub(super) unsafe fn destroy(self)->std::result::Result<(),(Self,c_int,c_int)> {
        let mut error=0;
        let result=unsafe{ptg_payload_gate_destroy(self.as_ptr(),&mut error)};
        if result==0 && error==0 {Ok(())} else {Err((self,result,error))}
    }
}
impl DomainControls {
    pub(super) fn cancel_release_peer(&self)->(c_int,c_int){let facts=self.gate.as_ref().map_or((-1,-1),|gate|(gate.cancel_peer(),gate.peer_cancelled()));if self.peer_cancel.get().is_none(){self.peer_cancel.set(Some(facts));}facts}
    pub(super) fn peer_cancelled(&self)->c_int{self.gate.as_ref().map_or(-1,SharedGate::peer_cancelled)}
    pub(super) fn cancel_release(&self) {
        if let Some(gate)=self.gate.as_ref() {
            let previous=gate.cancel();
            if self.gate_cancel.get().is_none(){self.gate_cancel.set(Some(previous));}
        }
        // Gate cancellation is not a substitute for the original held kill.
    }
}

// Private context is minted ONLY from the authenticated exact original-owner/IO-right bundle.
// No public raw-fd constructor and no privileged ambient context substitution.
pub(in super::super) struct LaunchContext {
    caller:Credentials,umask:u16,cancellation:Arc<super::super::super::entry::AuthenticatedCancellation>, stdio:[OwnedFd;3],cwd:OwnedFd,
    environment:Vec<CString>,envp:Vec<*const c_char>,
}
impl LaunchContext {
    pub(in super::super) fn from_received(caller:Credentials,io:CallerIo,environment:Vec<CString>,umask:u16,cancellation:Arc<super::super::super::entry::AuthenticatedCancellation>)->Self {
        let CallerIo{stdio,cwd}=io;
        let mut envp:Vec<_>=environment.iter().map(|item|item.as_ptr()).collect();
        envp.push(ptr::null());
        Self{caller,umask,cancellation,stdio,cwd,environment,envp}
    }
}
// Read-only host preconditions, never a sysctl write or ptrace workaround.
// The trusted privileged entry must establish its root/namespace/origin policy;
// these actual checks additionally refuse an observed tracer or permissive
// suid-dumpability transition before any cgroup/child resource is created.
#[derive(Default)]
struct ActivationHostReceipt {suid_dumpable:Option<Vec<u8>>,status:Option<String>,confirmed:bool}
pub(in super::super) fn bounded_host_read(path:&str,cap:usize)->Probe<Vec<u8>> {
    use std::io::Read;
    let file=io_result(fs::File::open(path))?;
    let mut bytes=Vec::new();
    io_result(file.take(cap as u64+1).read_to_end(&mut bytes))?;
    check(bytes.len()<=cap,"activation policy read exceeds bound")?;
    Ok(bytes)
}
impl ActivationHostReceipt {
    fn check(&mut self)->Probe<()> {
        self.suid_dumpable=Some(bounded_host_read("/proc/sys/fs/suid_dumpable",64)?);
        let raw=self.suid_dumpable.as_ref().expect("actual sysctl bytes stored");
        check(raw.as_slice()==b"0\n" || raw.as_slice()==b"0","activation requires fs.suid_dumpable=0")?;
        self.status=Some(String::from_utf8(bounded_host_read("/proc/self/status",65536)?)
            .map_err(|_|"activation status is not UTF-8")?);
        let status=self.status.as_ref().expect("actual status stored");
        let uid=status_ids(status,"Uid:")?;
        let gid=status_ids(status,"Gid:")?;
        check(uid==[0,0,0,0] && gid==[0,0,0,0]
            && unsafe{geteuid()}==0 && unsafe{getegid()}==0,"activation requires trusted real/effective/saved/filesystem root credentials")?;
        check(status_field(status,"TracerPid:")?=="0","activation refuses a traced privileged entry")?;
        self.confirmed=true;Ok(())
    }
}
struct OriginalWatch { owners:Arc<VerifiedInvocation>,timer:OwnedFd,supervisor:OwnedFd,deadline_ns:u64 }
fn permit_pair()->Probe<(OwnedFd,OwnedFd)> {
    let mut fds=[-1;2];
    native(os_result(unsafe{permit_socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_NONBLOCK|SOCK_CLOEXEC,0,fds.as_mut_ptr())} as isize))?;
    let [child,parent]=fds.map(|fd|unsafe{OwnedFd::from_raw_fd(fd)});
    Ok((child,parent))
}
fn duplicate(fd:&OwnedFd)->Probe<OwnedFd> {
    fd_from_call(unsafe{fcntl(fd.as_raw_fd(),F_DUPFD_CLOEXEC,3 as c_int)})
}
#[derive(Default)]
struct PayloadReceipt {cancellation_authorize:Option<super::super::super::entry::CancellationObservation>,abi:Option<[u64;18]>,setup:Option<Deadline>,spawn:Option<Spawn>,ready:Option<Ready>,release:Option<Release>,refused_kill:Option<native_arming::KillObservation>,expected_mask:Option<u64>}
struct NativePort {
    scope:VerifiedServiceScope,
    launch:LaunchContext, watch:Option<OriginalWatch>, receipt:PayloadReceipt, host_policy:ActivationHostReceipt,
    arguments:Vec<CString>, argv:Vec<*const c_char>, groups:Vec<u32>, uid:u32,gid:u32,
}
impl NativePort {
    fn new(launch:LaunchContext,scope:VerifiedServiceScope)->Self {
        Self{scope,launch,watch:None,receipt:PayloadReceipt::default(),host_policy:ActivationHostReceipt::default(),arguments:Vec::new(),argv:Vec::new(),groups:Vec::new(),uid:0,gid:0}
    }
    fn spec(&self,domain:&ArmedDomain,permit:c_int,ready:c_int,exec:c_int)->Probe<PayloadSpec> {
        let watch=self.watch.as_ref().ok_or("missing original retained watch")?;
        let gate=domain.controls.gate.as_ref().ok_or("missing shared cancellation authority")?;
        Ok(PayloadSpec{
            schema:3,bytes:size_of::<PayloadSpec>() as u32,cgroup_fd:domain.controls.directory.as_raw_fd(),
            kill_fd:domain.controls.supervisor_kill.as_raw_fd(),owners:watch.owners.watch(),timer_fd:watch.timer.as_raw_fd(),
            supervisor_fd:watch.supervisor.as_raw_fd(),watchdog_fd:domain.watchdog.pidfd.as_raw_fd(),
            cwd_fd:self.launch.cwd.as_raw_fd(),stdio_fd:self.launch.stdio.each_ref().map(AsRawFd::as_raw_fd),
            permit_reader:permit,ready_writer:ready,exec_writer:exec,uid:self.uid,gid:self.gid,
            group_count:self.groups.len() as u32,argc:self.arguments.len() as u32,envc:self.launch.environment.len() as u32,umask:u32::from(self.launch.umask),
            deadline_ns:watch.deadline_ns,original_mask:domain.arming.original_mask().ok_or("watchdog lacks original raw mask proof")?,
            groups:self.groups.as_ptr(),program:self.arguments.first().ok_or("missing authorized program")?.as_ptr(),
            argv:self.argv.as_ptr(),envp:self.launch.envp.as_ptr(),gate:gate.as_ptr(),
        })
    }
    fn cancel(&mut self,domain:&ArmedDomain) {
        domain.controls.cancel_release();
        let mut raw=native_arming::KillObservation::default();
        unsafe{native_arming::kill_held(domain.controls.supervisor_kill.as_raw_fd(),&mut raw)};
        self.receipt.refused_kill=Some(raw);
        // The owning terminal controller repeats/records mandatory settlement;
        // every C-path refusal additionally retains its own raw kill observation.
    }
}
impl OneShotPort for NativePort {
    fn caller_cancellation(&self)->&Arc<super::super::super::entry::AuthenticatedCancellation>{&self.launch.cancellation}
    fn authorize(&mut self,identity:&VerifiedIdentity,exec:&request::ExecPlan)->Probe<()> {
        let cancellation=self.launch.cancellation.sample();self.receipt.cancellation_authorize=Some(cancellation);
        check(cancellation.state()==super::super::super::entry::CancellationState::Clear,"caller cancellation observed before arming")?;
        self.scope.revalidate()?;
        self.scope.authorize_original(exec.invocation())?;
        self.host_policy.check()?;
        check(self.launch.cancellation.peer()==self.launch.caller,"caller cancellation capability differs from authenticated launch")?;
        check(self.launch.caller.uid==identity.uid && self.launch.caller.gid==identity.gid
            && exec.environment()==self.launch.environment&&exec.invocation().umask==self.launch.umask,"authenticated launch context differs from prepared identity/environment")?;
        check(identity.uid!=0 && identity.gid!=0 && !identity.groups.contains(&0),"native payload refuses privileged identity")?;
        check(exec.program().to_bytes().starts_with(b"/"),"native payload requires an absolute program")?;
        let abi=std::array::from_fn(|i|unsafe{ptg_payload_abi(i as u32)});
        self.receipt.abi=Some(abi);
        check(abi==[3,152,128,96,80,16,8,CLONE_PIDFD|CLONE_INTO_CGROUP,SIGCHLD,435,7,BLOCKED_MASK,144,16,249,16,92,511],"payload C/Rust bridge ABI differs")?;
        self.arguments=exec.arguments().to_vec();
        self.argv=self.arguments.iter().map(|value|value.as_ptr()).chain(std::iter::once(ptr::null())).collect();
        self.groups=identity.groups.clone();self.uid=identity.uid;self.gid=identity.gid;
        Ok(())
    }
    fn arm(&mut self,watch:LeaseWatch<'_>)->std::result::Result<ArmedDomain,ArmFailure> {
        if watch.owner_pid!=self.launch.caller.pid {
            return Err(ArmFailure::BeforeProvision{error:"original owner differs from authenticated caller".into(),arming:native_arming::ArmingReceipt::default()});
        }
        let binding=(||->Probe<OriginalWatch>{Ok(OriginalWatch{owners:Arc::clone(watch.owners),timer:duplicate(watch.timer)?,
            supervisor:duplicate(watch.supervisor)?,deadline_ns:watch.deadline_ns})})();
        match binding {
            Ok(value)=>self.watch=Some(value),
            Err(error)=>return Err(ArmFailure::BeforeProvision{error,arming:native_arming::ArmingReceipt::default()}),
        }
        unsafe{native_arming::arm(watch,&self.scope)}
    }
    fn place_held(&mut self,domain:&ArmedDomain,child:ChildPlan<'_>)->Probe<PlacedChild> {
        check(domain.controls.belongs_to(&self.scope),"payload domain belongs to a different managed service scope")?;
        check(self.receipt.spawn.is_none(),"native port may place only one payload")?;
        check(child.directory.as_raw_fd()==domain.controls.directory.as_raw_fd()
            && child.identity.uid==self.uid && child.identity.gid==self.gid && child.identity.groups==self.groups
            && child.exec.arguments()==self.arguments,"placement differs from original authorized plan")?;
        let now=boottime_ns()?;
        let until=now.checked_add(1_000_000_000).ok_or("payload readiness cutoff overflow")?
            .min(self.watch.as_ref().ok_or("missing retained original watch")?.deadline_ns);
        self.receipt.setup=Some(Deadline::arm_before_placement(until)?);
        let (permit_reader,permit_writer)=permit_pair()?;
        let (ready_reader,ready_writer)=pipe()?;
        let (exec_reader,exec_writer)=pipe()?;
        let spec=self.spec(domain,permit_reader.as_raw_fd(),ready_writer.as_raw_fd(),exec_writer.as_raw_fd())?;
        self.receipt.expected_mask=Some(spec.original_mask);
        let mut raw=Spawn::default();
        unsafe{ptg_payload_spawn(&spec,&mut raw)};
        // No formatting/clock/readiness/decoding before actual clone ownership.
        let child=if raw.child_created==1 {
            Some(unsafe{DirectChild::from_successful_clone(raw.pid,OwnedFd::from_raw_fd(raw.pidfd))})
        } else {None};
        self.receipt.spawn=Some(raw);
        drop((permit_reader,ready_writer,exec_writer));
        let Some(child)=child else {self.cancel(domain);return Err("payload clone did not succeed".into());};
        let exec_report=unsafe{terminal_controller::ExecReport::from_trusted_child(exec_reader)};
        let ready_permit=(||->Probe<OwnedFd>{
            check(raw.schema==3 && raw.child_created==1 && raw.pid>0 && raw.pidfd>=0 && raw.clone_result==raw.pid
                && raw.error==0 && raw.mask_set_result==0 && raw.mask_set_error==0
                && raw.mask_restore_result==0 && raw.mask_restore_error==0
                && raw.blocked_mask==BLOCKED_MASK && raw.restored_mask==raw.original_mask
                && raw.original_mask==spec.original_mask && raw.after_clone_ns>=raw.before_clone_ns
                && raw.before_clone_ns>=now && raw.after_clone_ns<until,"payload post-clone state refused")?;
            let setup=self.receipt.setup.as_ref().expect("stored pre-clone readiness timer");
            let mut ready=Ready::default();
            unsafe{ptg_payload_wait_ready(&spec,ready_reader.as_raw_fd(),child.pidfd.as_raw_fd(),setup.timer_fd(),until,&mut ready)};
            self.receipt.ready=Some(ready);
            check(ready.schema==3 && ready.result==0 && ready.error==0 && ready.read_count==16
                && ready.message_schema==3 && ready.message_token==u32::from(b'R') && ready.child_mask==BLOCKED_MASK
                && ready.before_ns>=raw.after_clone_ns && ready.after_ns>=ready.before_ns && ready.after_ns<until,
                "payload readiness refused")?;
            Ok(permit_writer)
        })();
        if ready_permit.is_err(){self.cancel(domain);}
        Ok(PlacedChild{identity:child,ready_permit,exec_report})
    }
    fn payload_parent_mask(&self)->Option<ParentMaskObservation> {
        self.receipt.spawn.zip(self.receipt.expected_mask).map(|(raw,expected)|ParentMaskObservation::from_spawn(raw,expected))
    }
    fn commit_permit(&mut self,domain:&ArmedDomain,child:&DirectChild,permit:&OwnedFd)->Probe<()> {
        let spec=self.spec(domain,-1,-1,-1)?;
        let mut release=Release::default();
        unsafe{ptg_payload_commit(&spec,child.pidfd.as_raw_fd(),permit.as_raw_fd(),&mut release)};
        self.receipt.release=Some(release);
        let valid=release.schema==3 && release.result==0 && release.error==0 && release.gate_before==0
            && release.gate_after==1 && release.write_result==1 && release.write_error==0
            && release.before_ns>0 && release.after_ns>=release.before_ns && release.after_ns<spec.deadline_ns;
        if !valid {self.cancel(domain);return Err("payload release refused".into());}
        Ok(())
    }
}

enum Completion {
    RefusedBeforeActivation{error:String,code:i32,preparation:Option<PreparationFailure>},
    Terminal(std::result::Result<terminal_controller::SettledRun,terminal_controller::TerminalQuarantine>),
    Arming{settled:std::result::Result<arming_failure_controller::ArmingSettled,arming_failure_controller::ArmingQuarantine>,primary_decision:Option<Decision>,diagnostic_sample:Probe<DecisionReceipt>,error:String},
}
/// Owning outcome: preserves original plan/lease in the actual terminal or
/// arming owner, all payload/bootstrap observations, and the caller launch context.
/// No Debug/Display exposes argv/environment/credential plans.
#[must_use="inspect terminal versus quarantine; dropping an outcome is not cleanup"]
pub(crate) struct RunOutcome {completion:Completion,port:Option<NativePort>,early_scope:Option<VerifiedServiceScope>,early_cancellation:Option<Arc<super::super::super::entry::AuthenticatedCancellation>>,admission:NativeReceipt}

pub(in super::super) unsafe fn run_admitted(prepared:PreparedRun,launch:LaunchContext,scope:VerifiedServiceScope,admission:NativeReceipt)->RunOutcome {
    let mut port=NativePort::new(launch,scope);
    let completion=match unsafe{activate(prepared,&mut port)} {
        Ok(pending)=>Completion::Terminal(pending.run_to_terminal()),
        Err(failure)=>{
            // The native armer already attempted held kill on any verified-domain
            // failure. Only the typed ORIGINAL refusal boundary can classify
            // expiration; this later sample is diagnostic/invalidation only.
            let (prepared,primary_decision)=match &failure {
                ActivationFailure::BeforeArm{prepared,decision,..}=>(prepared,*decision),
                ActivationFailure::Arming{prepared,..}=>(prepared,None),
            };
            let diagnostic_sample=prepared.lease.sample();
            let error=match &failure{ActivationFailure::BeforeArm{error,..}=>error.clone(),ActivationFailure::Arming{failure,..}=>match failure{ArmFailure::BeforeProvision{error,..}|ArmFailure::VerifiedDomain{error,..}=>error.clone(),ArmFailure::Provision{failure,..}=>failure.error.clone()}};
            Completion::Arming{settled:failure.settle_arming_failure(),primary_decision,diagnostic_sample,error}
        },
    };
    RunOutcome{completion,port:Some(port),early_scope:None,early_cancellation:None,admission}
}
pub(in super::super) fn before_activation(launch:LaunchContext,scope:VerifiedServiceScope,admission:NativeReceipt,failure:PreparationFailure)->RunOutcome {
    let code=failure.code;let error=failure.error.clone();
    RunOutcome{completion:Completion::RefusedBeforeActivation{error,code,preparation:Some(failure)},port:Some(NativePort::new(launch,scope)),early_scope:None,early_cancellation:None,admission}
}

fn unknown_watchdog_stop(reasons:u32)->bool{reasons&8!=0&&reasons&(1|2|512)==0}
impl RunOutcome {
    pub(crate) fn settled(&self)->bool{matches!(self.completion,Completion::RefusedBeforeActivation{..}|Completion::Terminal(Ok(_))|Completion::Arming{settled:Ok(_),..})}
    pub(crate) fn validate_final_scope(&self)->Probe<()>{match self.port.as_ref(){Some(port)=>port.scope.revalidate(),None=>self.early_scope.as_ref().ok_or("missing entry scope")?.revalidate()}}
    /// Distinguish a genuine payload exit125 from a guard/setup/exec failure.
    /// This summary contains no command/environment/credential-plan contents.
    pub(crate) fn guard_error(&self)->Option<String>{
        match &self.completion{
            Completion::RefusedBeforeActivation{error,code,..}=>if *code==124{None}else{Some(error.clone())},
            Completion::Arming{error,..}=>if self.exit_code()==124{None}else{Some(format!("native arming refused: {error}"))},
            Completion::Terminal(Err(_))=>Some("native settlement is quarantined; owning failure retained".into()),
            Completion::Terminal(Ok(run))=>{
                let facts=run.facts();
                if facts.stop_reasons&1!=0{return Some("original invocation owner lost".into());}
                if facts.stop_reasons&512!=0{return Some("authenticated caller canceled the invocation".into());}
                if unknown_watchdog_stop(facts.stop_reasons){return Some("watchdog stopped without a primary verified owner/deadline/cancellation cause".into());}
                if let Some(terminal_controller::ExecState::ReportedFailure{errno})=facts.exec_report{return Some(format!("payload setup/exec failed with errno {errno}"));}
                if facts.stop_reasons&2!=0{return None;}
                match facts.release{Some(Err(error))=>return Some(format!("native release refused: {error}")),Some(Ok(Decision::Permit))=>{},_=>return Some("native release was not permitted".into())}
                match facts.exec_report{Some(terminal_controller::ExecState::ReportedFailure{errno})=>Some(format!("payload setup/exec failed with errno {errno}")),Some(terminal_controller::ExecState::NoExecErrorReport)=>if facts.payload_exit.is_some(){None}else{Some("payload exit receipt missing".into())},_=>Some("payload exec-report state unverified".into())}
            }
        }
    }
    /// Fail closed on every unconfirmed setup/release/exec/settlement path.
    /// This is internal return mapping, not a public enabled Linux CLI.
    pub(crate) fn exit_code(&self)->i32 {
        match &self.completion {
            Completion::RefusedBeforeActivation{code,..}=>return *code,
            Completion::Arming{settled:Ok(_),primary_decision:Some(Decision::Deadline),
                diagnostic_sample:Ok(DecisionReceipt{decision:Decision::Deadline,..}),..}=>return 124,
            _=>{},
        }
        let Completion::Terminal(Ok(run))=&self.completion else{return 125;};
        let facts=run.facts();
        if facts.stop_reasons & (1|512) != 0 {return 125;}
        if facts.stop_reasons&8!=0&&facts.stop_reasons&(1|2|512)==0{return 125;}
        if facts.stop_reasons & 2 != 0 {return 124;}
        if !matches!(facts.release,Some(Ok(Decision::Permit)))
            || facts.exec_report!=Some(terminal_controller::ExecState::NoExecErrorReport) {return 125;}
        match facts.payload_exit {
            Some(child_reaper::ExitOutcome::Exited{code})=>i32::from(code),
            Some(child_reaper::ExitOutcome::Signaled{signal,..})=>128+signal,
            None=>125,
        }
    }
}

pub(in super::super) fn before_admission(scope:Option<VerifiedServiceScope>,cancellation:Arc<super::super::super::entry::AuthenticatedCancellation>,admission:NativeReceipt,error:String)->RunOutcome {
    before_admission_code(scope,cancellation,admission,error,125)
}

pub(in super::super) fn before_admission_code(scope:Option<VerifiedServiceScope>,cancellation:Arc<super::super::super::entry::AuthenticatedCancellation>,admission:NativeReceipt,error:String,code:i32)->RunOutcome {
    RunOutcome{completion:Completion::RefusedBeforeActivation{error,code,preparation:None},port:None,early_scope:scope,early_cancellation:Some(cancellation),admission}
}

#[cfg(test)]pub(super) fn gate_first_cause_regression(){
 let observe=|released:bool,generic_first:bool|{
  let gate=SharedGate::create().expect("actual native gate allocation");let mut raw=[-9;8];
  raw[0]=gate.state();raw[1]=gate.peer_cancelled();
  raw[2]=if released{unsafe{ptg_payload_gate_commit(gate.as_ptr())}}else{-1};
  raw[3]=if generic_first{gate.cancel()}else{-1};raw[4]=gate.cancel_peer();raw[5]=gate.state();raw[6]=gate.peer_cancelled();
  raw[7]=gate.cancel();let after=gate.peer_cancelled();let commit_after=unsafe{ptg_payload_gate_commit(gate.as_ptr())};
  match unsafe{gate.destroy()}{Ok(())=>{},Err((_owner,result,errno))=>panic!("actual native gate unmap refused result={result} errno={errno}")};
  (raw,after,commit_after)
 };
 let held=observe(false,false);let released=observe(true,false);let generic=observe(false,true);
 assert_eq!(held,([0,0,-1,-1,0,2,1,2],1,2));
 assert_eq!(released,([0,0,0,-1,1,2,1,2],1,2));
 assert_eq!(generic,([0,0,-1,0,2,2,0,2],0,2));
 println!("native_gate_first_cause held_peer=preserved released_peer=preserved generic_first=not_relabelled gates=3 mappings_destroyed=3");
}

#[cfg(test)]pub(super) fn watchdog_mapping_regression(){for other in [0,4,32]{assert!(unknown_watchdog_stop(8|other));}for primary in [1,2,512]{assert!(!unknown_watchdog_stop(8|primary));}assert!(!unknown_watchdog_stop(4));}
