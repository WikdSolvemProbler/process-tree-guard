// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Extendable runs: the channel, the holds, the deadline and the log.
//!
//! A guard started with `--extendable` serves a private named pipe. A process in
//! the guard's job can ask on it for its deadline to be held up, for a number of
//! seconds no greater than the cap of the run. The guard checks, before it
//! replies, that the process is alive, is in its job and started before the
//! guard learned of the connection, and keeps a hold on that process by
//! handle, never by process identifier. The deadline of the run at every
//! moment is the latest of its own deadline and the holds of live processes;
//! it is recalculated under one lock on every grant and every
//! exit. The clock is `GetTickCount64`, which counts
//! time asleep and does not follow changes of the wall clock. A request that
//! arrives after the deadline is refused, whether or not the supervisor has yet
//! noticed that the deadline passed.
//!
//! The pipe has four instances, each served by a thread of its own, started one
//! per connection as connections arrive, and what one
//! connection can cost is bounded: who the client is, and its share of the rate, are
//! settled before a byte of its message is waited for, and the waits are short. The
//! pipe has no outgoing buffer, so a reply is written only as far as the client reads
//! it; a grant whose reply is not taken in time is withdrawn, so that the client and
//! the run agree about whether a hold exists.
//!
//! The log is written by threads of its own, from bounded queues, so that a
//! destination that stops taking lines (a standard error that nobody reads) can
//! never hold up the answers or the deadline. The audit file is the one
//! destination that can stop the grants: while it cannot take the lines of the
//! grants, nothing is granted (see `AuditFile`). With no audit file, standard error is the
//! only record, and a grant stands only once its line has been written there (see
//! `Log::record`).
//!
//! The same module is the client: `request` asks the enclosing run, and
//! `register_start` is the request a nested guard makes when it starts. A library
//! call made inside a run is a nested guard in a process that goes on, so when it
//! returns it makes a `release`, which ends the hold its start made.

use crate::auth::Exception;
use crate::engine::format_seconds;
use crate::sys::windows::{
    GetSystemTimePreciseAsFileTime, GetTickCount64, PIPE_READMODE_MESSAGE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE,
};
use crate::GuardError;
use std::ffi::c_void;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::collections::VecDeque;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod client;
mod codec;
mod descriptor;
mod log;
mod serve;
mod server;
mod state;

pub(crate) use client::*;
pub(crate) use codec::*;
pub(crate) use descriptor::*;
pub(crate) use log::*;
use serve::*;
pub(crate) use server::*;
pub(crate) use state::*;

/// The environment variable that names the channel of the enclosing extendable run.
pub const CHANNEL_VARIABLE: &str = "PROCESS_TREE_GUARD_CHANNEL";

/// The largest extension a run grants unless `--max-extension` says otherwise: six hours.
pub const DEFAULT_MAX_EXTENSION: f64 = 21600.0;

