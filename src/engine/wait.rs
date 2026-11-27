// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The waits of a run: the plain and extendable deadline waits over the child and its owners, and the teardown wait.

use super::*;
use std::time::Instant;


use crate::sys::windows::{

    BASIC_ACCOUNTING_CLASS, BasicAccountingInformation, CloseHandle, CreateWaitableTimerExW,

    ERROR_MORE_DATA, GetLastError, GetSystemTimeAsFileTime, GetTickCount64, Handle, IsProcessInJob,

    OpenProcess, PROCESS_ID_LIST_CLASS, PROCESS_QUERY_LIMITED_INFORMATION, QueryInformationJobObject,
    SYNCHRONIZE, SetLastError, SetWaitableTimer,

    Sleep, TIMER_ALL_ACCESS, TerminateJobObject, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,

    WaitForMultipleObjects, os_error,

};
/// Wait for the child or an owner (the indexes of `waits`) while the deadline of an
/// extendable run is recalculated under its lock: whenever a process in the job
/// exits, whenever the backstop interval passes, and so on until the deadline
/// itself passes. Returns what `WaitForMultipleObjects` would, `WAIT_TIMEOUT` for
/// the deadline.
pub(super) fn wait_extendable(extension: &Extension, waits: &[Handle]) -> u32 {
    wait_until(|| extension.remaining(), extension.wake(), waits)
}

/// Keep a fixed deadline in suspend-inclusive interrupt time. System-clock
/// adjustments cannot shorten or extend it; only the wake source uses FILETIME.
pub(super) fn deadline_remaining(deadline: u64, now: u64) -> Option<u64> {
    deadline.checked_sub(now).filter(|remaining| *remaining > 0)
}

pub(super) fn absolute_timer_due(file_time: u64, remaining_ms: u64) -> i64 {
    file_time.saturating_add(remaining_ms.min(10_000).saturating_mul(10_000))
        .min(i64::MAX as u64) as i64
}

/// Sample wall time first. A suspend between samples then makes the absolute
/// wake early, while the later interrupt-time sample still detects expiry.
/// Reversing this order can arm a fresh delay after an already-expired sleep.
pub(super) fn sampled_wait_plan(file_time: impl FnOnce() -> u64, remaining: impl FnOnce() -> Option<u64>) -> Option<(u64, u64)> {
    let file_time = file_time();
    remaining().filter(|milliseconds| *milliseconds > 0).map(|milliseconds| (file_time, milliseconds))
}

/// The deadline is assigned before CreateProcessW, never refreshed on entry.
pub(super) fn fixed_deadline(milliseconds: u64, now: impl FnOnce() -> u64) -> u64 {
    now().saturating_add(milliseconds)
}

pub(super) fn wait_fixed(deadline: u64, mut ticks: Option<&mut TickEvidence>, decision: Option<&mut DecisionEvidence>, waits: &[Handle]) -> u32 {
    wait_with_deadline_observed(|| {
        let now = match ticks.as_mut() {
            Some(ticks) => ticks.remaining_read(),
            None => unsafe { GetTickCount64() },
        };
        deadline_remaining(deadline, now)
    }, None, waits, decision)
}

pub(crate) fn wait_diagnostic_writer(deadline: u64, writer: Handle) {
    let _ = wait_with_deadline(|| deadline_remaining(deadline, unsafe { GetTickCount64() }), None, &[writer]);
}

/// Preserve the extension's existing API and its backstop. Extension::remaining
/// uses GetTickCount64, the same suspend-inclusive clock as a fixed run.
pub(super) fn wait_until(remaining: impl FnMut() -> Option<u64>, wake: Handle, waits: &[Handle]) -> u32 {
    wait_with_deadline(remaining, Some(wake), waits)
}

pub(super) struct DeadlineTimer(pub(super) Handle);

impl Drop for DeadlineTimer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Expiry first polls the command and owners once, preserving exit precedence.
/// Child and owner handles keep their original indices. A positive absolute
/// timer can become signaled during suspend; the finite relative wait cap also
/// rechecks the interrupt deadline after a backward system-clock adjustment.
/// Failure to create or arm the timer refuses supervision instead of silently
/// changing to a suspend-excluding relative deadline.
pub(super) fn wait_with_deadline(remaining: impl FnMut() -> Option<u64>, wake: Option<Handle>, waits: &[Handle]) -> u32 {
    wait_with_deadline_observed(remaining, wake, waits, None)
}

pub(super) fn observed_wait(decision: &mut Option<&mut DecisionEvidence>, wait: impl FnOnce() -> u32) -> u32 {
    match decision.as_mut() { Some(decision) => decision.wait(wait), None => wait() }
}

