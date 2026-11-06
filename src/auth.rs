// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The exception above the cap: the approval of the signed-in user, at the start of a run.
//!
//! A run started with `--extendable` and a `--max-extension` over the default cap (six hours)
//! is a per-run exception. Before the job is created and before the command starts, the guard
//! asks the signed-in user for approval. The default method (`--authenticate desktop`) asks for
//! the consent of Windows Hello (`UserConsentVerifier`: the Personal Identification Number
//! (PIN), fingerprint or face of the user, whose PIN guessing the Trusted Platform Module
//! limits), provided that the guard runs as the user who is signed in to its session. When
//! Windows Hello is not available (it is not set up, a policy disables it, or there is no
//! device) the same method shows the credential dialog of Windows instead and asks for the
//! password of the account the guard runs as. With `--authenticate console` the password is
//! asked for by a prompt on a console that is really there. A password is checked with
//! `LogonUserW`, and the user of the logon token must be the user the guard runs as.
//! A run that nobody is present to approve, or that is refused, exits with 125 and never starts
//! its command. At most three attempts at the password are made (Windows Hello is asked once),
//! each failed one counting toward account lockout, and every attempt is a line of the log. A
//! run that another guard started (the automatic wrapper, or a hook that wraps a line whole) is
//! refused before anything is shown: a command that an agent runs is such a run, and the agent
//! must not be able to summon the prompt.
//! Two tests make that refusal. The first looks at the file names of the live processes above the
//! guard, by the rule that `lib.rs` uses to select automatic mode; the search follows the parent
//! chain only while each parent is alive, so a parent that has exited, or one that a launcher
//! chose, ends it. The second looks at the job the process is in: every job a guard makes kills its
//! processes when it closes, so a descendant of a guard is in such a job however its chain of
//! parents looks, unless it has put itself in a job of its own (nested jobs are allowed, so any
//! process of the user can). The reason of the refusal names the test that fired and what it
//! found. The process that started the guard is watched while a prompt is open: if it
//! exits, nobody is waiting for the run and it is refused. A refusal after a prompt was shown
//! keeps the right to a prompt for ten seconds more, so that guards started in a loop do not put
//! one prompt after another on the screen. A prompt left open at its limit is the exception: the
//! process ends at once, which is expected to end the dialog or the read, so that nothing typed
//! into it is lost to a run that is already refused (step 5 of the manual check records whether
//! it does). A request for Windows Hello that is left open is cancelled
//! (`IAsyncInfo::Cancel`) before the process ends; steps 5 and 12 of the manual check record what
//! the system does with the prompt.
//!
//! Windows Hello verifies the user who is signed in to the session, not the account that the
//! guard runs as, so the consent is used only when the two are the same user (see
//! `session_user_matches`); a guard started as another account (`runas`) is refused, and does not
//! borrow the Windows Hello of the user at the screen. The request is made only on the desktop that
//! receives the input of that user (see `on_input_desktop`): a face can be matched with no act by the
//! user, so a request that is placed where nobody is looking is not made. One request is made for a
//! run, and Windows Hello keeps its own count of tries and its own lockout.
//!
//! The password lives only in buffers that are zeroed with volatile writes when they are
//! dropped, on every path, and is never put in a `String`, never logged and never echoed. The
//! release profile aborts on a panic without running destructors, so a panic hook zeroes every
//! buffer that is alive before the abort (see `install_panic_hook`). The exception resists
//! accidents and agents that lack the password (or the PIN, fingerprint or face of the user, though a
//! face can be matched with no act by the user); it does not resist a process of the same user that
//! can tamper with the guard itself (see `SECURITY.md`).
//!
//! The logic is behind three traits, `Prompt`, `Verifier` and `Consenter`, so that the unit tests
//! drive the attempt loop and the request for consent with stubs. The stubs exist only under
//! `cfg(test)`: no argument, environment variable or file selects one, so a release executable
//! has no way to skip the prompt. The Windows Runtime and the window that owns the request of
//! Windows Hello are reached through system calls bound at run time (`sys.rs`), so the programs
//! that can never ask import none of them.

