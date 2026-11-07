// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Opt-in, source-pinned self-measurement. No I/O or workers during supervision.

use crate::sys::windows::{

    GetCurrentProcess, GetCurrentProcessId, GetLastError, GetTickCount64, QueryPerformanceCounter,

    QueryPerformanceFrequency, SetLastError, WAIT_TIMEOUT,

};
use std::cell::Cell;
use std::rc::Rc;

pub(crate) const PREFIX: &str = "PROCESS_GUARD_NATIVE_TIMING_V1";

fn nonce_from_entries(entries: impl IntoIterator<Item = (String, String)>, trace: bool) -> Option<String> {
    if !trace { return None; }
    let mut entries = entries.into_iter().filter(|(name, _)| name.eq_ignore_ascii_case("PROCESS_TREE_GUARD_TIMING_NONCE") || name.eq_ignore_ascii_case("PROCESS_GUARD_TIMING_NONCE"));
    let (_, nonce) = entries.next()?;
    if entries.next().is_some() || nonce.len() != 32 || !nonce.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    Some(nonce)
}

fn preserving_last_error<T>(read: impl FnOnce() -> T) -> T {
    let error = unsafe { GetLastError() };
    let value = read();
    unsafe { SetLastError(error) };
    value
}

fn qpc() -> Option<i64> {
    preserving_last_error(|| {
        let mut value = 0;
        (unsafe { QueryPerformanceCounter(&mut value) } != 0 && value >= 0).then_some(value)
    })
}

fn encoded_deadline(g: u64, duration: u64) -> (u64, bool) {
    (g.saturating_add(duration), g.checked_add(duration).is_none())
}

#[derive(Default)]
struct Timeline { last: Cell<Option<i64>>, invalid: Cell<bool> }

impl Timeline {
    fn accept(&self, value: Option<i64>) -> Option<i64> {
        self.invalid.set(self.invalid.get() || match value {
            Some(value) => value < 0 || self.last.get().is_some_and(|previous| value < previous),
            None => true,
        });
        if value.is_some() { self.last.set(value); }
        value
    }

    fn read(&self) -> Option<i64> { self.accept(qpc()) }
}

#[derive(Clone, Copy, Debug)]
struct TickRead { before: Option<i64>, g: u64, after: Option<i64> }

fn sample_tick_with(mut counter: impl FnMut() -> Option<i64>, tick: impl FnOnce() -> u64) -> TickRead {
    let before = counter();
    let g = tick();
    let after = counter();
    TickRead { before, g, after }
}

#[derive(Default)]
pub(crate) struct TickEvidence {
    timeline: Option<Rc<Timeline>>,
    assignment: Option<TickRead>,
    last: Option<TickRead>,
    below: Option<TickRead>,
    at: Option<TickRead>,
    deadline: Option<u64>,
    invalid: bool,
}

impl TickEvidence {
    fn accept(&mut self, sample: TickRead, assignment: bool) -> u64 {
        let ordered = matches!((sample.before, sample.after), (Some(a), Some(b)) if 0 <= a && a <= b);
        let previous = self.last.or(self.assignment);
        self.invalid |= !ordered || previous.is_some_and(|old| {
            old.g > sample.g || !matches!((old.after, sample.before), (Some(a), Some(b)) if a <= b)
        });
        if assignment { self.assignment = Some(sample); } else {
            self.last = Some(sample);
            if let Some(deadline) = self.deadline {
                if sample.g < deadline { self.below = Some(sample); }
                else if self.at.is_none() { self.at = Some(sample); }
            }
        }
        sample.g
    }

    fn read_with(&mut self, assignment: bool, tick: impl FnOnce() -> u64, mut counter: impl FnMut() -> Option<i64>) -> u64 {
        let sample = sample_tick_with(|| {
            let value = counter();
            self.timeline.as_ref().map_or(value, |timeline| timeline.accept(value))
        }, tick);
        self.accept(sample, assignment)
    }

    fn assignment(&mut self) -> u64 { self.read_with(true, || unsafe { GetTickCount64() }, qpc) }

    pub(crate) fn remaining_read(&mut self) -> u64 { self.read_with(false, || unsafe { GetTickCount64() }, qpc) }

}

