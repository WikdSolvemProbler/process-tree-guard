// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Admission: the arrival of a connection and the decision that answers its request.

use super::*;


use crate::sys::windows::{Handle, PeekNamedPipe, SetEvent};
/// Why a connection is refused whatever it sends.
pub(in crate::extend) enum Shut {
    /// A refusal for this reason, a line of its own as far as the budget allows.
    Refusal(Refusal),
    /// A refusal for the rate after the first of its window: counted, and logged with the rest as one line.
    Counted,
}

/// The file name of the program a client runs, as an arrival carries
/// it: resolved for every connection whose decision reads it, and
/// unresolved — no image query paid, no string built — for a
/// connection the rate limiter counted, whose decision drops it
/// unread unless the request is the release that is let past the
/// count (see `evaluate`, which resolves it there from the client it
/// still holds, once, before the first reader on that path).
pub(in crate::extend) struct ArrivalImage(pub(in crate::extend) Option<String>);

impl ArrivalImage {
    /// The name itself, for a reader that comes after the resolution
    /// point: a name that was never resolved reads as the name an
    /// unreadable client has always carried.
    pub(in crate::extend) fn as_str(&self) -> &str {
        self.0.as_deref().unwrap_or("unknown")
    }

    /// Resolve the name from the client the connection opened, when
    /// it is not resolved yet: one image query of that handle, or the
    /// unreadable client's name when there is no client to query.
    pub(in crate::extend) fn resolve(&mut self, client: &Option<Process>) {
        if self.0.is_none() {
            self.0 = Some(client.as_ref().map_or_else(|| "unknown".to_string(), Process::image));
        }
    }

    /// The name as the owned string the refusal and withdrawal
    /// records carry: the resolved name moves out, with no copy.
    pub(in crate::extend) fn into_string(self) -> String {
        self.0.unwrap_or_else(|| "unknown".to_string())
    }
}

impl From<&str> for ArrivalImage {
    fn from(name: &str) -> ArrivalImage {
        ArrivalImage(Some(name.to_string()))
    }
}

impl From<String> for ArrivalImage {
    fn from(name: String) -> ArrivalImage {
        ArrivalImage(Some(name))
    }
}

/// What is known of a connection before any of its message is read.
pub(in crate::extend) struct Arrival {
    pub(in crate::extend) id: u32,
    pub(in crate::extend) image: ArrivalImage,
    pub(in crate::extend) client: Option<Process>,
    pub(in crate::extend) in_job: bool,
    pub(in crate::extend) shut: Option<Shut>,
    /// The client has closed its end of the pipe: set once its message is read, before the decision.
    pub(in crate::extend) gone: bool,
}

/// True when the client has closed its end of the pipe, or the end can no longer be asked
/// about. A client waits for its reply with its end open, so an end that is closed is a
/// client that has gone, whatever process now has its identifier.
pub(in crate::extend) fn client_gone(pipe: Handle) -> bool {
    let (mut available, mut left) = (0u32, 0u32);
    unsafe { PeekNamedPipe(pipe, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut available, &mut left) == 0 }
}

