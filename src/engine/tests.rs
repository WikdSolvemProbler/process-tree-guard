// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

    use super::*;


    use crate::sys::windows::{

        AssignProcessToJobObject, Bool, CloseHandle, CreateEventW, CreateJobObjectW,

        DUPLICATE_SAME_ACCESS, DuplicateHandle, GetCurrentProcess, GetCurrentProcessId,

        GetCurrentThreadId, GetExitCodeProcess, GetLastError, GetProcessId, GetTickCount64,

        Handle, IsProcessInJob, ResetEvent, SetEvent, SetLastError, StartupInfoExW, StartupInfoW,

        WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT, WaitForMultipleObjects, WaitForSingleObject,

        wide,

    };
    #[test]
    fn unstarted_trace_never_takes_its_tls_queue() {
        assert_eq!(collected_trace(false, None, || panic!("trace queue was read")), None);
        assert_eq!(collected_trace(false, Some("notice".into()),
            || panic!("trace queue was read")), Some("notice".into()));
    }

    #[test]
    fn trace_cache_does_not_mask_returning_live_environment() {
        let cached = std::sync::OnceLock::new();
        assert!(!trace_enabled_with(true, &cached, || false));
        assert!(!trace_enabled_with(true, &cached, || true));
        assert!(trace_enabled_with(false, &cached, || true));
        assert!(!trace_enabled_with(false, &cached, || false));
    }

    #[test]
    fn started_trace_keeps_order_and_empty_notice_semantics() {
        assert_eq!(collected_trace(true, Some("notice".into()),
            || vec!["first".into(), "second".into()]),
            Some("first\nsecond\nnotice".into()));
        assert_eq!(collected_trace(true, None, Vec::new), None);
        assert_eq!(collected_trace(false, Some(String::new()),
            || panic!("trace queue was read")), Some(String::new()));
    }

struct DeepOwnerEvents(Vec<Handle>);
impl DeepOwnerEvents {
    fn new(count: usize) -> Self {
        let mut owned = Self(Vec::new());
        for _ in 0..count {
            let handle = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
            assert!(!handle.is_null());
            owned.0.push(handle);
        }
        owned
    }
    fn signal(&self, index: usize) { assert_ne!(unsafe { SetEvent(self.0[index]) }, 0); }
    fn close(&mut self, index: usize) {
        let handle = std::mem::replace(&mut self.0[index], std::ptr::null_mut());
        assert_ne!(unsafe { CloseHandle(handle) }, 0);
    }
}
impl Drop for DeepOwnerEvents {
    fn drop(&mut self) {
        for &handle in &self.0 {
            if !handle.is_null() { unsafe { CloseHandle(handle) }; }
        }
    }
}

#[test]
fn deep_owner_expiry_only_checks_late_group_after_clock_read() {
    for count in [61usize, 4096] {
        let owned = DeepOwnerEvents::new(count + 1);
        owned.signal(count); // Last owner was ready before this call.
        let mut reads = 0;
        let mut decision = DecisionEvidence::default();
        let status = wait_with_deadline_observed(|| { reads += 1; None }, None, &owned.0, Some(&mut decision));
        assert_eq!(reads, 1, "ordinary rotation must reach the clock before the expiry-only full sweep");
        let expected_local = ((count - 1) % OWNERS_PER_WINDOW + 1) as u32;
        assert_eq!(status, expected_local, "actual native local index, not a global owner index");
        assert_eq!(decision.recorded_wait_status(), Some(status));
    }
}

#[test]
fn deep_owner_expiry_clock_signal_gets_a_complete_real_final_sweep() {
    for count in [61usize, 4096] {
        let owned = DeepOwnerEvents::new(count + 1);
        let mut reads = 0;
        let status = wait_with_deadline_observed(|| {
            reads += 1;
            owned.signal(count); // Signal after the current-window poll, at expiry read.
            None
        }, None, &owned.0, None);
        assert_eq!(reads, 1);
        assert_eq!(status, ((count - 1) % OWNERS_PER_WINDOW + 1) as u32);
    }
}

#[test]
fn deep_owner_expiry_scan_keeps_child_priority_and_window_context() {
    let owned = DeepOwnerEvents::new(4097);
    let owners = &owned.0[1..];
    let mut handles = [std::ptr::null_mut(); 64];
    let mut count = 0;
    let selected = poll_all_owner_windows(owned.0[0], owners, &mut handles, |window, offset, len| {
        assert_eq!(window[0], owned.0[0]);
        assert_eq!(&window[1..], &owners[offset..offset + len]);
        assert_eq!(offset, count * OWNERS_PER_WINDOW);
        count += 1;
        unsafe { WaitForMultipleObjects(window.len() as u32, window.as_ptr(), 0, 0) }
    }).unwrap();
    assert_eq!(count, 69);
    assert_eq!(selected, NativeOwnerWindowResult { status: WAIT_TIMEOUT, owner_offset: 4080, owner_count: 16 });
    owned.signal(4096);
    let selected = poll_all_owner_windows(owned.0[0], owners, &mut handles, |window, _, _| unsafe {
        WaitForMultipleObjects(window.len() as u32, window.as_ptr(), 0, 0)
    }).unwrap();
    assert_eq!(selected, NativeOwnerWindowResult { status: 16, owner_offset: 4080, owner_count: 16 });
    owned.signal(0);
    let selected = poll_all_owner_windows(owned.0[0], owners, &mut handles, |window, _, _| unsafe {
        WaitForMultipleObjects(window.len() as u32, window.as_ptr(), 0, 0)
    }).unwrap();
    assert_eq!(selected.status, WAIT_OBJECT_0);
    assert_eq!(selected.owner_offset, 0);
    assert_eq!(poll_all_owner_windows(owned.0[0], &[], &mut handles, |_, _, _| {
        panic!("empty owner list must not invent a native status")
    }), None);
}

#[test]
fn deep_owner_expiry_timeout_is_the_final_real_native_receipt() {
    let owned = DeepOwnerEvents::new(4097);
    let mut reads = 0;
    let mut decision = DecisionEvidence::default();
    let status = wait_with_deadline_observed(|| { reads += 1; None }, None, &owned.0, Some(&mut decision));
    assert_eq!(reads, 1);
    assert_eq!(status, WAIT_TIMEOUT);
    assert_eq!(decision.recorded_wait_status(), Some(WAIT_TIMEOUT));
}

#[test]
fn deep_owner_expiry_after_existing_timer_preserves_native_last_error() {
    let mut owned = DeepOwnerEvents::new(62);
    let waits = owned.0.clone(); // Handle values only; owned remains the sole owner.
    let mut reads = 0;
    let status = wait_with_deadline_observed(|| {
        reads += 1;
        assert!(reads <= 2);
        // Close only between native waits, after the first timer wait exists.
        // Expiry then makes the new final sweep observe this invalid HANDLE.
        if reads == 2 { owned.close(61); None } else { Some(1) }
    }, None, &waits, None);
    let error = unsafe { GetLastError() };
    assert_eq!(status, WAIT_FAILED);
    assert_eq!(error, 6, "timer Drop must not replace ERROR_INVALID_HANDLE");
    assert_eq!(reads, 2);
}

