// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Returning APIs own optional output; only the explicit CLI finalizer writes it.

use crate::sys::windows::{raw_handle, GetCurrentProcess, GetTickCount64, TerminateProcess};
use std::cell::{Cell, RefCell};
use std::io::Write;

const GRACE_MS: u64 = 500;
/// The largest backlog `cli_exit` writes synchronously, in total across
/// its records. 4096 bytes is one page: it covers every ordinary record
/// with ample headroom (the help texts are a few hundred bytes; error
/// and notice lines are shorter), while a larger backlog is bulk
/// output, where abandoning a blocked writer after the grace deadline
/// matters more than the thread spawn it costs.
const SYNC_MAX_BYTES: usize = 4096;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_RECORDS: usize = 256;
const TRUNCATED: &str = "\nPROCESS_GUARD_DIAGNOSTICS_TRUNCATED\n";

/// The standard stream for an optional diagnostic record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticStream {
    /// The record belongs on standard output.
    Stdout,
    /// The record belongs on standard error.
    Stderr,
}

/// Owned optional output, including its original newlines. Records retain their
/// order even when stdout and stderr refer to the same pipe or file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticOutput {
    pub stream: DiagnosticStream,
    pub text: String,
}

#[derive(Default)]
struct Pending {
    records: Vec<DiagnosticOutput>,
    bytes: usize,
    truncated: bool,
}

impl Pending {
    /// The borrowed-text form, retained beside `push_owned` for the
    /// tests that hold a line they did not build.
    #[cfg(test)]
    fn push(&mut self, stream: DiagnosticStream, text: &str) {
        self.push_owned(stream, text.to_owned());
    }

    /// `push` with the record's text already owned: the string is
    /// moved into the record, not copied into it. A caller that
    /// built the text itself (a formatted error, a joined trace, a
    /// timing record) hands it over whole.
    fn push_owned(&mut self, stream: DiagnosticStream, mut text: String) {
        if text.is_empty() || self.truncated { return; }
        let room = MAX_BYTES - TRUNCATED.len() - self.bytes;
        if self.records.len() >= MAX_RECORDS - 1 || text.len() > room {
            if self.records.len() < MAX_RECORDS - 1 {
                let mut end = room.min(text.len());
                while !text.is_char_boundary(end) { end -= 1; }
                if end > 0 {
                    text.truncate(end);
                    self.bytes += end;
                    self.records.push(DiagnosticOutput { stream, text });
                }
            }
            self.records.push(DiagnosticOutput { stream: DiagnosticStream::Stderr, text: TRUNCATED.to_owned() });
            self.bytes += TRUNCATED.len();
            self.truncated = true;
        } else {
            self.bytes += text.len();
            self.records.push(DiagnosticOutput { stream, text });
        }
    }
}

thread_local! {
    static PENDING: RefCell<Pending> = RefCell::new(Pending::default());
    static CAPTURE_DEPTH: Cell<usize> = const { Cell::new(0) };
    static PENDING_INITIALIZED: Cell<bool> = const { Cell::new(false) };
}

// Every production access to PENDING goes through this helper. Mark it only
// after the TLS slot itself has initialized, and before any fallible mutation.
fn with_pending<R>(f: impl FnOnce(&RefCell<Pending>) -> R) -> R {
    PENDING.with(|pending| {
        PENDING_INITIALIZED.with(|used| used.set(true));
        f(pending)
    })
}

