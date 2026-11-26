// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The state of an extendable run: the processes it opened, the holds it granted, and the rate limit it keeps.

use super::*;

use crate::sys::windows::{Handle};
mod process;
mod rate;

pub(crate) use process::*;
pub(super) use rate::*;

// ------------------------------------------------------------- processes, state


pub(super) struct Hold {
    pub(super) process: Process,
    /// Which hold of the process this is (see `Token`).
    pub(super) token: Token,
    pub(super) until: u64,
    uncertain: bool,
}

impl Hold {
    pub(super) fn new(process: Process, token: Token, until: u64) -> Self {
        Self { process, token, until, uncertain: false }
    }

    fn key(&self) -> HoldKey {
        (self.process.id, self.process.created, self.token)
    }
}

type HoldKey = (u32, u64, Token);

struct PendingGrant {
    key: HoldKey,
    until: u64,
    previous: Option<u64>,
    stage: GrantStage,
}

#[derive(Clone, Copy)]
enum GrantStage {
    Awaiting,
    Permitted,
    Vetoed { _observation: ObservationFault },
}

/// Created only by the successful reservation under State. The private
/// fields keep an exact tuple from becoming a second cancellation owner.
#[derive(Debug)]
pub(super) struct UnsubmittedGrant {
    key: HoldKey,
    until: u64,
    previous: Option<u64>,
}

impl UnsubmittedGrant {
    pub(super) fn until(&self) -> u64 {
        self.until
    }

    pub(super) fn for_client(self, pid: u32, image: String, seconds: f64) -> Withdraw {
        Withdraw { grant: self, pid, image, seconds }
    }

    #[cfg(test)]
    pub(super) fn previous(&self) -> Option<u64> {
        self.previous
    }
}

/// The one reservation and the client as the log names it.
pub(super) struct Withdraw {
    grant: UnsubmittedGrant,
    pub(super) pid: u32,
    pub(super) image: String,
    pub(super) seconds: f64,
}

impl Withdraw {
    pub(super) fn shortens_on_delivery(&self) -> bool {
        self.grant.previous.is_some_and(|until| until > self.grant.until)
    }

    #[cfg(test)]
    pub(super) fn key(&self) -> (u32, u64, Token) {
        self.grant.key
    }

    #[cfg(test)]
    pub(super) fn previous(&self) -> Option<u64> {
        self.grant.previous
    }

    #[cfg(test)]
    pub(super) fn duplicate_for_test(&self, different_until: Option<u64>) -> Self {
        let grant = &self.grant;
        UnsubmittedGrant { key: grant.key, until: different_until.unwrap_or(grant.until), previous: grant.previous }
            .for_client(self.pid, self.image.clone(), self.seconds)
    }
}

/// Owned by the only connection whose exact submission won the seal RMW.
pub(super) struct PermittedGrant(Withdraw);

pub(super) enum GrantPermission {
    Permitted(PermittedGrant),
    Unsubmitted(Withdraw, Refusal),
    Stale(Refusal),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum GrantDisposition {
    Effective,
    AlreadyRetracted,
    Absent,
}

pub(super) struct SettledGrant {
    back: Withdraw,
    pub(super) disposition: GrantDisposition,
}

impl SettledGrant {
    pub(super) fn details(&self) -> &Withdraw {
        &self.back
    }
}


/// How many refusal lines are written in a window, over all processes, so that a flood
/// of refusals cannot push the earlier lines, the grants among them, out of the log.
#[derive(Default)]
pub(super) struct Budget {
    pub(super) written: Vec<u64>,
    pub(super) withheld: u32,
}

impl Budget {
    /// True when one more refusal line may be written now; otherwise it is counted.
    pub(super) fn allow(&mut self, now: u64) -> bool {
        self.written.retain(|time| now.saturating_sub(*time) < RATE_WINDOW_MS);
        if self.written.len() < REFUSAL_LINES {
            self.written.push(now);
            return true;
        }
        self.withheld += 1;
        false
    }

