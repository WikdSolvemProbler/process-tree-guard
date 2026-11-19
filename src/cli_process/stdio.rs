// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Standard handles and console-mode probes for process-owning and returning calls.

use super::*;


use crate::sys::windows::{

    CreateFileW, FILE_SHARE_READ_AND_WRITE, FILE_TYPE_PIPE, GENERIC_READ, GetConsoleMode, GetFileType,

    GetStdHandle, Handle,

    INVALID_HANDLE, OPEN_EXISTING, ReadFile, STD_ERROR_HANDLE, STD_INPUT_HANDLE,

    STD_OUTPUT_HANDLE, WriteConsoleW, WriteFile, wide,

};
#[derive(Clone, Copy)]
struct ProbedHandle {
    handle: Handle,
    console: bool,
}

struct CachedInput {
    handle: Handle,
    console: std::sync::OnceLock<bool>,
}

// These borrowed handles belong to the process. Keeping and sharing the raw
// values does not transfer ownership, and the console verdict is initialized
// at most once for each cached handle.
unsafe impl Send for ProbedHandle {}
unsafe impl Sync for ProbedHandle {}
unsafe impl Send for CachedInput {}
unsafe impl Sync for CachedInput {}

impl CachedInput {
    fn probed(&self, is_console: impl FnOnce(Handle) -> bool) -> ProbedHandle {
        ProbedHandle {
            handle: self.handle,
            console: *self.console.get_or_init(|| is_console(self.handle)),
        }
    }
}

static STANDARD_INPUT: std::sync::OnceLock<CachedInput> = std::sync::OnceLock::new();
static STANDARD_OUTPUTS: [std::sync::OnceLock<ProbedHandle>; 2] =
    [std::sync::OnceLock::new(), std::sync::OnceLock::new()];

fn stdin_handle() -> &'static CachedInput {
    STANDARD_INPUT.get_or_init(|| CachedInput {
        handle: unsafe { GetStdHandle(STD_INPUT_HANDLE) },
        console: std::sync::OnceLock::new(),
    })
}

fn probed_handle(which: u32) -> ProbedHandle {
    let slot = match which {
        STD_INPUT_HANDLE => return stdin_handle().probed(has_console_mode),
        STD_OUTPUT_HANDLE => &STANDARD_OUTPUTS[0],
        STD_ERROR_HANDLE => &STANDARD_OUTPUTS[1],
        _ => return ProbedHandle { handle: std::ptr::null_mut(), console: false },
    };
    *slot.get_or_init(|| {
        let (handle, console) = probe_console_handle(which);
        ProbedHandle { handle, console }
    })
}

/// A process-owning CLI captures each handle once. Launching a child needs
/// only the raw input handle. Output verdicts are captured before console
/// cleanup can change the active console. Returning callers fetch live.
pub(crate) fn cached_std_handle(which: u32) -> Option<Handle> {
    if !active() { return None; }
    Some(if which == STD_INPUT_HANDLE { stdin_handle().handle } else { probed_handle(which).handle })
}

/// The standard stream `which` as this invocation resolved it at entry,
/// for a writer on another thread: `GetStdHandle` read after a run can
/// name another object once the console cleanup has freed and attached
/// consoles, so such a writer takes the handle from here instead.
pub(crate) fn std_line_target(which: u32) -> (Handle, bool) {
    line_target(which)
}

/// Resolve a handle and its console verdict. A process-owning CLI uses its
/// cached handle. Returning callers probe the freshly fetched handle.
fn line_target(which: u32) -> (Handle, bool) {
    if active() {
        let probed = probed_handle(which);
        (probed.handle, probed.console)
    } else {
        probe_console_handle(which)
    }
}

fn probe_console_handle(which: u32) -> (Handle, bool) {
    let handle = unsafe { GetStdHandle(which) };
    (handle, has_console_mode(handle))
}

fn has_console_mode(handle: Handle) -> bool {
    if handle.is_null() || handle == INVALID_HANDLE {
        return false;
    }
    let mut mode = 0u32;
    unsafe { GetConsoleMode(handle, &mut mode) != 0 }
}

