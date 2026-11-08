// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Linux one-shot admission. Backend selection remains disabled pending qualification.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

#[path = "request.rs"]
mod request;
#[path="delegation.rs"]
mod delegation;
pub(crate) type SeparateService=delegation::SeparateService;
#[path="invocation.rs"]
mod invocation;
pub(crate) use invocation::OriginalInvocation;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum OwnerMode {Manual,Auto}

use std::ffi::{c_int, c_long, c_short, c_void};
use std::fs;
use std::io;
use std::mem::{size_of, size_of_val};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;

const AF_UNIX: c_int = 1;
const SOCK_SEQPACKET: c_int = 5;
const SOCK_NONBLOCK: c_int = 0o4000;
const SOCK_CLOEXEC: c_int = 0o2000000;
const SOL_SOCKET: c_int = 1;
#[cfg(test)]
const SO_SNDBUF: c_int = 7;
const SO_TYPE: c_int = 3;
const SO_PASSCRED: c_int = 16;
const SO_PROTOCOL: c_int = 38;
const SO_DOMAIN: c_int = 39;
const SO_PEERCRED: c_int = 17;
const SCM_RIGHTS: c_int = 1;
const SCM_CREDENTIALS: c_int = 2;
// Linux6.5+ may attach a separate newly installed pidfd. Reject, but own/close it.
const SCM_PIDFD: c_int = 4;
const MSG_CTRUNC: c_int = 0x08;
const MSG_TRUNC: c_int = 0x20;
const MSG_CMSG_CLOEXEC: c_int = 0x40000000;
const MSG_DONTWAIT: c_int = 0x40;
const MSG_NOSIGNAL: c_int = 0x4000;
const F_GETFD: c_int = 1;
const F_GETFL: c_int = 3;
const O_ACCMODE: c_int = 3;
const O_PATH: c_int = 0o10_000_000;
#[cfg(target_arch="aarch64")]
const O_DIRECTORY: c_int = 0o40_000;
#[cfg(target_arch="x86_64")]
const O_DIRECTORY: c_int = 0o200_000;
const FD_CLOEXEC: c_int = 1;
const POLLIN: c_short = 1;
const SYS_PIDFD_OPEN: c_long = 434;
// SCM_MAX_FD253: CMSG_SPACE(253*4)=1032, credentials32, optionalPIDFD24=1088.
const CONTROL_WORDS: usize = 136;
const CONTROL_BYTES: usize = CONTROL_WORDS * size_of::<u64>();
const HEADER_BYTES: usize = 16;

#[repr(C)]
struct IoVec {
    base: *mut u8,
    len: usize,
}

