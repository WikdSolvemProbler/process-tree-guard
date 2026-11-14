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
    gate:*mut c_void,host_dumpability:u32,core_reserved:u32,
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
    assert!(std::mem::offset_of!(PayloadSpec,host_dumpability)==152 && std::mem::offset_of!(PayloadSpec,core_reserved)==156);
    assert!(size_of::<PayloadSpec>()==160 && std::mem::offset_of!(PayloadSpec,gate)==144 && std::mem::offset_of!(PayloadSpec,owners)==16);
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

fn sample_payload_abi()->[u64;27]{std::array::from_fn(|i|unsafe{ptg_payload_abi(i as u32)})}
fn require_payload_abi(abi:&[u64;27])->Probe<()>{check(*abi==[4,160,128,96,80,16,8,CLONE_PIDFD|CLONE_INTO_CGROUP,SIGCHLD,435,7,BLOCKED_MASK,144,16,249,16,92,511,152,156,5,16,0,8,4,SYS_PRLIMIT64 as u64,1],"payload C/Rust bridge ABI differs")}

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
// Every Rust generic cancellation preserves both first observation and the
// first result invalid during ARMING. State1 is normal after payload release,
// so this second fact is a veto only for arming timeout eligibility.
pub(super) fn arming_gate_cancel_valid(value:c_int)->bool{matches!(value,0|2)}
fn record_gate_cancel(first:Option<c_int>,invalid:Option<c_int>,value:c_int)->(Option<c_int>,Option<c_int>){
 (first.or(Some(value)),invalid.or_else(||(!arming_gate_cancel_valid(value)).then_some(value)))
}
impl DomainControls {
    pub(super) fn cancel_release_peer(&self)->(c_int,c_int){let facts=self.gate.as_ref().map_or((-1,-1),|gate|(gate.cancel_peer(),gate.peer_cancelled()));if self.peer_cancel.get().is_none(){self.peer_cancel.set(Some(facts));}facts}
    pub(super) fn peer_cancelled(&self)->c_int{self.gate.as_ref().map_or(-1,SharedGate::peer_cancelled)}
    pub(super) fn cancel_release(&self) {
        if let Some(gate)=self.gate.as_ref() {
            let previous=gate.cancel();
            let (first,invalid)=record_gate_cancel(self.gate_cancel.get(),self.gate_cancel_invalid.get(),previous);
            self.gate_cancel.set(first);self.gate_cancel_invalid.set(invalid);
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
// these checks select strict mode0 or a reviewed protected mode2 path before
// any cgroup/child resource. No request field selects this host authority.
#[derive(Default)]
struct ActivationHostReceipt {suid_dumpable:Option<Vec<u8>>,status:Option<String>,confirmed:bool,mode:Option<u32>,
    kernel_release:Option<Vec<u8>>,kernel_family:Option<ReviewedKernelFamily>,core:Option<CoreObservation>,
    image:Option<cgroup_authority::service_scope::ProtectedElfObservation>}
#[repr(C)]
#[derive(Clone,Copy,Debug,Default)]
struct CoreLimit {soft:u64,hard:u64}
#[derive(Debug)]
struct CoreObservation {result:c_long,error:c_int,limit:CoreLimit}
const _:()={assert!(size_of::<CoreLimit>()==16);assert!(std::mem::offset_of!(CoreLimit,soft)==0);assert!(std::mem::offset_of!(CoreLimit,hard)==8);};
#[cfg(target_arch="x86_64")]const SYS_PRLIMIT64:c_long=302;
#[cfg(target_arch="aarch64")]const SYS_PRLIMIT64:c_long=261;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum ReviewedKernelFamily {Linux5_14,Linux6_8}
fn host_mode(raw:&[u8])->Probe<u32>{match raw{b"0"|b"0\n"=>Ok(0),b"2"|b"2\n"=>Ok(2),_=>Err("host dumpability mode is not supported".into())}}
fn reviewed_mode2_family(raw:&[u8])->Probe<ReviewedKernelFamily>{
    let raw=raw.strip_suffix(b"\n").unwrap_or(raw);
    check(!raw.is_empty()&&raw.len()<=256&&raw.iter().all(|b|b.is_ascii_alphanumeric()||b".-+_".contains(b)),"kernel release syntax is not reviewed")?;
    let (family,tail)=if let Some(tail)=raw.strip_prefix(b"5.14."){(ReviewedKernelFamily::Linux5_14,tail)}else if let Some(tail)=raw.strip_prefix(b"6.8."){(ReviewedKernelFamily::Linux6_8,tail)}else{return Err("mode2 requires a reviewed Linux5.14 or6.8 family".into())};
    check(tail.first().is_some_and(u8::is_ascii_digit)&&!raw.windows(3).any(|part|part==b"-rc"),"unreviewed kernel development release")?;Ok(family)
}
fn valid_current_elf(raw:&[u8])->bool{
    if raw.len()!=64||&raw[..7]!=b"\x7fELF\x02\x01\x01"||!matches!(raw[7],0|3){return false;}
    let kind=u16::from_le_bytes([raw[16],raw[17]]);let machine=u16::from_le_bytes([raw[18],raw[19]]);
    let expected=if cfg!(target_arch="aarch64"){183}else{62};
    matches!(kind,2|3)&&machine==expected&&u32::from_le_bytes(raw[20..24].try_into().expect("fixed ELF range"))==1&&u16::from_le_bytes([raw[52],raw[53]])==64
}
fn mode2_core_permitted(hard:u64)->bool{hard>=1}

pub(in super::super) fn bounded_host_read(path:&str,cap:usize)->Probe<Vec<u8>> {
    use std::io::Read;
    let file=io_result(fs::File::open(path))?;
    let mut bytes=Vec::new();
    io_result(file.take(cap as u64+1).read_to_end(&mut bytes))?;
    check(bytes.len()<=cap,"activation policy read exceeds bound")?;
    Ok(bytes)
}
impl ActivationHostReceipt {
    fn mode(&self)->Probe<u32>{check(self.confirmed,"host policy was not confirmed")?;self.mode.ok_or_else(||"host dumpability observation missing".into())}
    fn check(&mut self,scope:&VerifiedServiceScope)->Probe<()> {
        self.confirmed=false;self.mode=None;
        self.suid_dumpable=Some(bounded_host_read("/proc/sys/fs/suid_dumpable",64)?);
        let mode=host_mode(self.suid_dumpable.as_ref().expect("actual sysctl bytes stored"))?;
        self.status=Some(String::from_utf8(bounded_host_read("/proc/self/status",65536)?)
            .map_err(|_|"activation status is not UTF-8")?);
        let status=self.status.as_ref().expect("actual status stored");
        let uid=status_ids(status,"Uid:")?;let gid=status_ids(status,"Gid:")?;
        check(uid==[0,0,0,0]&&gid==[0,0,0,0]&&unsafe{geteuid()}==0&&unsafe{getegid()}==0,
            "activation requires trusted real/effective/saved/filesystem root credentials")?;
        check(status_field(status,"TracerPid:")?=="0","activation refuses a traced privileged entry")?;
        if mode==2{
            self.kernel_release=Some(bounded_host_read("/proc/sys/kernel/osrelease",256)?);
            self.kernel_family=Some(reviewed_mode2_family(self.kernel_release.as_ref().expect("kernel bytes stored"))?);
            self.image=Some(scope.protected_current_elf()?);
            check(valid_current_elf(&self.image.as_ref().expect("protected image stored").header),"mode2 requires the actual protected current LP64 ELF")?;
            let mut limit=CoreLimit::default();let result=unsafe{syscall(SYS_PRLIMIT64,0 as c_int,4 as c_int,std::ptr::null::<CoreLimit>(),&mut limit as *mut CoreLimit)};
            let error=if result<0{io::Error::last_os_error().raw_os_error().unwrap_or(-1)}else{0};
            self.core=Some(CoreObservation{result,error,limit});
            let observed=self.core.as_ref().expect("actual CORE observation stored");
            check(observed.result==0&&observed.error==0&&observed.limit.soft<=observed.limit.hard&&mode2_core_permitted(observed.limit.hard),"mode2 requires a readable current hard CORE limit of at least1")?;
        }
        self.mode=Some(mode);self.confirmed=true;Ok(())
    }
}
#[cfg(test)]pub(crate) fn mode2_host_policy_regression(){
    for raw in [b"0".as_slice(),b"0\n".as_slice()]{assert_eq!(host_mode(raw).unwrap(),0);}
    for raw in [b"2".as_slice(),b"2\n".as_slice()]{assert_eq!(host_mode(raw).unwrap(),2);}
    for raw in [b"1".as_slice(),b"3".as_slice(),b"02".as_slice(),b"2\r\n".as_slice(),b"".as_slice()]{assert!(host_mode(raw).is_err());}
    assert_eq!(reviewed_mode2_family(b"5.14.0-reviewed\n").unwrap(),ReviewedKernelFamily::Linux5_14);
    assert_eq!(reviewed_mode2_family(b"6.8.0-reviewed\n").unwrap(),ReviewedKernelFamily::Linux6_8);
    for release in [b"5.13.0".as_slice(),b"5.140.0".as_slice(),b"6.80.0".as_slice(),b"6.9.0".as_slice(),b"6.8.0-rc1".as_slice(),b"6.8.".as_slice(),b"6.8.0\r\n".as_slice()]{assert!(reviewed_mode2_family(release).is_err());}
    assert!(!mode2_core_permitted(0));assert!(mode2_core_permitted(1));assert!(mode2_core_permitted(u64::MAX));
    let mut elf=[0u8;64];elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");elf[16..18].copy_from_slice(&3u16.to_le_bytes());let machine=if cfg!(target_arch="aarch64"){183u16}else{62u16};elf[18..20].copy_from_slice(&machine.to_le_bytes());elf[20..24].copy_from_slice(&1u32.to_le_bytes());elf[52..54].copy_from_slice(&64u16.to_le_bytes());assert!(valid_current_elf(&elf));
    for offset in [0,4,5,6,16,18,20,52]{let mut bad=elf;bad[offset]=0;assert!(!valid_current_elf(&bad));}
    assert!(!valid_current_elf(&elf[..63]));
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
struct PayloadReceipt {cancellation_authorize:Option<super::super::super::entry::CancellationObservation>,abi:Option<[u64;27]>,setup:Option<Deadline>,spawn:Option<Spawn>,ready:Option<Ready>,release:Option<Release>,refused_kill:Option<native_arming::KillObservation>,expected_mask:Option<u64>}
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
            schema:4,bytes:size_of::<PayloadSpec>() as u32,cgroup_fd:domain.controls.directory.as_raw_fd(),
            kill_fd:domain.controls.supervisor_kill.as_raw_fd(),owners:watch.owners.watch(),timer_fd:watch.timer.as_raw_fd(),
            supervisor_fd:watch.supervisor.as_raw_fd(),watchdog_fd:domain.watchdog.pidfd.as_raw_fd(),
            cwd_fd:self.launch.cwd.as_raw_fd(),stdio_fd:self.launch.stdio.each_ref().map(AsRawFd::as_raw_fd),
            permit_reader:permit,ready_writer:ready,exec_writer:exec,uid:self.uid,gid:self.gid,
            group_count:self.groups.len() as u32,argc:self.arguments.len() as u32,envc:self.launch.environment.len() as u32,umask:u32::from(self.launch.umask),
            deadline_ns:watch.deadline_ns,original_mask:domain.arming.original_mask().ok_or("watchdog lacks original raw mask proof")?,
            groups:self.groups.as_ptr(),program:self.arguments.first().ok_or("missing authorized program")?.as_ptr(),
            argv:self.argv.as_ptr(),envp:self.launch.envp.as_ptr(),gate:gate.as_ptr(),host_dumpability:self.host_policy.mode()?,core_reserved:0,
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
        self.scope.authorize_original(exec.invocation(),identity.uid)?;
        self.host_policy.check(&self.scope)?;
        check(self.launch.cancellation.peer()==self.launch.caller,"caller cancellation capability differs from authenticated launch")?;
        check(self.launch.caller.uid==identity.uid && self.launch.caller.gid==identity.gid
            && exec.environment()==self.launch.environment&&exec.invocation().umask==self.launch.umask,"authenticated launch context differs from prepared identity/environment")?;
        check(identity.uid!=0 && identity.gid!=0 && !identity.groups.contains(&0),"native payload refuses privileged identity")?;
        check(exec.program().to_bytes().starts_with(b"/"),"native payload requires an absolute program")?;
        let abi=sample_payload_abi();self.receipt.abi=Some(abi);require_payload_abi(&abi)?;
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
        self.scope.require_delegation(self.launch.caller.uid)?;
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
            check(raw.schema==4 && raw.child_created==1 && raw.pid>0 && raw.pidfd>=0 && raw.clone_result==raw.pid
                && raw.error==0 && raw.mask_set_result==0 && raw.mask_set_error==0
                && raw.mask_restore_result==0 && raw.mask_restore_error==0
                && raw.blocked_mask==BLOCKED_MASK && raw.restored_mask==raw.original_mask
                && raw.original_mask==spec.original_mask && raw.after_clone_ns>=raw.before_clone_ns
                && raw.before_clone_ns>=now && raw.after_clone_ns<until,"payload post-clone state refused")?;
            let setup=self.receipt.setup.as_ref().expect("stored pre-clone readiness timer");
            let mut ready=Ready::default();
            unsafe{ptg_payload_wait_ready(&spec,ready_reader.as_raw_fd(),child.pidfd.as_raw_fd(),setup.timer_fd(),until,&mut ready)};
            self.receipt.ready=Some(ready);
            check(ready.schema==4 && ready.result==0 && ready.error==0 && ready.read_count==16
                && ready.message_schema==4 && ready.message_token==u32::from(b'R') && ready.child_mask==BLOCKED_MASK
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
        if let Err(error)=self.scope.require_delegation(self.launch.caller.uid){self.cancel(domain);return Err(error);}
        let spec=self.spec(domain,-1,-1,-1)?;
        let mut release=Release::default();
        unsafe{ptg_payload_commit(&spec,child.pidfd.as_raw_fd(),permit.as_raw_fd(),&mut release)};
        self.receipt.release=Some(release);
        let valid=release.schema==4 && release.result==0 && release.error==0 && release.gate_before==0
            && release.gate_after==1 && release.write_result==1 && release.write_error==0
            && release.before_ns>0 && release.after_ns>=release.before_ns && release.after_ns<spec.deadline_ns;
        if !valid {self.cancel(domain);return Err("payload release refused".into());}
        Ok(())
    }
}

enum Completion {
    RefusedBeforeActivation{error:String,cause:PrelaunchCause,preparation:Option<PreparationFailure>},
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
                ActivationFailure::Arming{prepared,failure,..}=>(prepared,failure.has_primary_deadline().then_some(Decision::Deadline)),
            };
            let diagnostic_sample=prepared.lease.sample();
            let error=match &failure{ActivationFailure::BeforeArm{error,..}=>error.clone(),ActivationFailure::Arming{failure,..}=>match failure{ArmFailure::BeforeProvision{error,..}|ArmFailure::VerifiedDomain{error,..}=>error.clone(),ArmFailure::Provision{failure,..}=>failure.error.clone()}};
            Completion::Arming{settled:failure.settle_arming_failure(Arc::clone(&port.launch.cancellation)),primary_decision,diagnostic_sample,error}
        },
    };
    RunOutcome{completion,port:Some(port),early_scope:None,early_cancellation:None,admission}
}
pub(in super::super) fn before_activation(launch:LaunchContext,scope:VerifiedServiceScope,admission:NativeReceipt,failure:PreparationFailure)->RunOutcome {
    let cause=failure.cause;let error=failure.error.clone();
    RunOutcome{completion:Completion::RefusedBeforeActivation{error,cause,preparation:Some(failure)},port:Some(NativePort::new(launch,scope)),early_scope:None,early_cancellation:None,admission}
}