#[cfg(test)]
pub(super) fn stdin_mode_cache_regression() {
    let handle = 17usize as Handle;
    let input = CachedInput { handle, console: std::sync::OnceLock::new() };
    assert_eq!(input.handle, handle);
    assert!(input.console.get().is_none());
    let first = input.probed(|observed| {
        assert_eq!(observed, handle);
        false
    });
    let again = input.probed(|_| panic!("cached input must not probe again"));
    assert_eq!(first.handle, again.handle);
    assert!(!first.console && !again.console);
    assert!(!has_console_mode(std::ptr::null_mut()));
    assert!(!has_console_mode(INVALID_HANDLE));
}

/// Standard input for the hook vendors, served on its raw handle when it
/// is not a console: the handle is fetched and probed once, and reads go
/// straight to `ReadFile`. A console keeps the standard path, whose bytes
/// need its UTF-16 conversion. The handle is never closed; process exit
/// reclaims it.
pub(crate) enum StandardInput {
    Raw(Handle),
    Std(std::io::Stdin),
}

pub(crate) fn standard_input() -> StandardInput {
    let (handle, console) = line_target(STD_INPUT_HANDLE);
    if !handle.is_null() && handle != INVALID_HANDLE && !console {
        return StandardInput::Raw(handle);
    }
    StandardInput::Std(std::io::stdin())
}

impl std::io::Read for StandardInput {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            StandardInput::Raw(handle) => {
                if buf.is_empty() {
                    return Ok(0);
                }
                let mut read = 0u32;
                let ok = unsafe {
                    ReadFile(*handle, buf.as_mut_ptr() as *mut c_void, buf.len() as u32, &mut read, std::ptr::null_mut())
                };
                if ok == 0 {
                    let error = std::io::Error::last_os_error();
                    // A pipe whose writer has closed ends with
                    // ERROR_BROKEN_PIPE (109): that is the end of the
                    // input, as the standard library's reader reports it,
                    // not a failure to read it.
                    if error.raw_os_error() == Some(109) { Ok(0) } else { Err(error) }
                } else { Ok(read as usize) }
            }
            StandardInput::Std(stdin) => stdin.read(buf),
        }
    }
}

/// The whole of a file as bytes. Returning callers use an owned file,
/// closed before this helper returns. A process-owning CLI reads through
/// its raw handle and leaves closure to process exit. Any failure gives None.
pub(crate) fn read_file_bytes(path: &str) -> Option<Vec<u8>> {
    if !active() {
        return std::fs::read(path).ok();
    }
    let wide = wide(path);
    // Shares include delete (4), as the standard file open shares it.
    let handle = unsafe {
        CreateFileW(wide.as_ptr(), GENERIC_READ, FILE_SHARE_READ_AND_WRITE | 4, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut())
    };
    if handle.is_null() || handle == INVALID_HANDLE {
        return None;
    }
    let mut bytes = Vec::new();
    // Uninitialised storage: each read writes the prefix whose length it
    // reports through `read`, and only that prefix is copied out, so no
    // byte is read before the read that wrote it and none needs clearing.
    let mut chunk = [std::mem::MaybeUninit::<u8>::uninit(); 8192];
    loop {
        let mut read = 0u32;
        let ok = unsafe { ReadFile(handle, chunk.as_mut_ptr() as *mut c_void, chunk.len() as u32, &mut read, std::ptr::null_mut()) };
        if ok == 0 {
            return None;
        }
        if read == 0 {
            return Some(bytes);
        }
        // The read wrote the `read` bytes the copy is confined to.
        bytes.extend_from_slice(unsafe { std::slice::from_raw_parts(chunk.as_ptr() as *const u8, read as usize) });
    }
}

/// The stack capacity of one written line, in bytes on the byte form
/// and in UTF-16 units on the console form: a line that fits is written
/// from stack storage, with the newline appended in place, and a longer
/// line takes the allocating form below, byte for byte the same.
const STACK_LINE: usize = 512;

/// Write one line to a standard stream on its raw handle, synchronously,
/// and ignore failures: a failed write must not change the caller's exit
/// code. Console handles take the wide form, as their bytes need the
/// UTF-16 conversion.
fn write_std_line(which: u32, text: &str) {
    let (handle, console) = line_target(which);
    let _ = write_text_to(handle, console, text, true);
}