/// Preserve the failing API's error across owned-handle cleanup. Successful
/// waits never call this helper or pay any extra native error-state calls.
pub(super) fn wait_failed_after_cleanup(cleanup: impl FnOnce()) -> u32 {
    let error = unsafe { GetLastError() };
    cleanup();
    unsafe { SetLastError(error) };
    WAIT_FAILED
}

pub(super) fn owner_window(owners: &[Handle], subset: usize) -> &[Handle] {
    let start = subset * OWNERS_PER_WINDOW;
    &owners[start..(start + OWNERS_PER_WINDOW).min(owners.len())]
}

/// A zero-time native receipt for the original suspended child and every
/// retained owner. Each deep window keeps the child at index zero. A positive
/// status is the actual index in that native window, never a fabricated global
/// owner index. WAIT_FAILED is converted before cleanup can change LastError.
pub(super) fn poll_before_resume(waits: &[Handle], mut decision: Option<&mut DecisionEvidence>) -> Result<Option<u32>, GuardError> {
    if waits.is_empty() || waits.len() > MAX_WATCHED_OWNERS + 1 {
        return Err(GuardError("pre-resume wait list is outside the owner limit".into()));
    }
    let owners = &waits[1..];
    let mut handles = [std::ptr::null_mut(); 1 + OWNERS_PER_WINDOW];
    handles[0] = waits[0];
    for subset in 0..owners.len().div_ceil(OWNERS_PER_WINDOW).max(1) {
        let window = owner_window(owners, subset);
        handles[1..1 + window.len()].copy_from_slice(window);
        let count = 1 + window.len();
        let status = observed_wait(&mut decision, || unsafe {
            WaitForMultipleObjects(count as u32, handles.as_ptr(), 0, 0)
        });
        match status {
            WAIT_TIMEOUT => {}
            WAIT_FAILED => return Err(os_error()),
            n if (n as usize) < count => return Ok(Some(n)),
            other => return Err(GuardError(format!("unexpected pre-resume native wait status: {other}"))),
        }
    }
    Ok(None)
}

/// The deadline is checked after a real zero-poll and before ResumeThread.
/// If expired, one further native zero-poll preserves child/owner precedence.
/// None authorizes a resume now, subject to the later scheduling step.
pub(super) fn pre_resume_with(
    expired: impl FnOnce() -> bool,
    mut poll: impl FnMut() -> Result<Option<u32>, GuardError>,
) -> Result<Option<u32>, GuardError> {
    if let Some(status) = poll()? { return Ok(Some(status)); }
    if expired() {
        return Ok(Some(poll()?.unwrap_or(WAIT_TIMEOUT)));
    }
    Ok(None)
}

pub(super) fn fixed_pre_resume_with(
    deadline: u64,
    now: impl FnOnce() -> u64,
    poll: impl FnMut() -> Result<Option<u32>, GuardError>,
) -> Result<Option<u32>, GuardError> {
    pre_resume_with(|| deadline_remaining(deadline, now()).is_none(), poll)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeOwnerWindowResult {
    pub(super) status: u32,
    pub(super) owner_offset: usize,
    pub(super) owner_count: usize,
}

/// Keep only the selected/latest actual native result. Production allocates no
/// vector, emits no per-poll diagnostic, and never makes a global native index.
pub(super) fn poll_all_owner_windows(
    child: Handle, owners: &[Handle], handles: &mut [Handle; 64],
    mut poll: impl FnMut(&[Handle], usize, usize) -> u32,
) -> Option<NativeOwnerWindowResult> {
    let mut last = None;
    for (subset, window) in owners.chunks(OWNERS_PER_WINDOW).enumerate() {
        handles[0] = child;
        handles[1..1 + window.len()].copy_from_slice(window);
        let owner_offset = subset * OWNERS_PER_WINDOW;
        let status = poll(&handles[..1 + window.len()], owner_offset, window.len());
        let selected = NativeOwnerWindowResult { status, owner_offset, owner_count: window.len() };
        last = Some(selected);
        if status != WAIT_TIMEOUT { return last; }
    }
    // Empty input yields None rather than an invented native WAIT_TIMEOUT.
    last
}

/// The owner-rotation window interval both wait paths share.
const ROTATE_INTERVAL_MS: u32 = 50;

pub(super) fn wait_with_deadline_observed(remaining: impl FnMut() -> Option<u64>, wake: Option<Handle>, waits: &[Handle], decision: Option<&mut DecisionEvidence>) -> u32 {
    wait_with_deadline_using(remaining, wake, waits, decision, |handles, milliseconds| unsafe {
        WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, milliseconds)
    })
}