#[repr(C)]
struct Message {
    name: *mut c_void,
    name_len: u32,
    vec: *mut IoVec,
    vec_len: usize,
    control: *mut u8,
    control_len: usize,
    flags: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Credentials {
    pid: c_int,
    uid: u32,
    gid: u32,
}

#[repr(C)]
#[derive(Clone,Copy)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

const _: () = {
    assert!(size_of::<IoVec>() == 16);
    assert!(size_of::<Message>() == 56);
    assert!(size_of::<Credentials>() == 12);
    assert!(size_of::<PollFd>() == 8);
};

unsafe extern "C" {
    #[cfg(test)]
    fn socketpair(domain: c_int, kind: c_int, protocol: c_int, fds: *mut c_int) -> c_int;
    fn getsockopt(fd: c_int, level: c_int, opt: c_int, value: *mut c_void, len: *mut u32) -> c_int;
    fn setsockopt(fd: c_int, level: c_int, opt: c_int, value: *const c_int, len: u32) -> c_int;
    fn recvmsg(fd: c_int, message: *mut Message, flags: c_int) -> isize;
    fn sendmsg(fd: c_int, message: *const Message, flags: c_int) -> isize;
    fn syscall(number: c_long, ...) -> c_long;
    fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    fn poll(fds: *mut PollFd, count: usize, timeout: c_int) -> c_int;
    fn getuid() -> u32;
    fn geteuid() -> u32;
    fn getgid() -> u32;
    fn getegid() -> u32;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Os(Option<c_int>),
    EmptyPacket,
    PayloadTruncated,
    ControlTruncated,
    UnexpectedFlags,
    Ancillary,
    PeerCredentials,
    SocketKind,
    SocketFlags,
    IoAccess,
    WorkingDirectory,
    RightsCount(usize),
    NotCloseOnExec,
    NotPidfd,
    WrongPidfd,
    DeadPidfd,
    InvalidRequest(request::Refusal),
    Invocation(invocation::InvocationError),
    Entry(String),
}

type Result<T> = std::result::Result<T, Refusal>;

impl From<io::Error> for Refusal {
    fn from(error: io::Error) -> Self {
        Self::Os(error.raw_os_error())
    }
}

fn os_result(value: isize) -> Result<usize> {
    if value < 0 {
        Err(io::Error::last_os_error().into())
    } else {
        Ok(value as usize)
    }
}

fn message(vec: &mut IoVec, control: *mut u8, control_len: usize) -> Message {
    Message {
        name: ptr::null_mut(),
        name_len: 0,
        vec,
        vec_len: 1,
        control,
        control_len,
        flags: 0,
    }
}

#[cfg(test)]
fn socket_pair() -> Result<(OwnedFd, OwnedFd)> {
    let mut descriptors = [-1; 2];
    os_result(unsafe {
        socketpair(
            AF_UNIX,
            SOCK_SEQPACKET | SOCK_NONBLOCK | SOCK_CLOEXEC,
            0,
            descriptors.as_mut_ptr(),
        )
    } as isize)?;
    // A successful socketpair installs two new, distinct file descriptors.
    let [tx, rx] = descriptors.map(|fd| unsafe { OwnedFd::from_raw_fd(fd) });
    let size = size_of::<c_int>() as u32;
    let send_buffer = request::MAX_PACKET_BYTES as c_int + 8192;
    os_result(
        unsafe { setsockopt(tx.as_raw_fd(), SOL_SOCKET, SO_SNDBUF, &send_buffer, size) } as isize,
    )?;
    os_result(unsafe { setsockopt(rx.as_raw_fd(), SOL_SOCKET, SO_PASSCRED, &1, size) } as isize)?;
    Ok((tx, rx))
}

fn kernel_peer_credentials(socket: RawFd) -> Result<Credentials> {
    let mut peer = Credentials::default();
    let expected = size_of::<Credentials>() as u32;
    let mut length = expected;
    os_result(unsafe {
        getsockopt(
            socket,
            SOL_SOCKET,
            SO_PEERCRED,
            (&mut peer as *mut Credentials).cast(),
            &mut length,
        )
    } as isize)?;
    if length != expected || peer.pid <= 0 {
        return Err(Refusal::PeerCredentials);
    }
    Ok(peer)
}

fn peer_credentials(socket:RawFd)->Result<Credentials> {
    let peer=kernel_peer_credentials(socket)?;
    if peer.uid==0 || peer.gid==0 {return Err(Refusal::PeerCredentials);}
    Ok(peer)
}
fn socket_option(socket:RawFd,option:c_int)->Result<c_int> {
    let mut value=0;let mut length=size_of::<c_int>() as u32;
    os_result(unsafe{getsockopt(socket,SOL_SOCKET,option,(&mut value as *mut c_int).cast(),&mut length)} as isize)?;
    if length!=size_of::<c_int>() as u32 {return Err(Refusal::SocketKind);}
    Ok(value)
}
fn validate_connection(socket:RawFd,receiving:bool)->Result<()> {
    close_on_exec(socket)?;
    if socket_option(socket,SO_DOMAIN)?!=AF_UNIX || socket_option(socket,SO_TYPE)?!=SOCK_SEQPACKET
        || socket_option(socket,SO_PROTOCOL)?!=0 {return Err(Refusal::SocketKind);}
    let flags=os_result(unsafe{fcntl(socket,F_GETFL)} as isize)? as c_int;
    if flags&SOCK_NONBLOCK==0 {return Err(Refusal::SocketFlags);}
    if receiving && socket_option(socket,SO_PASSCRED)?!=1 {return Err(Refusal::SocketFlags);}
    Ok(())
}
fn io_flags(fd:RawFd,slot:usize)->Result<c_int> {
    let flags=os_result(unsafe{fcntl(fd,F_GETFL)} as isize)? as c_int;
    if slot==3 {
        // O_PATH|O_DIRECTORY is checked by the kernel at the original open.
        // F_SETFL cannot manufacture these bits. Never reopen as root.
        if flags&(O_PATH|O_DIRECTORY)!=(O_PATH|O_DIRECTORY) {return Err(Refusal::WorkingDirectory);}
    } else {
        let access=flags&O_ACCMODE;
        let valid=if slot==0 {access==0 || access==2} else {access==1 || access==2};
        if flags&O_PATH!=0 || !valid {return Err(Refusal::IoAccess);}
    }
    Ok(flags)
}

fn close_on_exec(fd: RawFd) -> Result<()> {
    let flags = os_result(unsafe { fcntl(fd, F_GETFD) } as isize)?;
    if flags & FD_CLOEXEC as usize == 0 {
        return Err(Refusal::NotCloseOnExec);
    }
    Ok(())
}

fn send_packet(socket: RawFd, payload: &[u8], descriptors: &[RawFd]) -> Result<()> {
    let mut vec = IoVec {
        base: payload.as_ptr().cast_mut(),
        len: payload.len(),
    };
    let length = HEADER_BYTES + size_of_val(descriptors);
    let mut control = vec![0u64; length.div_ceil(size_of::<u64>())];
    let header = control.as_mut_ptr().cast::<u8>();
    let message = if descriptors.is_empty() {
        message(&mut vec, ptr::null_mut(), 0)
    } else {
        // sendmsg only reads this initialized, natively aligned buffer.
        let bytes = unsafe { std::slice::from_raw_parts_mut(header, length) };
        bytes[..8].copy_from_slice(&length.to_ne_bytes());
        bytes[8..12].copy_from_slice(&SOL_SOCKET.to_ne_bytes());
        bytes[12..16].copy_from_slice(&SCM_RIGHTS.to_ne_bytes());
        for (destination, fd) in bytes[HEADER_BYTES..].chunks_exact_mut(4).zip(descriptors) {
            destination.copy_from_slice(&fd.to_ne_bytes());
        }
        message(&mut vec, header, size_of_val(control.as_slice()))
    };
    let sent = os_result(unsafe { sendmsg(socket, &message, MSG_DONTWAIT | MSG_NOSIGNAL) })?;
    if sent != payload.len() {
        return Err(Refusal::PayloadTruncated);
    }
    Ok(())
}

#[derive(Default)]
struct Ancillary {
    credential: Option<Credentials>,
    rights: Vec<OwnedFd>,
    unexpected: bool,
}

fn ancillary(mut bytes: &[u8]) -> Result<Ancillary> {
    let mut packet = Ancillary::default();
    let integer = |data: &[u8]| c_int::from_ne_bytes(data.try_into().expect("ancillary integer"));
    while !bytes.is_empty() {
        if bytes.len() < HEADER_BYTES {
            return Err(Refusal::Ancillary);
        }
        let length = usize::from_ne_bytes(bytes[..8].try_into().expect("ancillary length"));
        if length < HEADER_BYTES || length > bytes.len() {
            return Err(Refusal::Ancillary);
        }
        let kind = (integer(&bytes[8..12]), integer(&bytes[12..16]));
        let data = &bytes[HEADER_BYTES..length];
        match kind {
            (SOL_SOCKET, SCM_RIGHTS) => {
                for word in data.chunks_exact(size_of::<c_int>()) {
                    let fd = integer(word);
                    if fd < 0 {
                        packet.unexpected = true;
                    } else {
                        // Each fd in recvmsg's native SCM_RIGHTS record is newly installed
                        // in this process. Adopt it even if a later check refuses the packet.
                        packet.rights.push(unsafe { OwnedFd::from_raw_fd(fd) });
                    }
                }
                packet.unexpected |= !data.len().is_multiple_of(size_of::<c_int>());
            }
            (SOL_SOCKET, SCM_PIDFD) if data.len() == size_of::<c_int>() => {
                let fd=integer(data);
                if fd>=0 {packet.rights.push(unsafe{OwnedFd::from_raw_fd(fd)});}
                packet.unexpected=true;
            }
            (SOL_SOCKET, SCM_CREDENTIALS) if data.len() == size_of::<Credentials>() => {
                let credential = Credentials {
                    pid: integer(&data[..4]),
                    uid: u32::from_ne_bytes(data[4..8].try_into().expect("ancillary uid")),
                    gid: u32::from_ne_bytes(data[8..12].try_into().expect("ancillary gid")),
                };
                packet.unexpected |= packet.credential.replace(credential).is_some();
            }
            _ => packet.unexpected = true,
        }
        // A last kernel record need not have its full trailing alignment padding.
        bytes = &bytes[length.next_multiple_of(size_of::<u64>()).min(bytes.len())..];
    }
    Ok(packet)
}

fn pidfd_for(pid: c_int) -> Result<OwnedFd> {
    let fd = os_result(unsafe { syscall(SYS_PIDFD_OPEN, pid, 0 as c_int) } as isize)? as RawFd;
    // pidfd_open returns a new owned descriptor on success.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

#[derive(Debug, Default)]
struct NativeReceipt {
    connection_verified: bool,
    io_flags: Option<[c_int;4]>,
    original_watchers: usize,
    claimed_invocation: Option<(u64,u64)>,
    owner_mode: Option<OwnerMode>,
    peer: Option<Credentials>,
    sender: Option<Credentials>,
    rights: usize,
    pidfd_cloexec: bool,
    pidfd_pid: Option<c_int>,
    pidfd_revents: Option<c_short>,
}

fn validate_pidfd(
    descriptor: &OwnedFd,
    pid: c_int,
    receipt: &mut NativeReceipt,
) -> Result<()> {
    let fd = descriptor.as_raw_fd();
    close_on_exec(fd)?;
    receipt.pidfd_cloexec = true;
    if fs::read_link(format!("/proc/self/fd/{fd}"))?.as_os_str() != "anon_inode:[pidfd]" {
        return Err(Refusal::NotPidfd);
    }
    let information = fs::read_to_string(format!("/proc/self/fdinfo/{fd}"))?;
    let observed_pid = information
        .lines()
        .find_map(|line| line.strip_prefix("Pid:"))
        .and_then(|field| field.trim().parse().ok());
    receipt.pidfd_pid = observed_pid;
    if observed_pid != Some(pid) {
        return Err(Refusal::WrongPidfd);
    }
    let mut watched = PollFd {
        fd,
        events: POLLIN,
        revents: 0,
    };
    let ready = os_result(unsafe { poll(&mut watched, 1, 0) } as isize)?;
    receipt.pidfd_revents = Some(watched.revents);
    if ready != 0 {
        return Err(Refusal::DeadPidfd);
    }
    Ok(())
}

#[derive(Debug)]
struct CallerIo {stdio:[OwnedFd;3],cwd:OwnedFd}

#[derive(Debug)]
struct Admitted {
    peer: Credentials,
    invocation: invocation::VerifiedInvocation,
    plan: request::ExecPlan,
    io: CallerIo,
}

#[cfg(test)]
fn receive(socket: RawFd) -> Result<Admitted> {
    receive_observed(socket, &mut NativeReceipt::default())
}

fn receive_observed(socket:RawFd,receipt:&mut NativeReceipt)->Result<Admitted> {
    let mut control=[0u64;CONTROL_WORDS];
    receive_with_control(socket,receipt,&mut control)
}
// One native receive/adopt/refuse path. Production always passes the full1088B;
// the existing native regression supplies a smaller aligned prefix for CTRUNC.
fn receive_with_control(socket:RawFd,receipt:&mut NativeReceipt,control:&mut [u64])->Result<Admitted> {
    receive_with_control_hook(socket,receipt,control,None)
}
fn receive_entry(socket:RawFd,receipt:&mut NativeReceipt,hook:&mut dyn FnMut(u64)->Result<()>)->Result<Admitted>{
    let mut control=[0u64;CONTROL_WORDS];receive_with_control_hook(socket,receipt,&mut control,Some(hook))
}
fn receive_with_control_hook(socket:RawFd,receipt:&mut NativeReceipt,control:&mut [u64],hook:Option<&mut dyn FnMut(u64)->Result<()>>)->Result<Admitted> {
    validate_connection(socket,true)?;
    receipt.connection_verified=true;
    let peer = peer_credentials(socket)?;
    receipt.peer = Some(peer);
    let mut payload = vec![0u8; request::MAX_PACKET_BYTES];
    let mut vec = IoVec {
        base: payload.as_mut_ptr(),
        len: payload.len(),
    };
    let capacity=size_of_val(control);
    if capacity>CONTROL_BYTES {return Err(Refusal::Ancillary);}
    let mut message = message(&mut vec, control.as_mut_ptr().cast(), capacity);
    let count = os_result(unsafe { recvmsg(socket, &mut message, MSG_CMSG_CLOEXEC | MSG_DONTWAIT) })?;
    // Only native recvmsg writes these bytes. Own any delivered fds before
    // considering truncation flags, identity, descriptor count or request bytes.
    let raw =
        unsafe { std::slice::from_raw_parts(control.as_ptr().cast::<u8>(), message.control_len.min(capacity)) };
    let packet = ancillary(raw)?;
    if message.control_len>capacity {return Err(Refusal::Ancillary);}
    receipt.sender = packet.credential;
    receipt.rights = packet.rights.len();
    if message.flags & MSG_CTRUNC != 0 {
        return Err(Refusal::ControlTruncated);
    }
    if message.flags & MSG_TRUNC != 0 {
        return Err(Refusal::PayloadTruncated);
    }
    if message.flags & !MSG_CMSG_CLOEXEC != 0 {
        return Err(Refusal::UnexpectedFlags);
    }
    if count == 0 {
        return Err(Refusal::EmptyPacket);
    }
    if packet.unexpected {
        return Err(Refusal::Ancillary);
    }
    if packet.credential != Some(peer) {
        return Err(Refusal::PeerCredentials);
    }
    let frame=payload.get(..count).ok_or(Refusal::PayloadTruncated)?;
    let plan=request::decode(frame).map_err(Refusal::InvalidRequest)?;
    let watchers=plan.invocation().identities.len();
    receipt.original_watchers=watchers;
    receipt.owner_mode=Some(plan.invocation().mode);
    receipt.claimed_invocation=Some((plan.invocation().captured_ns,plan.invocation().deadline_ns));
    if packet.rights.len()!=watchers+4 || packet.rights.len()>253 {return Err(Refusal::RightsCount(packet.rights.len()));}
    // A root service may only NARROW its own bootstrap lifetime here, before
    // potentially slower proc/ancestry checks. This grants no admission authority.
    if let Some(hook)=hook{hook(plan.invocation().deadline_ns)?;}
    let mut owners=packet.rights;
    let [stdin,stdout,stderr,cwd]:[OwnedFd;4]=owners.split_off(watchers).try_into()
        .map_err(|rights:Vec<OwnedFd>|Refusal::RightsCount(rights.len()))?;
    for fd in owners.iter().chain([&stdin,&stdout,&stderr,&cwd]){close_on_exec(fd.as_raw_fd())?;}
    let mut flags=[0;4];
    for (index,fd) in [&stdin,&stdout,&stderr,&cwd].into_iter().enumerate(){flags[index]=io_flags(fd.as_raw_fd(),index)?;}
    receipt.io_flags=Some(flags);
    validate_pidfd(&owners[0],peer.pid,receipt)?;
    let invocation=invocation::VerifiedInvocation::receive(plan.invocation().clone(),owners,peer).map_err(Refusal::Invocation)?;
    Ok(Admitted{peer,invocation,plan,io:CallerIo{stdio:[stdin,stdout,stderr],cwd}})

}

mod manager;
mod entry;
mod client;
mod supervisor;
pub(crate) type ClientOutcome=client::ClientOutcome;
pub(crate) fn root_service_main()->!{entry::root_service_main()}
pub(crate) fn run_library(command:&[std::ffi::OsString],seconds:f64,mode:OwnerMode)->ClientOutcome{client::run_library(command,seconds,mode)}
/// Explicit administrative capability; does not promise caller sandbox inheritance.
pub(crate) fn run_library_delegated(command:&[std::ffi::OsString],seconds:f64,mode:OwnerMode,acknowledgment:SeparateService)->ClientOutcome{client::run_library_delegated(command,seconds,mode,acknowledgment)}
pub(crate) fn run_cli(arguments:Vec<std::ffi::OsString>)->ClientOutcome{client::run_cli(arguments)}
// Concrete aliases/wrapper preserve the internal API in both actual binaries;
// a type alias grants no construction authority and avoids unused reexports.
pub(crate) type AuthenticatedCancellation=entry::AuthenticatedCancellation;
pub(crate) type VerifiedServiceScope=supervisor::VerifiedServiceScope;
pub(crate) type RunOutcome=supervisor::RunOutcome;
/// # Safety
/// Preserve the checked scope and dedicated-supervisor preconditions of the
/// consuming supervisor entry. No raw/unverified scope can enter this wrapper.
pub(crate) unsafe fn run_in_service(scope:VerifiedServiceScope,connection:OwnedFd,cancellation:AuthenticatedCancellation,
 authorizes:impl FnOnce(u32,u32,&[u32],&std::ffi::CStr)->bool)->RunOutcome{
 unsafe{supervisor::run_in_service(scope,connection,cancellation,authorizes)}
}

/// Unqualified privileged startup fails closed; there is no global cgroup
/// fallback or caller-constructed service authority.
/// # Safety
/// Complete public caller-parent/ancestor and original-time entry invariants
/// remain required before a Linux backend can be enabled.
pub(crate) unsafe fn run_received(
    connection:OwnedFd,cancellation:AuthenticatedCancellation,
    policy:impl FnOnce(u32,u32,&[u32],&std::ffi::CStr)->bool,
)->RunOutcome {unsafe{supervisor::run_received(connection,cancellation,policy)}}

/// Send the calling ordinary process's request to its connected privileged
/// one-shot peer. Borrowed IO/CWD are passed as actual kernel references; no
/// descriptor is reopened and neither peer's environment is read implicitly.
/// The client must already have NNP set (receiver verifies it); this routine
/// does not mutate that process-wide policy, launch a helper or wait forever.
pub(crate) fn send_request(
    connection:&OwnedFd,original:&OriginalInvocation,arguments:&[&[u8]],environment:&[&[u8]],
    stdio:[BorrowedFd<'_>;3],cwd:BorrowedFd<'_>,
)->Result<()> {
    validate_connection(connection.as_raw_fd(),false)?;
    let peer=kernel_peer_credentials(connection.as_raw_fd())?;
    if peer.uid!=0||peer.gid!=0{return Err(Refusal::PeerCredentials);}
    let uid=unsafe{getuid()};let gid=unsafe{getgid()};
    if uid==0||gid==0||unsafe{geteuid()}!=uid||unsafe{getegid()}!=gid{return Err(Refusal::PeerCredentials);}
    original.revalidate().map_err(Refusal::Invocation)?;
    for (index,fd) in stdio.iter().enumerate(){io_flags(fd.as_raw_fd(),index)?;}io_flags(cwd.as_raw_fd(),3)?;
    let packet=request::encode(original.metadata(),arguments,environment).map_err(Refusal::InvalidRequest)?;
    let mut rights:Vec<RawFd>=original.pidfds().iter().map(AsRawFd::as_raw_fd).collect();
    rights.extend([stdio[0].as_raw_fd(),stdio[1].as_raw_fd(),stdio[2].as_raw_fd(),cwd.as_raw_fd()]);
    if rights.len()>253{return Err(Refusal::RightsCount(rights.len()));}
    send_packet(connection.as_raw_fd(),&packet,&rights)
}

#[cfg(test)]
mod tests {
    #[test]
    fn watchdog_fault125_is_not_hidden_by_later_cancellation(){super::supervisor::native_activation::watchdog_fault_regression();}

    #[test]
    fn native_gate_first_cancellation_cause_is_irreversible(){super::supervisor::native_activation::gate_first_cause_regression();}

    use super::*;
    use std::io::{Read,Seek,SeekFrom,Write};
    use std::os::unix::fs::{DirBuilderExt,MetadataExt,OpenOptionsExt};
    use std::time::{SystemTime,UNIX_EPOCH};

    fn count_fds()->usize {
        fs::read_dir("/proc/self/fd").unwrap().collect::<io::Result<Vec<_>>>().unwrap().len()
    }
    fn exchange(payload:&[u8],rights:&[RawFd])->Result<(Admitted,NativeReceipt)> {
        let (tx,rx)=socket_pair()?;
        send_packet(tx.as_raw_fd(),payload,rights)?;
        let mut receipt=NativeReceipt::default();
        let admitted=receive_observed(rx.as_raw_fd(),&mut receipt)?;
        Ok((admitted,receipt))
    }
    fn refusal(label:&str,payload:&[u8],rights:&[RawFd],expected:Refusal) {
        let before=count_fds();
        let error=exchange(payload,rights).unwrap_err();
        assert_eq!(error,expected,"{label}");
        assert_eq!(count_fds(),before,"{label} leaked a received descriptor");
        println!("native_wire_refused={label} reason={error:?} descriptors=settled");
    }

    // Adapted existing native_seqpacket_admission_and_refusals qualification.
    // Ordinary self/socket/file operations only: no fork/exec/signal/cgroup or
    // global setting. All fixture files remain for enclosing raw preservation.
    #[test]
    fn native_seqpacket_admission_and_refusals() {
        let uid=unsafe{getuid()};let gid=unsafe{getgid()};
        assert_ne!(uid,0,"ordinary user required");assert_ne!(gid,0,"ordinary group required");
        assert_eq!(unsafe{geteuid()},uid);assert_eq!(unsafe{getegid()},gid);
        let beginning=count_fds();
        let root=std::env::temp_dir().join(format!("ptg-wire-v4-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let meta=fs::symlink_metadata(&root).unwrap();
        assert!(meta.is_dir() && meta.uid()==uid && meta.mode()&0o777==0o700);
        let mut input=fs::OpenOptions::new().create_new(true).read(true).write(true).open(root.join("input")).unwrap();
        input.write_all(b"AB").unwrap();input.seek(SeekFrom::Start(1)).unwrap();
        let output=fs::OpenOptions::new().create_new(true).write(true).open(root.join("output")).unwrap();
        let error=fs::OpenOptions::new().create_new(true).write(true).open(root.join("error")).unwrap();
        let cwd=fs::OpenOptions::new().read(true).custom_flags(O_PATH|O_DIRECTORY).open(&root).unwrap();
        // Real pre-bootstrap capture, including original direct parent and
        // complete auto chain. No owner is signalled, cloned or executed.
        let original=OriginalInvocation::capture(OwnerMode::Manual,60.25).unwrap().acknowledge_separate_service(SeparateService::Acknowledged);
        assert_eq!(original.metadata().identities.len(),2);
        assert_eq!(original.metadata().identities[0].pid,std::process::id());
        assert_eq!(original.metadata().identities[0].ppid,original.metadata().identities[1].pid);
        assert_eq!(original.metadata().deadline_ns-original.metadata().captured_ns,60_250_000_000);
        original.revalidate().unwrap();
        let mut rights:Vec<_>=original.pidfds().iter().map(AsRawFd::as_raw_fd).collect();
        rights.extend([input.as_raw_fd(),output.as_raw_fd(),error.as_raw_fd(),cwd.as_raw_fd()]);
        let owner_count=original.pidfds().len();
        let frame=request::encode(original.metadata(),&[b"/never-executed",b"",b"\xff native"],&[b"KEY=\xfe value",b"EMPTY="]).unwrap();
        let before=count_fds();
        let (accepted,receipt)=exchange(&frame,&rights).unwrap();
        assert_eq!(receipt.rights,6);assert_eq!(receipt.original_watchers,2);assert!(receipt.connection_verified);
        assert_eq!(receipt.peer,Some(Credentials{pid:std::process::id() as c_int,uid,gid}));
        assert_eq!(receipt.sender,receipt.peer);assert!(receipt.io_flags.is_some());
        assert_eq!(accepted.invocation.metadata(),original.metadata());
        assert_eq!(accepted.plan.environment()[0].as_bytes(),b"KEY=\xfe value");
        assert_eq!(accepted.plan.arguments()[2].as_bytes(),b"\xff native");
        for fd in accepted.invocation.owners().iter().chain([&accepted.io.stdio[0],&accepted.io.stdio[1],&accepted.io.stdio[2],&accepted.io.cwd]) {
            close_on_exec(fd.as_raw_fd()).unwrap();assert!(!rights.contains(&fd.as_raw_fd()));
        }
        let Admitted{invocation:received_owners,io:CallerIo{stdio:[received_in,received_out,received_err],cwd:received_cwd},plan,..}=accepted;
        let mut received_in=fs::File::from(received_in);
        let mut byte=[0];received_in.read_exact(&mut byte).unwrap();assert_eq!(&byte,b"B");
        assert_eq!(input.stream_position().unwrap(),2,"SCM transfer must preserve the open-file description");
        let mut received_out=fs::File::from(received_out);received_out.write_all(b"out").unwrap();
        let mut received_err=fs::File::from(received_err);received_err.write_all(b"err").unwrap();
        let received_cwd=fs::File::from(received_cwd);let held=received_cwd.metadata().unwrap();
        assert_eq!((held.dev(),held.ino()),(meta.dev(),meta.ino()));
        drop((received_owners,received_in,received_out,received_err,received_cwd,plan));
        assert_eq!(count_fds(),before,"accepted original-owner/IO request leaked descriptors");
        assert_eq!(fs::read(root.join("output")).unwrap(),b"out");
        assert_eq!(fs::read(root.join("error")).unwrap(),b"err");
        let auto=OriginalInvocation::capture(OwnerMode::Auto,60.25).unwrap().acknowledge_separate_service(SeparateService::Acknowledged);
        assert_eq!(auto.metadata().identities.last().unwrap().pid,1);
        assert_eq!(auto.metadata().identities.last().unwrap().ppid,0);
        let mut auto_rights:Vec<_>=auto.pidfds().iter().map(AsRawFd::as_raw_fd).collect();auto_rights.extend_from_slice(&rights[owner_count..]);
        let auto_frame=request::encode(auto.metadata(),&[b"/never-executed"],&[]).unwrap();
        let auto_before=count_fds();let (accepted,receipt)=exchange(&auto_frame,&auto_rights).unwrap();
        assert_eq!(receipt.original_watchers,auto.pidfds().len());
        assert_eq!(accepted.invocation.metadata(),auto.metadata());drop(accepted);
        assert_eq!(count_fds(),auto_before,"auto owner transfer leaked descriptors");
        println!("native_original_owners manual=2 auto={} captured_ns={} cutoff_ns={} requested_bits={} absolute_unchanged=true",auto.pidfds().len(),original.metadata().captured_ns,original.metadata().deadline_ns,original.metadata().seconds.to_bits());
        drop(auto);
        refusal("missing_right",&frame,&rights[..rights.len()-1],Refusal::RightsCount(5));
        let mut extra=rights.clone();extra.push(rights[owner_count]);
        refusal("extra_right",&frame,&extra,Refusal::RightsCount(7));
        // Preserve the genuine kernel CTRUNC/adopt/close assertion after the
        // production capacity increased to1088B for253 rights. Only this call
        // supplies a small natively aligned buffer to the SAME receive logic.
        let trunc_before=count_fds();
        let (tx,rx)=socket_pair().unwrap();send_packet(tx.as_raw_fd(),&frame,&[rights[0];32]).unwrap();
        let mut small=[0u64;12];
        assert_eq!(receive_with_control(rx.as_raw_fd(),&mut NativeReceipt::default(),&mut small).unwrap_err(),Refusal::ControlTruncated);
        drop((tx,rx));assert_eq!(count_fds(),trunc_before,"control_truncated leaked a received descriptor");
        println!("native_wire_refused=control_truncated test_control_bytes=96 production_control_bytes={CONTROL_BYTES} descriptors=settled");
        let mut wrong_in=rights.clone();wrong_in[owner_count]=output.as_raw_fd();
        refusal("write_only_stdin",&frame,&wrong_in,Refusal::IoAccess);
        let readonly=fs::File::open(root.join("output")).unwrap();
        let mut wrong_out=rights.clone();wrong_out[owner_count+1]=readonly.as_raw_fd();
        refusal("read_only_stdout",&frame,&wrong_out,Refusal::IoAccess);
        let mut wrong_err=rights.clone();wrong_err[owner_count+2]=readonly.as_raw_fd();
        refusal("read_only_stderr",&frame,&wrong_err,Refusal::IoAccess);
        let regular_path=fs::OpenOptions::new().read(true).custom_flags(O_PATH).open(root.join("input")).unwrap();
        let mut wrong_cwd=rights.clone();wrong_cwd[owner_count+3]=regular_path.as_raw_fd();
        refusal("regular_cwd",&frame,&wrong_cwd,Refusal::WorkingDirectory);
        let mut wrong_owner=rights.clone();wrong_owner[0]=input.as_raw_fd();
        refusal("file_instead_of_owner_pidfd",&frame,&wrong_owner,Refusal::NotPidfd);
        let mut wrong_ancestor=rights.clone();wrong_ancestor[1]=rights[0];
        refusal("self_instead_of_parent_pidfd",&frame,&wrong_ancestor,Refusal::Invocation(invocation::InvocationError::Kernel("WrongPidfd".into())));
        let mut expired=original.metadata().clone();expired.captured_ns=1;expired.deadline_ns=60_250_000_001;
        let expired=request::encode(&expired,&[b"/never-executed"],&[]).unwrap();
        refusal("original_deadline_expired",&expired,&rights,Refusal::Invocation(invocation::InvocationError::Expired));
        assert_eq!(invocation::InvocationError::Expired.exit_code(),124);
        let mut future=original.metadata().clone();future.captured_ns=supervisor::boottime_ns().unwrap()+3_600_000_000_000;
        future.deadline_ns=future.captured_ns+60_250_000_000;
        let future=request::encode(&future,&[b"/never-executed"],&[]).unwrap();
        refusal("future_original_start",&future,&rights,Refusal::Invocation(invocation::InvocationError::InvalidTime));
        for ack in [0,2]{let mut unacknowledged=frame.clone();unacknowledged[7]=ack;refusal("missing_or_invalid_context_ack",&unacknowledged,&rights,Refusal::InvalidRequest(request::Refusal::Context));}
        let mut invalid_umask=frame.clone();invalid_umask[38..40].copy_from_slice(&0o1000u16.to_le_bytes());
        refusal("invalid_caller_umask",&invalid_umask,&rights,Refusal::InvalidRequest(request::Refusal::Umask));
        let mut malformed=frame.clone();malformed[5]=b'2';
        refusal("wrong_version",&malformed,&rights,Refusal::InvalidRequest(request::Refusal::Version));
        let mut malformed=frame.clone();malformed.push(0);
        refusal("trailing_bytes",&malformed,&rights,Refusal::InvalidRequest(request::Refusal::TrailingBytes));
        let large=vec![b'x';request::MAX_PACKET_BYTES+1];
        refusal("payload_truncated",&large,&rights,Refusal::PayloadTruncated);
        let (tx,rx)=socket_pair().unwrap();drop(tx);
        let before_eof=count_fds();assert_eq!(receive(rx.as_raw_fd()).unwrap_err(),Refusal::EmptyPacket);
        assert_eq!(count_fds(),before_eof);drop(rx);
        drop((readonly,regular_path,original,input,output,error,cwd));
        assert_eq!(count_fds(),beginning,"native test left owned descriptors");
        println!("native_wire=PTGEX6 manual_rights=6 max_rights=253 io_ofd=preserved cwd_identity=verified env_native_bytes=preserved fixture={} fixtures_retained=true",root.display());
    }
    #[test]
    fn delegation_policy_is_exact_bounded_and_deny_default(){
        let header=b"PTG-DELEGATION-1\ncontext separate-service\n";
        let empty=delegation::parse(header).unwrap();assert!(!empty.permits(1));assert!(!empty.permits(501));
        let policy=delegation::parse(b"PTG-DELEGATION-1\ncontext separate-service\nuid 1\nuid 501\nuid 4294967294\n").unwrap();
        for uid in [1,501,4294967294]{assert!(policy.permits(uid));}for uid in [0,2,500,502,u32::MAX]{assert!(!policy.permits(uid));}
        for suffix in ["uid 0\n","uid 01\n","uid +1\n","uid -1\n","uid 4294967295\n","uid 4294967296\n","uid 99999999999999999999\n","uid 1","uid 1\r\n","uid 1 \n","uid 1\n\n","uid 1\nuid 1\n","uid 2\nuid 1\n","uid *\n","# comment\n","unknown 1\n","uid \n","uid １\n"]{
            let mut bytes=header.to_vec();bytes.extend_from_slice(suffix.as_bytes());assert!(delegation::parse(&bytes).is_err(),"accepted {suffix:?}");
        }
        for bytes in [&b""[..],&b"PTG-DELEGATION-1\ncontext inherited\n"[..],&b"PTG-DELEGATION-1\r\ncontext separate-service\n"[..],&b"PTG-DELEGATION-1\ncontext separate-service"[..]]{assert!(delegation::parse(bytes).is_err());}
        let mut maximum=header.to_vec();for uid in 1..=64{maximum.extend_from_slice(format!("uid {uid}\n").as_bytes());}assert!(delegation::parse(&maximum).unwrap().permits(64));
        maximum.extend_from_slice(b"uid 65\n");assert_eq!(delegation::parse(&maximum).unwrap_err(),delegation::Refusal::Count);
        let mut oversized=header.to_vec();oversized.resize(4097,b'x');assert_eq!(delegation::parse(&oversized).unwrap_err(),delegation::Refusal::Size);
    }
    #[test]
    fn separate_context_ack_precedes_client_capture_and_activation(){
        let transparent=run_library(&[],f64::NAN,OwnerMode::Manual);assert_eq!(transparent.exit_code(),125);
        assert!(transparent.error().unwrap().contains("transparent Linux execution context"));assert!(!transparent.complete());assert!(!transparent.may_have_active_service());assert_eq!(transparent.wire_evidence(),(&[][..],&[][..]));
        let cli=run_cli(vec!["auto".into(),"--timeout".into(),"invalid".into()]);assert_eq!(cli.exit_code(),125);assert!(cli.error().unwrap().contains("--separate-service-context"));assert!(!cli.may_have_active_service());assert_eq!(cli.wire_evidence(),(&[][..],&[][..]));
        supervisor::native_activation::mode2_host_policy_regression();
        let acknowledged=run_library_delegated(&[],0.0,OwnerMode::Manual,SeparateService::Acknowledged);assert_eq!(acknowledged.exit_code(),125);assert!(acknowledged.error().unwrap().contains("InvalidDuration"));assert!(!acknowledged.may_have_active_service());
    }
}

pub(crate) fn client_exit(result:ClientOutcome)->!{client::exit(result)}