/// Write complete UTF-8 text to a captured target, optionally appending one LF.
/// The caller owns the handle's lifetime; no standard stream is fetched here.
pub(crate) fn write_text_to(handle: Handle, console: bool, text: &str, append_newline: bool) -> bool {
    unsafe {
        if handle.is_null() || handle == INVALID_HANDLE {
            return false;
        }
        if console {
            // The text and any requested newline are written into stack
            // storage unit by unit; every unit is written before the
            // slice over the prefix is formed, so no unit is read
            // before the write that produced it.
            let mut storage = [std::mem::MaybeUninit::<u16>::uninit(); STACK_LINE];
            let mut count = 0usize;
            let mut fits = true;
            for unit in text.encode_utf16().chain(std::iter::once(b'\n' as u16).take(usize::from(append_newline))) {
                if count == storage.len() {
                    fits = false;
                    break;
                }
                storage[count].write(unit);
                count += 1;
            }
            if fits {
                let units = std::slice::from_raw_parts(storage.as_ptr() as *const u16, count);
                let mut done = 0usize;
                while done < units.len() {
                    let mut written = 0u32;
                    if WriteConsoleW(handle, units[done..].as_ptr() as *const c_void, (units.len() - done) as u32, &mut written, std::ptr::null_mut()) == 0 || written == 0 {
                        return false;
                    }
                    done += written as usize;
                }
                return true;
            }
            // The units are collected from the text directly, as
            // the stack form above encodes them: building the
            // line as a String first paid an allocation and a
            // copy of the whole text only to be encoded again
            // into the units that are written.
            let mut units: Vec<u16> = text.encode_utf16().collect();
            if append_newline { units.push(b'\n' as u16); }
            let mut done = 0usize;
            while done < units.len() {
                let mut written = 0u32;
                if WriteConsoleW(handle, units[done..].as_ptr() as *const c_void, (units.len() - done) as u32, &mut written, std::ptr::null_mut()) == 0 || written == 0 {
                    return false;
                }
                done += written as usize;
            }
            return true;
        }
        let length = text.len() + usize::from(append_newline);
        if length <= STACK_LINE {
            let mut storage = [std::mem::MaybeUninit::<u8>::uninit(); STACK_LINE];
            for (slot, &byte) in storage.iter_mut().zip(text.as_bytes()) {
                slot.write(byte);
            }
            if append_newline { storage[text.len()].write(b'\n'); }
            // The copy wrote the text and, when requested, its newline:
            // exactly the initialized `length` bytes the slice covers.
            let bytes = std::slice::from_raw_parts(storage.as_ptr() as *const u8, length);
            let mut done = 0usize;
            while done < bytes.len() {
                let mut written = 0u32;
                if WriteFile(handle, bytes[done..].as_ptr() as *const c_void, (bytes.len() - done) as u32, &mut written, std::ptr::null_mut()) == 0 || written == 0 {
                    return false;
                }
                done += written as usize;
            }
            return true;
        }
        let line = append_newline.then(|| format!("{}\n", text));
        let bytes = line.as_deref().unwrap_or(text).as_bytes();
        let mut done = 0usize;
        while done < bytes.len() {
            let mut written = 0u32;
            if WriteFile(handle, bytes[done..].as_ptr() as *const c_void, (bytes.len() - done) as u32, &mut written, std::ptr::null_mut()) == 0 || written == 0 {
                return false;
            }
            done += written as usize;
        }
        true
    }
}

/// Write a counted line to the target captured by this invocation before its
/// dedicated log worker starts. A later console attachment or standard-handle
/// replacement must not select another destination on the worker's first line.
/// The handle is borrowed: a detached or closed target may reject the write.
pub(crate) fn try_write_line_counted_to(handle: Handle, console: bool, text: &str) -> bool {
    if handle.is_null() || handle == INVALID_HANDLE {
        return false;
    }
    write_line_counted(handle, console, text)
}