use crate::engine::{format_seconds, image_path_of_handle, started_at};
use crate::extend::{current_user_sid, exception_line, hours_figure, image_of, now_ms, sid_text, token_user_sid, ExceptionRecord, Log, Settings, RECORD_MS};
use crate::sys::windows::{
    CREDUI_FLAGS_ALWAYS_SHOW_UI, CREDUI_FLAGS_DO_NOT_PERSIST, CREDUI_FLAGS_EXCLUDE_CERTIFICATES,
    CREDUI_FLAGS_GENERIC_CREDENTIALS, CloseHandle, CreateMutexW, ERROR_ALREADY_EXISTS,
    GetCurrentProcess, GetLastError, Handle, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    SYNCHRONIZE, WAIT_OBJECT_0, WaitForSingleObject, os_error, wide,
};
use crate::GuardError;
use std::ffi::c_void;
use std::sync::atomic::{compiler_fence, AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod console;
mod secrets;
mod desktop;
mod hello;
mod refusal;
mod message;
mod names;
mod password_budget;
mod verify;

pub(crate) use console::*;
pub(crate) use secrets::*;
pub(crate) use desktop::*;
pub(crate) use hello::*;
pub(crate) use refusal::*;
pub(crate) use message::*;
use names::*;
pub(crate) use verify::*;

/// The caption of the prompt, and the name the console prompt gives as its target: the product name.
pub(crate) const CAPTION: &str = "Process Tree Guard";

/// Prompts per run and slots in the shared password budget. Windows also
/// applies the account's lockout policy to each native password check.
pub(crate) const MAX_ATTEMPTS: u32 = 3;

/// How long a prompt waits for an answer before the run is refused.
const PROMPT_TIMEOUT_MS: u64 = 300_000;

/// How long the check of the credentials waits for the system before the run is refused. A
/// domain controller that does not answer can keep `LogonUserW` for a minute or more.
const LOGON_TIMEOUT_MS: u64 = 60_000;

/// How often the process that started the guard is looked at while a wait goes on.
const OWNER_SLICE_MS: u64 = 250;

/// How long a name of the account, and a write to the console, wait for the system before the run is refused.
const SYSTEM_TIMEOUT_MS: u64 = 5_000;

/// How long the guard waits after a name or password that was not accepted before it asks again:
/// a loop that starts guards to guess a password pays this for every try. It slows such a loop and does not stop it.
const RETRY_DELAY_MS: u64 = 2_000;

/// How long a refusal after a prompt was shown keeps the right to a prompt before the guard returns,
/// so that a loop of guards over the cap cannot put one prompt after another on the screen. A guard
/// started in that time finds the right taken and is refused at once, without showing anything.
const COOL_DOWN_MS: u64 = 10_000;

/// The most characters each field of the prompt shows once its characters are written out:
/// reading stops before the first character that would take the field past its limit, so no field
/// is longer than this, whatever its characters are (the longest escape is ten characters,
/// `\u{10ffff}`). With the fixed text, the note and the words that say how much was cut, the
/// whole message stays under `CREDUI_MAX_MESSAGE_LENGTH`, the most the desktop dialog takes.
const COMMAND_LIMIT: usize = 140;
const PROGRAM_LIMIT: usize = 80;
const ACCOUNT_LIMIT: usize = 40;
const PARENT_LIMIT: usize = 80;

/// The message of the request for Windows Hello is shorter than that of the credential dialog: the most UTF-16 units it may have
/// (checked before the request is made; a longer text refuses the run and never reaches the system), and the limits of its
/// fields, in characters once written out, which are the largest the text starts with. A text that is longer than the
/// message limit with them is measured with smaller ones and built once, at the chosen share (`hello_text`). The limits are defaults; step 12 of the manual
/// check reads the whole text in the dialog, and they are adjusted if it is clipped.
const HELLO_MESSAGE_LIMIT: usize = 512;
const HELLO_COMMAND_LIMIT: usize = 100;
const HELLO_PROGRAM_LIMIT: usize = 40;
const HELLO_ACCOUNT_LIMIT: usize = 30;
const HELLO_PARENT_LIMIT: usize = 40;

/// How long the guard waits, after it asked a request for Windows Hello to cancel, for the system to say that it did.
const CANCEL_GRACE_MS: u64 = 3_000;

/// How often the state of an operation of the Windows Runtime is read, and the window that owns the request is served.
const POLL_MS: u32 = 100;

/// The most posted messages of the owner window that are dispatched between two looks at the state of the request. A message that
/// another process sends to the window is handled by the system when the queue is read and is not counted by this limit.
const PUMP_LIMIT: usize = 256;

/// What the result of an operation is set to before the system is asked for it: a value that is none of the results or
/// availabilities of Windows Hello, so that a call that succeeds and leaves it alone cannot read as `Verified` or `Available`,
/// which are 0 (Common Weakness Enumeration, CWE-636, not failing securely; Saltzer and Schroeder, fail-safe defaults).
const NO_RESULT: i32 = -1;

/// The most characters a password in a protected form may have (the form is longer than the password it holds).
const PROTECTED_LIMIT: usize = 4096;

/// The class of the Windows Runtime that asks for the consent of Windows Hello.
const CONSENT_CLASS: &str = "Windows.Security.Credentials.UI.UserConsentVerifier";

/// The class of the window that owns the request.
const OWNER_CLASS: &str = "ProcessTreeGuardConsentOwner";

/// The flags of the console prompt: the prompt is shown even when credentials for the target are
/// stored (without `CREDUI_FLAGS_ALWAYS_SHOW_UI` a generic credential kept under the name of the
/// product could be returned with no prompt), no certificate or smart card (a prompt on a console
/// takes none, and a call without this flag fails with `ERROR_INVALID_FLAGS`), nothing is saved,
/// and the credentials are generic: the target names no server to check them against.
const CONSOLE_PROMPT_FLAGS: u32 = CREDUI_FLAGS_ALWAYS_SHOW_UI | CREDUI_FLAGS_EXCLUDE_CERTIFICATES | CREDUI_FLAGS_DO_NOT_PERSIST | CREDUI_FLAGS_GENERIC_CREDENTIALS;

const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_NONE_MAPPED: u32 = 1332;
const ERROR_NOT_ENOUGH_MEMORY: u32 = 8;

/// The code of a failure that has no Windows error: the account of the credentials could not be read.
const UNIDENTIFIED: u32 = 0;

/// Set by the unit tests only: the hook then zeroes the buffers of the panicking thread alone, so
/// that a test that panics on purpose does not wipe the secrets of tests that run beside it.
#[cfg(test)]
static HOOK_FOR_THIS_THREAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Why a prompt cannot be shown where a library of it is missing.
const NOT_AVAILABLE: &str = "the sign-in prompt is not available on this system";

/// Why a run whose starter has gone is refused.
const OWNER_GONE: &str = "the process that started the run exited, so nobody is waiting for it";

/// The reason a user name or password that the system does not take is given: the same text for a
/// name that is unknown, a password that is wrong and an account that is not the guard's own, so
/// that the reason tells a typist nothing about which accounts exist.
const NOT_ACCEPTED: &str = "the user name or password was not accepted";

/// The reason an empty password is given. It is not a secret, so a reason of its own tells a typist
/// nothing, and the record must not say that the system rejected a credential when none was sent.
const EMPTY_PASSWORD: &str = "no password was entered, so no logon was tried";

/// The errors of `LogonUserW` that Windows gives only once the password was found to be right: an
/// account restriction, invalid logon hours, an invalid workstation, a disabled account, an
/// expired account, an expired password, an account without the right to log on at this computer
/// (1380 and 1385, which have the same text) and a password that must be changed. At the console
/// prompt, where a program can be the one that types, they are given the reason of a password
/// that is not accepted, so that the reason of a refusal does not tell it that a guess was right
/// (Common Weakness Enumeration, CWE-204). The desktop prompt cannot be answered by a program, so
/// there the error is shown.
const AFTER_PASSWORD_CHECKED: [u32; 9] = [1327, 1328, 1329, 1330, 1331, 1380, 1385, 1793, 1907];

/// How the approval of the signed-in user is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// The consent of Windows Hello where it is available, and otherwise the credential dialog of Windows on the secure desktop.
    Desktop,
    /// A prompt on the console, accepted only when standard input is a real console.
    Console,
}