/// Share the complete production wait loop with focused native-boundary tests.
/// The production callback is the unchanged Win32 WaitAny call above.
pub(super) fn wait_with_deadline_using(
    mut remaining: impl FnMut() -> Option<u64>, wake: Option<Handle>, waits: &[Handle],
    mut decision: Option<&mut DecisionEvidence>, mut wait: impl FnMut(&[Handle], u32) -> u32,
) -> u32 {
    if waits.is_empty() || waits.len() > MAX_WATCHED_OWNERS + 1 {
        return WAIT_FAILED;
    }
    let owners = &waits[1..];
    let rotating = owners.len() > OWNERS_PER_WINDOW;
    let subsets = if rotating { owners.len().div_ceil(OWNERS_PER_WINDOW) } else { 1 };
    let mut subset = 0;
    let mut handles = [std::ptr::null_mut(); 64];
    handles[0] = waits[0];
    let first = owner_window(owners, 0);
    handles[1..1 + first.len()].copy_from_slice(first);
    let mut ordinary_count = 1 + first.len();
    let mut timer = DeadlineTimer(std::ptr::null_mut());
    let mut initial_passes = 0u8;
    // Every blocking pass needs an absolute wake, including the first one:
    // relative Win32 waits stop counting down while Windows 8+ is asleep.
    loop {
        if rotating {
            let window = owner_window(owners, subset);
            handles[1..1 + window.len()].copy_from_slice(window);
            ordinary_count = 1 + window.len();
        }
        // No zero-time poll stands at the head of the pass: the timed wait
        // below returns the same native receipt (a ready handle's index, in
        // the current window — never a manufactured global owner index, so
        // timing v1 still retains an exact native DWORD) and returns it at
        // once, so a poll first would only double the wait calls of every
        // pass. Deadline expiry keeps its own final poll in the branch
        // below, which is where exit-before-expiry precedence is decided.
        // Sample wall time before interrupt time on every pass. A suspend
        // between those samples then expires or advances the existing deadline
        // rather than starting a fresh relative delay after resume.
        let (file_time, milliseconds) = match sampled_wait_plan(|| {
            let mut file_time = 0u64;
            unsafe { GetSystemTimeAsFileTime(&mut file_time) };
            file_time
        }, &mut remaining) {
            Some(plan) => plan,
            None => {
                // For a deep owner set, an expired clock read must check every
                // retained owner before choosing timeout. Ordinary cycles keep
                // their original one-window 50 ms rotation and native call count.
                if rotating {
                    let selected = poll_all_owner_windows(waits[0], owners, &mut handles, |window, _, _| {
                        observed_wait(&mut decision, || wait(window, 0))
                    }).expect("validated deep owner list is nonempty");
                    return if selected.status == WAIT_FAILED {
                        wait_failed_after_cleanup(|| drop(timer))
                    } else { selected.status };
                }
                let status = observed_wait(&mut decision, || wait(&handles[..ordinary_count], 0));
                return if status == WAIT_FAILED { wait_failed_after_cleanup(|| drop(timer)) } else { status };
            }
        };
        if timer.0.is_null() {
            timer.0 = unsafe { CreateWaitableTimerExW(std::ptr::null_mut(), std::ptr::null(), 2, TIMER_ALL_ACCESS) };
            if timer.0.is_null() {
                timer.0 = unsafe { CreateWaitableTimerExW(std::ptr::null_mut(), std::ptr::null(), 0, TIMER_ALL_ACCESS) };
            }
            if timer.0.is_null() { return wait_failed_after_cleanup(|| drop(timer)); }
        }
        let wake_index = wake.map(|handle| { handles[ordinary_count] = handle; ordinary_count });
        let timer_index = ordinary_count + usize::from(wake.is_some());
        handles[timer_index] = timer.0;
        // Retain the original short fixed-run rechecks as well as the later
        // cap, including when a backward wall-clock step postpones this timer.
        let fixed_cap = if initial_passes < 3 { initial_passes += 1; 250 } else { 10_000 };
        let mut interval = if wake.is_some() { wait_slice_wake(milliseconds) } else { milliseconds.min(fixed_cap) as u32 };
        if rotating { interval = interval.min(ROTATE_INTERVAL_MS); }
        let due = absolute_timer_due(file_time, interval as u64);
        if unsafe { SetWaitableTimer(timer.0, &due, 0, None, std::ptr::null_mut(), 0) } == 0 {
            return wait_failed_after_cleanup(|| drop(timer));
        }
        let status = observed_wait(&mut decision, || wait(&handles[..timer_index + 1], interval));
        if status != WAIT_TIMEOUT && status as usize != timer_index && Some(status as usize) != wake_index {
            return if status == WAIT_FAILED { wait_failed_after_cleanup(|| drop(timer)) } else { status };
        }
        if rotating { subset = (subset + 1) % subsets; }
    }
}

