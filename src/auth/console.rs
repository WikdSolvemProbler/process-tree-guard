// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The console approval prompt: the password asked on a console that is really there, its attempts and its bounded worker.

use super::*;


use crate::sys::windows::{

    Bool, CREDUI_PASSWORD_CHARS, CREDUI_USER_CHARS, CloseHandle, ConsoleApi, CreateFileW,

    ERROR_CANCELLED, FILE_SHARE_READ_AND_WRITE, GENERIC_WRITE, GetConsoleMode,

    GetCurrentProcessId, GetNumberOfConsoleInputEvents, GetStdHandle, Handle, INVALID_HANDLE,

    OPEN_EXISTING, STD_INPUT_HANDLE, SetConsoleCtrlHandlerFn, SetConsoleMode, WriteConsoleW,

    console_api, error_for, wide,

};
/// A wait of `normal` milliseconds: between attempts, or after a refusal (`cooling`).
#[cfg(not(test))]
fn wait_ms(normal: u64, _cooling: bool) -> u64 {
    normal
}

/// In the unit tests the wait is what a test sets, none by default, so that the attempt loop is not slow.
#[cfg(test)]
fn wait_ms(_normal: u64, cooling: bool) -> u64 {
    let counter = if cooling { &COOL_DOWNS } else { &RETRIES };
    counter.with(|count| count.set(count.get() + 1));
    RETRY_DELAY.with(|delay| delay.get())
}

pub(super) fn retry_delay_ms() -> u64 {
    wait_ms(RETRY_DELAY_MS, false)
}

pub(super) fn cool_down_ms() -> u64 {
    wait_ms(COOL_DOWN_MS, true)
}

#[cfg(test)]
thread_local! {
    pub(super) static RETRY_DELAY: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// How many times the wait between attempts was asked for on this thread.
    pub(super) static RETRIES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// How many times the wait after a refusal was asked for on this thread.
    pub(super) static COOL_DOWNS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// True when standard input is a console input buffer: never a pipe, a file or the null
/// device, which an agent controls.
pub(crate) fn console_stdin() -> bool {
    stdin_handle().is_some_and(is_console_input)
}

/// True when `handle` is a console input buffer. `GetConsoleMode` alone is not enough: it also
/// succeeds on the screen buffer of a console, which is no input, so `GetNumberOfConsoleInputEvents`
/// must succeed too.
pub(super) fn is_console_input(handle: Handle) -> bool {
    let (mut mode, mut events) = (0u32, 0u32);
    unsafe { GetConsoleMode(handle, &mut mode) != 0 && GetNumberOfConsoleInputEvents(handle, &mut events) != 0 }
}

// ------------------------------------------------------------ the console prompt

/// The console prompt: `CredUICmdLinePromptForCredentialsW`, accepted only when standard input
/// is a real console. The prompt reads the console itself, so redirected standard input would
/// not feed it, and an agent that controls standard input must never be able to answer. What
/// the user is approving is written to the console first, once. A console that a process of the
/// same user made (a pseudo console) also passes the test; see `SECURITY.md`.
pub(crate) struct ConsolePrompt<'a> {
    owners: &'a Owners,
    /// The thread that makes every call of the prompt of this run, made when the first is asked.
    worker: std::cell::RefCell<Option<PromptWorker>>,
}

impl<'a> ConsolePrompt<'a> {
    pub(crate) fn new(owners: &'a Owners) -> ConsolePrompt<'a> {
        ConsolePrompt { owners, worker: std::cell::RefCell::new(None) }
    }
}

/// What the worker of a prompt is given to do.
type Job = Box<dyn FnOnce() -> Asked + Send>;

/// One thread that makes all the calls of the prompt of a run, desktop or console, and lives for
/// as long as they are asked. A console prompt that is made on a thread of its own each time is
/// answered at once with a cancel from the second call on, with nothing shown (G97 shows it in a
/// pseudo console): the functions of `credui.dll` that read the console keep state that
/// goes with the thread that first called them. The desktop prompt is the same library, and
/// makes its calls in the same way, so that a second and a third attempt are shown and are not
/// cancelled at once, which the guard would read as the user refusing the run.
pub(super) struct PromptWorker {
    jobs: std::sync::mpsc::Sender<Job>,
    answers: std::sync::mpsc::Receiver<Asked>,
}

impl PromptWorker {
    pub(super) fn start() -> Option<PromptWorker> {
        let (jobs, job_receiver) = std::sync::mpsc::channel::<Job>();
        let (answer_sender, answers) = std::sync::mpsc::channel::<Asked>();
        let thread = std::thread::Builder::new().name("prompt worker".into()).spawn(move || {
            while let Ok(job) = job_receiver.recv() {
                if answer_sender.send(job()).is_err() {
                    break;
                }
            }
        }).ok()?;
        // Hardened through its creation handle before any job is
        // queued for it: this thread makes the credential calls of
        // the run, so it is hardened like every other worker of a
        // process-owning invocation. A thread that cannot be
        // hardened is never started as a worker (its channels close
        // with this return and it ends at once).
        if crate::cli_process::harden_worker(&thread).is_err() {
            return None;
        }
        Some(PromptWorker { jobs, answers })
    }

