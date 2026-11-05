// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Why an approval is refused before anyone is asked: the refusal reasons, and the tests that detect a guard above.

use super::*;


use crate::sys::windows::{

    ERROR_LOGON_FAILURE, EXTENDED_LIMIT_CLASS, ExtendedLimitInformation, GetCurrentProcessId,

    KILL_ON_JOB_CLOSE, QueryInformationJobObject, error_for,

};
/// Why a run started inside another run cannot have an exception.
pub(super) const NESTED: &str = "an exception above the cap cannot be granted inside another run";

/// Why a run that another guard started is not asked.
pub(super) const WRAPPED: &str = "an exception above the cap is not asked for by a run that another guard started (the automatic wrapper or a hook envelope) \
    or that runs inside a job that kills its processes when it closes, because a command that an agent runs is such a run; \
    start it from a terminal that is not under a guard";

/// What the reason of that refusal says when the names above the guard found a program of the
/// guard: the file name follows it.
pub(crate) const FOUND_ABOVE: &str = "found above the guard: ";

/// What the reason says when the job the guard is in kills its processes when it closes.
pub(crate) const JOB_KILLS: &str = "the job the guard is in kills its processes when it closes";

/// The file names, in lower case, of the programs of the guard that run a command: the full
/// command, the deadline role and the contain role, the name of the guard installed under the earlier name, and the
/// copies that are the automatic wrapper. Any other file name that ends in -auto is one too (see
/// `is_automatic_copy`).
pub(super) const GUARD_PROGRAMS: [&str; 7] = [
    "process-tree-guard.exe", "process-guard.exe", "process-tree-guard-deadline.exe", "process-tree-guard-contain.exe",
    "process-tree-guard-auto.exe", "process-guard-auto.exe", "process-tree-guard-deadline-auto.exe",
];

// ----------------------------------------------------------------- the decision

/// The line for a refusal, in the log, and the error the run ends with. `attempt` is the
/// number of the attempt, or the attempts made before the refusal when nothing was shown for it
/// (0 for a run that was refused before any was made).
pub(super) fn refuse(settings: &Settings, log: &Log, subject: &Subject, attempt: u32, reason: &str) -> GuardError {
    refuse_by(settings, log, subject, method_of(settings), attempt, reason)
}

/// `refuse` for a run that was asked by `method` (the word the log gives, which for the desktop method is `hello` once
/// the consent of Windows Hello has been chosen).
pub(super) fn refuse_by(settings: &Settings, log: &Log, subject: &Subject, method: &str, attempt: u32, reason: &str) -> GuardError {
    // The refusal is a decision line, not a notice: it travels by the
    // decision entry point, and a refusal the audit file's sink could
    // not queue is reported as unrecorded in the error the run ends
    // with, as an approval that could not be recorded is. The refusal
    // itself is unchanged either way.
    if log.push_decision(&note(settings, subject, method, false, attempt, reason)) {
        GuardError(format!("exception refused: {}; the run did not start", reason))
    } else {
        GuardError(format!("exception refused: {}; the run did not start; the decision was not recorded in the audit file", reason))
    }
}

/// The word the log gives for the method of the exception, as the launch arguments name it.
pub(super) fn method_of(settings: &Settings) -> &'static str {
    settings.exception.as_ref().map_or("", |exception| exception.method.name())
}

/// The line of the log for a decision. It names what the decision is about: the account of the
/// guard (as the system writes it, not anything that was typed), the command line that was shown,
/// cut and written out as the prompt shows it, how the user was asked (`method`: `desktop` for the
/// credential dialog and for everything before Windows Hello was chosen, `hello`, or `console`),
/// the deadline the run starts with, and the nearest process that started the guard, as the prompt shows it. The message for
/// Windows Hello shows less of both than the credential dialog, so for that method they are cut as that message cuts them.
pub(super) fn note(settings: &Settings, subject: &Subject, method: &str, approved: bool, attempt: u32, reason: &str) -> String {
    let pid = unsafe { GetCurrentProcessId() };
    let share = match settings.exception.as_ref() {
        Some(exception) if method == HELLO_METHOD => Some(hello_share(subject, exception, settings.max_seconds)),
        _ => None,
    };
    let (command, timeout) = settings.exception.as_ref().map_or_else(|| (String::new(), 0.0), |exception| {
        let command = match share {
            Some(percent) => shown(&exception.command, HELLO_COMMAND_LIMIT * percent / 100, Keep::Start),
            None => shown_command(&exception.command),
        };
        (command, exception.timeout)
    });
    let started_by = match share {
        _ if subject.parent.is_empty() => String::new(),
        Some(percent) => shown(subject.parent, HELLO_PARENT_LIMIT * percent / 100, Keep::End),
        None => shown(subject.parent, PARENT_LIMIT, Keep::End),
    };
    exception_line(&ExceptionRecord {
        unix_ms: now_ms(), pid, image: &image_of(pid), account: subject.account, command: &command, method, timeout, started_by: &started_by,
        requested: settings.max_seconds, approved, attempt, reason,
    })
}

