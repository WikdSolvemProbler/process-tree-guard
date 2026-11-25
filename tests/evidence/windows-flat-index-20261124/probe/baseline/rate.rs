// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The rate limit of an extendable run: the share of requests each process is allowed in a window.

use super::*;

/// A process and the creation time that tells it from one that later takes its identifier.
pub(in crate::extend) type RateKey = (u32, u64);

/// The key every client outside the job shares: to the rate limit they are one party,
/// so they cannot take the place of the processes that are in the job.
pub(in crate::extend) const OUTSIDE: RateKey = (0, 0);

/// What the rate limit makes of a request.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::extend) enum Admit {
    Allowed,
    /// Over the limit, and the first in its window: logged as a line of its own.
    First,
    /// Over the limit, after the first: counted, and logged with the rest of its window of refusals as one line.
    Counted,
    /// Not counted at all, because the table holds as many live processes as it can.
    Full,
}

/// Whether the process of a rate key is the one it was and is still running.
pub(in crate::extend) fn process_alive(key: RateKey) -> bool {
    Process::open(key.0).is_some_and(|process| process.created == key.1 && process.alive())
}

pub(in crate::extend) struct Rate {
    pub(in crate::extend) key: RateKey,
    pub(in crate::extend) times: Vec<u64>,
    /// Requests refused for the rate since `refusing_since`, the first of them included.
    pub(in crate::extend) refused: u32,
    /// When the first of them was refused: the window of the refusals runs from here for `RATE_WINDOW_MS`.
    pub(in crate::extend) refusing_since: u64,
    pub(in crate::extend) pid: u32,
    pub(in crate::extend) image: String,
}

/// The refusals of one process in one window of refusals that were not logged one by one.
pub(in crate::extend) struct Summary {
    pub(in crate::extend) pid: u32,
    pub(in crate::extend) image: String,
    pub(in crate::extend) count: u32,
}

impl Rate {
    /// Close the window of refusals once it has run its course (at once when `all`): the
    /// ones after the first, which were counted and not logged, become a summary, and the
    /// next refusal opens a window of its own and is logged as the first of it. A process
    /// that keeps asking is never idle, so this cannot wait for it to stop.
    pub(in crate::extend) fn close_refusals(&mut self, now: u64, all: bool, summaries: &mut Vec<Summary>) {
        if self.refused > 0 && (all || now.saturating_sub(self.refusing_since) >= RATE_WINDOW_MS) {
            if self.refused > 1 {
                summaries.push(Summary { pid: self.pid, image: self.image.clone(), count: self.refused - 1 });
            }
            self.refused = 0;
        }
    }
}

#[derive(Default)]
pub(in crate::extend) struct Limiter {
    pub(in crate::extend) rates: Vec<Rate>,
    /// The position of each rate in `rates`, by its key. The table
    /// itself stays in insertion order — the sweep and the retaining
    /// passes walk it in that order, and the summaries they produce
    /// keep it — and this index is what a key is looked up through:
    /// one hash lookup instead of a scan of the whole table, on the
    /// path of every connection. It is kept exactly with the table
    /// at every change to it: a push records the new entry's
    /// position before the entry is appended, and a retaining pass
    /// rewrites the position of each entry it keeps and drops the
    /// entry it removes, inside its own pass over the table, so no
    /// path pays a second pass for the upkeep. No other path moves
    /// a rate or changes its key: the refusal-closing pass and the
    /// giving-back passes change a rate's counts in place, and the
    /// repair of a pushed entry in `arrive` changes the last entry's
    /// image alone. The final sweep of a shutdown empties the table
    /// through the retaining pass, and the index empties with it.
    pub(in crate::extend) index: std::collections::HashMap<RateKey, usize>,
    pub(in crate::extend) summaries: Vec<Summary>,
    /// When a pass over a full table last forgot nobody.
    pub(in crate::extend) unproductive: Option<u64>,
    /// The earliest moment at which a sweep could change anything: the
    /// minimum, over the rates, of the close of a refusal window
    /// (`refusing_since + RATE_WINDOW_MS`, for a rate that is refusing)
    /// and of a last-request window (`times.last() + RATE_WINDOW_MS`,
    /// for a rate that has a request). It is maintained on mutation:
    /// a mutation that adds a window, or lowers the last request of
    /// one, lowers it to match; a mutation that removes a window or
    /// raises a last request leaves it, so it stays at or before every
    /// real close, never after one; and a sweep that runs recomputes
    /// it exactly from the rates that remain (see `refresh_next_close`,
    /// which the retaining passes also use). `None` when no rate has
    /// a window that can close.
    pub(in crate::extend) next_close: Option<u64>,
}