fn unknown_watchdog_stop(reasons:u32)->bool{reasons&8!=0&&reasons&(1|2|512)==0}
impl RunOutcome {
    pub(crate) fn settled(&self)->bool{matches!(self.completion,Completion::RefusedBeforeActivation{..}|Completion::Terminal(Ok(_))|Completion::Arming{settled:Ok(_),..})}
    pub(crate) fn validate_final_scope(&self)->Probe<()>{match self.port.as_ref(){Some(port)=>port.scope.revalidate(),None=>self.early_scope.as_ref().ok_or("missing entry scope")?.revalidate()}}
    /// One effective classification retains cause independently of payload codes.
    pub(crate) fn classify(&self)->RunClassification {
        match &self.completion {
            Completion::RefusedBeforeActivation{cause,error,..}=>classify_prelaunch(*cause,error),
            Completion::Arming{settled,primary_decision,diagnostic_sample,error}=>classify_arming(*primary_decision,diagnostic_sample,settled.as_ref().is_ok_and(|value|value.permits_timeout()),error),
            Completion::Terminal(Err(_))=>RunClassification::failure("native settlement is quarantined; owning failure retained".into()),
            Completion::Terminal(Ok(run))=>{let facts=run.facts();classify_terminal(facts.stop_reasons,facts.release,facts.exec_report,facts.payload_exit)},
        }
    }
    pub(crate) fn guard_error(&self)->Option<String>{self.classify().error}
    pub(crate) fn exit_code(&self)->i32{self.classify().code}
}

