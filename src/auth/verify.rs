// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Checking a password: the bounded background logon, so a check that never answers cannot hold the run.

use super::*;


use crate::sys::windows::{

    ERROR_LOGON_FAILURE, GetLastError, Handle, LOGON32_LOGON_INTERACTIVE,

    LOGON32_PROVIDER_DEFAULT, WAIT_TIMEOUT, logon_api, wide,

};
// ------------------------------------------------------------ bounded waits

/// Why work that was run in the background gave no answer.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Unanswered {
    NotStarted,
    TimedOut,
    Ended,
    /// A process that started the guard exited while it was waited for.
    OwnerGone,
}

/// Wait for an answer on `receiver` for at most `timeout`, looking at `owners` every
/// `OWNER_SLICE_MS`.
pub(super) fn wait_for<T>(receiver: &std::sync::mpsc::Receiver<T>, timeout: Duration, owners: &Owners) -> Result<T, Unanswered> {
    let end = Instant::now() + timeout;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(left.min(Duration::from_millis(OWNER_SLICE_MS))) {
            Ok(answer) => return Ok(answer),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Err(Unanswered::Ended),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if owners.gone() {
                    return Err(Unanswered::OwnerGone);
                }
                if Instant::now() >= end {
                    return Err(Unanswered::TimedOut);
                }
            }
        }
    }
}

/// Run `work` on a thread and wait for its answer for at most `timeout`, looking at `owners`
/// every `OWNER_SLICE_MS`. A thread that is still at work when the wait ends is left, and ends
/// with the process (the run is refused, so the process is about to). An answer that comes after
/// that is dropped by the thread, which wipes it.
pub(super) fn bounded<T: Send + 'static>(timeout: Duration, owners: &Owners, work: impl FnOnce() -> T + Send + 'static) -> Result<T, Unanswered> {
    let (sender, receiver) = std::sync::mpsc::channel();
    // The work is handed to the thread only after the thread has
    // been hardened: the thread waits for its one job, the caller
    // hardens it through its creation handle (as every other worker
    // of a process-owning invocation is hardened), and only then is
    // the job — which may carry credentials — released to it. A
    // thread that cannot be hardened is never given the work: the
    // job channel closes with this return and the thread ends.
    let (job_sender, job_receiver) = std::sync::mpsc::channel::<Box<dyn FnOnce() -> T + Send>>();
    let thread = std::thread::Builder::new().name("prompt".into()).spawn(move || {
        if let Ok(work) = job_receiver.recv() {
            let _ = sender.send(work());
        }
    }).map_err(|_| Unanswered::NotStarted)?;
    if crate::cli_process::harden_worker(&thread).is_err() {
        return Err(Unanswered::NotStarted);
    }
    if job_sender.send(Box::new(work)).is_err() {
        return Err(Unanswered::Ended);
    }
    wait_for(&receiver, timeout, owners)
}

/// What a prompt that was waited for came to.
pub(super) fn asked_from(answer: Result<Asked, Unanswered>) -> Asked {
    match answer {
        Ok(asked) => asked,
        Err(Unanswered::NotStarted) => Asked::NotShown("the prompt could not be started".into()),
        Err(Unanswered::TimedOut) => Asked::Abandoned(format!("no answer within {} seconds", PROMPT_TIMEOUT_MS / 1000)),
        Err(Unanswered::Ended) => Asked::Unavailable("the prompt ended without an answer".into()),
        Err(Unanswered::OwnerGone) => Asked::Unavailable(OWNER_GONE.into()),
    }
}

// ----------------------------------------------------------------- verification

