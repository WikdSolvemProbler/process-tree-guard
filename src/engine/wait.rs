// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The waits of a run: the plain and extendable deadline waits over the child and its owners, and the teardown wait.

use super::*;


use crate::sys::windows::{

    BASIC_ACCOUNTING_CLASS, BasicAccountingInformation, CloseHandle, CreateWaitableTimerExW,

    GetLastError, GetSystemTimeAsFileTime, GetTickCount64, Handle, OpenProcess,

    PROCESS_ID_LIST_CLASS, QueryInformationJobObject, SYNCHRONIZE, SetLastError, SetWaitableTimer,

    Sleep, TIMER_ALL_ACCESS, TerminateJobObject, WAIT_FAILED, WAIT_TIMEOUT,

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

pub(super) fn wait_with_deadline_observed(mut remaining: impl FnMut() -> Option<u64>, wake: Option<Handle>, waits: &[Handle], mut decision: Option<&mut DecisionEvidence>) -> u32 {
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
    // The waitable timer exists for the suspend-inclusive wake on long waits.
    // The first passes wait plainly in short slices and re-read the deadline
    // each pass, so a run that ends inside them never creates, arms or closes
    // a timer at all; the timer is created on the fourth pass at the latest.
    let mut plain_passes = 0usize;
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
        // A plain pass cannot consume the wall clock — only a timer-armed
        // pass turns it into an absolute due time — so a plain pass samples
        // the remaining time alone and never calls for the wall clock. A
        // timer pass keeps the documented order of `sampled_wait_plan`: the
        // wall clock is sampled first and the remaining time after it, so
        // the remaining time a due time is computed from is the one sampled
        // after its wall clock, never one sampled before it.
        let plain = timer.0.is_null() && plain_passes < 3;
        let (file_time, milliseconds) = match if plain {
            remaining().filter(|left| *left > 0).map(|left| (0, left))
        } else {
            sampled_wait_plan(|| {
                let mut file_time = 0u64;
                unsafe { GetSystemTimeAsFileTime(&mut file_time) };
                file_time
            }, &mut remaining)
        } {
            Some(plan) => plan,
            None => {
                // For a deep owner set, an expired clock read must check every
                // retained owner before choosing timeout. Ordinary cycles keep
                // their original one-window 50 ms rotation and native call count.
                if rotating {
                    let selected = poll_all_owner_windows(waits[0], owners, &mut handles, |window, _, _| {
                        observed_wait(&mut decision, || unsafe {
                            WaitForMultipleObjects(window.len() as u32, window.as_ptr(), 0, 0)
                        })
                    }).expect("validated deep owner list is nonempty");
                    return if selected.status == WAIT_FAILED {
                        wait_failed_after_cleanup(|| drop(timer))
                    } else { selected.status };
                }
                let status = observed_wait(&mut decision, || unsafe {
                    WaitForMultipleObjects(ordinary_count as u32, handles.as_ptr(), 0, 0)
                });
                return if status == WAIT_FAILED { wait_failed_after_cleanup(|| drop(timer)) } else { status };
            }
        };
        if plain {
            plain_passes += 1;
            let plain_wake_index = wake.map(|handle| { handles[ordinary_count] = handle; ordinary_count });
            let plain_count = ordinary_count + usize::from(wake.is_some());
            let mut plain_interval = if wake.is_some() { wait_slice_wake(milliseconds) } else { milliseconds.min(250) as u32 };
            if rotating { plain_interval = plain_interval.min(ROTATE_INTERVAL_MS); }
            let status = observed_wait(&mut decision, || unsafe {
                WaitForMultipleObjects(plain_count as u32, handles.as_ptr(), 0, plain_interval)
            });
            if status == WAIT_TIMEOUT || Some(status as usize) == plain_wake_index {
                if rotating { subset = (subset + 1) % subsets; }
                continue;
            }
            return if status == WAIT_FAILED { wait_failed_after_cleanup(|| drop(timer)) } else { status };
        }
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
        let mut interval = if wake.is_some() { wait_slice_wake(milliseconds) } else { milliseconds.min(10_000) as u32 };
        if rotating { interval = interval.min(ROTATE_INTERVAL_MS); }
        let due = absolute_timer_due(file_time, interval as u64);
        if unsafe { SetWaitableTimer(timer.0, &due, 0, None, std::ptr::null_mut(), 0) } == 0 {
            return wait_failed_after_cleanup(|| drop(timer));
        }
        let status = observed_wait(&mut decision, || unsafe {
            WaitForMultipleObjects((timer_index + 1) as u32, handles.as_ptr(), 0, interval)
        });
        if status != WAIT_TIMEOUT && status as usize != timer_index && Some(status as usize) != wake_index {
            return if status == WAIT_FAILED { wait_failed_after_cleanup(|| drop(timer)) } else { status };
        }
        if rotating { subset = (subset + 1) % subsets; }
    }
}

/// How long to wait before the deadline is calculated again: no longer than the time
/// left, and never longer than the backstop, which is what finds an exit whose message
/// from the job was lost. Waits that carry the wake event use `wait_slice_wake`
/// instead. Tests only: production plain passes cap at 250 ms inline in
/// `wait_with_deadline_observed`; no production wait uses 100 ms.
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
/// sized IDs), or with None when the list
/// cannot be read. Callers that only test membership in the list or
/// open the processes in it use this form: no vector of the
/// identifiers is built and copied out for them, on a path the
/// supervisor walks on every pass while a hold is live and teardown
/// walks once per run.
pub(crate) fn with_job_process_ids<R>(job: Handle, use_ids: impl FnOnce(Option<&[usize]>) -> R) -> R {
    with_job_process_ids_form(job, false, use_ids)
}