#[test]
fn deep_owner_expiry_after_extension_wake_keeps_rechecking() {
    let owned = DeepOwnerEvents::new(62);
    let wake = DeepOwnerEvents::new(1);
    wake.signal(0);
    let mut reads = 0;
    let status = wait_with_deadline_observed(|| {
        reads += 1;
        if reads == 1 { Some(100_000) } else { None }
    }, Some(wake.0[0]), &owned.0, None);
    assert_eq!(reads, 2);
    assert_eq!(status, WAIT_TIMEOUT, "wake requests a recheck, not an owner status");
}


    struct NativeEvents(Vec<Handle>);
    impl NativeEvents {
        fn new() -> Self { Self(Vec::new()) }
        fn add(&mut self) -> Handle {
            let handle = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
            assert!(!handle.is_null());
            self.0.push(handle);
            handle
        }
    }
    impl Drop for NativeEvents {
        fn drop(&mut self) { for &handle in &self.0 { unsafe { CloseHandle(handle) }; } }
    }

    #[test]
    fn fixed_pre_resume_pure_clock_plan_covers_below_at_above_and_poll_error() {
        // These closures model only the no-signal planning branch. The tests
        // below use real events for every child and owner signal receipt.
        for (now, expected, expected_polls) in [(9, None, 1), (10, Some(WAIT_TIMEOUT), 2), (11, Some(WAIT_TIMEOUT), 2)] {
            let mut polls = 0;
            let selected = fixed_pre_resume_with(10, || now, || {
                polls += 1;
                Ok(None)
            }).unwrap();
            assert_eq!(selected, expected);
            assert_eq!(polls, expected_polls);
        }
        let mut clock_read = false;
        let error = fixed_pre_resume_with(10, || { clock_read = true; 9 }, || {
            Err(GuardError("native poll failed".into()))
        }).expect_err("the first poll error wins before the clock");
        assert_eq!(error.message(), "native poll failed");
        assert!(!clock_read);
        let mut polls = 0;
        let error = fixed_pre_resume_with(10, || 10, || {
            polls += 1;
            if polls == 1 { Ok(None) } else { Err(GuardError("second poll failed".into())) }
        }).expect_err("an expiry-edge poll error is not converted into timeout");
        assert_eq!(error.message(), "second poll failed");
        assert_eq!(polls, 2);
    }

    #[test]
    fn fixed_pre_resume_clock_boundary_and_actual_event_precedence() {
        let mut events = NativeEvents::new();
        let child = events.add();
        let owner = events.add();
        let waits = [child, owner];
        let mut polls = 0;
        let live = fixed_pre_resume_with(10, || 9, || {
            polls += 1;
            poll_before_resume(&waits, None)
        }).expect("ordinary unsignaled native events");
        assert_eq!(live, None, "an unexpired clock permits the later resume");
        assert_eq!(polls, 1);

        let mut decision = DecisionEvidence::default();
        let expired = fixed_pre_resume_with(10, || 10, || {
            poll_before_resume(&waits, Some(&mut decision))
        }).expect("two real zero-time native polls");
        assert_eq!(expired, Some(WAIT_TIMEOUT), "the exact boundary refuses resume");
        assert_eq!(decision.recorded_wait_status(), Some(WAIT_TIMEOUT));

        let mut polls = 0;
        let owner_at_expiry = fixed_pre_resume_with(10, || 10, || {
            polls += 1;
            if polls == 2 { assert_ne!(unsafe { SetEvent(owner) }, 0); }
            poll_before_resume(&waits, None)
        }).expect("the second real native poll observes the owner");
        assert_eq!(owner_at_expiry, Some(1), "observed owner beats expiry");
        assert_eq!(polls, 2);
        let mut clock_read = false;
        let owner_before_clock = fixed_pre_resume_with(10, || { clock_read = true; 0 }, || {
            poll_before_resume(&waits, None)
        }).expect("the original owner handle is signaled");
        assert_eq!(owner_before_clock, Some(1));
        assert!(!clock_read, "a real owner result is decided before a clock read");
        assert_ne!(unsafe { SetEvent(child) }, 0);
        assert_eq!(poll_before_resume(&waits, None).unwrap(), Some(0),
            "the original child at native index zero precedes the owner");
    }

    #[test]
    fn fixed_pre_resume_deep_owner_window_and_native_failure_keep_real_receipts() {
        let mut events = NativeEvents::new();
        let child = events.add();
        let owners: Vec<Handle> = (0..OWNERS_PER_WINDOW + 1).map(|_| events.add()).collect();
        let mut waits = vec![child];
        waits.extend_from_slice(&owners);
        assert_eq!(poll_before_resume(&waits, None).unwrap(), None);
        assert_ne!(unsafe { SetEvent(*owners.last().unwrap()) }, 0);
        let mut decision = DecisionEvidence::default();
        assert_eq!(poll_before_resume(&waits, Some(&mut decision)).unwrap(), Some(1),
            "owner 61 reports its actual local native window index one");
        assert_eq!(decision.recorded_wait_status(), Some(1));
        assert_ne!(unsafe { SetEvent(child) }, 0);
        assert_eq!(poll_before_resume(&waits, None).unwrap(), Some(0));
        assert_ne!(unsafe { ResetEvent(child) }, 0);
        let mut failed_decision = DecisionEvidence::default();
        let error = poll_before_resume(&[child, std::ptr::null_mut()], Some(&mut failed_decision))
            .expect_err("a null owner handle is a native WAIT_FAILED");
        assert_eq!(failed_decision.recorded_wait_status(), Some(WAIT_FAILED));
        unsafe { SetLastError(0x4321) };
        assert!(error.message().contains("[WinError 6]"), "{error}");
    }

    #[test]
    fn zero_fixed_budget_never_resumes_the_original_suspended_child() {
        use std::cell::Cell;
        use std::rc::Rc;
        let _cli_owner = crate::diagnostics::ProcessOwnership::begin();
        let seen = Rc::new(Cell::new(None));
        let original_wait = Rc::new(Cell::new(None));
        let wait_recorder = original_wait.clone();
        let recorder = seen.clone();
        let resume_attempts = Rc::new(Cell::new(0usize));
        let attempts = resume_attempts.clone();
        // Observe real native state before any possible resume. The duplicate
        // retains the exact child process object through normal Job cleanup.
        AFTER_CREATE.with(|hook| *hook.borrow_mut() = Some(Box::new(move |job, child| {
            let mut duplicate = std::ptr::null_mut();
            let current = unsafe { GetCurrentProcess() };
            let copied = unsafe { DuplicateHandle(current, child, current, &mut duplicate,
                0, 0, DUPLICATE_SAME_ACCESS) };
            let before = unsafe { WaitForSingleObject(child, 0) };
            let mut member = 0;
            let membership = unsafe { IsProcessInJob(child, job, &mut member) };
            let original_pid = unsafe { GetProcessId(child) };
            recorder.set(Some((child, duplicate, copied, before, membership, member, original_pid)));
        })));
        BEFORE_RESUME.with(|hook| *hook.borrow_mut() = Some(Box::new(move || {
            attempts.set(attempts.get() + 1);
        })));
        AFTER_CHILD_CLEANUP.with(|hook| *hook.borrow_mut() = Some(Box::new(move |child, status| {
            wait_recorder.set(Some((child, status)));
        })));
        let argv = ["cmd.exe".into(), "/d".into(), "/c".into(), "exit /b 37".into()];
        // The internal engine accepts zero milliseconds; the public CLI still
        // rejects a zero timeout. No artificial stall or process manipulation.
        let outcome = run_job_with(&argv, 0.0, None, false, &RunMode { library: true, ..RunMode::default() });
        AFTER_CREATE.with(|hook| *hook.borrow_mut() = None);
        BEFORE_RESUME.with(|hook| *hook.borrow_mut() = None);
        AFTER_CHILD_CLEANUP.with(|hook| *hook.borrow_mut() = None);
        let observed = seen.get();
        let (original_child, duplicate, copied, before, membership, member, original_pid) =
            observed.expect("CreateProcessW produced the original suspended child");
        let terminal = unsafe { WaitForSingleObject(duplicate, 0) };
        let mut exit_code = 0;
        let exit_read = unsafe { GetExitCodeProcess(duplicate, &mut exit_code) };
        let duplicate_pid = unsafe { GetProcessId(duplicate) };
        if !duplicate.is_null() { unsafe { CloseHandle(duplicate) }; }
        assert_eq!(outcome.expect("expired fixed budget is reported"), 124);
        assert_eq!(resume_attempts.get(), 0, "the real ResumeThread branch was never entered");
        assert_ne!(copied, 0, "retain the actual original process object");
        assert_eq!(before, WAIT_TIMEOUT, "the original child was unsignaled before resume");
        assert_ne!(membership, 0, "the native Job membership query succeeded");
        assert_eq!(member, 1, "the child was already inside its Job");
        assert_ne!(original_pid, 0);
        assert_eq!(duplicate_pid, original_pid);
        assert_eq!(original_wait.get(), Some((original_child, Some(WAIT_OBJECT_0))),
            "the existing cleanup waited on the exact original child HANDLE");
        assert_eq!(terminal, WAIT_OBJECT_0, "cleanup waited for the original child");
        assert_eq!(exit_read, 1);
        assert_eq!(exit_code, 124, "Job cleanup terminated the suspended child, not exit 37");
    }

    #[test]
    fn direct_wide_renderer_matches_prior_utf8_then_wide_for_quotes_unicode_and_spill() {
        let cases: &[&[&str]] = &[
            &[r"C:\Tools\guard.exe", "plain", "two words", ""],
            &[r"C:\Program Files\guard.exe", r"one\two", "say \"hi\"", "é中😀"],
            &["exe", "\\\"", "end slash \\", "tab\targ"],
            &["exe", "a"],
        ];
        for args in cases {
            for tail in [None, Some(""), Some("raw \"quotes\" \\ and 😀")] {
                let mut reference = command_line(args.iter().copied());
                if let Some(tail) = tail {
                    reference.push(' ');
                    reference.push('\"');
                    reference.push_str(tail);
                    reference.push('"');
                }
                assert_eq!(command_line_wide(args.iter().copied(), tail).as_slice(), wide(&reference), "{args:?} {tail:?}");
            }
        }
        let long = "a".repeat(1100);
        let spilled = command_line_wide(["exe", long.as_str()].into_iter(), None);
        assert!(spilled.heap.is_some());
        assert_eq!(spilled.as_slice(), wide(&format!("exe {long}")));
        let short = command_line_wide(["exe", "arg"].into_iter(), None);
        assert!(short.heap.is_none());
        // Keep the exact wide units, including an embedded NUL; CreateProcessW
        // observes the same first terminator on either rendering path.
        for tail in [None, Some("raw\0tail")] {
            let args = ["exe", "a\0b", "😀"];
            let mut old = command_line(args.iter().copied());
            if let Some(tail) = tail {
                old.push(' ');
                old.push('"');
                old.push_str(tail);
                old.push('"');
            }
            assert_eq!(command_line_wide(args.iter().copied(), tail).as_slice(), wide(&old));
        }
        // The high surrogate is the last stack unit and the low surrogate
        // begins the spill; the old renderer is the reference for every unit.
        let crossing = format!("{}😀", "a".repeat(1019));
        let args = ["exe", crossing.as_str()];
        let old = wide(&command_line(args.into_iter()));
        let direct = command_line_wide(args.into_iter(), None);
        assert!(direct.heap.is_some());
        assert_eq!(direct.as_slice(), old);
        assert_eq!(direct.as_slice()[1023], 0xD83D);
        assert_eq!(direct.as_slice()[1024], 0xDE00);
        let at_limit = "a".repeat(1019);
        let exact = command_line_wide(["exe", at_limit.as_str()].into_iter(), None);
        assert!(exact.heap.is_none(), "1024 units including NUL fit the stack");
        assert_eq!(exact.as_slice(), wide(&format!("exe {at_limit}")));
        for path in ["C:\\bin\\tool.exe", "C:\\Some Space\\😀.exe", "x\0suffix"] {
            let direct = application_wide(path);
            assert!(direct.heap.is_none());
            assert_eq!(direct.as_slice(), wide(path));
            assert_eq!(direct.as_ptr(), direct.as_slice().as_ptr());
        }
        let long_path = "x".repeat(1100);
        let direct = application_wide(&long_path);
        assert!(direct.heap.is_some());
        assert_eq!(direct.as_slice(), wide(&long_path));
        let exact_path = "x".repeat(259);
        let exact = application_wide(&exact_path);
        assert!(exact.heap.is_none());
        assert_eq!(exact.as_slice(), wide(&exact_path));
        let crossing_path = format!("{}😀", "x".repeat(258));
        let crossing = application_wide(&crossing_path);
        assert!(crossing.heap.is_some());
        assert_eq!(crossing.as_slice(), wide(&crossing_path));
        assert_eq!(crossing.as_slice()[258], 0xD83D);
        assert_eq!(crossing.as_slice()[259], 0xDE00);
    }

    #[test]
    fn fused_application_argv0_matches_independent_string_reference_at_boundaries() {
        let mut firsts = vec![
            "".to_string(), "tool.exe".to_string(), "ToOl.ExE".to_string(),
            r"C:\Tools\tool.exe".to_string(), r"C:\Program Files\tool.exe".to_string(),
            "quote\"inside.exe".to_string(), "tab\tinside.exe".to_string(),
            "embedded\0nul.exe".to_string(), "é中😀.exe".to_string(),
            r"C:\path\ending\".to_string(),
        ];
        for count in [258, 259, 260, 1023, 1024] {
            firsts.push("x".repeat(count));
        }
        firsts.push(format!("{}😀", "x".repeat(259)));
        firsts.push(format!("{}😀", "x".repeat(1023)));
        let following = ["", "plain", "two words", r#"\""#, r"trailing slash \", "😀\0next"];
        for first in &firsts {
            let application = application_wide(first);
            assert_eq!(application.as_slice(), wide(first), "application {first:?}");
            assert_eq!(application.as_ptr(), application.as_slice().as_ptr());
            for extra in [None, Some(following.as_slice())] {
                let mut args = vec![first.as_str()];
                args.extend(extra.unwrap_or(&[]).iter().copied());
                for tail in [None, Some(""), Some("raw \"quotes\" \\ and 😀"), Some("raw\0tail")] {
                    let fused = command_line_wide_from_application(
                        &application, args[1..].iter().copied(), tail);
                    let independent = command_line_wide(args.iter().copied(), tail);
                    let mut prior = command_line(args.iter().copied());
                    if let Some(tail) = tail {
                        prior.push(' ');
                        prior.push('"');
                        prior.push_str(tail);
                        prior.push('"');
                    }
                    assert_eq!(fused.as_slice(), independent.as_slice(), "{args:?} {tail:?}");
                    assert_eq!(fused.as_slice(), wide(&prior), "{args:?} {tail:?}");
                }
            }
        }
        let exact = application_wide(&"x".repeat(259));
        assert!(exact.heap.is_none(), "260 application units including NUL fit");
        let spill = application_wide(&"x".repeat(260));
        assert!(spill.heap.is_some(), "261 application units spill");
        let crossing = format!("{}😀", "x".repeat(259));
        let wide_application = application_wide(&crossing);
        assert_eq!(wide_application.as_slice()[259], 0xD83D);
        assert_eq!(wide_application.as_slice()[260], 0xDE00);
        let exact_line = command_line_wide_from_application(
            &application_wide(&"x".repeat(1023)), std::iter::empty(), None);
        assert!(exact_line.heap.is_none(), "1024 command units including NUL fit");
        let crossing_line = command_line_wide_from_application(
            &application_wide(&format!("{}😀", "x".repeat(1023))), std::iter::empty(), None);
        assert!(crossing_line.heap.is_some());
        assert_eq!(crossing_line.as_slice()[1023], 0xD83D);
        assert_eq!(crossing_line.as_slice()[1024], 0xDE00);
    }

    #[test]
    fn ordered_parent_map_preserves_duplicate_replacement_and_large_pid_lookup() {
        let mut map = ParentMap::new();
        for id in (1..=MAX_WATCHED_OWNERS as u32 + 1).rev() {
            map.insert(id, id - 1);
        }
        assert_eq!(map.insert(7, 2), Some(6));
        assert_eq!(map.get(&7), Some(&2));
        assert_eq!(map.get(&(MAX_WATCHED_OWNERS as u32 + 1)).copied(), Some(MAX_WATCHED_OWNERS as u32));
        assert_eq!(map.len(), MAX_WATCHED_OWNERS + 1);
        assert!(map.keys().copied().eq(1..=MAX_WATCHED_OWNERS as u32 + 1));
        let mut cycle = simulated_owner_chain(2);
        cycle.insert(1002, 1001);
        assert!(collect_owner_handles(&cycle, 1000, 1000, r"C:\Windows", |id| Ok(simulated_owner(id))).is_err());
    }

    #[test]
    fn call_local_parent_facts_own_the_exact_native_object_through_drop_and_refusal() {
        unsafe {
            let current = GetCurrentProcess();
            let duplicate = || {
                let mut handle = std::ptr::null_mut();
                assert_ne!(DuplicateHandle(current, current, current, &mut handle, 0, 0, DUPLICATE_SAME_ACCESS), 0);
                handle
            };
            let id = GetCurrentProcessId();
            let started = started_at(current).expect("current creation time");
            let handle = duplicate();
            let facts = ParentFacts::from_opened(id, started, handle).expect("a held process of the same age");
            assert_eq!(facts.handle, handle);
            assert_eq!(GetProcessId(facts.handle), id);
            assert_eq!(facts.started, started);
            assert_eq!(facts.image, image_path_of_handle(handle));
            assert_eq!(WaitForSingleObject(handle, 0), WAIT_TIMEOUT);
            drop(facts);
            assert_eq!(WaitForSingleObject(handle, 0), WAIT_FAILED);
            let later_object = duplicate();
            assert!(ParentFacts::from_opened(id, started - 1, later_object).is_err());
            assert_eq!(WaitForSingleObject(later_object, 0), WAIT_FAILED, "refusal closes its original owned handle");
        }
    }

    #[test]
    fn console_levels_use_only_the_admitted_original_owner_handles() {
        let map = simulated_owner_chain(3);
        let mut levels = Some(SmallList::new());
        let owners = collect_owner_handles_with_levels(&map, 1000, 1000, r"C:\Windows", |id| {
            let mut facts = simulated_owner(id);
            if id == 1002 { facts.image.clear(); }
            if id == 1003 { facts.image = r"C:\Windows\explorer.exe".into(); facts.waitable = false; }
            Ok(facts)
        }, &mut levels).expect("the verified system boundary is not a required outer owner");
        let levels = levels.unwrap();
        assert_eq!(owners, vec![1001usize as Handle, 1002usize as Handle]);
        assert_eq!(levels.iter().map(|level| level.handle).collect::<Vec<_>>(), owners.to_vec());
        assert_eq!(levels.iter().map(|level| level.id).collect::<Vec<_>>(), vec![1001, 1002]);
        assert!(!levels[0].recheck_boundary);
        assert!(levels[1].recheck_boundary, "unreadable images are retried on their retained object");
        let mut boundary = Some(SmallList::new());
        let _ = collect_owner_handles_with_levels(&map, 1000, 1000, r"C:\Windows", |id| {
            let mut facts = simulated_owner(id);
            facts.image = r"C:\Windows\explorer.exe".into();
            Ok(facts)
        }, &mut boundary).expect("a direct system-boundary parent is still watched");
        assert!(boundary.unwrap()[0].boundary, "console cleanup stops before that boundary");
        let mut capped = Some(SmallList::new());
        let all = collect_owner_handles_with_levels(&simulated_owner_chain(65), 1000, 1000, r"C:\Windows", |id| Ok(simulated_owner(id)), &mut capped).unwrap();
        assert_eq!(all.len(), 65, "console storage never truncates required owner monitoring");
        assert_eq!(capped.unwrap().len(), 64, "only console cleanup's bounded scope is recorded");

    }

    #[test]
    fn empty_job_accounting_does_not_skip_an_unobserved_original_child() {
        struct Event(Handle);
        impl Drop for Event { fn drop(&mut self) { unsafe { CloseHandle(self.0) }; } }
        let event = Event(unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) });
        assert!(!event.0.is_null());
        // This proxy deliberately presents the disputed ordering: native Job
        // accounting is already zero while the retained object's wait is 258.
        let fake_job_active_processes = 0;
        assert_eq!(fake_job_active_processes, 0);
        assert_eq!(unsafe { WaitForSingleObject(event.0, 0) }, WAIT_TIMEOUT);
        let calls = std::cell::Cell::new(0);
        let signaled = std::cell::Cell::new(false);
        let status = observe_child_cleanup(event.0, false, 5000, |child, milliseconds| {
            calls.set(calls.get() + 1);
            assert_eq!(child, event.0, "use the exact retained object, not its PID or a replacement");
            assert_eq!(milliseconds, 5000, "the existing cleanup bound is unchanged");
            signaled.set(unsafe { SetEvent(event.0) } != 0);
            unsafe { WaitForSingleObject(child, milliseconds) }
        });
        assert!(signaled.get());
        assert_eq!(status, Some(WAIT_OBJECT_0), "the terminal result comes from the native wait");
        assert_eq!(calls.get(), 1, "Job zero cannot elide the original-handle wait");
        assert_eq!(observe_child_cleanup(event.0, true, 5000, |_, _| {
            calls.set(calls.get() + 1); WAIT_FAILED
        }), None, "an already-observed natural child exit retains its no-extra-wait fast path");
        assert_eq!(observe_child_cleanup(std::ptr::null_mut(), false, 5000, |_, _| {
            calls.set(calls.get() + 1); WAIT_FAILED
        }), None, "no wait on a nonexistent child");
        assert_eq!(calls.get(), 1);
    }


    #[test]
    fn native_wait_error_survives_real_owned_timer_cleanup() {
        let event = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        assert!(!event.is_null(), "create the owned cleanup resource");
        let timer = DeadlineTimer(event);
        let raw = unsafe { WaitForSingleObject(std::ptr::null_mut(), 0) };
        let original_error = unsafe { GetLastError() };
        let result = wait_failed_after_cleanup(|| {
            drop(timer); // Real native CloseHandle, not a mock cleanup.
            // Windows cleanup may change LastError. Make that legal clobber
            // deterministic so the test does not depend on one kernel build.
            unsafe { SetLastError(0x4321) };
        });
        let retained_error = unsafe { GetLastError() };
        let closed = unsafe { WaitForSingleObject(event, 0) };
        assert_eq!(raw, WAIT_FAILED);
        assert_eq!(original_error, 6, "ERROR_INVALID_HANDLE from the original wait");
        assert_eq!(result, raw, "keep the exact failed native wait status");
        assert_eq!(retained_error, original_error, "cleanup must not replace the original API error");
        assert_eq!(closed, WAIT_FAILED, "the real owned resource was still closed");
    }


    #[test]
    fn deep_owner_windows_return_the_actual_native_window_index() {
        let handles: Vec<_> = (0..OWNERS_PER_WINDOW + 2).map(|_| unsafe {
            CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null())
        }).collect();
        let all_created = handles.iter().all(|handle| !handle.is_null());
        let owner_signal = all_created && unsafe { SetEvent(*handles.last().unwrap()) } != 0;
        let owner_result = if owner_signal {
            wait_fixed(unsafe { GetTickCount64() }.saturating_add(2000), None, None, &handles)
        } else { WAIT_FAILED };
        let child_signal = all_created && unsafe { SetEvent(handles[0]) } != 0;
        let child_result = if child_signal {
            wait_fixed(unsafe { GetTickCount64() }.saturating_add(2000), None, None, &handles)
        } else { WAIT_FAILED };
        for handle in handles { if !handle.is_null() { unsafe { CloseHandle(handle) }; } }
        assert!(all_created && owner_signal && child_signal);
        assert_eq!(owner_result, 1, "owner 61 is local native index 1, never invented global index 61");
        assert_eq!(child_result, 0, "the child remains index zero in every window");
    }

    #[test]
    fn every_deep_owner_is_in_one_bounded_native_window() {
        for count in [0usize, 1, 60, 61, MAX_WATCHED_OWNERS] {
            let owners: Vec<_> = (1..=count).map(|id| id as Handle).collect();
            let mut flattened = Vec::new();
            for subset in 0..count.div_ceil(OWNERS_PER_WINDOW).max(1) {
                let window = owner_window(&owners, subset);
                assert!(window.len() + 3 <= 64);
                flattened.extend_from_slice(window);
            }
            assert_eq!(flattened, owners);
        }
    }


    #[test]
    fn native_extended_startup_and_attribute_storage_have_supported_layouts() {
        assert_eq!(std::mem::align_of::<AttributeBlock>(), 16);
        assert_eq!(std::mem::offset_of!(StartupInfoExW, startup), 0);
        assert_eq!(std::mem::offset_of!(StartupInfoExW, attribute_list), std::mem::size_of::<StartupInfoW>());
        assert_eq!(std::mem::size_of::<StartupInfoExW>(), std::mem::size_of::<StartupInfoW>() + std::mem::size_of::<Handle>());
        if std::mem::size_of::<Handle>() == 8 {
            assert_eq!(std::mem::size_of::<StartupInfoW>(), 104);
            assert_eq!(std::mem::size_of::<StartupInfoExW>(), 112);
        }
    }

    #[test]
    fn the_created_child_is_already_in_its_private_job_before_its_first_resume() {
        use std::cell::Cell;
        use std::rc::Rc;
        let _cli_owner = crate::diagnostics::ProcessOwnership::begin();
        let folder = std::env::temp_dir().join(format!("process-tree-guard-atomic-create-{}-{}", std::process::id(), unsafe { GetCurrentThreadId() }));
        std::fs::create_dir_all(&folder).expect("create the ordinary extension audit folder");
        // Each observation is recorded without asserting or changing process
        // state. Normal guard supervision and cleanup finish before any assert.
        for (label, expected, extendable) in [("fixed", 7u32, false), ("fixed-dword", u32::MAX, false), ("extendable", 7u32, true)] {
            let seen = Rc::new(Cell::new(None));
            let recorder = seen.clone();
            AFTER_CREATE.with(|hook| *hook.borrow_mut() = Some(Box::new(move |job, child| {
                let mut child_member = 0;
                let mut host_member = 0;
                let before = unsafe { WaitForSingleObject(child, 0) };
                let child_call = unsafe { IsProcessInJob(child, job, &mut child_member) };
                let host_call = unsafe { IsProcessInJob(GetCurrentProcess(), job, &mut host_member) };
                let after = unsafe { WaitForSingleObject(child, 0) };
                recorder.set(Some((before, child_call, child_member, host_call, host_member, after)));
            })));
            let log = folder.join(format!("{}.log", label));
            let mode = RunMode { extendable: extendable.then(|| Settings {
                max_seconds: DEFAULT_MAX_EXTENSION, log_path: Some(std::sync::Arc::from(log.as_path())), exception: None,
            }), library: !extendable, ..RunMode::default() };
            let command = format!("exit /b {}", if expected == u32::MAX { "-1" } else { "7" });
            let argv = ["cmd.exe".into(), "/d".into(), "/c".into(), command];
            let outcome = run_job_with(&argv, 5.0, None, false, &mode);
            AFTER_CREATE.with(|hook| *hook.borrow_mut() = None);
            let observation = seen.get();
            // Remove the test's audit file before assertions can fail.
            let _ = std::fs::remove_file(log);
            assert_eq!(outcome.expect("normal guard execution completes"), expected, "{}", label);
            assert_eq!(observation, Some((WAIT_TIMEOUT, 1, 1, 1, 0, WAIT_TIMEOUT)),
                "{}: the same live child is already in the Job and the calling host is outside it", label);
        }
        let _ = std::fs::remove_dir(&folder);
    }


    #[test]
    fn the_image_of_a_process_keeps_its_letter_case_for_the_prompt_and_is_lower_case_for_the_rest() {
        use std::os::windows::io::AsRawHandle;
        // A process that is started here has a system folder path, which the system writes with capitals.
        let mut child = std::process::Command::new("cmd").args(["/d", "/c", "ping -n 6 127.0.0.1 >nul"])
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).spawn().expect("start a process");
        let path = image_path_of_handle(child.as_raw_handle() as Handle);
        let by_identifier = image_path_of_id(child.id());
        let _ = child.kill();
        let _ = child.wait();
        assert!(path.to_ascii_lowercase().ends_with("\\cmd.exe"), "{}", path);
        assert_ne!(path, path.to_ascii_lowercase(), "the path is as the system gives it: {}", path);
        assert_eq!(path, by_identifier);
        // The parent of this process: the path keeps its case; the old name is the same path in lower case.
        let parent = parent_image_path();
        assert!(!parent.is_empty() && parent_image() == parent.to_ascii_lowercase(), "{} {}", parent, parent_image());
        // An unreadable process has no image.
        assert_eq!(image_path_of_id(0xffff_fff0), "");
    }

    #[test]
    fn the_walk_ends_at_the_programs_at_their_system_paths_and_goes_past_a_copy_under_the_same_name() {
        let windows = windows_folder();
        assert!(windows.len() > 3 && !windows.ends_with('\\'), "{}", windows);
        // The files of the system end it at their system paths, whatever the letter case.
        assert!(ends_the_walk("explorer.exe", &format!("{}\\explorer.exe", windows), &windows));
        assert!(ends_the_walk("svchost.exe", &format!("{}\\System32\\svchost.exe", windows.to_ascii_uppercase()), &windows));
        for name in ["services.exe", "wininit.exe", "winlogon.exe", "svchost.exe", "smss.exe"] {
            assert!(ends_the_walk(name, &format!("{}\\System32\\{}", windows, name), &windows), "{}", name);
        }
        // Another folder, a folder whose name only begins like the Windows folder, a folder under the Windows folder that is not the
        // system path (some of them can be written to by a standard user), the system path of another program, a name that is not one
        // of the six, and a path that cannot be read do not end it.
        assert!(!ends_the_walk("explorer.exe", "C:\\Tools\\explorer.exe", &windows));
        assert!(!ends_the_walk("explorer.exe", &format!("{}2\\explorer.exe", windows), &windows));
        assert!(!ends_the_walk("explorer.exe", &format!("{}\\Temp\\explorer.exe", windows), &windows));
        assert!(!ends_the_walk("svchost.exe", &format!("{}\\Tasks\\svchost.exe", windows), &windows));
        assert!(!ends_the_walk("smss.exe", &format!("{}\\System32\\spool\\drivers\\color\\smss.exe", windows), &windows));
        assert!(!ends_the_walk("services.exe", &format!("{}\\explorer.exe", windows), &windows));
        assert!(!ends_the_walk("explorer.exe", &format!("{}\\System32\\explorer.exe", windows), &windows));
        assert!(!ends_the_walk("svchost.exe", &format!("{}\\svchost.exe", windows), &windows));
        assert!(!ends_the_walk("cmd.exe", &format!("{}\\System32\\cmd.exe", windows), &windows));
        assert!(!ends_the_walk("explorer.exe", "", &windows));
        assert!(!ends_the_walk("explorer.exe", &format!("{}\\explorer.exe", windows), ""));
        // A copy of an ordinary program under the name of the shell, as any process of the user can make, is walked past.
        let folder = std::env::temp_dir().join(format!("ptg-walk-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("make a folder");
        let copy = folder.join("explorer.exe");
        let system = format!("{}\\System32\\cmd.exe", windows);
        std::fs::copy(&system, &copy).expect("copy a program");
        let mut child = std::process::Command::new(&copy).args(["/d", "/c", "ping -n 6 127.0.0.1 >nul"])
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).spawn().expect("start the copy");
        let image = image_path_of_id(child.id());
        let ends = ends_the_walk("explorer.exe", &image, &windows);
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&folder);
        assert!(image.to_ascii_lowercase().ends_with("\\explorer.exe") && !ends, "{} {}", image, ends);
    }

    fn simulated_owner_chain(count: usize) -> ParentMap {
        let mut map = ParentMap::new();
        for index in 0..=count {
            let id = 1000 + index as u32;
            map.insert(id, if index == count { 0 } else { id + 1 });
        }
        map
    }

    fn simulated_owner(id: u32) -> OwnerObservation {
        OwnerObservation {
            handle: id as usize as Handle,
            started: Some(2000u64.saturating_sub(id as u64)),
            image: format!(r"C:\tools\worker-{}.exe", id),
            waitable: true,
        }
    }

    #[test]
    fn automatic_owner_walk_passes_a_fake_explorer_and_uses_the_verified_handles() {
        let map = simulated_owner_chain(3);
        let mut opened = Vec::new();
        let owners = collect_owner_handles(&map, 1000, 1000, r"C:\Windows", |id| {
            opened.push(id);
            let mut facts = simulated_owner(id);
            if id == 1002 { facts.image = r"C:\tools\explorer.exe".into(); }
            Ok(facts)
        }).expect("a copy of explorer is a required owner, not a boundary");
        assert_eq!(opened, vec![1001, 1002, 1003]);
        assert_eq!(owners, opened.iter().map(|&id| id as usize as Handle).collect::<Vec<_>>(),
            "the verified handles themselves are returned; identifiers are never reopened");
        let owners = collect_owner_handles(&map, 1000, 1000, r"C:\Windows", |id| {
            let mut facts = simulated_owner(id);
            if id == 1002 { facts.image = r"C:\Windows\explorer.exe".into(); facts.waitable = false; }
            Ok(facts)
        }).expect("a verified system boundary is excluded after a required owner");
        assert_eq!(owners, vec![1001usize as Handle]);
    }

    #[test]
    fn automatic_owner_walk_covers_more_than_32_and_refuses_a_continuing_chain_at_the_wait_limit() {
        for count in [33, MAX_WATCHED_OWNERS] {
            let owners = collect_owner_handles(&simulated_owner_chain(count), 1000, 1000, r"C:\Windows", |id| Ok(simulated_owner(id)))
                .expect("every owner fits in the wait set");
            assert_eq!(owners.len(), count);
        }
        let refusal = collect_owner_handles(&simulated_owner_chain(MAX_WATCHED_OWNERS + 1), 1000, 1000, r"C:\Windows", |id| Ok(simulated_owner(id)));
        assert!(refusal.is_err(), "continuing ancestry is refused rather than silently truncated");
    }

    #[test]
    fn automatic_owner_walk_rejects_reused_or_unverifiable_handle_identity() {
        let map = simulated_owner_chain(2);
        for started in [None, Some(1001)] {
            let result = collect_owner_handles(&map, 1000, 1000, r"C:\Windows", |id| {
                let mut facts = simulated_owner(id);
                facts.started = started;
                Ok(facts)
            });
            assert!(result.is_err(), "a later or unverifiable object cannot become the first owner");
        }
        let owners = collect_owner_handles(&map, 1000, 1000, r"C:\Windows", |id| {
            let mut facts = simulated_owner(id);
            if id == 1002 { facts.started = Some(2000); }
            Ok(facts)
        }).expect("a reused outer link ends the actual live chain");
        assert_eq!(owners, vec![1001usize as Handle]);
    }

    #[test]
    fn automatic_owner_walk_refuses_an_unwatchable_parent_or_any_required_outer_owner() {
        let map = simulated_owner_chain(3);
        for denied in [1001, 1002, 1003] {
            let result = collect_owner_handles(&map, 1000, 1000, r"C:\Windows", |id| {
                let mut facts = simulated_owner(id);
                facts.waitable = id != denied;
                Ok(facts)
            });
            assert!(result.is_err(), "query-only owner {} must not disappear from coverage", denied);
        }
        assert!(collect_owner_handles(&map, 1000, 1000, r"C:\Windows", |_| Err(GuardError("access denied".into()))).is_err());
    }

    #[test]
    fn fixed_launch_budget_is_not_refreshed_after_expensive_launch_work() {
        let clock = std::cell::Cell::new(1_000u64);
        let reads = std::cell::Cell::new(0usize);
        let deadline = fixed_deadline(300, || { reads.set(reads.get() + 1); clock.get() });
        // Process creation, assignment and resume consume this already-started budget.
        clock.set(1_500);
        assert_eq!(deadline, 1_300);
        assert_eq!(reads.get(), 1);
        assert_eq!(deadline_remaining(deadline, clock.get()), None);
        let running = event(false);
        assert_eq!(wait_fixed(0, None, None, &[running]), WAIT_TIMEOUT,
            "an expired deadline is not replaced with a fresh duration on wait entry");
        unsafe { CloseHandle(running) };
    }

    #[test]
    fn fixed_deadline_planning_includes_simulated_suspend_and_ignores_system_clock_steps() {
        let deadline = 1_000u64 + 5_000;
        assert_eq!(deadline_remaining(deadline, 1_000), Some(5_000));
        assert_eq!(deadline_remaining(deadline, 5_999), Some(1));
        // A suspended interval advances GetTickCount64 even though no relative
        // wait elapsed. These injected values exercise that expiry decision.
        assert_eq!(deadline_remaining(deadline, 20_000), None);
        assert_eq!(deadline_remaining(deadline, deadline), None);
        let file_time = 134_000_000_000_000_000u64;
        assert_eq!(absolute_timer_due(file_time, 5_000), (file_time + 50_000_000) as i64);
        // A wall-clock step changes only the absolute wake source; it cannot
        // change the remaining time obtained from the interrupt-time deadline.
        for sample in [file_time - 1_000_000_000, file_time, file_time + 1_000_000_000] {
            assert!(absolute_timer_due(sample, 5_000) > 0);
            assert_eq!(deadline_remaining(deadline, 5_999), Some(1));
        }
    }

    #[test]
    fn wall_time_is_sampled_before_interrupt_remaining_across_simulated_suspend() {
        let now = std::cell::Cell::new(1_000u64);
        let file_time = 134_000_000_000_000_000u64;
        let deadline = 6_000u64;
        let plan = sampled_wait_plan(|| {
            // Simulate suspend/preemption during the wall-clock sampling phase.
            now.set(21_000);
            file_time
        }, || deadline_remaining(deadline, now.get()));
        assert_eq!(plan, None, "an expired interrupt deadline cannot arm a new post-resume delay");
        // If suspend begins after the remaining sample, the previously sampled
        // wall time still makes the absolute due instant precede resume.
        now.set(1_000);
        let (sample, remaining) = sampled_wait_plan(|| file_time, || {
            let remaining = deadline_remaining(deadline, now.get());
            now.set(21_000);
            remaining
        }).unwrap();
        assert!(absolute_timer_due(sample, remaining) < (file_time + 200_000_000) as i64);
    }

    #[test]
    fn absolute_timer_planning_keeps_bounded_rechecks_and_the_extension_backstop() {
        let file_time = 134_000_000_000_000_000u64;
        assert_eq!(absolute_timer_due(file_time, 1), (file_time + 10_000) as i64);
        assert_eq!(absolute_timer_due(file_time, 10_000), (file_time + 100_000_000) as i64);
        assert_eq!(absolute_timer_due(file_time, 86_400_000), (file_time + 100_000_000) as i64);
        assert_eq!(absolute_timer_due(file_time, wait_slice(86_400_000) as u64),
            (file_time + BACKSTOP_MS * 10_000) as i64);
        assert_eq!(absolute_timer_due(u64::MAX, u64::MAX), i64::MAX);
        // An oversized wait is refused before an operating-system wait can
        // silently discard owners or exceed MAXIMUM_WAIT_OBJECTS.
        let waits = vec![1usize as Handle; MAX_WATCHED_OWNERS + 2];
        assert_eq!(wait_with_deadline(|| Some(10_000), Some(2usize as Handle), &waits), WAIT_FAILED);
    }

    #[test]
    fn the_deadline_is_calculated_again_at_least_every_backstop_interval() {
        assert_eq!(wait_slice(60_000), BACKSTOP_MS as u32, "a long wait is cut to the backstop");
        assert_eq!(wait_slice(BACKSTOP_MS), BACKSTOP_MS as u32);
        assert_eq!(wait_slice(30), 30, "and never past the time left");
        assert_eq!(wait_slice(0), 0);
    }

    fn event(signaled: bool) -> Handle {
        let handle = unsafe { CreateEventW(std::ptr::null_mut(), 1, signaled as Bool, std::ptr::null()) };
        assert!(!handle.is_null());
        handle
    }

    #[test]
    fn a_command_that_ended_wins_over_a_deadline_that_passed() {
        let (command, owner, wake) = (event(true), event(false), event(false));
        // No time is left, as when the hold of the last holder ended with the command.
        assert_eq!(wait_until(|| None, wake, &[command, owner]), 0, "the command's exit is reported, not the deadline");
        let owner_gone = event(true);
        let command_running = event(false);
        assert_eq!(wait_until(|| None, wake, &[command_running, owner_gone]), 1, "so is the exit of an owner");
        assert_eq!(wait_until(|| None, wake, &[command_running, owner]), WAIT_TIMEOUT, "and the deadline when nothing has ended");
        // The command ending while time is left is reported the same way.
        assert_eq!(wait_until(|| Some(10_000), wake, &[command, owner]), 0);
        // A wake with time left asks again; the second answer, none left, is the deadline.
        let mut asked = 0;
        let woken = event(true);
        let status = wait_until(|| { asked += 1; if asked == 1 { Some(10_000) } else { None } }, woken, &[command_running, owner]);
        assert_eq!((status, asked), (WAIT_TIMEOUT, 2));
        for handle in [command, owner, wake, owner_gone, command_running, woken] {
            unsafe { CloseHandle(handle) };
        }
    }

    #[test]
    fn a_command_that_was_the_last_holder_and_ended_after_the_runs_own_deadline_is_not_a_timeout() {
        use std::os::windows::io::AsRawHandle;
        let quiet = || std::process::Stdio::null();
        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            assert!(!job.is_null());
            // The command holds the run for 30 seconds and ends on its own after two, a second after the run's own deadline of one.
            let mut child = std::process::Command::new("ping.exe").args(["-n", "3", "127.0.0.1"]).stdin(quiet()).stdout(quiet()).stderr(quiet()).spawn().expect("start ping.exe");
            assert_ne!(AssignProcessToJobObject(job, child.as_raw_handle() as Handle), 0, "put the command in the job");
            let extension = Extension::start(job, &Settings { max_seconds: 60.0, log_path: None, exception: None }, 1.0, None).expect("start the extension");
            extension.hold_for_test(child.id(), 30.0);
            child.wait().expect("the command ends");
            // The hold ended with the command, so the deadline is back at the one that passed long ago, and the exit is still reported.
            assert_eq!(wait_extendable(&extension, &[child.as_raw_handle() as Handle]), 0, "the command's exit wins over the deadline");
            // A command that is still running when the deadline has passed is the deadline.
            let mut running = std::process::Command::new("ping.exe").args(["-n", "60", "127.0.0.1"]).stdin(quiet()).stdout(quiet()).stderr(quiet()).spawn().expect("start ping.exe");
            assert_ne!(AssignProcessToJobObject(job, running.as_raw_handle() as Handle), 0, "put the command in the job");
            assert_eq!(wait_extendable(&extension, &[running.as_raw_handle() as Handle]), WAIT_TIMEOUT);
            running.kill().expect("end the command");
            running.wait().expect("reap the command");
            drop(extension);
            CloseHandle(job);
        }
    }

    /// The entries of an environment block, as text.
    fn entries_of(block: &[u16]) -> Vec<String> {
        block.split(|unit| *unit == 0).filter(|entry| !entry.is_empty()).map(String::from_utf16_lossy).collect()
    }

    #[test]
    fn the_environment_block_is_sorted_by_name() {
        // '(' sorts below '=', so sorting the whole entry puts ProgramFiles(x86) before ProgramFiles.
        std::env::set_var("PTG_BLOCK_B", "1");
        std::env::set_var("PTG_BLOCK_B(X86)", "2");
        std::env::set_var("ptg_block_c", "3");
        let block = environment_block("PTG_BLOCK_A", "added");
        let entries = entries_of(&block);
        assert!(block.ends_with(&[0, 0]), "the block ends with one more NUL");
        let at = |prefix: &str| entries.iter().position(|entry| entry.starts_with(prefix)).unwrap_or_else(|| panic!("{} is in the block", prefix));
        assert!(at("PTG_BLOCK_A=") < at("PTG_BLOCK_B=") && at("PTG_BLOCK_B=") < at("PTG_BLOCK_B(X86)="), "{:?}", entries);
        assert!(at("PTG_BLOCK_B(X86)=") < at("ptg_block_c="), "names compare in upper case: {:?}", entries);
        assert!(entries.contains(&"PTG_BLOCK_A=added".to_string()));
        let names: Vec<String> = entries.iter().map(|entry| entry.char_indices().skip(1).find(|(_, c)| *c == '=').map_or(entry.clone(), |(at, _)| entry[..at].to_uppercase())).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "every entry of the block is in order of its name");
        // The variable that is added replaces an inherited one of the same name, in any letter case.
        std::env::set_var("PTG_BLOCK_D", "old");
        let replaced = entries_of(&environment_block("ptg_block_d", "new"));
        assert!(replaced.contains(&"ptg_block_d=new".to_string()) && !replaced.iter().any(|entry| entry.eq_ignore_ascii_case("PTG_BLOCK_D=old")), "{:?}", replaced);
    }

    #[test]
    fn returning_extendable_run_is_refused_before_command_resolution_or_log_start() {
        let mode = RunMode { extendable: Some(Settings { max_seconds: 60.0, log_path: None, exception: None }), ..RunMode::default() };
        // Empty argv would panic at resolution if the ownership check were late.
        let error = run_job_with(&[], 1.0, None, false, &mode).expect_err("returning callers cannot own extension writers");
        assert!(error.message().contains("requires the process-owning cli_exit adapter"));
        assert!(require_process_ownership(false).is_ok(), "ordinary returning runs remain available");
        { let _owner = crate::diagnostics::ProcessOwnership::begin(); assert!(require_process_ownership(true).is_ok()); }
        assert!(require_process_ownership(true).is_err(), "ownership is scoped to the explicit process owner");
    }

    #[test]
    fn a_cap_over_the_default_that_nobody_approved_is_not_a_run() {
        let _cli_owner = crate::diagnostics::ProcessOwnership::begin();
        // Settings built by a caller that did not build the exception: the cap is over the default and nothing asks for it.
        // The refusal comes before the job is made, so the command (which would end at once) is never started.
        let argv: Vec<String> = ["cmd.exe", "/c", "exit"].iter().map(|part| part.to_string()).collect();
        for max_seconds in [DEFAULT_MAX_EXTENSION + 1.0, 86_400.0] {
            let mode = RunMode { extendable: Some(Settings { max_seconds, log_path: None, exception: None }), ..RunMode::default() };
            let error = run_job_with(&argv, 5.0, None, false, &mode).expect_err("a cap that nobody approved");
            assert!(error.message().contains("needs the approval of the signed-in user") && error.message().contains("did not start"), "{}", error);
        }
    }

    /// The settings of a run over the default cap with the audit file `log`.
    fn exception_settings(log: &std::path::Path) -> Settings {
        Settings {
            max_seconds: 86_400.0,
            log_path: Some(std::sync::Arc::from(log)),
            exception: Some(auth::Exception { method: auth::Method::Console, command: "cmd.exe".into(), timeout: 20.0 }),
        }
    }

    #[test]
    fn an_approved_exception_runs_its_command_with_the_cap_and_the_owner_that_were_opened_and_a_refused_one_never_starts_it() {
        let _cli_owner = crate::diagnostics::ProcessOwnership::begin();
        let folder = crate::test_temp::fresh_dir("process-tree-guard-engine");
        let argv: Vec<String> = ["cmd.exe", "/d", "/s", "/c"].iter().map(|part| part.to_string()).collect();
        let command = |marker: &std::path::Path| format!("echo ran> \"{}\" & exit 3", marker.display());
        let lines = |log: &std::path::Path| std::fs::read_to_string(log).unwrap_or_default().lines().map(String::from).collect::<Vec<String>>();
        // Approved: the command runs and its exit code passes through; the run has the cap that was approved, one owner (the
        // process that started the guard, opened for the prompt and kept for the run), and one approval line.
        let (marker, log) = (folder.join("approved.txt"), folder.join("approved.log"));
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let recorder = seen.clone();
        BEFORE_START.with(|hook| *hook.borrow_mut() = Some(Box::new(move |extension, owners| {
            let extension = extension.expect("an extendable run has its extension");
            // A hold of the whole cap is granted only by a run that was made with that cap.
            extension.hold_for_test(std::process::id(), 86_400.0);
            recorder.borrow_mut().push(owners);
        })));
        auth::seam::ANSWER.with(|answer| answer.set(Some(true)));
        let mode = RunMode { extendable: Some(exception_settings(&log)), ..RunMode::default() };
        let code = run_job_with(&argv, 20.0, Some(&command(&marker)), false, &mode).expect("an approved exception runs its command");
        assert_eq!(code, 3, "the exit code of the command passes through");
        assert!(marker.exists(), "the command ran");
        assert_eq!(*seen.borrow(), [1], "the run was made with the cap, and watches the one owner that the prompt opened");
        let approved = lines(&log);
        assert_eq!(approved.iter().filter(|line| line.contains("decision=exception-approved")).count(), 1, "{:?}", approved);
        assert!(approved.iter().all(|line| !line.contains("decision=exception-refused")) && approved[0].contains("requested=86400")
            && approved[0].contains("attempt=1 "), "{:?}", approved);
        // Refused: the prompt is cancelled, the run does not start, and the extension is never made.
        let (marker, log) = (folder.join("refused.txt"), folder.join("refused.log"));
        seen.borrow_mut().clear();
        auth::seam::ANSWER.with(|answer| answer.set(Some(false)));
        let mode = RunMode { extendable: Some(exception_settings(&log)), ..RunMode::default() };
        let error = run_job_with(&argv, 20.0, Some(&command(&marker)), false, &mode).expect_err("a cancelled prompt refuses the run");
        assert!(error.message().contains("exception refused") && error.message().contains("the run did not start"), "{}", error);
        assert!(!marker.exists() && seen.borrow().is_empty(), "the command never started");
        let refused = lines(&log);
        assert!(refused.len() == 1 && refused[0].contains("decision=exception-refused") && refused[0].contains("the prompt was cancelled"), "{:?}", refused);
        // Approved, and then the command cannot be started (a real program, a copy of cmd.exe, that this test holds open without
        // sharing, so the system cannot open it to start it): the record does not leave a cap approved for a command that never
        // ran, and says that it did not start. A file whose bytes are not a program is never used: Windows would try it as a
        // 16-bit program and show a dialog. Should the hold not take, the copy runs `exit 0` and the test fails without a dialog.
        let (held_program, log) = (folder.join("held-program.exe"), folder.join("not-started.log"));
        std::fs::copy(format!("{}\\System32\\cmd.exe", windows_folder()), &held_program).expect("copy a real program");
        let _held = {
            use std::os::windows::fs::OpenOptionsExt;
            std::fs::OpenOptions::new().read(true).share_mode(0).open(&held_program).expect("hold the program without sharing")
        };
        auth::seam::ANSWER.with(|answer| answer.set(Some(true)));
        let mode = RunMode { extendable: Some(exception_settings(&log)), ..RunMode::default() };
        let argv = [held_program.display().to_string(), "/d".into(), "/c".into(), "exit 0".into()];
        let error = run_job_with(&argv, 20.0, None, false, &mode).expect_err("a program held without sharing does not start");
        let not_started = lines(&log);
        assert_eq!(not_started.iter().filter(|line| line.contains("decision=exception-approved")).count(), 1, "{:?}", not_started);
        assert!(not_started.iter().any(|line| line.contains("notice=\"the approved run did not start: ") && line.contains(&error.message().replace('\\', "\\\\"))),
            "the log says that the approved run did not start, and why: {:?} {}", not_started, error);
        auth::seam::ANSWER.with(|answer| answer.set(None));
        BEFORE_START.with(|hook| *hook.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn quoting_matches_the_c_runtime_rules() {
        let cases: [(&[&str], &str); 6] = [
            (&["a", "b c"], "a \"b c\""),
            (&["a\\\"b"], "a\\\\\\\"b"),
            (&["", "x"], "\"\" x"),
            (&["c:\\dir with space\\"], "\"c:\\dir with space\\\\\""),
            (&["plain\\path"], "plain\\path"),
            (&["a b", "say \"hi\"", "c:\\x y\\"], "\"a b\" \"say \\\"hi\\\"\" \"c:\\x y\\\\\""),
        ];
        for (args, expected) in cases {
            let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            assert_eq!(list2cmdline(&owned), expected);
        }
    }