/// How long to wait before the deadline is calculated again: no longer than the time
/// left, and never longer than the backstop, which is what finds an exit whose message
/// from the job was lost. Waits that carry the wake event use `wait_slice_wake`
/// instead. Tests only: production timer-backed waits use their explicit
/// finite caps in `wait_with_deadline_observed`.
#[cfg(test)]
pub(super) fn wait_slice(remaining_milliseconds: u64) -> u32 {
    remaining_milliseconds.min(BACKSTOP_MS) as u32
}

/// The slice of a wait that carries the wake event. A grant signals the event,
/// so the wait ends the moment a grant lands and its precision does not come
/// from the slice: the longer backstop only bounds how long a lost message
/// from the job can go unnoticed; per-pass work runs at most once per slice.
pub(super) fn wait_slice_wake(remaining_milliseconds: u64) -> u32 {
    remaining_milliseconds.min(BACKSTOP_WAKE_MS) as u32
}

/// Queue optional notices after cleanup; returning library calls never start a writer.
pub(super) fn notify(text: String) {
    crate::diagnostics::stderr_owned(text);
}

pub(super) fn require_process_ownership(extendable: bool) -> Result<(), GuardError> {
    if extendable && !crate::diagnostics::owns_process() {
        return Err(GuardError("--extendable requires the process-owning cli_exit adapter; returning library entry points cannot own its background writers".into()));
    }
    Ok(())
}

// The scratch buffer of `with_job_process_ids`, kept per thread: the supervisor
// asks for the list on every pass while a hold is live, and a fresh buffer
// would be zeroed on every one of those passes. Only the prefix the call
// reports is ever read, so the kept contents need no clearing between calls;
// the buffer grows to the largest list its thread has seen and is reused at
// that capacity from then on.
thread_local! { static JOB_ID_BUFFER: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) }; }

/// Run `use_ids` with the identifiers of the processes in the job,
/// those in jobs nested in it included, as the scratch
/// buffer holds them (pointer-sized; the list is
/// JOBOBJECT_BASIC_PROCESS_ID_LIST: two 32 bit counts, then pointer
/// sized IDs), or with None when the list cannot be read.
#[cfg(test)]
pub(crate) fn with_job_process_ids<R>(job: Handle, use_ids: impl FnOnce(Option<&[usize]>) -> R) -> R {
    with_job_process_ids_form(job, false, use_ids)
}

/// As `with_job_process_ids`, with the identifiers sorted before they
/// are handed over: for a caller that only tests membership in the
/// list, once per hold it reaps. Sorting the scratch prefix in place
/// is safe for that caller — the buffer's contents are not preserved
/// across calls (only the prefix a call reports is ever read, and
/// the next call refills it from the header onward) — and membership
/// in the sorted prefix, tested by binary search, is the membership
/// of the list as the query reported it: sorting moves identifiers,
/// it neither adds nor removes one, and duplicates search as present
/// exactly as they scan as present. The reap of the extension state
/// paid a linear scan of the whole list for every hold it tested —
/// holds times members comparisons on every pass — where the sorted
/// form pays the sort once and a logarithmic search per hold.
pub(crate) fn with_job_process_ids_sorted<R>(job: Handle, use_ids: impl FnOnce(Option<&[usize]>) -> R) -> R {
    with_job_process_ids_form(job, true, use_ids)
}

// JOBOBJECT_BASIC_PROCESS_ID_LIST places two DWORDs before its ULONG_PTRs.
const JOB_ID_HEADER_BYTES: usize = 2 * std::mem::size_of::<u32>();
const JOB_ID_HEADER_WORDS: usize = JOB_ID_HEADER_BYTES / std::mem::size_of::<usize>();

fn job_id_counts(buffer: &[usize]) -> (usize, usize) {
    let header = &buffer[..JOB_ID_HEADER_WORDS];
    // The checked prefix covers both initialized DWORDs. A native word is
    // DWORD-aligned on both supported pointer widths.
    let [assigned, listed] = unsafe { *header.as_ptr().cast::<[u32; 2]>() };
    (assigned as usize, listed as usize)
}

fn with_job_process_ids_form<R>(job: Handle, sorted: bool, use_ids: impl FnOnce(Option<&[usize]>) -> R) -> R {
    JOB_ID_BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        use_complete_job_ids(&mut buffer, sorted, |words, bytes| query_job_process_ids(job, words, bytes), use_ids)
    })
}

