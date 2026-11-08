// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Original public invocation: captured before helpers, with no numeric-PID reopen
//! after transfer. Raw wire metadata is not privileged timestamp attestation.
use super::*;
use std::time::Duration;

pub(super) const MAX_WATCHERS:usize=249;
pub(super) const MAX_ANCESTORS:usize=MAX_WATCHERS-1;
#[repr(C)]
#[derive(Clone,Copy)]
pub(super) struct OwnerWatch {pub(super) fds:*const c_int,pub(super) count:u32,pub(super) reserved:u32}
const _:()={assert!(size_of::<OwnerWatch>()==16);assert!(std::mem::offset_of!(OwnerWatch,count)==8);};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) struct ProcessIdentity {pub(super) pid:u32,pub(super) ppid:u32,pub(super) start_ticks:u64}
#[derive(Clone,Debug,PartialEq)]
pub(super) struct Metadata {
    pub(super) mode:OwnerMode,pub(super) umask:u16,pub(super) seconds:f64,pub(super) captured_ns:u64,pub(super) deadline_ns:u64,
    pub(super) identities:Vec<ProcessIdentity>,
}
#[derive(Debug,PartialEq,Eq)]
pub(crate) enum InvocationError {
    InvalidDuration,InvalidTime,InvalidUmask,Expired,OwnerLost,TooDeep,InvalidChain,IdentityChanged,Namespace,Kernel(String),
}
impl InvocationError {
    pub(crate) fn exit_code(&self)->i32 {if *self==Self::Expired {124}else{125}}
}
type CaptureResult<T>=std::result::Result<T,InvocationError>;
fn kernel<T>(value:super::Result<T>)->CaptureResult<T> {value.map_err(|e|InvocationError::Kernel(format!("{e:?}")))}
fn io<T>(value:std::io::Result<T>)->CaptureResult<T> {value.map_err(|e|InvocationError::Kernel(e.to_string()))}
fn clock()->CaptureResult<u64> {supervisor::boottime_ns().map_err(InvocationError::Kernel)}
pub(super) fn requested_ns(seconds:f64)->CaptureResult<u64> {
    if crate::deadline::invalid_seconds(seconds){return Err(InvocationError::InvalidDuration);}
    u64::try_from(Duration::try_from_secs_f64(seconds).map_err(|_|InvocationError::InvalidDuration)?
        .max(Duration::from_nanos(1)).as_nanos()).map_err(|_|InvocationError::InvalidDuration)
}
fn read_bounded(path:&str)->CaptureResult<Vec<u8>> {
    use std::io::Read;
    let file=io(fs::File::open(path))?;let mut bytes=Vec::new();
    io(file.take(16385).read_to_end(&mut bytes))?;
    if bytes.len()>16384 {return Err(InvocationError::InvalidChain);}
    Ok(bytes)
}
fn decimal<T:std::str::FromStr>(bytes:&[u8])->CaptureResult<T> {
    std::str::from_utf8(bytes).map_err(|_|InvocationError::InvalidChain)?.parse().map_err(|_|InvocationError::InvalidChain)
}
fn process_identity(fd:&OwnedFd,pid:u32)->CaptureResult<ProcessIdentity> {
    kernel(validate_pidfd(fd,pid as c_int,&mut NativeReceipt::default()))?;
    let bytes=read_bounded(&format!("/proc/{pid}/stat"))?;
    let open=bytes.iter().position(|b|*b==b'(').ok_or(InvocationError::InvalidChain)?;
    let close=bytes.iter().rposition(|b|*b==b')').ok_or(InvocationError::InvalidChain)?;
    if close<=open {return Err(InvocationError::InvalidChain);}
    let actual:u32=decimal(bytes[..open].trim_ascii())?;
    let fields:Vec<_>=bytes[close+1..].split(|b|b.is_ascii_whitespace()).filter(|v|!v.is_empty()).collect();
    if fields.len()<20 || actual!=pid || pid==0 {return Err(InvocationError::IdentityChanged);}
    if fields[0]==b"Z" || fields[0]==b"X" {return Err(InvocationError::OwnerLost);}
    let value=ProcessIdentity{pid,ppid:decimal(fields[1])?,start_ticks:decimal(fields[19])?};
    kernel(validate_pidfd(fd,pid as c_int,&mut NativeReceipt::default()))?;
    Ok(value)
}
fn namespaces(pid:u32)->CaptureResult<(std::path::PathBuf,std::path::PathBuf)> {
    Ok((io(fs::read_link(format!("/proc/{pid}/ns/pid")))?,io(fs::read_link(format!("/proc/{pid}/ns/time")))?))
}
fn live_set(fds:&[OwnedFd])->CaptureResult<()> {
    if !(2..=MAX_WATCHERS).contains(&fds.len()){return Err(InvocationError::InvalidChain);}
    let mut watched=[PollFd{fd:-1,events:POLLIN,revents:0};MAX_WATCHERS];
    for (slot,fd) in watched.iter_mut().zip(fds){slot.fd=fd.as_raw_fd();}
    let result=unsafe{poll(watched.as_mut_ptr(),fds.len(),0)};
    if result<0{return Err(InvocationError::Kernel(std::io::Error::last_os_error().to_string()));}
    if result!=0 || watched[..fds.len()].iter().any(|v|v.revents!=0){return Err(InvocationError::OwnerLost);}
    Ok(())
}
impl Metadata {
    pub(super) fn validate(&self)->CaptureResult<()> {
        if self.umask>0o777{return Err(InvocationError::InvalidUmask);}
        let nanos=requested_ns(self.seconds)?;
        if self.captured_ns==0 || self.captured_ns.checked_add(nanos)!=Some(self.deadline_ns){return Err(InvocationError::InvalidTime);}
        if !(2..=MAX_WATCHERS).contains(&self.identities.len()) {return Err(InvocationError::TooDeep);}
        if self.mode==OwnerMode::Manual && self.identities.len()!=2{return Err(InvocationError::InvalidChain);}
        for (index,item) in self.identities.iter().enumerate(){
            if item.pid==0 || item.pid>c_int::MAX as u32 || self.identities[..index].iter().any(|p|p.pid==item.pid){return Err(InvocationError::InvalidChain);}
            if let Some(parent)=self.identities.get(index+1){
                if item.ppid!=parent.pid || parent.start_ticks>item.start_ticks{return Err(InvocationError::InvalidChain);}
            }
        }
        if self.mode==OwnerMode::Auto {
            let end=self.identities.last().ok_or(InvocationError::InvalidChain)?;
            if end.pid!=1 || end.ppid!=0 {return Err(InvocationError::InvalidChain);}
        }
        Ok(())
    }
}