fn classify_prelaunch(cause:PrelaunchCause,error:&str)->RunClassification{match cause{PrelaunchCause::OriginalDeadline=>RunClassification::deadline(TimeoutCause::PreLaunch),PrelaunchCause::Failure=>RunClassification::failure(error.into())}}
fn classify_arming(primary:Option<Decision>,diagnostic:&Probe<DecisionReceipt>,settled_permits:bool,error:&str)->RunClassification{
 if primary==Some(Decision::Deadline)&&matches!(diagnostic,Ok(DecisionReceipt{decision:Decision::Deadline,..}))&&settled_permits{RunClassification::deadline(TimeoutCause::Arming)}else{RunClassification::failure(format!("native arming refused: {error}"))}
}
// Called only with fault-free SettledRun facts; independent release faults are
// quarantined by the terminal controller and also refused defensively here.
fn classify_terminal(reasons:u32,release:Option<&Probe<Decision>>,exec:Option<terminal_controller::ExecState>,payload:Option<child_reaper::ExitOutcome>)->RunClassification {
    if reasons&1!=0{return RunClassification::failure("original invocation owner lost".into());}
    if reasons&512!=0{return RunClassification::failure("authenticated caller canceled the invocation".into());}
    if unknown_watchdog_stop(reasons){return RunClassification::failure("watchdog stopped without a primary verified owner/deadline/cancellation cause".into());}
    if let Some(terminal_controller::ExecState::ReportedFailure{errno})=exec{return RunClassification::failure(format!("payload setup/exec failed with errno {errno}"));}
    if let Some(Err(error))=release{return RunClassification::failure(format!("native release refused: {error}"));}
    if reasons&2!=0{return RunClassification::deadline(TimeoutCause::Terminal);}
    match release{Some(Err(error))=>return RunClassification::failure(format!("native release refused: {error}")),Some(Ok(Decision::Permit))=>{},_=>return RunClassification::failure("native release was not permitted".into())}
    if exec!=Some(terminal_controller::ExecState::NoExecErrorReport){return RunClassification::failure("payload exec-report state unverified".into());}
    match payload{
        Some(child_reaper::ExitOutcome::Exited{code})=>RunClassification::payload(i32::from(code)),
        Some(child_reaper::ExitOutcome::Signaled{signal,..})=>RunClassification::payload(128+signal),
        None=>RunClassification::failure("payload exit receipt missing".into()),
    }
}