fn use_complete_job_ids<R>(
    buffer: &mut Vec<usize>, sorted: bool,
    query: impl FnMut(&mut [usize], u32) -> Result<usize, u32>,
    use_ids: impl FnOnce(Option<&[usize]>) -> R,
) -> R {
    // A successful but incomplete reply has no negative membership authority.
    // Unavailable lists use the reaper's existing exact-handle fallback.
    let ids = read_complete_job_process_ids(buffer, || false, query).ok().map(|ids| {
        if sorted {
            ids.sort_unstable();
        }
        &*ids
    });
    use_ids(ids)
}

#[cfg(test)]
pub(crate) fn with_reaper_job_ids_for_test<R>(
    query: impl FnMut(&mut [usize], u32) -> Result<usize, u32>,
    use_ids: impl FnOnce(Option<&[usize]>) -> R,
) -> R {
    use_complete_job_ids(&mut Vec::new(), true, query, use_ids)
}

struct CleanupMemberHandles(HandleList);

impl Drop for CleanupMemberHandles {
    fn drop(&mut self) {
        for &handle in self.0.iter() {
            unsafe { CloseHandle(handle) };
        }
    }
}

/// Retain verified original members before terminating the owned Job, so
/// membership information released by termination cannot elide their waits.
pub(super) fn terminate_and_wait(
    job: Handle, code: u32, deadline: Instant,
    member_wait_uncertainty: &mut Option<MemberWaitUncertainty>,
    member_capture_failure: &mut Option<MemberCaptureFailure>,
    job_termination_error: &mut Option<u32>,
) -> bool {
    let mut members = CleanupMemberHandles(HandleList::new());
    if let Some(empty) = begin_job_cleanup(
        deadline, Instant::now, || active_processes(job),
        |failure| with_cleanup_job_process_ids(job, deadline, |ids| {
            capture_job_members(ids, &mut members.0, failure,
                |id| open_cleanup_member(job, id, deadline));
        }),
        || unsafe { TerminateJobObject(job, code) }, member_capture_failure, job_termination_error,
    ) {
        return empty;
    }
    for (index, chunk) in members.0.chunks(64).enumerate() {
        let unvisited = members.0.len() - index * 64;
        if !observe_member_cleanup(chunk, deadline, Instant::now, unvisited, member_wait_uncertainty,
            |handles, milliseconds| unsafe {
                WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 1, milliseconds)
            })
        {
            break;
        }
    }
    drop(members);
    wait_until_empty(job, deadline, member_capture_failure)
}

/// An empty initial result needs no kill. A late result cannot certify cleanup.
/// For any active or unknown Job, finish or refuse bounded capture before the
/// one termination request. Capture faults never suppress termination.
pub(super) fn begin_job_cleanup(
    deadline: Instant, now: impl FnMut() -> Instant,
    query: impl FnOnce() -> Result<u32, u32>,
    capture: impl FnOnce(&mut Option<MemberCaptureFailure>), terminate: impl FnOnce() -> i32,
    failure: &mut Option<MemberCaptureFailure>, termination_error: &mut Option<u32>,
) -> Option<bool> {
    let observed = observe_job_accounting(deadline, now, query);
    let accounting_fault = match observed {
        Some((_, true)) => None,
        Some((result, false)) => Some(MemberCaptureFailure::JobAccountingOverdue { result }),
        None => Some(MemberCaptureFailure::BudgetExpired),
    };
    if let Some(reason) = accounting_fault {
        failure.get_or_insert(reason);
    }
    if matches!(observed, Some((Ok(0), _))) {
        return Some(accounting_fault.is_none());
    }
    if accounting_fault.is_none() {
        capture(failure);
    }
    observe_job_termination(termination_error, terminate);
    accounting_fault.map(|_| false)
}

fn observe_job_accounting(
    deadline: Instant, mut now: impl FnMut() -> Instant, query: impl FnOnce() -> Result<u32, u32>,
) -> Option<(Result<u32, u32>, bool)> {
    if now() >= deadline { return None; }
    let result = query();
    Some((result, now() < deadline))
}

/// Retain a failed kill request separately from wait or member-capture facts.
/// Read LastError immediately on failure; successful requests add no native call.
#[inline]
pub(super) fn observe_job_termination(error: &mut Option<u32>, terminate: impl FnOnce() -> i32) {
    if terminate() == 0 {
        let win32_error = unsafe { GetLastError() };
        if error.is_none() { *error = Some(win32_error); }
    }
}