/// As `with_job_process_ids`, with the identifiers sorted before they
/// are handed over: for a caller that only tests membership in the
/// list, once per hold it reaps. Sorting the scratch prefix in place
/// is safe for that caller — the buffer's contents are not preserved
/// across calls (only the prefix a call reports is ever read, and
/// the next call refills it from the header word on) — and membership
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

fn with_job_process_ids_form<R>(job: Handle, sorted: bool, use_ids: impl FnOnce(Option<&[usize]>) -> R) -> R {
    JOB_ID_BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        // Grow the buffer until the whole list fits (the call fails with more data otherwise).
        let mut capacity = 1024usize;
        let mut ok;
        loop {
            if buffer.len() < 1 + capacity {
                buffer.resize(1 + capacity, 0);
            }
            ok = unsafe {
                QueryInformationJobObject(job, PROCESS_ID_LIST_CLASS, buffer.as_mut_ptr() as *mut c_void,
                    ((1 + capacity) * std::mem::size_of::<usize>()) as u32, std::ptr::null_mut())
            };
            let assigned = (buffer[0] as u64 & 0xffff_ffff) as usize;
            if ok != 0 || assigned <= capacity || capacity >= 1 << 20 {
                break;
            }
            capacity = (assigned + 256).next_power_of_two();
        }
        if ok == 0 {
            return use_ids(None);
        }
        let count = ((buffer[0] as u64) >> 32) as usize;
        let listed = 1 + count.min(capacity);
        if sorted {
            buffer[1..listed].sort_unstable();
        }
        use_ids(Some(&buffer[1..listed]))
    })
}

/// Terminate every process in the job and wait, at most `milliseconds`, until
/// each has fully exited. Termination is asynchronous, so without this the
/// guard could return while a descendant is still exiting. Processes are
/// opened before termination so each one can be waited on by handle.
/// Returns whether the job emptied, and the unspent remainder of the one
/// teardown budget: the wait on the child shares that budget, it does not
/// start a second one.
pub(super) fn terminate_and_wait(job: Handle, code: u32, milliseconds: u32) -> (bool, u32) {
    if active_processes(job) == Some(0) { return (true, milliseconds); }
    let start = std::time::Instant::now();
    let deadline = start + std::time::Duration::from_millis(u64::from(milliseconds));
    // The members are held in the same inline list as the run's other
    // handle sets: the ordinary handful of processes never allocates,
    // and a deeper tree spills to one vector, as `HandleList` does
    // everywhere else. The windows below slice it through its `Deref`,
    // which is the spill when there is one and the inline array when
    // there is not — either way one contiguous run of handles.
    let mut members = HandleList::new();
    with_job_process_ids(job, |ids| {
        if let Some(ids) = ids {
            for &id in ids {
                let handle = unsafe { OpenProcess(SYNCHRONIZE, 0, id as u32) };
                if !handle.is_null() {
                    members.push(handle);
                }
            }
        }
    });
    unsafe { TerminateJobObject(job, code) };
    // Wait on the members in windows of the native wait's handle
    // limit, every member of a window at once: one wait and one
    // clock read per window replace one wait and one clock read per
    // member, under the same single budget the serial waits shared —
    // a window's wait spends at most what is left of it, and a window
    // reached with nothing left is not waited on at all.
    let mut next = 0;
    while next < members.len() {
        let end = (next + 64).min(members.len());
        let remaining = deadline.saturating_duration_since(std::time::Instant::now()).as_millis() as u32;
        if remaining == 0 {
            break;
        }
        let chunk = &members[next..end];
        unsafe { WaitForMultipleObjects(chunk.len() as u32, chunk.as_ptr(), 1, remaining) };
        next = end;
    }
    for &handle in members.iter() {
        unsafe { CloseHandle(handle) };
    }
    let empty = wait_until_empty(job, deadline);
    (empty, deadline.saturating_duration_since(std::time::Instant::now()).as_millis() as u32)
}

/// Only a prior native observation of this child establishes its terminality.
/// Job accounting is not a substitute for waiting on the retained child handle.
/// Return the actual bounded wait result; never manufacture a native signal.
/// The wait is bounded by `milliseconds`, the unspent remainder of the
/// teardown budget, never a fresh budget of its own.
pub(super) fn observe_child_cleanup(child: Handle, child_exited: bool, milliseconds: u32, wait: impl FnOnce(Handle, u32) -> u32) -> Option<u32> {
    if child.is_null() || child_exited { None } else { Some(wait(child, milliseconds)) }
}

/// Wait, at most `milliseconds`, until the job reports no active process.
fn active_processes(job: Handle) -> Option<u32> {
    let mut info = BasicAccountingInformation::default();
    let ok = unsafe { QueryInformationJobObject(job, BASIC_ACCOUNTING_CLASS,
        &mut info as *mut _ as *mut c_void, std::mem::size_of::<BasicAccountingInformation>() as u32,
        std::ptr::null_mut()) };
    (ok != 0).then_some(info.active_processes)
}

/// Wait until the job reports no active process, or the teardown
/// deadline is reached: the one deadline of `terminate_and_wait`,
/// passed in, not a second budget measured from here.
fn wait_until_empty(job: Handle, deadline: std::time::Instant) -> bool {
    loop {
        match active_processes(job) {
            Some(0) => return true,
            None => return false,
            _ => {}
        }
        if std::time::Instant::now() >= deadline { return false; }
        unsafe { Sleep(10) };
    }
}