    /// The number of lines withheld, once the window has passed (at once when `all`).
    pub(super) fn overflow(&mut self, now: u64, all: bool) -> Option<u32> {
        // With nothing withheld there is no count to hand back, on
        // any pass: the prune below is then work whose result no
        // reader can observe, because every reader of `written`
        // (`allow`, and this function on a pass that does hold a
        // count) prunes it before reading it. The supervisor asks
        // on every pass, so an idle run paid the scan of the whole
        // written window — up to REFUSAL_LINES stamps — per pass
        // for a None it could have had at the first field.
        if self.withheld == 0 {
            return None;
        }
        self.written.retain(|time| now.saturating_sub(*time) < RATE_WINDOW_MS);
        if self.withheld > 0 && (all || self.written.is_empty()) {
            return Some(std::mem::take(&mut self.withheld));
        }
        None
    }
}

/// A request that was refused: why, and whether it was only counted for the log.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Refused {
    pub(super) refusal: Refusal,
    pub(super) counted: bool,
}

impl From<Refusal> for Refused {
    fn from(refusal: Refusal) -> Refused {
        Refused { refusal, counted: false }
    }
}

pub(super) struct State {
    /// The run's own deadline, from `--timeout`, in ticks.
    pub(super) base: u64,
    pub(super) cap_seconds: f64,
    pub(super) holds: Vec<Hold>,
    /// Reserve each holder and token until its grant is delivered or rolled back.
    /// A reservation can outlive an expired or explicitly released hold.
    pending: Vec<PendingGrant>,
    pub(super) limiter: Limiter,
    pub(super) budget: Budget,
    /// Nothing more is granted: the command has finished, or the channel can no longer be
    /// served. The deadline is not touched by it: the holds already granted stand.
    pub(super) closed: bool,
    /// The deadline has passed. It never goes back.
    pub(super) passed: bool,
    /// The deadline the last `expire` pass computed, from the holds
    /// that pass left standing: the supervisor's pass reads it back
    /// instead of folding the whole hold list a second time in the
    /// same pass. It is set by every `expire` pass that runs (one
    /// that finds the deadline already passed leaves it, and its
    /// readers take the passed branch, which does not read it).
    pub(super) expire_deadline: u64,
    /// The tick of the last completed `release` reap, or none when no
    /// reap has completed yet: a `release` called with that same tick
    /// stands on that reap's answers and skips its own job-list query
    /// and aliveness polls (see `release`). Only a reap that ran in
    /// full sets it — neither the no-holds return nor a skipped call
    /// does — so the first settle at any later tick reaps in full.
    pub(super) release_stamp: Option<u64>,
    // An unknown row can protect existing time without authorizing new time.
    reap_has_uncertain_hold: bool,
    reap_shortened: bool,
    /// Whether a process of the rate table is still running; a field so a test can say.
    pub(super) alive: fn(RateKey) -> bool,
}

/// The latest of the run's own deadline and the given holds.
pub(super) fn latest(base: u64, holds: impl Iterator<Item = u64>) -> u64 {
    holds.fold(base, u64::max)
}

impl State {
    pub(super) fn new(base: u64, cap_seconds: f64) -> State {
        State { base, cap_seconds, holds: Vec::new(), pending: Vec::new(), limiter: Limiter::default(), budget: Budget::default(), closed: false, passed: false, expire_deadline: base, release_stamp: None, reap_has_uncertain_hold: false, reap_shortened: false, alive: process_alive }
    }

    /// The latest of the run's own deadline and the holds of the live processes.
    pub(super) fn deadline(&self) -> u64 {
        latest(self.base, self.holds.iter().map(|hold| hold.until))
    }