impl Method {
    /// The method named by `--authenticate`.
    pub fn from_name(name: &str) -> Option<Method> {
        match name {
            "desktop" => Some(Method::Desktop),
            "console" => Some(Method::Console),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Method::Desktop => "desktop",
            Method::Console => "console",
        }
    }
}

/// An exception above the cap: how to ask, and for which command line. The cap it is for is
/// the one the run is enforced against, `Settings::max_seconds`, and nothing else.
pub struct Exception {
    pub method: Method,
    /// The command line the prompt shows (for a shell, the command text).
    pub command: String,
    /// The deadline the run starts with (`--timeout`), in seconds, which the prompt shows beside the cap.
    pub timeout: f64,
}

/// The processes that started the guard, held by handle while a prompt is open. If one exits,
/// nobody is waiting for the run any more, and a process that takes its identifier must not be
/// taken for it: a process is taken only if it started before the guard did, as a parent must
/// have. They are opened before the first prompt and closed on every path, except those that
/// `into_handles` gives to the run, which watches the same handles for as long as it lasts.
pub(crate) struct Owners(Vec<Handle>);

/// True when a process created at `theirs` can be the parent of one created at `ours`: both
/// times are known, and it is not later. A parent that is gone, whose identifier was taken by a
/// process that started after the child, fails this.
fn started_before(theirs: Option<u64>, ours: Option<u64>) -> bool {
    matches!((theirs, ours), (Some(theirs), Some(ours)) if theirs <= ours)
}