pub(in super::super) fn before_admission(scope:Option<VerifiedServiceScope>,cancellation:Arc<super::super::super::entry::AuthenticatedCancellation>,admission:NativeReceipt,error:String)->RunOutcome {
    before_admission_cause(scope,cancellation,admission,error,PrelaunchCause::Failure)
}

pub(in super::super) fn before_admission_cause(scope:Option<VerifiedServiceScope>,cancellation:Arc<super::super::super::entry::AuthenticatedCancellation>,admission:NativeReceipt,error:String,cause:PrelaunchCause)->RunOutcome {
    RunOutcome{completion:Completion::RefusedBeforeActivation{error,cause,preparation:None},port:None,early_scope:scope,early_cancellation:Some(cancellation),admission}
}

#[cfg(test)]pub(super) fn gate_first_cause_regression(){
 let abi=sample_payload_abi();require_payload_abi(&abi).expect("actual compiled payload ABI27 must match production validator");
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
 println!("native_payload_abi schema={} spec={} words={} prlimit64={} core_guard_soft={} verified=true",abi[0],abi[1],abi.len(),abi[25],abi[26]);
 println!("native_gate_first_cause held_peer=preserved released_peer=preserved generic_first=not_relabelled gates=3 mappings_destroyed=3");
}

#[cfg(test)]fn gate_cancel_recording_regression(){
 for (sequence,expected) in [([0,1,2],1),([0,-1,2],-1),([2,9,0],9),([1,-1,2],1)]{
  let mut first=None;let mut invalid=None;
  for value in sequence{(first,invalid)=record_gate_cancel(first,invalid,value);}
  assert_eq!(first,Some(sequence[0]));assert_eq!(invalid,Some(expected));
 }
 let mut facts=(None,None);for value in [0,2,0,2]{facts=record_gate_cancel(facts.0,facts.1,value);}assert_eq!(facts,(Some(0),None));
 for value in [-1,1,3,9]{assert!(!arming_gate_cancel_valid(value));}for value in [0,2]{assert!(arming_gate_cancel_valid(value));}
 println!("arming_gate_cancel every_attempt_recorded=true first_invalid_sticky=true valid_invalid_valid_veto=true pure=true");
}
#[cfg(test)]pub(super) fn watchdog_mapping_regression(){timeout_cause_regression();gate_cancel_recording_regression();for other in [0,4,32]{assert!(unknown_watchdog_stop(8|other));}for primary in [1,2,512]{assert!(!unknown_watchdog_stop(8|primary));}assert!(!unknown_watchdog_stop(4));}

