// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Process behavior cases shared by platform-specific test drivers.

use super::common::*;

pub(super) fn g01_missing_timeout_never_runs() {
    let result = run_guard(&["--", testkit(), "print", "SHOULD_NOT_RUN"]);
    assert_eq!(result.code, 125, "{}", result.err());
    assert!(!result.out().contains("SHOULD_NOT_RUN"), "the child ran without a deadline");
}

pub(super) fn g11_to_g15_invalid_timeout() {
    let mut checks = Checks::new("invalid timeout");
    for (id, value) in [("G11", "0"), ("G12", "-1"), ("G13", "nan"), ("G14", "inf"), ("G15", "86401")] {
        let result = guarded(value, &[testkit(), "exit", "0"]);
        checks.check(id, &format!("timeout {} rejected", value), result.code == 125, format!("exit {}", result.code));
    }
    checks.finish();
}

pub(super) fn g16_streams_exit_code_and_stdin() {
    let result = run_guard_input(&["--timeout", "5", "--", testkit(), "relay", "7"], b"input-stream");
    assert_eq!(result.code, 7, "{}", result.err());
    assert!(result.out().contains("input-stream"), "stdout: {}", result.out());
    assert!(result.err().contains("error-stream"), "stderr: {}", result.err());
}

pub(super) fn g17_arguments_survive_quoting() {
    let tricky = ["a b", "quote\"inside", "trail\\", "", "back\\\\slash \"mix\"", "\u{fc}n\u{ef}code"];
    let mut args = vec!["--timeout", "5", "--", testkit(), "args-json"];
    args.extend_from_slice(&tricky);
    let result = run_guard(&args);
    assert_eq!(result.code, 0, "{}", result.err());
    let decoded = Json::parse(result.out().trim()).and_then(|j| j.strings());
    let expected: Vec<String> = tricky.iter().map(|s| s.to_string()).collect();
    assert_eq!(decoded, Some(expected), "stdout: {}", result.out());
}

pub(super) fn g18_timeout() {
    let result = guarded(".3", &[testkit(), "sleep", "10"]);
    assert_eq!(result.code, 124, "{}", result.err());
    assert!(result.err().contains("PROCESS_TIMEOUT"), "stderr: {}", result.err());
    if timing_strict() {
        assert!(result.elapsed.as_secs_f64() < 3.0, "took {:?}", result.elapsed);
    }
}

pub(super) fn g19_g20_descendant_killed_on_timeout_and_normal_exit() {
    let mut checks = Checks::new("descendants");
    // The parent starts a sleeper child that records its identifier. On timeout the parent
    // holds well past the deadline; on a normal exit it leaves once the marker exists.
    for (id, mode, expected) in [("G19", "timeout", 124u32), ("G20", "normal", 0u32)] {
        let dir = TempDir::new("g19-");
        let marker = dir.join("pid.txt");
        let marker_text = text(&marker);
        let result = if mode == "timeout" {
            guarded("5", &[testkit(), "spawn", "0", "30", testkit(), "mark", &marker_text, "20"])
        } else {
            guarded("30", &[testkit(), "spawn-marked", "0", &marker_text, "15", testkit(), "mark", &marker_text, "20"])
        };
        let child = read_id(&marker);
        checks.check(id, &format!("{} exit code", mode), result.code == expected, format!("exit {} {}", result.code, result.err().trim()));
        checks.check(id, &format!("{} marker written", mode), child.is_some(), "");
        checks.check(id, &format!("{} child dead on return", mode), child.map_or(false, |c| !alive(c)), format!("child {:?}", child));
    }
    checks.finish();
}