    /// Drop the holds that have run out, whose process has exited, or (when there is a
    /// `job` to ask) that are not in it: the backstop for a lost completion message.
    /// Every holder is polled, by its handle, whatever the port reported. Membership
    /// is decided by a complete job process list, nested jobs included. An
    /// unavailable or incomplete list uses the original held process directly. A call at the tick of the last completed reap polls
    /// nobody and asks nothing: it stands on that reap's answers (see the stamp
    /// check in the body). The deadline bookkeeping is not part of the reap and
    /// is not skipped: `expire` folds the hold list on every call, this one
    /// included.
    pub(super) fn release(&mut self, now: u64, job: Option<Handle>) {
        // With no holds there is nothing to reap: skip the job process
        // list query (and its buffer) that every supervisor pass and
        // every grant decision would otherwise pay for an empty table.
        if self.holds.is_empty() {
            return;
        }
        // A reap has already completed at this very tick — a request
        // settles the deadline once when it arrives and again when it
        // is granted, microseconds apart, and each settle used to pay
        // the whole reap. Nothing the request itself did in between
        // can have changed what that reap found: the only changes
        // there can be are a holder exiting or a hold running out,
        // and running out is a function of the tick, which is the
        // same, so the query and the polls are skipped and the holds
        // stand exactly as that reap left them. A holder that exits
        // inside the same tick as the reap can therefore stand until
        // the supervisor's next pass, which the job-exit completion
        // packet wakes at once (and whose wait is in any case bounded
        // by the backstop slice): the same bound that already governs
        // every other lost reap, and no wider — the first pass at a
        // different tick reaps in full, as the stamp is only ever set
        // by a reap that ran.
        if self.release_stamp == Some(now) {
            return;
        }
        // Membership is tested against the list as the query buffer
        // holds it: building the vector form of the list first would
        // allocate and copy it on every pass and every decision. The
        // list is handed over sorted (see
        // `with_job_process_ids_sorted`), so each hold's membership
        // is a binary search of the list, not a scan of it: a reap
        // tests every hold, and the scan form paid holds times
        // members comparisons for answers the search gives in
        // logarithmic steps per hold.
        match job {
            Some(job) => crate::engine::with_job_process_ids_sorted(job, |listed| self.retain_after_census(now, Some(job), listed)),
            None => self.retain_after_census(now, None, None),
        }
        // A completed reap supplies the answers for another settle at this tick.
        self.release_stamp = Some(now);
    }

    pub(super) fn retain_after_census(&mut self, now: u64, job: Option<Handle>, listed: Option<&[usize]>) {
        self.retain_after_census_with(now, job, listed, Process::observe_alive, Process::observe_in_job);
    }

    pub(super) fn retain_after_census_with(
        &mut self, now: u64, job: Option<Handle>, listed: Option<&[usize]>,
        mut poll: impl FnMut(&Process) -> Result<bool, ObservationFault>,
        mut membership: impl FnMut(&Process, Handle) -> Result<bool, ObservationFault>,
    ) {
        let mut any_uncertain = false;
        let mut shortened = false;
        let pending = &mut self.pending;
        self.holds.retain_mut(|hold| {
            // A completed census miss or elapsed bound settles this row
            // without a native poll. The fallback only asks about Job
            // membership after a positive poll on the original handle.
            if hold.until <= now || listed.is_some_and(|ids| ids.binary_search(&(hold.process.id as usize)).is_err()) {
                return false;
            }
            let observation = match poll(&hold.process) {
                Ok(true) if listed.is_none() => job.map_or(Ok(true), |job| membership(&hold.process, job)),
                other => other,
            };
            match observation {
                Ok(live) => {
                    hold.uncertain = false;
                    live
                }
                Err(observation) => {
                    if let Some(entry) = pending.iter_mut().find(|entry| entry.key == hold.key()) {
                        if matches!(entry.stage, GrantStage::Awaiting) {
                            entry.stage = GrantStage::Vetoed { _observation: observation };
                            let Some(previous) = entry.previous.filter(|&until| until > now) else {
                                shortened = true;
                                return false;
                            };
                            shortened |= previous < hold.until;
                            hold.until = previous;
                        }
                    }
                    hold.uncertain = true;
                    any_uncertain = true;
                    true
                }
            }
        });
        self.reap_has_uncertain_hold = any_uncertain;
        self.reap_shortened |= shortened;
    }

    pub(super) fn take_reap_wake(&mut self) -> bool {
        std::mem::take(&mut self.reap_shortened)
    }

    /// Release what no longer holds the run up and note that the deadline has passed.
    /// True when it has. Every decision about a hold is made after this, under the same
    /// lock, so a request that arrives after the deadline is refused even if nobody has
    /// yet noticed that the deadline passed.
    pub(super) fn expire(&mut self, now: u64, job: Option<Handle>) -> bool {
        if !self.passed {
            self.release(now, job);
            // The one fold of the hold list in this pass: the result
            // is kept in `expire_deadline` for the pass's reader (the
            // supervisor), which no longer folds the list again.
            self.expire_deadline = self.deadline();
            if now >= self.expire_deadline {
                self.passed = true;
            }
        }
        self.passed
    }

    /// True when a request now is refused because the run has ended: its deadline has
    /// passed, or nothing more is granted.
    pub(super) fn ended(&mut self, now: u64, job: Option<Handle>) -> bool {
        self.expire(now, job) || self.closed
    }