impl Limiter {
    /// The moment the windows of `rate` can first close: the earlier
    /// of its refusal window and its last-request window, as recorded
    /// for `next_close`.
    fn close_of(rate: &Rate) -> Option<u64> {
        let refusal = if rate.refused > 0 { Some(rate.refusing_since.saturating_add(RATE_WINDOW_MS)) } else { None };
        let last = rate.times.last().map(|last| last.saturating_add(RATE_WINDOW_MS));
        match (refusal, last) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (close, None) => close,
            (None, close) => close,
        }
    }

    /// Record a window's close, lowering `next_close` to it when it
    /// is the earliest so far.
    fn note_close(&mut self, close: Option<u64>) {
        if let Some(close) = close {
            self.next_close = Some(self.next_close.map_or(close, |earliest| earliest.min(close)));
        }
    }

    /// Recompute `next_close` exactly from the rates as they stand.
    fn refresh_next_close(&mut self) {
        self.next_close = self.rates.iter().filter_map(Self::close_of).min();
    }

    /// Count a request of `key` (the process `who`) at `now`. A process is held to
    /// `RATE_LIMIT` requests in any window. A process that has exited cannot ask again,
    /// so it is forgotten when the table is full (`alive` says whether a process of the
    /// table is still running); a live process is never forgotten to make room for
    /// another, so a process that starts many cannot reset the share of the rest.
    pub(in crate::extend) fn admit(&mut self, key: RateKey, who: (u32, &str), now: u64, alive: &dyn Fn(RateKey) -> bool) -> Admit {
        self.sweep(now, false);
        if self.rates.len() >= MAX_RATE_KEYS && !self.index.contains_key(&key) {
            self.forget_the_dead(now, alive);
        }
        let Some(position) = self.index.get(&key).copied() else {
            if self.rates.len() >= MAX_RATE_KEYS {
                return Admit::Full;
            }
            let (pid, image) = if key == OUTSIDE { (0, "outside the job") } else { who };
            self.index.insert(key, self.rates.len());
            self.rates.push(Rate { key, times: vec![now], refused: 0, refusing_since: 0, pid, image: image.to_string() });
            self.note_close(Self::close_of(self.rates.last().expect("the rate just pushed")));
            return Admit::Allowed;
        };
        let rate = &mut self.rates[position];
        rate.times.retain(|time| now.saturating_sub(*time) < RATE_WINDOW_MS);
        let outcome = if rate.times.len() >= RATE_LIMIT {
            // The sweep above has closed a window of refusals that ran its course, so the first
            // refusal after it is the first of a new window.
            rate.refused += 1;
            if rate.refused == 1 {
                rate.refusing_since = now;
                Admit::First
            } else {
                Admit::Counted
            }
        } else {
            rate.times.push(now);
            Admit::Allowed
        };
        let close = Self::close_of(rate);
        self.note_close(close);
        outcome
    }

    /// Give back the latest request of `key`, which was counted when its connection arrived.
    pub(in crate::extend) fn refund(&mut self, key: RateKey) {
        let close = if let Some(position) = self.index.get(&key).copied() {
            let rate = &mut self.rates[position];
            rate.times.pop();
            Self::close_of(rate)
        } else {
            None
        };
        // The last request of the rate is now an earlier one, whose
        // window closes earlier: record that close.
        self.note_close(close);
    }

    /// Take back the latest refusal for the rate of `key`, which was counted when its connection arrived.
    pub(in crate::extend) fn unrefuse(&mut self, key: RateKey) {
        if let Some(position) = self.index.get(&key).copied() {
            let rate = &mut self.rates[position];
            rate.refused = rate.refused.saturating_sub(1);
        }
    }

    /// Keep the entries `keep` accepts, with a summary of the refusals that were counted
    /// and not logged for each one that goes.
    pub(in crate::extend) fn retain(&mut self, keep: impl Fn(&Rate) -> bool) {
        let rates = &mut self.rates;
        let summaries = &mut self.summaries;
        let index = &mut self.index;
        // The position each kept entry holds once the pass has
        // compacted the table: the pass visits the entries in table
        // order and the kept ones take the places in that order, so
        // counting the kept ones as they are visited gives each one
        // its place, and a removed one's entry goes with it.
        let mut position = 0usize;
        rates.retain(|rate| {
            let kept = keep(rate);
            if !kept && rate.refused > 1 {
                summaries.push(Summary { pid: rate.pid, image: rate.image.clone(), count: rate.refused - 1 });
            }
            if kept {
                index.insert(rate.key, position);
                position += 1;
            } else {
                index.remove(&rate.key);
            }
            kept
        });
        self.refresh_next_close();
    }

    /// Close the windows of refusals that have run their course, whether or not the process
    /// is still asking, and forget the processes whose window has passed (all of them when `all`).
    pub(in crate::extend) fn sweep(&mut self, now: u64, all: bool) {
        // Before the earliest close no window can have run its course
        // and no last request can have left its window, so the pass
        // below would close nothing and forget nobody: it is skipped
        // whole (the scan of the table and the moving of every live
        // rate with it). At or after the earliest close the pass runs
        // exactly as it always has. A sweep of all (`all`) never skips:
        // it closes and forgets regardless of the time.
        if !all {
            if let Some(next_close) = self.next_close {
                if now < next_close {
                    return;
                }
            }
        }
        for rate in self.rates.iter_mut() {
            rate.close_refusals(now, all, &mut self.summaries);
        }
        self.retain(|rate| !all && rate.times.last().is_some_and(|last| now.saturating_sub(*last) < RATE_WINDOW_MS));
    }

    /// Forget the processes that have exited, whose window has not passed. A pass that
    /// forgets nobody is not repeated for `FORGET_MS`, so a table full of live processes
    /// cannot make every request pay for a look at all of them.
    pub(in crate::extend) fn forget_the_dead(&mut self, now: u64, alive: &dyn Fn(RateKey) -> bool) {
        if self.unproductive.is_some_and(|at| now.saturating_sub(at) < FORGET_MS) {
            return;
        }
        let before = self.rates.len();
        self.retain(|rate| rate.key == OUTSIDE || alive(rate.key));
        self.unproductive = if self.rates.len() == before { Some(now) } else { None };
    }
}