#[derive(Default)]
pub(crate) struct DecisionEvidence {
    timeline: Option<Rc<Timeline>>,
    before: Option<i64>,
    wait_status: Option<u32>,
    invalid: bool,
}

impl DecisionEvidence {
    fn wait_with(&mut self, counter: impl FnOnce() -> Option<i64>, wait: impl FnOnce() -> u32) -> u32 {
        let value = counter();
        let before = self.timeline.as_ref().map_or(value, |timeline| timeline.accept(value));
        self.invalid |= before.is_none() || matches!((self.before, before), (Some(a), Some(b)) if a > b);
        self.before = before;
        let status = wait();
        self.wait_status = Some(status);
        status
    }

    pub(crate) fn wait(&mut self, wait: impl FnOnce() -> u32) -> u32 { self.wait_with(qpc, wait) }

    #[cfg(test)]
    pub(crate) fn recorded_wait_status(&self) -> Option<u32> { self.wait_status }
}

pub(crate) struct FixedTiming {
    timeline: Rc<Timeline>,
    nonce: String,
    pid: u32,
    creation: Option<u64>,
    frequency: Option<i64>,
    duration: u64,
    deadline: Option<u64>,
    pub(crate) ticks: TickEvidence,
    pub(crate) decision: DecisionEvidence,
    after: Option<i64>,
    outcome: &'static str,
    status: Option<u32>,
    error_stage: Option<&'static str>,
    invalid: bool,
}

impl FixedTiming {
    pub(crate) fn from_environment(extendable: bool, duration: u64) -> Option<Self> {
        if extendable { return None; }
        let trace = crate::engine::trace_enabled();
        if !trace { return None; }
        // Direct lookups for the nonce, chained new-first:
        // `PROCESS_TREE_GUARD_TIMING_NONCE`, then
        // `PROCESS_GUARD_TIMING_NONCE` when the new name is not set.
        // Materializing the whole environment to find a single name
        // would tax the run the trace measures. The chain pays one
        // extra failed `var_os` probe per traced fixed run when the
        // new name is unset; the nonce is read only when the trace is
        // on, once per fixed run. A duplicated name in the environment
        // block cannot be rejected through a single lookup; within a
        // spelling the first entry wins, as `var_os` defines, and the
        // new spelling wins over the legacy when both are set. The
        // nonce pins a measurement, not a security decision.
        let entry = std::env::var_os("PROCESS_TREE_GUARD_TIMING_NONCE")
            .map(|value| ("PROCESS_TREE_GUARD_TIMING_NONCE".to_owned(), value.to_string_lossy().into_owned()))
            .or_else(|| std::env::var_os("PROCESS_GUARD_TIMING_NONCE")
                .map(|value| ("PROCESS_GUARD_TIMING_NONCE".to_owned(), value.to_string_lossy().into_owned())));
        let nonce = nonce_from_entries(entry, trace)?;
        let (frequency, creation, pid) = preserving_last_error(|| {
            let mut frequency = 0;
            let frequency = (unsafe { QueryPerformanceFrequency(&mut frequency) } != 0 && frequency > 0).then_some(frequency);
            (frequency, crate::engine::started_at(unsafe { GetCurrentProcess() }), unsafe { GetCurrentProcessId() })
        });
        let timeline = Rc::new(Timeline::default());
        let before = timeline.read();
        Some(Self {
            nonce, pid, creation, frequency, duration, deadline: None,
            ticks: TickEvidence { timeline: Some(timeline.clone()), ..TickEvidence::default() },
            decision: DecisionEvidence { before, timeline: Some(timeline.clone()), ..DecisionEvidence::default() }, timeline,
            after: None, outcome: "error", status: None, error_stage: None,
            invalid: frequency.is_none() || creation.is_none() || before.is_none() || duration == 0 || pid == 0,
        })
    }

    pub(crate) fn assign(&mut self) -> u64 {
        let g = self.ticks.assignment();
        let (deadline, saturated) = encoded_deadline(g, self.duration);
        self.invalid |= saturated;
        self.deadline = Some(deadline);
        self.ticks.deadline = Some(deadline);
        if !saturated && self.duration > 0 { self.ticks.below = self.ticks.assignment; }
        self.invalid |= !matches!((self.decision.before, self.ticks.assignment.and_then(|s| s.before)), (Some(a), Some(b)) if a <= b);
        deadline
    }

