// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! One audit line's form: how a request, a grant and a refusal are written into the log's text.

use super::*;

// ---------------------------------------------------------------------- the log

/// One request as the log records it.
pub(in crate::extend) struct Entry<'a> {
    pub(in crate::extend) pid: u32,
    pub(in crate::extend) image: &'a str,
    pub(in crate::extend) requested: Option<f64>,
    pub(in crate::extend) granted: Option<f64>,
    pub(in crate::extend) reason: &'a str,
}

/// A number of seconds as the log records it: exactly, and in a bounded number of
/// characters. A value too large or too small to read as a plain decimal is written in
/// scientific form (`1e300`, `5e-324`), never rounded or cut.
/// The guards in this form are for numbers a client sent, which
/// nobody here has validated; the seconds form the rest of the
/// tree uses takes only numbers that were validated or derived
/// from the run's own tick counts and carries no such guards,
/// so the two are not interchangeable even where their text
/// agrees.
/// The string form is compiled for the tests only, which hold text
/// against it; production writes into its buffers with
/// `log_seconds_into`.
#[cfg(test)]
pub(in crate::extend) fn log_seconds(value: f64) -> String {
    if !value.is_finite() {
        "invalid".into()
    } else if value != 0.0 && (value.abs() >= 1e15 || value.abs() < 1e-6) {
        format!("{:e}", value)
    } else {
        format!("{}", value)
    }
}

/// `log_seconds` written into a buffer the caller already holds, in
/// the same text.
fn log_seconds_into(value: f64, out: &mut String) {
    use std::fmt::Write as _;
    if !value.is_finite() {
        out.push_str("invalid");
    } else if value != 0.0 && (value.abs() >= 1e15 || value.abs() < 1e-6) {
        let _ = write!(out, "{:e}", value);
    } else {
        let _ = write!(out, "{}", value);
    }
}

pub(in crate::extend) fn log_line(unix_ms: u64, entry: &Entry) -> String {
    // The line is assembled in its one buffer: the time, the two
    // escaped fields and the two numbers were each built as their own
    // string first (up to six allocations, counting a formatted string
    // per escaped character inside `escape`) and then copied into the
    // line by its format — on the path of every extension request,
    // grant and refusal alike.
    use std::fmt::Write as _;
    let mut out = String::with_capacity(160 + entry.image.len() + entry.reason.len());
    out.push_str("PROCESS_TREE_GUARD_EXTENSION: time=");
    iso8601_into(unix_ms, &mut out);
    let _ = write!(out, " pid={} image=\"", entry.pid);
    escape_into(entry.image, &mut out);
    out.push_str("\" requested=");
    match entry.requested {
        Some(value) => log_seconds_into(value, &mut out),
        None => out.push_str("unknown"),
    }
    out.push_str(" granted=");
    log_seconds_into(entry.granted.unwrap_or(0.0), &mut out);
    out.push_str(if entry.granted.is_some() { " decision=granted reason=\"" } else { " decision=refused reason=\"" });
    escape_into(entry.reason, &mut out);
    out.push('"');
    out
}

/// The line for the end of a hold at the request of the process that held it. It moves
/// the deadline only earlier, and is a line of its own like a grant.
pub(in crate::extend) fn released_line(unix_ms: u64, pid: u32, image: &str, reason: &str) -> String {
    // Assembled in its one buffer, as `log_line` is: building the
    // time and the two escaped fields as their own strings first
    // allocated three more times for a line of the same shape.
    use std::fmt::Write as _;
    let mut out = String::with_capacity(128 + image.len() + reason.len());
    out.push_str("PROCESS_TREE_GUARD_EXTENSION: time=");
    iso8601_into(unix_ms, &mut out);
    let _ = write!(out, " pid={} image=\"", pid);
    escape_into(image, &mut out);
    out.push_str("\" requested=0 granted=0 decision=released reason=\"");
    escape_into(reason, &mut out);
    out.push('"');
    out
}