impl Owners {
    /// Open each process of `ids`; the run is refused when one cannot be opened, or is not one
    /// that started before this process (its identifier was reused).
    pub(crate) fn open(ids: &[u32]) -> Result<Owners, GuardError> {
        let mut owners = Owners(Vec::new());
        let ours = started_at(unsafe { GetCurrentProcess() });
        for id in ids {
            let handle = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, *id) };
            if handle.is_null() {
                return Err(os_error());
            }
            // Closed with the others on every path from here.
            owners.0.push(handle);
            if !started_before(started_at(handle), ours) {
                return Err(GuardError("the process that started the run is gone and its identifier was reused".into()));
            }
        }
        Ok(owners)
    }

    /// No processes: nobody is watched.
    pub(crate) fn empty() -> Owners {
        Owners(Vec::new())
    }

    /// True when one of the processes has exited.
    fn gone(&self) -> bool {
        self.0.iter().any(|handle| unsafe { WaitForSingleObject(*handle, 0) } == WAIT_OBJECT_0)
    }

    /// The path of the file of the nearest process, read from the handle that is held; empty when it cannot be read.
    pub(crate) fn image(&self) -> String {
        self.0.first().map_or_else(String::new, |handle| image_path_of_handle(*handle))
    }

    /// The handles, which the caller must close: they are not closed with this value.
    pub(crate) fn into_handles(mut self) -> Vec<Handle> {
        std::mem::take(&mut self.0)
    }

    /// Watch the objects `handles`, which are closed with it; signalled means gone.
    #[cfg(test)]
    fn of(handles: Vec<Handle>) -> Owners {
        Owners(handles)
    }
}

impl Drop for Owners {
    fn drop(&mut self) {
        for handle in self.0.drain(..) {
            unsafe { CloseHandle(handle) };
        }
    }
}

/// The right to have a prompt open on this desktop session. A guard that finds another one's
/// prompt open does not add its own: several guards started together (an agent that starts
/// one per tool call) would otherwise stack prompts on the screen, each waiting for minutes. The
/// lock is a named object made by whoever comes first and gone when its last handle closes. A
/// guard that refuses a run after its prompt was shown keeps the lock for `COOL_DOWN_MS` more, so
/// that the prompts do not follow one another either.
struct PromptLock(Handle);

/// The name of the lock of the prompts of the guard in a session.
const PROMPT_LOCK_NAME: &str = "Local\\ProcessTreeGuardPrompt";