/// Open the client, find out whether it is in the job, and count the connection against
/// its share of the rate: a client that is not in the job has no share of its own, all
/// of them draw on one, so they cannot crowd out the processes in it.
pub(in crate::extend) fn arrive(shared: &Shared, id: u32) -> Arrival {
    let client = Process::open(id);
    let job = shared.job as Handle;
    let in_job = client.as_ref().is_some_and(|client| client.in_job(job));
    let key = match &client {
        Some(client) if in_job => (client.id, client.created),
        _ => OUTSIDE,
    };
    // The client's image is not queried yet: which connections read
    // it is settled by the admission below, and a counted connection
    // — the flood's steady state — reads it nowhere on its ordinary
    // path, so it must not pay the query and the string for text that
    // is dropped unread.
    let mut image: Option<String> = None;
    let shut = {
        let mut state = shared.lock();
        let now = tick();
        if state.ended(now, Some(job)) {
            Some(Shut::Refusal(Refusal::Ended))
        } else {
            // `admit` reads its `who` only when it pushes a new rate
            // entry, and the OUTSIDE key substitutes its own fixed
            // text regardless, so the admission is given an empty
            // placeholder: a connection whose entry already stands
            // never has it read, and a counted connection is exactly
            // one of those. A push is the one outcome that reads it
            // (a new entry is `Allowed`, and only that outcome is
            // checked for one below): the entry it made is repaired
            // at once, still under this lock, before another
            // connection can read it — a real image is never empty
            // (an unreadable client is "unknown"), so the empty text
            // marks the push, and the repair resolves the image
            // here, under the lock, on that first connection of a
            // process only, and carries it out as the arrival's own.
            let alive = state.alive;
            let outcome = state.limiter.admit(key, (id, ""), now, &alive);
            if outcome == Admit::Allowed {
                // A rate this admission pushed is the table's last
                // entry: `admit` appends a new rate and mutates
                // nothing after it, and no rate with an empty image
                // can already stand in the table — the placeholder
                // above is the only empty image a push can carry
                // (every other push's image is a resolved name, the
                // OUTSIDE key's fixed text included), and a pushed
                // placeholder is repaired here, under this same lock,
                // before any other admission can run. So the entry
                // to repair, when this admission pushed one, is the
                // last one when it carries this key and the empty
                // image, and scanning the whole table for it — up to
                // MAX_RATE_KEYS entries, under the state lock, on
                // every allowed connection, pushed or not — bought
                // nothing the last entry does not say in one look.
                if let Some(rate) = state.limiter.rates.last_mut().filter(|rate| rate.key == key && rate.image.is_empty()) {
                    let resolved = client.as_ref().map_or_else(|| "unknown".to_string(), Process::image);
                    rate.image = resolved.clone();
                    image = Some(resolved);
                }
            }
            match outcome {
                Admit::Allowed => None,
                Admit::First => Some(Shut::Refusal(Refusal::RateLimited)),
                Admit::Counted => Some(Shut::Counted),
                Admit::Full => Some(Shut::Refusal(Refusal::Crowded)),
            }
        }
    };
    // Every outcome but the counted one is read by the decision, so
    // its image is resolved here when the admission did not resolve
    // it; a counted connection carries it unresolved, and `evaluate`
    // resolves it there only for the release let past the count.
    if !matches!(shut, Some(Shut::Counted)) && image.is_none() {
        image = Some(client.as_ref().map_or_else(|| "unknown".to_string(), Process::image));
    }
    Arrival { id, image: ArrivalImage(image), client, in_job, shut, gone: false }
}

/// A refusal's line, not yet built: everything the line is built
/// from, and nothing built. A refusal in a flood — past the budget
/// of refusal lines in its window — is dropped whole by `decide`,
/// and building its line first (the reason text, the escaping, the
/// formatted line) was work paid for every refused request only to
/// be thrown away. The descriptor is built instead — the arrival's
/// image is moved into it, not cloned — and `decide` materialises
/// the line from it only once the budget has allowed the line to be
/// kept.
pub(in crate::extend) struct RefusedLine {
    pub(in crate::extend) pid: u32,
    pub(in crate::extend) image: String,
    pub(in crate::extend) requested: Option<f64>,
    pub(in crate::extend) refusal: Refusal,
    pub(in crate::extend) cap: f64,
}

impl RefusedLine {
    /// Build the line, exactly as `refused` built it before the line
    /// was made lazy: same fields, same reason text. Its wall-clock
    /// time is sampled here, at its materialisation, not where
    /// `refused` constructed this descriptor: `materialise` is
    /// called within the same decision, after only the state lock,
    /// a `tick()` and the budget's in-memory arithmetic, so at the
    /// log's millisecond resolution a kept line's time is unchanged
    /// except across a millisecond boundary during that
    /// sub-millisecond arithmetic — and a line the budget withholds
    /// is never built and pays no wall-clock query at all.
    fn materialise(&self) -> String {
        let reason = self.refusal.describe(self.cap);
        log_line(unix_ms_now(), &Entry { pid: self.pid, image: &self.image, requested: self.requested, granted: None, reason: &reason })
    }
}

/// What the log is told of a request.
pub(in crate::extend) enum Line {
    /// A grant: always a line of its own, and what it takes to back out of it.
    Granted(String, Withdraw),
    /// The end of a hold at its process's request: always a line of its own.
    Released(String),
    /// A refusal: a line of its own, as far as the budget of refusal lines allows.
    Refused(RefusedLine),
    /// A refusal for the rate after the first of its window: counted, and logged with the rest as one line.
    Counted,
}

