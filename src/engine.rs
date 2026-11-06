// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The engine: one command in a job object, a deadline, and the cleanup of
//! every process in the job.

use crate::auth;
use crate::extend::{
    enclosed, millis, next_token, not_started_text, open_log, register_start, release, Extension, Log, Settings,
    Token, BACKSTOP_WAKE_MS, CHANNEL_VARIABLE, DEFAULT_MAX_EXTENSION, LOG_FLUSH_MS,
};
#[cfg(test)]
use crate::extend::BACKSTOP_MS;
use crate::sys::windows::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CloseHandle, CreateJobObjectW,
    CreateProcessW, DUPLICATE_SAME_ACCESS, DuplicateHandle, EXTENDED_LIMIT_CLASS,
    EXTENDED_STARTUPINFO_PRESENT, ExtendedLimitInformation, GetCurrentProcess, GetExitCodeProcess,
    GetStdHandle, GetTickCount64, Handle, INVALID_HANDLE, KILL_ON_JOB_CLOSE, ProcessInformation,
    ResumeThread, STARTF_USESTDHANDLES, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    SetHandleInformation, SetInformationJobObject, StartupInfoExW, TerminateProcess, WAIT_TIMEOUT,
    WaitForSingleObject, checked, os_error,
};
use crate::timing::{DecisionEvidence, FixedTiming, TickEvidence};
use crate::{GuardError, SmallList, EXIT_GUARD_ERROR, EXIT_TIMEOUT};
use std::ffi::c_void;

mod attributes;
mod environment;
mod owners;
mod resolve;
mod trace;
mod wait;

pub(crate) use owners::*;
pub(crate) use resolve::*;
pub(crate) use trace::*;
pub(crate) use wait::*;
pub use trace::trace;
use attributes::{AttributeBlock, JobAttributeList};
use environment::environment_block;

type HandleList = SmallList<Handle, 16>;

/// The one teardown budget of a run, in milliseconds: what
/// terminating the job did not spend of it is all the wait on the
/// child that follows may spend.
const TEARDOWN_BUDGET_MS: u32 = 5000;


/// Seconds as text, in the form `format_seconds_into` writes. The
/// string form is retained beside it for callers that hold no
/// buffer of their own (a formatted error, a prompt field); a
/// caller assembling a line it already holds writes in place.
/// Every caller passes a finite number that was either
/// validated against the deadline bounds or derived from the
/// run's own tick counts: this form carries no guard for a
/// non-finite or client-supplied number, because the audit
/// log's own seconds form — the one that records numbers a
/// client sent, unvalidated — carries those guards instead.
/// The two forms are not interchangeable.
pub(crate) fn format_seconds(seconds: f64) -> String {
    let mut out = String::new();
    format_seconds_into(seconds, &mut out);
    out
}

/// `format_seconds` written into a buffer the caller already
/// holds, in the same text: a line that formats its seconds as
/// its own string first allocates for text the line then copies
/// out of it (the grant receipt of `extend` is such a line, and
/// its assembly elsewhere already writes every other field in
/// place).
pub(crate) fn format_seconds_into(seconds: f64, out: &mut String) {
    use std::fmt::Write as _;
    if seconds.fract() == 0.0 {
        let _ = write!(out, "{}", seconds as u64);
    } else {
        let _ = write!(out, "{}", seconds);
    }
}

/// What a unit test of the engine looks at in a run just before its command starts: the
/// extension (the cap it was made with) and how many owners are watched. Only under `cfg(test)`.
#[cfg(test)]
type BeforeStart = Box<dyn Fn(Option<&Extension>, usize)>;

#[cfg(test)]
thread_local! {
    static BEFORE_START: std::cell::RefCell<Option<BeforeStart>> = const { std::cell::RefCell::new(None) };
    // Observation only: tests record facts, let normal supervision/cleanup
    // finish, and assert afterward. No production hook or failure injection.
    static AFTER_CREATE: std::cell::RefCell<Option<Box<dyn Fn(Handle, Handle)>>> = const { std::cell::RefCell::new(None) };
    // Observation only: records whether the actual ResumeThread branch is reached.
    static BEFORE_RESUME: std::cell::RefCell<Option<Box<dyn Fn()>>> = const { std::cell::RefCell::new(None) };
    // Records only the existing wait on the exact original child HANDLE.
    static AFTER_CHILD_CLEANUP: std::cell::RefCell<Option<Box<dyn Fn(Handle, Option<u32>)>>> = const { std::cell::RefCell::new(None) };
}