/// The line for a grant taken back because the process did not take the reply: the run and
/// the process must not disagree about whether a hold exists. It moves the deadline only earlier.
pub(in crate::extend) fn withdrawn_line(unix_ms: u64, back: &Withdraw, take_ms: u32) -> String {
    // One buffer, as in `released_line` above.
    use std::fmt::Write as _;
    let mut out = String::with_capacity(192 + back.image.len());
    out.push_str("PROCESS_TREE_GUARD_EXTENSION: time=");
    iso8601_into(unix_ms, &mut out);
    let _ = write!(out, " pid={} image=\"", back.pid);
    escape_into(&back.image, &mut out);
    out.push_str("\" requested=");
    log_seconds_into(back.seconds, &mut out);
    let _ = write!(out, " granted=0 decision=withdrawn reason=\"the reply was not taken within {} milliseconds; the hold was withdrawn\"", take_ms);
    out
}

/// The line for a grant taken back because its own line was not written to standard error, which
/// is the only record when there is no audit file: a deadline is not moved by a request that is
/// recorded nowhere. It moves the deadline only earlier.
pub(in crate::extend) fn unrecorded_line(unix_ms: u64, back: &Withdraw, wait_ms: u64) -> String {
    // One buffer, as in `released_line` above.
    use std::fmt::Write as _;
    let mut out = String::with_capacity(256 + back.image.len());
    out.push_str("PROCESS_TREE_GUARD_EXTENSION: time=");
    iso8601_into(unix_ms, &mut out);
    let _ = write!(out, " pid={} image=\"", back.pid);
    escape_into(&back.image, &mut out);
    out.push_str("\" requested=");
    log_seconds_into(back.seconds, &mut out);
    let _ = write!(out, " granted=0 decision=withdrawn reason=\"the line of the grant was not written to standard error within {} milliseconds, and there is no audit file; the hold was withdrawn\"", wait_ms);
    out
}

/// A notice about the log itself.
pub(in crate::extend) fn notice_line(unix_ms: u64, text: &str) -> String {
    // One buffer, as in `released_line` above.
    let mut out = String::with_capacity(80 + text.len());
    out.push_str("PROCESS_TREE_GUARD_EXTENSION: time=");
    iso8601_into(unix_ms, &mut out);
    out.push_str(" notice=\"");
    escape_into(text, &mut out);
    out.push('"');
    out
}

/// The hours of a number of seconds as a figure, rounded up to two decimals, with the zeros at
/// the end left off (`24`, `6.5`, `6.95`), so that a cap such as 25000 seconds is not shown as
/// `6.944444444444445`. Rounded up from the whole seconds, so that the figure is never smaller
/// than the cap: a cap just over six hours is never shown as the default cap that needs no approval.
pub(crate) fn hours_figure(seconds: f64) -> String {
    let mut out = String::with_capacity(8);
    hours_figure_into(seconds, &mut out);
    out
}

/// `hours_figure` written into a buffer the caller already holds, in
/// the same text: the figure's two digits are formed on the stack,
/// not as a formatted string the result then copies out of.
fn hours_figure_into(seconds: f64, out: &mut String) {
    use std::fmt::Write as _;
    let hundredths = (seconds / 36.0).ceil() as u64;
    let mut fraction = [0u8; 2];
    fraction[0] = b'0' + (hundredths % 100 / 10) as u8;
    fraction[1] = b'0' + (hundredths % 100 % 10) as u8;
    let digits = match &fraction {
        b"00" => "",
        [_, b'0'] => std::str::from_utf8(&fraction[..1]).unwrap_or(""),
        _ => std::str::from_utf8(&fraction).unwrap_or(""),
    };
    let _ = write!(out, "{}", hundredths / 100);
    if !digits.is_empty() {
        out.push('.');
        out.push_str(digits);
    }
}