const PIPE_PREFIX: &str = "\\\\.\\pipe\\process-tree-guard-";
const REQUEST_MAGIC: u64 = u64::from_le_bytes(*b"PTGERQ01");
const REPLY_MAGIC: u64 = u64::from_le_bytes(*b"PTGERP01");
const REQUEST_SIZE: usize = 24;
const REPLY_SIZE: usize = 40;
/// Requests allowed per process in one window, and the window. Every client outside
/// the job shares one such allowance.
const RATE_LIMIT: usize = 12;
const RATE_WINDOW_MS: u64 = 10_000;
/// Live processes the rate limit tells apart at once; when they are all live, a new one is refused.
/// A process that has exited is forgotten as soon as the table is full.
const MAX_RATE_KEYS: usize = 1024;
/// After a pass over the table that forgot nobody (every process in it is live), the next one waits this long.
const FORGET_MS: u64 = 250;
/// Live holds at once.
const MAX_HOLDS: usize = 256;
/// Live holds one process can have at once, all its tokens together, so that one process
/// cannot take the table from the rest.
const MAX_HOLDS_PER_PROCESS: usize = 8;
/// Refusal lines written in one window, over all processes; the rest are counted in one line.
const REFUSAL_LINES: usize = 24;
/// Instances of the pipe, all made by the guard before the command starts, each served by a
/// thread of its own, so that a client that stalls one cannot keep the others waiting.
const INSTANCES: u32 = 4;
/// How long a client that the run will answer has to send its request, and then to take the
/// reply. A client that sends nothing is dropped when the first runs out. The pipe has no
/// outgoing buffer, so the write of a reply completes only when the client reads it: a reply
/// that is not taken in time is cancelled, and the hold it granted is withdrawn.
const READ_MS: u32 = 250;
const TAKE_MS: u32 = 500;
/// How long a grant waits for its line to be written to standard error when that is the only record.
pub(crate) const RECORD_MS: u64 = 500;
/// The same for a client that is refused whatever it sends (the rate limit, a run that has
/// ended): the request is read only to put its numbers in the log.
const REFUSED_READ_MS: u32 = 25;
const REFUSED_TAKE_MS: u32 = 250;
/// Tests only: the slice the tests pin through `wait_slice`. No production
/// wait uses it; production plain passes cap at 250 ms in `engine/wait.rs`.
#[cfg(test)]
pub(crate) const BACKSTOP_MS: u64 = 100;
/// The backstop for a wait that carries the wake event: grants and job
/// exits signal the event and wake the wait at once, so the interval is only
/// the lost-message backstop and can be longer than a plain pass's
/// 250 ms without moving the precision of a grant.
pub(crate) const BACKSTOP_WAKE_MS: u64 = 1000;
/// The log file is rotated once it would pass this size.
const LOG_LIMIT: u64 = 1 << 20;
/// The most a file that cannot be turned over keeps free below twice its limit before the run
/// stops granting: room for the lines that are queued when it happens.
const MAX_RESERVE: u64 = 64 * 1024;
/// How long after a turn over of the audit file failed it is tried again.
const TURN_OVER_RETRY_MS: u64 = 1000;
/// How long the audit file waits for a line before it tries again to write the ones it holds.
const RETRY_MS: u64 = 500;
/// Lines the audit file holds back, to try again, while it cannot be written; past that,
/// lines are left out, and counted.
const HELD_LINES: usize = LOG_QUEUE;
/// Lines each copy of the log can have waiting; the next ones are dropped, and counted.
const LOG_QUEUE: usize = 256;
/// How long the end of a run waits for a copy of the log to take its last lines.
pub(crate) const LOG_FLUSH_MS: u64 = 500;
/// How long a client waits for the channel.
const CONNECT_MS: u64 = 5000;
/// How long a client waits for the reply once its request is sent.
const REPLY_MS: u32 = 5000;
/// The mode of the channel: messages, not bytes, and no client from another machine.
const CHANNEL_PIPE_MODE: u32 = PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_REJECT_REMOTE_CLIENTS;
/// The size of the outgoing buffer of the channel: none, so that a write completes when the
/// client reads it, and the guard knows whether a reply was taken.
const OUT_BUFFER: u32 = 0;
/// Rights on the pipe for its one user: read and write data, read attributes,
/// read the access list and synchronize; not the right to make another instance.
const PIPE_RIGHTS: &str = "0x12019b";
/// The same with the right to make another instance, which the guard needs for the length of
/// the time it makes the instances.
const PIPE_RIGHTS_WHILE_MAKING: &str = "0x12019f";
/// What a client asks for: exactly those rights, since the generic write right
/// includes the one the list leaves out.
const CLIENT_ACCESS: u32 = 0x0010_0183;

/// A grant: how long the deadline was held, and until when (wall clock time).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grant {
    /// The seconds granted, from the moment of the grant.
    pub seconds: f64,
    /// The time the hold ends, as the guard's clock read it at the grant.
    pub until: SystemTime,
}

// ------------------------------------------------------------------ clocks, text

pub(crate) fn tick() -> u64 {
    unsafe { GetTickCount64() }
}

/// The wall clock to the precision the system has, in the units of a process's
/// creation time. The coarse clock can read earlier than a process that began a
/// moment ago, so it is never used to compare with one.
fn filetime_now() -> u64 {
    let mut time = 0u64;
    unsafe { GetSystemTimePreciseAsFileTime(&mut time) };
    time
}

fn unix_ms_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Seconds as whole milliseconds, rounded up: a deadline is never
/// shortened by its conversion. The one home of the conversion: the
/// engine's fixed deadline and every extension deadline and hold
/// are converted here, so the rounding rule cannot drift between
/// the paths that set a deadline.
pub(crate) fn millis(seconds: f64) -> u64 {
    (seconds * 1000.0).ceil() as u64
}

/// Universal time as `2026-10-20T15:04:05.123Z`, from milliseconds since 1970.
/// The string form is compiled for the tests only, which hold text
/// against it; production writes into its buffers with `iso8601_into`.
#[cfg(test)]
pub(crate) fn iso8601(unix_ms: u64) -> String {
    let mut out = String::with_capacity(24);
    iso8601_into(unix_ms, &mut out);
    out
}

/// `iso8601` written into a buffer the caller already holds, in the
/// same text: a log line that formats its time as its own string
/// first allocates for 24 characters it then copies into the line.
pub(crate) fn iso8601_into(unix_ms: u64, out: &mut String) {
    use std::fmt::Write as _;
    let days = (unix_ms / 86_400_000) as i64;
    let in_day = unix_ms % 86_400_000;
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 { shifted + 3 } else { shifted - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    let _ = write!(out, "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", year, month, day,
        in_day / 3_600_000, in_day / 60_000 % 60, in_day / 1000 % 60, in_day % 1000);
}

/// The characters of the Unicode category Cf (format): the ones that change how the text
/// around them is shown without being shown themselves, such as the bidirectional
/// controls, the zero width characters and the byte order mark.
pub(crate) fn is_format(c: char) -> bool {
    matches!(c as u32,
        0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x0890..=0x0891 | 0x08E2 | 0x180E
        | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F | 0xFEFF | 0xFFF9..=0xFFFB
        | 0x110BD | 0x110CD | 0x13430..=0x1343F | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F)
}

/// Text for a log value in double quotes: the quote, the backslash and every
/// control, line separating or format character are escaped, so no value can end a
/// line or start a field of its own, and none can change how the rest of the line is
/// shown, as a right-to-left override does (Common Weakness Enumeration, CWE-117 and CWE-451).
/// The string form is compiled for the tests only, which hold text
/// against it; production writes into its buffers with `escape_into`.
#[cfg(test)]
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_into(text, &mut out);
    out
}