/// How a run differs from a plain deadline.
#[derive(Default)]
pub struct RunMode {
    /// A top-level extendable run (`--extendable`): its channel, cap and audit file.
    pub extendable: Option<Settings>,
    /// The contain role: refuse to run unless an extendable run encloses the program.
    pub require_channel: bool,
    /// A call of the library: the process that made it goes on when it returns, so the
    /// hold that its start made on the enclosing run ends with the call.
    pub library: bool,
}

/// The run for a call of the library: inside an extendable run, the hold that its start
/// makes ends when the call returns.
pub fn run_library_job(argv: &[String], seconds: f64, raw_tail: Option<&str>, watch_ancestors: bool) -> Result<u32, GuardError> {
    run_job_plain_parent(argv, seconds, raw_tail, watch_ancestors, &RunMode { library: true, ..RunMode::default() }, None)
}

/// The run for the contain role: the run is a child of an extendable run and
/// its deadline is a request to that run. The role takes no extension options,
/// so its run cannot carry an exception.
pub fn run_contained_job(argv: &[String], seconds: f64, raw_tail: Option<&str>) -> Result<u32, GuardError> {
    run_job_plain_parent(argv, seconds, raw_tail, false, &RunMode { require_channel: true, ..RunMode::default() }, None)
}

/// Ends the hold that the start of a library call made on the enclosing run, when the call
/// returns. The hold is the call's own (its token), so a call that overlaps it, on another
/// thread of the same process, keeps its hold.
struct EndHold(Option<Token>);

impl Drop for EndHold {
    fn drop(&mut self) {
        if let Some(token) = self.0 {
            let _ = release(token);
        }
    }
}

/// Run `argv` in a new job object with a deadline of `seconds`. With
/// `watch_ancestors` the guard also ends the tree when any live ancestor exits,
/// not only its parent (automatic mode). Returns the child's exit code, 124 on
/// the deadline, or 125 when an owner exited first. The run carries a mode:
/// extendable at the top, or inside an extendable run.
pub fn run_job_with(argv: &[String], seconds: f64, raw_tail: Option<&str>, watch_ancestors: bool, mode: &RunMode) -> Result<u32, GuardError> {
    run_job_with_parent(argv, seconds, raw_tail, watch_ancestors, mode, None)
}

/// Only the one-shot AUTO CLI passes these call-local retained parent facts.
/// Returning public APIs neither populate nor consume a process-global cache.
pub(crate) fn run_auto_job(argv: &[String], seconds: f64, raw_tail: Option<&str>, library: bool, parent: Option<&ParentFacts>) -> Result<u32, GuardError> {
    run_job_plain_parent(argv, seconds, raw_tail, true, &RunMode { library, ..RunMode::default() }, parent)
}

/// The decision on an exception that is started inside an extendable run: a
/// guard with an exception that is started inside an extendable run is refused
/// before it asks that run for anything, so that a run that is refused places
/// no hold on the run around it. Ok(()) means the run goes on.
fn decide_enclosed_exception(mode: &RunMode, executable: &str) -> Result<(), GuardError> {
    if let Some(settings) = mode.extendable.as_ref().filter(|settings| settings.exception.is_some()) {
        if enclosed() {
            let opened = open_log(settings)?;
            let error = match auth::decide(settings, true, executable, "", &[], &auth::Owners::empty(), &opened) {
                Err(error) => error,
                Ok(()) => GuardError("an exception above the cap cannot be granted inside another run".into()),
            };
            opened.finish(LOG_FLUSH_MS);
            return Err(error);
        }
    }
    Ok(())
}