/// The name of the account the guard runs as, read on a thread with a limit; empty when it cannot be read.
pub(super) fn account_of_the_guard(owners: &Owners) -> String {
    bounded(Duration::from_millis(SYSTEM_TIMEOUT_MS), owners, own_account_name).ok().flatten().unwrap_or_default()
}

/// The refusal of a run whose starter cannot be watched (it cannot be read or opened, or its
/// identifier was reused): logged as a refusal before any attempt, like every other.
pub(crate) fn refuse_unwatched(settings: &Settings, log: &Log, reason: &str) -> GuardError {
    let account = account_of_the_guard(&Owners::empty());
    let subject = Subject { account: &account, parent: "", program: "" };
    refuse(settings, log, &subject, 0, &format!("the process that started the run cannot be watched: {}", reason))
}

/// The hours of a cap in seconds, as the prompt and the approval say it: rounded up, and with the
/// seconds beside it when the figure is not exact, so that it can never be less than the cap.
pub(super) fn hours_phrase(seconds: f64) -> String {
    let text = hours_figure(seconds);
    let hours = if text == "1" { "1 hour".to_string() } else { format!("{} hours", text) };
    if (seconds / 36.0).fract() == 0.0 { hours } else { format!("{} ({} seconds)", hours, format_seconds(seconds)) }
}

/// The first of `ancestors`, nearest first, that is a program of the guard that runs a command,
/// by its file name (the programs of the guard, and any file name that ends in -auto, which
/// selects automatic mode); None when none is. A run that one of them started is a run whose caller
/// chose the command: the automatic wrapper and the hooks that wrap a line whole start the guard
/// for every command that an agent runs, so a prompt would be one that an agent can summon, with
/// text it chooses, as often as it likes. The name returned is the lower case
/// form the refusal has always named: it is made once, for the one matching
/// ancestor, not for every ancestor the search walks past.
pub(crate) fn guard_above(ancestors: &[String]) -> Option<String> {
    ancestors.iter().map(|image| crate::file_name_of(image))
        .find(|name| GUARD_PROGRAMS.iter().any(|program| program.eq_ignore_ascii_case(name)) || is_automatic_copy(name))
        .map(|name| name.to_ascii_lowercase())
}

/// Automatic mode is selected by a file stem that ends in -auto (lib.rs, `stem_ends_with_auto`), whatever the rest of the name: a name that is exactly `..`, or whose only dot leads it, is its own stem.
/// The suffix is plain ASCII, so the stem's trailing characters are compared
/// case-insensitively in place; a tail that is not five whole characters
/// cannot be the suffix, exactly as a byte-wise `ends_with` decided.
pub(super) fn is_automatic_copy(name: &str) -> bool {
    let stem = if name == ".." {
        name
    } else {
        match name.rfind('.') {
            None | Some(0) => name,
            Some(dot) => &name[..dot],
        }
    };
    stem.len() >= 5 && stem.get(stem.len() - 5..).is_some_and(|tail| tail.eq_ignore_ascii_case("-auto"))
}

/// Decide on the exception of a run: refused inside another run or when another guard started it
/// (by the names above it, or by the job it is in), otherwise asked for with the method the launch
/// arguments name. `parent` is the image of the process that started the guard, which the prompt
/// names, `ancestors` are the images of the live processes above the guard
/// that the search of the parent chain reaches, nearest first, and `owners` are the processes that
/// are watched while a prompt is open. Ok(()) means the run may go on to create its job.
pub(crate) fn decide(settings: &Settings, held: bool, program: &str, parent: &str, ancestors: &[String], owners: &Owners, log: &Log) -> Result<(), GuardError> {
    let Some(exception) = settings.exception.as_ref() else { return Ok(()) };
    // The name is read on a thread with a limit: the system may have to ask a domain.
    let account = account_of_the_guard(owners);
    if held {
        let subject = Subject { account: &account, parent: "", program: "" };
        return Err(refuse(settings, log, &subject, 0, NESTED));
    }
    install_panic_hook();
    let subject = Subject { account: &account, parent, program };
    // For the unit tests of the engine only: a stub answers in place of the prompt, so that the run
    // that follows an approval is exercised. It exists under `cfg(test)` and nowhere else.
    #[cfg(test)]
    if let Some(approve) = seam::ANSWER.with(|answer| answer.get()) {
        return authorize(settings, &subject, &seam::SeamPrompt(approve), &seam::SeamVerifier, owners, log);
    }
    // Before any prompt, lock or logon: a run that another guard started is not asked. The job is
    // looked at as well as the names above, because a launcher can name its own parent or leave an
    // intermediate process to exit, which ends the search of the names.
    let above = guard_above(ancestors);
    if above.is_some() || in_guard_job() {
        let cause = match above {
            Some(name) => format!("{}{}", FOUND_ABOVE, shown_command(&name)),
            None => JOB_KILLS.to_string(),
        };
        return Err(refuse(settings, log, &subject, 0, &format!("{} ({})", WRAPPED, cause)));
    }
    let verifier = LogonVerifier { owners };
    match exception.method {
        Method::Desktop => authorize_desktop(settings, &subject, &HelloConsenter, &DesktopPrompt::new(owners), &verifier, owners, log),
        Method::Console => authorize(settings, &subject, &ConsolePrompt::new(owners), &verifier, owners, log),
    }
}