#[cfg(test)]fn timeout_cause_regression(){
 let permit=Ok(Decision::Permit);let before_release=Ok(Decision::Deadline);let fault=Err("independent release fault".into());
 let exec=Some(terminal_controller::ExecState::NoExecErrorReport);
 for code in [0,124,125]{let value=classify_terminal(0,Some(&permit),exec,Some(child_reaper::ExitOutcome::Exited{code}));assert_eq!(value.code,i32::from(code));assert_eq!(value.timeout,None);assert!(value.error.is_none());}
 for release in [Some(&permit),Some(&before_release)]{let value=classify_terminal(2,release,exec,None);assert_eq!(value.code,124);assert_eq!(value.timeout,Some(TimeoutCause::Terminal));assert!(value.error.is_none());}
 for reasons in [1|2,512|2,8]{let value=classify_terminal(reasons,Some(&permit),exec,None);assert_eq!(value.code,125);assert_eq!(value.timeout,None);}
 for (release,state) in [(Some(&fault),exec),(Some(&permit),Some(terminal_controller::ExecState::ReportedFailure{errno:2}))]{let value=classify_terminal(2,release,state,None);assert_eq!(value.code,125);assert_eq!(value.timeout,None);assert!(value.error.is_some());}
 let deadline=Ok(DecisionReceipt{decision:Decision::Deadline,owner_events:0,timer_events:POLLIN,sampled_ns:1});
 let permit_sample=Ok(DecisionReceipt{decision:Decision::Permit,owner_events:0,timer_events:0,sampled_ns:1});let bad:Probe<DecisionReceipt>=Err("sample fault".into());
 let good=classify_arming(Some(Decision::Deadline),&deadline,true,"");assert_eq!(good.timeout,Some(TimeoutCause::Arming));assert_eq!(good.code,124);
 for (primary,sample,settled) in [(None,&deadline,true),(Some(Decision::Permit),&deadline,true),(Some(Decision::Deadline),&permit_sample,true),(Some(Decision::Deadline),&bad,true),(Some(Decision::Deadline),&deadline,false)]{let value=classify_arming(primary,sample,settled,"");assert_eq!(value.code,125);assert_eq!(value.timeout,None);}
 for error in [super::super::super::invocation::InvocationError::OwnerLost,super::super::super::invocation::InvocationError::Kernel("124".into())]{let value=classify_prelaunch(PrelaunchCause::invocation(&error),"124");assert_eq!(value.code,125);assert_eq!(value.timeout,None);}
 let value=classify_prelaunch(PrelaunchCause::invocation(&super::super::super::invocation::InvocationError::Expired),"");assert_eq!(value.timeout,Some(TimeoutCause::PreLaunch));assert_eq!(value.code,124);
 eprintln!("typed_original_timeout_classification_payload124_125_vetoes_verified");
}