/// The decision on the exception of a run. A cap over the default needs the
/// approval of the signed-in user. It is asked for now, once the command is
/// known to be runnable and before the job exists or the command starts, and
/// the log it is recorded in is passed on to the extension. A run inside
/// another run is refused. The process that started the guard is opened first
/// and watched while a prompt is open, so that a prompt is never answered for
/// a run that nobody started. The handle of that process that the prompt
/// watched is kept, and is the one the run is watched against: a process that
/// took its identifier in between is never taken for it. On approval the log
/// and the kept handles pass to the run; without an exception both are empty.
fn decide_run_exception(mode: &RunMode, held: bool, executable: &str) -> Result<(Option<Log>, Vec<Handle>), GuardError> {
    let Some(settings) = mode.extendable.as_ref().filter(|settings| settings.exception.is_some()) else {
        return Ok((None, Vec::new()));
    };
    let opened = open_log(settings)?;
    // A run inside another run is refused before anything else is looked at, so it needs no owner.
    let owners = if held {
        auth::Owners::empty()
    } else {
        match parent_process_id().and_then(|id| auth::Owners::open(&[id])) {
            Ok(owners) => owners,
            Err(error) => {
                let error = auth::refuse_unwatched(settings, &opened, error.message());
                opened.finish(LOG_FLUSH_MS);
                return Err(error);
            }
        }
    };
    let parent = owners.image();
    // Every live process above the guard, for the refusal of a run that another guard started.
    let ancestors = if held { Vec::new() } else { ancestor_images() };
    match auth::decide(settings, held, executable, &parent, &ancestors, &owners, &opened) {
        Ok(()) => Ok((Some(opened), owners.into_handles())),
        Err(error) => {
            opened.finish(LOG_FLUSH_MS);
            Err(error)
        }
    }
}

/// The enclosed decision of a run that cannot carry an exception: its mode is
/// built without extension settings at every call site, so there is no
/// exception to refuse and nothing is asked.
fn decide_no_enclosed_exception(_mode: &RunMode, _executable: &str) -> Result<(), GuardError> {
    Ok(())
}

/// The exception decision of a run that cannot carry an exception: no log is
/// opened, no owner is held for a prompt and nothing is asked.
fn decide_no_exception(_mode: &RunMode, _held: bool, _executable: &str) -> Result<(Option<Log>, Vec<Handle>), GuardError> {
    Ok((None, Vec::new()))
}

/// The start of the extension of a run that can serve one: with extension
/// settings the channel is made and served for as long as the run lasts, and
/// the log the run was given passes to it; without settings nothing is
/// started and the log stays with the run.
fn start_extension(job: Handle, mode: &RunMode, seconds: f64, log: &mut Option<Log>) -> Result<Option<Extension>, GuardError> {
    match &mode.extendable {
        Some(settings) => Ok(Some(Extension::start(job, settings, seconds, log.take())?)),
        None => Ok(None),
    }
}

/// The extension start of a run that cannot serve one: its mode is built
/// without extension settings at every call site, so there is nothing to
/// start, no log to take and no server code to reach. The run's extension
/// slot is the unit value: no extension exists in it, even as an empty one.
fn start_no_extension(_job: Handle, _mode: &RunMode, _seconds: f64, _log: &mut Option<Log>) -> Result<(), GuardError> {
    Ok(())
}

/// The exception decisions and the extension handling a run is made with,
/// supplied by its entry point as a type: the run is compiled once per set,
/// so a run on the plain path is compiled with the set that asks nothing and
/// serves nothing — its extension slot is the unit value, no extension
/// exists in the run even as an empty one — and the approval machinery and
/// the extension server are not in its image at all.
trait Decisions {
    /// What the run holds for its extension: the extension it started, or
    /// nothing at all on a path that can serve none.
    type Ext;