    /// Only the base and still-observed delivered time can admit a new grant.
    /// Provisional time can keep the supervisor alive for its own handoff.
    fn require_delivered_deadline(&self, now: u64) -> Result<(), Refusal> {
        if now < self.base {
            return Ok(());
        }
        let mut uncertain = false;
        for hold in &self.holds {
            let delivered_until = match self.pending.iter().find(|pending| pending.key == hold.key()) {
                Some(pending) => match pending.stage {
                    GrantStage::Vetoed { .. } => pending.previous.filter(|&previous| hold.until == previous),
                    _ => pending.previous.filter(|&previous| hold.until == pending.until.max(previous)),
                },
                None => Some(hold.until),
            };
            if delivered_until.is_some_and(|until| now < until) {
                if !hold.uncertain {
                    return Ok(());
                }
                uncertain = true;
            }
        }
        Err(if uncertain { Refusal::Unverified } else { Refusal::Ended })
    }

    /// Install an already delivered grant for tests of the run state and supervisor.
    #[cfg(test)]
    pub(super) fn grant(&mut self, now: u64, job: Option<Handle>, seconds: f64, process: Process, token: Token, parent: Option<(RateKey, u32, &str)>) -> Result<Option<u64>, Refused> {
        self.apply_grant(now, job, seconds, process, token, parent).map(|(_, previous)| previous)
    }

    /// Reserve a grant while its audit line and reply are in flight. An older
    /// delivered deadline still stands until the client takes a shorter renewal.
    pub(super) fn begin_grant(&mut self, now: u64, job: Option<Handle>, seconds: f64, process: Process, token: Token, parent: Option<(RateKey, u32, &str)>) -> Result<UnsubmittedGrant, Refused> {
        if !process.alive() {
            return Err(Refusal::Unverified.into());
        }
        let key = (process.id, process.created, token);
        let (index, previous) = self.apply_grant(now, job, seconds, process, token, parent)?;
        let until = self.holds[index].until;
        self.pending.push(PendingGrant { key, until, previous, stage: GrantStage::Awaiting });
        if let Some(previous) = previous {
            self.holds[index].until = self.holds[index].until.max(previous);
        }
        Ok(UnsubmittedGrant { key, until, previous })
    }

