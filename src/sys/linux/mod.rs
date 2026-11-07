// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Linux one-shot admission. Backend selection remains disabled pending qualification.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

#[path = "request.rs"]
mod request;

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
// LP64 CMSG_SPACE(5*4)+CMSG_SPACE(12)=40+32=72; retain spare bounded room.
const CONTROL_WORDS: usize = 12;
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
    peer: Option<Credentials>,
    sender: Option<Credentials>,
    rights: usize,
    pidfd_cloexec: bool,
    pidfd_pid: Option<c_int>,
    pidfd_revents: Option<c_short>,
}

fn validate_pidfd(
    descriptor: &OwnedFd,
    credential: Credentials,
    receipt: &mut NativeReceipt,
) -> Result<()> {
    let fd = descriptor.as_raw_fd();
    close_on_exec(fd)?;
    receipt.pidfd_cloexec = true;
    if fs::read_link(format!("/proc/self/fd/{fd}"))?.as_os_str() != "anon_inode:[pidfd]" {
        return Err(Refusal::NotPidfd);
    }
    let information = fs::read_to_string(format!("/proc/self/fdinfo/{fd}"))?;
    let pid = information
        .lines()
        .find_map(|line| line.strip_prefix("Pid:"))
        .and_then(|field| field.trim().parse().ok());
    receipt.pidfd_pid = pid;
    if pid != Some(credential.pid) {
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
    owner: OwnedFd,
    plan: request::ExecPlan,
    io: CallerIo,
}

#[cfg(test)]
fn receive(socket: RawFd) -> Result<Admitted> {
    receive_observed(socket, &mut NativeReceipt::default())
}

fn receive_observed(socket: RawFd, receipt: &mut NativeReceipt) -> Result<Admitted> {
    validate_connection(socket,true)?;
    receipt.connection_verified=true;
    let peer = peer_credentials(socket)?;
    receipt.peer = Some(peer);
    let mut payload = vec![0u8; request::MAX_PACKET_BYTES];
    let mut vec = IoVec {
        base: payload.as_mut_ptr(),
        len: payload.len(),
    };
    let mut control = [0u64; CONTROL_WORDS];
    let mut message = message(&mut vec, control.as_mut_ptr().cast(), CONTROL_BYTES);
    let count = os_result(unsafe { recvmsg(socket, &mut message, MSG_CMSG_CLOEXEC | MSG_DONTWAIT) })?;
    // Only native recvmsg writes these bytes. Own any delivered fds before
    // considering truncation flags, identity, descriptor count or request bytes.
    let raw =
        unsafe { std::slice::from_raw_parts(control.as_ptr().cast::<u8>(), message.control_len.min(CONTROL_BYTES)) };
    let packet = ancillary(raw)?;
    if message.control_len>CONTROL_BYTES {return Err(Refusal::Ancillary);}
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
    if packet.rights.len() != 5 {
        return Err(Refusal::RightsCount(packet.rights.len()));
    }
    let [owner,stdin,stdout,stderr,cwd]:[OwnedFd;5]=packet.rights.try_into()
        .map_err(|rights:Vec<OwnedFd>|Refusal::RightsCount(rights.len()))?;
    for fd in [&owner,&stdin,&stdout,&stderr,&cwd] {close_on_exec(fd.as_raw_fd())?;}
    let mut flags=[0;4];
    for (index,fd) in [&stdin,&stdout,&stderr,&cwd].into_iter().enumerate() {flags[index]=io_flags(fd.as_raw_fd(),index)?;}
    receipt.io_flags=Some(flags);
    validate_pidfd(&owner, peer, receipt)?;
    let frame = payload.get(..count).ok_or(Refusal::PayloadTruncated)?;
    let plan = request::decode(frame).map_err(Refusal::InvalidRequest)?;
    Ok(Admitted { peer, owner, plan, io:CallerIo{stdio:[stdin,stdout,stderr],cwd} })
}

mod supervisor;
pub(crate) use supervisor::{run_received, RunOutcome};

/// Send the calling ordinary process's request to its connected privileged
/// one-shot peer. Borrowed IO/CWD are passed as actual kernel references; no
/// descriptor is reopened and neither peer's environment is read implicitly.
/// The client must already have NNP set (receiver verifies it); this routine
/// does not mutate that process-wide policy, launch a helper or wait forever.
pub(crate) fn send_request(
    connection:&OwnedFd,seconds:f64,arguments:&[&[u8]],environment:&[&[u8]],
    stdio:[BorrowedFd<'_>;3],cwd:BorrowedFd<'_>,
)->Result<()> {
    validate_connection(connection.as_raw_fd(),false)?;
    let peer=kernel_peer_credentials(connection.as_raw_fd())?;
    if peer.uid!=0 || peer.gid!=0 {return Err(Refusal::PeerCredentials);}
    let uid=unsafe{getuid()};let gid=unsafe{getgid()};
    if uid==0 || gid==0 || unsafe{geteuid()}!=uid || unsafe{getegid()}!=gid {return Err(Refusal::PeerCredentials);}
    for (index,fd) in stdio.iter().enumerate(){io_flags(fd.as_raw_fd(),index)?;}
    io_flags(cwd.as_raw_fd(),3)?;
    let packet=request::encode(seconds,arguments,environment).map_err(Refusal::InvalidRequest)?;
    let owner=pidfd_for(std::process::id() as c_int)?;
    send_packet(connection.as_raw_fd(),&packet,&[owner.as_raw_fd(),stdio[0].as_raw_fd(),
        stdio[1].as_raw_fd(),stdio[2].as_raw_fd(),cwd.as_raw_fd()])
}

#[cfg(test)]
mod tests {
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
        let root=std::env::temp_dir().join(format!("ptg-wire-v3-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let meta=fs::symlink_metadata(&root).unwrap();
        assert!(meta.is_dir() && meta.uid()==uid && meta.mode()&0o777==0o700);
        let mut input=fs::OpenOptions::new().create_new(true).read(true).write(true).open(root.join("input")).unwrap();
        input.write_all(b"AB").unwrap();input.seek(SeekFrom::Start(1)).unwrap();
        let output=fs::OpenOptions::new().create_new(true).write(true).open(root.join("output")).unwrap();
        let error=fs::OpenOptions::new().create_new(true).write(true).open(root.join("error")).unwrap();
        let cwd=fs::OpenOptions::new().read(true).custom_flags(O_PATH|O_DIRECTORY).open(&root).unwrap();
        let owner=pidfd_for(std::process::id() as c_int).unwrap();
        let rights=[owner.as_raw_fd(),input.as_raw_fd(),output.as_raw_fd(),error.as_raw_fd(),cwd.as_raw_fd()];
        let frame=request::encode(2.25,&[b"/never-executed",b"",b"\xff native"],&[b"KEY=\xfe value",b"EMPTY="]).unwrap();
        let before=count_fds();
        let (accepted,receipt)=exchange(&frame,&rights).unwrap();
        assert_eq!(receipt.rights,5);assert!(receipt.connection_verified);
        assert_eq!(receipt.peer,Some(Credentials{pid:std::process::id() as c_int,uid,gid}));
        assert_eq!(receipt.sender,receipt.peer);assert!(receipt.io_flags.is_some());
        assert_eq!(accepted.plan.environment()[0].as_bytes(),b"KEY=\xfe value");
        assert_eq!(accepted.plan.arguments()[2].as_bytes(),b"\xff native");
        for fd in [&accepted.owner,&accepted.io.stdio[0],&accepted.io.stdio[1],&accepted.io.stdio[2],&accepted.io.cwd] {
            close_on_exec(fd.as_raw_fd()).unwrap();
            assert!(!rights.contains(&fd.as_raw_fd()));
        }
        let Admitted{owner:received_owner,io:CallerIo{stdio:[received_in,received_out,received_err],cwd:received_cwd},plan,..}=accepted;
        let mut received_in=fs::File::from(received_in);
        let mut byte=[0];received_in.read_exact(&mut byte).unwrap();assert_eq!(&byte,b"B");
        assert_eq!(input.stream_position().unwrap(),2,"SCM transfer must preserve the open-file description");
        let mut received_out=fs::File::from(received_out);received_out.write_all(b"out").unwrap();
        let mut received_err=fs::File::from(received_err);received_err.write_all(b"err").unwrap();
        let received_cwd=fs::File::from(received_cwd);let held=received_cwd.metadata().unwrap();
        assert_eq!((held.dev(),held.ino()),(meta.dev(),meta.ino()));
        drop((received_owner,received_in,received_out,received_err,received_cwd,plan));
        assert_eq!(count_fds(),before,"accepted five-right request leaked descriptors");
        assert_eq!(fs::read(root.join("output")).unwrap(),b"out");
        assert_eq!(fs::read(root.join("error")).unwrap(),b"err");
        refusal("missing_right",&frame,&rights[..4],Refusal::RightsCount(4));
        let six=[rights[0],rights[1],rights[2],rights[3],rights[4],rights[1]];
        refusal("extra_right",&frame,&six,Refusal::RightsCount(6));
        refusal("control_truncated",&frame,&[rights[0];32],Refusal::ControlTruncated);
        let wrong_in=[rights[0],rights[2],rights[2],rights[3],rights[4]];
        refusal("write_only_stdin",&frame,&wrong_in,Refusal::IoAccess);
        let readonly=fs::File::open(root.join("output")).unwrap();
        let wrong_out=[rights[0],rights[1],readonly.as_raw_fd(),rights[3],rights[4]];
        refusal("read_only_stdout",&frame,&wrong_out,Refusal::IoAccess);
        let wrong_err=[rights[0],rights[1],rights[2],readonly.as_raw_fd(),rights[4]];
        refusal("read_only_stderr",&frame,&wrong_err,Refusal::IoAccess);
        let regular_path=fs::OpenOptions::new().read(true).custom_flags(O_PATH).open(root.join("input")).unwrap();
        let wrong_cwd=[rights[0],rights[1],rights[2],rights[3],regular_path.as_raw_fd()];
        refusal("regular_cwd",&frame,&wrong_cwd,Refusal::WorkingDirectory);
        let wrong_owner=[rights[1],rights[1],rights[2],rights[3],rights[4]];
        refusal("file_instead_of_owner_pidfd",&frame,&wrong_owner,Refusal::NotPidfd);
        let mut malformed=frame.clone();malformed[5]=b'2';
        refusal("wrong_version",&malformed,&rights,Refusal::InvalidRequest(request::Refusal::Version));
        let mut malformed=frame.clone();malformed.push(0);
        refusal("trailing_bytes",&malformed,&rights,Refusal::InvalidRequest(request::Refusal::TrailingBytes));
        let large=vec![b'x';request::MAX_PACKET_BYTES+1];
        refusal("payload_truncated",&large,&rights,Refusal::PayloadTruncated);
        let (tx,rx)=socket_pair().unwrap();drop(tx);
        let before_eof=count_fds();
        assert_eq!(receive(rx.as_raw_fd()).unwrap_err(),Refusal::EmptyPacket);
        assert_eq!(count_fds(),before_eof);drop(rx);
        drop((readonly,regular_path,owner,input,output,error,cwd));
        assert_eq!(count_fds(),beginning,"native test left owned descriptors");
        println!("native_wire=PTGEX3 rights=5 io_ofd=preserved cwd_identity=verified env_native_bytes=preserved fixture={} fixtures_retained=true",root.display());
    }
}