/// Wait `ms` milliseconds before the next attempt, looking at `owners` every `OWNER_SLICE_MS`:
/// false when one has exited, which ends the wait early.
pub(super) fn pause(owners: &Owners, ms: u64) -> bool {
    let end = Instant::now() + Duration::from_millis(ms);
    loop {
        if owners.gone() {
            return false;
        }
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        std::thread::sleep(left.min(Duration::from_millis(OWNER_SLICE_MS)));
    }
}

/// Run the attempts. At most `MAX_ATTEMPTS`: a name or password the system does not accept, or
/// that is not the guard's own account's, or that is empty, or an answer that cannot be read as a
/// name and a password, is an attempt used (and the next prompt comes after a wait of
/// `RETRY_DELAY_MS`); a cancel, a prompt that cannot be shown, a starter that has gone and a
/// failure that more attempts cannot mend end it at once. Each attempt is one line of the log,
/// and the approval stands only if its line is recorded and the process that started the run is
/// still there. A refusal after a prompt was shown keeps the right to a prompt for `COOL_DOWN_MS`
/// more (the starter ending cuts it short), so guards started in that time are refused by the lock;
/// a prompt left open at its limit is refused at once and the process ends, which closes it.
pub(crate) fn authorize(settings: &Settings, subject: &Subject, prompt: &dyn Prompt, verifier: &dyn Verifier, owners: &Owners, log: &Log) -> Result<(), GuardError> {
    let Some(exception) = settings.exception.as_ref() else { return Ok(()) };
    if owners.gone() {
        return Err(refuse(settings, log, subject, 0, OWNER_GONE));
    }
    let text = prompt_text(subject, exception, settings.max_seconds);
    // Held until the attempts are over, so that no second prompt of the guard opens beside this one.
    let _claim = match prompt.begin(&text) {
        Ok(claim) => claim,
        Err(reason) => return Err(refuse(settings, log, subject, 0, &reason)),
    };
    attempts(settings, subject, prompt, verifier, owners, log, &text, "")
}

/// What stops an approval from standing once the user has given it.
pub(super) enum NotRecorded {
    /// The process that started the run has exited, so nobody is waiting for it.
    OwnerGone,
    /// The audit file, or standard error when there is none, cannot take lines.
    Blocked,
    /// The line could not be written.
    Unwritten,
}

impl NotRecorded {
    /// The reason of the refusal.
    pub(super) fn reason(&self) -> &'static str {
        match self {
            NotRecorded::OwnerGone => OWNER_GONE,
            NotRecorded::Blocked => "the audit file (or standard error, when there is no audit file) cannot take lines, so the approval could not be recorded",
            NotRecorded::Unwritten => "the approval could not be recorded",
        }
    }
}

/// Record the approval of `method` at `attempt`: it stands only if the starter is still there and its line is written.
pub(super) fn record_approval(settings: &Settings, subject: &Subject, method: &str, attempt: u32, reason: &str, owners: &Owners, log: &Log) -> Result<(), NotRecorded> {
    if owners.gone() {
        return Err(NotRecorded::OwnerGone);
    }
    if log.blocked() {
        return Err(NotRecorded::Blocked);
    }
    if !log.record_decision(&note(settings, subject, method, true, attempt, reason), RECORD_MS) {
        return Err(NotRecorded::Unwritten);
    }
    Ok(())
}