    /// Called after the selected Result is known, before any cleanup or optional output.
    pub(crate) fn finish(&mut self, result: &Result<u32, crate::GuardError>) {
        self.after = self.timeline.read();
        self.status = result.as_ref().ok().copied();
        self.outcome = match (result, self.decision.wait_status) {
            (Err(_), _) => "error",
            (Ok(_), Some(WAIT_TIMEOUT)) => "timeout",
            (Ok(_), Some(0)) => "child",
            (Ok(_), Some(_)) => "owner",
            _ => "error",
        };
        self.error_stage = result.is_err().then_some(if self.deadline.is_some() { "launch_or_supervision" } else { "before_assignment" });
        self.invalid |= self.after.is_none() || !matches!((self.decision.before, self.after), (Some(a), Some(b)) if a <= b);
        if let Some(sample) = self.ticks.last.or(self.ticks.assignment) {
            self.invalid |= !matches!((sample.after, self.after), (Some(a), Some(b)) if a <= b);
            if self.decision.wait_status.is_some() {
                self.invalid |= !matches!((sample.after, self.decision.before), (Some(a), Some(b)) if a <= b);
            }
        }
    }

    /// Cleanup changes the final classification, never the supervision endpoint.
    /// This failure-only update reads no clock and performs no native operation.
    pub(crate) fn invalidate_cleanup(&mut self) {
        self.outcome = "error";
        self.status = None;
        self.error_stage = Some("cleanup");
        self.invalid = true;
    }

    fn record(&self) -> String {
        fn number<T: std::fmt::Display>(value: Option<T>) -> String { value.map_or_else(|| "null".into(), |value| value.to_string()) }
        fn string(value: Option<&str>) -> String { value.map_or_else(|| "null".into(), |value| format!("\"{}\"", value)) }
        let a = self.ticks.assignment;
        let last = self.ticks.last;
        let below = self.ticks.below;
        let at = self.ticks.at;
        format!(concat!("{} {{\"version\":1,\"nonce\":\"{}\",\"sut_pid\":{},\"sut_creation_filetime\":{},\"mode\":\"fixed\",",
            "\"qpc_frequency\":{},\"assignment_qpc_before\":{},\"assignment_g_ms\":{},\"assignment_qpc_after\":{},",
            "\"duration_ms\":{},\"deadline_g_ms\":{},\"last_deadline_read_qpc_before\":{},\"last_deadline_read_g_ms\":{},\"last_deadline_read_qpc_after\":{},",
            "\"last_below_deadline_qpc_before\":{},\"last_below_deadline_g_ms\":{},\"last_below_deadline_qpc_after\":{},",
            "\"first_at_deadline_qpc_before\":{},\"first_at_deadline_g_ms\":{},\"first_at_deadline_qpc_after\":{},",
            "\"outcome\":\"{}\",\"outcome_qpc_before\":{},\"outcome_qpc_after\":{},\"wait_status_u32\":{},\"wait_origin\":\"{}\",\"status_u32\":{},",
            "\"error_stage\":{},\"assurance\":\"SOURCE_PINNED_SELF_MEASUREMENT\",\"win32_error_u32\":null,\"valid\":{}}}"),
            PREFIX, self.nonce, self.pid, number(self.creation), number(self.frequency), number(a.and_then(|s| s.before)), number(a.map(|s| s.g)), number(a.and_then(|s| s.after)),
            self.duration, number(self.deadline), number(last.and_then(|s| s.before)), number(last.map(|s| s.g)), number(last.and_then(|s| s.after)),
            number(below.and_then(|s| s.before)), number(below.map(|s| s.g)), number(below.and_then(|s| s.after)),
            number(at.and_then(|s| s.before)), number(at.map(|s| s.g)), number(at.and_then(|s| s.after)),
            self.outcome, number(self.decision.before), number(self.after), number(self.decision.wait_status), if self.decision.wait_status.is_some() { "native" } else { "none" },
            number(self.status), string(self.error_stage), !(self.invalid || self.ticks.invalid || self.decision.invalid || self.timeline.invalid.get()))
    }