/// `escape` into a buffer the caller already holds: a log line escapes
/// two of its fields, and building each as its own string first —
/// plus a third, formatted string for every character that is itself
/// escaped — allocated for text the line then copied out of them.
/// Plain text is copied in runs, not character by character: within
/// ASCII the escape decision is a byte question (the quote, the
/// backslash and the C0 controls and DEL), and a non-ASCII character
/// is decoded only at its lead byte, escaping when it is a control
/// (the C1 range), a line or paragraph separator or a format
/// character, and otherwise joining the run untouched.
pub(crate) fn escape_into(text: &str, out: &mut String) {
    use std::fmt::Write as _;
    let bytes = text.as_bytes();
    let mut run_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\\' => {
                out.push_str(&text[run_start..i]);
                out.push('\\');
                out.push(bytes[i] as char);
                i += 1;
                run_start = i;
            }
            0x00..=0x1F | 0x7F => {
                out.push_str(&text[run_start..i]);
                let _ = write!(out, "\\u{{{:x}}}", bytes[i] as u32);
                i += 1;
                run_start = i;
            }
            b if b < 0x80 => i += 1,
            _ => {
                let c = text[i..].chars().next().unwrap();
                if c.is_control() || c == '\u{2028}' || c == '\u{2029}' || is_format(c) {
                    out.push_str(&text[run_start..i]);
                    let _ = write!(out, "\\u{{{:x}}}", c as u32);
                    i += c.len_utf8();
                    run_start = i;
                } else {
                    i += c.len_utf8();
                }
            }
        }
    }
    out.push_str(&text[run_start..]);
}

// ----------------------------------------------------------------------- the wire

/// Whose deadline a request holds up: the process that asks, or its parent (the
/// `extend` program, which ends at once, asks for the process that ran it). A release
/// is the end of a hold the process asked for: a library call makes one when it returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Target {
    Caller = 1,
    Parent = 2,
    Release = 3,
}

/// Why a request was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Refusal {
    OverCap = 1,
    NotInJob,
    RateLimited,
    Malformed,
    TooManyHolds,
    Ended,
    InvalidSeconds,
    Unverified,
    Crowded,
    NoHold,
    TooManyHoldsHere,
    AuditBlocked,
    HolderNotInJob,
}

impl Refusal {
    fn from_code(code: u64) -> Option<Refusal> {
        [Refusal::OverCap, Refusal::NotInJob, Refusal::RateLimited, Refusal::Malformed, Refusal::TooManyHolds,
            Refusal::Ended, Refusal::InvalidSeconds, Refusal::Unverified, Refusal::Crowded, Refusal::NoHold,
            Refusal::TooManyHoldsHere, Refusal::AuditBlocked, Refusal::HolderNotInJob].into_iter().find(|r| *r as u64 == code)
    }

    /// The reason as the log and the refused process read it. `cap` is the run's cap in seconds.
    /// Every reason but the over-cap one is a fixed text, so it is
    /// borrowed, not built: describing a refusal allocated a string
    /// for text that stands in the binary, on the path of every
    /// refused request — the flood path of the extension server,
    /// whose refusal line materialises from this text, and of every
    /// refused client, whose error formats it a second time.
    pub(crate) fn describe(self, cap: f64) -> std::borrow::Cow<'static, str> {
        match self {
            Refusal::OverCap => std::borrow::Cow::Owned(format!("the request is over the cap of {} seconds", format_seconds(cap))),
            Refusal::NotInJob => "the process is not in the job of this run".into(),
            Refusal::RateLimited => "too many requests from this process; try again later".into(),
            Refusal::Malformed => "the request is malformed".into(),
            Refusal::TooManyHolds => "too many processes hold the deadline".into(),
            Refusal::Ended => "the run has ended".into(),
            Refusal::InvalidSeconds => std::borrow::Cow::Owned(format!("the seconds must be greater than zero and at most {}", format_seconds(crate::MAX_SECONDS))),
            Refusal::Unverified => "the process could not be verified".into(),
            Refusal::Crowded => "too many processes are asking at once; try again later".into(),
            Refusal::NoHold => "the process holds no deadline to end".into(),
            Refusal::TooManyHoldsHere => "this process holds too many deadlines".into(),
            Refusal::AuditBlocked => "the audit file (or standard error, when there is no audit file) cannot take lines, so nothing is granted until it can".into(),
            Refusal::HolderNotInJob => "the process to hold is not in the job of this run".into(),
        }
    }
}


#[cfg(test)]
mod tests;