    /// The decision on an exception that is started inside an extendable run.
    fn enclosed(mode: &RunMode, executable: &str) -> Result<(), GuardError>;
    /// The decision on the exception of a run: on approval, its log and the
    /// handles it kept.
    fn exception(mode: &RunMode, held: bool, executable: &str) -> Result<(Option<Log>, Vec<Handle>), GuardError>;
    /// The extension slot of a run before its start: nothing started.
    fn none() -> Self::Ext;
    /// The start of the extension of a run: on a run that serves one, the
    /// extension it started.
    fn starter(job: Handle, mode: &RunMode, seconds: f64, log: &mut Option<Log>) -> Result<Self::Ext, GuardError>;
    /// Whether the run serves an extension: its deadline then moves with
    /// the holds, and no fixed deadline is assigned before the start.
    fn serves(ext: &Self::Ext) -> bool;
    /// Whether a served run's moving deadline has already expired.
    fn extension_expired(ext: &Self::Ext) -> bool;
    /// The name of the extension's channel, for the command's environment.
    fn channel(ext: &Self::Ext) -> Option<&str>;
    /// The extension of the run, for the unit tests of the engine only.
    #[cfg(test)]
    fn extension_ref(ext: &Self::Ext) -> Option<&Extension>;
    /// Wait for the child or an owner: on a run that serves an extension,
    /// under its moving deadline; on any other run, the fixed wait the
    /// caller supplies.
    fn wait(ext: &Self::Ext, waits: &[Handle], fixed: impl FnOnce(&[Handle]) -> u32) -> u32;
    /// The seconds the run is reported to have run: the extension's own
    /// count on a run that serves one, else the run's deadline.
    fn elapsed(ext: &Self::Ext, seconds: f64) -> f64;
    /// Say in the extension's log that an approved run did not start. Only
    /// a run whose exception was approved has anything to say.
    fn note_not_started(ext: &Self::Ext, approved: bool, error: &GuardError);
    /// Stop granting on the extension of the run, once its command has
    /// finished or its deadline has passed.
    fn close(ext: &Self::Ext);
    /// End the extension of the run, after its tree is gone.
    fn shutdown(ext: Self::Ext);
}

/// The decisions of a run that can carry an exception: they ask for its
/// approval, and its extension start serves its channel.
struct AskingDecisions;

impl Decisions for AskingDecisions {
    type Ext = Option<Extension>;

    fn enclosed(mode: &RunMode, executable: &str) -> Result<(), GuardError> { decide_enclosed_exception(mode, executable) }
    fn exception(mode: &RunMode, held: bool, executable: &str) -> Result<(Option<Log>, Vec<Handle>), GuardError> { decide_run_exception(mode, held, executable) }
    fn none() -> Self::Ext { None }
    fn starter(job: Handle, mode: &RunMode, seconds: f64, log: &mut Option<Log>) -> Result<Self::Ext, GuardError> { start_extension(job, mode, seconds, log) }
    fn serves(ext: &Self::Ext) -> bool { ext.is_some() }
    fn extension_expired(ext: &Self::Ext) -> bool {
        ext.as_ref().is_some_and(|extension| extension.remaining().is_none())
    }
    fn channel(ext: &Self::Ext) -> Option<&str> { ext.as_ref().map(|extension| extension.channel.as_str()) }
    #[cfg(test)]
    fn extension_ref(ext: &Self::Ext) -> Option<&Extension> { ext.as_ref() }
    fn wait(ext: &Self::Ext, waits: &[Handle], fixed: impl FnOnce(&[Handle]) -> u32) -> u32 {
        match ext {
            None => fixed(waits),
            Some(extension) => wait_extendable(extension, waits),
        }
    }
    fn elapsed(ext: &Self::Ext, seconds: f64) -> f64 { ext.as_ref().map_or(seconds, Extension::elapsed_seconds) }
    fn note_not_started(ext: &Self::Ext, approved: bool, error: &GuardError) {
        if approved {
            if let Some(extension) = ext {
                extension.note(&not_started_text(error));
            }
        }
    }
    fn close(ext: &Self::Ext) {
        if let Some(extension) = ext {
            extension.close();
        }
    }
    fn shutdown(ext: Self::Ext) {
        if let Some(mut extension) = ext {
            extension.shutdown();
        }
    }
}

/// The decisions of a run that cannot carry an exception: they ask nothing,
/// and the run serves nothing: its extension slot is the unit value and
/// every extension operation on it is the fixed, unserved behaviour.
struct UnaskingDecisions;

impl Decisions for UnaskingDecisions {
    type Ext = ();

    fn enclosed(mode: &RunMode, executable: &str) -> Result<(), GuardError> { decide_no_enclosed_exception(mode, executable) }
    fn exception(mode: &RunMode, held: bool, executable: &str) -> Result<(Option<Log>, Vec<Handle>), GuardError> { decide_no_exception(mode, held, executable) }
    fn none() -> Self::Ext {}
    fn starter(job: Handle, mode: &RunMode, seconds: f64, log: &mut Option<Log>) -> Result<Self::Ext, GuardError> { start_no_extension(job, mode, seconds, log) }
    fn serves(_ext: &Self::Ext) -> bool { false }
    fn extension_expired(_ext: &Self::Ext) -> bool { false }
    fn channel(_ext: &Self::Ext) -> Option<&str> { None }
    #[cfg(test)]
    fn extension_ref(_ext: &Self::Ext) -> Option<&Extension> { None }
    fn wait(_ext: &Self::Ext, waits: &[Handle], fixed: impl FnOnce(&[Handle]) -> u32) -> u32 { fixed(waits) }
    fn elapsed(_ext: &Self::Ext, seconds: f64) -> f64 { seconds }
    fn note_not_started(_ext: &Self::Ext, _approved: bool, _error: &GuardError) {}
    fn close(_ext: &Self::Ext) {}
    fn shutdown(_ext: Self::Ext) {}
}