/// The first missing or unverified member observation remains a cleanup fault.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MemberCaptureFailure {
    Query { win32_error: u32 },
    Capacity { assigned: usize, capacity: usize },
    Allocation { capacity: usize },
    JobAccountingQuery { win32_error: u32 },
    JobAccountingOverdue { result: Result<u32, u32> },
    IncompleteList { assigned: usize, listed: usize, returned_bytes: usize },
    BudgetExpired,
    InvalidPid { reported_pid: usize },
    Open { reported_pid: usize, win32_error: u32 },
    Membership { reported_pid: usize, win32_error: u32 },
    OutsideJob { reported_pid: usize },
}

pub(super) fn with_cleanup_job_process_ids<R>(
    job: Handle,
    deadline: std::time::Instant,
    use_ids: impl FnOnce(Result<&[usize], MemberCaptureFailure>) -> R,
) -> R {
    JOB_ID_BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        let ids = cleanup_job_process_ids(&mut buffer, deadline, Instant::now,
            |words, bytes| query_job_process_ids(job, words, bytes));
        use_ids(ids)
    })
}

fn query_job_process_ids(job: Handle, words: &mut [usize], buffer_bytes: u32) -> Result<usize, u32> {
    let mut returned_bytes = 0;
    let ok = unsafe {
        QueryInformationJobObject(job, PROCESS_ID_LIST_CLASS, words.as_mut_ptr().cast(),
            buffer_bytes, &mut returned_bytes)
    };
    if ok == 0 { Err(unsafe { GetLastError() }) } else { Ok(returned_bytes as usize) }
}

pub(super) const MAX_CLEANUP_JOB_IDS: usize = 1 << 20;

pub(super) fn bounded_cleanup_capacity(capacity: usize, assigned: usize) -> usize {
    capacity.checked_mul(2).unwrap_or(MAX_CLEANUP_JOB_IDS)
        .max(assigned).min(MAX_CLEANUP_JOB_IDS)
}

/// Read a complete native list under the teardown deadline. The two DWORD
/// counts precede the pointer-sized IDs. Size retries cannot exceed the cap.
pub(super) fn cleanup_job_process_ids(
    buffer: &mut Vec<usize>, deadline: Instant, mut now: impl FnMut() -> Instant,
    query: impl FnMut(&mut [usize], u32) -> Result<usize, u32>,
) -> Result<&[usize], MemberCaptureFailure> {
    read_complete_job_process_ids(buffer, || now() >= deadline, query).map(|ids| &*ids)
}

fn read_complete_job_process_ids(
    buffer: &mut Vec<usize>, mut budget_expired: impl FnMut() -> bool,
    mut query: impl FnMut(&mut [usize], u32) -> Result<usize, u32>,
) -> Result<&mut [usize], MemberCaptureFailure> {
    const WORD_BYTES: usize = std::mem::size_of::<usize>();
    let mut capacity = 1024;
    loop {
        if budget_expired() { return Err(MemberCaptureFailure::BudgetExpired); }
        let capacity_error = MemberCaptureFailure::Capacity { assigned: capacity, capacity: MAX_CLEANUP_JOB_IDS };
        let words = JOB_ID_HEADER_WORDS.checked_add(capacity).ok_or(capacity_error)?;
        let byte_capacity = words.checked_mul(WORD_BYTES).ok_or(capacity_error)?;
        let native_bytes = u32::try_from(byte_capacity).map_err(|_| capacity_error)?;
        if buffer.len() < words {
            buffer.try_reserve_exact(words - buffer.len())
                .map_err(|_| MemberCaptureFailure::Allocation { capacity })?;
            buffer.resize(words, 0);
        }
        buffer[..JOB_ID_HEADER_WORDS].fill(0);
        if budget_expired() { return Err(MemberCaptureFailure::BudgetExpired); }
        let returned_bytes = match query(&mut buffer[..words], native_bytes) {
            Ok(bytes) => bytes,
            Err(ERROR_MORE_DATA) if capacity < MAX_CLEANUP_JOB_IDS => {
                capacity = bounded_cleanup_capacity(capacity, 0);
                continue;
            }
            Err(win32_error) => return Err(MemberCaptureFailure::Query { win32_error }),
        };
        if budget_expired() { return Err(MemberCaptureFailure::BudgetExpired); }
        let (assigned, listed) = job_id_counts(buffer);
        let incomplete = MemberCaptureFailure::IncompleteList { assigned, listed, returned_bytes };
        let used_words = JOB_ID_HEADER_WORDS.checked_add(listed).ok_or(incomplete)?;
        let needed = used_words.checked_mul(WORD_BYTES).ok_or(incomplete)?;
        if listed > capacity || !(needed..=byte_capacity).contains(&returned_bytes) {
            return Err(incomplete);
        }
        if assigned > MAX_CLEANUP_JOB_IDS {
            return Err(MemberCaptureFailure::Capacity { assigned, capacity: MAX_CLEANUP_JOB_IDS });
        }
        if assigned > capacity && listed == capacity {
            capacity = bounded_cleanup_capacity(capacity, assigned);
            continue;
        }
        if assigned != listed {
            return Err(incomplete);
        }
        return Ok(&mut buffer[JOB_ID_HEADER_WORDS..used_words]);
    }
}