/// A refusal: the reply, and the line to log — as a descriptor, for
/// `decide` to materialise only if the budget allows it.
pub(in crate::extend) fn refused(id: u32, image: String, requested: Option<f64>, refusal: Refusal, cap: f64) -> (Reply, Line) {
    (Reply::Refused { refusal, cap }, Line::Refused(RefusedLine { pid: id, image, requested, refusal, cap }))
}

/// Verify the process and decide the request, and say what the log is to be told. The
/// checks that read the process take no lock; every decision about the deadline, the
/// rate and the hold is made under the one lock that also guards the deadline.
pub(in crate::extend) fn decide(shared: &Shared, arrival: Arrival, connected_at: u64, message: &Message) -> (Reply, Option<String>, Option<Withdraw>) {
    let (reply, line) = evaluate(shared, arrival, connected_at, message);
    match line {
        Line::Granted(line, back) => (reply, Some(line), Some(back)),
        Line::Released(line) => (reply, Some(line), None),
        Line::Counted => (reply, None, None),
        // The budget is asked first, and the refusal's line is
        // materialised from its descriptor only when the budget has
        // already allowed it: a line the budget withholds is never built.
        // The answer is taken into a value first, as in `handle_client`:
        // a guard formed in the condition would hold the state lock
        // across the materialisation (its wall-clock sample and its
        // whole line build) on every refused request.
        Line::Refused(line) => {
            let allowed = shared.lock().budget.allow(tick());
            (reply, if allowed { Some(line.materialise()) } else { None }, None)
        }
    }
}

