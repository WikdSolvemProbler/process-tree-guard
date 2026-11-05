// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

    use super::*;

        use crate::sys::windows::{

        ASYNC_CANCELED, ASYNC_COMPLETED, ASYNC_ERROR, ASYNC_STARTED, AssignProcessToJobObject,

        Bool, CREDUI_MAX_MESSAGE_LENGTH, CREDUI_PASSWORD_CHARS, CRED_UNPROTECTED,

        CheckAvailabilityAsyncFn, CloseHandle, Com, ConsoleApi, ConsoleCtrlHandlerFn,

        CreateEventW, CreateFileW, CreateJobObjectW, CredUiInfoW, DesktopApi, ERROR_CANCELLED,

        ERROR_LOGON_FAILURE, EXTENDED_LIMIT_CLASS, E_FAIL, E_NOTIMPL, E_UNEXPECTED,

        ExtendedLimitInformation, FILE_SHARE_READ_AND_WRITE, GENERIC_WRITE, GetConsoleMode,

        GetConsoleWindow, GetCurrentProcess, GetCurrentThreadId, GetLastError, Guid, HString,

        Handle, IID_ASYNC_INFO, IID_AVAILABILITY_OPERATION, IID_CONSENT_INTEROP,

        IID_CONSENT_STATICS, IID_RESULT_OPERATION, INVALID_HANDLE, KILL_ON_JOB_CLOSE, LocalFree,

        Msg, OPEN_EXISTING, PM_REMOVE, SetEvent, SetInformationJobObject, WAIT_FAILED,

        WAIT_OBJECT_0, WAIT_TIMEOUT, WM_SYSTIMER, WM_TIMER, WSF_VISIBLE, WaitForSingleObject,

        WindowApi, WndClassExW, desktop_api, hello_api, slot, wide, window_api,

        window_station_api,

    };
    use crate::extend::{is_format, DEFAULT_MAX_EXTENSION};
    use std::collections::VecDeque;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    /// What the stub prompt does on each call, in order.
    enum Answer {
        Password(&'static str),
        Cancel,
        Unavailable(&'static str),
        Abandoned(&'static str),
        NotShown(&'static str),
    }

    /// A prompt that answers from a script, and keeps what it was asked.
    struct StubPrompt {
        answers: Mutex<VecDeque<Answer>>,
        asked: Mutex<Vec<(u32, u32, String)>>,
        /// The reason `begin` gives for refusing, if it does.
        closed: Option<&'static str>,
    }

    impl StubPrompt {
        fn new(answers: Vec<Answer>) -> StubPrompt {
            StubPrompt { answers: Mutex::new(answers.into()), asked: Mutex::new(Vec::new()), closed: None }
        }

        fn asked(&self) -> Vec<(u32, u32, String)> {
            self.asked.lock().unwrap().clone()
        }
    }

    impl Prompt for StubPrompt {
        fn begin(&self, _text: &str) -> Result<Claim, String> {
            match self.closed {
                Some(reason) => Err(reason.into()),
                None => Ok(Claim::none()),
            }
        }

        fn ask(&self, attempt: u32, last_error: u32, text: &str) -> Asked {
            self.asked.lock().unwrap().push((attempt, last_error, text.to_string()));
            match self.answers.lock().unwrap().pop_front().expect("the prompt was asked more often than the test allows") {
                Answer::Password(password) => Asked::Entered(Box::new(Credentials::with("me", "", password))),
                Answer::Cancel => Asked::Cancelled,
                Answer::Unavailable(reason) => Asked::Unavailable(reason.into()),
                Answer::Abandoned(reason) => Asked::Abandoned(reason.into()),
                Answer::NotShown(reason) => Asked::NotShown(reason.into()),
            }
        }
    }

    /// A verifier that accepts one password, for the account of the guard or for another, or fails.
    struct StubVerifier {
        right: &'static str,
        same_user: bool,
        failure: Option<u32>,
    }

    impl Verifier for StubVerifier {
        fn verify(&self, credentials: Box<Credentials>) -> Verified {
            if let Some(code) = self.failure {
                return Verified::Failed(code);
            }
            if credentials.password_units() != self.right.encode_utf16().collect::<Vec<u16>>() {
                Verified::Wrong
            } else if self.same_user {
                Verified::Approved
            } else {
                Verified::OtherUser
            }
        }
    }

    fn settings(command: &str) -> Settings {
        Settings {
            max_seconds: 86_400.0,
            log_path: None,
            exception: Some(Exception { method: Method::Console, command: command.into(), timeout: 20.0 }),
        }
    }

    /// The parent the prompt is told in the tests.
    const STARTER: &str = "starter.exe";

    /// `authorize` with no owner to watch and the same names every time.
    fn authorize_with(settings: &Settings, prompt: &dyn Prompt, verifier: &dyn Verifier, log: &Log) -> Result<(), GuardError> {
        authorize_owned(settings, prompt, verifier, &Owners::empty(), log)
    }

    fn authorize_owned(settings: &Settings, prompt: &dyn Prompt, verifier: &dyn Verifier, owners: &Owners, log: &Log) -> Result<(), GuardError> {
        authorize(settings, &Subject { account: "PC\\me", parent: STARTER, program: "prog.exe" }, prompt, verifier, owners, log)
    }

    fn lines_of(lines: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        lines.lock().unwrap().clone()
    }

    fn decisions(lines: &[String]) -> Vec<&str> {
        lines.iter().map(|line| if line.contains("decision=exception-approved") { "approved" } else if line.contains("decision=exception-refused") { "refused" } else { "other" }).collect()
    }

    /// An event that is signalled, or is not; a process handle stands in for it in the tests of the owner.
    fn event(signalled: bool) -> Handle {
        let handle = unsafe { CreateEventW(std::ptr::null_mut(), 1, signalled as Bool, std::ptr::null()) };
        assert!(!handle.is_null());
        handle
    }

    #[test]
    fn the_right_password_of_the_signed_in_user_approves_and_the_line_is_the_record() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password("correct horse")]);
        let verifier = StubVerifier { right: "correct horse", same_user: true, failure: None };
        assert!(authorize_with(&settings("C:\\tools\\run.exe --all"), &prompt, &verifier, &log).is_ok());
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(decisions(&lines), ["approved"], "{:?}", lines);
        assert!(lines[0].contains("requested=86400 hours=24 granted=0 decision=exception-approved attempt=1 reason=\"a cap of 24 hours was approved"), "{}", lines[0]);
        assert!(lines[0].starts_with("PROCESS_TREE_GUARD_EXTENSION: time=") && !lines[0].contains("correct horse") && !lines[0].contains("user="), "{}", lines[0]);
        let asked = prompt.asked();
        assert_eq!(asked.len(), 1);
        assert_eq!((asked[0].0, asked[0].1), (1, 0), "the first attempt has no error before it");
        assert!(asked[0].2.contains("Command line: [C:\\tools\\run.exe --all] Program: [prog.exe] Account: [PC\\me] Parent process, as the system reports it: [starter.exe] ")
            && asked[0].2.contains("up to 24 hours at a time"), "{}", asked[0].2);
        // The record says what was approved and for which account: the command line as the prompt shows it, and the guard's own account.
        assert!(lines[0].contains("account=\"PC\\\\me\" command=\"C:\\\\tools\\\\run.exe --all\" method=\"console\" timeout=20 started_by=\"starter.exe\" requested=86400"), "{}", lines[0]);
        assert!(lines[0].contains("the password of the account the guard runs as was given at the console prompt") && !lines[0].contains("signed-in user gave"), "the approval says what the guard knows: {}", lines[0]);
    }

    #[test]
    fn a_wrong_password_is_an_attempt_used_and_the_prompt_says_why_the_next_time() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password("wrong"), Answer::Password("right")]);
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        assert!(authorize_with(&settings("x"), &prompt, &verifier, &log).is_ok());
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(decisions(&lines), ["refused", "approved"], "{:?}", lines);
        assert!(lines[0].contains("attempt=1 ") && lines[0].contains("the user name or password was not accepted") && !lines[0].contains("no attempts are left"), "{}", lines[0]);
        assert!(lines[1].contains("attempt=2 "), "{}", lines[1]);
        let asked = prompt.asked();
        assert_eq!((asked[1].0, asked[1].1), (2, ERROR_LOGON_FAILURE), "the second prompt is told the logon failed");
        assert!(lines.iter().all(|line| !line.contains("wrong") && !line.contains("right\"")), "no password is in the log: {:?}", lines);
    }

    #[test]
    fn a_password_of_another_account_does_not_approve_counts_and_is_not_told_apart_from_a_wrong_one() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password("theirs"), Answer::Password("theirs"), Answer::Password("theirs")]);
        let verifier = StubVerifier { right: "theirs", same_user: false, failure: None };
        let error = authorize_with(&settings("x"), &prompt, &verifier, &log).expect_err("another account's password is refused");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(decisions(&lines), ["refused", "refused", "refused"], "{:?}", lines);
        assert!(lines.iter().all(|line| line.contains("reason=\"the user name or password was not accepted")), "the reason is the same text as for a wrong password: {:?}", lines);
        assert!(error.message().contains("exception refused") && error.message().contains("the run did not start"), "{}", error);
    }

    #[test]
    fn three_failures_end_the_attempts_and_a_fourth_is_never_asked() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        // A fourth answer is scripted, and the stub panics if it is not left unasked: it is left in the queue.
        let prompt = StubPrompt::new(vec![Answer::Password("a"), Answer::Password("b"), Answer::Password("c"), Answer::Password("right")]);
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        let error = authorize_with(&settings("x"), &prompt, &verifier, &log).expect_err("three wrong passwords end the run");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(prompt.asked().len(), 3, "the prompt is asked three times and no more");
        assert_eq!(decisions(&lines), ["refused", "refused", "refused"], "{:?}", lines);
        assert!(lines[0].contains("attempt=1 ") && lines[1].contains("attempt=2 ") && lines[2].contains("attempt=3 "), "{:?}", lines);
        assert!(!lines[1].contains("no attempts are left") && lines[2].contains("no attempts are left"), "{:?}", lines);
        assert!(error.message().contains("no attempts are left"), "{}", error);
    }

    #[test]
    fn an_empty_password_is_never_checked_and_never_approves() {
        // A verifier that would approve anything: the empty password does not reach it.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password(""), Answer::Password(""), Answer::Password(""), Answer::Password("")]);
        let verifier = StubVerifier { right: "", same_user: true, failure: None };
        let error = authorize_with(&settings("x"), &prompt, &verifier, &log).expect_err("an empty password is refused three times");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(prompt.asked().len(), 3, "an empty password uses an attempt like any other");
        assert_eq!(decisions(&lines), ["refused", "refused", "refused"], "{:?}", lines);
        assert!(error.message().contains("no attempts are left") && !lines.iter().any(|line| line.contains("exception-approved")), "{} {:?}", error, lines);
        assert!(lines.iter().all(|line| line.contains(EMPTY_PASSWORD) && !line.contains(NOT_ACCEPTED)), "the record does not say that the system rejected a credential when none was sent: {:?}", lines);
        // And the system verifier turns it away before it calls the system.
        let owners = Owners::empty();
        assert_eq!(LogonVerifier { owners: &owners }.verify(Box::new(Credentials::with("me", "", ""))), Verified::Wrong);
        assert!(Credentials::with("me", "", "").password_is_empty() && !Credentials::with("me", "", "x").password_is_empty());
    }

    #[test]
    fn a_cancel_ends_the_attempts_at_once() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password("wrong"), Answer::Cancel, Answer::Password("right")]);
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        let error = authorize_with(&settings("x"), &prompt, &verifier, &log).expect_err("a cancel refuses the run");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(prompt.asked().len(), 2, "nothing is asked after the cancel");
        assert_eq!(decisions(&lines), ["refused", "refused"], "{:?}", lines);
        assert!(lines[1].contains("attempt=2 ") && lines[1].contains("the prompt was cancelled"), "{}", lines[1]);
        assert!(error.message().contains("cancelled"), "{}", error);
    }

    /// A prompt that takes a lock of this test's own as its right to be open, and cancels when asked.
    struct HoldsALock(String);

    impl Prompt for HoldsALock {
        fn begin(&self, _text: &str) -> Result<Claim, String> {
            Ok(Claim { _lock: Some(PromptLock::take_named(&self.0)?) })
        }

        fn ask(&self, _attempt: u32, _last_error: u32, _text: &str) -> Asked {
            Asked::Cancelled
        }
    }

    #[test]
    fn a_refusal_after_a_prompt_was_shown_keeps_the_right_to_a_prompt_for_a_while_and_other_ends_do_not() {
        COOL_DOWNS.with(|count| count.set(0));
        RETRY_DELAY.with(|delay| delay.set(600));
        // A name of this test's own, so that a prompt of a real guard in the same session is not touched.
        let name = format!("Local\\ProcessTreeGuardCoolDown{}", std::process::id());
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        // Another thread tries to take the right as soon as the refusal is in the log, while the cool-down goes on.
        let watcher = {
            let (kept, name) = (kept.clone(), name.clone());
            std::thread::spawn(move || {
                let end = Instant::now() + Duration::from_secs(10);
                while kept.lock().unwrap().is_empty() && Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(5));
                }
                PromptLock::take_named(&name).is_ok()
            })
        };
        let started = Instant::now();
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        assert!(authorize_with(&settings("x"), &HoldsALock(name.clone()), &verifier, &log).is_err(), "the cancel refuses the run");
        assert!(started.elapsed() >= Duration::from_millis(600), "the right is kept after the refusal: {:?}", started.elapsed());
        log.finish(1_000);
        assert!(!watcher.join().unwrap(), "a guard started in that time finds the right taken and is refused at once");
        assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 1);
        assert!(PromptLock::take_named(&name).is_ok(), "the right returns when the guard has returned");
        // The same for a prompt that fails after it was shown, a failure of the system, and three names or passwords that were not accepted.
        for answers in [vec![Answer::Unavailable("the prompt failed")], vec![Answer::Password("right")], vec![Answer::Password("a"), Answer::Password("b"), Answer::Password("c")]] {
            let failing = StubVerifier { right: "other", same_user: true, failure: if answers.len() == 1 && matches!(answers[0], Answer::Password(_)) { Some(1909) } else { None } };
            let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
            assert!(authorize_with(&settings("x"), &StubPrompt::new(answers), &failing, &log).is_err());
            log.finish(1_000);
            assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 1, "one cool-down for each refusal after a prompt");
        }
        // Not after an approval, a prompt that was never shown, a starter that has gone, or a prompt left open at its limit (the process
        // ends at once, which is expected to end the dialog or the read): nobody is shown a prompt that follows.
        let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
        assert!(authorize_with(&settings("x"), &StubPrompt::new(vec![Answer::Abandoned("no answer within 300 seconds")]), &verifier, &log).is_err());
        assert!(authorize_with(&settings("x"), &StubPrompt::new(vec![Answer::Password("right")]), &verifier, &log).is_ok());
        let mut closed = StubPrompt::new(vec![]);
        closed.closed = Some("nobody is present");
        assert!(authorize_with(&settings("x"), &closed, &verifier, &log).is_err());
        assert!(authorize_with(&settings("x"), &StubPrompt::new(vec![Answer::NotShown("the prompt could not be started")]), &verifier, &log).is_err());
        assert!(authorize_owned(&settings("x"), &StubPrompt::new(vec![Answer::Password("right")]), &verifier, &Owners::of(vec![event(true)]), &log).is_err());
        log.finish(1_000);
        assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 0);
        RETRY_DELAY.with(|delay| delay.set(0));
    }

    #[test]
    fn a_prompt_that_cannot_be_shown_is_refused_before_any_attempt_and_says_why() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let mut prompt = StubPrompt::new(vec![]);
        prompt.closed = Some("standard input is not an interactive console");
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        let error = authorize_with(&settings("x"), &prompt, &verifier, &log).expect_err("nobody can be asked");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(decisions(&lines), ["refused"], "{:?}", lines);
        assert!(lines[0].contains("attempt=0 ") && lines[0].contains("standard input is not an interactive console"), "{}", lines[0]);
        assert!(error.message().contains("standard input is not an interactive console") && prompt.asked().is_empty(), "{}", error);
    }

    #[test]
    fn a_prompt_that_fails_after_it_was_shown_is_an_attempt_and_one_that_never_started_is_not() {
        // Unanswered after it was shown: the attempt it was.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Abandoned("no answer within 300 seconds")]);
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        assert!(authorize_with(&settings("x"), &prompt, &verifier, &log).is_err());
        log.finish(1_000);
        assert!(lines_of(&kept)[0].contains("attempt=1 ") && lines_of(&kept)[0].contains("no answer within 300 seconds"), "{:?}", lines_of(&kept));
        // Not started for the first attempt: none was made. Not started for the second: one was.
        for (answers, attempt) in [(vec![Answer::NotShown("the prompt could not be started")], 0), (vec![Answer::Password("wrong"), Answer::NotShown("the prompt could not be started")], 1)] {
            let kept = Arc::new(Mutex::new(Vec::new()));
            let log = Log::capture(kept.clone(), None);
            let prompt = StubPrompt::new(answers);
            assert!(authorize_with(&settings("x"), &prompt, &verifier, &log).is_err());
            log.finish(1_000);
            let last = lines_of(&kept).last().cloned().unwrap_or_default();
            assert!(last.contains(&format!("attempt={} ", attempt)) && last.contains("the prompt could not be started"), "{}", last);
        }
    }

    #[test]
    fn a_failure_that_more_attempts_cannot_mend_ends_them_and_names_the_error() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password("right")]);
        let locked_out = StubVerifier { right: "right", same_user: true, failure: Some(1909) };
        let error = authorize_with(&settings("x"), &prompt, &locked_out, &log).expect_err("a locked out account is refused");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(prompt.asked().len(), 1);
        assert!(lines[0].contains("[WinError 1909]") && error.message().contains("[WinError 1909]"), "{:?} {}", lines, error);
        // An account that may not log on locally is the same kind of failure, shown at the desktop prompt.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password("right")]);
        let no_right = StubVerifier { right: "right", same_user: true, failure: Some(1385) };
        let mut desktop = settings("x");
        if let Some(exception) = desktop.exception.as_mut() {
            exception.method = Method::Desktop;
        }
        assert!(authorize_with(&desktop, &prompt, &no_right, &log).is_err());
        log.finish(1_000);
        assert!(lines_of(&kept)[0].contains("[WinError 1385]") && prompt.asked().len() == 1, "{:?}", lines_of(&kept));
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let unknown = StubVerifier { right: "right", same_user: true, failure: Some(UNIDENTIFIED) };
        let prompt = StubPrompt::new(vec![Answer::Password("right")]);
        assert!(authorize_with(&settings("x"), &prompt, &unknown, &log).is_err());
        log.finish(1_000);
        assert!(lines_of(&kept)[0].contains("could not be identified"), "{:?}", lines_of(&kept));
    }

    #[test]
    fn at_the_console_an_error_given_only_for_a_right_password_is_not_told_apart_from_a_wrong_one() {
        // 1380 and 1385 have the same text in the system, so both are in the list.
        assert!(AFTER_PASSWORD_CHECKED.contains(&1380) && AFTER_PASSWORD_CHECKED.contains(&1385));
        for code in AFTER_PASSWORD_CHECKED {
            let kept = Arc::new(Mutex::new(Vec::new()));
            let log = Log::capture(kept.clone(), None);
            let prompt = StubPrompt::new(vec![Answer::Password("right"), Answer::Password("right"), Answer::Password("right")]);
            let verifier = StubVerifier { right: "right", same_user: true, failure: Some(code) };
            let error = authorize_with(&settings("x"), &prompt, &verifier, &log).expect_err("the run is refused");
            log.finish(1_000);
            let lines = lines_of(&kept);
            assert_eq!(prompt.asked().len(), 3, "{}: a program that types is given the answer of a wrong password, attempts included", code);
            assert_eq!(lines.len(), 3, "{:?}", lines);
            assert!(lines.iter().all(|line| line.contains(NOT_ACCEPTED) && !line.contains(&format!("WinError {}", code))), "{}: {:?}", code, lines);
            assert!(!error.message().contains("WinError"), "{}: {}", code, error);
        }
        // Another failure of the system is still named.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password("right")]);
        let verifier = StubVerifier { right: "right", same_user: true, failure: Some(1909) };
        assert!(authorize_with(&settings("x"), &prompt, &verifier, &log).is_err());
        log.finish(1_000);
        assert!(lines_of(&kept)[0].contains("[WinError 1909]"), "{:?}", lines_of(&kept));
    }

    #[test]
    fn an_approval_that_no_record_holds_does_not_stand() {
        let fail = Arc::new(AtomicBool::new(true));
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), Some(fail.clone()));
        let prompt = StubPrompt::new(vec![Answer::Password("right")]);
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        let error = authorize_with(&settings("x"), &prompt, &verifier, &log).expect_err("an approval that cannot be written is not one");
        assert!(error.message().contains("could not be recorded"), "{}", error);
        log.finish(1_000);
        // The same log with a standard error that works records the approval and the run goes on.
        fail.store(false, Ordering::SeqCst);
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), Some(fail));
        let prompt = StubPrompt::new(vec![Answer::Password("right")]);
        assert!(authorize_with(&settings("x"), &prompt, &verifier, &log).is_ok());
        log.finish(1_000);
        assert_eq!(decisions(&lines_of(&kept)), ["approved"]);
    }

    #[test]
    fn an_approval_that_the_audit_file_could_not_take_does_not_stand() {
        let folder = crate::test_temp::fresh_dir("ptg-approval");
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        // A file in a folder that is not there takes no line: the run is refused, as for a standard error that takes none.
        let bad = Log::start(Some(Arc::from(folder.join("missing").join("audit.log").as_path())), 1 << 20, None).expect("start a log whose file cannot be written");
        let error = authorize_with(&settings("x"), &StubPrompt::new(vec![Answer::Password("right")]), &verifier, &bad).expect_err("an approval that no file holds is not one");
        bad.finish(1_000);
        assert!(error.message().contains("could not be recorded"), "{}", error);
        // A file that takes lines holds the approval, and the run goes on.
        let path = folder.join("audit.log");
        let good = Log::start(Some(Arc::from(path.as_path())), 1 << 20, std::fs::OpenOptions::new().create(true).append(true).open(&path).ok()).expect("start a log with a file");
        assert!(authorize_with(&settings("x"), &StubPrompt::new(vec![Answer::Password("right")]), &verifier, &good).is_ok());
        good.finish(1_000);
        let contents = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(contents.contains("decision=exception-approved") && contents.contains("account=\"PC\\\\me\""), "{}", contents);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_refusal_the_audit_file_could_not_queue_says_so_in_its_error_and_still_refuses() {
        // The file sink's queue is closed: the copy takes the line,
        // the audit file takes nothing, and the error the run ends
        // with names the decision as not recorded in the audit file.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture_with_closed_file(kept.clone());
        let subject = Subject { account: "PC\\me", parent: STARTER, program: "prog.exe" };
        let error = refuse(&settings("x"), &log, &subject, 0, "a test refusal");
        assert_eq!(error.message(), "exception refused: a test refusal; the run did not start; the decision was not recorded in the audit file");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(decisions(&lines), ["refused"], "the copy still holds the refusal line: {:?}", lines);
    }

    #[test]
    fn a_refusal_the_audit_file_took_is_in_the_file_and_its_error_is_unchanged() {
        let folder = crate::test_temp::fresh_dir("ptg-refusal");
        let path = folder.join("audit.log");
        let log = Log::start(Some(Arc::from(path.as_path())), 1 << 20, std::fs::OpenOptions::new().create(true).append(true).open(&path).ok()).expect("start a log with a file");
        let subject = Subject { account: "PC\\me", parent: STARTER, program: "prog.exe" };
        let error = refuse(&settings("x"), &log, &subject, 0, "a test refusal");
        assert_eq!(error.message(), "exception refused: a test refusal; the run did not start", "a recorded refusal's error is the error it has always been, byte for byte");
        log.finish(1_000);
        let contents = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(contents.contains("decision=exception-refused") && contents.contains("a test refusal"), "the refusal is in the audit file: {}", contents);
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_panic_zeroes_the_secrets_that_are_alive_before_the_process_would_abort() {
        // The hook of the tests zeroes the buffers of the panicking thread only, so a panic here does not wipe a test beside this one.
        HOOK_FOR_THIS_THREAD.store(true, Ordering::SeqCst);
        install_panic_hook();
        let credentials = Credentials::with("me", "DOM", "hunter2");
        let address = credentials.password.0.as_ptr() as usize;
        assert!(live().iter().any(|placed| placed.0 == address), "a buffer that holds a secret is listed while it is alive");
        assert_eq!(credentials.password_units().len(), 7);
        // The destructor does not run for this value (it is outside the closure that panics), as in a release build that aborts.
        let panicked = std::panic::catch_unwind(|| -> () { panic!("a panic while a password is alive") }).is_err();
        assert!(panicked);
        assert!(credentials.password_units().is_empty() && credentials.names() == Some((String::new(), String::new())), "the hook zeroed every buffer");
        drop(credentials);
        assert!(!live().iter().any(|placed| placed.0 == address), "a buffer that was dropped is no longer listed, so the hook never writes to freed memory");
        // The authentication buffer of the prompt is listed and zeroed in the same way.
        unsafe extern "system" fn keep(_memory: *mut c_void) {}
        let mut raw = [0xffu8; 24];
        register(raw.as_mut_ptr(), raw.len());
        let _ = std::panic::catch_unwind(|| -> () { panic!("a panic while the buffer of the prompt is alive") });
        assert!(raw.iter().all(|byte| *byte == 0), "the buffer the prompt returned is zero after a panic");
        // Dropping the buffer takes it out of the list again, so the list does not keep an address that was freed.
        drop(OutBuffer { pointer: raw.as_mut_ptr() as *mut c_void, size: raw.len() as u32, free: keep });
        assert!(!live().iter().any(|placed| placed.0 == raw.as_ptr() as usize));
    }

    /// Set in the environment of the copy of this test executable in which no test has installed the panic hook.
    const HOOK_CHILD: &str = "PROCESS_TREE_GUARD_UNIT_TEST_PANIC_HOOK_CHILD";

    #[test]
    fn making_a_buffer_for_a_secret_is_enough_to_have_the_hook_that_zeroes_it_on_a_panic() {
        if std::env::var_os(HOOK_CHILD).is_some() {
            // The copy: nothing here installs the hook by name, as `decide` does in a program. A buffer for a secret is made and filled,
            // and a panic is caught while it is alive, as the abort of a release build leaves it.
            HOOK_FOR_THIS_THREAD.store(true, Ordering::SeqCst);
            let mut secret = zeroed(8);
            secret.0.iter_mut().for_each(|unit| *unit = 0x5a5a);
            let _ = std::panic::catch_unwind(|| -> () { panic!("a panic while a secret is alive") });
            println!("HOOK {}", if secret.0.iter().all(|unit| *unit == 0) { "zeroed" } else { "left" });
            return;
        }
        // The hook is installed once for the whole process, so the check needs a copy of this executable in which no other test has done it.
        let output = std::process::Command::new(std::env::current_exe().expect("the path of this test executable"))
            .args(["making_a_buffer_for_a_secret_is_enough_to_have_the_hook", "--nocapture", "--test-threads=1"])
            .env(HOOK_CHILD, "1").stdin(std::process::Stdio::null()).output().expect("run the copy");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(output.status.success() && stdout.contains("HOOK zeroed"), "a secret must not be alive without the hook that zeroes it: {}", stdout);
    }

    #[test]
    fn the_credential_dialog_is_refused_a_message_longer_than_it_takes_and_checking_that_shows_nothing() {
        // `ready` loads the libraries and measures the message; the dialog is not called. The limit counts UTF-16 units, as the dialog does.
        if desktop_api().is_none() || window_station_api().is_none() {
            eprintln!("NOT ASSERTED: the libraries of the credential dialog cannot be loaded here");
            return;
        }
        let owners = Owners::empty();
        let prompt = DesktopPrompt::new(&owners);
        assert!(prompt.ready("short").is_ok());
        assert!(prompt.ready(&"x".repeat(CREDUI_MAX_MESSAGE_LENGTH - 1)).is_ok(), "one unit under the limit is taken");
        let reason = prompt.ready(&"x".repeat(CREDUI_MAX_MESSAGE_LENGTH)).expect_err("a message at the limit is not taken");
        assert!(reason.contains("longer than the dialog takes"), "{}", reason);
        let wide_characters = "\u{1d11e}".repeat(CREDUI_MAX_MESSAGE_LENGTH / 2);
        assert!(wide_characters.chars().count() < CREDUI_MAX_MESSAGE_LENGTH && prompt.ready(&wide_characters).is_err(), "each character outside the basic plane is two units");
    }

    #[test]
    fn the_console_mode_is_put_back_on_a_control_event_only_while_a_mode_was_read_and_the_guard_is_not_ended_by_the_handler() {
        use std::sync::atomic::AtomicU32;
        static CALLS: AtomicU32 = AtomicU32::new(0);
        unsafe extern "system" fn count(_handler: Option<ConsoleCtrlHandlerFn>, _add: Bool) -> Bool {
            CALLS.fetch_add(1, Ordering::SeqCst);
            1
        }
        // With no mode read there is nothing to put back: the handler is not registered, and a control event restores nothing.
        let api = ConsoleApi { prompt: unused_prompt, handler: count, expunge: None };
        drop(EchoGuard::new(&api, None));
        assert_eq!(CALLS.load(Ordering::SeqCst), 0, "no handler is added or removed when there is no mode to put back");
        assert_eq!(MODE_TO_RESTORE.load(Ordering::SeqCst), NO_MODE);
        // The handler never claims the event, so that the default action still ends the guard.
        assert_eq!(unsafe { restore_on_control_event(0) }, 0);
        assert_eq!(unsafe { restore_on_control_event(2) }, 0);
        // Where the call to clear the history is missing nothing happens.
        expunge_history(&api);
    }

    unsafe extern "system" fn unused_prompt(_target: *const u16, _context: *mut c_void, _error: u32, _user: *mut u16, _user_chars: u32,
        _password: *mut u16, _password_chars: u32, _save: *mut Bool, _flags: u32) -> u32 {
        ERROR_CANCELLED
    }

    #[test]
    fn a_run_inside_another_run_is_refused_without_asking() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let error = decide(&settings("x"), true, "prog.exe", STARTER, &[], &Owners::empty(), &log).expect_err("an exception cannot be granted inside another run");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(decisions(&lines), ["refused"]);
        assert!(lines[0].contains("attempt=0 ") && lines[0].contains(NESTED) && error.message().contains(NESTED), "{:?} {}", lines, error);
        assert!(lines[0].contains("method=\"console\" timeout=20 started_by=\"\" requested="), "the refusal names the method and the deadline: {}", lines[0]);
        // A run with no exception has nothing to decide, held or not.
        let plain = Settings { max_seconds: DEFAULT_MAX_EXTENSION, log_path: None, exception: None };
        assert!(decide(&plain, true, "prog.exe", STARTER, &[], &Owners::empty(), &Log::capture(Arc::new(Mutex::new(Vec::new())), None)).is_ok());
        // Inside another run comes first: the guard that holds the run is above this one too.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let above = vec!["C:\\tools\\process-tree-guard.exe".to_string()];
        let error = decide(&settings("x"), true, "prog.exe", STARTER, &above, &Owners::empty(), &log).expect_err("nested");
        log.finish(1_000);
        assert!(error.message().contains(NESTED) && !error.message().contains("another guard started"), "{}", error);
    }

    #[test]
    fn a_run_that_another_guard_started_is_refused_before_any_prompt_and_says_so() {
        let names = ["process-tree-guard.exe", "process-guard.exe", "process-tree-guard-deadline.exe", "process-tree-guard-contain.exe",
            "process-tree-guard-auto.exe", "process-guard-auto.exe", "process-tree-guard-deadline-auto.exe"];
        for name in names {
            for image in [format!("C:\\Program Files\\Guard\\{}", name), format!("c:/tools/{}", name.to_ascii_uppercase()), name.to_string()] {
                let chain = vec!["C:\\Windows\\System32\\cmd.exe".to_string(), image.clone(), "C:\\Windows\\explorer.exe".to_string()];
                assert_eq!(guard_above(&chain), Some(name.to_string()), "{}", image);
                let kept = Arc::new(Mutex::new(Vec::new()));
                let log = Log::capture(kept.clone(), None);
                // The real prompt would be reached if this were not refused first, and nothing here can be answered.
                let error = decide(&settings("x"), false, "prog.exe", STARTER, &chain, &Owners::empty(), &log).expect_err("a run that a guard started is not asked");
                log.finish(1_000);
                let lines = lines_of(&kept);
                assert_eq!(decisions(&lines), ["refused"], "{:?}", lines);
                assert!(lines[0].contains("attempt=0 ") && lines[0].contains("reason=\"an exception above the cap is not asked for by a run that another guard started")
                    && lines[0].contains("started_by=\"starter.exe\""), "{}", lines[0]);
                // The reason names the test that made the refusal, and what it found.
                assert!(lines[0].contains(&format!("({}{})", FOUND_ABOVE, name)) && !lines[0].contains(JOB_KILLS), "{}", lines[0]);
                assert!(error.message().contains(&format!("{}{}", FOUND_ABOVE, name)), "{}", error);
                assert!(error.message().contains("start it from a terminal that is not under a guard") && error.message().contains("the run did not start"), "{}", error);
            }
        }
        // A file name whose stem ends in -auto selects automatic mode, whatever the rest of the name, so a host can name its wrapper freely.
        for (image, name) in [("C:\\hosts\\harness-auto.exe", "harness-auto.exe"), ("C:\\x\\Host-Auto.EXE", "host-auto.exe"), ("tool-auto", "tool-auto"), ("D:/a/b.c-auto.exe", "b.c-auto.exe")] {
            let chain = vec!["C:\\Windows\\System32\\cmd.exe".to_string(), image.to_string()];
            assert_eq!(guard_above(&chain), Some(name.to_string()), "{}", image);
        }
        // Only a program of the guard counts, by its file name and not by a folder or a part of it.
        for chain in [vec![], vec![String::new()], vec!["C:\\tools\\other.exe".to_string(), "C:\\process-tree-guard.exe\\cmd.exe".to_string()],
            vec!["C:\\tools\\my-process-tree-guard.exe".to_string(), "C:\\tools\\process-tree-guard-gate.exe".to_string(), "C:\\tools\\testkit.exe".to_string()],
            vec!["C:\\tools\\auto.exe".to_string(), "C:\\tools\\automatic.exe".to_string(), "C:\\auto-x\\cmd.exe".to_string(), "C:\\tools\\x-auto.exe\\cmd.exe".to_string()]] {
            assert_eq!(guard_above(&chain), None, "{:?}", chain);
        }
        // The nearest is the one named, and a guard far above counts as much as the parent.
        let far: Vec<String> = (0..30).map(|n| format!("C:\\tools\\p{}.exe", n)).chain(std::iter::once("C:\\x\\Process-Guard-Auto.exe".to_string())).collect();
        assert_eq!(guard_above(&far), Some("process-guard-auto.exe".to_string()));
    }

    #[test]
    fn a_name_whose_only_dot_leads_it_is_its_own_stem_for_the_automatic_copy_test() {
        // lib.rs's stem rule, as `is_automatic_copy` takes it: a name that is exactly `..`, or whose only dot leads it, is its own stem,
        // so a leading-dot name that ends in -auto is an automatic copy, as a plain one is.
        for name in [".-auto", ".foo-auto", ".foo-AUTO", ".é-auto", "foo-auto", "foo-auto.exe", "-auto", "-auto.exe", "x-auto.", ".a-auto.exe"] {
            assert!(is_automatic_copy(name), "{}", name);
        }
        // A stem taken at a later dot, or a name without the suffix, is not: `..` is its own stem and under five long,
        // and in `..-auto`, `a.b-auto` and `foo.-auto` the stem the last dot leaves is `.`, `a` and `foo`.
        for name in ["..", "..-auto", "a.b-auto", "foo.-auto", "foo", "foo.exe", "auto", "plain"] {
            assert!(!is_automatic_copy(name), "{}", name);
        }
    }

    #[test]
    fn a_run_whose_starter_has_exited_is_refused_not_approved() {
        // The starter is gone before the first prompt: nothing is shown and no attempt is made.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Password("right")]);
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        let gone = Owners::of(vec![event(true)]);
        let error = authorize_owned(&settings("x"), &prompt, &verifier, &gone, &log).expect_err("a run nobody started is refused");
        log.finish(1_000);
        assert!(prompt.asked().is_empty() && lines_of(&kept)[0].contains("attempt=0 ") && error.message().contains("nobody is waiting for it"), "{:?} {}", lines_of(&kept), error);
        // The prompt reports the starter gone while it was open.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = StubPrompt::new(vec![Answer::Unavailable(OWNER_GONE)]);
        assert!(authorize_with(&settings("x"), &prompt, &verifier, &log).is_err());
        log.finish(1_000);
        assert!(lines_of(&kept)[0].contains("attempt=1 ") && lines_of(&kept)[0].contains(OWNER_GONE), "{:?}", lines_of(&kept));
        // A right password given just as the starter exits is not an approval, and no approval is logged.
        let starter = event(false);
        struct ExitsWhileAsked(Handle);
        impl Prompt for ExitsWhileAsked {
            fn begin(&self, _text: &str) -> Result<Claim, String> {
                Ok(Claim::none())
            }
            fn ask(&self, _attempt: u32, _last_error: u32, _text: &str) -> Asked {
                unsafe { SetEvent(self.0) };
                Asked::Entered(Box::new(Credentials::with("me", "", "right")))
            }
        }
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let owners = Owners::of(vec![starter]);
        let error = authorize_owned(&settings("x"), &ExitsWhileAsked(starter), &verifier, &owners, &log).expect_err("the starter exited");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert!(decisions(&lines) == ["refused"] && error.message().contains("nobody is waiting for it"), "{:?} {}", lines, error);
    }

    /// The text of the prompt of `method` for an account, a parent, the program and the command, for
    /// a run that starts with a deadline of 20 seconds.
    fn text_for(method: Method, account: &str, parent: &str, program: &str, command: &str, seconds: f64) -> String {
        prompt_text(&Subject { account, parent, program }, &Exception { method, command: command.into(), timeout: 20.0 }, seconds)
    }

    /// The text of the desktop prompt for a parent, the program and the command, in the account `PC\me`.
    fn text_of(parent: &str, program: &str, command: &str, seconds: f64) -> String {
        text_for(Method::Desktop, "PC\\me", parent, program, command, seconds)
    }

    #[test]
    fn the_message_names_the_product_what_the_cap_means_the_account_the_parent_and_the_command_with_nothing_that_can_forge_a_line() {
        let text = text_of("C:\\Apps\\host.exe", "C:\\Windows\\prog.exe", "C:\\Program Files\\tool.exe \"quoted\" --flag", 86_400.0);
        assert!(text.starts_with("Process Tree Guard is asked to run: Command line: [C:\\Program Files\\tool.exe \"quoted\" --flag] Program: [C:\\Windows\\prog.exe] Account: [PC\\me] Parent process, as the system reports it: [C:\\Apps\\host.exe] "),
            "the fields to check come first, and paths and quotes are shown as they are, not doubled: {}", text);
        assert!(text.ends_with("The processes of the run may each ask to keep it alive for up to 24 hours at a time, and may renew without a total limit. \
            The run starts with a deadline of 20 seconds. Enter the password of the account above to allow it, or cancel to refuse the run."), "{}", text);
        assert!(!text.contains("for up to 24 hours.") && !text.contains("run a command for up to") && !text.contains("signed-in user") && !text.contains("Started by"), "the cap is not described as a bound on the run: {}", text);
        // The deadline the run starts with is shown as it is (a fraction too), apart from the cap.
        let started = |timeout: f64| prompt_text(&Subject { account: "PC\\me", parent: "", program: "p" }, &Exception { method: Method::Desktop, command: "x".into(), timeout }, 86_400.0);
        assert!(started(86_400.0).contains("The run starts with a deadline of 86400 seconds."), "{}", started(86_400.0));
        assert!(started(0.5).contains("The run starts with a deadline of 0.5 seconds."), "{}", started(0.5));
        assert!(text_of("s", "p", "x", 3_600.0).contains("up to 1 hour at a time"), "one hour is not hours");
        assert!(text_of("s", "p", "x", 23_400.0).contains("up to 6.5 hours at a time"));
        // Rounded up and with the seconds beside it, so that the figure is never less than the cap and a cap just over the default shows.
        assert!(text_of("s", "p", "x", 21_601.0).contains("up to 6.01 hours (21601 seconds) at a time"), "{}", text_of("s", "p", "x", 21_601.0));
        assert!(text_of("s", "p", "x", 25_000.0).contains("up to 6.95 hours (25000 seconds) at a time"));
        // A parent or an account that cannot be read is said so.
        assert!(text_of("", "p", "x", 86_400.0).contains("Parent process, as the system reports it: [(unknown)] The processes"), "a parent that cannot be read is said so");
        assert!(text_for(Method::Desktop, "", "s", "p", "x", 86_400.0).contains("Account: [(unknown)]"), "an account that cannot be read is said so");
        // Controls, line separators and format characters are escaped; nothing can start another line or reorder the rest.
        let hostile = text_of("s", "p", "a\r\nb\u{85}c\u{2028}d\u{202e}e\u{200b}f\0g\ttab", 86_400.0);
        assert!(hostile.contains("Command line: [a\\u{d}\\u{a}b\\u{85}c\\u{2028}d\\u{202e}e\\u{200b}f\\u{0}g\\u{9}tab] Program: [p]"), "{}", hostile);
        assert!(!hostile.chars().any(|c| c.is_control() || c == '\u{2028}' || is_format(c)), "{}", hostile);
        // A command cannot close its own frame and write a label of its own: the brackets in it are escaped.
        let forged = text_of("s", "p", "x] Program: [C:\\Windows\\System32\\winver.exe", 86_400.0);
        assert!(forged.contains("Command line: [x\\u{5d} Program: \\u{5b}C:\\Windows\\System32\\winver.exe] Program: [p] Account:"), "{}", forged);
        assert_eq!(forged.matches('[').count(), 4);
        assert_eq!(forged.matches(']').count(), 4);
        // So can no name of an account: each field is framed alone.
        let account = text_for(Method::Desktop, "me] Parent process, as the system reports it: [C:\\Windows\\explorer.exe", "s", "p", "x", 86_400.0);
        assert_eq!(account.matches('[').count(), 4, "{}", account);
        assert_eq!(shown_command(""), "(none)");
    }

    #[test]
    fn the_message_says_how_to_refuse_for_the_method_that_asks_and_the_console_has_no_cancel() {
        let desktop = text_for(Method::Desktop, "PC\\me", "s", "p", "x", 86_400.0);
        let console = text_for(Method::Console, "PC\\me", "s", "p", "x", 86_400.0);
        assert!(desktop.ends_with("to allow it, or cancel to refuse the run."), "the desktop prompt has a cancel: {}", desktop);
        assert!(console.ends_with("to allow it, or leave the prompt unanswered for 300 seconds, or close this console, to refuse the run."), "{}", console);
        assert!(!console.contains("cancel"), "the console prompt has no cancel key and must not tell the user to cancel: {}", console);
        // Everything else is the same text.
        assert_eq!(desktop.replace("cancel to refuse the run", "X"), console.replace("leave the prompt unanswered for 300 seconds, or close this console, to refuse the run", "X"));
    }

    #[test]
    fn a_name_in_another_script_is_written_out_and_said_so_and_a_plain_one_has_no_note() {
        const NOTE: &str = " Note: \\u{..} is a character written out; check each field letter by letter.";
        // A Cyrillic es and o for the Latin c and o: the program looks like cmd.exe and is not.
        let program = "C:\\Windows\\System32\\\u{441}md.exe";
        let spoofed = text_for(Method::Desktop, "PC\\me", "C:\\Apps\\host.exe", program, "x", 86_400.0);
        assert!(spoofed.contains("Program: [C:\\Windows\\System32\\\\u{441}md.exe]"), "the look-alike letter is written out: {}", spoofed);
        assert!(spoofed.ends_with(NOTE), "{}", spoofed);
        assert!(!spoofed.contains('\u{441}'), "no letter of another script reaches the dialog: {}", spoofed);
        // Any of the fields can carry it: the account, the parent, the program and the command line.
        for (account, parent, program, command) in [("PC\\\u{43c}e", "s", "p", "x"), ("PC\\me", "\u{440}.exe", "p", "x"), ("PC\\me", "s", "\u{440}", "x"), ("PC\\me", "s", "p", "\u{3b1}")] {
            let text = text_for(Method::Desktop, account, parent, program, command, 86_400.0);
            assert!(text.ends_with(NOTE), "{}", text);
        }
        // Plain Latin has no note.
        let plain = text_of("C:\\Apps\\host.exe", "C:\\Windows\\System32\\cmd.exe", "cmd /c echo hello", 86_400.0);
        assert!(!plain.contains("Note:"), "{}", plain);
        // A command that holds the written-out form of a character has the note too, and an ordinary backslash has none.
        let imitating = text_of("C:\\Apps\\host.exe", "C:\\Windows\\System32\\cmd.exe", "echo a\\u{200b}b", 86_400.0);
        assert!(imitating.contains("Command line: [echo a\\u{5c}u{200b}b]") && imitating.ends_with(NOTE), "{}", imitating);
        // The note comes after the fields and the command line is the first of them.
        assert!(spoofed.starts_with("Process Tree Guard is asked to run: Command line: [x] Program: ["), "{}", spoofed);
    }

    #[test]
    fn the_command_keeps_its_start_and_a_path_keeps_its_end_and_each_says_how_much_was_left_out() {
        let exact = "a".repeat(COMMAND_LIMIT);
        assert_eq!(shown_command(&exact), exact, "a command of exactly the limit is shown whole");
        let over = "b".repeat(COMMAND_LIMIT + 1);
        assert_eq!(shown_command(&over), format!("{}... (and 1 more characters)", "b".repeat(COMMAND_LIMIT)), "one more character cuts it and says so");
        let tail = format!("{}{}{}", "ok ".repeat(10), " ".repeat(COMMAND_LIMIT), "evil".repeat(75));
        assert!(shown_command(&tail).ends_with(&format!("... (and {} more characters)", tail.chars().count() - COMMAND_LIMIT)),
            "a short prefix, blank padding and a long tail do not look like a short command: {}", shown_command(&tail));
        // A long path keeps its end, so the file name stays in view, and says how much came before it.
        let program = format!("C:\\{}\\tool.exe", "d\\".repeat(200));
        let shown_program = shown(&program, PROGRAM_LIMIT, Keep::End);
        let before = program.chars().count() - PROGRAM_LIMIT;
        assert!(shown_program.starts_with(&format!("({} characters before) ...", before)) && shown_program.ends_with("\\tool.exe"), "{}", shown_program);
        let text = text_of(&program, &program, "x", 86_400.0);
        assert!(text.contains("\\tool.exe] Account:") && text.contains("\\tool.exe] The processes"), "the program and the parent end with their file name: {}", text);
        // Characters, not bytes: a character outside basic Latin is written out, so it takes the room of its escape.
        let wide_text = "\u{4e2d}".repeat(COMMAND_LIMIT + 5);
        let kept = COMMAND_LIMIT / "\\u{4e2d}".len();
        assert_eq!(shown_command(&wide_text), format!("{}... (and {} more characters)", "\\u{4e2d}".repeat(kept), COMMAND_LIMIT + 5 - kept));
        // The escapes are the worst case: a field of the longest escapes (ten characters, `\u{10ffff}`) stays within its limit.
        for limit in [COMMAND_LIMIT, PROGRAM_LIMIT, ACCOUNT_LIMIT, PARENT_LIMIT] {
            for keep in [Keep::Start, Keep::End] {
                let field = shown(&format!("{}{}{}", "\u{10ffff}".repeat(39), "a".repeat(3), "\u{10ffff}".repeat(10)), limit, keep);
                let body = field.split("... (and").next().unwrap_or("").rsplit(") ...").next().unwrap_or("");
                assert!(body.len() <= limit, "a field is never longer than its limit once written out: {} {}", body.len(), limit);
            }
        }
    }

    #[test]
    fn the_whole_message_stays_under_the_limit_of_the_dialog_whatever_the_fields_hold() {
        // The largest the message can be: every field is as long as a field can be and is made of the longest escapes, the
        // account holds a backslash before u{ as well, the hours phrase is the longest, the deadline has a fraction and the most
        // digits, and the method is the one with the longest way to refuse.
        let longest = |escaped: &str| escaped.repeat(40_000);
        for escaped in ["\u{202e}", "\u{10ffff}", "\u{e0020}", "\\u{"] {
            let field = longest(escaped);
            let account = format!("{}\u{202e}", "\\u{".repeat(40_000));
            for method in [Method::Desktop, Method::Console] {
                let text = prompt_text(&Subject { account: &account, parent: &field, program: &field },
                    &Exception { method, command: field.clone(), timeout: 86_399.5 }, 21_601.5);
                assert!(text.encode_utf16().count() < CREDUI_MAX_MESSAGE_LENGTH, "the message stays under the limit of the dialog: {}", text.len());
            }
        }
        // The command line is the field the user most needs and it is first, so that a dialog that clips text keeps it.
        let hostile = "\u{202e}".repeat(10_000);
        let text = text_for(Method::Console, &hostile, &hostile, &hostile, &hostile, 21_601.0);
        let (kept, left) = (COMMAND_LIMIT / "\\u{202e}".len(), 10_000 - COMMAND_LIMIT / "\\u{202e}".len());
        assert!(text.starts_with(&format!("Process Tree Guard is asked to run: Command line: [{}... (and {} more characters)] Program: [", "\\u{202e}".repeat(kept), left)),
            "the command line is the first field: {}", text);
    }

    #[test]
    fn what_looks_like_a_blank_or_stacks_over_the_text_or_looks_like_a_letter_is_written_out() {
        for c in ['\u{a0}', '\u{3164}', '\u{115f}', '\u{1160}', '\u{ffa0}', '\u{2800}', '\u{34f}', '\u{301}', '\u{fe0f}', '\u{2003}', '\u{3000}', '\u{7f}'] {
            assert_eq!(shown_command(&format!("a{}b", c)), format!("a\\u{{{:x}}}b", c as u32), "U+{:04x}", c as u32);
        }
        assert_eq!(shown_command("a b\\c:d/e\"f'g&&h%PATH%i"), "a b\\c:d/e\"f'g&&h%PATH%i", "printable basic Latin is shown as it is, the ampersand and the percent sign included");
        // Letters and digits of other scripts are written out too: a Cyrillic es, o and er, a Greek alpha, a mathematical bold a.
        for c in ['\u{441}', '\u{43e}', '\u{440}', '\u{3b1}', '\u{1d41a}', '\u{e9}', '\u{4e2d}', '\u{661}', '\u{ff21}'] {
            assert_eq!(shown_command(&format!("a{}b", c)), format!("a\\u{{{:x}}}b", c as u32), "U+{:04x}: a letter that can pass for a Latin one", c as u32);
            assert!(!shown_as_it_is(c));
        }
        assert_eq!(shown_command("C:\\tools\\\u{440}ython.exe"), "C:\\tools\\\\u{440}ython.exe", "a program that is not python.exe cannot look like it");
        // The written-out form cannot be imitated: the text of six characters and the character itself are shown differently.
        let imitation = shown_command("a\\u{200b}b");
        let real = shown_command("a\u{200b}b");
        assert_eq!(imitation, "a\\u{5c}u{200b}b");
        assert_eq!(real, "a\\u{200b}b");
        assert_ne!(imitation, real, "an argument cannot pass for a character that is written out");
        // A backslash before anything else, and one at the end, is shown as it is, so paths are not doubled.
        assert_eq!(shown_command("C:\\u\\v\\u{x"), "C:\\u\\v\\u{5c}u{x");
        assert_eq!(shown_command("C:\\dir\\"), "C:\\dir\\");
        assert_eq!(shown_command("\\u"), "\\u");
        assert_eq!(shown_command("\\"), "\\");
    }

    #[test]
    fn hours_are_shown_rounded_up_to_two_decimals_and_never_less_than_the_cap() {
        for (seconds, phrase) in [(86_400.0, "24 hours"), (3_600.0, "1 hour"), (23_400.0, "6.5 hours"), (44_100.0, "12.25 hours"),
            (25_000.0, "6.95 hours (25000 seconds)"), (21_601.0, "6.01 hours (21601 seconds)"), (21_600.5, "6.01 hours (21600.5 seconds)"), (21_636.0, "6.01 hours")] {
            assert_eq!(hours_phrase(seconds), phrase, "{}", seconds);
        }
    }

    #[test]
    fn a_user_name_is_split_for_the_logon_by_its_form() {
        assert_eq!(split_name("DOMAIN\\alice", ""), (Some("DOMAIN".into()), "alice".into()));
        assert_eq!(split_name("PC\\a\\b", ""), (Some("PC".into()), "a\\b".into()), "the first backslash splits");
        assert_eq!(split_name("MicrosoftAccount\\a@b.example", ""), (Some("MicrosoftAccount".into()), "a@b.example".into()));
        assert_eq!(split_name("alice@corp.example", ""), (None, "alice@corp.example".into()), "a principal name takes no domain");
        assert_eq!(split_name("alice@corp.example", "IGNORED"), (None, "alice@corp.example".into()));
        assert_eq!(split_name("alice", ""), (Some(".".into()), "alice".into()), "a bare name is a local account");
        assert_eq!(split_name("alice", "CORP"), (Some("CORP".into()), "alice".into()), "the domain the prompt returned is used when the name has none");
        assert_eq!(split_name("DOMAIN\\alice", "OTHER"), (Some("DOMAIN".into()), "alice".into()), "the name decides over the domain field");
        // The name for the look up of the account.
        assert_eq!(full_name(Some("CORP"), "alice"), "CORP\\alice");
        assert_eq!(full_name(Some("."), "alice"), "alice", "the local database is looked in with the bare name");
        assert_eq!(full_name(None, "alice@corp.example"), "alice@corp.example");
    }

    #[test]
    fn nobody_is_present_without_a_session_and_a_visible_window_station() {
        assert_eq!(present(Some(1), Some(WSF_VISIBLE)), Ok(()));
        assert_eq!(present(Some(2), Some(WSF_VISIBLE | 0x4)), Ok(()), "other flags do not matter");
        let reasons = [
            present(Some(0), Some(WSF_VISIBLE)).unwrap_err(),
            present(None, Some(WSF_VISIBLE)).unwrap_err(),
            present(Some(1), None).unwrap_err(),
            present(Some(1), Some(0)).unwrap_err(),
            present(Some(1), Some(0x2)).unwrap_err(),
        ];
        assert!(reasons[0].contains("session 0") && reasons[1].contains("session of the process") && reasons[2].contains("window station of the process cannot be read")
            && reasons[3].contains("no interactive window station") && reasons[3] == reasons[4], "{:?}", reasons);
        for (i, a) in reasons[..4].iter().enumerate() {
            for b in &reasons[i + 1..4] {
                assert_ne!(a, b, "each case has a reason of its own");
            }
        }
        assert_eq!(present(Some(0), None).unwrap_err(), reasons[0], "session 0 is reported first");
    }

    #[test]
    fn detect_reads_this_process() {
        // It shows nothing: it reads the session of this process and the flags of its window station.
        let (session, flags) = detect();
        assert!(session.is_some() && flags.is_some(), "{:?} {:?}", session, flags);
        // Where this runs is not asserted: a shell over Secure Shell (SSH), a service, a scheduled task or a build agent has no session with a
        // desktop or a visible window station, and the guard is right to refuse there. What holds everywhere is that somebody
        // is present exactly when the session is not 0 and the window station is visible.
        assert_eq!(present(session, flags).is_ok(), session.map_or(false, |session| session != 0) && flags.map_or(false, |flags| flags & WSF_VISIBLE != 0), "{:?} {:?}", session, flags);
        // The libraries the prompts ask for are on every Windows with a desktop; loading them shows nothing.
        assert!(window_station_api().is_some());
    }

    #[link(name = "user32")]
    extern "system" {
        fn CreateWindowStationW(name: *const u16, flags: u32, access: u32, attributes: *mut c_void) -> Handle;
        fn SetProcessWindowStation(station: Handle) -> Bool;
    }

    /// Set in the environment of the copy of this test executable that `detect` is tried in.
    const DETECT_CHILD: &str = "PROCESS_TREE_GUARD_UNIT_TEST_HIDDEN_STATION_CHILD";

    #[test]
    fn detect_reports_a_window_station_that_is_not_visible_so_that_no_prompt_is_shown_on_one() {
        const GENERIC_ALL: u32 = 0x1000_0000;
        if std::env::var_os(DETECT_CHILD).is_some() {
            // The copy: it makes a window station of its own, which is not visible, makes it the station of the process, and says what
            // `detect` reads and `present` makes of it. Nothing is shown; no prompt is made.
            let station = unsafe { CreateWindowStationW(std::ptr::null(), 0, GENERIC_ALL, std::ptr::null_mut()) };
            if station.is_null() || unsafe { SetProcessWindowStation(station) } == 0 {
                println!("DETECTED no window station could be made [WinError {}]", unsafe { GetLastError() });
                return;
            }
            let (session, flags) = detect();
            println!("DETECTED flags={:?} {}", flags.map(|flags| flags & WSF_VISIBLE), match present(session, flags) {
                Ok(()) => "Ok".to_string(),
                Err(reason) => format!("Err {}", reason),
            });
            // The seat of the credential dialog, which the desktop method takes first, looks at the same thing before it takes the lock or
            // shows anything; it is what G95 and G103 see end to end, and those two run only where the variable that allows them is set.
            let owners = Owners::empty();
            println!("SEATED {}", match DesktopPrompt::new(&owners).seat("text") {
                Ok(_) => "accepted".to_string(),
                Err(reason) => format!("refused: {}", reason),
            });
            return;
        }
        // This copy cannot change its own window station without affecting the tests beside it, so a copy of the executable is run for
        // this test alone, with the marker in its environment (only this module reads it).
        let output = std::process::Command::new(std::env::current_exe().expect("the path of this test executable"))
            .args(["detect_reports_a_window_station_that_is_not_visible", "--nocapture", "--test-threads=1"])
            .env(DETECT_CHILD, "1").stdin(std::process::Stdio::null()).output().expect("run the copy");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        // The harness writes the name of the test before the line, so the report is found inside a line and not at its start.
        let said = stdout.lines().find_map(|line| line.find("DETECTED ").map(|at| line[at + "DETECTED ".len()..].to_string())).unwrap_or_default();
        if said.starts_with("no window station could be made") {
            // Some hosts refuse a process that is not a service a window station of its own; the manual check and G95 cover the path there.
            assert!(std::env::var_os("PROCESS_TREE_GUARD_REQUIRE_WINDOW_STATION").map_or(true, |value| value.is_empty()), "{}", said);
            eprintln!("NOT ASSERTED: {}", said);
            return;
        }
        assert!(output.status.success() && said.contains("flags=Some(0) Err ")
            && (said.contains("no interactive window station") || said.contains("session 0")), "detect must report a window station that is not visible: {} / {}", said, stdout);
        let seated = stdout.lines().find_map(|line| line.find("SEATED ").map(|at| line[at + "SEATED ".len()..].to_string())).unwrap_or_default();
        assert!(seated.starts_with("refused: ") && (seated.contains("no interactive window station") || seated.contains("session 0")),
            "the seat of the credential dialog must refuse on a window station that is not visible, before the lock is taken: {} / {}", seated, stdout);
    }

    /// Set in the environment of the copy of this test executable that the console prompt is begun in.
    const BEGIN_CHILD: &str = "PROCESS_TREE_GUARD_UNIT_TEST_CONSOLE_BEGIN_CHILD";

    #[test]
    fn the_console_prompt_is_refused_before_anything_is_written_when_standard_input_is_not_a_console() {
        if std::env::var_os(BEGIN_CHILD).is_some() {
            // The copy: its standard input is the null device. With it `begin` returns at the first check, before the lock is taken
            // or anything is written, so nothing can be shown.
            let owners = Owners::empty();
            match ConsolePrompt::new(&owners).begin("text") {
                Err(reason) => println!("BEGIN refused: {}", reason),
                Ok(_) => println!("BEGIN accepted"),
            }
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().expect("the path of this test executable"))
            .args(["the_console_prompt_is_refused_before_anything_is_written", "--nocapture", "--test-threads=1"])
            .env(BEGIN_CHILD, "1").stdin(std::process::Stdio::null()).output().expect("run the copy");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(output.status.success() && stdout.contains("BEGIN refused: standard input is not an interactive console"), "{}", stdout);
    }

    /// Set in the environment of the copy of this test executable that puts itself in a job; its value is `kill` or `plain`.
    const JOB_CHILD: &str = "PROCESS_TREE_GUARD_UNIT_TEST_JOB_CHILD";

    #[test]
    fn a_process_is_in_a_guard_job_when_the_job_it_is_in_kills_its_processes_when_it_closes() {
        if let Some(mode) = std::env::var_os(JOB_CHILD) {
            // The copy: it makes a job of its own, with the limit of a job of the guard or with none, and is put in it (nested in
            // whatever job it is in already). It says what `in_guard_job` reads.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
                let mut limits = ExtendedLimitInformation::default();
                if mode == "kill" {
                    limits.basic.limit_flags = KILL_ON_JOB_CLOSE;
                }
                let set = SetInformationJobObject(job, EXTENDED_LIMIT_CLASS, &mut limits as *mut _ as *mut c_void, std::mem::size_of::<ExtendedLimitInformation>() as u32);
                let assigned = !job.is_null() && set != 0 && AssignProcessToJobObject(job, GetCurrentProcess()) != 0;
                println!("JOBBED assigned={} in_guard_job={}", assigned, in_guard_job());
            }
            return;
        }
        // This copy cannot change its own job without affecting the tests beside it, so a copy of the executable is run for each case.
        for (mode, expected) in [("plain", false), ("kill", true)] {
            let output = std::process::Command::new(std::env::current_exe().expect("the path of this test executable"))
                .args(["a_process_is_in_a_guard_job_when_the_job_it_is_in", "--nocapture", "--test-threads=1"])
                .env(JOB_CHILD, mode).stdin(std::process::Stdio::null()).output().expect("run the copy");
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let said = stdout.lines().find_map(|line| line.find("JOBBED ").map(|at| line[at + "JOBBED ".len()..].to_string())).unwrap_or_default();
            if said.starts_with("assigned=false") {
                // A host whose job does not take a nested job: the integration tests (G100) cover the path there.
                eprintln!("NOT ASSERTED: the copy ({}) could not be put in a job of its own", mode);
                continue;
            }
            assert!(output.status.success() && said.trim() == format!("assigned=true in_guard_job={}", expected), "{}: {} / {}", mode, said, stdout);
        }
    }

    const GENERIC_READ: u32 = 0x8000_0000;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreatePipe(read: *mut Handle, write: *mut Handle, attributes: *mut c_void, size: u32) -> Bool;
    }

    #[test]
    fn only_a_console_input_buffer_is_taken_for_a_console_and_the_screen_buffer_of_a_console_is_not() {
        use std::os::windows::io::AsRawHandle;
        // The null device, a pipe and a file are what a program that starts the guard can give it for standard input.
        let null = std::fs::File::open("NUL").expect("open the null device");
        assert!(!is_console_input(null.as_raw_handle() as Handle), "the null device is not a console");
        let (mut read, mut write): (Handle, Handle) = (std::ptr::null_mut(), std::ptr::null_mut());
        assert_ne!(unsafe { CreatePipe(&mut read, &mut write, std::ptr::null_mut(), 0) }, 0, "make a pipe");
        assert!(!is_console_input(read) && !is_console_input(write), "neither end of a pipe is a console");
        unsafe {
            CloseHandle(read);
            CloseHandle(write);
        }
        let path = std::env::temp_dir().join(format!("ptg-console-input-{}", std::process::id()));
        let file = std::fs::File::create(&path).expect("make a file");
        assert!(!is_console_input(file.as_raw_handle() as Handle), "a file is not a console");
        drop(file);
        let _ = std::fs::remove_file(&path);
        // Where this run has a console: its screen buffer takes `GetConsoleMode` too and is still no input, which only the
        // second call tells; its input buffer is one. A run with no console (a build agent, a pipe-hosted shell) skips this part.
        if unsafe { GetConsoleWindow() }.is_null() {
            return;
        }
        let open = |name: &str| unsafe { CreateFileW(wide(name).as_ptr(), GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ_AND_WRITE, std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
        let (screen, input) = (open("CONOUT$"), open("CONIN$"));
        let mut mode = 0u32;
        if screen != INVALID_HANDLE {
            assert_ne!(unsafe { GetConsoleMode(screen, &mut mode) }, 0, "the screen buffer answers GetConsoleMode, which is why that call alone is not the test");
            assert!(!is_console_input(screen), "the screen buffer of a console is not a console input");
            unsafe { CloseHandle(screen) };
        }
        if input != INVALID_HANDLE {
            assert!(is_console_input(input), "the input buffer of a console is");
            unsafe { CloseHandle(input) };
        }
    }

    #[test]
    fn two_prompts_of_the_guard_are_not_open_at_once() {
        // A name of this test's own, so that a prompt of a real guard in the same session is neither refused by this test nor refuses it.
        let name = format!("Local\\ProcessTreeGuardTest{}", std::process::id());
        let first = PromptLock::take_named(&name).expect("the first prompt may open");
        let refusal = PromptLock::take_named(&name).err().expect("a second one is refused while the first is open");
        assert!(refusal.contains("is open or was refused less than 10 seconds ago, or another program holds its lock name"), "{}", refusal);
        assert_eq!(COOL_DOWN_MS, 10_000, "the figure the reason names");
        drop(first);
        assert!(PromptLock::take_named(&name).is_ok(), "the right returns when the first prompt is gone");
        assert_eq!(PROMPT_LOCK_NAME, "Local\\ProcessTreeGuardPrompt", "the name that the guard takes, which a test never takes");
    }

    #[test]
    fn a_wait_ends_at_its_limit_when_the_starter_goes_and_when_the_answer_comes() {
        let none = Owners::empty();
        let started = Instant::now();
        let slow = bounded(Duration::from_millis(50), &none, || {
            std::thread::sleep(Duration::from_millis(1_500));
            7
        });
        assert_eq!(slow, Err(Unanswered::TimedOut));
        assert!(started.elapsed() < Duration::from_millis(1_000), "the wait ends at its limit, not when the thread does");
        assert_eq!(bounded(Duration::from_secs(10), &none, || 5), Ok(5));
        // A starter that has exited ends the wait at once, long before the limit.
        let gone = Owners::of(vec![event(true)]);
        let started = Instant::now();
        let waiting = bounded(Duration::from_secs(60), &gone, || {
            std::thread::sleep(Duration::from_millis(3_000));
            7
        });
        assert_eq!(waiting, Err(Unanswered::OwnerGone));
        assert!(started.elapsed() < Duration::from_millis(2_000), "{:?}", started.elapsed());
        // One that is still there does not.
        let there = Owners::of(vec![event(false)]);
        assert_eq!(bounded(Duration::from_secs(10), &there, || 5), Ok(5));
        assert_eq!(bounded(Duration::from_millis(30), &there, || { std::thread::sleep(Duration::from_millis(1_000)); 5 }), Err(Unanswered::TimedOut));
        // The same for a prompt: the answer of a prompt that is waited for is Unavailable with the reason.
        assert_eq!(match ask_bounded_for(&gone) { Asked::Unavailable(reason) => reason, _ => String::new() }, OWNER_GONE);
    }

    /// The worker of a prompt over a prompt that never answers, for an owner that has gone.
    fn ask_bounded_for(owners: &Owners) -> Asked {
        PromptWorker::start().expect("the thread of the prompt").ask(owners, Duration::from_secs(60), || {
            std::thread::sleep(Duration::from_millis(3_000));
            Asked::Cancelled
        })
    }

    #[test]
    fn the_check_of_the_credentials_is_bounded_and_watches_the_starter() {
        let owners = Owners::empty();
        let slow = check_bounded(Duration::from_millis(50), &owners, Box::new(Credentials::with("me", "", "x")), |_| {
            std::thread::sleep(Duration::from_millis(1_500));
            Verified::Approved
        });
        assert_eq!(slow, Verified::Failed(WAIT_TIMEOUT), "a system that does not answer refuses the run and does not approve it");
        assert_eq!(check_bounded(Duration::from_secs(10), &owners, Box::new(Credentials::with("me", "", "x")), |_| Verified::Approved), Verified::Approved);
        let gone = Owners::of(vec![event(true)]);
        let waiting = check_bounded(Duration::from_secs(60), &gone, Box::new(Credentials::with("me", "", "x")), |_| {
            std::thread::sleep(Duration::from_millis(3_000));
            Verified::Approved
        });
        assert_eq!(waiting, Verified::OwnerGone);
    }

    #[test]
    fn the_buffers_are_zeroed_when_they_are_dropped_on_every_path() {
        let mut units = [0x5a5au16; 16];
        wipe(&mut units);
        assert!(units.iter().all(|unit| *unit == 0));
        let mut bytes = [0xa5u8; 33];
        wipe(&mut bytes);
        assert!(bytes.iter().all(|byte| *byte == 0));
        // A secret over storage that the test keeps: after the drop the storage is zero.
        let mut storage = [0x1234u16; 8];
        drop(Secret(&mut storage[..]));
        assert!(storage.iter().all(|unit| *unit == 0), "dropping a secret zeroes it");
        // The credentials: the typed password is there, and then the drop of the three buffers finds it and wipes it.
        let credentials = Credentials::with("me", "DOM", "hunter2");
        assert_eq!(credentials.password_units(), "hunter2".encode_utf16().collect::<Vec<u16>>());
        assert_eq!(credentials.names(), Some(("me".into(), "DOM".into())));
        WIPES.with(|wipes| wipes.borrow_mut().clear());
        drop(credentials);
        let dropped = WIPES.with(|wipes| wipes.borrow().clone());
        assert_eq!(dropped, [(2, 0), (3, 0), (7, 0)], "the user, the domain and the password were there, and are zero");
        // The buffer the prompt returns: dropping it zeroes it over its whole size before it is freed (here nothing is).
        unsafe extern "system" fn keep(_memory: *mut c_void) {}
        let mut raw = [0xffu8; 40];
        let buffer = OutBuffer { pointer: raw.as_mut_ptr() as *mut c_void, size: raw.len() as u32, free: keep };
        drop(buffer);
        assert!(raw.iter().all(|byte| *byte == 0), "the authentication buffer is zero after its drop");
    }

    #[test]
    fn the_credentials_are_wiped_on_the_path_of_every_verdict() {
        // The drop is the path of every verdict: approved, a wrong password, another account's, a failure and an empty password.
        for (name, verifier, password) in [
            ("approved", StubVerifier { right: "hunter2", same_user: true, failure: None }, "hunter2"),
            ("wrong", StubVerifier { right: "other", same_user: true, failure: None }, "hunter2"),
            ("another account", StubVerifier { right: "hunter2", same_user: false, failure: None }, "hunter2"),
            ("failed", StubVerifier { right: "hunter2", same_user: true, failure: Some(1909) }, "hunter2"),
        ] {
            WIPES.with(|wipes| wipes.borrow_mut().clear());
            let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
            let prompt = StubPrompt::new(vec![Answer::Password(password), Answer::Password(password), Answer::Password(password)]);
            let _ = authorize_with(&settings("x"), &prompt, &verifier, &log);
            log.finish(1_000);
            let dropped = WIPES.with(|wipes| wipes.borrow().clone());
            let asked = prompt.asked().len();
            assert_eq!(dropped.len(), 3 * asked, "{}: three buffers for each set of credentials that was asked for", name);
            assert!(dropped.iter().all(|(_, after)| *after == 0), "{}: {:?}", name, dropped);
            assert_eq!(dropped.iter().filter(|(before, _)| *before == 7).count(), asked, "{}: the password was in each set before its drop: {:?}", name, dropped);
        }
        // An empty password never reaches a verifier, and its buffers are wiped just the same.
        WIPES.with(|wipes| wipes.borrow_mut().clear());
        let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
        let prompt = StubPrompt::new(vec![Answer::Password(""), Answer::Password(""), Answer::Password("")]);
        let _ = authorize_with(&settings("x"), &prompt, &StubVerifier { right: "", same_user: true, failure: None }, &log);
        log.finish(1_000);
        let dropped = WIPES.with(|wipes| wipes.borrow().clone());
        assert_eq!(dropped.len(), 9);
        assert!(dropped.iter().all(|(_, after)| *after == 0));
    }

    #[test]
    fn only_the_security_identifier_of_the_guards_own_user_approves() {
        let sid = |text: &str| Ok::<String, GuardError>(text.to_string());
        let mine = "S-1-5-21-1111111111-2222222222-3333333333-1001";
        assert_eq!(same_user(sid(mine), sid(mine)), Verified::Approved);
        assert_eq!(same_user(sid("S-1-5-21-1111111111-2222222222-3333333333-1002"), sid(mine)), Verified::OtherUser, "another account of the same machine");
        assert_eq!(same_user(sid("S-1-5-21-4444444444-2222222222-3333333333-1001"), sid(mine)), Verified::OtherUser, "the same relative identifier on another machine");
        assert_eq!(same_user(sid(&mine.to_lowercase()), sid(mine)), Verified::OtherUser, "the text form is compared as it is");
        assert_eq!(same_user(sid("S-1-5-18"), sid(mine)), Verified::OtherUser);
        let unreadable = || Err::<String, GuardError>(GuardError("cannot read".into()));
        assert_eq!(same_user(unreadable(), sid(mine)), Verified::Failed(UNIDENTIFIED), "a token that cannot be read approves nothing");
        assert_eq!(same_user(sid(mine), unreadable()), Verified::Failed(UNIDENTIFIED));
        assert_eq!(same_user(unreadable(), unreadable()), Verified::Failed(UNIDENTIFIED));
        assert_eq!(same_user(sid(""), sid("")), Verified::OtherUser, "two empty identifiers are not the same user");
    }

    #[test]
    fn what_the_system_says_to_a_logon_and_to_a_look_up_is_sorted_into_wrong_and_failed() {
        // The mapping alone: the system is not called, so no failed logon is written to the audit log of the machine.
        assert_eq!(classify_logon_error(ERROR_LOGON_FAILURE), Verified::Wrong);
        for code in [1909, 1385, 1327, ERROR_INSUFFICIENT_BUFFER, 1328, 1331] {
            assert_eq!(classify_logon_error(code), Verified::Failed(code), "{}", code);
        }
        assert_eq!(classify_lookup_error(ERROR_NONE_MAPPED), Verified::Wrong, "no such account is a name that is not accepted");
        assert_eq!(classify_lookup_error(1722), Verified::Failed(1722), "a domain that cannot be reached is a failure of the system");
    }

    #[test]
    fn a_name_that_is_not_the_guards_own_account_is_turned_away_before_any_logon() {
        let own = own_account_name().expect("the name of the account this test runs as");
        assert!(!own.is_empty(), "{}", own);
        assert!(account_check(&own).is_none(), "the account itself goes on to the logon: {}", own);
        assert!(account_check(&own.to_uppercase()).is_none(), "in any letter case: {}", own);
        // A well known account of the local system, which is looked up with no network and is the account of no user: it is another account.
        // Its name is asked of the system by its security identifier, since it is spelled differently on a localized Windows.
        let system_name = name_of_well_known_account("S-1-5-18").expect("the name of the local system account");
        let ours = current_user_sid().expect("the user of this process");
        let system = account_check(&system_name);
        if ours == "S-1-5-18" {
            assert!(system.is_none(), "{}", system_name);
        } else {
            assert_eq!(system, Some(Verified::OtherUser), "another account is turned away without a logon: {}", system_name);
        }
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn ConvertStringSidToSidW(text: *const u16, sid: *mut *mut c_void) -> Bool;
        fn LookupAccountSidW(system: *const u16, sid: *mut c_void, name: *mut u16, name_chars: *mut u32, domain: *mut u16, domain_chars: *mut u32, kind: *mut u32) -> Bool;
        fn CredProtectW(as_self: Bool, credentials: *const u16, chars: u32, protected: *mut u16, protected_chars: *mut u32, kind: *mut i32) -> Bool;
    }

    /// The name, `DOMAIN\name`, that the system gives the account with the security identifier `text` (looked up locally).
    fn name_of_well_known_account(text: &str) -> Option<String> {
        unsafe {
            let mut sid: *mut c_void = std::ptr::null_mut();
            if ConvertStringSidToSidW(wide(text).as_ptr(), &mut sid) == 0 {
                return None;
            }
            let (mut name, mut domain) = (vec![0u16; 256], vec![0u16; 256]);
            let (mut name_chars, mut domain_chars, mut kind) = (name.len() as u32, domain.len() as u32, 0u32);
            let found = LookupAccountSidW(std::ptr::null(), sid, name.as_mut_ptr(), &mut name_chars, domain.as_mut_ptr(), &mut domain_chars, &mut kind) != 0;
            LocalFree(sid);
            found.then(|| format!("{}\\{}", String::from_utf16_lossy(&domain[..domain_chars as usize]), String::from_utf16_lossy(&name[..name_chars as usize])))
        }
    }

    #[test]
    fn a_password_in_a_protected_form_is_told_from_one_in_clear_text() {
        // Loading the libraries shows nothing; the function is the one the desktop prompt asks.
        let api = desktop_api().expect("the libraries of the desktop prompt");
        let mut plain = Credentials::with("me", "", "hunter2");
        assert!(!password_is_protected(api, &mut plain.password.0), "a password in clear text is not protected");
        // A password protected for the current user, as the system can hand one back.
        let clear: Vec<u16> = "hunter2".encode_utf16().chain(std::iter::once(0)).collect();
        let (mut size, mut kind) = (0u32, 0i32);
        unsafe { CredProtectW(0, clear.as_ptr(), clear.len() as u32, std::ptr::null_mut(), &mut size, &mut kind) };
        assert!(size > 0, "the size of the protected form is asked for first");
        let mut blob = vec![0u16; size as usize];
        assert_ne!(unsafe { CredProtectW(0, clear.as_ptr(), clear.len() as u32, blob.as_mut_ptr(), &mut size, &mut kind) }, 0, "protect a password for the test");
        assert!(password_is_protected(api, &mut blob), "a protected password is not sent to the logon");
        wipe(&mut blob);
    }

    #[test]
    fn credentials_have_the_sizes_the_prompt_fills_and_a_password_that_ends() {
        let credentials = Credentials::new();
        assert_eq!((credentials.user.0.len(), credentials.domain.0.len(), credentials.password.0.len()), (514, 338, 257));
        assert!(credentials.password_ends(), "a zeroed buffer ends");
        let mut full = Credentials::new();
        for unit in full.password.0.iter_mut() {
            *unit = 0x41;
        }
        assert!(!full.password_ends(), "a password with no end is not passed to the system");
        let owners = Owners::empty();
        assert_eq!(LogonVerifier { owners: &owners }.verify(Box::new(full)), Verified::Failed(ERROR_INSUFFICIENT_BUFFER));
    }

    /// A verifier that must not be called: for answers that never reach a logon.
    struct NoLogon;

    impl Verifier for NoLogon {
        fn verify(&self, _credentials: Box<Credentials>) -> Verified {
            panic!("a logon was tried");
        }
    }

    /// A prompt that gives `Rejected` each time it is asked, and keeps how often it was asked.
    struct RejectsEvery(Mutex<Vec<(u32, u32)>>);

    impl Prompt for RejectsEvery {
        fn begin(&self, _text: &str) -> Result<Claim, String> {
            Ok(Claim::none())
        }
        fn ask(&self, attempt: u32, last_error: u32, _text: &str) -> Asked {
            self.0.lock().unwrap().push((attempt, last_error));
            Asked::Rejected("the answer of the prompt could not be read as a user name and password: smart card".into())
        }
    }

    #[test]
    fn an_answer_that_is_not_a_name_and_a_password_is_an_attempt_used_with_no_logon_and_ends_after_three() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = RejectsEvery(Mutex::new(Vec::new()));
        let error = authorize_with(&settings("x"), &prompt, &NoLogon, &log).expect_err("three answers that cannot be read end the run");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(*prompt.0.lock().unwrap(), [(1, 0), (2, ERROR_LOGON_FAILURE), (3, ERROR_LOGON_FAILURE)], "the prompt is asked three times and told that the logon failed");
        assert_eq!(decisions(&lines), ["refused", "refused", "refused"], "{:?}", lines);
        assert!(lines.iter().all(|line| line.contains("smart card")) && lines[2].contains("attempt=3 ") && lines[2].contains("no attempts are left"), "{:?}", lines);
        assert!(error.message().contains("no attempts are left"), "{}", error);
        // A good answer after one of them still approves.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        struct RejectsThenAnswers(Mutex<u32>);
        impl Prompt for RejectsThenAnswers {
            fn begin(&self, _text: &str) -> Result<Claim, String> {
                Ok(Claim::none())
            }
            fn ask(&self, _attempt: u32, _last_error: u32, _text: &str) -> Asked {
                let mut asked = self.0.lock().unwrap();
                *asked += 1;
                if *asked == 1 { Asked::Rejected("not a password".into()) } else { Asked::Entered(Box::new(Credentials::with("me", "", "right"))) }
            }
        }
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        assert!(authorize_with(&settings("x"), &RejectsThenAnswers(Mutex::new(0)), &verifier, &log).is_ok());
        log.finish(1_000);
        assert_eq!(decisions(&lines_of(&kept)), ["refused", "approved"], "{:?}", lines_of(&kept));
    }

    #[test]
    fn a_wrong_answer_is_followed_by_a_wait_that_the_starter_ending_cuts_short() {
        assert_eq!(RETRY_DELAY_MS, 2_000, "the wait that makes a loop of guesses cost time");
        let verifier = StubVerifier { right: "right", same_user: true, failure: None };
        RETRY_DELAY.with(|delay| delay.set(400));
        let waits = || RETRIES.with(|retries| retries.replace(0));
        waits();
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let started = Instant::now();
        assert!(authorize_with(&settings("x"), &StubPrompt::new(vec![Answer::Password("wrong"), Answer::Password("right")]), &verifier, &log).is_ok());
        log.finish(1_000);
        assert!(started.elapsed() >= Duration::from_millis(400) && waits() == 1, "the second prompt comes after the wait: {:?}", started.elapsed());
        // Nothing is waited for after an approval, or after the last attempt: three attempts are two waits.
        let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
        assert!(authorize_with(&settings("x"), &StubPrompt::new(vec![Answer::Password("right")]), &verifier, &log).is_ok());
        log.finish(1_000);
        assert_eq!(waits(), 0);
        let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
        assert!(authorize_with(&settings("x"), &StubPrompt::new(vec![Answer::Password("a"), Answer::Password("b"), Answer::Password("c")]), &verifier, &log).is_err());
        log.finish(1_000);
        assert_eq!(waits(), 2);
        // A starter that exits during the wait ends it at once, and the run is refused.
        RETRY_DELAY.with(|delay| delay.set(30_000));
        let starter = event(false);
        struct ExitsDuringTheWait(Handle, Mutex<u32>);
        impl Prompt for ExitsDuringTheWait {
            fn begin(&self, _text: &str) -> Result<Claim, String> {
                Ok(Claim::none())
            }
            fn ask(&self, _attempt: u32, _last_error: u32, _text: &str) -> Asked {
                *self.1.lock().unwrap() += 1;
                let handle = self.0 as usize;
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(300));
                    unsafe { SetEvent(handle as Handle) };
                });
                Asked::Entered(Box::new(Credentials::with("me", "", "wrong")))
            }
        }
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let prompt = ExitsDuringTheWait(starter, Mutex::new(0));
        let started = Instant::now();
        let error = authorize_owned(&settings("x"), &prompt, &verifier, &Owners::of(vec![starter]), &log).expect_err("the starter went");
        log.finish(1_000);
        assert!(started.elapsed() < Duration::from_secs(3) && *prompt.1.lock().unwrap() == 1 && error.message().contains("nobody is waiting for it"), "{:?} {}", started.elapsed(), error);
        assert!(!pause(&Owners::of(vec![event(true)]), 30_000) && pause(&Owners::empty(), 20));
        RETRY_DELAY.with(|delay| delay.set(0));
    }

    #[test]
    fn the_starter_is_taken_only_if_it_started_before_the_guard_and_its_handle_is_the_one_that_is_kept() {
        assert!(started_before(Some(5), Some(5)) && started_before(Some(4), Some(5)));
        assert!(!started_before(Some(6), Some(5)) && !started_before(None, Some(5)) && !started_before(Some(5), None) && !started_before(None, None));
        // The parent of this process started before it, and its handle watches it: alive, so not signalled.
        let parent = crate::engine::parent_process_id().expect("the parent of the test run");
        let owners = Owners::open(&[parent]).expect("the parent of this process is a starter");
        assert!(!owners.gone() && !owners.image().is_empty(), "the image is read from the handle: {:?}", owners.image());
        // The handles are given on and not closed with the value: the one used for the wait is the one opened for the prompt.
        let handles = owners.into_handles();
        assert_eq!(handles.len(), 1);
        assert_eq!(unsafe { WaitForSingleObject(handles[0], 0) }, WAIT_TIMEOUT, "the handle is still open and the parent is alive");
        assert!(started_at(handles[0]).is_some());
        assert_eq!(Owners::of(handles.clone()).into_handles(), handles, "the same handles come out as went in");
        unsafe { CloseHandle(handles[0]) };
        // A process that started after this one cannot be its parent: a reused identifier is refused.
        let mut later = std::process::Command::new("cmd").args(["/d", "/c", "ping -n 6 127.0.0.1 >nul"]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).spawn().expect("start a later process");
        let refusal = Owners::open(&[later.id()]).err().expect("a process that started later is not a starter");
        assert!(refusal.message().contains("its identifier was reused"), "{}", refusal);
        let _ = later.kill();
        let _ = later.wait();
        // A process that cannot be opened is refused too.
        assert!(Owners::open(&[0xffff_fff0]).is_err());
    }

    #[test]
    fn a_run_whose_starter_cannot_be_watched_is_refused_with_a_line_before_any_attempt() {
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let error = refuse_unwatched(&settings("x"), &log, "[WinError 5] Access is denied.");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(decisions(&lines), ["refused"], "{:?}", lines);
        assert!(lines[0].contains("attempt=0 ") && lines[0].contains("decision=exception-refused") && lines[0].contains("cannot be watched: [WinError 5] Access is denied."), "{}", lines[0]);
        assert!(error.message().contains("exception refused") && error.message().contains("the run did not start"), "{}", error);
    }

    #[test]
    fn a_write_to_the_console_is_bounded_and_watches_the_starter() {
        let none = Owners::empty();
        assert_eq!(write_bounded(&none, Duration::from_secs(5), || true), Ok(()));
        assert!(write_bounded(&none, Duration::from_secs(5), || false).unwrap_err().contains("cannot be written to"));
        let started = Instant::now();
        let blocked = write_bounded(&none, Duration::from_millis(60), || {
            std::thread::sleep(Duration::from_millis(1_500));
            true
        });
        assert!(blocked.unwrap_err().contains("within the time allowed") && started.elapsed() < Duration::from_millis(1_000), "{:?}", started.elapsed());
        let gone = Owners::of(vec![event(true)]);
        let started = Instant::now();
        let blocked = write_bounded(&gone, Duration::from_secs(60), || {
            std::thread::sleep(Duration::from_millis(3_000));
            true
        });
        assert_eq!(blocked, Err(OWNER_GONE.to_string()));
        assert!(started.elapsed() < Duration::from_millis(2_000), "{:?}", started.elapsed());
    }

    #[test]
    fn the_name_of_the_account_is_read_inside_the_limit_of_the_prompt_and_every_call_is_made_on_one_thread() {
        // A look up that does not come back is waited for by the same bounded wait as the prompt, with the owner watched.
        let worker = PromptWorker::start().expect("the thread of the prompt");
        let gone = Owners::of(vec![event(true)]);
        let started = Instant::now();
        let asked = worker.ask(&gone, Duration::from_secs(60), || {
            std::thread::sleep(Duration::from_millis(3_000));
            Asked::Cancelled
        });
        assert!(matches!(asked, Asked::Unavailable(ref reason) if reason == OWNER_GONE) && started.elapsed() < Duration::from_millis(2_000), "{:?}", started.elapsed());
        // Every call of a run is made on one thread, which is not the caller's.
        let worker = PromptWorker::start().expect("the thread of the prompt");
        let none = Owners::empty();
        let first = std::thread::current().id();
        let seen = Arc::new(Mutex::new(Vec::new()));
        for _ in 0..3 {
            let seen = seen.clone();
            let asked = worker.ask(&none, Duration::from_secs(10), move || {
                seen.lock().unwrap().push(std::thread::current().id());
                Asked::Cancelled
            });
            assert!(matches!(asked, Asked::Cancelled));
        }
        let seen = seen.lock().unwrap().clone();
        assert!(seen.len() == 3 && seen.iter().all(|id| *id == seen[0]) && seen[0] != first, "one thread of its own for all the calls of a run: {:?}", seen);
        // The wait for a prompt that is not answered ends at its limit.
        let slow = worker.ask(&none, Duration::from_millis(60), || {
            std::thread::sleep(Duration::from_millis(1_500));
            Asked::Cancelled
        });
        assert!(matches!(slow, Asked::Abandoned(ref reason) if reason.starts_with("no answer within")), "a prompt left open is abandoned, not failed");
        // The real name is DOMAIN\user, read without a look up of the identifier.
        let own = own_account_name().expect("the account this test runs as");
        assert!(own.contains('\\') && !own.contains('\0'), "{}", own);
    }

    // ------------------------------------------------------------------ Windows Hello
    //
    // Nothing below shows a prompt, a dialog or a window to the person who runs the tests: the request for consent is never
    // made (a scripted `Consenter` stands in), the owner window is made only on a window station that is not visible, and the
    // one call of the system that is made for real, the question whether Windows Hello is available, shows nothing.

    /// A consent that does what a test scripts, and keeps what it was asked.
    struct FakeConsent {
        availability: Result<Availability, String>,
        session: Result<(), String>,
        outcome: Outcome,
        /// The calls made, in order.
        calls: Mutex<Vec<&'static str>>,
        /// The message of each request.
        messages: Mutex<Vec<String>>,
        /// A name of a lock that the request tries to take, and whether it found the lock taken.
        lock: Option<String>,
        lock_was_taken: Mutex<Option<bool>>,
    }

    impl FakeConsent {
        fn new(availability: Result<Availability, String>, outcome: Outcome) -> FakeConsent {
            FakeConsent { availability, session: Ok(()), outcome, calls: Mutex::new(Vec::new()), messages: Mutex::new(Vec::new()), lock: None, lock_was_taken: Mutex::new(None) }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Consenter for FakeConsent {
        fn availability(&self, _owners: &Owners) -> Result<Availability, String> {
            self.calls.lock().unwrap().push("availability");
            self.availability.clone()
        }

        fn session_user(&self, _owners: &Owners) -> Result<(), String> {
            self.calls.lock().unwrap().push("session_user");
            self.session.clone()
        }

        fn request(&self, message: &str, _owners: &Owners) -> Outcome {
            self.calls.lock().unwrap().push("request");
            self.messages.lock().unwrap().push(message.to_string());
            if let Some(name) = &self.lock {
                *self.lock_was_taken.lock().unwrap() = Some(PromptLock::take_named(name).is_err());
            }
            self.outcome
        }
    }

    /// Settings for a run by the desktop method.
    fn desktop(command: &str) -> Settings {
        let mut desktop = settings(command);
        if let Some(exception) = desktop.exception.as_mut() {
            exception.method = Method::Desktop;
        }
        desktop
    }

    fn run_desktop(settings: &Settings, consent: &dyn Consenter, prompt: &dyn Prompt, verifier: &dyn Verifier, owners: &Owners, log: &Log) -> Result<(), GuardError> {
        authorize_desktop(settings, &Subject { account: "PC\\me", parent: STARTER, program: "prog.exe" }, consent, prompt, verifier, owners, log)
    }

    /// A verifier for the runs where no password is checked.
    fn any_verifier() -> StubVerifier {
        StubVerifier { right: "right", same_user: true, failure: None }
    }

    #[test]
    fn what_windows_hello_says_is_named_as_the_record_names_it() {
        let availability = ["Available", "DeviceNotPresent", "NotConfiguredForUser", "DisabledByPolicy", "DeviceBusy"];
        for (code, name) in availability.iter().enumerate() {
            assert_eq!(Availability::from_code(code as i32).name(), *name, "{}", code);
        }
        assert_eq!(Availability::from_code(0), Availability::Available);
        for code in [5, 6, 7, -1, 99] {
            assert_eq!(Availability::from_code(code), Availability::Unknown(code));
            assert_eq!(Availability::from_code(code).name(), format!("Unknown({})", code), "a code that is not known is named by its number");
        }
        let consent = ["Verified", "DeviceNotPresent", "NotConfiguredForUser", "DisabledByPolicy", "DeviceBusy", "RetriesExhausted", "Canceled"];
        for (code, name) in consent.iter().enumerate() {
            assert_eq!(Consent::from_code(code as i32).name(), *name, "{}", code);
        }
        assert_eq!(Consent::from_code(0), Consent::Verified);
        assert_eq!(Consent::from_code(5), Consent::RetriesExhausted);
        assert_eq!(Consent::from_code(6), Consent::Canceled);
        for code in [7, 8, -1] {
            assert_eq!(Consent::from_code(code).name(), format!("Unknown({})", code));
        }
        // The value a result is set to before the system fills it is neither a verification nor an availability.
        assert_ne!(Consent::from_code(NO_RESULT), Consent::Verified);
        assert_ne!(Availability::from_code(NO_RESULT), Availability::Available);
    }

    #[test]
    fn only_a_verified_user_approves_and_every_other_answer_refuses_with_its_name_and_the_password_is_not_asked_for() {
        // Also the value a result is set to before the system fills it, and the largest and smallest values: none of them is `Verified`.
        for code in (0..=7).chain([NO_RESULT, i32::MAX, i32::MIN]) {
            let kept = Arc::new(Mutex::new(Vec::new()));
            let log = Log::capture(kept.clone(), None);
            let consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(code));
            let prompt = StubPrompt::new(vec![]);
            COOL_DOWNS.with(|count| count.set(0));
            let result = run_desktop(&desktop("C:\\tools\\run.exe --all"), &consent, &prompt, &any_verifier(), &Owners::empty(), &log);
            log.finish(1_000);
            let lines = lines_of(&kept);
            assert_eq!(consent.calls(), ["availability", "session_user", "request"], "{}: in this order, and one request", code);
            assert!(prompt.asked().is_empty(), "{}: the password is never asked for after Windows Hello was", code);
            assert!(lines.len() == 1 && lines[0].contains("method=\"hello\"") && lines[0].contains("attempt=1 "), "{}: {:?}", code, lines);
            if code == 0 {
                assert!(result.is_ok(), "{}: {:?}", code, result.err());
                assert_eq!(decisions(&lines), ["approved"]);
                assert!(lines[0].contains("reason=\"a cap of 24 hours was approved: Windows Hello verified the signed-in user\"") && !lines[0].contains("not available"), "{}", lines[0]);
                assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 0, "no cool-down after an approval");
            } else {
                let error = result.expect_err("any answer but Verified refuses the run");
                let name = Consent::from_code(code).name();
                assert_eq!(decisions(&lines), ["refused"]);
                assert!(lines[0].contains(&format!("reason=\"Windows Hello did not verify the user: {}\"", name)), "{}: {}", code, lines[0]);
                assert!(error.message().contains(&format!("Windows Hello did not verify the user: {}", name)) && error.message().contains("the run did not start"), "{}", error);
                assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 1, "{}: the right to a prompt is kept for a while after an answer was asked for", code);
            }
        }
    }

    #[test]
    fn a_request_that_failed_or_had_to_be_cancelled_refuses_and_names_what_happened() {
        let cases: [(Outcome, &str, u32); 5] = [
            (Outcome::Failed(0x8007_0005), "reason=\"Windows Hello failed: 0x80070005\"", 1),
            (Outcome::Cancelled, "reason=\"Windows Hello did not verify the user: the request was cancelled\"", 1),
            (Outcome::TimedOut, "reason=\"no answer within 300 seconds, the request was cancelled\"", 0),
            (Outcome::OwnerGone, "nobody is waiting for it", 0),
            (Outcome::CancelIgnored, "reason=\"the request for Windows Hello was cancelled and did not end\"", 0),
        ];
        for (outcome, said, cool_downs) in cases {
            let kept = Arc::new(Mutex::new(Vec::new()));
            let log = Log::capture(kept.clone(), None);
            let consent = FakeConsent::new(Ok(Availability::Available), outcome);
            COOL_DOWNS.with(|count| count.set(0));
            let error = run_desktop(&desktop("x"), &consent, &StubPrompt::new(vec![]), &any_verifier(), &Owners::empty(), &log).expect_err("the run is refused");
            log.finish(1_000);
            let lines = lines_of(&kept);
            assert!(lines.len() == 1 && lines[0].contains(said) && lines[0].contains("method=\"hello\"") && lines[0].contains("attempt=1 "), "{:?}: {:?}", outcome, lines);
            assert!(error.message().contains("exception refused") && error.message().contains("the run did not start"), "{}", error);
            assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), cool_downs,
                "{:?}: a request that was cancelled ends the process at once, any other refusal keeps the right to a prompt", outcome);
        }
    }

    #[test]
    fn where_windows_hello_is_not_available_the_dialog_asks_for_the_password_and_the_record_says_why() {
        let cases: Vec<(Result<Availability, String>, &str)> = vec![
            (Ok(Availability::DeviceNotPresent), "DeviceNotPresent"),
            (Ok(Availability::NotConfiguredForUser), "NotConfiguredForUser"),
            (Ok(Availability::DisabledByPolicy), "DisabledByPolicy"),
            (Ok(Availability::DeviceBusy), "DeviceBusy"),
            (Ok(Availability::Unknown(9)), "Unknown(9)"),
            (Err("no answer within 5 seconds".into()), "no answer within 5 seconds"),
            (Err(HELLO_MISSING.into()), HELLO_MISSING),
        ];
        for (availability, said) in cases {
            let suffix = format!(" (Windows Hello was not available: {})", said);
            let kept = Arc::new(Mutex::new(Vec::new()));
            let log = Log::capture(kept.clone(), None);
            let consent = FakeConsent::new(availability, Outcome::Done(0));
            let prompt = StubPrompt::new(vec![Answer::Password("wrong"), Answer::Password("right")]);
            assert!(run_desktop(&desktop("x"), &consent, &prompt, &any_verifier(), &Owners::empty(), &log).is_ok(), "{}", said);
            log.finish(1_000);
            let lines = lines_of(&kept);
            assert_eq!(consent.calls(), ["availability"], "{}: neither the user nor a request is looked at when Windows Hello is not used", said);
            assert_eq!(prompt.asked().len(), 2, "{}", said);
            assert_eq!(decisions(&lines), ["refused", "approved"], "{:?}", lines);
            assert!(lines.iter().all(|line| line.contains("method=\"desktop\"") && line.contains(&format!("{}\"", suffix))), "every reason says why: {:?}", lines);
            assert!(lines[1].contains("the password of the account the guard runs as was given at the desktop prompt"), "{}", lines[1]);
        }
        // A refusal in the dialog says it too, and so does the last attempt.
        for (answers, said) in [(vec![Answer::Cancel], "the prompt was cancelled"), (vec![Answer::Password("a"), Answer::Password("b"), Answer::Password("c")], "no attempts are left")] {
            let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
            let consent = FakeConsent::new(Ok(Availability::NotConfiguredForUser), Outcome::Done(0));
            let error = run_desktop(&desktop("x"), &consent, &StubPrompt::new(answers), &any_verifier(), &Owners::empty(), &log).expect_err("refused");
            log.finish(1_000);
            assert!(error.message().contains(said) && error.message().ends_with(" (Windows Hello was not available: NotConfiguredForUser); the run did not start"), "{}", error);
        }
        // A dialog that cannot be shown is refused before an attempt, with the reason Windows Hello was not used.
        struct NotReady;
        impl Prompt for NotReady {
            fn begin(&self, _text: &str) -> Result<Claim, String> {
                Ok(Claim::none())
            }
            fn ready(&self, _text: &str) -> Result<(), String> {
                Err(NOT_AVAILABLE.into())
            }
            fn ask(&self, _attempt: u32, _last_error: u32, _text: &str) -> Asked {
                panic!("the dialog was asked");
            }
        }
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let consent = FakeConsent::new(Ok(Availability::DisabledByPolicy), Outcome::Done(0));
        assert!(run_desktop(&desktop("x"), &consent, &NotReady, &any_verifier(), &Owners::empty(), &log).is_err());
        log.finish(1_000);
        assert!(lines_of(&kept)[0].contains("attempt=0 ") && lines_of(&kept)[0].contains(&format!("{} (Windows Hello was not available: DisabledByPolicy)", NOT_AVAILABLE)), "{:?}", lines_of(&kept));
    }

    #[test]
    fn a_consent_that_was_asked_for_and_not_given_never_falls_back_to_the_password() {
        for code in 1..=7 {
            let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
            let consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(code));
            let prompt = StubPrompt::new(vec![Answer::Password("right")]);
            assert!(run_desktop(&desktop("x"), &consent, &prompt, &any_verifier(), &Owners::empty(), &log).is_err(), "{}", code);
            log.finish(1_000);
            assert!(prompt.asked().is_empty(), "{}: a cancel or a wrong PIN is not answered by another door", code);
        }
    }

    /// A consent that verifies, and makes the starter exit at the call named by `at`.
    struct StarterLeaves {
        starter: usize,
        at: &'static str,
        calls: Mutex<Vec<&'static str>>,
    }

    impl StarterLeaves {
        fn call(&self, name: &'static str) {
            self.calls.lock().unwrap().push(name);
            if name == self.at {
                unsafe { SetEvent(self.starter as Handle) };
            }
        }
    }

    impl Consenter for StarterLeaves {
        fn availability(&self, _owners: &Owners) -> Result<Availability, String> {
            self.call("availability");
            Ok(Availability::Available)
        }

        fn session_user(&self, _owners: &Owners) -> Result<(), String> {
            self.call("session_user");
            Ok(())
        }

        fn request(&self, _message: &str, _owners: &Owners) -> Outcome {
            self.call("request");
            Outcome::Done(0)
        }
    }

    #[test]
    fn a_starter_that_leaves_before_the_approval_is_recorded_gets_no_request_and_no_approval_and_a_verification_that_no_record_holds_does_not_stand() {
        // The starter exits during the question, during the look at the user, and while the request is answered (a verification that
        // arrives with the starter gone is no approval). No request is made for a run that nobody started, no approval is written, and
        // the process ends at once.
        for (at, calls, attempt, method) in [
            ("availability", vec!["availability"], "attempt=0 ", "desktop"),
            ("session_user", vec!["availability", "session_user"], "attempt=0 ", "hello"),
            ("request", vec!["availability", "session_user", "request"], "attempt=1 ", "hello"),
        ] {
            let starter = event(false);
            let consent = StarterLeaves { starter: starter as usize, at, calls: Mutex::new(Vec::new()) };
            let kept = Arc::new(Mutex::new(Vec::new()));
            let log = Log::capture(kept.clone(), None);
            COOL_DOWNS.with(|count| count.set(0));
            let error = run_desktop(&desktop("x"), &consent, &StubPrompt::new(vec![]), &any_verifier(), &Owners::of(vec![starter]), &log).expect_err("nobody is waiting");
            log.finish(1_000);
            let lines = lines_of(&kept);
            assert_eq!(*consent.calls.lock().unwrap(), calls, "{}: the starter leaves at this call, and nothing is asked after the check that follows", at);
            assert!(error.message().contains("nobody is waiting for it"), "{}: {}", at, error);
            assert!(decisions(&lines) == ["refused"] && lines[0].contains(attempt) && lines[0].contains(&format!("method=\"{}\"", method)), "{}: {:?}", at, lines);
            assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 0, "{}: the process ends at once", at);
        }
        // A verification whose line no record can hold is no approval: standard error that cannot take it refuses the run, and keeps
        // the right to a prompt for a while, as any refusal after a request does.
        let fail = Arc::new(AtomicBool::new(true));
        let log = Log::capture(Arc::new(Mutex::new(Vec::new())), Some(fail.clone()));
        COOL_DOWNS.with(|count| count.set(0));
        let error = run_desktop(&desktop("x"), &FakeConsent::new(Ok(Availability::Available), Outcome::Done(0)), &StubPrompt::new(vec![]), &any_verifier(), &Owners::empty(), &log)
            .expect_err("an approval that no record holds is not one");
        log.finish(1_000);
        assert!(error.message().contains("could not be recorded"), "{}", error);
        assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 1);
        // The same consent with a standard error that works is recorded, and the run goes on.
        fail.store(false, Ordering::SeqCst);
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), Some(fail));
        assert!(run_desktop(&desktop("x"), &FakeConsent::new(Ok(Availability::Available), Outcome::Done(0)), &StubPrompt::new(vec![]), &any_verifier(), &Owners::empty(), &log).is_ok());
        log.finish(1_000);
        assert_eq!(decisions(&lines_of(&kept)), ["approved"]);
    }

    #[test]
    fn nothing_is_asked_of_windows_hello_before_somebody_is_present_and_nothing_is_requested_for_another_user() {
        // Somebody must be present first: the seat is refused and Windows Hello is not asked, not even whether it is available.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let mut closed = StubPrompt::new(vec![]);
        closed.closed = Some("the process has no interactive window station to show the prompt on");
        let consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(0));
        assert!(run_desktop(&desktop("x"), &consent, &closed, &any_verifier(), &Owners::empty(), &log).is_err());
        log.finish(1_000);
        assert!(consent.calls().is_empty(), "availability is never called before presence: {:?}", consent.calls());
        assert!(lines_of(&kept)[0].contains("attempt=0 ") && lines_of(&kept)[0].contains("method=\"desktop\"") && lines_of(&kept)[0].contains("no interactive window station"), "{:?}", lines_of(&kept));
        // A starter that has gone comes before everything.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(0));
        let error = run_desktop(&desktop("x"), &consent, &StubPrompt::new(vec![]), &any_verifier(), &Owners::of(vec![event(true)]), &log).expect_err("nobody is waiting");
        log.finish(1_000);
        assert!(consent.calls().is_empty() && error.message().contains("nobody is waiting for it") && lines_of(&kept)[0].contains("attempt=0 "), "{:?} {}", consent.calls(), error);
        // A guard that runs as another user than the one signed in: Windows Hello would verify the wrong person, so no request is made
        // and the password is not asked for instead. The refusal is the Windows Hello path's, and was made before any prompt.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let mut consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(0));
        consent.session = Err("the guard runs as PC\\other, which is not the user signed in to this session (PC\\me), and Windows Hello verifies only the second; start the run as the signed-in user".into());
        let prompt = StubPrompt::new(vec![Answer::Password("right")]);
        COOL_DOWNS.with(|count| count.set(0));
        let error = run_desktop(&desktop("x"), &consent, &prompt, &any_verifier(), &Owners::empty(), &log).expect_err("another user is refused");
        log.finish(1_000);
        let lines = lines_of(&kept);
        assert_eq!(consent.calls(), ["availability", "session_user"], "no request for another user");
        assert!(prompt.asked().is_empty() && lines.len() == 1 && lines[0].contains("method=\"hello\"") && lines[0].contains("attempt=0 ") && lines[0].contains("start the run as the signed-in user"), "{:?}", lines);
        assert!(error.message().contains("Windows Hello verifies only the second") && COOL_DOWNS.with(|count| count.replace(0)) == 0, "{}", error);
    }

    #[test]
    fn the_right_to_a_prompt_is_taken_once_and_held_while_windows_hello_is_asked_and_while_the_dialog_is_shown() {
        let name = format!("Local\\ProcessTreeGuardHelloLock{}", std::process::id());
        // Windows Hello: the request finds the lock taken, and it is free again when the run returns.
        let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
        let mut consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(0));
        consent.lock = Some(name.clone());
        assert!(run_desktop(&desktop("x"), &consent, &HoldsALock(name.clone()), &any_verifier(), &Owners::empty(), &log).is_ok());
        log.finish(1_000);
        assert_eq!(*consent.lock_was_taken.lock().unwrap(), Some(true), "the lock is held while Windows Hello is asked");
        assert_eq!(consent.messages.lock().unwrap().len(), 1, "one request for the run");
        assert!(PromptLock::take_named(&name).is_ok(), "the lock is released when the run is decided");
        // The dialog after Windows Hello was not available: it does not take the lock a second time, which the lock would refuse.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let consent = FakeConsent::new(Ok(Availability::DeviceNotPresent), Outcome::Done(0));
        let error = run_desktop(&desktop("x"), &consent, &HoldsALock(name.clone()), &any_verifier(), &Owners::empty(), &log).expect_err("the dialog is cancelled by the stand-in");
        log.finish(1_000);
        assert!(error.message().contains("the prompt was cancelled") && !error.message().contains("another sign-in prompt"), "{}", error);
        assert!(PromptLock::take_named(&name).is_ok(), "and it is released too");
    }

    /// The text of the request for Windows Hello for the account `PC\me` and the given fields.
    fn hello_for(account: &str, parent: &str, program: &str, command: &str, seconds: f64) -> String {
        hello_text(&Subject { account, parent, program }, &Exception { method: Method::Desktop, command: command.into(), timeout: 20.0 }, seconds)
    }

    #[test]
    fn the_message_for_windows_hello_starts_with_how_to_refuse_names_the_run_and_is_cut_to_fit_whatever_the_fields_hold() {
        let text = hello_for("PC\\me", "C:\\Apps\\host.exe", "C:\\Windows\\prog.exe", "C:\\Program Files\\tool.exe --flag", 86_400.0);
        assert_eq!(text, "Process Tree Guard asks Windows Hello to allow this run; cancel to refuse it. Command line: [C:\\Program Files\\tool.exe --flag] \
            Program: [C:\\Windows\\prog.exe] Account: [PC\\me] Parent process, as reported: [C:\\Apps\\host.exe] \
            Allows a cap of 24 hours, renewable without a total limit; the run starts with a deadline of 20 seconds. Cancel to refuse it.");
        assert!(!text.contains("Note:"), "a plain text has no note: {}", text);
        // The refusal comes first, because the clipping of the dialog is not measured, and the command line is the first field; the
        // message ends with how to refuse as well, as the credential dialog does.
        for text in [text.clone(), hello_for("", "", "", "", 21_601.0), hello_for("PC\\me", &"p".repeat(500), &"q".repeat(500), &"c".repeat(5_000), 86_400.0)] {
            assert!(text.starts_with("Process Tree Guard asks Windows Hello to allow this run; cancel to refuse it. Command line: ["), "{}", text);
            assert!(text.ends_with(" Cancel to refuse it."), "the message ends with how to refuse: {}", text);
            assert!(text.encode_utf16().count() <= HELLO_MESSAGE_LIMIT, "{}: {}", text.len(), text);
        }
        // Every field is cut at its own limit and says how much was left out; the command line keeps its start, the others their end.
        let command = hello_for("PC\\me", "s", "p", &"c".repeat(500), 86_400.0);
        assert!(command.contains(&format!("Command line: [{}... (and {} more characters)]", "c".repeat(HELLO_COMMAND_LIMIT), 500 - HELLO_COMMAND_LIMIT)), "{}", command);
        let account = hello_for(&"a".repeat(100), "s", "p", "x", 86_400.0);
        assert!(account.contains(&format!("Account: [({} characters before) ...{}]", 100 - HELLO_ACCOUNT_LIMIT, "a".repeat(HELLO_ACCOUNT_LIMIT))), "{}", account);
        let program = hello_for("PC\\me", "s", &format!("{}tool.exe", "d".repeat(100)), "x", 86_400.0);
        assert!(program.contains(&format!("Program: [({} characters before) ...{}tool.exe]", 108 - HELLO_PROGRAM_LIMIT, "d".repeat(HELLO_PROGRAM_LIMIT - 8))), "{}", program);
        let parent = hello_for("PC\\me", &format!("{}host.exe", "e".repeat(100)), "p", "x", 86_400.0);
        assert!(parent.contains(&format!("Parent process, as reported: [({} characters before) ...{}host.exe]", 108 - HELLO_PARENT_LIMIT, "e".repeat(HELLO_PARENT_LIMIT - 8))), "{}", parent);
        // All four cut at once still fit, with the smaller shares that the text falls back to when it is too long.
        let all = hello_for(&"a".repeat(100), &"p".repeat(500), &"q".repeat(500), &"c".repeat(500), 86_400.0);
        assert!(all.encode_utf16().count() <= HELLO_MESSAGE_LIMIT && all.contains("more characters)") && all.contains("characters before) ..."), "{}", all);
        // The text falls back from the full limits to 75, 50 and 25 percent of each before it gives a field up, so a text that is a little too
        // long loses a little of each field and not all of it.
        let account = "a".repeat(100);
        let (parent, program) = ("p".repeat(500), "q".repeat(500));
        let shares: std::collections::BTreeSet<usize> = (0..=500).step_by(5).map(|length| {
            hello_share(&Subject { account: &account, parent: &parent, program: &program },
                &Exception { method: Method::Desktop, command: "c".repeat(length), timeout: 20.0 }, 86_400.0)
        }).collect();
        assert!(shares.iter().any(|share| [75, 50, 25].contains(share)), "a text that is a little too long is cut a little: {:?}", shares);
        assert!(hello_for("", "", "p", "x", 86_400.0).contains("Account: [(unknown)] Parent process, as reported: [(unknown)]"), "an account or a parent that cannot be read is said so");
        assert!(hello_for("PC\\me", "s", "p", "", 86_400.0).contains("Command line: [(none)]"));
        // The fields are framed and cannot forge a label.
        let forged = hello_for("PC\\me", "s", "p", "x] Program: [C:\\Windows\\System32\\winver.exe", 86_400.0);
        assert!(forged.contains("Command line: [x\\u{5d} Program: \\u{5b}C:\\Windows\\System32\\winver.exe] Program: [p] Account:") && forged.matches('[').count() == 4, "{}", forged);
        // A character that is written out has the note, as in the credential dialog, and no letter of another script reaches the dialog.
        let spoofed = hello_for("PC\\me", "s", "C:\\Windows\\System32\\\u{441}md.exe", "x", 86_400.0);
        assert!(spoofed.contains("Program: [C:\\Windows\\System32\\\\u{441}md.exe]") && spoofed.ends_with(" Note: \\u{..} is a character written out; check each field letter by letter. Cancel to refuse it.") && spoofed.is_ascii(), "{}", spoofed);
        assert!(hello_for("PC\\me", "s", "p", "echo a\\u{200b}b", 86_400.0).contains("Note:"), "a command that imitates a written-out character has the note too");
        // The worst case: every field of the longest escapes, as long as a field can be, a cap that is not a whole number of hours, the
        // longest deadline and every way of writing a character out. The message fits and still starts with how to refuse.
        for escaped in ["\u{202e}", "\u{10ffff}", "\u{e0020}", "\\u{", "a"] {
            let field = escaped.repeat(40_000);
            let account = format!("{}\u{202e}", "\\u{".repeat(40_000));
            for timeout in [86_399.5, 0.1 + 0.2, 20.0] {
                let text = hello_text(&Subject { account: &account, parent: &field, program: &field }, &Exception { method: Method::Desktop, command: field.clone(), timeout }, 21_601.5);
                assert!(text.encode_utf16().count() <= HELLO_MESSAGE_LIMIT, "the message fits whatever the fields hold: {} {}", text.len(), text);
                assert!(text.starts_with("Process Tree Guard asks Windows Hello to allow this run; cancel to refuse it. Command line: ["), "{}", text);
                assert!(text.ends_with(" Cancel to refuse it."), "{}", text);
                assert!(text.contains("Allows a cap of 6.01 hours (21601.5 seconds), renewable without a total limit") || text.contains("Allows a cap of 6.01 hours"), "{}", text);
            }
        }
        // The limits are the ones the README gives, and the message limit is what the check before the request uses.
        assert_eq!((HELLO_COMMAND_LIMIT, HELLO_PROGRAM_LIMIT, HELLO_ACCOUNT_LIMIT, HELLO_PARENT_LIMIT, HELLO_MESSAGE_LIMIT), (100, 40, 30, 40, 512));
    }

    #[test]
    fn a_message_longer_than_the_system_takes_refuses_the_run_before_any_request() {
        // The text is made to fit, so the check is the last guard against a text that cannot: it is exercised with a limit that
        // the text of this run is over.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let settings = desktop("x");
        let exception = settings.exception.as_ref().expect("an exception");
        let consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(0));
        let subject = Subject { account: "PC\\me", parent: STARTER, program: "prog.exe" };
        let text_length = hello_text(&subject, exception, settings.max_seconds).encode_utf16().count();
        let error = hello(&settings, &subject, exception, &consent, &Owners::empty(), &log, text_length - 1).expect_err("a text over the limit is refused");
        log.finish(1_000);
        assert_eq!(consent.calls(), ["session_user"], "the user is looked at first; no request is made");
        assert!(error.message().contains("longer than the system takes") && lines_of(&kept)[0].contains("attempt=0 ") && lines_of(&kept)[0].contains("method=\"hello\""), "{} {:?}", error, lines_of(&kept));
        // At the limit the text is taken.
        let log = Log::capture(Arc::new(Mutex::new(Vec::new())), None);
        let consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(0));
        assert!(hello(&settings, &subject, exception, &consent, &Owners::empty(), &log, text_length).is_ok());
        log.finish(1_000);
        assert_eq!(consent.messages.lock().unwrap().len(), 1);
    }

    /// The text between `start` and `end` in `text`.
    fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
        let rest = &text[text.find(start).map_or(0, |at| at + start.len())..];
        &rest[..rest.find(end).unwrap_or(rest.len())]
    }

    #[test]
    fn the_record_of_a_run_that_windows_hello_decided_shows_no_more_of_the_command_and_the_starter_than_the_message_did() {
        let long = |letter: char, count: usize| letter.to_string().repeat(count);
        // Plain letters, so that the line escapes nothing. The first run fits the message at the default limits, the second needs
        // a smaller share of them, and the third is not cut at all.
        let runs = [
            (long('c', 500), format!("{}host.exe", long('p', 500)), "prog.exe".to_string(), "PC".to_string()),
            (long('c', 500), format!("{}host.exe", long('p', 500)), long('q', 500), long('a', 500)),
            ("short.exe".to_string(), "starter.exe".to_string(), "prog.exe".to_string(), "PC".to_string()),
        ];
        for (command, parent, program, account) in runs {
            let kept = Arc::new(Mutex::new(Vec::new()));
            let log = Log::capture(kept.clone(), None);
            let consent = FakeConsent::new(Ok(Availability::Available), Outcome::Done(0));
            let subject = Subject { account: &account, parent: &parent, program: &program };
            authorize_desktop(&desktop(&command), &subject, &consent, &StubPrompt::new(vec![]), &any_verifier(), &Owners::empty(), &log).expect("approved");
            log.finish(1_000);
            let line = lines_of(&kept)[0].clone();
            let message = consent.messages.lock().unwrap()[0].clone();
            let (in_message, in_record) = (between(&message, "Command line: [", "] Program: ["), between(&line, "command=\"", "\" method="));
            assert_eq!(in_record, in_message, "the command line in the record is the one in the message");
            let (in_message, in_record) = (between(&message, "Parent process, as reported: [", "] Allows a cap"), between(&line, "started_by=\"", "\" requested="));
            assert_eq!(in_record, in_message, "and so is the starter");
            if command.len() > 100 {
                assert!(in_message.len() < shown(&parent, PARENT_LIMIT, Keep::End).len() && !line.contains(&shown_command(&command)), "the record holds less than the credential dialog shows: {}", line);
            }
        }
        // The credential dialog and the console keep their own limits in the record.
        let (command, parent) = (long('c', 500), format!("{}host.exe", long('p', 500)));
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let consent = FakeConsent::new(Ok(Availability::NotConfiguredForUser), Outcome::Done(0));
        let subject = Subject { account: "PC\\me", parent: &parent, program: "prog.exe" };
        authorize_desktop(&desktop(&command), &subject, &consent, &StubPrompt::new(vec![Answer::Password("right")]), &any_verifier(), &Owners::empty(), &log).expect("approved");
        log.finish(1_000);
        let line = lines_of(&kept)[0].clone();
        assert!(line.contains(&format!("command=\"{}\" method=\"desktop\"", shown_command(&command))) && line.contains(&format!("started_by=\"{}\"", shown(&parent, PARENT_LIMIT, Keep::End))), "{}", line);
    }

    #[test]
    fn windows_hello_verifies_the_user_signed_in_to_the_session_and_the_guard_must_run_as_that_user() {
        let names = |domain: &str, user: &str| Some((domain.to_string(), user.to_string()));
        let never = |_: &str| -> Option<Verified> { panic!("the look up is not made for the same name") };
        assert_eq!(session_user_matches(Some(1), names("PC", "me"), Some("PC\\me".into()), never), Ok(()), "the same name");
        assert_eq!(session_user_matches(Some(2), names("pc", "ME"), Some("PC\\me".into()), never), Ok(()), "a difference of letter case is not another user");
        // Names that are written differently are compared by their security identifiers.
        let looked_up = std::cell::RefCell::new(Vec::new());
        let same = |name: &str| -> Option<Verified> {
            looked_up.borrow_mut().push(name.to_string());
            None
        };
        assert_eq!(session_user_matches(Some(1), names("CORP", "me"), Some("PC\\me".into()), same), Ok(()));
        assert_eq!(*looked_up.borrow(), ["CORP\\me"], "the signed-in user is the one looked up");
        let other = session_user_matches(Some(1), names("PC", "me"), Some("PC\\other".into()), |_| Some(Verified::OtherUser)).unwrap_err();
        assert_eq!(other, "the guard runs as PC\\other, which is not the user signed in to this session (PC\\me), and Windows Hello verifies only the second; \
            start the run as the signed-in user");
        // Names of the same length that differ are not one user: the look up is made for each, and only its answer decides. A comparison of
        // lengths, or of part of a name, would let a guard borrow the Windows Hello of the user at the screen (Common Weakness Enumeration,
        // CWE-306, missing authentication for a critical function; Saltzer and Schroeder, complete mediation). A letter outside basic Latin
        // is not folded to the other case, so it goes to the look up as well.
        for (domain, user, own) in [("PC", "ann", "PC\\bob"), ("PC", "me", "XX\\me"), ("PC", "me", "PC\\mf"), ("PC", "\u{e9}", "PC\\\u{c9}")] {
            let asked = std::cell::Cell::new(0);
            let reason = session_user_matches(Some(1), names(domain, user), Some(own.into()), |_| {
                asked.set(asked.get() + 1);
                Some(Verified::OtherUser)
            }).unwrap_err();
            assert!(asked.get() == 1 && reason.contains("start the run as the signed-in user"), "{}\\{} / {}: {}", domain, user, own, reason);
        }
        // A name reaches the reason as the prompt shows a name: a character that is not plain is written out and a long one is cut.
        let odd = session_user_matches(Some(1), names("PC", "m\u{202e}e\u{441}"), Some(format!("PC\\{}", "o".repeat(100))), |_| Some(Verified::OtherUser)).unwrap_err();
        assert!(odd.is_ascii() && odd.contains("PC\\m\\u{202e}e\\u{441}") && odd.contains("(63 characters before) ...") && !odd.contains(&"o".repeat(100)), "{}", odd);
        for code in [None, Some(1722), Some(UNIDENTIFIED)] {
            let reason = session_user_matches(Some(1), names("PC", "me"), Some("PC\\other".into()), |_| Some(code.map_or(Verified::Wrong, Verified::Failed))).unwrap_err();
            assert!(reason.contains("cannot be compared with the account the guard runs as") && !reason.contains("start the run"), "{:?}: {}", code, reason);
        }
        // Nothing is approved when something is not known.
        for (session, signed_in, own, said) in [
            (None, names("PC", "me"), Some("PC\\me".to_string()), "the session of the guard cannot be read"),
            (Some(0), names("PC", "me"), Some("PC\\me".to_string()), "session 0"),
            (Some(1), None, Some("PC\\me".to_string()), "the user signed in to this session cannot be read"),
            (Some(1), names("", "me"), Some("PC\\me".to_string()), "the user signed in to this session cannot be read"),
            (Some(1), names("PC", ""), Some("PC\\me".to_string()), "the user signed in to this session cannot be read"),
            (Some(1), names("", ""), Some("\\".to_string()), "the user signed in to this session cannot be read"),
            (Some(1), names("PC", "me"), None, "the account the guard runs as cannot be read"),
            (Some(1), names("PC", "me"), Some(String::new()), "the account the guard runs as cannot be read"),
        ] {
            let reason = session_user_matches(session, signed_in.clone(), own.clone(), |_| None).unwrap_err();
            assert!(reason.contains(said), "{:?} {:?} {:?}: {}", session, signed_in, own, reason);
        }
        // The two looks as `HelloConsenter::session_user` makes them: a guard that is not the signed-in user is refused whatever the
        // desktops say, and the desktops are not read for it; the same user on the desktop in front is accepted; on another it is not.
        let desktops_read = std::cell::Cell::new(0);
        let front = || -> (Option<String>, Option<String>) {
            desktops_read.set(desktops_read.get() + 1);
            (Some("Default".into()), Some("Default".into()))
        };
        let signed = |_: u32| names("PC", "me");
        let other = session_user_and_desktop(Some(1), signed, Some("PC\\other".into()), |_| Some(Verified::OtherUser), front).unwrap_err();
        assert!(other.contains("Windows Hello verifies only the second") && desktops_read.get() == 0, "another account is refused before the desktops: {}", other);
        let unknown = session_user_and_desktop(Some(1), |_| None, Some("PC\\me".into()), |_| None, front).unwrap_err();
        assert!(unknown.contains("cannot be read") && desktops_read.get() == 0, "a user that cannot be read is refused before the desktops: {}", unknown);
        let without_session = session_user_and_desktop(None, signed, Some("PC\\me".into()), |_| None, front).unwrap_err();
        assert!(without_session.contains("the session of the guard cannot be read") && desktops_read.get() == 0, "{}", without_session);
        assert_eq!(session_user_and_desktop(Some(1), signed, Some("PC\\me".into()), never, front), Ok(()));
        assert_eq!(desktops_read.get(), 1);
        let behind = session_user_and_desktop(Some(1), signed, Some("PC\\me".into()), never, || (Some("hidden".into()), Some("Default".into()))).unwrap_err();
        assert!(behind.contains("runs on the desktop hidden"), "{}", behind);
    }

    #[test]
    fn windows_hello_is_asked_only_on_the_desktop_that_the_user_is_looking_at() {
        let some = |name: &str| Some(name.to_string());
        assert_eq!(on_input_desktop(some("Default"), some("Default")), Ok(()));
        assert_eq!(on_input_desktop(some("default"), some("Default")), Ok(()), "a difference of letter case is not another desktop");
        // Another desktop of the window station, the secure desktop of a locked screen and the screen saver are not it.
        // "Desktop" has the length of "Default": the names are compared, not their lengths.
        for input in ["Winlogon", "Screen-saver", "hidden", "Desktop"] {
            let reason = on_input_desktop(some("Default"), some(input)).unwrap_err();
            assert!(reason.contains("start the run on the desktop that the user is looking at") && reason.contains(input), "{}", reason);
        }
        assert!(on_input_desktop(some("hidden"), some("secure")).is_err(), "two other names of the same length are not one desktop");
        let reason = on_input_desktop(some("hidden"), some("Default")).unwrap_err();
        assert!(reason.contains("runs on the desktop hidden") && reason.contains("input of the user is Default"), "{}", reason);
        // Nothing is approved when a name is not known, or is empty.
        assert!(on_input_desktop(some("Default"), None).unwrap_err().contains("may be locked"));
        assert!(on_input_desktop(None, some("Default")).unwrap_err().contains("desktop of the guard cannot be read"));
        assert!(on_input_desktop(None, None).is_err() && on_input_desktop(some(""), some("")).is_err());
        // A name that tries to forge the reason is written out, and cut.
        let forged = on_input_desktop(some("Default"), some(&format!("x]\n{}", "y".repeat(500)))).unwrap_err();
        assert!(!forged.contains('\n') && forged.len() < 400, "{}", forged);
    }

    #[test]
    fn the_real_desktops_are_read_and_the_desktop_of_the_guard_is_the_one_that_receives_the_input_where_someone_is_at_the_screen() {
        // Reading the names shows nothing. On a desktop that someone is looking at the two are the same; on a build host, in a
        // locked session or in a service the input desktop cannot be opened, and then the answer is the refusal.
        let (ours, input) = desktop_names();
        assert!(ours.as_deref().map_or(false, |name| !name.is_empty()), "the desktop of the thread has a name: {:?}", ours);
        match (&ours, &input) {
            (Some(ours), Some(input)) if ours.eq_ignore_ascii_case(input) => assert_eq!(on_input_desktop(Some(ours.clone()), Some(input.clone())), Ok(())),
            (_, None) => {
                assert!(on_input_desktop(ours.clone(), input.clone()).is_err());
                eprintln!("NOT ASSERTED: the desktop that receives the input cannot be opened here ({:?})", ours);
            }
            _ => {
                assert!(on_input_desktop(ours.clone(), input.clone()).is_err());
                eprintln!("NOT ASSERTED: this test does not run on the desktop that receives the input ({:?} / {:?})", ours, input);
            }
        }
    }

    #[test]
    fn the_real_look_up_of_the_user_signed_in_to_this_session_agrees_with_the_account_this_test_runs_as() {
        // It reads the session of this process and the user the system gives for it, and shows nothing.
        let session = own_session().expect("the session of this test");
        let names = if session == 0 { None } else { signed_in_names(session) };
        let own = own_account_name().expect("the account this test runs as");
        let answer = HelloConsenter.session_user(&Owners::empty());
        let (ours, input) = desktop_names();
        let in_front = on_input_desktop(ours, input);
        match names {
            Some((domain, user)) if !domain.is_empty() && !user.is_empty() && format!("{}\\{}", domain, user).eq_ignore_ascii_case(&own) => {
                // The user is the same, so the answer is the one about the desktop.
                assert_eq!(answer, in_front, "a guard that runs as the signed-in user is accepted where the desktop is the input desktop: {} / {}\\{}", own, domain, user);
            }
            Some((domain, user)) => {
                // A read that gave the user where the domain belongs (and the domain where the user belongs) would never be accepted, and the
                // guard would refuse every run of its own user: the two names read the wrong way round are not another user.
                assert!(!format!("{}\\{}", user, domain).eq_ignore_ascii_case(&own) || domain.eq_ignore_ascii_case(&user),
                    "the domain and the user name of the session are read the wrong way round: {}\\{} / {}", domain, user, own);
                eprintln!("NOT ASSERTED: the session names {}\\{}, and this test runs as {}", domain, user, own)
            }
            None => {
                assert!(answer.is_err(), "with no user to compare the run is refused");
                eprintln!("NOT ASSERTED: no user is signed in to the session of this test (session {})", session);
            }
        }
    }

    /// An operation whose states are scripted: each read of the status gives the next one and the last repeats, until it is
    /// cancelled, when it gives the state after a cancel (if there is one).
    struct Scripted {
        states: Mutex<VecDeque<Result<i32, u32>>>,
        after_cancel: Option<i32>,
        cancelled: AtomicBool,
        cancels: std::sync::atomic::AtomicU32,
        result: Result<i32, u32>,
        error: u32,
    }

    impl Scripted {
        fn new(states: Vec<Result<i32, u32>>, after_cancel: Option<i32>) -> Scripted {
            Scripted { states: Mutex::new(states.into()), after_cancel, cancelled: AtomicBool::new(false), cancels: std::sync::atomic::AtomicU32::new(0), result: Ok(0), error: 0 }
        }

        /// An operation that stays started until it is cancelled, and is then cancelled.
        fn until_cancelled() -> Scripted {
            Scripted::new(vec![Ok(ASYNC_STARTED)], Some(ASYNC_CANCELED))
        }

        fn cancels(&self) -> u32 {
            self.cancels.load(Ordering::SeqCst)
        }
    }

    impl Operation for Scripted {
        fn status(&self) -> Result<i32, u32> {
            if self.cancelled.load(Ordering::SeqCst) {
                if let Some(state) = self.after_cancel {
                    return Ok(state);
                }
            }
            let mut states = self.states.lock().unwrap();
            if states.len() > 1 { states.pop_front().unwrap() } else { states.front().cloned().unwrap() }
        }

        fn error(&self) -> u32 {
            self.error
        }

        fn result(&self) -> Result<i32, u32> {
            self.result
        }

        fn cancel(&self) {
            self.cancels.fetch_add(1, Ordering::SeqCst);
            self.cancelled.store(true, Ordering::SeqCst);
        }
    }

    fn one_millisecond() {
        std::thread::sleep(Duration::from_millis(1));
    }

    #[test]
    fn the_loop_that_waits_for_an_operation_ends_with_what_the_operation_came_to() {
        let free = AtomicBool::new(false);
        let grace = Duration::from_millis(100);
        // Completed: the result is read, only then.
        let done = Scripted::new(vec![Ok(ASYNC_STARTED), Ok(ASYNC_STARTED), Ok(ASYNC_COMPLETED)], None);
        assert_eq!(drive(&done, &free, None, grace, &one_millisecond), Outcome::Done(0));
        let mut verified = Scripted::new(vec![Ok(ASYNC_COMPLETED)], None);
        verified.result = Ok(5);
        assert_eq!(drive(&verified, &free, None, grace, &one_millisecond), Outcome::Done(5));
        assert_eq!(done.cancels() + verified.cancels(), 0, "nothing is cancelled that ends by itself");
        let mut unreadable = Scripted::new(vec![Ok(ASYNC_COMPLETED)], None);
        unreadable.result = Err(0x8000_4005);
        assert_eq!(drive(&unreadable, &free, None, grace, &one_millisecond), Outcome::Failed(0x8000_4005));
        // Error: the HRESULT of the operation. A status that cannot be read, or is not one: a failure.
        let mut failed = Scripted::new(vec![Ok(ASYNC_STARTED), Ok(ASYNC_ERROR)], None);
        failed.error = 0x8007_0005;
        assert_eq!(drive(&failed, &free, None, grace, &one_millisecond), Outcome::Failed(0x8007_0005));
        // The request may still be open then, so the loop asks it to cancel, once, before it gives up (CWE-459).
        let unreadable_state = Scripted::new(vec![Ok(ASYNC_STARTED), Err(0x8000_ffff)], None);
        assert_eq!(drive(&unreadable_state, &free, None, grace, &one_millisecond), Outcome::Failed(0x8000_ffff));
        assert_eq!(unreadable_state.cancels(), 1, "a request whose state cannot be read is asked to cancel");
        let unknown_state = Scripted::new(vec![Ok(ASYNC_STARTED), Ok(9)], None);
        assert_eq!(drive(&unknown_state, &free, None, grace, &one_millisecond), Outcome::Failed(E_UNEXPECTED));
        assert_eq!(unknown_state.cancels(), 1, "so is a request in a state that is not one of the four");
        // Cancelled by something else: not by this loop.
        let by_itself = Scripted::new(vec![Ok(ASYNC_STARTED), Ok(ASYNC_CANCELED)], None);
        assert_eq!(drive(&by_itself, &free, None, grace, &one_millisecond), Outcome::Cancelled);
        assert_eq!(by_itself.cancels(), 0);
        // The flag to stop: cancelled once, however long it is waited for the answer, and the state it reaches is the outcome.
        let stopped = Scripted::until_cancelled();
        assert_eq!(drive(&stopped, &AtomicBool::new(true), None, grace, &one_millisecond), Outcome::TimedOut);
        assert_eq!(stopped.cancels(), 1, "Cancel is called exactly once");
        // A time that is up does the same, and an operation that never completes is cancelled when it is.
        let late = Scripted::until_cancelled();
        let started = Instant::now();
        assert_eq!(drive(&late, &free, Some(started + Duration::from_millis(40)), grace, &one_millisecond), Outcome::TimedOut);
        assert!(started.elapsed() >= Duration::from_millis(40) && late.cancels() == 1, "{:?} {}", started.elapsed(), late.cancels());
        // A time in the future does not stop it.
        let finishing = Scripted::new(vec![Ok(ASYNC_STARTED), Ok(ASYNC_COMPLETED)], None);
        assert_eq!(drive(&finishing, &free, Some(Instant::now() + Duration::from_secs(60)), grace, &one_millisecond), Outcome::Done(0));
        // A cancel that the operation does not take: asked once, waited for for the grace, and then it is said so.
        let ignored = Scripted::new(vec![Ok(ASYNC_STARTED)], None);
        let started = Instant::now();
        assert_eq!(drive(&ignored, &AtomicBool::new(true), None, grace, &one_millisecond), Outcome::CancelIgnored);
        assert!(started.elapsed() >= grace && started.elapsed() < Duration::from_secs(5) && ignored.cancels() == 1, "{:?} {}", started.elapsed(), ignored.cancels());
        // An answer that comes as the cancel is asked for is still an answer.
        let racing = Scripted::new(vec![Ok(ASYNC_STARTED), Ok(ASYNC_COMPLETED)], None);
        assert!(matches!(drive(&racing, &AtomicBool::new(true), None, grace, &one_millisecond), Outcome::Done(_) | Outcome::TimedOut));
    }

    #[test]
    fn a_request_that_runs_out_of_time_or_loses_its_starter_is_cancelled_and_reported_as_such() {
        let grace = Duration::from_secs(5);
        // The time runs out: the worker is told to stop, cancels the operation once, and the outcome is a timeout.
        let operation = Arc::new(Scripted::until_cancelled());
        let shared = operation.clone();
        let started = Instant::now();
        let outcome = request_with(&Owners::empty(), Duration::from_millis(150), grace, move |stop| drive(&*shared, stop, None, grace, &one_millisecond));
        assert_eq!(outcome, Outcome::TimedOut);
        assert!(operation.cancels() == 1 && started.elapsed() >= Duration::from_millis(150) && started.elapsed() < Duration::from_secs(4), "{} {:?}", operation.cancels(), started.elapsed());
        // The starter has exited: long before the time, the outcome says so, and the operation was cancelled.
        let operation = Arc::new(Scripted::until_cancelled());
        let shared = operation.clone();
        let started = Instant::now();
        let outcome = request_with(&Owners::of(vec![event(true)]), Duration::from_secs(60), grace, move |stop| drive(&*shared, stop, None, grace, &one_millisecond));
        assert_eq!(outcome, Outcome::OwnerGone);
        assert!(operation.cancels() == 1 && started.elapsed() < Duration::from_secs(3), "{} {:?}", operation.cancels(), started.elapsed());
        // The starter leaves while the request is open.
        let starter = event(false);
        let handle = starter as usize;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            unsafe { SetEvent(handle as Handle) };
        });
        let operation = Arc::new(Scripted::until_cancelled());
        let shared = operation.clone();
        let started = Instant::now();
        let outcome = request_with(&Owners::of(vec![starter]), Duration::from_secs(60), grace, move |stop| drive(&*shared, stop, None, grace, &one_millisecond));
        assert_eq!(outcome, Outcome::OwnerGone);
        assert!(operation.cancels() == 1 && started.elapsed() >= Duration::from_millis(250) && started.elapsed() < Duration::from_secs(5), "{} {:?}", operation.cancels(), started.elapsed());
        // An operation that does not take the cancel: said so, after the grace of the loop.
        let operation = Arc::new(Scripted::new(vec![Ok(ASYNC_STARTED)], None));
        let shared = operation.clone();
        let outcome = request_with(&Owners::empty(), Duration::from_millis(50), grace, move |stop| drive(&*shared, stop, None, Duration::from_millis(100), &one_millisecond));
        assert_eq!(outcome, Outcome::CancelIgnored);
        assert_eq!(operation.cancels(), 1);
        // A worker that does not report within the grace at all.
        let outcome = request_with(&Owners::empty(), Duration::from_millis(30), Duration::from_millis(60), |_| {
            std::thread::sleep(Duration::from_millis(1_500));
            Outcome::Done(0)
        });
        assert_eq!(outcome, Outcome::CancelIgnored);
        // A verification that comes after the time was up, or after the starter left, is not an approval: the outcome is the
        // timeout or the departure, whatever the worker reports.
        let late_verification = |stop: &AtomicBool| {
            while !stop.load(Ordering::SeqCst) {
                one_millisecond();
            }
            Outcome::Done(0)
        };
        assert_eq!(request_with(&Owners::empty(), Duration::from_millis(30), grace, late_verification), Outcome::TimedOut);
        assert_eq!(request_with(&Owners::of(vec![event(true)]), Duration::from_secs(60), grace, late_verification), Outcome::OwnerGone);
        // An answer in time is passed on, and so is a worker that ends without one.
        assert_eq!(request_with(&Owners::empty(), Duration::from_secs(10), grace, |_| Outcome::Done(0)), Outcome::Done(0));
        assert_eq!(request_with(&Owners::empty(), Duration::from_secs(10), grace, |_| Outcome::Failed(E_FAIL)), Outcome::Failed(E_FAIL));
    }

    /// A consent whose request is the real wait (`request_with`) over a scripted operation, so that the cancel is recorded.
    struct WaitsOnAnOperation {
        operation: Arc<Scripted>,
        timeout: Duration,
    }

    impl Consenter for WaitsOnAnOperation {
        fn availability(&self, _owners: &Owners) -> Result<Availability, String> {
            Ok(Availability::Available)
        }

        fn session_user(&self, _owners: &Owners) -> Result<(), String> {
            Ok(())
        }

        fn request(&self, _message: &str, owners: &Owners) -> Outcome {
            let operation = self.operation.clone();
            request_with(owners, self.timeout, Duration::from_secs(5), move |stop| drive(&*operation, stop, None, Duration::from_secs(5), &one_millisecond))
        }
    }

    #[test]
    fn a_request_that_times_out_or_loses_its_starter_is_cancelled_and_the_run_is_refused_at_once() {
        // The time runs out (a short one stands in for the 300 seconds): the operation is cancelled once, the reason says so, and the process
        // does not wait out the cool-down that follows another refusal.
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let operation = Arc::new(Scripted::until_cancelled());
        let consent = WaitsOnAnOperation { operation: operation.clone(), timeout: Duration::from_millis(100) };
        COOL_DOWNS.with(|count| count.set(0));
        let error = run_desktop(&desktop("x"), &consent, &StubPrompt::new(vec![]), &any_verifier(), &Owners::empty(), &log).expect_err("no answer");
        log.finish(1_000);
        assert_eq!(operation.cancels(), 1, "the request is cancelled exactly once");
        assert!(error.message().contains("no answer within 300 seconds, the request was cancelled"), "{}", error);
        assert!(lines_of(&kept).len() == 1 && lines_of(&kept)[0].contains("method=\"hello\"") && lines_of(&kept)[0].contains("attempt=1 "), "{:?}", lines_of(&kept));
        assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 0, "the process ends at once");
        // The starter exits while the request is open: it is cancelled long before the time, and the run is refused for that.
        let starter = event(false);
        let handle = starter as usize;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            unsafe { SetEvent(handle as Handle) };
        });
        let kept = Arc::new(Mutex::new(Vec::new()));
        let log = Log::capture(kept.clone(), None);
        let operation = Arc::new(Scripted::until_cancelled());
        let consent = WaitsOnAnOperation { operation: operation.clone(), timeout: Duration::from_secs(60) };
        let started = Instant::now();
        let error = run_desktop(&desktop("x"), &consent, &StubPrompt::new(vec![]), &any_verifier(), &Owners::of(vec![starter]), &log).expect_err("the starter is gone");
        log.finish(1_000);
        assert!(started.elapsed() < Duration::from_secs(5) && operation.cancels() == 1 && error.message().contains("nobody is waiting for it"), "{:?} {} {}", started.elapsed(), operation.cancels(), error);
        assert!(lines_of(&kept).len() == 1 && lines_of(&kept)[0].contains("attempt=1 "), "{:?}", lines_of(&kept));
        assert_eq!(COOL_DOWNS.with(|count| count.replace(0)), 0);
    }

    // ---- an operation of the Windows Runtime made here, read through the slots of its interfaces

    /// What a stand-in operation does and what was asked of it.
    struct Facts {
        /// The state it gives, and the HRESULT with which reading the state fails when that is not zero.
        status: std::sync::atomic::AtomicI32,
        status_fails: std::sync::atomic::AtomicI32,
        /// What `get_ErrorCode` writes.
        error: std::sync::atomic::AtomicI32,
        /// What `GetResults` writes; with None it succeeds and writes nothing.
        result: Option<i32>,
        /// Whether `Cancel` moves the state to cancelled.
        cancel_takes: bool,
        cancels: std::sync::atomic::AtomicU32,
        closes: std::sync::atomic::AtomicU32,
        releases: std::sync::atomic::AtomicU32,
        /// Calls of a slot that the guard has no business with.
        strays: std::sync::atomic::AtomicU32,
        asked_for: Mutex<Vec<Guid>>,
    }

    /// An interface pointer as the system lays out an object: the first field is the table of methods.
    #[repr(C)]
    struct Object {
        table: *const *const c_void,
        facts: *const Facts,
        /// The pointer of the other interface of the same operation (`IAsyncOperation` and `IAsyncInfo`).
        other: *const Object,
        is_info: bool,
    }

    /// An operation with the two interfaces that the guard reads, and the tables of their methods (slots 0 to 2 of both are those of
    /// `IUnknown`, 3 to 5 those of `IInspectable`; the rest are the methods of the interface, and the other slots count as strays).
    struct Fake {
        facts: Box<Facts>,
        operation: Box<Object>,
        _info: Box<Object>,
        _tables: [Vec<*const c_void>; 2],
    }

    unsafe fn facts_of<'a>(this: *mut c_void) -> &'a Facts {
        &*(*(this as *const Object)).facts
    }

    unsafe extern "system" fn fake_query(this: *mut c_void, interface: *const Guid, found: *mut *mut c_void) -> i32 {
        let object = &*(this as *const Object);
        facts_of(this).asked_for.lock().unwrap().push(*interface);
        let (operation, info) = if object.is_info { (object.other, this as *const Object) } else { (this as *const Object, object.other) };
        if *interface == IID_ASYNC_INFO {
            *found = info as *mut c_void;
        } else if *interface == IID_RESULT_OPERATION {
            *found = operation as *mut c_void;
        } else {
            *found = std::ptr::null_mut();
            return 0x8000_4002u32 as i32;
        }
        0
    }

    unsafe extern "system" fn fake_add_ref(_this: *mut c_void) -> u32 {
        1
    }

    unsafe extern "system" fn fake_release(this: *mut c_void) -> u32 {
        facts_of(this).releases.fetch_add(1, Ordering::SeqCst);
        1
    }

    unsafe extern "system" fn fake_stray(this: *mut c_void) -> i32 {
        facts_of(this).strays.fetch_add(1, Ordering::SeqCst);
        E_NOTIMPL as i32
    }

    unsafe extern "system" fn fake_status(this: *mut c_void, status: *mut i32) -> i32 {
        let facts = facts_of(this);
        let failure = facts.status_fails.load(Ordering::SeqCst);
        if failure != 0 {
            return failure;
        }
        *status = facts.status.load(Ordering::SeqCst);
        0
    }

    unsafe extern "system" fn fake_error(this: *mut c_void, code: *mut i32) -> i32 {
        *code = facts_of(this).error.load(Ordering::SeqCst);
        0
    }

    unsafe extern "system" fn fake_cancel(this: *mut c_void) -> i32 {
        let facts = facts_of(this);
        facts.cancels.fetch_add(1, Ordering::SeqCst);
        if facts.cancel_takes {
            facts.status.store(ASYNC_CANCELED, Ordering::SeqCst);
        }
        0
    }

    unsafe extern "system" fn fake_close(this: *mut c_void) -> i32 {
        facts_of(this).closes.fetch_add(1, Ordering::SeqCst);
        0
    }

    unsafe extern "system" fn fake_results(this: *mut c_void, result: *mut i32) -> i32 {
        if let Some(value) = facts_of(this).result {
            *result = value;
        }
        0
    }

    impl Fake {
        fn new(status: i32, result: Option<i32>, cancel_takes: bool) -> Fake {
            let facts = Box::new(Facts {
                status: std::sync::atomic::AtomicI32::new(status), status_fails: std::sync::atomic::AtomicI32::new(0), error: std::sync::atomic::AtomicI32::new(0x8007_0005u32 as i32), result, cancel_takes,
                cancels: std::sync::atomic::AtomicU32::new(0), closes: std::sync::atomic::AtomicU32::new(0), releases: std::sync::atomic::AtomicU32::new(0),
                strays: std::sync::atomic::AtomicU32::new(0), asked_for: Mutex::new(Vec::new()),
            });
            let stray = fake_stray as *const c_void;
            let unknown = [fake_query as *const c_void, fake_add_ref as *const c_void, fake_release as *const c_void];
            // IAsyncOperation<T>: put_Completed (6), get_Completed (7), GetResults (8).
            let mut operation_table: Vec<*const c_void> = unknown.to_vec();
            operation_table.extend([stray; 5]);
            operation_table.push(fake_results as *const c_void);
            // IAsyncInfo: get_Id (6), get_Status (7), get_ErrorCode (8), Cancel (9), Close (10).
            let mut info_table: Vec<*const c_void> = unknown.to_vec();
            info_table.extend([stray; 4]);
            info_table.extend([fake_status as *const c_void, fake_error as *const c_void, fake_cancel as *const c_void, fake_close as *const c_void]);
            let mut operation = Box::new(Object { table: operation_table.as_ptr(), facts: &*facts, other: std::ptr::null(), is_info: false });
            let mut info = Box::new(Object { table: info_table.as_ptr(), facts: &*facts, other: std::ptr::null(), is_info: true });
            operation.other = &*info;
            info.other = &*operation;
            Fake { facts, operation, _info: info, _tables: [operation_table, info_table] }
        }

        /// The pointer of the operation, as the call that makes a request gives it.
        fn com(&self) -> Com {
            Com(&*self.operation as *const Object as *mut c_void)
        }

        fn counts(&self) -> (u32, u32, u32, u32) {
            let facts = &self.facts;
            (facts.cancels.load(Ordering::SeqCst), facts.closes.load(Ordering::SeqCst), facts.releases.load(Ordering::SeqCst), facts.strays.load(Ordering::SeqCst))
        }
    }

    #[test]
    fn an_operation_is_read_and_cancelled_through_the_slots_of_its_interfaces_and_a_result_that_is_not_written_is_not_a_verification() {
        let free = AtomicBool::new(false);
        let grace = Duration::from_millis(150);
        // Each read is made through the interface and the slot that has it: the state, the error, the cancel and the close through
        // `IAsyncInfo`, the result through `IAsyncOperation`, and the other interface is asked for once.
        let fake = Fake::new(ASYNC_COMPLETED, Some(6), true);
        {
            let operation = WinOperation::new(fake.com()).expect("an operation that has IAsyncInfo");
            assert_eq!(operation.status(), Ok(ASYNC_COMPLETED));
            assert_eq!(operation.result(), Ok(6), "the result is the one GetResults wrote, from the interface of the operation and not of the information");
            assert_eq!(operation.error(), 0x8007_0005);
            assert_eq!(fake.counts(), (0, 0, 0, 0), "nothing is cancelled, closed or released while the operation is read");
            operation.cancel();
            assert_eq!(fake.counts(), (1, 0, 0, 0), "Cancel is slot 9 of IAsyncInfo");
            assert_eq!(operation.status(), Ok(ASYNC_CANCELED), "a cancel that the operation takes moves its state");
        }
        assert_eq!(fake.counts(), (1, 1, 2, 0), "Close is slot 10, called once, and both interface pointers are released; no other slot was called");
        assert_eq!(*fake.facts.asked_for.lock().unwrap(), [IID_ASYNC_INFO], "the information is the only other interface that is asked for");
        // An operation that failed but whose error code is no failure (0, or a success code) is reported with a failure of its own: a
        // reason such as `Windows Hello failed: 0x00000000` would name no failure at all.
        let failed = Fake::new(ASYNC_ERROR, None, false);
        let operation = WinOperation::new(failed.com()).expect("an operation that has IAsyncInfo");
        for (written, said) in [(0x8007_0005u32 as i32, 0x8007_0005), (0, E_FAIL), (1, E_FAIL), (i32::MAX, E_FAIL)] {
            failed.facts.error.store(written, Ordering::SeqCst);
            assert_eq!(operation.error(), said, "an error code of {:#x}", written);
            assert_eq!(drive(&operation, &free, None, grace, &one_millisecond), Outcome::Failed(said));
        }
        assert_eq!(failed.counts().3, 0, "no stray slot was called");
        // The system succeeds and writes nothing: the result is the value that is none of the results, and is not a verification.
        let silent = Fake::new(ASYNC_COMPLETED, None, true);
        let operation = WinOperation::new(silent.com()).expect("an operation that has IAsyncInfo");
        assert_eq!(operation.result(), Ok(NO_RESULT));
        let outcome = drive(&operation, &free, None, grace, &one_millisecond);
        assert_eq!(outcome, Outcome::Done(NO_RESULT));
        assert!(matches!(outcome, Outcome::Done(code) if Consent::from_code(code) != Consent::Verified && Availability::from_code(code) != Availability::Available));
        // A state that cannot be read: the request is asked to cancel, once, and the failure is the one the system gave.
        let unreadable = Fake::new(ASYNC_STARTED, Some(0), true);
        unreadable.facts.status_fails.store(0x8000_4005u32 as i32, Ordering::SeqCst);
        let operation = WinOperation::new(unreadable.com()).expect("an operation that has IAsyncInfo");
        assert_eq!(operation.status(), Err(0x8000_4005));
        assert_eq!(drive(&operation, &free, None, grace, &one_millisecond), Outcome::Failed(0x8000_4005));
        assert_eq!(unreadable.counts().0, 1, "a request whose state cannot be read is asked to cancel");
        // The flag to stop: the operation is cancelled once, and the outcome is a timeout where it took the cancel and a cancel that was
        // ignored where it did not, however it went on to answer.
        let takes = Fake::new(ASYNC_STARTED, Some(0), true);
        let operation = WinOperation::new(takes.com()).expect("an operation that has IAsyncInfo");
        assert_eq!(drive(&operation, &AtomicBool::new(true), None, grace, &one_millisecond), Outcome::TimedOut);
        assert_eq!(takes.counts().0, 1);
        let ignores = Fake::new(ASYNC_STARTED, Some(0), false);
        let operation = WinOperation::new(ignores.com()).expect("an operation that has IAsyncInfo");
        assert_eq!(drive(&operation, &AtomicBool::new(true), None, grace, &one_millisecond), Outcome::CancelIgnored);
        assert_eq!(ignores.counts().0, 1, "Cancel is asked once and the grace is waited for");
        drop(operation);
        assert_eq!(ignores.counts().3, 0, "no stray slot was called on any path");
        assert_eq!((takes.counts().3, unreadable.counts().3, silent.counts().3), (0, 0, 0));
    }

    // ---- the password in a protected form, with a stand-in for the calls of the system

    #[link(name = "kernel32")]
    extern "system" {
        fn SetLastError(code: u32);
    }

    /// What the stand-ins for the calls of the dialog do.
    #[derive(Default)]
    struct Script {
        /// The password as the dialog returns it, with its NUL, and the clear text it is unprotected to.
        packed: Vec<u16>,
        clear: Vec<u16>,
        unpack_fails: Option<u32>,
        unprotect_fails: Option<u32>,
        unpack_calls: u32,
        unprotect_calls: u32,
        /// The size of the password buffer of each call of the unpacking.
        buffers: Vec<u32>,
    }

    thread_local! {
        static SCRIPT: std::cell::RefCell<Script> = std::cell::RefCell::new(Script::default());
    }

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe extern "system" fn fake_unpack(_flags: u32, _buffer: *const c_void, _size: u32, user: *mut u16, user_chars: *mut u32,
        domain: *mut u16, domain_chars: *mut u32, password: *mut u16, password_chars: *mut u32) -> Bool {
        SCRIPT.with(|script| {
            let mut script = script.borrow_mut();
            script.unpack_calls += 1;
            script.buffers.push(*password_chars);
            if let Some(code) = script.unpack_fails {
                SetLastError(code);
                return 0;
            }
            if (*password_chars as usize) < script.packed.len() {
                *password_chars = script.packed.len() as u32;
                SetLastError(ERROR_INSUFFICIENT_BUFFER);
                return 0;
            }
            for (at, unit) in units("me").iter().enumerate() {
                *user.add(at) = *unit;
            }
            *user_chars = 3;
            *domain = 0;
            *domain_chars = 1;
            for (at, unit) in script.packed.iter().enumerate() {
                *password.add(at) = *unit;
            }
            1
        })
    }

    /// A password that begins with `@` is in a protected form.
    unsafe extern "system" fn fake_protected(password: *mut u16, kind: *mut i32) -> Bool {
        *kind = if *password == '@' as u16 { 1 } else { CRED_UNPROTECTED };
        1
    }

    unsafe extern "system" fn fake_unprotect(_as_self: Bool, _protected: *mut u16, _length: u32, clear: *mut u16, chars: *mut u32) -> Bool {
        SCRIPT.with(|script| {
            let mut script = script.borrow_mut();
            script.unprotect_calls += 1;
            if let Some(code) = script.unprotect_fails {
                SetLastError(code);
                return 0;
            }
            if clear.is_null() || (*chars as usize) < script.clear.len() {
                *chars = script.clear.len() as u32;
                SetLastError(ERROR_INSUFFICIENT_BUFFER);
                return 0;
            }
            for (at, unit) in script.clear.iter().enumerate() {
                *clear.add(at) = *unit;
            }
            1
        })
    }

    unsafe extern "system" fn fake_free(_memory: *mut c_void) {}

    unsafe extern "system" fn fake_prompt(_info: *const CredUiInfoW, _error: u32, _package: *mut u32, _in: *const c_void, _in_size: u32,
        _out: *mut *mut c_void, _out_size: *mut u32, _save: *mut Bool, _flags: u32) -> u32 {
        ERROR_CANCELLED
    }

    fn fake_api() -> DesktopApi {
        DesktopApi { prompt: fake_prompt, unpack: fake_unpack, free: fake_free, protected: fake_protected, unprotect: fake_unprotect }
    }

    /// Script the dialog and read the credentials it returns; the wipes of the buffers that were dropped on the way are cleared first.
    fn read_with(script: Script) -> (Asked, Script) {
        SCRIPT.with(|slot| *slot.borrow_mut() = script);
        WIPES.with(|wipes| wipes.borrow_mut().clear());
        let asked = read_credentials(&fake_api(), std::ptr::null(), 0);
        (asked, SCRIPT.with(|slot| std::mem::take(&mut *slot.borrow_mut())))
    }

    fn rejected(asked: Asked) -> String {
        match asked {
            Asked::Rejected(reason) => reason,
            Asked::Entered(_) => "(entered)".into(),
            _ => "(something else)".into(),
        }
    }

    #[test]
    fn a_password_in_a_clear_form_is_taken_as_it_is_and_one_in_a_protected_form_is_unprotected_into_a_buffer_that_is_wiped() {
        // Clear text: no unprotecting.
        let (asked, script) = read_with(Script { packed: units("hunter2"), ..Script::default() });
        let Asked::Entered(credentials) = asked else { panic!("a clear password is taken") };
        assert_eq!(credentials.password_units(), "hunter2".encode_utf16().collect::<Vec<u16>>());
        assert_eq!((script.unpack_calls, script.unprotect_calls), (1, 0));
        drop(credentials);
        // A protected form that fits the buffer of a password: unprotected into one of the same size.
        let (asked, script) = read_with(Script { packed: units(&format!("@{}", "B".repeat(200))), clear: units("hunter2"), ..Script::default() });
        let Asked::Entered(credentials) = asked else { panic!("a protected form that can be unprotected is taken") };
        assert_eq!(credentials.password_units(), "hunter2".encode_utf16().collect::<Vec<u16>>(), "the clear form is what is checked");
        assert!(credentials.password_ends() && credentials.password.0.len() == CREDUI_PASSWORD_CHARS && (script.unpack_calls, script.unprotect_calls) == (1, 2), "{}", script.unprotect_calls);
        drop(credentials);
        let wiped = WIPES.with(|wipes| wipes.borrow().clone());
        assert!(wiped.iter().all(|(_, after)| *after == 0), "every buffer that was alive is zero once dropped: {:?}", wiped);
        assert!(wiped.iter().any(|(before, _)| *before == 201) && wiped.iter().any(|(before, _)| *before == 7), "the protected form and the clear password were each there and were wiped: {:?}", wiped);
    }

    #[test]
    fn a_protected_form_longer_than_a_password_is_read_with_a_larger_buffer_and_no_larger_than_the_limit() {
        // The dialog returns a form of 400 characters: the buffer of a password is too small, and the unpacking is made again with the
        // size the system asked for. The password it holds is then 7 characters, which fit the buffer of a password.
        let (asked, script) = read_with(Script { packed: units(&format!("@{}", "B".repeat(399))), clear: units("hunter2"), ..Script::default() });
        let Asked::Entered(credentials) = asked else { panic!("a long protected form is taken") };
        assert_eq!(credentials.password_units(), "hunter2".encode_utf16().collect::<Vec<u16>>());
        assert_eq!(script.buffers, [CREDUI_PASSWORD_CHARS as u32, 401], "asked with the buffer of a password, then with the size that was asked for");
        assert_eq!((script.unpack_calls, script.unprotect_calls), (2, 2));
        assert_eq!(credentials.password.0.len(), CREDUI_PASSWORD_CHARS, "the password is held in a buffer of the size of a password");
        drop(credentials);
        let wiped = WIPES.with(|wipes| wipes.borrow().clone());
        assert!(wiped.iter().all(|(_, after)| *after == 0) && wiped.iter().any(|(before, _)| *before == 400), "the larger buffer was wiped: {:?}", wiped);
        // At the limit it is taken, over it the dialog's answer is turned away (an attempt used, no logon), after one call.
        let at_limit = Script { packed: units(&format!("@{}", "B".repeat(PROTECTED_LIMIT - 2))), clear: units("x"), ..Script::default() };
        assert!(matches!(read_with(at_limit).0, Asked::Entered(_)), "the limit itself is taken");
        let (asked, script) = read_with(Script { packed: units(&format!("@{}", "B".repeat(PROTECTED_LIMIT))), clear: units("x"), ..Script::default() });
        assert_eq!(rejected(asked), "the password is longer than the guard takes");
        assert_eq!((script.unpack_calls, script.unprotect_calls), (1, 0));
        assert!(WIPES.with(|wipes| wipes.borrow().iter().all(|(_, after)| *after == 0)));
    }

    #[test]
    fn a_password_that_cannot_be_unprotected_or_read_uses_an_attempt_and_leaves_nothing_in_memory() {
        // The system fails to unprotect.
        let (asked, script) = read_with(Script { packed: units("@protected"), clear: units("hunter2"), unprotect_fails: Some(5), ..Script::default() });
        let reason = rejected(asked);
        assert!(reason.starts_with("the protected form of the password could not be unprotected: ") && reason.contains("[WinError 5]"), "{}", reason);
        assert_eq!(script.unprotect_calls, 1);
        assert!(WIPES.with(|wipes| wipes.borrow().iter().all(|(_, after)| *after == 0)), "the protected form was wiped");
        // The clear password would be longer than a password can be.
        let (asked, script) = read_with(Script { packed: units("@protected"), clear: units(&"p".repeat(CREDUI_PASSWORD_CHARS)), ..Script::default() });
        assert_eq!(rejected(asked), "the password is longer than the guard takes");
        assert_eq!(script.unprotect_calls, 1, "the clear text is not asked for");
        // The size is asked for and the system fills nothing: the first call does not fail with the error for the size.
        let (asked, _) = read_with(Script { packed: units("@protected"), clear: units("x"), unprotect_fails: Some(ERROR_NONE_MAPPED), ..Script::default() });
        assert!(rejected(asked).contains("could not be unprotected"));
        // The dialog's answer cannot be read at all, or the name is too long: as before.
        let (asked, _) = read_with(Script { unpack_fails: Some(1168), ..Script::default() });
        assert!(rejected(asked).starts_with("the answer of the prompt could not be read as a user name and password: "));
        let (asked, script) = read_with(Script { unpack_fails: Some(ERROR_INSUFFICIENT_BUFFER), ..Script::default() });
        assert_eq!(rejected(asked), "the name or password is longer than the prompt allows");
        assert_eq!(script.unpack_calls, 1, "a size that did not grow is not asked again");
        assert!(WIPES.with(|wipes| wipes.borrow().iter().all(|(_, after)| *after == 0)));
    }

    #[test]
    fn a_password_protected_by_the_system_is_unprotected_by_the_system() {
        // The real calls: a password protected for the test with `CredProtectW` and given back by `CredUnprotectW`. Neither shows anything.
        let api = desktop_api().expect("the libraries of the desktop prompt");
        let clear: Vec<u16> = "hunter2".encode_utf16().chain(std::iter::once(0)).collect();
        let (mut size, mut kind) = (0u32, 0i32);
        unsafe { CredProtectW(0, clear.as_ptr(), clear.len() as u32, std::ptr::null_mut(), &mut size, &mut kind) };
        let mut blob = vec![0u16; size as usize];
        assert_ne!(unsafe { CredProtectW(0, clear.as_ptr(), clear.len() as u32, blob.as_mut_ptr(), &mut size, &mut kind) }, 0, "protect a password for the test");
        assert!(password_is_protected(api, &mut blob));
        match unprotect(api, &mut blob) {
            Ok(back) => assert_eq!(text_before_nul(&back.0).as_deref(), Some("hunter2")),
            Err(reason) => panic!("{}", reason),
        }
        wipe(&mut blob);
    }

    // ---- the calls of the Windows Runtime, for real, where they show nothing

    #[test]
    fn the_class_and_the_interfaces_that_the_request_uses_are_there_and_whether_windows_hello_is_available_is_answered_without_showing_anything() {
        let api = hello_api().expect("combase.dll");
        let _apartment = Apartment::enter(api).expect("a Windows Runtime apartment");
        let class = HString::new(api, CONSENT_CLASS).expect("a string of the Windows Runtime");
        // The two interfaces of the class that are used: the code to ask for the request is slot 6 of the second, and a request is never made here.
        match factory(api, &class, &IID_CONSENT_INTEROP) {
            Ok(interop) => assert!(interop.query(&IID_CONSENT_INTEROP).is_ok() && !interop.0.is_null()),
            // A system without the class (0x80040154) or without the interop interface (an edition of Windows older than 1703 or a server edition).
            Err(code) if code == 0x8004_0154 || code == 0x8000_4002 => eprintln!("NOT ASSERTED: the class or the interop interface of Windows Hello is not there (0x{:08x})", code),
            Err(code) => panic!("the interop interface of the class: 0x{:08x}", code),
        }
        if let Ok(statics) = factory(api, &class, &IID_CONSENT_STATICS) {
            assert!(!statics.0.is_null());
        }
        // The question: it shows nothing, and the answer is one of the names that are logged.
        match HelloConsenter.availability(&Owners::empty()) {
            Ok(answer) => {
                let name = answer.name();
                assert!(["Available", "DeviceNotPresent", "NotConfiguredForUser", "DisabledByPolicy", "DeviceBusy"].contains(&name.as_str()) || name.starts_with("Unknown("), "{}", name);
                eprintln!("Windows Hello on this machine: {}", name);
            }
            Err(reason) => eprintln!("NOT ASSERTED: the question was not answered here: {}", reason),
        }
        // A starter that has gone ends the question early, with its reason, unless the system answered first; either way it returns
        // within the limit of the question, and an answer is one of the names that are logged.
        let gone = Owners::of(vec![event(true)]);
        let started = Instant::now();
        match HelloConsenter.availability(&gone) {
            Ok(answer) => assert!(["Available", "DeviceNotPresent", "NotConfiguredForUser", "DisabledByPolicy", "DeviceBusy"].contains(&answer.name().as_str()) || answer.name().starts_with("Unknown(")),
            Err(reason) => assert!(!reason.is_empty(), "a question that is not answered says why"),
        }
        assert!(started.elapsed() < Duration::from_millis(SYSTEM_TIMEOUT_MS + 2_000), "{:?}", started.elapsed());
    }

    #[link(name = "user32")]
    extern "system" {
        fn CreateDesktopW(name: *const u16, device: *const u16, devmode: *mut c_void, flags: u32, access: u32, attributes: *mut c_void) -> Handle;
        fn SetThreadDesktop(desktop: Handle) -> Bool;
        fn PostMessageW(window: Handle, message: u32, w: usize, l: isize) -> Bool;
        fn PostThreadMessageW(thread: u32, message: u32, w: usize, l: isize) -> Bool;
    }

    /// Set by the function that a timer message posted to the owner window names, if it is ever called.
    static TIMER_FUNCTION_CALLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn timer_function(_window: Handle, _message: u32, _id: usize, _time: u32) {
        TIMER_FUNCTION_CALLED.store(true, Ordering::SeqCst);
    }

    #[test]
    fn a_question_about_windows_hello_that_is_cancelled_ends_cancelled_through_the_cancel_slot_of_the_system() {
        // The question shows nothing, and it is the one real operation of the Windows Runtime that can be cancelled here: the request is
        // never made. Cancel is slot 9 of `IAsyncInfo`; only a system that lays the table out as the guard declares ends the operation
        // as cancelled, which the stand-in operations cannot show. The loop that waits for an operation cancels it when its flag is set.
        let Some(api) = hello_api() else { return eprintln!("NOT ASSERTED: combase.dll is not there") };
        let (mut cancelled, mut asked) = (0, 0);
        for _ in 0..20 {
            let Ok(_apartment) = Apartment::enter(api) else { return eprintln!("NOT ASSERTED: no apartment of the Windows Runtime") };
            let Ok(class) = HString::new(api, CONSENT_CLASS) else { return eprintln!("NOT ASSERTED: no string of the Windows Runtime") };
            let Ok(statics) = factory(api, &class, &IID_CONSENT_STATICS) else { return eprintln!("NOT ASSERTED: the class of Windows Hello is not there") };
            let mut raw: *mut c_void = std::ptr::null_mut();
            // SAFETY: slot 6 of IUserConsentVerifierStatics is CheckAvailabilityAsync.
            let code = unsafe { slot::<CheckAvailabilityAsyncFn>(statics.0, 6)(statics.0, &mut raw) };
            assert!(code >= 0 && !raw.is_null(), "the question is asked: 0x{:08x}", code);
            let typed = Com(raw).query(&IID_AVAILABILITY_OPERATION).expect("the operation as the interface whose result is read");
            let operation = WinOperation::new(typed).expect("an operation that has IAsyncInfo");
            asked += 1;
            match drive(&operation, &AtomicBool::new(true), None, Duration::from_secs(5), &one_millisecond) {
                Outcome::TimedOut => cancelled += 1,
                // The answer came before the loop looked at its flag: an answer is an answer.
                Outcome::Done(code) => assert!(!matches!(Availability::from_code(code), Availability::Unknown(_)), "an availability that is known: {}", code),
                other => panic!("a question that is cancelled ends as cancelled or answered, not {:?}", other),
            }
        }
        assert!(cancelled > 0, "no question of {} ended as cancelled: Cancel (slot 9) does not reach the operation", asked);
    }

    #[test]
    fn only_a_message_the_window_can_have_asked_for_is_dispatched() {
        let (own, none) = (0x1000usize as Handle, std::ptr::null_mut());
        let message = |window: Handle, message: u32, l: isize| Msg { window, message, w: 1, l, time: 0, point: [0; 2] };
        // A timer message is never dispatched, for the window or for the thread, with a function named or none: the thread sets no timer.
        for kind in [WM_TIMER, WM_SYSTIMER] {
            for window in [own, none] {
                assert!(!dispatchable(&message(window, kind, timer_function as *const () as usize as isize)), "{}: the function it names is never called", kind);
                assert!(!dispatchable(&message(window, kind, 0)), "{}: and nor is one that names none", kind);
            }
        }
        // Everything else is.
        assert!(dispatchable(&message(own, 0, 0)) && dispatchable(&message(own, 0x0010, 0)) && dispatchable(&message(none, 0x0400, 0)), "WM_NULL, WM_CLOSE and a message of the thread");
    }

    thread_local! {
        /// What the stand-in for the wait of the owner window gives.
        static WAIT_GIVES: std::cell::Cell<u32> = const { std::cell::Cell::new(WAIT_OBJECT_0) };
    }

    unsafe extern "system" fn fake_wait(_count: u32, _handles: *const Handle, _milliseconds: u32, _mask: u32, _flags: u32) -> u32 {
        WAIT_GIVES.with(|gives| gives.get())
    }

    unsafe extern "system" fn fake_register(_class: *const WndClassExW) -> u16 {
        1
    }

    unsafe extern "system" fn fake_create(_ex: u32, _class: *const u16, _name: *const u16, _style: u32, _x: i32, _y: i32, _width: i32, _height: i32,
        _parent: Handle, _menu: Handle, _instance: Handle, _parameter: *mut c_void) -> Handle {
        std::ptr::null_mut()
    }

    unsafe extern "system" fn fake_destroy(_window: Handle) -> Bool {
        1
    }

    unsafe extern "system" fn fake_unregister(_class: *const u16, _instance: Handle) -> Bool {
        1
    }

    unsafe extern "system" fn fake_peek(_message: *mut Msg, _window: Handle, _first: u32, _last: u32, _flags: u32) -> Bool {
        0
    }

    unsafe extern "system" fn fake_dispatch(_message: *const Msg) -> isize {
        0
    }

    unsafe extern "system" fn fake_procedure(_window: Handle, _message: u32, _w: usize, _l: isize) -> isize {
        0
    }

    #[test]
    fn a_wait_for_the_owner_window_that_fails_sleeps_the_time_and_does_not_spin() {
        let api: &'static WindowApi = Box::leak(Box::new(WindowApi {
            register: fake_register, create: fake_create, destroy: fake_destroy, unregister: fake_unregister, peek: fake_peek,
            dispatch: fake_dispatch, default_procedure: fake_procedure, wait: fake_wait,
        }));
        let window = OwnerWindow { api, handle: std::ptr::null_mut(), class: wide("none"), instance: std::ptr::null_mut() };
        // A wait that could not be made returns at once, which the loop that waits for the request would turn into a spin: the time is slept.
        WAIT_GIVES.with(|gives| gives.set(WAIT_FAILED));
        let started = Instant::now();
        window.serve();
        assert!(started.elapsed() >= Duration::from_millis(POLL_MS as u64 - 20), "{:?}", started.elapsed());
        // A wait that ended because of a message, or because the time was up, is not slept again.
        for gives in [WAIT_OBJECT_0, WAIT_TIMEOUT] {
            WAIT_GIVES.with(|slot| slot.set(gives));
            window.serve();
        }
    }

    /// Set in the environment of the copy of this test executable that makes the owner window.
    const OWNER_CHILD: &str = "PROCESS_TREE_GUARD_UNIT_TEST_OWNER_WINDOW_CHILD";

    #[test]
    fn the_window_that_owns_the_request_is_made_served_and_destroyed_on_a_window_station_that_is_not_visible() {
        const GENERIC_ALL: u32 = 0x1000_0000;
        if std::env::var_os(OWNER_CHILD).is_some() {
            // The copy: it makes a window station of its own, which is not visible, and a desktop on it, makes them its own and makes the
            // window on that desktop, so that no window is ever on the desktop of whoever runs the tests. It is served once and
            // destroyed, twice, to see that the class can be made again.
            let station = unsafe { CreateWindowStationW(std::ptr::null(), 0, GENERIC_ALL, std::ptr::null_mut()) };
            if station.is_null() || unsafe { SetProcessWindowStation(station) } == 0 {
                println!("OWNER no window station could be made [WinError {}]", unsafe { GetLastError() });
                return;
            }
            let desktop = unsafe { CreateDesktopW(wide("hidden").as_ptr(), std::ptr::null(), std::ptr::null_mut(), 0, GENERIC_ALL, std::ptr::null_mut()) };
            if desktop.is_null() || unsafe { SetThreadDesktop(desktop) } == 0 {
                println!("OWNER no desktop could be made [WinError {}]", unsafe { GetLastError() });
                return;
            }
            // The real read of the two desktops from a desktop that does not receive the input of anyone: it is not accepted.
            let (ours, input) = desktop_names();
            println!("OWNER desktops ours={} input={:?} accepted={}", ours.as_deref().unwrap_or("(none)"), input, on_input_desktop(ours.clone(), input.clone()).is_ok());
            let api = window_api().expect("user32.dll");
            for round in 1..=2 {
                match OwnerWindow::create(api) {
                    Ok(window) => {
                        window.serve();
                        println!("OWNER round {} made and served", round);
                        // With nothing in its queue the window waits for the poll and does not return at once, which would turn the loop that
                        // waits for the request into a spin that reads the operation on every turn (CWE-834).
                        for pass in 0..4 {
                            let before = Instant::now();
                            window.serve();
                            println!("OWNER round {} idle serve {} {}", round, pass, before.elapsed().as_millis());
                        }
                        // One look at the request serves what waits for the window: the loop that waits for the request serves the window nowhere
                        // else, so a few posted messages are gone after it.
                        let mut seen = Msg { window: std::ptr::null_mut(), message: 0, w: 0, l: 0, time: 0, point: [0; 2] };
                        unsafe {
                            for _ in 0..3 {
                                PostMessageW(window.handle, 0, 0, 0);
                            }
                        }
                        window.serve();
                        println!("OWNER round {} serve drains {}", round, unsafe { (window.api.peek)(&mut seen, std::ptr::null_mut(), 0, 0, 0) } == 0);
                        // A program of the same user can post to the window and to its thread: a timer message that names a function
                        // to each, then more messages than one look at the queue takes. One look serves no more than PUMP_LIMIT of them and calls no
                        // function, and a few looks empty the queue.
                        let mut probe = Msg { window: std::ptr::null_mut(), message: 0, w: 0, l: 0, time: 0, point: [0; 2] };
                        unsafe {
                            PostMessageW(window.handle, WM_TIMER, 1, timer_function as *const () as usize as isize);
                            PostThreadMessageW(GetCurrentThreadId(), WM_TIMER, 2, timer_function as *const () as usize as isize);
                            for _ in 0..PUMP_LIMIT * 2 {
                                PostMessageW(window.handle, 0, 0, 0);
                            }
                        }
                        window.pump();
                        let left = unsafe { (window.api.peek)(&mut probe, std::ptr::null_mut(), 0, 0, 0) } != 0;
                        for _ in 0..4 {
                            window.pump();
                        }
                        let drained = unsafe { (window.api.peek)(&mut probe, std::ptr::null_mut(), 0, 0, 0) } == 0;
                        // A pass of `serve`, which is what a look at the request allows, dispatches no more than that either.
                        unsafe {
                            for _ in 0..PUMP_LIMIT * 2 {
                                PostMessageW(window.handle, 0, 0, 0);
                            }
                        }
                        window.serve();
                        let left_by_serve = unsafe { (window.api.peek)(&mut probe, std::ptr::null_mut(), 0, 0, 0) } != 0;
                        for _ in 0..4 {
                            window.pump();
                        }
                        println!("OWNER round {} flood left_after_one_look={} drained={} left_after_serve={} timer_function_called={}", round, left, drained, left_by_serve,
                            TIMER_FUNCTION_CALLED.load(Ordering::SeqCst));
                        // What the system itself does with such a message, dispatched without the filter: recorded and not asserted, because
                        // the guard does not rely on it and it may differ between systems.
                        TIMER_FUNCTION_CALLED.store(false, Ordering::SeqCst);
                        unsafe { PostMessageW(window.handle, WM_TIMER, 3, timer_function as *const () as usize as isize) };
                        while unsafe { (window.api.peek)(&mut probe, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
                            if probe.message == WM_TIMER {
                                unsafe { (window.api.dispatch)(&probe) };
                            }
                        }
                        println!("OWNER round {} the system calls the function of a timer message from outside: {}", round, TIMER_FUNCTION_CALLED.load(Ordering::SeqCst));
                        TIMER_FUNCTION_CALLED.store(false, Ordering::SeqCst);
                    }
                    Err(code) => {
                        println!("OWNER round {} failed 0x{:08x}", round, code);
                        return;
                    }
                }
            }
            println!("OWNER ok");
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().expect("the path of this test executable"))
            .args(["the_window_that_owns_the_request_is_made_served_and_destroyed", "--nocapture", "--test-threads=1"])
            .env(OWNER_CHILD, "1").stdin(std::process::Stdio::null()).output().expect("run the copy");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let said: Vec<String> = stdout.lines().filter_map(|line| line.find("OWNER ").map(|at| line[at + "OWNER ".len()..].to_string())).collect();
        if said.iter().any(|line| line.starts_with("no window station") || line.starts_with("no desktop")) {
            assert!(std::env::var_os("PROCESS_TREE_GUARD_REQUIRE_WINDOW_STATION").map_or(true, |value| value.is_empty()), "{:?}", said);
            eprintln!("NOT ASSERTED: {:?}", said);
            return;
        }
        assert!(output.status.success() && said.last().map(String::as_str) == Some("ok"), "{:?} / {}", said, stdout);
        assert!(said.iter().any(|line| line.starts_with("desktops ours=hidden ") && line.ends_with(" accepted=false")),
            "a desktop of a window station that is not visible is not the one that receives the input of the user: {:?}", said);
        for round in 1..=2 {
            assert!(said.iter().any(|line| *line == format!("round {} flood left_after_one_look=true drained=true left_after_serve=true timer_function_called=false", round)),
                "round {}: a flood is served in parts and a timer message calls nothing: {:?}", round, said);
            assert!(said.iter().any(|line| *line == format!("round {} serve drains true", round)), "round {}: one look at the request serves the messages that wait: {:?}", round, said);
            // A message that the system sends to a window can end one wait early, so three of the four idle waits must last about the poll.
            let prefix = format!("round {} idle serve ", round);
            let waits: Vec<u128> = said.iter().filter_map(|line| line.strip_prefix(prefix.as_str())).filter_map(|rest| rest.split_whitespace().nth(1)?.parse().ok()).collect();
            assert!(waits.len() == 4 && waits.iter().filter(|ms| **ms >= POLL_MS as u128 / 2).count() >= 3 && waits.iter().all(|ms| *ms < 5_000),
                "round {}: a window with nothing in its queue waits for the poll and does not spin: {:?}", round, waits);
        }
        for line in said.iter().filter(|line| line.contains("the system calls the function")) {
            eprintln!("Recorded, not asserted: {}", line);
        }
    }