/// Checks credentials with `LogonUserW`, as an interactive logon (`LOGON32_LOGON_INTERACTIVE`,
/// the default provider). That is the check Windows sign-in itself makes, and the logon itself
/// works for a domain account with cached credentials. The look up of the name before it is local
/// when the name is the guard's own account name as the system writes it; a name in another form
/// (a user principal name of a domain account) may contact a domain controller, and where none
/// answers the run is refused with the error of the system. An account that lacks the right to
/// log on locally fails with 1385 and cannot approve.
/// The cost is a logon session (event 4624, logon type 2) that closing the token ends at once, and
/// that an interactive logon caches the logon information as a sign-in does. The security
/// identifier of the token's user must equal that of the guard's own token, so the password of
/// another account that the system accepts does not approve anything, and a name that is not the
/// guard's own account is turned away before any logon is tried for it. The check runs on a
/// thread, and a system that does not answer within `LOGON_TIMEOUT_MS` refuses the run.
pub(crate) struct LogonVerifier<'a> {
    pub(super) owners: &'a Owners,
}

impl Verifier for LogonVerifier<'_> {
    fn verify(&self, credentials: Box<Credentials>) -> Verified {
        if !credentials.password_ends() {
            return Verified::Failed(ERROR_INSUFFICIENT_BUFFER);
        }
        // Nothing is tried for an empty password: it is not a secret, and it would use an attempt of the lockout.
        if credentials.password_is_empty() {
            return Verified::Wrong;
        }
        check_bounded(Duration::from_millis(LOGON_TIMEOUT_MS), self.owners, credentials, logon)
    }
}

/// Run `work` on the credentials with a limit, and say what it came to.
pub(super) fn check_bounded(limit: Duration, owners: &Owners, credentials: Box<Credentials>, work: impl FnOnce(Box<Credentials>) -> Verified + Send + 'static) -> Verified {
    match bounded(limit, owners, move || work(credentials)) {
        Ok(verdict) => verdict,
        Err(Unanswered::NotStarted) => Verified::Failed(ERROR_NOT_ENOUGH_MEMORY),
        Err(Unanswered::TimedOut) => Verified::Failed(WAIT_TIMEOUT),
        Err(Unanswered::Ended) => Verified::Failed(UNIDENTIFIED),
        Err(Unanswered::OwnerGone) => Verified::OwnerGone,
    }
}

/// What a failed `LogonUserW` with the error `code` comes to: the error for a wrong name or
/// password is a name or password that is not accepted; any other is a failure of the system (a
/// locked or disabled account, an expired password, an account without the right to log on
/// locally, a domain that cannot be reached), which another try cannot mend.
pub(super) fn classify_logon_error(code: u32) -> Verified {
    if code == ERROR_LOGON_FAILURE { Verified::Wrong } else { Verified::Failed(code) }
}

fn logon(credentials: Box<Credentials>) -> Verified {
    let Some((name, domain)) = credentials.names() else { return Verified::Failed(ERROR_INSUFFICIENT_BUFFER) };
    let (domain, user) = split_name(&name, &domain);
    if let Some(verdict) = account_check(&full_name(domain.as_deref(), &user)) {
        return verdict;
    }
    let Some(api) = logon_api() else { return Verified::Failed(UNIDENTIFIED) };
    let (user, domain) = (wide(&user), domain.map(|domain| wide(&domain)));
    let mut handle: Handle = std::ptr::null_mut();
    let logged_on = unsafe {
        (api.logon)(user.as_ptr(), domain.as_ref().map_or(std::ptr::null(), |domain| domain.as_ptr()), credentials.password_pointer(),
            LOGON32_LOGON_INTERACTIVE, LOGON32_PROVIDER_DEFAULT, &mut handle)
    };
    if logged_on == 0 {
        return classify_logon_error(unsafe { GetLastError() });
    }
    let token = Token(handle);
    same_user(token_user_sid(token.0), current_user_sid())
}

/// Approved when the user of the logon token is the user the guard runs as: the text forms of
/// their security identifiers are equal. If either cannot be read, nothing is approved.
pub(super) fn same_user(theirs: Result<String, GuardError>, ours: Result<String, GuardError>) -> Verified {
    match (theirs, ours) {
        (Ok(theirs), Ok(ours)) if !theirs.is_empty() && theirs == ours => Verified::Approved,
        (Ok(_), Ok(_)) => Verified::OtherUser,
        _ => Verified::Failed(UNIDENTIFIED),
    }
}
