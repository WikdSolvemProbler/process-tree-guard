// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Test control through handles acquired before the guard hardens itself.

use std::ffi::c_void;
use std::io;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, ExitStatus};

use super::common::{kill, refuse_prompt_in, wait_limited};

const CREATE_SUSPENDED: u32 = 0x0000_0004;
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
const THREAD_SUSPEND_RESUME: u32 = 0x0002;
const THREAD_QUERY_LIMITED_INFORMATION: u32 = 0x0800;
const ERROR_NO_MORE_FILES: i32 = 18;

#[repr(C)]
struct ThreadEntry {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
    fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
    fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry) -> i32;
    fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> *mut c_void;
    fn GetProcessIdOfThread(thread: *mut c_void) -> u32;
    fn SuspendThread(thread: *mut c_void) -> u32;
    fn ResumeThread(thread: *mut c_void) -> u32;
}

#[link(name = "ntdll")]
extern "system" {
    fn NtSuspendProcess(process: *mut c_void) -> i32;
    fn NtResumeProcess(process: *mut c_void) -> i32;
}

pub(super) struct TestGuard {
    child: Child,
    supervisor: Option<OwnedHandle>,
}

impl TestGuard {
    pub(super) fn spawn(mut command: Command, retain_supervisor: bool) -> io::Result<Self> {
        refuse_prompt_in(&command);
        if retain_supervisor {
            command.creation_flags(CREATE_SUSPENDED);
        }
        let mut guard = Self { child: command.spawn()?, supervisor: None };
        if retain_supervisor {
            // The sole thread has not entered the guard yet. Keep its handle
            // across the initial resume and subsequent DACL hardening.
            let thread = initial_thread(&guard.child)?;
            let previous = unsafe { ResumeThread(thread.as_raw_handle()) };
            check_suspend_count("initial ResumeThread", previous, 1)?;
            guard.supervisor = Some(thread);
        }
        Ok(guard)
    }

    pub(super) fn suspend_supervisor(&self) -> io::Result<Suspension<'_>> {
        let thread = self.supervisor.as_ref()
            .ok_or_else(|| io::Error::other("the supervisor handle was not retained"))?
            .as_handle();
        let previous = unsafe { SuspendThread(thread.as_raw_handle()) };
        if previous == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        let suspension = Suspension { target: Target::Thread(thread), active: true };
        // On an unexpected count, Drop undoes just this call's suspension.
        check_suspend_count("SuspendThread", previous, 0)?;
        Ok(suspension)
    }

    pub(super) fn suspend_process(&self) -> io::Result<Suspension<'_>> {
        // Child owns the original process handle returned by creation.
        let process = self.child.as_handle();
        let status = unsafe { NtSuspendProcess(process.as_raw_handle()) };
        check_ntstatus("NtSuspendProcess", status)?;
        Ok(Suspension { target: Target::Process(process), active: true })
    }

    pub(super) fn wait(&mut self, secs: u64) -> Option<ExitStatus> {
        let status = wait_limited(&mut self.child, secs);
        if status.is_none() {
            kill(&mut self.child);
        }
        status
    }
}

impl Drop for TestGuard {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            kill(&mut self.child);
        }
    }
}

enum Target<'a> {
    Thread(BorrowedHandle<'a>),
    Process(BorrowedHandle<'a>),
}

pub(super) struct Suspension<'a> {
    target: Target<'a>,
    active: bool,
}

impl Suspension<'_> {
    pub(super) fn resume(mut self) -> io::Result<()> {
        self.thaw()
    }

    fn thaw(&mut self) -> io::Result<()> {
        match self.target {
            Target::Thread(thread) => {
                let previous = unsafe { ResumeThread(thread.as_raw_handle()) };
                if previous != u32::MAX {
                    // Even an unexpected count means this call decremented it.
                    self.active = false;
                }
                check_suspend_count("ResumeThread", previous, 1)
            }
            Target::Process(process) => {
                let status = unsafe { NtResumeProcess(process.as_raw_handle()) };
                check_ntstatus("NtResumeProcess", status)?;
                self.active = false;
                Ok(())
            }
        }
    }
}

impl Drop for Suspension<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.thaw();
        }
    }
}

fn check_suspend_count(operation: &str, previous: u32, expected: u32) -> io::Result<()> {
    if previous == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    if previous != expected {
        return Err(io::Error::other(format!("{operation}: expected suspend count {expected}, got {previous}")));
    }
    Ok(())
}

fn check_ntstatus(operation: &str, status: i32) -> io::Result<()> {
    if status < 0 {
        return Err(io::Error::other(format!("{operation} failed with NTSTATUS {:#010x}", status as u32)));
    }
    Ok(())
}

fn initial_thread(child: &Child) -> io::Result<OwnedHandle> {
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw.is_null() || raw as isize == -1 {
        return Err(io::Error::last_os_error());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry: ThreadEntry = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<ThreadEntry>() as u32;
    entry.size = size;
    let mut found = None;
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) };
    while more != 0 {
        // Toolhelp may report a shorter record than the buffer it receives.
        if entry.size < 4 * std::mem::size_of::<u32>() as u32 {
            return Err(io::Error::other("thread snapshot omitted the owner process identifier"));
        }
        if entry.owner_process_id == child.id() && found.replace(entry.thread_id).is_some() {
            return Err(io::Error::other("new suspended guard has more than one thread"));
        }
        entry.size = size;
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) };
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_NO_MORE_FILES) {
        return Err(error);
    }
    let tid = found.ok_or_else(|| io::Error::other("no primary thread for the suspended guard"))?;
    let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME | THREAD_QUERY_LIMITED_INFORMATION, 0, tid) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
    let owner = unsafe { GetProcessIdOfThread(thread.as_raw_handle()) };
    if owner == 0 {
        return Err(io::Error::last_os_error());
    }
    if owner != child.id() {
        return Err(io::Error::other("primary thread does not belong to the suspended guard"));
    }
    Ok(thread)
}
