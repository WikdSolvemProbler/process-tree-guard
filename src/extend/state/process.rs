// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! A process opened by handle: its image, its parent, and the shell that encloses it.

use super::*;


use crate::sys::windows::{

    Bool, CloseHandle, GetProcessTimes, Handle, IsProcessInJob, NtQueryInformationProcess,

    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, ProcessBasicInformation,

    QueryFullProcessImageNameW, SYNCHRONIZE, WAIT_TIMEOUT, WaitForSingleObject,

};
/// A process opened by handle, with the time it was created.
pub(in crate::extend) struct Process {
    pub(in crate::extend) handle: Handle,
    pub(in crate::extend) id: u32,
    pub(in crate::extend) created: u64,
}

unsafe impl Send for Process {}

impl Process {
    pub(in crate::extend) fn open(id: u32) -> Option<Process> {
        if id == 0 {
            return None;
        }
        let handle = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, id) };
        if handle.is_null() {
            return None;
        }
        let (mut created, mut exited, mut kernel, mut user) = (0u64, 0u64, 0u64, 0u64);
        if unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
            unsafe { CloseHandle(handle) };
            return None;
        }
        Some(Process { handle, id, created })
    }

    pub(in crate::extend) fn alive(&self) -> bool {
        unsafe { WaitForSingleObject(self.handle, 0) == WAIT_TIMEOUT }
    }

    /// True when the process is alive and began no later than `connected_at`, a time in
    /// the units of its creation time, read from the system (wall) clock: one that began
    /// after the client connected is not that client, but a process that took over its
    /// identifier. `connected_at` is when the guard learned of the connection, so a
    /// process that took the identifier between the connection and that moment passes.
    pub(in crate::extend) fn started_before(&self, connected_at: u64) -> bool {
        self.created <= connected_at && self.alive()
    }

    pub(in crate::extend) fn in_job(&self, job: Handle) -> bool {
        let mut result: Bool = 0;
        unsafe { IsProcessInJob(self.handle, job, &mut result) != 0 && result != 0 }
    }

    /// The path of the program the process runs, as Windows spells it.
    pub(in crate::extend) fn image_path(&self) -> Option<String> {
        let mut buffer = [std::mem::MaybeUninit::<u16>::uninit(); 1024];
        let mut size = buffer.len() as u32;
        if unsafe { QueryFullProcessImageNameW(self.handle, 0, buffer.as_mut_ptr() as *mut u16, &mut size) } == 0 {
            return None;
        }
        // On success the call sets `size` to the unit count it wrote;
        // the read is confined to that prefix.
        Some(String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u16, size as usize) }))
    }

    /// The file name of the program the process runs.
    pub(in crate::extend) fn image(&self) -> String {
        // Only the name is wanted, so only the name is converted:
        // building the full path as a string first (as `image_path`
        // does) allocated and scanned the whole path, folder
        // included, for a tail of a dozen or so units, on the path
        // of every connection the run serves. The cut is made in
        // the reported units at the last separator unit: a
        // separator is a plain ASCII unit, never half of a
        // surrogate pair, so the tail's lossy text is exactly the
        // file name `image_file_name` cuts from the full string,
        // and a path that ends at its folder names nothing, as
        // there.
        let mut buffer = [std::mem::MaybeUninit::<u16>::uninit(); 1024];
        let mut size = buffer.len() as u32;
        if unsafe { QueryFullProcessImageNameW(self.handle, 0, buffer.as_mut_ptr() as *mut u16, &mut size) } == 0 {
            return "unknown".into();
        }
        // On success the call sets `size` to the unit count it wrote;
        // the read is confined to that prefix.
        let units = unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u16, size as usize) };
        let start = units.iter().rposition(|&unit| unit == b'\\' as u16 || unit == b'/' as u16).map_or(0, |at| at + 1);
        if start == units.len() {
            return "unknown".into();
        }
        String::from_utf16_lossy(&units[start..])
    }

    pub(in crate::extend) fn parent_id(&self) -> Option<u32> {
        unsafe {
            let mut info: ProcessBasicInformation = std::mem::zeroed();
            let status = NtQueryInformationProcess(self.handle, 0, &mut info as *mut _ as *mut c_void,
                std::mem::size_of::<ProcessBasicInformation>() as u32, std::ptr::null_mut());
            if status == 0 { Some(info.parent_process_id as u32) } else { None }
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

/// The file name of the program a process runs, or "unknown" when it cannot be read.
pub(crate) fn image_of(id: u32) -> String {
    Process::open(id).map_or_else(|| "unknown".into(), |process| process.image())
}

/// The file name of an image path, as `Process::image` reports it:
/// the path's own tail, cut off the string already built instead of
/// copied into a second string while the first is dropped, and
/// "unknown" for a path that ends at its folder.
pub(in crate::extend) fn image_file_name(mut path: String) -> String {
    let start = path.rfind(['\\', '/']).map_or(0, |at| at + 1);
    if start == path.len() {
        return "unknown".into();
    }
    path.drain(..start);
    path
}

/// Git Bash runs a program by forking: the program's parent is a copy of the shell
/// that ends as soon as the program does, so a hold on it would end with the program.
/// The hold goes to the shell that started the copy, which is the same program, older,
/// alive and in the job. Anything that is not such a copy is returned as it is.
/// The image path the walk last read for the process it returns comes back
/// with it: the walk read that path to test the process, and a caller that
/// needs the process's name next would otherwise query the same path a
/// second time to get it.
pub(in crate::extend) fn enclosing_shell(mut process: Process, job: Handle) -> (Process, Option<String>) {
    // The image path of the process the walk stands on, read once and
    // carried up with it: the parent candidate's path is read to test
    // it against the child's, and when the walk moves up to that
    // parent the carried string is the one the next level reads —
    // reading it again there paid a second image query and a second
    // string for every level the walk climbed.
    let mut carried_path = process.image_path();
    for _ in 0..MAX_SHELL_COPIES {
        let Some(path) = carried_path.as_deref() else { break };
        // The name is compared in place, case-insensitively: copying
        // it out and lowering the copy allocated twice per level of
        // the walk, on the path of every parent-target request.
        let name = crate::file_name_of(path);
        if !name.eq_ignore_ascii_case("bash.exe") && !name.eq_ignore_ascii_case("sh.exe") {
            break;
        }
        let step = process.parent_id().and_then(Process::open).and_then(|up| {
            if up.created < process.created && up.alive() && up.in_job(job) {
                up.image_path().filter(|other| other.eq_ignore_ascii_case(path)).map(|other| (up, other))
            } else {
                None
            }
        });
        match step {
            Some((up, other)) => {
                process = up;
                carried_path = Some(other);
            }
            None => break,
        }
    }
    (process, carried_path)
}

/// The copies of a shell that are looked through, at most.
pub(in crate::extend) const MAX_SHELL_COPIES: usize = 8;
