// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The trace of a run: the stage lines kept until the tree has ended, and the run's sample of whether the trace is on.

use super::*;

static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

// The trace lines not yet written.
thread_local! { static TRACE_LINES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) }; }

/// With PROCESS_TREE_GUARD_TRACE set, or PROCESS_GUARD_TRACE when the new name is not set, note the milliseconds since start at each stage. The lines are
/// kept and written to standard error by `flush_trace` once the tree has ended, never while it
/// runs: a write to a standard error that nobody reads waits, and a wait must never be what
/// keeps the tree past its deadline.
pub(crate) fn trace_enabled_with(active: bool, cache: &std::sync::OnceLock<bool>, live: impl FnOnce() -> bool) -> bool {
    if active { *cache.get_or_init(live) } else { live() }
}

pub(crate) fn trace_enabled() -> bool {
    if let Some(sampled) = TRACE_SAMPLE.with(|sample| sample.get()) {
        return sampled;
    }
    trace_enabled_live()
}

fn trace_enabled_live() -> bool {
    static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    trace_enabled_with(crate::cli_process::active(), &TRACE, || std::env::var_os("PROCESS_TREE_GUARD_TRACE").is_some() || std::env::var_os("PROCESS_GUARD_TRACE").is_some())
}

thread_local! { static TRACE_SAMPLE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) }; }

/// One run's sample of whether the trace is on, taken when the run
/// starts and restored when it ends. `trace` is called at every
/// stage of a run, and on a returning library call — where the live
/// environment is read, never a process cache — each of those calls
/// otherwise pays an environment lookup (a system call and an
/// allocation) only to learn again what the run's first stage
/// already learned. The sample is taken fresh for every run, so a
/// change between runs is honoured exactly as before; a change
/// during one run no longer toggles its trace halfway through.
pub(crate) struct TraceSample {
    previous: Option<bool>,
}

impl TraceSample {
    pub(super) fn begin() -> Self {
        let sampled = trace_enabled_live();
        TraceSample { previous: TRACE_SAMPLE.with(|sample| sample.replace(Some(sampled))) }
    }
}

impl Drop for TraceSample {
    fn drop(&mut self) {
        TRACE_SAMPLE.with(|sample| sample.set(self.previous));
    }
}

pub fn trace(stage: &str) {
    if trace_enabled() {
        let start = STARTED.get_or_init(std::time::Instant::now);
        let line = format!("PROCESS_GUARD_TRACE {:>8.2} ms {}", start.elapsed().as_secs_f64() * 1000.0, stage);
        TRACE_LINES.with(|lines| lines.borrow_mut().push(line));
    }
}

/// Capture trace lines and the notice after cleanup, in their original order.
/// Returning callers retrieve them; the explicit CLI adapter writes them.
pub(super) fn collected_trace(started: bool, notice: Option<String>, take: impl FnOnce() -> Vec<String>) -> Option<String> {
    // STARTED is initialized before every TRACE_LINES push. Until then, no
    // thread can have a queued trace line, so leave its TLS slot untouched.
    if !started { return notice; }
    let mut lines = take();
    lines.extend(notice);
    (!lines.is_empty()).then(|| lines.join("\n"))
}

pub(crate) fn flush_trace(notice: Option<String>) {
    if let Some(text) = collected_trace(STARTED.get().is_some(), notice,
        || TRACE_LINES.with(|lines| std::mem::take(&mut *lines.borrow_mut()))) {
        notify(text);
    }
}

/// Captures deferred trace output when the run returns, once the tree is gone.
pub(super) struct FlushTrace;

impl Drop for FlushTrace {
    fn drop(&mut self) {
        flush_trace(None);
    }
}
