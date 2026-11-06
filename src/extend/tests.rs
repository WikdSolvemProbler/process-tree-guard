// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

    use super::*;


    use crate::sys::windows::{

        AssignProcessToJobObject, CloseHandle, CreateEventW, CreateJobObjectW, CreateNamedPipeW,

        FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, GetCurrentProcess,

        GetSystemTimePreciseAsFileTime, Handle, INVALID_HANDLE, OpenProcessToken,

        PIPE_ACCESS_DUPLEX, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE,

        SetEvent, TOKEN_QUERY, wide,

    };
    /// Every process of the table is running.
    fn live(_: RateKey) -> bool {
        true
    }

    #[test]
    fn time_is_written_in_universal_time() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso8601(1_700_000_000_000), "2023-11-14T22:13:20.000Z");
        assert_eq!(iso8601(951_782_400_123), "2000-02-29T00:00:00.123Z");
        assert_eq!(iso8601(4_102_444_799_999), "2099-12-31T23:59:59.999Z");
    }

    #[test]
    fn log_values_cannot_end_a_line_or_start_a_field() {
        assert_eq!(escape("plain.exe"), "plain.exe");
        assert_eq!(escape("a\"b\\c"), "a\\\"b\\\\c");
        let hostile = "x\r\nPROCESS_TREE_GUARD_EXTENSION: pid=1\t\u{85}\u{2028}\u{7f}\0end";
        let escaped = escape(hostile);
        assert!(escaped.chars().all(|c| !c.is_control() && c != '\u{2028}' && c != '\u{2029}'), "{}", escaped);
        assert!(escaped.contains("\\u{d}\\u{a}PROCESS_TREE_GUARD_EXTENSION"));
        let line = log_line(0, &Entry { pid: 7, image: hostile, requested: Some(60.0), granted: None, reason: "a\nb" });
        assert_eq!(line.lines().count(), 1, "{}", line);
        assert!(line.starts_with("PROCESS_TREE_GUARD_EXTENSION: time=1970-01-01T00:00:00.000Z pid=7 image=\""));
        assert!(line.ends_with("requested=60 granted=0 decision=refused reason=\"a\\u{a}b\""), "{}", line);
    }

    #[test]
    fn log_values_cannot_change_how_the_rest_of_the_line_is_shown() {
        // A right-to-left override without its end would show the rest of the line reversed.
        let spoof = "ok\u{202e}detnarg\u{202c}.exe";
        let escaped = escape(spoof);
        assert_eq!(escaped, "ok\\u{202e}detnarg\\u{202c}.exe");
        for c in ['\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202d}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
            '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}', '\u{061c}', '\u{00ad}', '\u{e0001}', '\u{e0041}'] {
            assert_eq!(escape(&c.to_string()), format!("\\u{{{:x}}}", c as u32), "U+{:04X}", c as u32);
        }
        for c in ['a', 'Z', '.', ' ', '\u{e9}', '\u{4e2d}', '\u{1f600}', '\u{200a}', '\u{2010}', '\u{2065}', '\u{fe0f}'] {
            assert_eq!(escape(&c.to_string()), c.to_string(), "U+{:04X} is shown as it is", c as u32);
        }
        let line = log_line(0, &Entry { pid: 7, image: spoof, requested: Some(5.0), granted: None, reason: "r" });
        assert!(!line.chars().any(|c| is_format(c) || c.is_control()), "{}", line);
    }

    #[test]
    fn the_log_records_the_seconds_asked_exactly_and_in_bounded_text() {
        for (value, text) in [
            (60.0, "60"), (1.5, "1.5"), (0.25, "0.25"), (0.0, "0"), (-5.0, "-5"), (86401.0, "86401"), (1e9, "1000000000"),
            (1e15, "1e15"), (1e19, "1e19"), (1e300, "1e300"), (1.8446744073709552e19, "1.8446744073709552e19"), (f64::MAX, "1.7976931348623157e308"),
            (5e-324, "5e-324"), (1e-7, "1e-7"), (1e-6, "0.000001"), (-1e300, "-1e300"),
        ] {
            assert_eq!(log_seconds(value), text);
            assert!(log_seconds(value).len() <= 24, "{}", value);
            let line = log_line(0, &Entry { pid: 1, image: "x", requested: Some(value), granted: None, reason: "r" });
            assert!(line.contains(&format!(" requested={} granted=0 ", text)), "{}", line);
        }
        // A number the log writes can be read back as the number that was asked for.
        for value in [1e300, 18446744073709551616.0, 5e-324, 1e19, 0.1 + 0.2] {
            assert_eq!(log_seconds(value).parse::<f64>().ok(), Some(value));
        }
        assert_eq!(log_seconds(f64::INFINITY), "invalid");
        assert_eq!(log_seconds(f64::NAN), "invalid");
    }

    #[test]
    fn log_lines_name_every_field() {
        let granted = log_line(1_700_000_000_000, &Entry { pid: 42, image: "kit.exe", requested: Some(1.5), granted: Some(1.5), reason: "within the cap" });
        assert_eq!(granted, "PROCESS_TREE_GUARD_EXTENSION: time=2023-11-14T22:13:20.000Z pid=42 image=\"kit.exe\" requested=1.5 granted=1.5 decision=granted reason=\"within the cap\"");
        let odd = log_line(0, &Entry { pid: 1, image: "x", requested: None, granted: None, reason: "r" });
        assert!(odd.contains("requested=unknown granted=0 decision=refused"));
        let bad = log_line(0, &Entry { pid: 1, image: "x", requested: Some(f64::NAN), granted: None, reason: "r" });
        assert!(bad.contains("requested=invalid"));
    }

    #[test]
    fn the_line_for_an_exception_has_decisions_of_its_own_and_the_attempt_and_escapes_what_it_is_given() {
        let record = |account, command, started_by, requested, approved, attempt, reason| ExceptionRecord {
            unix_ms: 1_700_000_000_000, pid: 42, image: "guard.exe", account, command, method: "console", timeout: 20.0, started_by, requested, approved, attempt, reason,
        };
        let approved = exception_line(&record("PC\\me", "C:\\tools\\run.exe --all", "C:\\Apps\\host.exe", 86_400.0, true, 2, "a cap of 24 hours was approved"));
        assert_eq!(approved, "PROCESS_TREE_GUARD_EXTENSION: time=2023-11-14T22:13:20.000Z pid=42 image=\"guard.exe\" account=\"PC\\\\me\" \
            command=\"C:\\\\tools\\\\run.exe --all\" method=\"console\" timeout=20 started_by=\"C:\\\\Apps\\\\host.exe\" requested=86400 hours=24 granted=0 \
            decision=exception-approved attempt=2 reason=\"a cap of 24 hours was approved\"");
        let refused = exception_line(&record("", "", "", 21_600.5, false, 0, "no \"one\"\r\nis here"));
        assert!(refused.contains("requested=21600.5 hours=6.01 granted=0 decision=exception-refused attempt=0 reason=\"no \\\"one\\\"\\u{d}\\u{a}is here\""), "{}", refused);
        assert!(refused.contains("started_by=\"\" requested="), "a starter that is not known is an empty field: {}", refused);
        assert!(exception_line(&record("a", "c", "s", 25_000.0, false, 1, "r")).contains("requested=25000 hours=6.95 granted=0"), "hours are rounded up to two decimals");
        let desktop = exception_line(&ExceptionRecord { method: "desktop", timeout: 0.5, ..record("a", "c", "s", 86_400.0, false, 1, "r") });
        assert!(desktop.contains("method=\"desktop\" timeout=0.5 started_by=\"s\" requested=86400"), "{}", desktop);
        for (seconds, text) in [(86_400.0, "24"), (3_600.0, "1"), (23_400.0, "6.5"), (25_000.0, "6.95"), (21_601.0, "6.01"), (21_600.0, "6"), (21_600.5, "6.01"),
            (21_600.000_000_1, "6.01"), (1_800.0, "0.5"), (86_396.4, "24"), (44_100.0, "12.25")] {
            assert_eq!(hours_figure(seconds), text, "{}", seconds);
        }
        // Never smaller than the cap, so a figure can only overstate what a person approves.
        for seconds in (21_601..86_400).step_by(37) {
            let shown: f64 = hours_figure(seconds as f64).parse().unwrap_or(0.0);
            assert!(shown * 3600.0 + 1e-6 >= seconds as f64 && shown > 6.0, "{} {}", seconds, shown);
        }
        assert!(!refused.chars().any(|c| c.is_control()), "a line is one line of printable text: {:?}", refused);
        let odd_image = exception_line(&ExceptionRecord { image: "k\u{2028}it \"x\"=1.exe", ..record("a\u{202e}\"b", "c\r\n\"d\"", "s\u{85}\"t\"", 86_400.0, false, 1, "r") });
        assert!(odd_image.contains("image=\"k\\u{2028}it \\\"x\\\"=1.exe\""), "{}", odd_image);
        assert!(odd_image.contains("account=\"a\\u{202e}\\\"b\" command=\"c\\u{d}\\u{a}\\\"d\\\"\" method="), "the account and the command are escaped like the rest: {}", odd_image);
        assert!(odd_image.contains("started_by=\"s\\u{85}\\\"t\\\"\" requested="), "and so is the starter: {}", odd_image);
        assert!(!odd_image.chars().any(|c| c.is_control() || is_format(c)), "{:?}", odd_image);
    }

    #[test]
    fn the_user_of_a_token_is_read_for_the_check_of_an_exception_and_the_log_is_opened_before_the_job() {
        use std::os::windows::io::AsRawHandle;
        let mut token: Handle = std::ptr::null_mut();
        assert_ne!(unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }, 0);
        let from_token = token_user_sid(token);
        unsafe { CloseHandle(token) };
        let ours = current_user_sid().expect("the user of this process");
        assert_eq!(from_token.as_ref().ok(), Some(&ours));
        assert!(ours.starts_with("S-1-"), "{}", ours);
        // A handle that is not a token has no user: the check of an exception cannot be passed with one.
        let file = std::fs::File::open(std::env::current_exe().expect("this program")).expect("open this program");
        assert!(token_user_sid(file.as_raw_handle() as Handle).is_err());
        // The log of a run is opened before the job, and refuses a file that is not on a disk.
        let device = Settings { max_seconds: 86_400.0, log_path: Some(std::sync::Arc::from(PathBuf::from("NUL").as_path())), exception: None };
        assert!(matches!(open_log(&device), Err(GuardError(m)) if m.contains("must be a regular file")));
    }

    #[test]
    fn requests_and_replies_round_trip_in_fixed_sizes() {
        for (target, token, seconds) in [(Target::Caller, 0, 60.0), (Target::Parent, 0, 0.25), (Target::Release, 0, 0.0), (Target::Caller, 7, 5.0), (Target::Release, (1 << TOKEN_BITS) - 1, 0.0)] {
            let bytes = encode_request(target, token, seconds);
            assert_eq!(bytes.len(), REQUEST_SIZE);
            assert_eq!(decode_request(&bytes), Some((target, token, seconds)));
        }
        assert_eq!(decode_request(&encode_request(Target::Caller, u64::MAX, 1.0)).map(|(_, token, _)| token), Some((1 << TOKEN_BITS) - 1), "a token is 56 bits and never changes the target");
        let mut bytes = encode_request(Target::Caller, 0, 1.0);
        assert_eq!(decode_request(&bytes[..23]), None);
        bytes[0] ^= 1;
        assert_eq!(decode_request(&bytes), None);
        let mut bytes = encode_request(Target::Caller, 0, 1.0);
        bytes[8] = 4;
        assert_eq!(decode_request(&bytes), None);
        assert_eq!(decode_request(&[]), None);

        let granted = encode_reply(&Reply::Granted { seconds: 5.0, until_unix_ms: 123, held: 77 });
        assert_eq!(granted.len(), REPLY_SIZE);
        assert!(matches!(decode_reply(&granted), Some(Reply::Granted { seconds, until_unix_ms: 123, held: 77 }) if seconds == 5.0));
        for code in 1..=13u64 {
            let refusal = Refusal::from_code(code).expect("a code in range");
            let bytes = encode_reply(&Reply::Refused { refusal, cap: 9.0 });
            assert!(matches!(decode_reply(&bytes), Some(Reply::Refused { refusal: r, cap }) if r == refusal && cap == 9.0));
        }
        assert!(Refusal::from_code(0).is_none() && Refusal::from_code(14).is_none());
        assert!(decode_reply(&granted[..39]).is_none());
    }

    #[test]
    fn the_deadline_is_the_latest_of_its_own_and_the_live_holds() {
        // Time is in units of one second: the boss has 2, a helper holds until 6 and another until 4.
        assert_eq!(latest(2, [6, 4].into_iter()), 6);
        assert_eq!(latest(2, [4].into_iter()), 4, "the helper that held until 6 exited, so the deadline falls to 4");
        assert_eq!(latest(2, std::iter::empty()), 2, "no live hold: the run's own deadline");
        assert_eq!(latest(9, [4].into_iter()), 9, "a hold earlier than the run's own deadline does not shorten it");
    }

    #[test]
    fn requests_are_limited_per_process_in_a_window() {
        let mut limiter = Limiter::default();
        let (key, who) = ((10, 100), (10, "worker.exe"));
        for n in 0..RATE_LIMIT {
            assert_eq!(limiter.admit(key, who, n as u64, &live), Admit::Allowed, "request {}", n);
        }
        assert_eq!(limiter.admit(key, who, 500, &live), Admit::First, "the next request in the window is refused, and is the one that is logged");
        assert_eq!(limiter.admit(key, who, 600, &live), Admit::Counted);
        assert_eq!(limiter.admit(key, who, 700, &live), Admit::Counted);
        assert_eq!(limiter.admit((11, 100), (11, "other.exe"), 500, &live), Admit::Allowed, "another process has its own share");
        assert!(limiter.summaries.is_empty(), "nothing is summarised while the window is open");
        assert_eq!(limiter.admit(key, who, RATE_WINDOW_MS + RATE_LIMIT as u64 + 1, &live), Admit::Allowed, "the window has passed");
        assert_eq!(limiter.summaries.len(), 1);
        let summary = &limiter.summaries[0];
        assert_eq!((summary.pid, summary.image.as_str(), summary.count), (10, "worker.exe", 2), "the two refusals after the logged one");
    }

    #[test]
    fn a_live_process_is_never_forgotten_to_make_room_and_clients_outside_the_job_share_one_allowance() {
        let mut limiter = Limiter::default();
        for n in 0..MAX_RATE_KEYS as u32 {
            assert_eq!(limiter.admit((1000 + n, 1), (1000 + n, "p.exe"), 1, &live), Admit::Allowed);
        }
        assert_eq!(limiter.admit((1000, 1), (1000, "p.exe"), 1, &live), Admit::Allowed, "a process in the table keeps its share");
        assert_eq!(limiter.admit((9000, 1), (9000, "late.exe"), 2, &live), Admit::Full, "a new process is refused, and nobody is evicted");
        assert!(limiter.rates.len() == MAX_RATE_KEYS && limiter.rates.iter().any(|rate| rate.key == (1000, 1)));
        assert_eq!(limiter.admit((9001, 1), (9001, "later.exe"), 3, &live), Admit::Full, "and a look at the table that finds everyone live is not repeated at once");
        assert_eq!(limiter.unproductive, Some(2));
        let mut outside = Limiter::default();
        for n in 0..RATE_LIMIT as u32 {
            assert_eq!(outside.admit(OUTSIDE, (5000 + n, "x.exe"), n as u64, &live), Admit::Allowed, "request {} from a different process", n);
        }
        assert_eq!(outside.admit(OUTSIDE, (7777, "y.exe"), 100, &live), Admit::First, "all the clients outside the job draw on one allowance");
        assert_eq!(outside.rates.len(), 1);
    }

    #[test]
    fn a_process_that_has_exited_is_forgotten_when_the_table_is_full() {
        // Short-lived processes, each asking once, far more of them than the table holds in one window.
        let exited = |key: RateKey| key.0 >= 100_000;
        let mut limiter = Limiter::default();
        let mut refused = 0;
        for n in 0..3 * MAX_RATE_KEYS as u32 {
            // Every process is gone by the time the next one asks, except the first thousand, which stay.
            let pid = if n < 1000 { 1000 + n } else { 100_000 + n };
            if limiter.admit((pid, 1), (pid, "p.exe"), n as u64, &|key| !exited(key)) != Admit::Allowed {
                refused += 1;
            }
        }
        assert_eq!(refused, 0, "a process that is gone takes no room from the ones that come after it");
        assert!(limiter.rates.len() <= MAX_RATE_KEYS);
        assert!(limiter.rates.iter().filter(|rate| rate.key.0 < 100_000).count() == 1000, "the live ones were never forgotten");
        // Everyone alive: the table stays full, and the refusal is the table's own, not the rate's.
        let mut full = Limiter::default();
        for n in 0..MAX_RATE_KEYS as u32 {
            assert_eq!(full.admit((n + 1, 1), (n + 1, "p.exe"), 5, &live), Admit::Allowed);
        }
        assert_eq!(full.admit((70_000, 1), (70_000, "q.exe"), 5, &live), Admit::Full);
        // The first of them exits: its place goes to the next process that asks, with no wait.
        assert_eq!(full.admit((70_000, 1), (70_000, "q.exe"), 5 + FORGET_MS, &|key| key.0 != 1), Admit::Allowed);
        assert_eq!(full.rates.len(), MAX_RATE_KEYS);
        // A client outside the job is never forgotten as a dead process.
        let mut outside = Limiter::default();
        outside.admit(OUTSIDE, (9, "x.exe"), 1, &live);
        for n in 0..MAX_RATE_KEYS as u32 - 1 {
            outside.admit((n + 1, 1), (n + 1, "p.exe"), 1, &live);
        }
        assert_eq!(outside.admit((70_000, 1), (70_000, "q.exe"), 2, &|_| false), Admit::Allowed);
        assert!(outside.rates.iter().any(|rate| rate.key == OUTSIDE));
        assert_eq!(outside.rates.len(), 2, "the table was emptied of the exited processes, and the outside key stayed");
    }

    #[test]
    fn the_refusal_for_a_full_table_is_not_the_rate_limits() {
        assert_ne!(Refusal::Crowded.describe(1.0), Refusal::RateLimited.describe(1.0));
        assert!(Refusal::Crowded.describe(1.0).contains("too many processes"));
        let mut limiter = Limiter::default();
        for n in 0..MAX_RATE_KEYS as u32 {
            limiter.admit((n + 1, 1), (n + 1, "p.exe"), 1, &live);
        }
        let mut state = State::new(1_000_000, 60.0);
        state.limiter = limiter;
        state.alive = live;
        let late = this_process();
        let key = (late.id, late.created);
        assert_eq!(state.grant(2, None, 5.0, late, 0, Some((key, 4242, "late.exe"))), Err(Refusal::Crowded.into()));
    }

    #[test]
    fn refusal_lines_are_a_budget_and_the_rest_is_one_line() {
        let mut budget = Budget::default();
        for n in 0..REFUSAL_LINES {
            assert!(budget.allow(n as u64), "line {}", n);
        }
        assert!(!budget.allow(100) && !budget.allow(200), "over the budget the lines are counted");
        assert_eq!(budget.overflow(300, false), None, "the window is still open");
        assert_eq!(budget.overflow(RATE_WINDOW_MS + REFUSAL_LINES as u64, false), Some(2), "one line for the two that were withheld");
        assert_eq!(budget.overflow(RATE_WINDOW_MS + REFUSAL_LINES as u64, false), None);
        assert!(budget.allow(RATE_WINDOW_MS + 1000), "a new window starts afresh");
        let mut ending = Budget::default();
        for _ in 0..REFUSAL_LINES + 3 {
            ending.allow(5);
        }
        assert_eq!(ending.overflow(6, true), Some(3), "at the end of the run what was withheld is told at once");
    }

    #[test]
    fn counted_refusals_become_one_line_when_their_window_ends() {
        let mut state = State::new(0, 60.0);
        for n in 0..RATE_LIMIT {
            state.limiter.admit((7, 1), (7, "w.exe"), n as u64, &live);
        }
        for _ in 0..5 {
            state.limiter.admit((7, 1), (7, "w.exe"), 100, &live);
        }
        assert!(state.notices(200, 0, false).is_empty(), "the window has not passed");
        let lines = state.notices(RATE_WINDOW_MS + 100, 1_700_000_000_000, false);
        assert_eq!(lines.len(), 1, "{:?}", lines);
        assert!(lines[0].contains(" pid=7 image=\"w.exe\"") && lines[0].contains("decision=refused") && lines[0].contains("4 more refused"), "{}", lines[0]);
        let mut ending = State::new(0, 60.0);
        for n in 0..RATE_LIMIT + 3 {
            ending.limiter.admit((8, 1), (8, "v.exe"), n as u64, &live);
        }
        let lines = ending.notices(50, 0, true);
        assert!(lines.len() == 1 && lines[0].contains("2 more refused"), "the end of a run flushes what is pending: {:?}", lines);
    }

    fn this_process() -> Process {
        Process::open(std::process::id()).expect("open this process")
    }

    #[test]
    fn a_request_after_the_deadline_is_refused_and_moves_nothing() {
        // Ticks are milliseconds: the run's own deadline is at 10 000.
        let mut state = State::new(10_000, 60.0);
        assert_eq!(state.grant(2_000, None, 5.0, this_process(), 0, None), Ok(None));
        assert_eq!(state.deadline(), 10_000, "a hold earlier than the run's own deadline moves nothing");
        assert_eq!(state.grant(9_000, None, 5.0, this_process(), 0, None), Ok(None), "the first hold ran out before this grant, so there is no earlier hold to go back to");
        assert_eq!(state.deadline(), 14_000);
        // The run is past its deadline at 14 000 and nobody has noticed: the request is refused all the same.
        assert_eq!(state.grant(14_000, None, 5.0, this_process(), 0, None), Err(Refusal::Ended.into()));
        assert!(state.passed && state.deadline() == 10_000, "the hold that ran out is gone and the deadline did not move");
        assert_eq!(state.grant(14_001, None, 5.0, this_process(), 0, None), Err(Refusal::Ended.into()), "and it stays ended");
        // A run with no hold at all, past its own deadline.
        let mut plain = State::new(10_000, 60.0);
        assert_eq!(plain.grant(10_500, None, 5.0, this_process(), 0, None), Err(Refusal::Ended.into()));
        assert!(plain.holds.is_empty() && plain.expire(10_501, None));
    }

    #[test]
    fn a_channel_that_stopped_serving_grants_nothing_and_the_run_still_ends_at_its_deadline() {
        // Ticks are milliseconds: the run's own deadline is at 10 000, and a hold granted before the channel
        // stopped serving runs to 14 000. Closing to grants is not the deadline passing.
        let mut state = State::new(10_000, 60.0);
        assert_eq!(state.grant(9_000, None, 5.0, this_process(), 0, None), Ok(None));
        state.closed = true;
        assert!(!state.expire(9_500, None), "the deadline has not passed: the run goes on");
        assert_eq!(state.deadline(), 14_000, "the hold that was granted stands");
        assert!(state.ended(9_500, None), "but a request is refused as the run being over");
        assert_eq!(state.grant(9_600, None, 5.0, this_process(), 0, None), Err(Refusal::Ended.into()));
        assert_eq!(state.deadline(), 14_000, "and moves nothing");
        assert!(!state.expire(13_999, None) && state.deadline() == 14_000, "the time left is the hold's, not nothing");
        assert!(state.expire(14_000, None), "the run ends at the deadline it had");
        // A hold that ran out stops holding the run up although nothing can be granted.
        let mut ran_out = State::new(10_000, 60.0);
        assert_eq!(ran_out.grant(9_000, None, 2.0, this_process(), 0, None), Ok(None));
        ran_out.closed = true;
        assert!(!ran_out.expire(10_500, None) && ran_out.deadline() == 11_000);
        assert!(!ran_out.expire(10_999, None));
        assert!(ran_out.expire(11_000, None));
    }

    #[test]
    fn a_hold_is_refused_beyond_the_limit_and_renewed_below_it() {
        let mut state = State::new(1_000_000, 3600.0);
        let fake = |n: u32| {
            let mut process = this_process();
            process.id = 4_000_000 + n;
            process.created = n as u64;
            process
        };
        for n in 0..MAX_HOLDS as u32 {
            assert_eq!(state.grant(1, None, 60.0, fake(n), 0, None), Ok(None), "hold {}", n);
        }
        assert_eq!(state.grant(1, None, 60.0, fake(MAX_HOLDS as u32), 0, None), Err(Refusal::TooManyHolds.into()));
        assert_eq!(state.holds.len(), MAX_HOLDS);
        assert_eq!(state.grant(2, None, 90.0, fake(0), 0, None), Ok(Some(60_001)), "a process that holds already can renew");
        assert_eq!(state.holds.len(), MAX_HOLDS);
        assert_eq!(state.grant(2, None, 7200.0, fake(0), 0, None), Err(Refusal::OverCap.into()));
        assert_eq!(state.grant(2, None, f64::NAN, fake(0), 0, None), Err(Refusal::InvalidSeconds.into()));
        assert_eq!(state.grant(2, None, 0.0, fake(0), 0, None), Err(Refusal::InvalidSeconds.into()));
    }

    #[test]
    fn one_process_cannot_take_the_table_of_holds_from_the_rest() {
        let mut state = State::new(1_000_000, 3600.0);
        let fake = |n: u32| {
            let mut process = this_process();
            process.id = 4_000_000 + n;
            process.created = n as u64;
            process
        };
        for token in 0..MAX_HOLDS_PER_PROCESS as u64 {
            assert_eq!(state.grant(1, None, 60.0, fake(0), token, None), Ok(None), "token {}", token);
        }
        assert_eq!(state.grant(1, None, 60.0, fake(0), MAX_HOLDS_PER_PROCESS as u64, None), Err(Refusal::TooManyHoldsHere.into()));
        assert_eq!(state.holds.len(), MAX_HOLDS_PER_PROCESS);
        assert_eq!(state.grant(2, None, 90.0, fake(0), 3, None), Ok(Some(60_001)), "a hold the process has renews");
        assert_eq!(state.grant(2, None, 60.0, fake(1), 0, None), Ok(None), "another process is not held back");
        // Not the same process: the same identifier with another creation time is another process.
        let mut later = fake(0);
        later.created = 77;
        assert_eq!(state.grant(2, None, 60.0, later, 0, None), Ok(None));
        assert!(state.drop_hold(4_000_000, 0, 0, false));
        assert_eq!(state.grant(3, None, 60.0, fake(0), 99, None), Ok(None), "a hold that ended makes room");
        assert_eq!(Refusal::TooManyHoldsHere.describe(60.0), "this process holds too many deadlines");
        assert_eq!(Refusal::from_code(Refusal::TooManyHoldsHere as u64), Some(Refusal::TooManyHoldsHere));
    }

    /// A log whose only record is a standard error simulated by a sink: `fail` makes its writes fail, and `delay_ms` makes them slow.
    fn standard_error_only(fail: &Arc<AtomicBool>, delay_ms: u64) -> Log {
        let last_written = Arc::new(AtomicBool::new(true));
        let (seen, failing) = (last_written.clone(), fail.clone());
        let write = move |_: &str| {
            std::thread::sleep(Duration::from_millis(delay_ms));
            let written = !failing.load(Ordering::SeqCst);
            seen.store(written, Ordering::SeqCst);
            written
        };
        let sink = Arc::new(Sink::start("test-stderr", Box::new(write), 8).expect("start the sink"));
        Log { sinks: vec![sink.clone()], file_sink: None, blocked: Arc::new(AtomicBool::new(false)), only: Some(StandardErrorOnly { sink, last_written }) }
    }

    #[test]
    fn a_grant_stands_only_when_its_line_reached_standard_error_if_that_is_the_only_record() {
        let fail = Arc::new(AtomicBool::new(false));
        let log = standard_error_only(&fail, 0);
        assert!(log.record("granted", 2_000) && !log.blocked(), "a line that is written is a record");
        fail.store(true, Ordering::SeqCst);
        assert!(!log.record("granted", 2_000), "a line that could not be written is not");
        assert!(log.blocked(), "and nothing is granted while writes fail");
        fail.store(false, Ordering::SeqCst);
        log.push("a refusal");
        let end = Instant::now() + Duration::from_secs(5);
        while log.blocked() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!log.blocked(), "a write that works lifts it");
        assert!(log.record("granted", 2_000));
        log.finish(1_000);
    }

    #[test]
    fn a_line_that_does_not_finish_is_not_a_record_and_a_file_is_not_waited_for() {
        let fail = Arc::new(AtomicBool::new(false));
        let slow = standard_error_only(&fail, 400);
        assert!(!slow.record("granted", 20), "standard error that is slow to take the line: no record within the wait");
        slow.finish(1_000);
        let with_file = Log { sinks: Vec::new(), file_sink: None, blocked: Arc::new(AtomicBool::new(false)), only: None };
        assert!(with_file.record("granted", 0) && !with_file.blocked(), "with an audit file the file's own rule decides, and nothing is waited for");
        with_file.blocked.store(true, Ordering::SeqCst);
        assert!(with_file.blocked());
    }

    #[test]
    fn a_file_sink_whose_queue_has_closed_blocks_grants() {
        // The state the start of the file sink leaves when its thread
        // could not be started or could not be hardened: the starting
        // materials taken and the queue closed. No line ever reaches
        // the audit file from there, so the file's own state can
        // never report the failure; the log must read the sink's.
        let sink = Arc::new(Sink::start("test-file", Box::new(|_: &str| true), 8).expect("start the sink"));
        let log = Log { sinks: Vec::new(), file_sink: Some(sink.clone()), blocked: Arc::new(AtomicBool::new(false)), only: None };
        assert!(!log.blocked(), "a file sink that has not started has not failed: its first line starts it");
        sink.started.store(true, Ordering::SeqCst);
        assert!(!log.blocked(), "a started file sink with its queue open takes lines");
        relock(&sink.queue).take();
        assert!(sink.closed());
        assert!(log.blocked(), "a file sink whose queue has closed takes no line ever: nothing is granted");
        // The line of a grant is not waited for with an audit file, and
        // `blocked`, which the server reads before it grants, refuses
        // every later grant — but a grant whose own line is the run's
        // first arrives before that state exists, so the record itself
        // must refuse it: the closed queue takes no line, and the grant
        // does not stand.
        assert!(!log.record("granted", 0));
    }

    #[test]
    fn a_grant_whose_line_the_file_sink_cannot_queue_is_not_recorded() {
        // The file sink's queue is full: its thread was never started
        // in this construction, so the one place its queue holds is
        // taken by the first line and the second is refused by the
        // queue itself. A grant line the queue refuses reaches no
        // record — not the audit file, whose held lines it never
        // joins, and not its blocked state — so the record refuses
        // the grant, as it refuses one the closed queue of the test
        // above refuses.
        let sink = Arc::new(Sink::start("test-file-full", Box::new(|_: &str| true), 1).expect("start the sink"));
        sink.started.store(true, Ordering::SeqCst);
        let log = Log { sinks: vec![sink.clone()], file_sink: Some(sink), blocked: Arc::new(AtomicBool::new(false)), only: None };
        assert!(log.record("granted", 0), "a line the file sink queued is a record, waited for not at all");
        assert!(!log.record("granted again", 0), "a line the full queue refused is no record, and the grant does not stand");
    }

    #[test]
    fn with_an_audit_file_the_line_of_an_approval_is_a_record_only_when_the_file_took_it() {
        let folder = crate::test_temp::fresh_dir("ptg-record");
        // A file that takes lines: the line is waited for and it is in the file.
        let path = folder.join("audit.log");
        let good = Log::start(Some(std::sync::Arc::from(path.as_path())), 1 << 20, AuditFile::open(&path).ok()).expect("start a log with a file");
        assert!(good.record_decision("approved one", 5_000) && !good.blocked(), "a line the file wrote is a record");
        good.finish(1_000);
        assert!(std::fs::read_to_string(&path).unwrap_or_default().contains("approved one"));
        // A file in a folder that is not there cannot take a line: the approval of an exception is not recorded.
        let bad = Log::start(Some(std::sync::Arc::from(folder.join("missing").join("audit.log").as_path())), 1 << 20, None).expect("start a log whose file cannot be written");
        assert!(!bad.record_decision("approved two", 5_000), "a line that the file could not take is not a record");
        assert!(bad.blocked(), "and nothing is granted while it cannot");
        // The line of a grant is not waited for with an audit file, as before.
        assert!(bad.record("granted", 0), "the line of a grant is queued and stands at once");
        bad.finish(1_000);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn an_approval_is_decided_by_the_file_sink_named_not_by_whichever_sink_is_last() {
        // The negative control for the approval side of the closed
        // class: the sinks are ordered file-first / copy-last, the
        // file sink's queue is closed and the copy takes the line.
        // A record decided by the last sink stands the approval on
        // the copy's write; the named file sink's refused enqueue
        // must refuse it.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture_with_closed_file(kept.clone());
        assert!(!log.record_decision("approved", 1_000), "an approval whose file sink queue is closed is not recorded, even when the copy sink takes the line");
        log.finish(1_000);
        assert!(relock(&kept).iter().any(|line| line == "approved"), "the copy sink did take the line: {:?}", relock(&kept));
    }

    #[test]
    fn a_released_line_whose_file_enqueue_is_refused_reaches_the_waited_standard_error_record() {
        // The file sink's queue is closed and the copy is the
        // remaining record: the decision entry point reports the
        // file's refusal, and its waited fallback puts the line in
        // the copy as a waited record.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture_with_closed_file(kept.clone());
        let line = released_line(0, 7, "client.exe", "the hold ended with the library call that asked for it");
        assert!(!log.push_decision(&line), "the closed file sink refuses the decision line");
        assert!(log.push_decision_waited(&line, 2_000), "the waited copy is the record the line falls back to");
        log.finish(1_000);
        assert!(relock(&kept).iter().any(|kept_line| kept_line == &line), "the released line reached the copy: {:?}", relock(&kept));
    }

    #[test]
    fn a_grant_whose_reply_was_not_taken_is_taken_back() {
        let mut state = State::new(1_000, 3600.0);
        let me = this_process();
        let back = |until: u64, previous: Option<u64>| Withdraw { id: me.id, created: me.created, token: 5, until, previous, pid: me.id, image: "client.exe".into(), seconds: 60.0 };
        // A first grant is removed.
        let previous = state.hold_until(me.id, me.created, 5);
        assert_eq!(previous, None);
        assert_eq!(state.begin_grant(10, None, 60.0, this_process(), 5, None), Ok(None));
        assert_eq!(state.deadline(), 60_010);
        assert!(state.finish_grant(&back(60_010, previous), false));
        assert!(state.holds.is_empty() && state.deadline() == 1_000, "the hold is gone");
        // A renewal goes back to the hold it replaced, which the client was told of.
        assert_eq!(state.grant(10, None, 20.0, this_process(), 5, None), Ok(None));
        let previous = state.hold_until(me.id, me.created, 5);
        assert_eq!(state.begin_grant(11, None, 90.0, this_process(), 5, None), Ok(Some(20_010)));
        assert!(state.finish_grant(&back(90_011, previous), false));
        assert_eq!(state.deadline(), 20_010, "the earlier hold stands");
        // A hold that something else has changed since is not touched, and the other holds of the process stand.
        assert_eq!(state.begin_grant(12, None, 5.0, this_process(), 6, None), Ok(None));
        assert!(!state.finish_grant(&back(90_011, None), false), "it is no longer the grant that was made");
        assert!(state.finish_grant(&Withdraw { token: 6, ..back(5_012, None) }, false), "the other hold is withdrawn by its own record");
        assert_eq!(state.holds.len(), 1);
    }

    #[test]
    fn an_overlapping_renewal_cannot_restore_an_undelivered_grant() {
        let mut state = State::new(1_000, 3600.0);
        let me = this_process();
        let back = Withdraw { id: me.id, created: me.created, token: 0, until: 60_010, previous: None, pid: me.id, image: "client.exe".into(), seconds: 60.0 };
        assert_eq!(state.begin_grant(10, None, 60.0, this_process(), 0, None), Ok(None));
        assert_eq!(state.begin_grant(11, None, 90.0, this_process(), 0, None), Err(Refusal::Crowded.into()));
        assert!(state.finish_grant(&back, false));
        assert_eq!(state.deadline(), 1_000, "no delivered grant can be restored");

        // Even release must not allow a second grant to reuse a pending reservation.
        assert_eq!(state.begin_grant(10, None, 60.0, this_process(), 0, None), Ok(None));
        assert!(state.drop_hold(me.id, me.created, 0, false));
        assert_eq!(state.begin_grant(11, None, 90.0, this_process(), 0, None), Err(Refusal::Crowded.into()));
        assert!(!state.finish_grant(&back, false));
        assert_eq!(state.begin_grant(12, None, 0.001, this_process(), 0, None), Ok(None));
        assert!(!state.expire(13, None));
        assert!(state.holds.is_empty());
        assert_eq!(state.begin_grant(13, None, 90.0, this_process(), 0, None), Err(Refusal::Crowded.into()));
        assert!(!state.finish_grant(&Withdraw { until: 13, seconds: 0.001, ..back }, false));
        assert_eq!(state.begin_grant(14, None, 60.0, this_process(), 0, None), Ok(None));
    }

    #[test]
    fn a_provisional_grant_cannot_admit_another_after_the_delivered_deadline() {
        let mut state = State::new(1_000, 3600.0);
        let me = this_process();
        let back = Withdraw { id: me.id, created: me.created, token: 0, until: 60_999, previous: None, pid: me.id, image: "client.exe".into(), seconds: 60.0 };
        assert_eq!(state.begin_grant(999, None, 60.0, this_process(), 0, None), Ok(None));
        assert!(!state.expire(1_000, None), "the supervisor allows the first reply to finish");
        assert_eq!(state.begin_grant(1_000, None, 90.0, this_process(), 1, None), Err(Refusal::Ended.into()));
        assert_eq!(state.holds.len(), 1, "the refused request did not create a hold");
        assert!(state.finish_grant(&back, true));
        assert_eq!(state.begin_grant(1_000, None, 90.0, this_process(), 1, None), Ok(None), "the delivered grant now admits a different token");
    }

    #[test]
    fn a_shorter_renewal_takes_effect_only_after_delivery() {
        let mut state = State::new(1_000, 3600.0);
        let me = this_process();
        assert_eq!(state.grant(10, None, 20.0, this_process(), 0, None), Ok(None));
        let back = Withdraw { id: me.id, created: me.created, token: 0, until: 12, previous: Some(20_010), pid: me.id, image: "client.exe".into(), seconds: 0.001 };
        assert_eq!(state.begin_grant(11, None, 0.001, this_process(), 0, None), Ok(Some(20_010)));
        assert!(!state.expire(1_001, None), "the tentative shorter deadline cannot expire a delivered hold");
        assert!(state.finish_grant(&back, false));
        assert_eq!(state.deadline(), 20_010);

        assert_eq!(state.begin_grant(1_002, None, 1.0, this_process(), 0, None), Ok(Some(20_010)));
        assert!(state.finish_grant(&Withdraw { until: 2_002, seconds: 1.0, ..back }, true));
        assert_eq!(state.deadline(), 2_002);
    }

    #[test]
    fn the_channel_is_a_message_mode_pipe_that_refuses_remote_clients() {
        // The mode create_pipes passes to CreateNamedPipeW, and that it is the one passed.
        assert_ne!(CHANNEL_PIPE_MODE & PIPE_TYPE_MESSAGE, 0, "message type");
        assert_ne!(CHANNEL_PIPE_MODE & PIPE_READMODE_MESSAGE, 0, "message read mode");
        assert_ne!(CHANNEL_PIPE_MODE & PIPE_REJECT_REMOTE_CLIENTS, 0, "remote clients rejected");
        assert_eq!(OUT_BUFFER, 0, "no outgoing buffer, so a write completes when the reply is read");
        // The call is spelled in parts here, so that this test does not find itself.
        let call = format!("{}, {}, {}, 512, 0, &mut attributes", "CHANNEL_PIPE_MODE", "INSTANCES", "OUT_BUFFER");
        assert_eq!(include_str!("descriptor.rs").matches(call.as_str()).count(), 1, "create_pipes passes the mode and the buffer sizes");
    }

    #[test]
    fn a_client_must_have_started_before_it_connected() {
        let me = this_process();
        assert!(me.started_before(me.created) && me.started_before(me.created + 1), "started at or before the connection");
        assert!(!me.started_before(me.created - 1), "a process that started after the connection is not the one that connected");
    }

    #[test]
    fn the_poll_releases_a_holder_that_exited_with_no_message_from_the_job() {
        // No completion port is involved: this is the backstop that the supervisor runs at least every BACKSTOP_WAKE_MS.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            assert!(!job.is_null());
            let mut child = std::process::Command::new("ping.exe").args(["-n", "60", "127.0.0.1"])
                .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().expect("start ping.exe");
            use std::os::windows::io::AsRawHandle;
            assert_ne!(AssignProcessToJobObject(job, child.as_raw_handle() as Handle), 0, "put the holder in the job");
            let mut state = State::new(1_000_000, 3600.0);
            state.holds.push(Hold { process: Process::open(child.id()).expect("open the holder"), token: 0, until: 900_000 });
            state.release(1, Some(job));
            assert_eq!(state.holds.len(), 1, "a holder that is alive, in the job and listed keeps its hold");
            child.kill().expect("end the holder");
            child.wait().expect("reap the holder");
            // The backstop's next pass, at a later tick than the reap above: a pass at
            // that reap's own tick would stand on its answers (see the stamp test below).
            state.release(2, Some(job));
            assert!(state.holds.is_empty(), "the poll finds the holder gone");
            let mut late = State::new(1_000_000, 3600.0);
            late.holds.push(Hold { process: this_process(), token: 0, until:500 });
            late.release(501, None);
            assert!(late.holds.is_empty(), "a hold whose seconds ran out is released");
            CloseHandle(job);
        }
    }

    #[test]
    fn a_settle_at_the_tick_of_the_last_reap_skips_it_and_the_next_tick_reaps_in_full() {
        // The double settle of one request, with the time a number: `arrive` settles the
        // deadline, and `State::grant` settles it again under a fresh lock. When the two
        // settles share a tick, the second stands on the first's reap; at a later tick
        // it reaps in full, exactly as before the stamp.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            assert!(!job.is_null());
            let mut child = std::process::Command::new("ping.exe").args(["-n", "60", "127.0.0.1"])
                .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().expect("start ping.exe");
            use std::os::windows::io::AsRawHandle;
            assert_ne!(AssignProcessToJobObject(job, child.as_raw_handle() as Handle), 0, "put the holder in the job");
            let mut state = State::new(100_000, 3600.0);
            state.holds.push(Hold { process: Process::open(child.id()).expect("open the holder"), token: 0, until: 900_000 });
            // The settle the request makes when it arrives: a full reap, which keeps the
            // live holder (as the poll test above shows) and stamps its tick.
            assert!(!state.expire(1, Some(job)), "the run goes on");
            assert_eq!(state.holds.len(), 1, "the live holder is kept by the reap");
            assert_eq!(state.release_stamp, Some(1), "the completed reap is stamped with its tick");
            child.kill().expect("end the holder");
            child.wait().expect("reap the holder");
            // The settle the grant then makes, at the same tick: the reap is skipped, so
            // the holder that has just exited still stands — the staleness the stamp
            // names, bounded by the supervisor's next pass — while the deadline is
            // still folded from the holds as they stand, on this call as on every call.
            assert!(!state.expire(1, Some(job)), "the run goes on");
            assert_eq!(state.holds.len(), 1, "a second settle at the reap's tick does not reap again");
            assert_eq!(state.expire_deadline, 900_000, "the deadline is folded from the hold that stands");
            assert_eq!(state.release_stamp, Some(1), "a skipped settle does not move the stamp");
            // The grant itself, at the same tick, is decided on the reap's answers.
            assert_eq!(state.grant(1, Some(job), 60.0, this_process(), 1, None), Ok(None));
            assert_eq!(state.holds.len(), 2, "the exited holder still stands beside the new hold");
            // The first settle at a later tick reaps in full: the exited holder goes
            // (and the new hold with it, its process not being in this job), and the
            // deadline falls back to the run's own.
            assert!(!state.expire(2, Some(job)), "the run goes on");
            assert!(state.holds.is_empty(), "a settle at a later tick reaps in full");
            assert_eq!(state.expire_deadline, 100_000, "the deadline is the run's own again");
            assert_eq!(state.release_stamp, Some(2), "the later reap is the stamped one now");
            CloseHandle(job);
        }
    }

    #[test]
    fn a_release_at_the_reaps_own_tick_polls_nobody_and_a_later_tick_polls_again() {
        // The reap at the level of `release` itself, with no process to spawn: the
        // holder is an event in a `Process`'s clothes, unsignalled while it "runs"
        // (the aliveness poll waits with a zero timeout and times out) and signalled
        // once it has "exited" (the poll returns at once). No job is given, so
        // membership asks nothing and the poll is the only question a reap asks.
        unsafe {
            let event = CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null());
            assert!(!event.is_null());
            // The event's handle moves into the hold's `Process`, which closes it
            // when the hold is dropped at the end of the test.
            let holder = Process { handle: event, id: 4242, created: 1 };
            let mut state = State::new(100_000, 3600.0);
            state.holds.push(Hold { process: holder, token: 0, until: 900_000 });
            state.release(10, None);
            assert_eq!(state.holds.len(), 1, "the running holder is kept by the reap");
            assert_eq!(state.release_stamp, Some(10), "the completed reap is stamped with its tick");
            SetEvent(event);
            state.release(10, None);
            assert_eq!(state.holds.len(), 1, "a release at the reap's own tick polls nobody: the exited holder stands");
            assert_eq!(state.release_stamp, Some(10), "the skipped release does not move the stamp");
            state.release(11, None);
            assert!(state.holds.is_empty(), "a release at a later tick polls again: the exited holder is reaped");
            assert_eq!(state.release_stamp, Some(11), "the later reap is the stamped one now");
        }
    }

    #[test]
    fn a_log_destination_that_stalls_holds_up_only_its_own_thread() {
        let (open, gate) = std::sync::mpsc::channel::<()>();
        let written = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = written.clone();
        // The destination takes no line until the gate is dropped, like a standard error that nobody reads.
        let sink = Sink::start("test-sink", Box::new(move |line: &str| {
            let _ = gate.recv();
            seen.lock().unwrap().push(line.to_string());
            true
        }), 4).expect("start the sink");
        let began = std::time::Instant::now();
        for n in 0..100 {
            sink.push(&format!("line {}", n));
        }
        assert!(began.elapsed() < Duration::from_secs(1), "queueing a line never waits for its destination");
        assert!(sink.dropped.load(Ordering::SeqCst) >= 95, "what does not fit in the queue is dropped and counted: {}", sink.dropped.load(Ordering::SeqCst));
        let began = std::time::Instant::now();
        sink.finish(100);
        assert!(began.elapsed() < Duration::from_secs(2), "finishing does not wait for a destination that is stuck");
        drop(open);
        let limit = std::time::Instant::now() + Duration::from_secs(10);
        while !written.lock().unwrap().iter().any(|line| line.contains("log lines were not written")) {
            assert!(std::time::Instant::now() < limit, "the notice of the dropped lines is written once the destination takes lines: {:?}", written.lock().unwrap());
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(written.lock().unwrap()[0], "line 0", "the lines that were kept come first, in order");
    }

    #[test]
    fn a_log_destination_that_works_gets_every_line_before_the_end() {
        let written = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = written.clone();
        let sink = Sink::start("test-sink", Box::new(move |line: &str| {
            seen.lock().unwrap().push(line.to_string());
            true
        }), LOG_QUEUE).expect("start the sink");
        for n in 0..LOG_QUEUE {
            sink.push(&format!("line {}", n));
        }
        sink.finish(5000);
        assert_eq!(written.lock().unwrap().len(), LOG_QUEUE, "finishing waits for the queued lines");
        sink.push("too late");
        assert_eq!(written.lock().unwrap().len(), LOG_QUEUE, "a finished sink takes no more lines");
    }

    #[test]
    fn a_sink_thread_started_behind_its_gate_writes_the_line_that_started_it() {
        // The first line starts the sink's thread behind its gate:
        // the thread is hardened through its creation handle and
        // released before it takes a line, and the line that started
        // it is then written like any other, as a waited record.
        let written = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = written.clone();
        let sink = Sink::start("test-sink", Box::new(move |line: &str| {
            seen.lock().unwrap().push(line.to_string());
            true
        }), 8).expect("start the sink");
        let failed_before = sink.failed_writes();
        let number = sink.enqueue("first line").expect("the first line is queued");
        assert!(sink.wait_recorded(number, failed_before, 5_000), "the line that started the sink is written once the gate is released");
        sink.finish(1_000);
        assert_eq!(written.lock().unwrap().as_slice(), ["first line"]);
    }

    #[test]
    fn refusals_say_why() {
        assert_eq!(Refusal::OverCap.describe(21600.0), "the request is over the cap of 21600 seconds");
        assert_eq!(Refusal::OverCap.describe(0.5), "the request is over the cap of 0.5 seconds");
        assert!(Refusal::NotInJob.describe(1.0).contains("not in the job"));
    }

    #[test]
    fn only_a_channel_name_is_opened() {
        let good = format!("{}{}", PIPE_PREFIX, "0123456789abcdef0123456789abcdef");
        assert!(valid_channel(&good));
        for bad in [
            "", "C:\\Windows\\win.ini", "\\\\.\\pipe\\other", "\\\\.\\pipe\\process-tree-guard-", "\\\\.\\pipe\\process-tree-guard-0123",
            "\\\\.\\pipe\\process-tree-guard-0123456789ABCDEF0123456789ABCDEF", "\\\\.\\pipe\\process-tree-guard-0123456789abcdef0123456789abcde/",
            "\\\\.\\pipe\\process-tree-guard-0123456789abcdef0123456789abcdef0", "\\\\server\\pipe\\process-tree-guard-0123456789abcdef0123456789abcdef",
        ] {
            assert!(!valid_channel(bad), "{}", bad);
        }
    }

    /// A folder under the system temporary folder that is removed when it goes out of scope, whether
    /// the test passed or failed. It is declared before the files a test holds open, so it is dropped
    /// after them: a folder with an open file in it cannot be removed.
    struct Scratch(PathBuf);

    impl std::ops::Deref for Scratch {
        type Target = PathBuf;
        fn deref(&self) -> &PathBuf {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            for _ in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    fn scratch_folder(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("ptg-{}-{}-{}", name, std::process::id(), tick()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn size_of(path: &Path) -> u64 {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    }

    fn numbered(n: usize) -> String {
        format!("PROCESS_TREE_GUARD_EXTENSION: line {:02} padded to a useful length", n)
    }

    #[test]
    fn the_log_is_bounded_with_one_rotation() {
        let dir = scratch_folder("log");
        let path = dir.join("extension.log");
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 300, AuditFile::open(&path).ok());
        for n in 0..40 {
            file.append(&numbered(n));
        }
        let rotated = dir.join("extension.log.1");
        assert!(size_of(&path) > 0 && size_of(&path) <= 300, "{}", size_of(&path));
        assert!(size_of(&rotated) > 0 && size_of(&rotated) <= 300, "{}", size_of(&rotated));
        assert!(std::fs::read_to_string(&path).unwrap().contains("line 39"), "the newest line is in the file");
        assert!(!dir.join("extension.log.2").exists(), "one rotation only");
        assert!(!std::fs::read_to_string(&path).unwrap().contains("notice="), "a rotation that works says nothing");
    }

    /// Open `path` for reading the way a program that follows a file does, sharing reading and
    /// writing but not deleting, so a rename of the file is refused.
    fn follow(path: &Path) -> std::fs::File {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().read(true).share_mode(1 | 2).open(path).expect("follow the file")
    }

    #[test]
    fn a_file_that_cannot_be_renamed_keeps_its_earlier_lines() {
        let dir = scratch_folder("follow");
        let path = dir.join("extension.log");
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 300, AuditFile::open(&path).ok());
        file.append(&numbered(0));
        let _follower = follow(&path);
        assert!(std::fs::rename(&path, dir.join("probe")).is_err(), "a follower that does not share delete stops a rename");
        for n in 1..6 {
            file.append(&numbered(n));
        }
        let rotated = dir.join("extension.log.1");
        let earlier = std::fs::read_to_string(&rotated).unwrap_or_default();
        let now = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(earlier.contains("line 00") && earlier.contains("line 03"), "the earlier lines were copied to PATH.1: {:?} / {:?}", earlier, now);
        assert!(now.contains("line 05") && !now.contains("line 00"), "the file holds the later lines: {:?}", now);
        assert!(size_of(&path) <= 300, "and is bounded: {}", size_of(&path));
    }

    #[test]
    fn a_file_that_cannot_be_turned_over_at_all_grows_to_twice_its_limit_and_says_so() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = scratch_folder("stuck");
        let path = dir.join("extension.log");
        let rotated = dir.join("extension.log.1");
        std::fs::write(&rotated, "an earlier turn over\n").unwrap();
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 300, AuditFile::open(&path).ok());
        file.append(&numbered(0));
        // The file is followed, and PATH.1 is held open with no sharing at all, so neither a
        // rename nor a copy can replace it.
        let follower = follow(&path);
        assert!(std::fs::rename(&path, dir.join("probe")).is_err(), "a follower that does not share delete stops a rename");
        let held = std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(&rotated).expect("hold PATH.1");
        for n in 1..30 {
            file.append(&numbered(n));
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(text.contains("line 00"), "the first line is still there: {:?}", text);
        assert!(text.contains("could not be turned over"), "and the file says that it could not be turned over: {:?}", text);
        assert!(text.contains("takes no more lines"), "and that it takes no more lines: {:?}", text);
        assert!(!text.contains("line 29"), "the lines past twice the limit are not written: {:?}", text);
        assert!(size_of(&path) <= 2 * 300 + 400, "bounded at twice the limit and a notice: {}", size_of(&path));
        drop(held);
        drop(follower);
        std::thread::sleep(Duration::from_millis(TURN_OVER_RETRY_MS + 100));
        for n in 30..33 {
            file.append(&numbered(n));
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(text.contains("line 32") && !text.contains("line 00"), "once it can be turned over it is: {:?}", text);
        assert!(std::fs::read_to_string(&rotated).unwrap_or_default().contains("line 00"), "with the earlier lines in PATH.1");
    }

    #[test]
    fn a_release_ends_a_hold_and_costs_its_process_nothing_if_there_was_one() {
        let me = this_process();
        let key = (me.id, me.created);
        let mut late = State::new(1_000, 60.0);
        assert_eq!(late.grant(5, None, 20.0, this_process(), 0, None), Ok(None));
        assert_eq!(late.deadline(), 20_005);
        assert!(late.drop_hold(me.id, me.created, 0,false), "there was a hold to end");
        assert_eq!(late.deadline(), 1_000, "the deadline falls back to the run's own");
        assert!(!late.drop_hold(me.id, me.created, 0,false), "and a second release finds none");
        // The rate: the request of a release that ended a hold is given back, so the share
        // of a process that makes library calls is used by the calls and not the releases.
        let mut counted = State::new(1_000_000, 60.0);
        for n in 0..RATE_LIMIT as u64 - 1 {
            assert_eq!(counted.limiter.admit(key, (me.id, "w.exe"), n, &live), Admit::Allowed);
        }
        counted.holds.push(Hold { process: this_process(), token: 0, until:900_000 });
        assert_eq!(counted.limiter.admit(key, (me.id, "w.exe"), RATE_LIMIT as u64, &live), Admit::Allowed, "the release arrives and is counted");
        assert!(counted.drop_hold(me.id, me.created, 0,false));
        assert_eq!(counted.limiter.admit(key, (me.id, "w.exe"), RATE_LIMIT as u64 + 1, &live), Admit::Allowed, "it was given back, so one more request fits");
        assert_eq!(counted.limiter.admit(key, (me.id, "w.exe"), RATE_LIMIT as u64 + 2, &live), Admit::First, "and then the share is used up");
        // A release that arrives when the share is used up is let past it: the refusal it was counted as is taken back.
        let mut full = State::new(1_000_000, 60.0);
        for n in 0..RATE_LIMIT as u64 {
            assert_eq!(full.limiter.admit(key, (me.id, "w.exe"), n, &live), Admit::Allowed);
        }
        full.holds.push(Hold { process: this_process(), token: 0, until:900_000 });
        assert_eq!(full.limiter.admit(key, (me.id, "w.exe"), 100, &live), Admit::First, "the release arrives over its share");
        assert!(full.drop_hold(me.id, me.created, 0,true));
        assert_eq!(full.limiter.admit(key, (me.id, "w.exe"), 101, &live), Admit::First, "and the refusal it was counted as is taken back: the next one is the first again");
        // A release that found no hold stays counted.
        let mut empty = State::new(1_000_000, 60.0);
        for n in 0..RATE_LIMIT as u64 {
            assert_eq!(empty.limiter.admit(key, (me.id, "w.exe"), n, &live), Admit::Allowed);
        }
        assert!(!empty.drop_hold(me.id, me.created, 0,false));
        assert_eq!(empty.limiter.admit(key, (me.id, "w.exe"), RATE_LIMIT as u64, &live), Admit::First);
    }

    /// The number a summary line counts.
    fn counted_in(line: &str) -> Option<u32> {
        line.split("requests; ").nth(1)?.split(' ').next()?.parse().ok()
    }

    #[test]
    fn a_process_that_keeps_asking_is_summarised_for_each_window_while_it_asks() {
        // One process asks every 400 milliseconds for 45 seconds, so it is never idle for a window, and the
        // supervisor looks at the table every 100 milliseconds, as it does. Nothing here waits: the time is a number.
        let mut state = State::new(0, 60.0);
        let (key, who) = ((9, 1), (9, "steady.exe"));
        let (mut firsts, mut counted, mut summed) = (0u32, 0u32, 0u32);
        let mut windows = Vec::new();
        for now in (0..45_000u64).step_by(100) {
            if now % 400 == 0 {
                match state.limiter.admit(key, who, now, &live) {
                    Admit::First => firsts += 1,
                    Admit::Counted => counted += 1,
                    Admit::Allowed => {}
                    Admit::Full => panic!("the table is not full"),
                }
            }
            for line in state.notices(now, 0, false) {
                assert!(line.contains(" pid=9 image=\"steady.exe\"") && line.contains("within 10 seconds of the first"), "{}", line);
                windows.push((now, counted_in(&line).expect("a count in the line")));
                summed += windows.last().unwrap().1;
            }
        }
        assert!(windows.len() >= 3, "a line for each window that ran its course while the process was still asking: {:?}", windows);
        assert!(windows.iter().all(|(_, count)| (1..=25).contains(count)), "a window of 10 seconds holds at most 25 requests of 400 milliseconds: {:?}", windows);
        assert!(windows.windows(2).all(|pair| pair[1].0 - pair[0].0 >= RATE_WINDOW_MS), "one line for each window, a window apart: {:?}", windows);
        assert!(firsts as usize >= windows.len(), "and the first refusal of each window is a line of its own: {} firsts", firsts);
        for line in state.notices(45_000, 0, true) {
            summed += counted_in(&line).expect("a count in the line");
        }
        assert_eq!(summed, counted, "every refusal that was counted is in a line, none twice");
    }

    #[test]
    fn a_reply_out_of_range_is_not_understood() {
        let granted = |seconds: f64, until: u64| encode_reply(&Reply::Granted { seconds, until_unix_ms: until, held: 0 });
        let refused = |cap: f64| encode_reply(&Reply::Refused { refusal: Refusal::OverCap, cap });
        assert!(decode_reply(&granted(5.0, LATEST_UNTIL_MS)).is_some() && decode_reply(&granted(0.0, 0)).is_some() && decode_reply(&granted(86400.0, 1)).is_some());
        assert!(decode_reply(&refused(0.0)).is_some() && decode_reply(&refused(86400.0)).is_some());
        for until in [LATEST_UNTIL_MS + 1, u64::MAX] {
            assert!(decode_reply(&granted(5.0, until)).is_none(), "until {}", until);
        }
        for seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 86401.0, 1e300] {
            assert!(decode_reply(&granted(seconds, 1)).is_none() && decode_reply(&refused(seconds)).is_none(), "seconds {}", seconds);
        }
        // What the sum would have done with such a time.
        assert!(UNIX_EPOCH.checked_add(Duration::from_millis(u64::MAX)).is_none());
        assert!(UNIX_EPOCH.checked_add(Duration::from_millis(LATEST_UNTIL_MS)).is_some());
    }

    /// The shared state of a run with nothing serving: enough to decide a request.
    fn shared_run(job: Handle) -> Shared {
        Shared {
            state: Mutex::new(State::new(tick() + 600_000, 3600.0)), cap_seconds: 3600.0, log: Log { sinks: Vec::new(), file_sink: None, blocked: Arc::new(AtomicBool::new(false)), only: None }, job: job as usize, wake: 0,
            stopping: AtomicBool::new(false), serving: AtomicUsize::new(0),
        }
    }

    #[test]
    fn a_client_that_started_after_it_connected_or_has_exited_is_not_verified() {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            assert!(!job.is_null());
            let shared = shared_run(job);
            let request = Message::Bytes(MessageBytes::from_slice(&encode_request(Target::Caller, 0, 5.0)));
            let arrival = |client: Process| Arrival { id: client.id, image: "client.exe".into(), client: Some(client), in_job: true, shut: None, gone: false };
            let unverified = |reply: &Reply| matches!(reply, Reply::Refused { refusal: Refusal::Unverified, .. });
            let created = this_process().created;
            // Connected before the process began: what is there now took the identifier of the one that connected.
            let (reply, line) = evaluate(&shared, arrival(this_process()), created - 1, &request);
            assert!(unverified(&reply) && matches!(line, Line::Refused(_)), "a process created after the connection is refused");
            assert!(shared.lock().holds.is_empty(), "and holds nothing");
            // The client connected, wrote its request and has exited: its record is still open, and it is not alive.
            let mut child = std::process::Command::new("ping.exe").args(["-n", "60", "127.0.0.1"])
                .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().expect("start ping.exe");
            let gone = Process::open(child.id()).expect("open the client");
            child.kill().expect("end the client");
            child.wait().expect("reap the client");
            let (reply, line) = evaluate(&shared, arrival(gone), filetime_now(), &request);
            assert!(unverified(&reply) && matches!(line, Line::Refused(_)), "a client that has exited before the reply is refused");
            assert!(shared.lock().holds.is_empty(), "and holds nothing");
            // A process that is alive, in the job and older than the connection, but whose identifier is not the
            // one that connected: the end of the pipe the client connected with is closed. It is what a process
            // that took the identifier of a client that wrote and exited looks like.
            let (reply, line) = evaluate(&shared, Arrival { gone: true, ..arrival(this_process()) }, created + 1, &request);
            assert!(unverified(&reply) && matches!(line, Line::Refused(_)), "a client that has closed its end is refused, whoever now has its identifier");
            assert!(shared.lock().holds.is_empty(), "and holds nothing");
            // The control: the same request from a client that began before the connection and is alive is granted.
            let (reply, line) = evaluate(&shared, arrival(this_process()), created + 1, &request);
            assert!(matches!(reply, Reply::Granted { .. }) && matches!(line, Line::Granted(..)));
            assert_eq!(shared.lock().holds.len(), 1);
            CloseHandle(job);
        }
    }

    #[test]
    fn a_released_line_names_its_decision() {
        let line = released_line(1_700_000_000_000, 42, "host.exe", "the hold ended with the library call that asked for it");
        assert_eq!(line, "PROCESS_TREE_GUARD_EXTENSION: time=2023-11-14T22:13:20.000Z pid=42 image=\"host.exe\" requested=0 granted=0 decision=released reason=\"the hold ended with the library call that asked for it\"");
        assert_eq!(notice_line(0, "a\nb"), "PROCESS_TREE_GUARD_EXTENSION: time=1970-01-01T00:00:00.000Z notice=\"a\\u{a}b\"");
    }

    /// The notices an `AuditFile` sends to standard error, as a test sees them.
    fn alerts() -> (Arc<Mutex<Vec<String>>>, Box<dyn Fn(&str) + Send>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let copy = seen.clone();
        (seen, Box::new(move |text: &str| copy.lock().unwrap().push(text.to_string())))
    }

    /// Open `path` for reading the way a reader does that denies write sharing: .NET's
    /// `File.OpenRead` and `FileStream` for reading share reading only.
    fn deny_writers(path: &Path) -> std::io::Result<std::fs::File> {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().read(true).share_mode(1).open(path)
    }

    fn contents(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn a_reader_that_denies_write_sharing_cannot_open_the_file_during_a_run() {
        const SHARING_VIOLATION: i32 = 32;
        let dir = scratch_folder("deny");
        let path = dir.join("extension.log");
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 300, AuditFile::open(&path).ok());
        assert_eq!(deny_writers(&path).err().and_then(|error| error.raw_os_error()), Some(SHARING_VIOLATION), "the file is held open for appending, so a reader that denies writers is refused");
        file.append(&numbered(0));
        assert!(follow(&path).metadata().is_ok(), "a reader that allows writers can read it");
        assert!(contents(&path).contains("line 00"), "and so can the plain read of a file");
        for n in 1..10 {
            file.append(&numbered(n));
        }
        assert!(dir.join("extension.log.1").exists(), "the file was turned over");
        assert_eq!(deny_writers(&path).err().and_then(|error| error.raw_os_error()), Some(SHARING_VIOLATION), "and the new file is held too");
    }

    #[test]
    fn lines_that_cannot_be_written_are_held_told_of_and_written_late_in_order() {
        let dir = scratch_folder("blind");
        let path = dir.join("extension.log");
        std::fs::write(&path, "").unwrap();
        // A reader that denies writers was there first: the guard cannot append.
        let reader = deny_writers(&path).expect("read the file first");
        let (seen, alert) = alerts();
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 100_000, AuditFile::open(&path).ok()).alerting(alert);
        for n in 0..3 {
            file.append(&numbered(n));
        }
        assert_eq!(contents(&path), "", "nothing could be written");
        let told = seen.lock().unwrap().clone();
        assert!(told.len() == 1 && told[0].starts_with("PROCESS_TREE_GUARD_EXTENSION: ") && told[0].contains("cannot be written"), "standard error is told at once: {:?}", told);
        file.append(&numbered(3));
        assert_eq!(seen.lock().unwrap().len(), 1, "once for the failure, not for every line");
        drop(reader);
        file.flush();
        let text = contents(&path);
        let at = |what: &str| text.find(what).unwrap_or(usize::MAX);
        assert!(at("line 00") < at("line 01") && at("line 01") < at("line 02") && at("line 02") < at("line 03"), "the lines are written in order: {:?}", text);
        assert!(at("line 03") < at("4 lines were written late and 0 were not written"), "and the file says how many were late, after them: {:?}", text);
        let told = seen.lock().unwrap().clone();
        assert!(told.len() == 2 && told[1].contains("4 lines were written late and 0 were not written"), "and so does standard error: {:?}", told);
        file.flush();
        assert_eq!(contents(&path), text, "once written, nothing is written twice");
    }

    #[test]
    fn lines_past_the_limit_of_those_held_are_counted_and_a_run_that_ends_blind_says_so() {
        let dir = scratch_folder("overflow");
        let path = dir.join("extension.log");
        std::fs::write(&path, "").unwrap();
        let reader = deny_writers(&path).expect("read the file first");
        let (seen, alert) = alerts();
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 1 << 20, AuditFile::open(&path).ok()).alerting(alert);
        for n in 0..HELD_LINES + 5 {
            file.append(&numbered(n % 90));
        }
        assert_eq!((file.held.len(), file.lost), (HELD_LINES, 5), "what does not fit is counted");
        file.close();
        let told = seen.lock().unwrap().clone();
        assert!(told.last().map_or(false, |last| last.contains(&format!("{} log lines were not written", HELD_LINES + 5))), "the end of the run says how many the file never got: {:?}", told);
        drop(reader);
        file.flush();
        let text = contents(&path);
        assert_eq!(text.lines().count(), HELD_LINES + 1, "the held lines and a notice: {}", text.lines().count());
        assert!(text.lines().last().unwrap_or("").contains(&format!("{} lines were written late and 5 were not written", HELD_LINES)), "{:?}", text.lines().last());
    }

    #[test]
    fn the_file_copy_of_the_log_is_tried_again_without_a_new_line() {
        let dir = scratch_folder("retry");
        let path = dir.join("extension.log");
        std::fs::write(&path, "").unwrap();
        let reader = deny_writers(&path).expect("read the file first");
        let log = Log::start(Some(std::sync::Arc::from(path.as_path())), LOG_LIMIT, AuditFile::open(&path).ok()).expect("start the log");
        log.push("PROCESS_TREE_GUARD_EXTENSION: one");
        log.push("PROCESS_TREE_GUARD_EXTENSION: two");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(contents(&path), "", "the reader keeps the file from taking lines");
        drop(reader);
        let limit = std::time::Instant::now() + Duration::from_secs(10);
        while !contents(&path).contains(": two") {
            assert!(std::time::Instant::now() < limit, "the lines are written when the reader has gone, with no further line to wake the file: {:?}", contents(&path));
            std::thread::sleep(Duration::from_millis(50));
        }
        log.finish(2000);
        let text = contents(&path);
        assert!(text.find(": one").unwrap_or(usize::MAX) < text.find(": two").unwrap_or(0), "in order: {:?}", text);
        assert!(text.contains("2 lines were written late"), "with the notice: {:?}", text);
    }

    /// The clock, read here and not through `filetime_now`, so a test of that function's use cannot share its fault.
    fn clock() -> u64 {
        let mut time = 0u64;
        unsafe { GetSystemTimePreciseAsFileTime(&mut time) };
        time
    }

    #[test]
    fn the_time_a_client_connected_is_read_from_the_clock_when_the_server_learns_of_it() {
        let open_client = |name: &str| std::fs::OpenOptions::new().read(true).write(true).open(name).expect("connect to the pipe");
        unsafe {
            for client_first in [false, true] {
                let name = format!("{}test-{}-{}-{}", PIPE_PREFIX, std::process::id(), tick(), client_first);
                let pipe = CreateNamedPipeW(wide(&name).as_ptr(), PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                    PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_REJECT_REMOTE_CLIENTS, 1, 512, 512, 0, std::ptr::null_mut());
                assert!(pipe != INVALID_HANDLE, "make the pipe");
                let event = CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null());
                let stop = CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null());
                let (client, stamp, before, after);
                if client_first {
                    // The client is there before the server calls: the server learns of it at once, which is later than the connect.
                    before = clock();
                    client = open_client(&name);
                    stamp = accept(pipe, event, stop);
                    after = clock();
                } else {
                    let (p, e, s) = (pipe as usize, event as usize, stop as usize);
                    let server = std::thread::spawn(move || accept(p as Handle, e as Handle, s as Handle));
                    std::thread::sleep(Duration::from_millis(150));
                    before = clock();
                    client = open_client(&name);
                    stamp = server.join().expect("the server thread");
                    after = clock();
                }
                let stamp = stamp.expect("a client connected");
                assert!(stamp >= before && stamp <= after, "client first {}: the stamp {} lies between the moment before the client connected ({}) and the moment the server returned ({})", client_first, stamp, before, after);
                // A process that began before the stamp passes the check made with it, and one that began after does not.
                let me = this_process();
                assert!(me.created <= stamp && me.started_before(stamp), "this process began before the client connected");
                assert!(!me.started_before(me.created - 1), "and one that began after the stamp does not pass");
                drop(client);
                CloseHandle(pipe);
                CloseHandle(event);
                CloseHandle(stop);
            }
            // With no client, the wait ends when it is told to stop, and there is no stamp.
            let name = format!("{}test-{}-{}-stop", PIPE_PREFIX, std::process::id(), tick());
            let pipe = CreateNamedPipeW(wide(&name).as_ptr(), PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_REJECT_REMOTE_CLIENTS, 1, 512, 512, 0, std::ptr::null_mut());
            let event = CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null());
            let stop = CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null());
            let (p, e, s) = (pipe as usize, event as usize, stop as usize);
            let server = std::thread::spawn(move || accept(p as Handle, e as Handle, s as Handle));
            std::thread::sleep(Duration::from_millis(100));
            SetEvent(stop);
            assert_eq!(server.join().expect("the server thread"), None);
            CloseHandle(pipe);
            CloseHandle(event);
            CloseHandle(stop);
        }
    }

    #[test]
    fn overlapping_calls_of_one_process_each_have_a_hold_of_their_own() {
        let me = this_process();
        let mut state = State::new(1_000, 3600.0);
        // Two calls of the library from threads of one process: A asks for 60 seconds, B for 5.
        assert_eq!(state.grant(5, None, 60.0, this_process(), 1, None), Ok(None));
        assert_eq!(state.grant(6, None, 5.0, this_process(), 2, None), Ok(None));
        assert_eq!(state.holds.len(), 2, "B's grant did not replace A's hold");
        assert_eq!(state.deadline(), 60_005, "the deadline is the latest of the holds");
        assert!(state.drop_hold(me.id, me.created, 2, false), "B returns and ends its own hold");
        assert_eq!(state.deadline(), 60_005, "A's hold stands");
        assert!(!state.drop_hold(me.id, me.created, 2, false), "B has no second hold to end");
        assert_eq!(state.grant(7, None, 90.0, this_process(), 1, None), Ok(Some(60_005)));
        assert_eq!(state.holds.len(), 1, "a call that renews replaces its own hold");
        assert_eq!(state.deadline(), 90_007);
        // The hold of the process itself (token 0, what `extend` asks for) is another hold again.
        assert_eq!(state.grant(8, None, 20.0, this_process(), 0, None), Ok(None));
        assert!(state.drop_hold(me.id, me.created, 1, false), "A returns");
        assert_eq!(state.deadline(), 20_008, "and the process's own hold stands");
        assert!(state.drop_hold(me.id, me.created, 0, false));
        assert_eq!(state.deadline(), 1_000);
    }

    #[test]
    fn a_token_is_next_to_none_that_this_process_has_used() {
        let (first, second) = (next_token(), next_token());
        assert!(first != 0 && second != 0 && first != second, "{} {}", first, second);
    }

    #[test]
    fn a_process_that_renews_in_time_keeps_the_deadline_a_fixed_time_ahead_and_one_that_is_late_cannot() {
        // Ticks are milliseconds: the run's own deadline is at 2 000 and the cap is 3 seconds.
        let mut state = State::new(2_000, 3.0);
        // A process renews before each hold ends. Each renewal is within the cap, so at every moment the deadline is
        // 3 seconds ahead of the latest one, and the run goes on for as long as the renewals do.
        for now in (1_000..=60_000).step_by(2_000) {
            assert_eq!(state.grant(now, None, 3.0, this_process(), 0, None), Ok(if now == 1_000 { None } else { Some(now + 1_000) }), "renewal at {}", now);
            assert_eq!(state.deadline(), now + 3_000, "renewal at {}", now);
            assert!(!state.expire(now + 1_000, None), "the run is not over at {}", now + 1_000);
        }
        // A request over the cap moves nothing, however well the process stands.
        assert_eq!(state.grant(61_000, None, 3.5, this_process(), 0, None), Err(Refusal::OverCap.into()));
        assert_eq!(state.deadline(), 62_000);
        // Once the hold has run out the run is over, and a request that comes then moves nothing.
        assert!(state.expire(62_000, None));
        assert_eq!(state.grant(62_000, None, 3.0, this_process(), 0, None), Err(Refusal::Ended.into()));
        assert!(state.passed, "the run has ended and cannot go on");
    }

    #[test]
    fn only_the_first_instance_of_the_channel_is_made_with_the_first_instance_flag() {
        // The flag makes the call fail when the name already has an instance, whoever made it. It is on the first
        // instance and on no other, since the others are made on the name the first has made.
        assert_ne!(instance_open_mode(0) & FILE_FLAG_FIRST_PIPE_INSTANCE, 0, "the first instance");
        for index in 1..INSTANCES {
            assert_eq!(instance_open_mode(index) & FILE_FLAG_FIRST_PIPE_INSTANCE, 0, "instance {}", index);
        }
        for index in 0..INSTANCES {
            assert_eq!(instance_open_mode(index) & PIPE_ACCESS_DUPLEX, PIPE_ACCESS_DUPLEX, "instance {}", index);
            assert_ne!(instance_open_mode(index) & FILE_FLAG_OVERLAPPED, 0, "instance {}", index);
        }
        // The call that makes the instances takes its mode from the function and from nothing else.
        let call = format!("{}({}),", "CreateNamedPipeW(wide_name.as_ptr(), instance_open_mode", "index");
        assert_eq!(include_str!("descriptor.rs").matches(call.as_str()).count(), 1, "create_pipes passes the mode of the function");
    }

    #[test]
    fn the_channel_cannot_be_made_over_a_pipe_of_the_same_name_that_somebody_made_first() {
        // Somebody makes the name first, with the settings the guard uses so that nothing else differs. The guard
        // makes all four instances the pipe allows, so a name that already has one cannot be served.
        let name = format!("{}test-{}-{}-squat", PIPE_PREFIX, std::process::id(), tick());
        let squatter = unsafe {
            CreateNamedPipeW(wide(&name).as_ptr(), PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED, CHANNEL_PIPE_MODE, INSTANCES, OUT_BUFFER, 512, 0, std::ptr::null_mut())
        };
        assert!(squatter != INVALID_HANDLE, "the test makes its pipe");
        assert!(create_pipes(&name).is_err(), "a name that is not new is refused");
        unsafe { CloseHandle(squatter) };
        // A new name is served, with all the instances.
        let fresh = format!("{}test-{}-{}-fresh", PIPE_PREFIX, std::process::id(), tick());
        let pipes = create_pipes(&fresh).expect("a new name is served");
        assert_eq!(pipes.len(), INSTANCES as usize);
        for pipe in pipes {
            unsafe { CloseHandle(pipe) };
        }
    }

    #[test]
    fn a_file_that_cannot_be_turned_over_stops_the_run_granting_before_it_is_full() {
        let dir = scratch_folder("blocked");
        let path = dir.join("extension.log");
        let rotated = dir.join("extension.log.1");
        std::fs::write(&rotated, "an earlier turn over\n").unwrap();
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 600, AuditFile::open(&path).ok());
        assert!(!file.blocked.load(Ordering::SeqCst));
        // A program in the job reads PATH.1 and shares reading only (read access is all it needs): neither the rename
        // nor the copy can replace the file, whatever the folder allows.
        let reader = deny_writers(&rotated).expect("read PATH.1");
        let mut told = 0;
        for n in 0..60 {
            file.append(&numbered(n));
            if file.blocked.load(Ordering::SeqCst) {
                told = n;
                break;
            }
        }
        assert!(told > 0, "the file was blocked before the lines ran out");
        let size = size_of(&path);
        assert!(size + file.reserve > 2 * 600 && size <= 2 * 600, "blocked within the reserve of twice the limit, and not past it: {}", size);
        assert!(contents(&path).contains("could not be turned over") && !contents(&path).contains("takes no more lines until it can"), "blocked with room left, so every line so far is in the file: {:?}", contents(&path));
        assert!(contents(&path).contains(&format!("line {:02}", told)), "the line that blocked it is in the file");
        // Nobody asks it to turn over, and nothing is written, until the reader lets go and a turn over works: the retry needs no line.
        file.flush();
        assert!(file.blocked.load(Ordering::SeqCst), "still blocked while the reader holds PATH.1");
        drop(reader);
        std::thread::sleep(Duration::from_millis(TURN_OVER_RETRY_MS + 100));
        file.flush();
        assert!(!file.blocked.load(Ordering::SeqCst), "once a turn over works the file is relied on again");
        assert!(contents(&rotated).contains(&format!("line {:02}", told)) && size_of(&path) == 0, "with the lines in PATH.1: {:?}", contents(&rotated));
    }

    #[test]
    fn a_file_that_cannot_be_written_stops_the_run_granting_until_it_can() {
        let dir = scratch_folder("noroom");
        let path = dir.join("extension.log");
        std::fs::write(&path, "").unwrap();
        let reader = deny_writers(&path).expect("read the file first");
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 100_000, AuditFile::open(&path).ok());
        file.append(&numbered(0));
        assert!(file.blocked.load(Ordering::SeqCst), "a line that cannot be written is a grant that is not in the record");
        drop(reader);
        file.flush();
        assert!(!file.blocked.load(Ordering::SeqCst), "and it is relied on again once it takes lines");
        assert!(contents(&path).contains("line 00"));
    }

    #[test]
    fn a_file_that_is_past_its_limit_at_the_start_and_cannot_be_turned_over_stops_the_run_before_it_starts() {
        let dir = scratch_folder("settle");
        let path = dir.join("extension.log");
        let rotated = dir.join("extension.log.1");
        std::fs::write(&rotated, "an earlier turn over\n").unwrap();
        std::fs::write(&path, "x".repeat(1190)).unwrap();
        let reader = deny_writers(&rotated).expect("read PATH.1");
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 600, AuditFile::open(&path).ok());
        assert!(!file.settle(), "1190 bytes of a limit of 600 with PATH.1 held: no room is left");
        drop(reader);
        let mut file = AuditFile::new(std::sync::Arc::from(path.as_path()), 600, AuditFile::open(&path).ok());
        assert!(file.settle(), "the same file with PATH.1 free is turned over before the run starts");
        assert_eq!(size_of(&path), 0);
        assert_eq!(size_of(&rotated), 1190);
    }

    #[test]
    fn a_blocked_file_refuses_every_request_that_would_move_the_deadline() {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            assert!(!job.is_null());
            let shared = shared_run(job);
            let request = Message::Bytes(MessageBytes::from_slice(&encode_request(Target::Caller, 0, 5.0)));
            let arrival = || Arrival { id: std::process::id(), image: "client.exe".into(), client: Some(this_process()), in_job: true, shut: None, gone: false };
            let created = this_process().created;
            shared.log.blocked.store(true, Ordering::SeqCst);
            let (reply, line) = evaluate(&shared, arrival(), created + 1, &request);
            assert!(matches!(reply, Reply::Refused { refusal: Refusal::AuditBlocked, .. }) && matches!(line, Line::Refused(_)));
            assert!(shared.lock().holds.is_empty(), "and holds nothing");
            shared.log.blocked.store(false, Ordering::SeqCst);
            let (reply, line) = evaluate(&shared, arrival(), created + 1, &request);
            assert!(matches!(reply, Reply::Granted { .. }), "granted again once the file can be relied on");
            let Line::Granted(_, back) = line else { panic!("the granted reply has a reservation") };
            assert!(shared.lock().finish_grant(&back, true));
            // A release only brings the deadline in, so it is let through while the file is blocked.
            shared.log.blocked.store(true, Ordering::SeqCst);
            let release = Message::Bytes(MessageBytes::from_slice(&encode_request(Target::Release, 0, 0.0)));
            let (reply, line) = evaluate(&shared, arrival(), created + 1, &release);
            assert!(matches!(reply, Reply::Granted { .. }) && matches!(line, Line::Released(_)));
            assert!(shared.lock().holds.is_empty());
            CloseHandle(job);
        }
        assert!(Refusal::AuditBlocked.describe(1.0).contains("audit file"));
    }

    #[test]
    fn a_client_that_has_closed_its_end_of_the_pipe_is_gone() {
        unsafe {
            let name = format!("{}test-{}-{}-gone", PIPE_PREFIX, std::process::id(), tick());
            let pipe = CreateNamedPipeW(wide(&name).as_ptr(), PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_REJECT_REMOTE_CLIENTS, 1, 512, 512, 0, std::ptr::null_mut());
            assert!(pipe != INVALID_HANDLE, "make the pipe");
            let event = CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null());
            let stop = CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null());
            let client = std::fs::OpenOptions::new().read(true).write(true).open(&name).expect("connect to the pipe");
            assert!(accept(pipe, event, stop).is_some());
            assert!(!client_gone(pipe), "a client that waits for its reply, with its end open, is there");
            drop(client);
            assert!(client_gone(pipe), "one that has closed its end is gone");
            CloseHandle(pipe);
            CloseHandle(event);
            CloseHandle(stop);
        }
    }

    #[test]
    fn a_gated_thread_runs_its_body_only_after_a_live_release() {
        let (sender, receiver) = std::sync::mpsc::channel::<()>();
        let (handle, gate) = spawn_gated("test-gated", move || {
            let _ = sender.send(());
        }).expect("start the gated thread");
        assert!(receiver.recv_timeout(Duration::from_millis(200)).is_err(), "the body does not run before the gate is released");
        gate.release(true);
        assert!(receiver.recv_timeout(Duration::from_secs(5)).is_ok(), "the body runs once the gate is released live");
        handle.join().expect("the gated thread ends");
        let (sender, receiver) = std::sync::mpsc::channel::<()>();
        let (handle, gate) = spawn_gated("test-gated-dead", move || {
            let _ = sender.send(());
        }).expect("start the gated thread");
        gate.release(false);
        handle.join().expect("a dead release ends the thread");
        assert!(receiver.try_recv().is_err(), "and its body never ran");
    }

    #[test]
    fn a_failed_grant_line_is_not_masked_by_a_later_line_that_worked() {
        // The sink takes the grant line's write as failed and the
        // next line's as written: the record of the grant is the
        // outcome of its own line, not of the last write made.
        let outcomes = Arc::new(Mutex::new(std::collections::VecDeque::from(vec![false, true, true])));
        let scripted = outcomes.clone();
        let last_written = Arc::new(AtomicBool::new(true));
        let seen = last_written.clone();
        let write = move |_: &str| {
            let written = relock(&scripted).pop_front().unwrap_or(true);
            seen.store(written, Ordering::SeqCst);
            written
        };
        let sink = Arc::new(Sink::start("test-stderr", Box::new(write), 8).expect("start the sink"));
        let log = Log { sinks: vec![sink.clone()], file_sink: None, blocked: Arc::new(AtomicBool::new(false)), only: Some(StandardErrorOnly { sink, last_written }) };
        assert!(!log.record("granted", 5_000), "a grant whose own line failed is not recorded, though the next line is written");
        assert!(log.record("granted again", 5_000), "a line that is written is a record");
        log.finish(1_000);
    }
