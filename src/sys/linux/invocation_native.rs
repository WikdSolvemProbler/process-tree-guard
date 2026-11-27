// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Shared Linux original-owner descriptor and boot-clock primitives.

use std::ffi::{c_int, c_long, c_short};
use std::fs;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

const SYS_PIDFD_OPEN: c_long = 434;
const F_GETFD: c_int = 1;
const FD_CLOEXEC: c_int = 1;
pub(super) const CLOCK_BOOTTIME: c_int = 7;
pub(super) const NS_PER_SECOND: u64 = 1_000_000_000;

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct PollFd {
    pub(super) fd: c_int,
    pub(super) events: c_short,
    pub(super) revents: c_short,
}

pub(super) const POLLIN: c_short = 1;

#[repr(C)]
#[derive(Default)]
pub(super) struct Timespec {
    pub(super) seconds: i64,
    pub(super) nanoseconds: i64,
}

unsafe extern "C" {
    pub(super) fn syscall(number: c_long, ...) -> c_long;
    pub(super) fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    pub(super) fn poll(fds: *mut PollFd, count: usize, timeout: c_int) -> c_int;
    pub(super) fn clock_gettime(clock: c_int, time: *mut Timespec) -> c_int;
}

#[derive(Debug)]
pub(super) enum PidfdError {
    Os(Option<c_int>),
    NotCloseOnExec,
    NotPidfd,
    WrongPidfd,
    DeadPidfd,
}

impl From<io::Error> for PidfdError {
    fn from(error: io::Error) -> Self {
        Self::Os(error.raw_os_error())
    }
}

impl std::fmt::Display for PidfdError {
    fn fmt(&self, format: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Preserve the previous separate-service error text for these cases.
        match self {
            Self::Os(code) => write!(format, "Os({code:?})"),
            other => write!(format, "{other:?}"),
        }
    }
}

pub(super) fn pidfd_for(pid: c_int) -> Result<OwnedFd, PidfdError> {
    let fd = unsafe { syscall(SYS_PIDFD_OPEN, pid, 0 as c_int) };
    if fd < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // pidfd_open returns a new owned descriptor on success.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as RawFd) })
}

#[derive(Default)]
pub(super) struct PidfdObservation {
    pub(super) cloexec: bool,
    pub(super) pid: Option<c_int>,
    pub(super) revents: Option<c_short>,
}

pub(super) fn validate_pidfd(descriptor: &OwnedFd, pid: c_int, observed: &mut PidfdObservation) -> Result<(), PidfdError> {
    let fd = descriptor.as_raw_fd();
    let flags = unsafe { fcntl(fd, F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if flags & FD_CLOEXEC == 0 {
        return Err(PidfdError::NotCloseOnExec);
    }
    observed.cloexec = true;
    if fs::read_link(format!("/proc/self/fd/{fd}"))?.as_os_str() != "anon_inode:[pidfd]" {
        return Err(PidfdError::NotPidfd);
    }
    let information = fs::read_to_string(format!("/proc/self/fdinfo/{fd}"))?;
    observed.pid = information.lines().find_map(|line| line.strip_prefix("Pid:"))
        .and_then(|field| field.trim().parse().ok());
    if observed.pid != Some(pid) {
        return Err(PidfdError::WrongPidfd);
    }
    let mut watched = PollFd { fd, events: POLLIN, revents: 0 };
    let ready = unsafe { poll(&mut watched, 1, 0) };
    if ready < 0 {
        return Err(io::Error::last_os_error().into());
    }
    observed.revents = Some(watched.revents);
    if ready != 0 {
        return Err(PidfdError::DeadPidfd);
    }
    Ok(())
}

pub(super) fn boottime_ns() -> Result<u64, String> {
    let mut time = Timespec::default();
    if unsafe { clock_gettime(CLOCK_BOOTTIME, &mut time) } < 0 {
        return Err(PidfdError::from(io::Error::last_os_error()).to_string());
    }
    let seconds = u64::try_from(time.seconds).map_err(|_| "negative boot time")?;
    let nanoseconds = u64::try_from(time.nanoseconds).map_err(|_| "negative boot fraction")?;
    if nanoseconds >= NS_PER_SECOND {
        return Err("invalid boot fraction".into());
    }
    seconds.checked_mul(NS_PER_SECOND).and_then(|value| value.checked_add(nanoseconds))
        .ok_or_else(|| "boot time overflow".into())
}