/// A run with a mode that can carry an exception: the decisions above ask for
/// its approval.
fn run_job_with_parent(argv: &[String], seconds: f64, raw_tail: Option<&str>, watch_ancestors: bool, mode: &RunMode, parent: Option<&ParentFacts>) -> Result<u32, GuardError> {
    run_job_core::<AskingDecisions>(argv, seconds, raw_tail, watch_ancestors, mode, parent)
}

/// A run whose mode cannot carry an exception (a plain, library, automatic or
/// contain run): the decisions ask nothing and hold nothing, and the extension
/// start starts nothing, so the run never reaches the approval machinery or
/// the extension server at all.
fn run_job_plain_parent(argv: &[String], seconds: f64, raw_tail: Option<&str>, watch_ancestors: bool, mode: &RunMode, parent: Option<&ParentFacts>) -> Result<u32, GuardError> {
    run_job_core::<UnaskingDecisions>(argv, seconds, raw_tail, watch_ancestors, mode, parent)
}

fn run_job_core<D: Decisions>(
    argv: &[String], seconds: f64, raw_tail: Option<&str>, watch_ancestors: bool, mode: &RunMode, parent: Option<&ParentFacts>,
) -> Result<u32, GuardError> {
    let _trace_sample = TraceSample::begin();
    require_process_ownership(mode.extendable.is_some())?;
    // CreateProcessW would silently stop reading either native string at a
    // NUL. Returning callers can supply strings the OS argv could never hold.
    if argv.iter().any(|arg| arg.as_bytes().contains(&0))
        || raw_tail.is_some_and(|tail| tail.as_bytes().contains(&0))
    {
        return Err(GuardError("command arguments and shell text must contain no NUL".into()));
    }
    trace("run");
    let _flush = FlushTrace;
    let process_cli = crate::cli_process::active() && !mode.library;
    let executable = if process_cli && std::path::Path::new(&argv[0]).is_absolute()
        && [".exe", ".com"].iter().any(|suffix| argv[0].as_bytes().get(argv[0].len().saturating_sub(suffix.len())..)
            .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix.as_bytes()))) {
        std::borrow::Cow::Borrowed(argv[0].as_str())
    } else {
        std::borrow::Cow::Owned(which(&argv[0]).ok_or_else(|| GuardError(format!("executable not found: {}", argv[0])))?)
    };
    let bytes = executable.as_bytes();
    if [b".cmd", b".bat"].iter().any(|suffix| bytes.get(bytes.len().saturating_sub(suffix.len())..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case(*suffix))) {
        return Err(GuardError("For .cmd/.bat, explicitly invoke cmd.exe /d /s /c under this guard".into()));
    }
    trace("resolved executable");
    D::enclosed(mode, &executable)?;
    // Started inside an extendable run, the guard asks it for this deadline first; a
    // refusal (over the cap, a rate limit, no verified place in the job) ends it here.
    let token = if mode.library { next_token() } else { 0 };
    let held = register_start(seconds, mode.require_channel, token)?;
    // A guard program ends with its command, and its hold with it. A library call returns to a
    // process that goes on, so the hold is ended when the call returns, whatever the outcome.
    let _end = EndHold((held && mode.library).then_some(token));

    // Every cap over the default is an exception that was built with the means to ask for it. A
    // caller that builds settings with a large cap and no exception gets no run, not a run with
    // a cap that nobody approved.
    if let Some(settings) = mode.extendable.as_ref() {
        if settings.max_seconds > DEFAULT_MAX_EXTENSION && settings.exception.is_none() {
            return Err(GuardError("a cap over the default needs the approval of the signed-in user; the run did not start".into()));
        }
    }
    let (mut log, kept) = D::exception(mode, held, &executable)?;

    // The kept handles are closed with the others, at the end of the run.
    let mut handles = HandleList::from_slice(&kept);
    let mut info = ProcessInformation { process: std::ptr::null_mut(), thread: std::ptr::null_mut(), process_id: 0, thread_id: 0 };
    let mut assigned = false;
    let mut child_exited = false;
    // Set once the command is running, so that a run whose exception was approved and which then fails to start says so in the log.
    let mut started_command = false;
    let mut extension = D::none();
    // The notice for the end of the run, written once the tree is gone.
    let mut notice: Option<String> = None;
    let job = unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) };
    if job.is_null() {
        let error = os_error();
        if let Some(log) = log.take() {
            log.note(&not_started_text(&error));
            log.finish(LOG_FLUSH_MS);
        }
        for &handle in handles.iter() {
            unsafe { CloseHandle(handle) };
        }
        return Err(error);
    }
    handles.push(job);
    let milliseconds = millis(seconds);
    let mut timing = FixedTiming::from_environment(mode.extendable.is_some(), milliseconds);
    let mut deadline = None;
    let mut first_owner = None;
    let mut console_owners = (process_cli && watch_ancestors).then(SmallList::new);
    // The run's one Windows folder string, filled by the owner walk
    // when it walks the ancestors and handed to the console finish
    // below, so the folder is queried and converted once per run.
    // The parent-only route computes none, and its slot stays empty.
    let mut console_windows: Option<String> = None;
    // The console is probed inside the outcome, immediately before the
    // creation attributes are prepared: the only readers of the probe are
    // the CREATE_NO_WINDOW decision there and `finish` after the run, so a
    // run that fails before that point — before the job is set up, at the
    // extension start or at the owner open — never pays its CreateFileW
    // probes. The probed console is stored here so `finish` can read it.
    let mut console: Option<crate::cli_process::Console> = None;

    let outcome = (|| -> Result<u32, GuardError> {
        unsafe {
            let mut limits = ExtendedLimitInformation::default();
            limits.basic.limit_flags = KILL_ON_JOB_CLOSE; // No breakaway allowed.
            checked(SetInformationJobObject(job, EXTENDED_LIMIT_CLASS, &mut limits as *mut _ as *mut c_void,
                std::mem::size_of::<ExtendedLimitInformation>() as u32))?;
            extension = D::starter(job, mode, seconds, &mut log)?;

            let mut startup: StartupInfoExW = std::mem::zeroed();
            startup.startup.cb = std::mem::size_of::<StartupInfoExW>() as u32;
            // Keep the child's own initial window state, including GUI children.
            startup.startup.flags = STARTF_USESTDHANDLES;
            let mut inherited = [std::ptr::null_mut(); 3];
            let mut inherited_count = 0;
            // The current-process pseudo-handle serves only the
            // returning-library branch below, which duplicates the
            // standard handles with it: it is fetched on that branch's
            // first duplication and kept for the rest. A process-owning
            // CLI invocation passes its original handles and never
            // reads the value, so fetching it before the loop charged
            // every such run one call for a handle it never used.
            let mut current: Option<Handle> = None;
            for (which_handle, slot) in [(STD_INPUT_HANDLE, 0usize), (STD_OUTPUT_HANDLE, 1), (STD_ERROR_HANDLE, 2)] {
                // On a process-owning CLI invocation the entry probe
                // already fetched each standard handle once; read those
                // back instead of fetching the same three again here.
                let original = crate::cli_process::cached_std_handle(which_handle)
                    .unwrap_or_else(|| GetStdHandle(which_handle));
                if !original.is_null() && original != INVALID_HANDLE {
                    let duplicate = if process_cli {
                        if !inherited[..inherited_count].contains(&original) {
                            checked(SetHandleInformation(original, 1, 1))?;
                        }
                        original
                    } else {
                        let current = *current.get_or_insert_with(|| GetCurrentProcess());
                        let mut duplicate = std::ptr::null_mut();
                        checked(DuplicateHandle(current, original, current, &mut duplicate, 0, 1, DUPLICATE_SAME_ACCESS))?;
                        handles.push(duplicate);
                        duplicate
                    };
                    inherited[inherited_count] = duplicate;
                    inherited_count += 1;
                    match slot {
                        0 => startup.startup.std_input = duplicate,
                        1 => startup.startup.std_output = duplicate,
                        _ => startup.startup.std_error = duplicate,
                    }
                }
            }
            // Owners: the parent, or in automatic mode every live ancestor, so a
            // host that kills only the outer launcher of a shell still ends the tree.
            // A run with an exception already holds its parent (nearest first), opened for the prompt.
            let owners = watched_owner_handles(watch_ancestors, &kept, &mut handles, parent, &mut console_owners, &mut console_windows)?;
            first_owner = owners.first().copied();
            trace("job ready, owners opened");
            // For the unit tests of the engine only: a look at the run just before its command starts.
            #[cfg(test)]
            BEFORE_START.with(|hook| {
                if let Some(hook) = hook.borrow().as_ref() {
                    hook(D::extension_ref(&extension), owners.len());
                }
            });

            // The child cannot run or spawn until it belongs to the job.
            let application = application_wide(&executable);
            // cmd.exe does not read backslash-escaped quotes, so its payload is
            // appended verbatim inside one pair of quotes that /s strips.
            let mut command_line = command_line_wide_from_application(
                &application, argv[1..].iter().map(String::as_str), raw_tail);
            // The child of an extendable run is told where its channel is.
            let mut environment = D::channel(&extension).map(|channel| environment_block(CHANNEL_VARIABLE, channel));
            let (environment_pointer, environment_flag) = match environment.as_mut() {
                Some(block) => (block.as_mut_ptr() as *mut c_void, CREATE_UNICODE_ENVIRONMENT),
                None => (std::ptr::null_mut(), 0),
            };
            // Attachment, including a pseudoconsole, is independent of having
            // a console window. Probing opens only non-inheritable devices.
            let probed = crate::cli_process::Console::probe(process_cli);
            let console_flags = if probed.attached { 0 } else { CREATE_NO_WINDOW };
            console = Some(probed);
            // Attribute preparation precedes the fixed clock anchor. Failure,
            // including unsupported JOB_LIST, never falls back to a launch gap.
            let mut job_list = [job];
            let restrict = process_cli.then_some(&mut inherited[..inherited_count]);
            let mut attribute_stack = [AttributeBlock([0; 16]); 8];
            let mut attributes = JobAttributeList::new(&mut attribute_stack, &mut job_list, restrict, !process_cli)?;
            startup.attribute_list = attributes.as_mut_ptr();
            // Native creation (including Job assignment) and resume consume one budget.
            if !D::serves(&extension) {
                deadline = Some(match timing.as_mut() {
                    Some(timing) => timing.assign(),
                    None => fixed_deadline(milliseconds, || GetTickCount64()),
                });
            }
            checked(CreateProcessW(application.as_ptr(), command_line.as_mut_ptr(), std::ptr::null_mut(),
                std::ptr::null_mut(), i32::from(!process_cli || inherited_count > 0),
                CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | console_flags | environment_flag, environment_pointer,
                std::ptr::null(), &mut startup.startup, &mut info))?;
            // A successful CreateProcessW already placed the child in this Job.
            // Set ownership before allocation, observation, or resume can fail.
            assigned = true;
            drop(attributes); // CreateProcessW has consumed the attributes.
            handles.push(info.process);
            handles.push(info.thread);
            #[cfg(test)]
            AFTER_CREATE.with(|hook| {
                if let Some(observe) = hook.borrow().as_ref() { observe(job, info.process); }
            });
            let mut waits = HandleList::from_slice(&[info.process]);
            waits.extend_from_slice(&owners);
            // Creation can outlast either assigned budget. Check the actual
            // owner/child objects and the applicable deadline while the child
            // is still suspended. Assigned-Job cleanup handles every
            // no-resume result on the original child and thread handles.
            let before_resume = if !D::serves(&extension) {
                let assigned_deadline = deadline.expect("fixed deadline assigned before launch");
                match timing.as_mut() {
                    Some(timing) => {
                        let (ticks, decision) = (&mut timing.ticks, &mut timing.decision);
                        fixed_pre_resume_with(assigned_deadline,
                            || ticks.remaining_read(),
                            || poll_before_resume(&waits, Some(&mut *decision)))?
                    }
                    None => fixed_pre_resume_with(assigned_deadline,
                        || GetTickCount64(),
                        || poll_before_resume(&waits, None))?,
                }
            } else {
                pre_resume_with(|| D::extension_expired(&extension), || poll_before_resume(&waits, None))?
            };
            let status = if let Some(status) = before_resume {
                status
            } else {
                #[cfg(test)]
                BEFORE_RESUME.with(|hook| {
                    if let Some(observe) = hook.borrow().as_ref() { observe(); }
                });
                if ResumeThread(info.thread) == u32::MAX {
                    return Err(os_error());
                }
                started_command = true;
                trace("child started");
                D::wait(&extension, &waits, |waits| match timing.as_mut() {
                    Some(timing) => wait_fixed(deadline.expect("fixed deadline assigned before launch"), Some(&mut timing.ticks), Some(&mut timing.decision), waits),
                    None => wait_fixed(deadline.expect("fixed deadline assigned before launch"), None, None, waits),
                })
            };

            match status {
                WAIT_TIMEOUT => {
                    // An extendable run reports the time it ran, since its deadline moved.
                    let shown = D::elapsed(&extension, seconds);
                    notice = Some(format!("PROCESS_TIMEOUT: exceeded {} seconds; terminating process tree", format_seconds(shown)));
                    Ok(EXIT_TIMEOUT as u32)
                }
                n if n >= 1 && (n as usize) <= OWNERS_PER_WINDOW && (n as usize) < waits.len() => {
                    notice = Some("PROCESS_OWNER_EXITED: terminating process tree".into());
                    Ok(EXIT_GUARD_ERROR as u32)
                }
                0 => {
                    let mut code = 0u32;
                    checked(GetExitCodeProcess(info.process, &mut code))?;
                    child_exited = true;
                    Ok(code)
                }
                _ => Err(os_error()),
            }
        }
    })();
    if let Some(timing) = timing.as_mut() { timing.finish(&outcome); }
    trace("child finished");

    // A log that was opened for an exception and never passed on (the job could not be set up): it says that the approved run
    // did not start, as the log of the extension does when the failure comes after it was made.
    if let Some(log) = log.take() {
        if let Err(error) = &outcome {
            log.note(&not_started_text(error));
        }
        log.finish(LOG_FLUSH_MS);
    }
    if let Err(error) = &outcome {
        if !started_command {
            let approved = mode.extendable.as_ref().is_some_and(|settings| settings.exception.is_some());
            D::note_not_started(&extension, approved, error);
        }
    }
    // No more grants once the command has finished or the deadline has passed.
    D::close(&extension);
    let mut job_proven_empty = false;
    // One teardown budget: what terminating the job did not spend of it is
    // all the wait on the child that follows may spend.
    let mut teardown_remainder = TEARDOWN_BUDGET_MS;
    unsafe {
        if assigned {
            let (empty, remainder) = terminate_and_wait(job, EXIT_TIMEOUT as u32, TEARDOWN_BUDGET_MS);
            job_proven_empty = empty;
            teardown_remainder = remainder;
            trace("job empty");
        } else if !info.process.is_null() {
            TerminateProcess(info.process, EXIT_GUARD_ERROR as u32);
        }
        let _original_child_wait = observe_child_cleanup(info.process, child_exited, teardown_remainder, |child, milliseconds| {
            WaitForSingleObject(child, milliseconds)
        });
        #[cfg(test)]
        AFTER_CHILD_CLEANUP.with(|hook| {
            if let Some(observe) = hook.borrow().as_ref() { observe(info.process, _original_child_wait); }
        });
    }
    // The channel is closed after the tree is gone, so a stalled client never delays the kill.
    D::shutdown(extension);
    if process_cli && !job_proven_empty { unsafe { CloseHandle(job) }; }
    if process_cli && !info.process.is_null() {
        // A created process is only ever reached past the probe inside the
        // outcome, so the console is present on every run that gets here.
        if let Some(console) = &console { console.finish(first_owner, console_owners.as_deref(), console_windows.as_deref()); }
    }
    // Optional output is queued only after supervision and Job cleanup finish.
    flush_trace(notice.take());
    unsafe {
        // The real CLI reclaims now-unused handles at its immediate native exit.
        if !process_cli {
            for &handle in handles.iter().rev() { CloseHandle(handle); }
        }
    }
    if let Some(timing) = timing { timing.emit(); }
    outcome
}

#[cfg(test)]
mod tests;