/// The counted write itself, on a handle and a console verdict the
/// caller has already resolved (the captured-target form above),
/// for a caller whose grant stands only when its line was recorded.
/// The returned flag keeps the accounting of the extension log's
/// former private copy of this writer, branch by branch: on a
/// console it means the single `WriteConsoleW` call succeeded and
/// took every unit of the line (one call, never looped, unlike
/// `write_std_line` above — the line is one call so that no other
/// line of the log can land inside it — but a call that succeeds
/// while taking only part of the line has not written the line, and
/// counts as a failed write, as a partial take does in every other
/// writer of this tree); on a byte stream it means the line was
/// fully written after looping until every byte was taken.
fn write_line_counted(handle: Handle, console: bool, text: &str) -> bool {
    unsafe {
        if console {
            let mut storage = [std::mem::MaybeUninit::<u16>::uninit(); STACK_LINE];
            let mut count = 0usize;
            let mut fits = true;
            for unit in text.encode_utf16().chain(std::iter::once(b'\n' as u16)) {
                if count == storage.len() {
                    fits = false;
                    break;
                }
                storage[count].write(unit);
                count += 1;
            }
            if fits {
                let units = std::slice::from_raw_parts(storage.as_ptr() as *const u16, count);
                let mut written = 0u32;
                return WriteConsoleW(handle, units.as_ptr() as *const c_void, units.len() as u32, &mut written, std::ptr::null_mut()) != 0
                    && written as usize == units.len();
            }
            // Collected from the text directly, as the stack
            // form above encodes them, not through a String the
            // encoding would walk a second time.
            let mut units: Vec<u16> = text.encode_utf16().collect();
            units.push(b'\n' as u16);
            let mut written = 0u32;
            return WriteConsoleW(handle, units.as_ptr() as *const c_void, units.len() as u32, &mut written, std::ptr::null_mut()) != 0
                && written as usize == units.len();
        }
        if text.len() < STACK_LINE {
            let mut storage = [std::mem::MaybeUninit::<u8>::uninit(); STACK_LINE];
            for (slot, &byte) in storage.iter_mut().zip(text.as_bytes()) {
                slot.write(byte);
            }
            storage[text.len()].write(b'\n');
            let bytes = std::slice::from_raw_parts(storage.as_ptr() as *const u8, text.len() + 1);
            let mut done = 0usize;
            while done < bytes.len() {
                let mut written = 0u32;
                if WriteFile(handle, bytes[done..].as_ptr() as *const c_void, (bytes.len() - done) as u32, &mut written, std::ptr::null_mut()) == 0 || written == 0 {
                    return false;
                }
                done += written as usize;
            }
            return true;
        }
        let line = format!("{}\n", text);
        let bytes = line.as_bytes();
        let mut done = 0usize;
        while done < bytes.len() {
            let mut written = 0u32;
            if WriteFile(handle, bytes[done..].as_ptr() as *const c_void, (bytes.len() - done) as u32, &mut written, std::ptr::null_mut()) == 0 || written == 0 {
                return false;
            }
            done += written as usize;
        }
        true
    }
}

/// A rewriting hook may report success only after its decision reaches
/// stdout. Reject missing handles and non-console character devices such
/// as NUL, which report a successful write while discarding the decision.
pub(crate) fn try_write_stdout_line(text: &str) -> bool {
    const FILE_TYPE_DISK: u32 = 1;
    let (handle, console) = line_target(STD_OUTPUT_HANDLE);
    if handle.is_null() || handle == INVALID_HANDLE {
        return false;
    }
    if !console && !matches!(unsafe { GetFileType(handle) }, FILE_TYPE_DISK | FILE_TYPE_PIPE) {
        return false;
    }
    write_line_counted(handle, console, text)
}

/// Write one line to standard output: the hook's decision write.
pub(crate) fn write_stdout_line(text: &str) {
    write_std_line(STD_OUTPUT_HANDLE, text);
}

/// Write one line to standard error, synchronously, ignoring failures
/// exactly as the decision write on standard output does.
pub(crate) fn write_stderr_line(text: &str) {
    write_std_line(STD_ERROR_HANDLE, text);
}

/// The standard-error handle and its console verdict together, from
/// the handle's own slot on a process-owning invocation and from the
/// live probe otherwise: a caller that asks whether standard error
/// can hold a record before the run's handle loop reads the same
/// slot pays the handle's one probe there, and the handle loop reads
/// it back instead of probing the handle a second time.
pub(crate) fn standard_error_target() -> (Handle, bool) {
    line_target(STD_ERROR_HANDLE)
}