impl PromptLock {
    fn take() -> Result<PromptLock, &'static str> {
        PromptLock::take_named(PROMPT_LOCK_NAME)
    }

    /// The lock under `name`. A program of the same session that holds the name keeps every run
    /// from taking it, which is told apart from a real second prompt only by the reason given.
    fn take_named(name: &str) -> Result<PromptLock, &'static str> {
        let name = wide(name);
        unsafe {
            let handle = CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
            // Nothing may come between the call and the error code it left.
            let existed = GetLastError() == ERROR_ALREADY_EXISTS;
            if handle.is_null() {
                return Err("the lock that keeps two sign-in prompts apart cannot be made");
            }
            if existed {
                CloseHandle(handle);
                return Err("another sign-in prompt of the guard is open or was refused less than 10 seconds ago, or another program holds its lock name");
            }
            Ok(PromptLock(handle))
        }
    }
}

impl Drop for PromptLock {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

// ------------------------------------------------------------------- the seam

/// What a prompt came back with.
pub(crate) enum Asked {
    Entered(Box<Credentials>),
    Cancelled,
    /// The prompt could not be started for this attempt, so nothing was shown for it; the reason
    /// says why. It is logged as the attempts made before it.
    NotShown(String),
    /// The prompt failed or went unanswered after it was shown; the reason says why.
    Unavailable(String),
    /// The prompt is still open and nobody answered within the time allowed: its thread is
    /// left at work, so the run is refused and the process ends at once, without the wait that
    /// follows another refusal, which is expected to end the dialog or the read.
    Abandoned(String),
    /// The prompt was answered, but what it returned cannot be read as a name and a password (a
    /// Personal Identification Number (PIN) or a smart card, or a name or password longer than the
    /// buffers). It uses an attempt like a wrong password, and no logon is tried for it.
    Rejected(String),
}

/// What checking credentials came to.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Verified {
    Approved,
    /// The system does not accept the name and password, or the name is another account's.
    Wrong,
    /// The system accepts them, for an account other than the one the guard runs as.
    OtherUser,
    /// The failure is of the system, not of what was typed: a Windows error, or `UNIDENTIFIED`.
    Failed(u32),
    /// The cross-run budget could not admit or settle a password check.
    RetryBudget(&'static str),
    /// The process that started the guard exited while the check went on.
    OwnerGone,
}

/// What a run holds while its prompts are shown: the right to have one open.
pub(crate) struct Claim {
    _lock: Option<PromptLock>,
}

impl Claim {
    #[cfg(test)]
    fn none() -> Claim {
        Claim { _lock: None }
    }
}

pub(crate) trait Prompt {
    /// Check that the prompt can be shown at all, and take what it needs for the whole run (the
    /// right to have a prompt open, the console written to). `text` is what the prompt says. An
    /// error is the reason that nobody can be asked: nothing was shown, and no attempt was made.
    fn begin(&self, text: &str) -> Result<Claim, String>;

    /// The right to be asked at all, apart from what the dialog needs: that somebody is present and no other prompt of the
    /// guard is open. The desktop method takes it once, before it asks for Windows Hello, and keeps it if the dialog is shown
    /// after it. For a prompt that has nothing apart it is `begin`.
    fn seat(&self, text: &str) -> Result<Claim, String> {
        self.begin(text)
    }

    /// What the dialog itself needs, checked once the right is held and the dialog is to be shown. An error is the reason
    /// that nobody can be asked, as for `begin`.
    fn ready(&self, _text: &str) -> Result<(), String> {
        Ok(())
    }

    /// Ask for credentials. `last_error` is the error of the attempt before (`ERROR_LOGON_FAILURE`
    /// after a refusal, 0 on the first).
    fn ask(&self, attempt: u32, last_error: u32, text: &str) -> Asked;
}

pub(crate) trait Verifier {
    /// Check the credentials, which are wiped when this returns (or when the check that
    /// went on without it ends).
    fn verify(&self, credentials: Box<Credentials>) -> Verified;
}

#[cfg(test)]
mod tests;

/// The answers a unit test of the engine can give in place of the prompt, so that the run an
/// approval starts is exercised end to end. Only `decide` looks at them, and only under `cfg(test)`.
#[cfg(test)]
pub(crate) mod seam;