pub(in crate::extend) fn evaluate(shared: &Shared, arrival: Arrival, connected_at: u64, message: &Message) -> (Reply, Line) {
    let Arrival { id, mut image, client, in_job, shut, gone } = arrival;
    let job = shared.job as Handle;
    let cap = shared.cap_seconds;
    let request = match message {
        Message::Bytes(bytes) => decode_request(bytes.as_slice()),
        _ => None,
    };
    let requested = request.map(|(_, _, seconds)| seconds);
    // Every refusal below returns at once, so each one calls
    // `refused` directly and moves the arrival's image into the
    // refusal's descriptor (`image.into_string()`): a closure over
    // the image could only lend it, and lending it forced the
    // descriptor to clone it on every refused request — the flood
    // path, where refusals are the load.
    let counted = || (Reply::Refused { refusal: Refusal::RateLimited, cap }, Line::Counted);
    // Before anything is believed of the client: the run may be over, and the client is
    // held to its share of the rate (settled when it connected). A release is let past the
    // rate: it only ends a hold that a counted grant made, and one that finds none is refused
    // below with the rest of its share used, so the share still bounds what a process can do.
    let release = matches!(request, Some((Target::Release, _, _)));
    let limited = matches!(shut, Some(Shut::Counted | Shut::Refusal(Refusal::RateLimited)));
    match shut {
        Some(Shut::Refusal(refusal)) if !(release && limited) => return refused(id, image.into_string(), requested, refusal, cap),
        Some(Shut::Counted) if !release => return counted(),
        _ => {}
    }
    let Some((target, token, seconds)) = request else { return refused(id, image.into_string(), requested, Refusal::Malformed, cap) };
    // A counted connection reaches this point only with a release,
    // the one request let past the count, and its image arrives
    // unresolved: this is its resolution point, before the first
    // reader on the release's path — the unverified and not-in-job
    // refusals below, the released line, and the refusal of a
    // release that finds no hold — and it resolves at most once,
    // from the client this connection still holds (an arrival whose
    // image is already resolved, and a counted non-release
    // connection, which returned above, pay nothing here).
    image.resolve(&client);
    let Some(client) = client else { return refused(id, image.into_string(), requested, Refusal::Unverified, cap) };
    // The process must be the one that connected: alive now, and started before the guard
    // learned of the connection, which a process that took over a reused identifier later did
    // not; and the end of the pipe it connected with must still be open, since a client waits
    // for its reply. One that has closed it is gone, and whatever process now has its identifier
    // (one that began between the connection and the guard's look at it) is not the client.
    if gone || !client.started_before(connected_at) {
        return refused(id, image.into_string(), requested, Refusal::Unverified, cap);
    }
    if !in_job {
        return refused(id, image.into_string(), requested, Refusal::NotInJob, cap);
    }
    // The extend program ends at once, so it asks for the process that ran it. That
    // parent is read from the verified client's own record and must be older than it
    // (a reused identifier is newer), alive and in the job like any other. When it is
    // a copy of Git Bash made to run the program, the shell that made the copy is held.
    if target == Target::Release {
        let removed = shared.lock().drop_hold(client.id, client.created, token, limited);
        if !removed {
            // Over its share, the refusal is the one for the rate; within it, there was no hold.
            return match shut {
                Some(Shut::Counted) => counted(),
                Some(Shut::Refusal(refusal)) => refused(id, image.into_string(), requested, refusal, cap),
                None => refused(id, image.into_string(), requested, Refusal::NoHold, cap),
            };
        }
        unsafe { SetEvent(shared.wake as Handle) };
        let line = released_line(unix_ms_now(), id, image.as_str(), "the hold ended with the library call that asked for it");
        return (Reply::Granted { seconds: 0.0, until_unix_ms: 0, held: 0 }, Line::Released(line));
    }
    // A grant that the audit file could not record would be a deadline moved by a request that
    // is not in the record, so nothing is granted while the file cannot take lines.
    if shared.log.blocked() {
        return refused(id, image.into_string(), requested, Refusal::AuditBlocked, cap);
    }
    let mut holder_path: Option<String> = None;
    let holder = match target {
        Target::Caller | Target::Release => None,
        Target::Parent => match client.parent_id().and_then(Process::open) {
            Some(parent) if parent.created < client.created && parent.alive() => {
                let (parent, path) = enclosing_shell(parent, job);
                if !parent.in_job(job) {
                    return refused(id, image.into_string(), requested, Refusal::HolderNotInJob, cap);
                }
                holder_path = path;
                Some(parent)
            }
            _ => return refused(id, image.into_string(), requested, Refusal::Unverified, cap),
        },
    };
    // The held process's name comes from the image path the shell walk
    // already read for it, when it read one: querying that path again
    // here paid a second `QueryFullProcessImageNameW` and a second
    // full-path string for a name the walk had in hand. A walk that
    // read no path takes the query it always took.
    let held = holder.as_ref().map(|parent| (parent.id, match holder_path.take() {
        Some(path) => image_file_name(path),
        None => parent.image(),
    }));
    let parent_rate = holder.as_ref().zip(held.as_ref()).map(|(parent, (pid, name))| ((parent.id, parent.created), *pid, name.as_str()));
    let process = holder.unwrap_or(client);
    let (id_held, created_held) = (process.id, process.created);
    // The grant's own pass over the holds reads the deadline the
    // hold had before it, and it comes back with the grant's
    // success: a separate search of the same table for it, under
    // the same lock, paid a second full scan per granted request.
    let (outcome, now) = {
        let mut state = shared.lock();
        let now = tick();
        (state.grant(now, Some(job), seconds, process, token, parent_rate), now)
    };
    let previous = match outcome {
        Ok(previous) => previous,
        Err(no) => {
            return match no {
                Refused { refusal: Refusal::RateLimited, counted: true } => counted(),
                Refused { refusal, .. } => refused(id, image.into_string(), requested, refusal, cap),
            };
        }
    };
    unsafe { SetEvent(shared.wake as Handle) };
    let until = now + millis(seconds);
    // The reason of a caller grant is a fixed text: formatting it
    // built the string (and an empty second string for the absent
    // suffix) on every grant, for text the log line holds as well
    // from the literal. Only a parent grant, whose reason names the
    // held process, formats — and formats once, into its one string.
    let reason_owned;
    let reason: &str = match &held {
        Some((pid, name)) => {
            reason_owned = format!("within the cap; held for the parent, process {} ({})", pid, name);
            &reason_owned
        }
        None => "within the cap",
    };
    // One wall-clock sample serves both the line's time and the
    // reply's deadline time: the two are the same instant's reading,
    // and sampling the clock twice for them bought a second reading
    // that could only disagree with the first by the time between.
    let unix_ms = unix_ms_now();
    let line = log_line(unix_ms, &Entry { pid: id, image: image.as_str(), requested: Some(seconds), granted: Some(seconds), reason });
    let back = Withdraw { id: id_held, created: created_held, token, until, previous, pid: id, image: image.into_string(), seconds };
    (Reply::Granted { seconds, until_unix_ms: unix_ms + (until - now), held: held.map_or(0, |(pid, _)| pid) }, Line::Granted(line, back))
}