/// The PID list is a hint. Wait only on the opened process object that
/// a native membership query places in the exact held Job.
pub(super) fn open_cleanup_member(
    job: Handle, reported_pid: usize, deadline: Instant,
) -> Result<Handle, MemberCaptureFailure> {
    if Instant::now() >= deadline { return Err(MemberCaptureFailure::BudgetExpired); }
    let pid = u32::try_from(reported_pid).ok().filter(|pid| *pid != 0)
        .ok_or(MemberCaptureFailure::InvalidPid { reported_pid })?;
    let handle = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return Err(MemberCaptureFailure::Open { reported_pid, win32_error: unsafe { GetLastError() } });
    }
    let failure = if Instant::now() >= deadline {
        MemberCaptureFailure::BudgetExpired
    } else {
        let mut member = 0;
        if unsafe { IsProcessInJob(handle, job, &mut member) } == 0 {
            MemberCaptureFailure::Membership { reported_pid, win32_error: unsafe { GetLastError() } }
        } else if member == 0 {
            MemberCaptureFailure::OutsideJob { reported_pid }
        } else if Instant::now() >= deadline {
            MemberCaptureFailure::BudgetExpired
        } else {
            return Ok(handle);
        }
    };
    unsafe { CloseHandle(handle) };
    Err(failure)
}

/// Keep validated handles in native list order and the first capture fault.
pub(super) fn capture_job_members(
    ids: Result<&[usize], MemberCaptureFailure>,
    members: &mut HandleList,
    failure: &mut Option<MemberCaptureFailure>,
    mut capture: impl FnMut(usize) -> Result<Handle, MemberCaptureFailure>,
) {
    let ids = match ids {
        Ok(ids) => ids,
        Err(reason) => {
            failure.get_or_insert(reason);
            return;
        }
    };
    for &id in ids {
        match capture(id) {
            Ok(handle) => members.push(handle),
            Err(reason) => {
                failure.get_or_insert(reason);
                if reason == MemberCaptureFailure::BudgetExpired { break; }
            }
        }
    }
}

/// The first retained-member window whose terminality was not confirmed.
/// A missing status means no native call began before the deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MemberWaitUncertainty {
    pub(super) status: Option<u32>,
    pub(super) win32_error: Option<u32>,
    pub(super) in_budget: bool,
    pub(super) unconfirmed_handles: usize,
}

/// Keep actual status and LastError even if the result returned too late.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CleanupWait {
    pub(super) status: Option<u32>,
    pub(super) win32_error: Option<u32>,
    pub(super) in_budget: bool,
}

fn observe_cleanup_wait(
    deadline: Instant, mut now: impl FnMut() -> Instant, wait: impl FnOnce(u32) -> u32,
) -> CleanupWait {
    let Some(remaining) = deadline.checked_duration_since(now()).filter(|t| !t.is_zero()) else {
        return CleanupWait::default();
    };
    // Rounding down keeps the wait finite. Zero polls fit a positive remainder.
    let milliseconds = remaining.as_millis().min(u128::from(u32::MAX - 1)) as u32;
    let status = wait(milliseconds);
    let win32_error = (status == WAIT_FAILED).then(|| unsafe { GetLastError() });
    CleanupWait { status: Some(status), win32_error, in_budget: now() < deadline }
}

/// WaitAll uses the same absolute deadline and keeps the first fault.
#[inline]
pub(super) fn observe_member_cleanup(
    handles: &[Handle], deadline: Instant, now: impl FnMut() -> Instant,
    unvisited_handles: usize, uncertainty: &mut Option<MemberWaitUncertainty>,
    wait: impl FnOnce(&[Handle], u32) -> u32,
) -> bool {
    let result = observe_cleanup_wait(deadline, now, |milliseconds| wait(handles, milliseconds));
    let complete = result.in_budget && result.status
        .and_then(|status| status.checked_sub(WAIT_OBJECT_0))
        .is_some_and(|index| (index as usize) < handles.len());
    if !complete && uncertainty.is_none() {
        let unconfirmed_handles = if result.status.is_some() { handles.len() } else { unvisited_handles };
        *uncertainty = Some(MemberWaitUncertainty {
            status: result.status, win32_error: result.win32_error, in_budget: result.in_budget,
            unconfirmed_handles,
        });
    }
    result.in_budget
}