    /// Apply a grant and return its slot and previous deadline. A holder
    /// targeted by another process has its own share of the rate limit.
    fn apply_grant(&mut self, now: u64, job: Option<Handle>, seconds: f64, process: Process, token: Token, parent: Option<(RateKey, u32, &str)>) -> Result<(usize, Option<u64>), Refused> {
        if self.ended(now, job) {
            return Err(Refusal::Ended.into());
        }
        if !self.pending.is_empty() || self.reap_has_uncertain_hold {
            self.require_delivered_deadline(now)?;
        }
        if let Some((key, pid, image)) = parent {
            match self.limiter.admit(key, (pid, image), now, &self.alive) {
                Admit::Allowed => {}
                Admit::Counted => return Err(Refused { refusal: Refusal::RateLimited, counted: true }),
                Admit::First => return Err(Refusal::RateLimited.into()),
                Admit::Full => return Err(Refusal::Crowded.into()),
            }
        }
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err(Refusal::InvalidSeconds.into());
        }
        if seconds > self.cap_seconds || seconds > crate::MAX_SECONDS {
            return Err(Refusal::OverCap.into());
        }
        let key = (process.id, process.created, token);
        if self.pending.iter().any(|pending| pending.key == key) {
            return Err(Refusal::Crowded.into());
        }
        let until = now + millis(seconds);
        // One pass over the holds settles the three facts the scans
        // below it settled separately: the index of the hold this
        // request renews (same process, same token), how many
        // holds that process already has, and the deadline that
        // hold had before this grant, read here before the renewal
        // write below replaces it. The pass runs under the
        // state lock on every grant, so each further scan was lock
        // time — and supervisor wait — paid again for one table.
        let mut existing_index = None;
        let mut same_process = 0usize;
        for (index, hold) in self.holds.iter().enumerate() {
            if hold.process.id == process.id && hold.process.created == process.created {
                same_process += 1;
                if hold.token == token {
                    existing_index = Some(index);
                }
            }
        }
        let previous = existing_index.map(|index| self.holds[index].until);
        let index = match existing_index {
            Some(index) => {
                if self.holds[index].uncertain {
                    return Err(Refusal::Unverified.into());
                }
                self.holds[index].until = until;
                index
            }
            None => {
                if same_process >= MAX_HOLDS_PER_PROCESS {
                    return Err(Refusal::TooManyHoldsHere.into());
                }
                if self.holds.len() >= MAX_HOLDS {
                    return Err(Refusal::TooManyHolds.into());
                }
                self.holds.push(Hold::new(process, token, until));
                self.holds.len() - 1
            }
        };
        Ok((index, previous))
    }

    /// When the hold `token` of the process `id`, created at `created`, ends, if it has one.
    /// Retained for the unit tests, which read a grant's end back
    /// directly; production code tracks the holds it made itself.
    #[cfg(test)]
    pub(super) fn hold_until(&self, id: u32, created: u64, token: Token) -> Option<u64> {
        self.holds.iter().find(|hold| hold.process.id == id && hold.process.created == created && hold.token == token).map(|hold| hold.until)
    }

    pub(super) fn cancel_unsubmitted(&mut self, back: Withdraw) -> SettledGrant {
        let disposition = match self.pending_grant_index(&back.grant) {
            Some(index) if !matches!(self.pending[index].stage, GrantStage::Permitted) =>
                self.settle_reservation(index, false),
            _ => GrantDisposition::Absent,
        };
        SettledGrant { back, disposition }
    }

    pub(super) fn complete_grant(&mut self, permit: PermittedGrant, delivered: bool) -> SettledGrant {
        let back = permit.0;
        let disposition = match self.pending_grant_index(&back.grant) {
            Some(index) if matches!(self.pending[index].stage, GrantStage::Permitted) =>
                self.settle_reservation(index, delivered),
            _ => GrantDisposition::Absent,
        };
        SettledGrant { back, disposition }
    }

    fn pending_grant_index(&self, grant: &UnsubmittedGrant) -> Option<usize> {
        self.pending.iter().position(|entry| entry.key == grant.key && entry.until == grant.until && entry.previous == grant.previous)
    }

    #[cfg(test)]
    pub(super) fn retraction_fault_for_test(&self, back: &Withdraw) -> Option<ObservationFault> {
        match self.pending[self.pending_grant_index(&back.grant)?].stage {
            GrantStage::Vetoed { _observation: fault } => Some(fault),
            _ => None,
        }
    }

    fn provisional_hold_index(&self, key: HoldKey, until: u64, previous: Option<u64>) -> Option<usize> {
        let effective_until = until.max(previous.unwrap_or(0));
        self.holds.iter().position(|hold| hold.key() == key && hold.until == effective_until)
    }

    // A reservation that lost publication is still in its original
    // awaiting stage under the decision lock. Consume its private owner.
    pub(super) fn cancel_unpublished(&mut self, grant: UnsubmittedGrant) -> GrantDisposition {
        match self.pending_grant_index(&grant) {
            Some(index) if matches!(self.pending[index].stage, GrantStage::Awaiting) =>
                self.settle_reservation(index, false),
            _ => GrantDisposition::Absent,
        }
    }

    fn settle_reservation(&mut self, index: usize, delivered: bool) -> GrantDisposition {
        let entry = self.pending.swap_remove(index);
        if matches!(entry.stage, GrantStage::Vetoed { .. }) {
            return GrantDisposition::AlreadyRetracted;
        }
        let Some(index) = self.provisional_hold_index(entry.key, entry.until, entry.previous) else {
            return GrantDisposition::Absent;
        };
        let deadline = if delivered { Some(entry.until) } else { entry.previous };
        match deadline.filter(|&until| self.release_stamp.is_none_or(|now| until > now)) {
            Some(until) => self.holds[index].until = until,
            None => {
                self.holds.remove(index);
            }
        }
        GrantDisposition::Effective
    }

    /// End the hold `token` of the process `id`, created at `created`, at its own request;
    /// its other holds stand. True when there was one; the request is then not counted against the process's share of the
    /// rate, since it can only follow a grant that was, and only brings the deadline in.
    /// `limited` says that the request came in over the share and was counted as a refusal
    /// when it arrived, which is then taken back; otherwise it was counted as a request, and is given back.
    pub(super) fn drop_hold(&mut self, id: u32, created: u64, token: Token, limited: bool) -> bool {
        // A grant renews a hold of the same (process, token) in place,
        // so at most one hold can match: find and remove that one, as
        // `withdraw` does, instead of walking the whole table with a
        // retaining scan that visits every hold after the match too,
        // under the state lock, on every release a library call makes
        // when it returns.
        let Some(index) = self.holds.iter().position(|hold| hold.process.id == id && hold.process.created == created && hold.token == token) else { return false };
        self.holds.remove(index);
        if limited {
            self.limiter.unrefuse((id, created));
        } else {
            self.limiter.refund((id, created));
        }
        true
    }

    /// The lines for what was counted and not logged one by one: the refusals of each
    /// process in each window of refusals that has run its course (all of them when `all`),
    /// written only as the budget of refusal lines allows, and one line for the lines the
    /// budget withheld. A process that keeps asking has a line for each window while it asks.
    // The eager-clock form, retained for the unit tests, which drive
    // the state with fixed clocks; production passes through
    // `notices_lazy`, so a pass with nothing to report never samples
    // the wall clock.
    #[cfg(test)]
    pub(super) fn notices(&mut self, now: u64, unix_ms: u64, all: bool) -> Vec<String> {
        self.notices_lazy(now, all, &mut || unix_ms)
    }

    /// `notices` with the wall-clock time sampled by `sample`, and only when a
    /// line is actually produced: a pass with nothing to report never samples
    /// it. When a sample is taken it is taken once, so every line of one pass
    /// carries the same time, as with `notices`.
    pub(super) fn notices_lazy(&mut self, now: u64, all: bool, sample: &mut dyn FnMut() -> u64) -> Vec<String> {
        self.limiter.sweep(now, all);
        let mut lines = Vec::new();
        let mut unix_ms: Option<u64> = None;
        for summary in std::mem::take(&mut self.limiter.summaries) {
            if self.budget.allow(now) {
                let reason = format!("too many requests; {} more refused within {} seconds of the first, not logged one by one",
                    summary.count, RATE_WINDOW_MS / 1000);
                if unix_ms.is_none() {
                    unix_ms = Some(sample());
                }
                lines.push(log_line(unix_ms.unwrap(), &Entry { pid: summary.pid, image: &summary.image, requested: None, granted: None, reason: &reason }));
            }
        }
        if let Some(count) = self.budget.overflow(now, all) {
            if unix_ms.is_none() {
                unix_ms = Some(sample());
            }
            lines.push(withheld_line(unix_ms.unwrap(), count));
        }
        lines
    }
}