// Fields are private; no raw metadata/FD constructor and no timeout setter.
// Capture is the actual trusted library/client API boundary, not a privileged
// attestation of arbitrary replacement wire callers or general user memory.
#[must_use="original invocation capabilities must cross bootstrap unchanged"]
#[derive(Debug)]
pub(crate) struct OriginalInvocation {
    metadata:Metadata,pidfds:Vec<OwnedFd>,namespaces:(std::path::PathBuf,std::path::PathBuf),
}
impl OriginalInvocation {
    pub(crate) fn capture(mode:OwnerMode,seconds:f64)->CaptureResult<Self> {
        let duration=requested_ns(seconds)?;
        let captured_ns=clock()?;
        let deadline_ns=captured_ns.checked_add(duration).ok_or(InvocationError::InvalidTime)?;
        let umask=supervisor::native_activation::capture_calling_umask().map_err(InvocationError::Kernel)?;
        let requester=std::process::id();
        let captured_namespaces=namespaces(requester)?;
        let first=kernel(pidfd_for(requester as c_int))?;
        let first_identity=process_identity(&first,requester)?;
        let mut pidfds=vec![first];let mut identities=vec![first_identity];
        let mut next=first_identity.ppid;
        loop {
            if next==0 {return Err(InvocationError::OwnerLost);}
            if pidfds.len()-1==MAX_ANCESTORS{return Err(InvocationError::TooDeep);}
            if identities.iter().any(|v|v.pid==next){return Err(InvocationError::InvalidChain);}
            let owner=kernel(pidfd_for(next as c_int))?;
            let identity=process_identity(&owner,next)?;
            pidfds.push(owner);identities.push(identity);
            if mode==OwnerMode::Manual || next==1 {break;}
            next=identity.ppid;
        }
        let metadata=Metadata{mode,umask,seconds,captured_ns,deadline_ns,identities};
        metadata.validate()?;
        let result=Self{metadata,pidfds,namespaces:captured_namespaces};
        result.revalidate()?;
        Ok(result)
    }
    pub(super) fn revalidate(&self)->CaptureResult<()> {
        if self.metadata.identities[0].pid!=std::process::id() || namespaces(std::process::id())?!=self.namespaces {
            return Err(InvocationError::Namespace);
        }
        verify_chain(&self.metadata,&self.pidfds)?;
        validate_time(&self.metadata,clock()?)
    }
    pub(super) fn metadata(&self)->&Metadata {&self.metadata}
    pub(super) fn pidfds(&self)->&[OwnedFd] {&self.pidfds}
}
fn verify_chain(metadata:&Metadata,fds:&[OwnedFd])->CaptureResult<()> {
    metadata.validate()?;
    if fds.len()!=metadata.identities.len(){return Err(InvocationError::InvalidChain);}
    live_set(fds)?;
    // Two complete, independent native passes while all exact objects remain
    // held. Parent exit/reparent/reuse, missing identity and permission errors
    // refuse the whole request; no ancestor is silently skipped.
    for _ in 0..2 {
        for (expected,fd) in metadata.identities.iter().zip(fds){
            if process_identity(fd,expected.pid)?!=*expected{return Err(InvocationError::IdentityChanged);}
        }
        live_set(fds)?;
    }
    Ok(())
}
fn validate_time(metadata:&Metadata,now:u64)->CaptureResult<()> {
    metadata.validate()?;
    if metadata.captured_ns>now{return Err(InvocationError::InvalidTime);}
    if now>=metadata.deadline_ns{return Err(InvocationError::Expired);}
    Ok(())
}
#[must_use="verified original owner set must remain held through terminal/quarantine"]
#[derive(Debug)]
pub(super) struct VerifiedInvocation {metadata:Metadata,pidfds:Vec<OwnedFd>,numbers:Vec<c_int>}
impl VerifiedInvocation {
    pub(super) fn receive(metadata:Metadata,pidfds:Vec<OwnedFd>,peer:Credentials)->CaptureResult<Self> {
        metadata.validate()?;
        if metadata.identities[0].pid!=peer.pid as u32{return Err(InvocationError::IdentityChanged);}
        if namespaces(peer.pid as u32)?!=namespaces(std::process::id())?{return Err(InvocationError::Namespace);}
        verify_chain(&metadata,&pidfds)?;
        validate_time(&metadata,clock()?)?;
        let numbers=pidfds.iter().map(AsRawFd::as_raw_fd).collect();
        Ok(Self{metadata,pidfds,numbers})
    }
    pub(super) fn requester(&self)->&OwnedFd {&self.pidfds[0]}
    pub(super) fn metadata(&self)->&Metadata {&self.metadata}
    pub(super) fn owners(&self)->&[OwnedFd] {&self.pidfds}
    pub(super) fn numbers(&self)->&[c_int] {&self.numbers}
    pub(super) fn watch(&self)->OwnerWatch {OwnerWatch{fds:self.numbers.as_ptr(),count:self.numbers.len() as u32,reserved:0}}
    pub(super) fn check_live(&self)->CaptureResult<()> {live_set(&self.pidfds)}
    pub(super) fn check_time(&self)->CaptureResult<()> {validate_time(&self.metadata,clock()?)}
}