thread_local! { static PROCESS_OWNERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

/// Only cli_exit establishes process ownership. Returning entry points may be
/// nested inside it, but cannot establish that ownership on their own.
pub(crate) struct ProcessOwnership;

impl ProcessOwnership {
    pub(crate) fn begin() -> Self {
        PROCESS_OWNERS.with(|owners| owners.set(owners.get() + 1));
        ProcessOwnership
    }
}

impl Drop for ProcessOwnership {
    fn drop(&mut self) {
        PROCESS_OWNERS.with(|owners| owners.set(owners.get() - 1));
    }
}

pub(crate) fn owns_process() -> bool { PROCESS_OWNERS.with(|owners| owners.get() > 0) }

/// A new outer invocation replaces output from the previous invocation on this
/// thread. Nested public calls append to the same capture without resetting it.
pub(crate) struct Invocation;

impl Invocation {
    pub(crate) fn begin() -> Self {
        CAPTURE_DEPTH.with(|depth| {
            let prior = depth.get();
            if PENDING_INITIALIZED.with(Cell::get) {
                // Every initialized invocation borrows before incrementing depth.
                // A failed nested borrow must leave depth unchanged too.
                with_pending(|pending| {
                    let mut pending = pending.borrow_mut();
                    if prior == 0 {
                        pending.records.clear();
                        pending.bytes = 0;
                        pending.truncated = false;
                    }
                });
            }
            depth.set(prior + 1);
        });
        Invocation
    }
}

impl Drop for Invocation {
    fn drop(&mut self) {
        if PENDING_INITIALIZED.with(Cell::get) {
            with_pending(|pending| {
                let _borrow = pending.borrow_mut();
                CAPTURE_DEPTH.with(|depth| depth.set(depth.get() - 1));
            });
        } else {
            CAPTURE_DEPTH.with(|depth| depth.set(depth.get() - 1));
        }
    }
}

/// The borrowed-text form, retained beside `stderr_owned` for
/// the tests that hold a line they did not build.
#[cfg(test)]
pub(crate) fn stderr_line(text: &str) {
    // The formatted line is already owned: it moves into its record
    // through `push_owned`, where routing it through `push` copied it
    // into a second string first.
    with_pending(|pending| pending.borrow_mut().push_owned(DiagnosticStream::Stderr, format!("{}\n", text)));
}

/// `stderr_line` for a text the caller built and owns: the newline
/// is appended to that string and the string itself becomes the
/// record, where `stderr_line` would format a second string from
/// it and copy that into the record.
pub(crate) fn stderr_owned(text: String) {
    let mut text = text;
    text.push('\n');
    with_pending(|pending| pending.borrow_mut().push_owned(DiagnosticStream::Stderr, text));
}

pub(crate) fn stdout_line(text: &str) {
    with_pending(|pending| pending.borrow_mut().push_owned(DiagnosticStream::Stdout, format!("{}\n", text)));
}

/// Take optional diagnostics from the most recent outer API call on this thread.
/// Returning APIs perform no optional diagnostic I/O and start no diagnostic
/// writer. Call this after each call, before starting the next outer invocation.
/// Nested calls retain their records in order. The queue is capped at one MiB
/// and 256 records; an explicit truncation record reports a reached cap.
/// Mandatory hook protocol output and grant receipts remain on stdout.
pub fn take_diagnostics() -> Vec<DiagnosticOutput> {
    if !PENDING_INITIALIZED.with(Cell::get) { return Vec::new(); }
    with_pending(|pending| {
        let mut pending = pending.borrow_mut();
        pending.bytes = 0;
        pending.truncated = false;
        std::mem::take(&mut pending.records)
    })
}

/// Whether a backlog takes the synchronous path: at most
/// `SYNC_MAX_BYTES` in total, every record ending with the newline the
/// line writers append themselves. Records queued by `stderr_line` and
/// `stdout_line` always end with that newline; anything else keeps the
/// writer thread, whose `write_records` path preserves its bytes
/// exactly. The byte bound is the same one a single record already
/// accepts, so a small multi-record backlog (an error line behind a
/// trace record, say) pays no thread spawn for the same blocked-write
/// exposure a single record of that size would already carry.
fn sync_eligible(records: &[DiagnosticOutput]) -> bool {
    !records.is_empty()
        && records.iter().map(|record| record.text.len()).sum::<usize>() <= SYNC_MAX_BYTES
        && records.iter().all(|record| record.text.ends_with('\n'))
}

/// Write one record synchronously on its raw standard handle. The line
/// writers append the record's terminating newline themselves, so exactly
/// that one trailing newline is stripped here and re-added by the writer:
/// every other byte, internal newlines included, passes through unchanged.
fn write_record_sync(record: &DiagnosticOutput) {
    let line = record.text.strip_suffix('\n').unwrap_or(&record.text);
    match record.stream {
        DiagnosticStream::Stdout => crate::cli_process::write_stdout_line(line),
        DiagnosticStream::Stderr => crate::cli_process::write_stderr_line(line),
    }
}

/// A standard stream for the writer thread: the handle the invocation
/// resolved at entry, written with `WriteFile` when it is not a console;
/// a console, or a stream with no handle, goes through the standard
/// library's own stream, which writes a console in its wide form.
struct EntryStream {
    handle: usize,
    console: bool,
    stdout: bool,
}

impl EntryStream {
    fn of(which: u32, stdout: bool) -> Self {
        let (handle, console) = crate::cli_process::std_line_target(which);
        Self { handle: handle as usize, console, stdout }
    }
}

impl Write for EntryStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let handle = self.handle as crate::sys::windows::Handle;
        if self.console || handle.is_null() || handle == crate::sys::windows::INVALID_HANDLE {
            return if self.stdout { std::io::stdout().write(buf) } else { std::io::stderr().write(buf) };
        }
        let mut written = 0u32;
        let length = buf.len().min(u32::MAX as usize) as u32;
        if unsafe { crate::sys::windows::WriteFile(handle, buf.as_ptr() as *const std::ffi::c_void, length, &mut written, std::ptr::null_mut()) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.console { if self.stdout { std::io::stdout().flush() } else { std::io::stderr().flush() } } else { Ok(()) }
    }
}

