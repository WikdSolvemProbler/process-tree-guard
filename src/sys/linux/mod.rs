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
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;

const AF_UNIX: c_int = 1;
const SOCK_SEQPACKET: c_int = 5;
const SOCK_NONBLOCK: c_int = 0o4000;
const SOCK_CLOEXEC: c_int = 0o2000000;
const SOL_SOCKET: c_int = 1;
#[cfg(test)]
const SO_SNDBUF: c_int = 7;
const SO_PASSCRED: c_int = 16;
const SO_PEERCRED: c_int = 17;
const SCM_RIGHTS: c_int = 1;
const SCM_CREDENTIALS: c_int = 2;
const MSG_CTRUNC: c_int = 0x08;
const MSG_TRUNC: c_int = 0x20;
const MSG_CMSG_CLOEXEC: c_int = 0x40000000;
const F_GETFD: c_int = 1;
const FD_CLOEXEC: c_int = 1;
const POLLIN: c_short = 1;
const SYS_PIDFD_OPEN: c_long = 434;
const CONTROL_WORDS: usize = 8;
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
enum Refusal {
    Os(Option<c_int>),
    EmptyPacket,
    PayloadTruncated,
    ControlTruncated,
    UnexpectedFlags,
    Ancillary,
    PeerCredentials,
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

fn peer_credentials(socket: RawFd) -> Result<Credentials> {
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
    if length != expected || peer.pid <= 0 || peer.uid == 0 || peer.gid == 0 {
        return Err(Refusal::PeerCredentials);
    }
    Ok(peer)
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
    let sent = os_result(unsafe { sendmsg(socket, &message, 0) })?;
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
struct Admitted {
    peer: Credentials,
    owner: OwnedFd,
    plan: request::ExecPlan,
}

#[cfg(test)]
fn receive(socket: RawFd) -> Result<Admitted> {
    receive_observed(socket, &mut NativeReceipt::default())
}

fn receive_observed(socket: RawFd, receipt: &mut NativeReceipt) -> Result<Admitted> {
    let peer = peer_credentials(socket)?;
    receipt.peer = Some(peer);
    let mut payload = vec![0u8; request::MAX_PACKET_BYTES];
    let mut vec = IoVec {
        base: payload.as_mut_ptr(),
        len: payload.len(),
    };
    let mut control = [0u64; CONTROL_WORDS];
    let mut message = message(&mut vec, control.as_mut_ptr().cast(), CONTROL_BYTES);
    let count = os_result(unsafe { recvmsg(socket, &mut message, MSG_CMSG_CLOEXEC) })?;
    if message.control_len > CONTROL_BYTES {
        return Err(Refusal::Ancillary);
    }
    // Only native recvmsg writes these bytes. Own any delivered fds before
    // considering truncation flags, identity, descriptor count or request bytes.
    let raw =
        unsafe { std::slice::from_raw_parts(control.as_ptr().cast::<u8>(), message.control_len) };
    let mut packet = ancillary(raw)?;
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
    if packet.rights.len() != 1 {
        return Err(Refusal::RightsCount(packet.rights.len()));
    }
    let owner = packet.rights.pop().expect("exactly one descriptor");
    validate_pidfd(&owner, peer, receipt)?;
    let frame = payload.get(..count).ok_or(Refusal::PayloadTruncated)?;
    let plan = request::decode(frame).map_err(Refusal::InvalidRequest)?;
    Ok(Admitted { peer, owner, plan })
}

mod supervisor;
pub(crate) use supervisor::{run_connection, LaunchContext, RunOutcome};