pub(super) const GRANTS_UNPUBLISHED: u8 = 0;
const GRANTS_USE_STATE: u8 = 1;
const GRANTS_SEALED: u8 = 2;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum BaseRemaining {
    Remaining(u64),
    Expired,
    UseState,
}

pub(super) struct Shared {
    pub(super) state: Mutex<State>,
    pub(super) base_deadline: u64,
    pub(super) prepublication_notices_due: AtomicU64,
    /// The cap of the run, in seconds: fixed when the run starts and never
    /// written again, so a decision reads it here instead of taking the
    /// state lock for it alone. The state's own copy, used under the lock,
    /// is unchanged.
    pub(super) cap_seconds: f64,
    pub(super) log: Log,
    pub(super) job: usize,
    pub(super) wake: usize,
    pub(super) grant_phase: AtomicU8,
    pub(super) stopping: AtomicBool,
    /// Serve threads running: one at the start of the run, and one more for
    /// each instance a connection has called into service, up to INSTANCES.
    pub(super) serving: AtomicUsize,
}

impl Shared {
    pub(super) fn lock(&self) -> MutexGuard<'_, State> {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // Keep the existing end-state visible to every state consumer.
        // Only lock holders mirror the seal, never the supervisor's stop path.
        if self.grants_are_sealed() {
            state.closed = true;
        }
        state
    }

    pub(super) fn seal_grants(&self) {
        self.grant_phase.swap(GRANTS_SEALED, Ordering::AcqRel);
    }

    pub(super) fn grants_are_sealed(&self) -> bool {
        self.grant_phase.load(Ordering::Acquire) == GRANTS_SEALED
    }

    pub(super) fn uses_base_deadline(&self) -> bool {
        self.grant_phase.load(Ordering::Acquire) == GRANTS_UNPUBLISHED
    }

    // Once a reservation has been published, or the monitor needs the old
    // backstop, empty holds alone cannot restore the base-only authority.
    pub(super) fn require_state(&self) -> bool {
        self.grant_phase.compare_exchange(
            GRANTS_UNPUBLISHED, GRANTS_USE_STATE, Ordering::AcqRel, Ordering::Acquire,
        ).is_ok()
    }

    pub(super) fn base_remaining(&self, sample: impl FnOnce() -> u64) -> BaseRemaining {
        match self.grant_phase.load(Ordering::Acquire) {
            GRANTS_SEALED => return BaseRemaining::Expired,
            GRANTS_USE_STATE => return BaseRemaining::UseState,
            _ => {}
        }
        let now = sample();
        if now < self.base_deadline {
            // A concurrent grant can only make this wake early.
            return BaseRemaining::Remaining(self.base_deadline - now);
        }
        match self.grant_phase.compare_exchange(
            GRANTS_UNPUBLISHED, GRANTS_SEALED, Ordering::AcqRel, Ordering::Acquire,
        ) {
            Err(GRANTS_USE_STATE) => BaseRemaining::UseState,
            _ => BaseRemaining::Expired,
        }
    }

    // The reservation is still private under State. A monitor fallback can
    // win the first transition without sealing the run, so take the same
    // permission as any later reservation in that case.
    pub(super) fn publish_grant(&self) -> bool {
        (self.uses_base_deadline() && self.require_state()) || self.authorize_grant()
    }

    pub(super) fn try_service_prepublication_notices(&self, now: u64) -> bool {
        let mut state = match self.state.try_lock() {
            Ok(state) => state,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return false,
        };
        self.service_prepublication_notices(&mut state, now);
        true
    }

    // All consumers call this under State, including the pushes, so first
    // publication cannot pass an earlier batch of notices and the queue is
    // consumed only once. A stop does not wait for this work.
    fn service_prepublication_notices(&self, state: &mut State, now: u64) -> bool {
        if !self.uses_base_deadline() || now < self.prepublication_notices_due.load(Ordering::Relaxed) {
            return false;
        }
        self.prepublication_notices_due.store(now.saturating_add(BACKSTOP_WAKE_MS), Ordering::Relaxed);
        for line in state.notices_lazy(now, false, &mut || unix_ms_now()) {
            self.log.push(&line);
        }
        true
    }

    pub(super) fn tick_after_prepublication_notices(&self, state: &mut State) -> u64 {
        let now = tick();
        if self.service_prepublication_notices(state, now) { tick() } else { now }
    }

    pub(super) fn allow_refusal(&self) -> bool {
        let mut state = self.lock();
        let now = self.tick_after_prepublication_notices(&mut state);
        state.budget.allow(now)
    }

    // A successful read-modify-write is ordered with the one-way seal. A
    // load alone would not authorize publication of the reservation or reply.
    pub(super) fn authorize_grant(&self) -> bool {
        self.grant_phase.compare_exchange(
            GRANTS_USE_STATE, GRANTS_USE_STATE, Ordering::AcqRel, Ordering::Acquire,
        ).is_ok()
    }

    pub(super) fn authorize_grant_reply(&self, back: Withdraw) -> GrantPermission {
        let mut state = self.lock();
        let grant = &back.grant;
        let pending = state.pending_grant_index(grant);
        let (can_cancel, refusal) = match pending.map(|index| (index, state.pending[index].stage)) {
            Some((index, GrantStage::Awaiting)) => {
                if !state.passed && !state.closed &&
                    state.provisional_hold_index(grant.key, grant.until, grant.previous)
                        .is_some_and(|index| !state.holds[index].uncertain)
                {
                    if self.authorize_grant() {
                        state.pending[index].stage = GrantStage::Permitted;
                        return GrantPermission::Permitted(PermittedGrant(back));
                    }
                    return GrantPermission::Unsubmitted(back, Refusal::Ended);
                }
                (true, Refusal::NoHold)
            }
            Some((_, GrantStage::Vetoed { .. })) => (true, Refusal::Unverified),
            _ => (false, Refusal::NoHold),
        };
        // Refusal paths have no RMW. Observe the seal at their decision
        // point, without adding a load to the healthy submission path.
        let refusal = if state.passed || state.closed || self.grants_are_sealed() { Refusal::Ended } else { refusal };
        if can_cancel {
            GrantPermission::Unsubmitted(back, refusal)
        } else {
            GrantPermission::Stale(refusal)
        }
    }
}