fn write_records(records: &[DiagnosticOutput], out: &mut impl Write, err: &mut impl Write) {
    for record in records {
        match record.stream {
            DiagnosticStream::Stdout => { let _ = out.write_all(record.text.as_bytes()); let _ = out.flush(); }
            DiagnosticStream::Stderr => { let _ = err.write_all(record.text.as_bytes()); }
        }
    }
}

/// Harden the owned CLI process and supervising thread before dispatch, then
/// run one guard CLI entry point and terminate this process with its selected
/// 32-bit status. This is deliberately separate from all returning APIs.
/// After entry cleanup, optional output is delivered one of two ways. A
/// small backlog (see `sync_eligible`) whose streams are none of them a
/// pipe is written synchronously on its raw standard handles: no writer
/// thread is spawned. A pipe that nobody reads would hold that write, and
/// the exit with it, for ever after the tree is gone, so a backlog for a
/// pipe, any larger backlog, or one whose records do not all end with a
/// newline, keeps the writer thread, which has a total 500 ms suspend-inclusive
/// grace and is never joined without a deadline. Native self-termination
/// ends a blocked writer without Rust/CRT flushing or DLL detach
/// handlers. Required stdout must have been delivered by the entry point
/// before it returns; hook output does so.
pub fn cli_exit(kind: crate::Role) -> ! {
    let _process = ProcessOwnership::begin();
    let _invocation = Invocation::begin();
    let mut cached_args_unwind_guard = None;
    let status = match crate::cli_process::enter(kind) {
        Ok(()) => {
            cached_args_unwind_guard = Some(crate::cli_process::CachedArgumentsUnwindGuard::new());
            match kind {
                crate::Role::Full => crate::main_full(),
                crate::Role::Deadline => crate::main_deadline(),
                crate::Role::Gate => crate::main_gate(),
                crate::Role::Contain => crate::main_contain(),
            }
        },
        Err((error, hook)) => {
            stderr_owned(format!("PROCESS_GUARD_ERROR: {}", error));
            if hook { crate::EXIT_HOOK_DENY } else { crate::EXIT_GUARD_ERROR }
        }
    };
    let records = take_diagnostics();
    let to_pipe = records.iter().any(|record| crate::cli_process::std_target_is_pipe(match record.stream {
        DiagnosticStream::Stdout => crate::sys::windows::STD_OUTPUT_HANDLE,
        DiagnosticStream::Stderr => crate::sys::windows::STD_ERROR_HANDLE,
    }));
    if !to_pipe && sync_eligible(&records) {
        for record in &records {
            write_record_sync(record);
        }
    } else if !records.is_empty() {
        let deadline = unsafe { GetTickCount64() }.saturating_add(GRACE_MS);
        // The records are handed to the writer only after the
        // writer has been hardened through its creation handle,
        // as every other worker of a process-owning invocation is
        // hardened: the thread waits for its one job, and a thread
        // that cannot be hardened is never given the records (the
        // job channel closes as this block ends and the thread
        // ends unwritten — the output is optional by definition,
        // and mandatory output was delivered by the entry point).
        let (job_sender, job_receiver) = std::sync::mpsc::channel::<Vec<DiagnosticOutput>>();
        // The streams are resolved here, on the invocation's thread, from
        // the handles it took at entry, and handed to the writer.
        let (mut out, mut err) = (EntryStream::of(crate::sys::windows::STD_OUTPUT_HANDLE, true), EntryStream::of(crate::sys::windows::STD_ERROR_HANDLE, false));
        if let Ok(writer) = std::thread::Builder::new().name("optional-output".into()).spawn(move || {
            if let Ok(records) = job_receiver.recv() {
                write_records(&records, &mut out, &mut err);
            }
        }) {
            if crate::cli_process::harden_worker(&writer).is_ok() && job_sender.send(records).is_ok() {
                crate::engine::wait_diagnostic_writer(deadline, raw_handle(&writer));
            }
        }
    }
    std::mem::forget(cached_args_unwind_guard);
    unsafe { TerminateProcess(GetCurrentProcess(), status as u32); }
    // Native self-termination does not return on success. An unexpected API
    // failure must still fail fast without a panic hook or blocking flush.
    std::process::abort()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_output_invocation_leaves_pending_tls_uninitialized() {
        std::thread::spawn(|| {
            assert!(!PENDING_INITIALIZED.with(Cell::get));
            let _owner = ProcessOwnership::begin();
            { let _call = Invocation::begin(); }
            assert!(take_diagnostics().is_empty());
            assert!(!PENDING_INITIALIZED.with(Cell::get));
        }).join().unwrap();
    }

    #[test]
    fn notice_still_initializes_optional_output_with_one_newline() {
        std::thread::spawn(|| {
            let _call = Invocation::begin();
            assert!(!PENDING_INITIALIZED.with(Cell::get));
            crate::engine::flush_trace(Some("notice".into()));
            assert!(PENDING_INITIALIZED.with(Cell::get));
            assert_eq!(take_diagnostics(), vec![
                DiagnosticOutput { stream: DiagnosticStream::Stderr, text: "notice\n".into() },
            ]);
        }).join().unwrap();
    }

    #[test]
    fn actual_output_initializes_pending_and_nested_calls_keep_order() {
        std::thread::spawn(|| {
            assert!(!PENDING_INITIALIZED.with(Cell::get));
            let outer = Invocation::begin();
            stderr_line("first");
            { let _nested = Invocation::begin(); stdout_line("second"); }
            drop(outer);
            assert!(PENDING_INITIALIZED.with(Cell::get));
            assert_eq!(take_diagnostics(), vec![
                DiagnosticOutput { stream: DiagnosticStream::Stderr, text: "first\n".into() },
                DiagnosticOutput { stream: DiagnosticStream::Stdout, text: "second\n".into() },
            ]);
            assert!(PENDING_INITIALIZED.with(Cell::get));
        }).join().unwrap();
    }

    #[test]
    fn failed_nested_borrow_preserves_outer_capture_depth() {
        std::thread::spawn(|| {
            let outer = Invocation::begin();
            stderr_line("outer");
            let outer_depth = CAPTURE_DEPTH.with(Cell::get);
            with_pending(|pending| {
                let _borrow = pending.borrow_mut();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(Invocation::begin));
                assert!(result.is_err());
            });
            assert_eq!(CAPTURE_DEPTH.with(Cell::get), outer_depth);
            drop(outer);
            assert_eq!(take_diagnostics(), vec![
                DiagnosticOutput { stream: DiagnosticStream::Stderr, text: "outer\n".into() },
            ]);
        }).join().unwrap();
    }

    #[test]
    fn failed_outer_clear_preserves_depth_and_next_call_replaces_output() {
        std::thread::spawn(|| {
            { let _prior = Invocation::begin(); stderr_line("prior"); }
            let prior_depth = CAPTURE_DEPTH.with(Cell::get);
            with_pending(|pending| {
                let _borrow = pending.borrow_mut();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(Invocation::begin));
                assert!(result.is_err());
            });
            assert_eq!(CAPTURE_DEPTH.with(Cell::get), prior_depth);
            { let _next = Invocation::begin(); stderr_line("next"); }
            assert_eq!(take_diagnostics(), vec![
                DiagnosticOutput { stream: DiagnosticStream::Stderr, text: "next\n".into() },
            ]);
        }).join().unwrap();
    }

    #[test]
    fn invocation_capture_does_not_grant_or_reset_process_ownership() {
        assert!(!owns_process());
        let outer = Invocation::begin();
        stderr_line("outer");
        { let _inner = Invocation::begin(); assert!(!owns_process()); stderr_line("inner"); }
        let owner = ProcessOwnership::begin();
        { let _inner = Invocation::begin(); assert!(owns_process()); }
        assert!(owns_process());
        drop(owner);
        assert!(!owns_process());
        drop(outer);
        assert_eq!(take_diagnostics().len(), 2, "nested capture preserves output without acquiring ownership");
    }

    #[test]
    fn ownership_and_capture_depth_are_released_when_a_call_unwinds() {
        assert!(!owns_process());
        let result = std::panic::catch_unwind(|| {
            let _owner = ProcessOwnership::begin();
            let _call = Invocation::begin();
            stderr_line("aborted call");
            panic!("simulate a returning caller's unwind");
        });
        assert!(result.is_err());
        assert!(!owns_process());
        { let _next = Invocation::begin(); stdout_line("next call"); }
        assert_eq!(take_diagnostics(), vec![DiagnosticOutput { stream: DiagnosticStream::Stdout, text: "next call\n".into() }]);
    }

    #[test]
    fn returning_capture_preserves_nested_record_order_and_owned_output() {
        let outer = Invocation::begin();
        stderr_line("first é😀");
        { let _nested = Invocation::begin(); stdout_line("help"); stderr_line("last"); }
        drop(outer);
        let records = take_diagnostics();
        assert_eq!(records, vec![
            DiagnosticOutput { stream: DiagnosticStream::Stderr, text: "first é😀\n".into() },
            DiagnosticOutput { stream: DiagnosticStream::Stdout, text: "help\n".into() },
            DiagnosticOutput { stream: DiagnosticStream::Stderr, text: "last\n".into() },
        ]);
        assert!(take_diagnostics().is_empty());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        write_records(&records, &mut out, &mut err);
        assert_eq!(out, b"help\n");
        assert_eq!(err, "first é😀\nlast\n".as_bytes());
    }

    #[test]
    fn next_outer_invocation_replaces_previous_optional_output() {
        { let _call = Invocation::begin(); stderr_line("previous"); }
        { let _call = Invocation::begin(); stdout_line("current"); }
        assert_eq!(take_diagnostics(), vec![DiagnosticOutput { stream: DiagnosticStream::Stdout, text: "current\n".into() }]);
    }

    #[test]
    fn optional_queue_caps_are_explicit_and_preserve_utf8() {
        let mut pending = Pending::default();
        pending.push(DiagnosticStream::Stderr, &"界".repeat(MAX_BYTES / 3 + 1));
        pending.push(DiagnosticStream::Stdout, "no second marker");
        assert!(pending.truncated && pending.bytes <= MAX_BYTES);
        assert_eq!(pending.records.last().unwrap().text, TRUNCATED);
        assert!(pending.records.iter().all(|record| std::str::from_utf8(record.text.as_bytes()).is_ok()));
        let mut pending = Pending::default();
        for _ in 0..MAX_RECORDS + 2 { pending.push(DiagnosticStream::Stderr, "a"); }
        assert_eq!(pending.records.len(), MAX_RECORDS);
        assert_eq!(pending.records.last().unwrap().text, TRUNCATED);
    }

    #[test]
    fn optional_stream_error_does_not_drop_later_records() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> { Err(std::io::ErrorKind::BrokenPipe.into()) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let records = vec![
            DiagnosticOutput { stream: DiagnosticStream::Stderr, text: "broken\n".into() },
            DiagnosticOutput { stream: DiagnosticStream::Stdout, text: "delivered\n".into() },
        ];
        let mut out = Vec::new();
        write_records(&records, &mut out, &mut Broken);
        assert_eq!(out, b"delivered\n");
    }
}