/// The attempts at the password, with the right to a prompt already held by the caller and the checks that
/// `prompt` makes done. `why` ends the reason of every line of the record and of every refusal: empty, or the
/// words that say why the password is asked for where Windows Hello would have been (it was not available).
pub(super) fn attempts(settings: &Settings, subject: &Subject, prompt: &dyn Prompt, verifier: &dyn Verifier, owners: &Owners, log: &Log, text: &str, why: &str) -> Result<(), GuardError> {
    let Some(exception) = settings.exception.as_ref() else { return Ok(()) };
    let method = method_of(settings);
    let said = |reason: &str| format!("{}{}", reason, why);
    let stop = |attempt: u32, reason: &str| refuse(settings, log, subject, attempt, &said(reason));
    // A refusal after the prompt was shown: the line is written at once and the right to a prompt is
    // kept for a while longer (it is dropped with the claim when the caller returns).
    let refuse_shown = |attempt: u32, reason: &str| {
        let error = stop(attempt, reason);
        pause(owners, cool_down_ms());
        error
    };
    let mut last_error = 0;
    for attempt in 1..=MAX_ATTEMPTS {
        // The credentials are dropped, and so wiped, as soon as they have been checked.
        let mut rejection: Option<String> = None;
        let verdict = match prompt.ask(attempt, last_error, text) {
            Asked::Entered(credentials) if credentials.password_is_empty() => {
                rejection = Some(EMPTY_PASSWORD.to_string());
                Verified::Wrong
            }
            Asked::Entered(credentials) => verifier.verify(credentials),
            Asked::Rejected(reason) => {
                rejection = Some(reason);
                Verified::Wrong
            }
            Asked::Cancelled => return Err(refuse_shown(attempt, "the prompt was cancelled")),
            Asked::NotShown(reason) => return Err(stop(attempt - 1, &reason)),
            Asked::Unavailable(reason) => return Err(refuse_shown(attempt, &reason)),
            // The prompt is still open: nothing may keep the process alive, or the person could type into a prompt
            // whose answer is never read.
            Asked::Abandoned(reason) => return Err(stop(attempt, &reason)),
        };
        let reason = match verdict {
            Verified::Approved => {
                let reason = format!("a cap of {} was approved: the password of the account the guard runs as was given at the {} prompt", hours_phrase(settings.max_seconds), exception.method.name());
                return match record_approval(settings, subject, method, attempt, &said(&reason), owners, log) {
                    Ok(()) => Ok(()),
                    Err(NotRecorded::OwnerGone) => Err(stop(attempt, OWNER_GONE)),
                    Err(unrecorded) => Err(refuse_shown(attempt, unrecorded.reason())),
                };
            }
            Verified::OwnerGone => return Err(stop(attempt, OWNER_GONE)),
            Verified::Wrong | Verified::OtherUser => rejection.unwrap_or_else(|| NOT_ACCEPTED.to_string()),
            Verified::Failed(code) if exception.method == Method::Console && AFTER_PASSWORD_CHECKED.contains(&code) => NOT_ACCEPTED.to_string(),
            Verified::Failed(UNIDENTIFIED) => return Err(refuse_shown(attempt, "the account of the credentials could not be identified")),
            Verified::Failed(code) => return Err(refuse_shown(attempt, &format!("the credentials could not be checked: {}", error_for(code).message()))),
        };
        if attempt == MAX_ATTEMPTS {
            return Err(refuse_shown(attempt, &format!("{}; no attempts are left", reason)));
        }
        log.push_decision(&note(settings, subject, method, false, attempt, &said(&reason)));
        if !pause(owners, retry_delay_ms()) {
            return Err(stop(attempt, OWNER_GONE));
        }
        last_error = ERROR_LOGON_FAILURE;
    }
    Err(GuardError("exception refused: no attempt was made; the run did not start".into()))
}

/// What the prompt is told about the run: the account whose password it asks for (empty when it
/// cannot be read), the image of the process that started the guard (empty when it cannot be
/// read), and the program that will run.
pub(crate) struct Subject<'a> {
    pub account: &'a str,
    pub parent: &'a str,
    pub program: &'a str,
}

/// True when the job this process is in (the innermost one, where jobs are nested) kills its
/// processes when it closes. Every job the guard makes does (`KILL_ON_JOB_CLOSE`, engine.rs), so
/// every descendant of a guard is in such a job whatever its chain of parents looks like, unless it
/// has put itself in a job of its own, which replaces the innermost job that is read here. A host
/// that puts its terminal in a job of that kind is taken for a guard as well. A process in no job,
/// or a job that cannot be read, is not.
pub(super) fn in_guard_job() -> bool {
    let mut limits = ExtendedLimitInformation::default();
    let read = unsafe {
        QueryInformationJobObject(std::ptr::null_mut(), EXTENDED_LIMIT_CLASS, &mut limits as *mut _ as *mut c_void,
            std::mem::size_of::<ExtendedLimitInformation>() as u32, std::ptr::null_mut())
    };
    read != 0 && limits.basic.limit_flags & KILL_ON_JOB_CLOSE != 0
}