/// What the line for a decision on an exception above the cap is about. `account` is the account
/// the guard runs as and `command` the command line the decision is about (as the prompt shows it),
/// so that the record says what was approved and for whom; `method` is how the user was asked
/// (`hello`, `desktop` or `console`), `timeout` the deadline the run starts with, in seconds, and
/// `started_by` the nearest process that started the guard, as the prompt shows it (empty when it
/// is not known). `requested` is the cap asked for, in seconds. `attempt` is the number of the
/// attempt (the attempts made before it when the run was refused before one was shown: 0 for a
/// prompt that was never shown).
pub(crate) struct ExceptionRecord<'a> {
    pub unix_ms: u64,
    pub pid: u32,
    pub image: &'a str,
    pub account: &'a str,
    pub command: &'a str,
    pub method: &'a str,
    pub timeout: f64,
    pub started_by: &'a str,
    pub requested: f64,
    pub approved: bool,
    pub attempt: u32,
    pub reason: &'a str,
}

/// The line for the decision on an exception above the cap, taken at the start of the run:
/// `exception-approved` once the user has approved the run (by Windows Hello or by the password),
/// `exception-refused` for an attempt that failed or a run that cannot be asked. `hours` is the
/// cap in hours (rounded up). Nothing the user typed is in it: not the password, and not a name.
pub(crate) fn exception_line(record: &ExceptionRecord) -> String {
    // Assembled in its one buffer, as every sibling line builder in
    // this file already is: the time, the six escaped fields, the two
    // second figures and the hours figure were each built as their
    // own string first (a dozen allocations, counting the formatted
    // string inside each `escape`) and then copied into the line by
    // its format, on the path of every exception decision — each
    // prompt attempt and each refusal before one.
    use std::fmt::Write as _;
    let mut out = String::with_capacity(192 + record.image.len() + record.account.len()
        + record.command.len() + record.started_by.len() + record.reason.len());
    out.push_str("PROCESS_TREE_GUARD_EXTENSION: time=");
    iso8601_into(record.unix_ms, &mut out);
    let _ = write!(out, " pid={} image=\"", record.pid);
    escape_into(record.image, &mut out);
    out.push_str("\" account=\"");
    escape_into(record.account, &mut out);
    out.push_str("\" command=\"");
    escape_into(record.command, &mut out);
    out.push_str("\" method=\"");
    escape_into(record.method, &mut out);
    out.push_str("\" timeout=");
    log_seconds_into(record.timeout, &mut out);
    out.push_str(" started_by=\"");
    escape_into(record.started_by, &mut out);
    out.push_str("\" requested=");
    log_seconds_into(record.requested, &mut out);
    out.push_str(" hours=");
    hours_figure_into(record.requested, &mut out);
    out.push_str(if record.approved { " granted=0 decision=exception-approved attempt=" } else { " granted=0 decision=exception-refused attempt=" });
    let _ = write!(out, "{} reason=\"", record.attempt);
    escape_into(record.reason, &mut out);
    out.push('"');
    out
}

/// The text of the notice for a run whose exception was approved and which then could not start.
pub(crate) fn not_started_text(error: &GuardError) -> String {
    format!("the approved run did not start: {}", error.message())
}

/// The time now, as the lines of the log write it.
pub(crate) fn now_ms() -> u64 {
    unix_ms_now()
}

/// The notice that lines were left out of a copy of the log because it could not keep up.
pub(in crate::extend) fn dropped_line(unix_ms: u64, count: usize) -> String {
    // One buffer, as in `released_line` above.
    use std::fmt::Write as _;
    let mut out = String::with_capacity(128);
    out.push_str("PROCESS_TREE_GUARD_EXTENSION: time=");
    iso8601_into(unix_ms, &mut out);
    let _ = write!(out, " notice=\"{} log lines were not written because this copy of the log could not keep up\"", count);
    out
}

/// The notice that refusal lines were left out, so that the lines before them stay in the log.
pub(in crate::extend) fn withheld_line(unix_ms: u64, count: u32) -> String {
    // One buffer, as in `released_line` above.
    use std::fmt::Write as _;
    let mut out = String::with_capacity(128);
    out.push_str("PROCESS_TREE_GUARD_EXTENSION: time=");
    iso8601_into(unix_ms, &mut out);
    let _ = write!(out, " notice=\"{} refusal lines were not written, to keep the earlier lines in the log\"", count);
    out
}

pub(in crate::extend) fn relock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