/// Facts already collected by the one bounded teardown; this performs no wait.
#[derive(Clone, Copy, Debug)]
pub(super) struct CleanupObservation {
    pub(super) assigned: bool,
    pub(super) job_proven_empty: bool,
    pub(super) member_wait_uncertainty: Option<MemberWaitUncertainty>,
    pub(super) member_capture_failure: Option<MemberCaptureFailure>,
    pub(super) job_termination_error: Option<u32>,
    pub(super) child_created: bool,
    pub(super) child_exited: bool,
    pub(super) original_child_wait: Option<u32>,
    pub(super) original_child_wait_error: Option<u32>,
    pub(super) original_child_wait_in_budget: bool,
}

/// Preserve the original result only after the required native cleanup proof.
/// No new syscalls, waits or allocations occur on the confirmed path.
#[inline]
pub(super) fn finalize_cleanup(
    outcome: Result<u32, GuardError>,
    observed: CleanupObservation,
) -> (Result<u32, GuardError>, bool) {
    let job_confirmed = !observed.assigned || observed.job_proven_empty;
    let child_confirmed = !observed.child_created || observed.child_exited
        || (observed.original_child_wait_in_budget && observed.original_child_wait == Some(WAIT_OBJECT_0));
    if job_confirmed && child_confirmed && observed.member_wait_uncertainty.is_none()
        && observed.member_capture_failure.is_none() && observed.job_termination_error.is_none() {
        (outcome, true)
    } else {
        (Err(unconfirmed_cleanup_error(outcome, observed)), false)
    }
}

#[cold]
fn unconfirmed_cleanup_error(outcome: Result<u32, GuardError>, observed: CleanupObservation) -> GuardError {
    let original = match outcome {
        Ok(code) => format!("original run result: exit code {code}"),
        Err(error) => format!("original run error: {}", error.message()),
    };
    GuardError(format!(
        "process-tree cleanup could not be confirmed (job_assigned={}, job_empty={}, member_wait_uncertainty={:?}, member_capture_failure={:?}, job_termination_error={:?}, child_created={}, child_previously_exited={}, original_child_wait={:?}, native_wait_error={:?}, original_child_wait_in_budget={}); {original}",
        observed.assigned, observed.job_proven_empty, observed.member_wait_uncertainty, observed.member_capture_failure, observed.job_termination_error, observed.child_created, observed.child_exited,
        observed.original_child_wait, observed.original_child_wait_error, observed.original_child_wait_in_budget,
    ))
}

/// A previously observed exit needs no other wait. Otherwise compute the
/// remaining allowance immediately before waiting on the original handle.
pub(super) fn observe_child_cleanup(
    child: Handle, child_exited: bool, deadline: Instant, now: impl FnMut() -> Instant,
    wait: impl FnOnce(Handle, u32) -> u32,
) -> CleanupWait {
    if child.is_null() || child_exited { return CleanupWait::default(); }
    observe_cleanup_wait(deadline, now, |milliseconds| wait(child, milliseconds))
}

/// Query the exact held Job and retain the native error at the call boundary.
fn active_processes(job: Handle) -> Result<u32, u32> {
    let mut info = BasicAccountingInformation::default();
    let ok = unsafe { QueryInformationJobObject(job, BASIC_ACCOUNTING_CLASS,
        &mut info as *mut _ as *mut c_void, std::mem::size_of::<BasicAccountingInformation>() as u32,
        std::ptr::null_mut()) };
    if ok == 0 { Err(unsafe { GetLastError() }) } else { Ok(info.active_processes) }
}

fn wait_until_empty(job: Handle, deadline: Instant, failure: &mut Option<MemberCaptureFailure>) -> bool {
    loop {
        let reason = match observe_job_accounting(deadline, Instant::now, || active_processes(job)) {
            Some((Ok(0), true)) => return true,
            Some((Ok(_), true)) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let milliseconds = remaining.as_millis().min(10) as u32;
                if milliseconds == 0 { return false; }
                unsafe { Sleep(milliseconds) };
                continue;
            }
            Some((Err(win32_error), true)) => MemberCaptureFailure::JobAccountingQuery { win32_error },
            Some((result, false)) => MemberCaptureFailure::JobAccountingOverdue { result },
            None => MemberCaptureFailure::BudgetExpired,
        };
        failure.get_or_insert(reason);
        return false;
    }
}