    /// Give `job` to the thread and wait for what it comes back with, for at most `timeout`, with
    /// `owners` watched. A job that is still at work when the wait ends is left, and ends with the
    /// process (the run is refused, so the process is about to).
    pub(super) fn ask(&self, owners: &Owners, timeout: Duration, job: impl FnOnce() -> Asked + Send + 'static) -> Asked {
        if self.jobs.send(Box::new(job)).is_err() {
            return Asked::Unavailable("the prompt ended without an answer".into());
        }
        asked_from(wait_for(&self.answers, timeout, owners))
    }
}

impl Prompt for ConsolePrompt<'_> {
    fn begin(&self, text: &str) -> Result<Claim, String> {
        if !console_stdin() {
            return Err("standard input is not an interactive console; the console prompt is never read from a pipe, a file or the null device".into());
        }
        if console_api().is_none() {
            return Err(NOT_AVAILABLE.into());
        }
        let lock = PromptLock::take()?;
        // A console that does not take the text (a selection is being made in it, or nobody reads
        // it) must not hold the guard, with the lock, for ever.
        let text = text.to_string();
        write_bounded(self.owners, Duration::from_millis(SYSTEM_TIMEOUT_MS), move || write_console(&text))?;
        Ok(Claim { _lock: Some(lock) })
    }

    fn ask(&self, _attempt: u32, last_error: u32, _text: &str) -> Asked {
        let Some(api) = console_api() else { return Asked::NotShown(NOT_AVAILABLE.into()) };
        let mut slot = self.worker.borrow_mut();
        if slot.is_none() {
            *slot = PromptWorker::start();
        }
        let Some(worker) = slot.as_ref() else { return Asked::NotShown("the prompt could not be started".into()) };
        // The prompt reads with echo off. When the prompt returns, or the wait gives up on a read
        // that goes on, the console is put back as it was, so the shell is not left with no echo.
        // While the prompt reads, a control event that ends the guard (Ctrl+C, Ctrl+Break, closing
        // the console) puts it back too, before the default action ends the process. Nothing can
        // do it when the guard is ended from outside (Task Manager). The name of the account is
        // read inside the same limit as the prompt.
        let echo = EchoGuard::new(api, console_mode());
        let asked = worker.ask(self.owners, Duration::from_millis(PROMPT_TIMEOUT_MS), move || console_dialog(api, last_error, own_account_name()));
        drop(echo);
        // The line that was typed may have reached the command history that the console keeps for
        // the program: clear it, whatever the prompt came back with.
        expunge_history(api);
        if matches!(asked, Asked::Unavailable(_) | Asked::Abandoned(_)) {
            // The prompt line is left open when nobody answers (the starter exits, or the time is up): end it, so that the
            // refusal that follows is a line of its own in the record on the console.
            let _ = write_bounded(self.owners, Duration::from_millis(SYSTEM_TIMEOUT_MS), || write_console(""));
        }
        asked
    }
}

/// A write to the console, with a limit and the owner watched.
pub(super) fn write_bounded(owners: &Owners, limit: Duration, write: impl FnOnce() -> bool + Send + 'static) -> Result<(), String> {
    match bounded(limit, owners, write) {
        Ok(true) => Ok(()),
        Ok(false) => Err("the console cannot be written to, so the user cannot be shown what is asked".into()),
        Err(Unanswered::OwnerGone) => Err(OWNER_GONE.into()),
        Err(_) => Err("the console did not take what is asked within the time allowed, so the user cannot be shown it".into()),
    }
}

/// The handle of standard input, when there is one.
pub(super) fn stdin_handle() -> Option<Handle> {
    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    (!handle.is_null() && handle != INVALID_HANDLE).then_some(handle)
}

/// The mode of the console that standard input is, when it is one.
pub(super) fn console_mode() -> Option<u32> {
    let mut mode = 0u32;
    (unsafe { GetConsoleMode(stdin_handle()?, &mut mode) } != 0).then_some(mode)
}

/// The mode of the console that a control event puts back: `NO_MODE` when there is none to put back.
pub(super) static MODE_TO_RESTORE: AtomicU64 = AtomicU64::new(NO_MODE);
pub(super) const NO_MODE: u64 = u64::MAX;

/// The handler of the control events while a console prompt reads: it puts the mode back and
/// returns false, so that the default action still ends the guard.
pub(super) unsafe extern "system" fn restore_on_control_event(_event: u32) -> Bool {
    let mode = MODE_TO_RESTORE.load(Ordering::SeqCst);
    if mode != NO_MODE {
        restore_console_mode(Some(mode as u32));
    }
    0
}