    /// Queue owned output only; returning callers retain their no-writer contract.
    pub(crate) fn emit(self) { crate::diagnostics::stderr_owned(self.record()); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_requires_trace_exact_lowercase_hex_and_one_case_insensitive_entry() {
        let entry = || ("PROCESS_GUARD_TIMING_NONCE".into(), "0123456789abcdef0123456789abcdef".into());
        assert!(nonce_from_entries([entry()], true).is_some());
        assert!(nonce_from_entries([("PROCESS_TREE_GUARD_TIMING_NONCE".into(), "0123456789abcdef0123456789abcdef".into())], true).is_some());
        assert!(nonce_from_entries([("process_tree_guard_timing_nonce".into(), "0123456789abcdef0123456789abcdef".into())], true).is_some());
        assert!(nonce_from_entries([entry()], false).is_none());
        assert!(nonce_from_entries([entry(), entry()], true).is_none());
        assert!(nonce_from_entries([entry(), ("process_guard_timing_nonce".into(), entry().1)], true).is_none());
        for value in ["", "0123456789ABCDEF0123456789ABCDEF", "0123456789abcdef0123456789abcde", "0123456789abcdef0123456789abcdefg"] {
            assert!(nonce_from_entries([("PROCESS_GUARD_TIMING_NONCE".into(), value.into())], true).is_none());
        }
    }

    #[test]
    fn bracket_uses_one_actual_tick_read_in_order_and_keeps_it_on_qpc_failure() {
        let calls = std::cell::RefCell::new(Vec::new());
        let mut q = 9;
        let sample = sample_tick_with(|| { calls.borrow_mut().push("qpc"); q += 1; Some(q) }, || { calls.borrow_mut().push("tick"); 123 });
        assert_eq!(*calls.borrow(), ["qpc", "tick", "qpc"]);
        assert_eq!((sample.before, sample.g, sample.after), (Some(10), 123, Some(11)));
        let failed = sample_tick_with(|| None, || 456);
        let mut evidence = TickEvidence::default();
        assert_eq!(evidence.accept(failed, true), 456);
        assert!(evidence.invalid);
    }

    #[test]
    fn backwards_qpc_or_tick_invalidates_evidence_without_changing_the_clock_value() {
        let sample = |before, g, after| TickRead { before: Some(before), g, after: Some(after) };
        let mut evidence = TickEvidence::default();
        assert_eq!(evidence.accept(sample(10, 100, 11), true), 100);
        assert_eq!(evidence.accept(sample(12, 101, 13), false), 101);
        assert!(!evidence.invalid);
        assert_eq!(evidence.accept(sample(12, 99, 14), false), 99);
        assert!(evidence.invalid);
    }

    fn fixture(status: u32, wait: u32) -> FixedTiming {
        FixedTiming { timeline: Rc::new(Timeline::default()), nonce: "0123456789abcdef0123456789abcdef".into(), pid: 123, creation: Some(u64::MAX), frequency: Some(10_000_000),
            duration: 300, deadline: Some(400), ticks: TickEvidence { assignment: Some(TickRead { before: Some(1), g: 100, after: Some(2) }),
                last: Some(TickRead { before: Some(3), g: 400, after: Some(4) }), ..TickEvidence::default() },
            decision: DecisionEvidence { before: Some(5), wait_status: Some(wait), ..DecisionEvidence::default() }, after: Some(6),
            outcome: if wait == 0 { "child" } else { "timeout" }, status: Some(status), error_stage: None, invalid: false }
    }

    #[test]
    fn record_preserves_full_dword_child124_and_error_nullability() {
        let high = fixture(0xfedcba98, 0).record();
        assert!(high.contains("\"status_u32\":4275878552") && high.contains("\"sut_creation_filetime\":18446744073709551615"));
        let child = fixture(124, 0).record();
        assert!(child.contains("\"outcome\":\"child\"") && child.contains("\"wait_status_u32\":0") && child.contains("\"status_u32\":124"));
        let timeout = fixture(124, WAIT_TIMEOUT).record();
        assert!(timeout.contains("\"outcome\":\"timeout\"") && timeout.contains("\"wait_status_u32\":258"));
        let mut error = fixture(124, 0);
        error.outcome = "error"; error.status = None; error.error_stage = Some("launch_or_supervision");
        assert!(error.record().contains("\"status_u32\":null,\"error_stage\":\"launch_or_supervision\""));
    }
    #[test]
    fn cleanup_failure_invalidates_success_without_moving_supervision_time() {
        let mut timing = fixture(0, 0);
        let endpoint = timing.after;
        let decision = timing.decision.before;
        let native_wait = timing.decision.wait_status;
        timing.invalidate_cleanup();
        assert_eq!(timing.after, endpoint);
        assert_eq!(timing.decision.before, decision);
        assert_eq!(timing.decision.wait_status, native_wait);
        let record = timing.record();
        assert!(record.contains("\"outcome\":\"error\""));
        assert!(record.contains("\"status_u32\":null"));
        assert!(record.contains("\"error_stage\":\"cleanup\""));
        assert!(record.contains("\"valid\":false"));
    }

    #[test]
    fn threshold_interval_keeps_latest_below_and_first_at_from_actual_reads() {
        let read = |before, g, after| TickRead { before: Some(before), g, after: Some(after) };
        let mut ticks = TickEvidence::default();
        ticks.accept(read(1, 100, 2), true);
        ticks.deadline = Some(400);
        ticks.below = ticks.assignment;
        ticks.accept(read(3, 399, 4), false);
        ticks.accept(read(5, 400, 6), false);
        ticks.accept(read(7, 401, 8), false);
        assert_eq!(ticks.below.unwrap().g, 399);
        assert_eq!((ticks.at.unwrap().before, ticks.at.unwrap().g, ticks.at.unwrap().after), (Some(5), 400, Some(6)));
        assert_eq!(ticks.last.unwrap().g, 401);
        assert!(!ticks.invalid);
        assert_eq!(encoded_deadline(u64::MAX - 2, 3), (u64::MAX, true));
        assert_eq!(encoded_deadline(100, 300), (400, false));
    }

    #[test]
    fn optional_native_helpers_restore_pending_core_last_error() {
        unsafe { SetLastError(0x87654321) };
        let _ = qpc();
        assert_eq!(unsafe { GetLastError() }, 0x87654321);
        let result = preserving_last_error(|| { unsafe { SetLastError(17) }; 9 });
        assert_eq!(result, 9);
        assert_eq!(unsafe { GetLastError() }, 0x87654321);
    }

    #[test]
    fn shared_timeline_rejects_backwards_qpc_between_production_tick_and_wait_paths() {
        let timeline = Rc::new(Timeline::default());
        let mut ticks = TickEvidence { timeline: Some(timeline.clone()), ..TickEvidence::default() };
        let mut decision = DecisionEvidence { timeline: Some(timeline.clone()), ..DecisionEvidence::default() };
        timeline.accept(Some(1));
        let mut values = [Some(10), Some(11)].into_iter();
        ticks.read_with(true, || 100, || values.next().unwrap());
        decision.wait_with(|| Some(9), || WAIT_TIMEOUT);
        let mut values = [Some(12), Some(13)].into_iter();
        ticks.read_with(false, || 101, || values.next().unwrap());
        decision.wait_with(|| Some(14), || WAIT_TIMEOUT);
        timeline.accept(Some(15));
        assert!(timeline.invalid.get(), "assignment11 -> wait9 is a backwards native sequence");
        let timeline = Rc::new(Timeline::default());
        let mut ticks = TickEvidence { timeline: Some(timeline.clone()), ..TickEvidence::default() };
        let mut decision = DecisionEvidence { timeline: Some(timeline.clone()), ..DecisionEvidence::default() };
        let mut values = [Some(10), Some(11)].into_iter();
        ticks.read_with(true, || 100, || values.next().unwrap());
        decision.wait_with(|| Some(14), || WAIT_TIMEOUT);
        let mut values = [Some(12), Some(13)].into_iter();
        ticks.read_with(false, || 101, || values.next().unwrap());
        assert!(timeline.invalid.get(), "wait14 -> remaining12 must use the same tracker too");
    }

}