/// Puts the console mode back when it is dropped, and for as long as it lives puts it back on a
/// control event that ends the guard.
pub(super) struct EchoGuard {
    saved: Option<u32>,
    handler: SetConsoleCtrlHandlerFn,
}

impl EchoGuard {
    pub(super) fn new(api: &ConsoleApi, saved: Option<u32>) -> EchoGuard {
        if let Some(mode) = saved {
            MODE_TO_RESTORE.store(mode as u64, Ordering::SeqCst);
            unsafe { (api.handler)(Some(restore_on_control_event), 1) };
        }
        EchoGuard { saved, handler: api.handler }
    }
}

impl Drop for EchoGuard {
    fn drop(&mut self) {
        if self.saved.is_some() {
            unsafe { (self.handler)(Some(restore_on_control_event), 0) };
            MODE_TO_RESTORE.store(NO_MODE, Ordering::SeqCst);
        }
        restore_console_mode(self.saved);
    }
}

/// Clear the command history that the console keeps for the file name of this program. The
/// result is ignored: a system without the call, or a console without a history, has nothing to clear.
pub(super) fn expunge_history(api: &ConsoleApi) {
    let Some(expunge) = api.expunge else { return };
    let image = image_of(unsafe { GetCurrentProcessId() });
    let name = crate::file_name_of(&image);
    if name.is_empty() {
        return;
    }
    unsafe { expunge(wide(name).as_ptr()) };
}

/// Put the mode of the console back to `saved`, if it was read.
pub(super) fn restore_console_mode(saved: Option<u32>) {
    if let (Some(mode), Some(handle)) = (saved, stdin_handle()) {
        unsafe { SetConsoleMode(handle, mode) };
    }
}

/// Write `text` and a line end to the console itself (`CONOUT$`), whatever standard output is.
pub(super) fn write_console(text: &str) -> bool {
    unsafe {
        let name = wide("CONOUT$");
        let console = CreateFileW(name.as_ptr(), GENERIC_WRITE, FILE_SHARE_READ_AND_WRITE, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut());
        if console == INVALID_HANDLE {
            return false;
        }
        // Encoded from the text directly, with the line end's
        // two units appended: a formatted String first would be
        // an allocation and a copy the encoding walks again.
        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(b'\r' as u16);
        units.push(b'\n' as u16);
        // The count is followed until every unit is taken, as at
        // every other write of the tree: this text is what the
        // user approves by, so a write the console took only in
        // part must not read as shown (the desktop prompt refuses
        // a message the dialog would clip, for the same reason).
        let mut done = 0usize;
        let mut ok = true;
        while done < units.len() {
            let mut written = 0u32;
            if WriteConsoleW(console, units[done..].as_ptr() as *const c_void, (units.len() - done) as u32, &mut written, std::ptr::null_mut()) == 0 || written == 0 {
                ok = false;
                break;
            }
            done += written as usize;
        }
        CloseHandle(console);
        ok
    }
}

/// The prompt, with the name of the account the guard runs as already in the user field, so that
/// it asks for the password alone (the name cannot be changed there). Only when the guard cannot
/// read its own account name does the prompt ask for a name too, and a name that is typed is looked
/// up before a logon is tried (see `account_check`). The flags are `CONSOLE_PROMPT_FLAGS`:
/// `CREDUI_FLAGS_EXCLUDE_CERTIFICATES` is required (a prompt on a console takes no certificate or
/// smart card, and any call without it fails with `ERROR_INVALID_FLAGS`), the prompt is always
/// shown, nothing is saved, and the credentials are generic: the target names no server to check
/// them against.
pub(super) fn console_dialog(api: &ConsoleApi, last_error: u32, own_name: Option<String>) -> Asked {
    let mut credentials = Box::new(Credentials::new());
    if let Some(name) = own_name {
        for (slot, unit) in credentials.user.0.iter_mut().zip(name.encode_utf16().take(CREDUI_USER_CHARS - 1)) {
            *slot = unit;
        }
    }
    let target = wide(CAPTION);
    let mut save = 0;
    let status = unsafe {
        (api.prompt)(target.as_ptr(), std::ptr::null_mut(), last_error,
            credentials.user.0.as_mut_ptr(), CREDUI_USER_CHARS as u32,
            credentials.password.0.as_mut_ptr(), CREDUI_PASSWORD_CHARS as u32,
            &mut save, CONSOLE_PROMPT_FLAGS)
    };
    match status {
        0 => Asked::Entered(credentials),
        ERROR_CANCELLED => Asked::Cancelled,
        code => Asked::Unavailable(format!("the console prompt failed: {}", error_for(code).message())),
    }
}
