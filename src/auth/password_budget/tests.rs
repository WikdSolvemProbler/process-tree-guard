// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Exercise the native retry store from separate test processes.

use super::{reserve_in, wide, Key, BUDGET_BUSY, BUDGET_FULL, KEY_PATH, STORAGE_UNAVAILABLE};
use crate::extend::current_user_sid;
use crate::sys::windows::{
    AssignProcessToJobObject, CreateJobObjectW, ExtendedLimitInformation, GetStdHandle, Handle,
    PeekNamedPipe, SetInformationJobObject, TerminateJobObject, WaitForSingleObject,
    EXTENDED_LIMIT_CLASS, KILL_ON_JOB_CLOSE, STD_INPUT_HANDLE, WAIT_OBJECT_0,
};
use std::io::{self, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TEST_NONCE: &str = "PTG_TEST_PASSWORD_BUDGET_NONCE";
const TEST_ACTION: &str = "PTG_TEST_PASSWORD_BUDGET_ACTION";
const READY: &str = "PTG_BUDGET_RESERVATION_READY";
const DONE: &str = "PTG_BUDGET_TEST_DONE";
const TIMEOUT: Duration = Duration::from_secs(15);
const NOW_MS: u64 = 1_700_000_000_000;

use crate::sys::windows::test_api::RegDeleteKeyW;

fn test_paths(nonce: &str) -> (String, String) {
    (
        format!(r"Global\ProcessTreeGuardBudgetTest-{nonce}"),
        format!("{KEY_PATH}-Test-{nonce}"),
    )
}

struct Fixture(String);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let instant = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        Self(format!(
            "{}-{instant}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn spawn(&self, action: &str) -> Worker {
        let (job, mut limits) = (
            unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) },
            ExtendedLimitInformation::default(),
        );
        assert!(
            !job.is_null(),
            "create test job: {}",
            io::Error::last_os_error()
        );
        let job = unsafe { OwnedHandle::from_raw_handle(job) };
        limits.basic.limit_flags = KILL_ON_JOB_CLOSE;
        assert_ne!(
            unsafe {
                SetInformationJobObject(
                    job.as_raw_handle(),
                    EXTENDED_LIMIT_CLASS,
                    (&mut limits as *mut ExtendedLimitInformation).cast(),
                    std::mem::size_of::<ExtendedLimitInformation>() as u32,
                )
            },
            0,
            "set test job cleanup: {}",
            io::Error::last_os_error()
        );

        let module = module_path!().split_once("::").expect("test module path").1;
        let test = format!("{module}::budget_process_worker");
        let child = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", &test, "--nocapture", "--test-threads=1"])
            .env(TEST_NONCE, &self.0)
            .env(TEST_ACTION, action)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn budget worker");
        let mut worker = Worker {
            child,
            job: Some(job),
            output: String::new(),
        };
        let job = worker.job.as_ref().expect("test job");
        assert_ne!(
            unsafe { AssignProcessToJobObject(job.as_raw_handle(), worker.child.as_raw_handle()) },
            0,
            "assign budget worker before releasing it: {}",
            io::Error::last_os_error()
        );
        worker
            .child
            .stdin
            .take()
            .expect("worker launch gate")
            .write_all(b"1")
            .expect("release budget worker");
        worker
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let (_, path) = test_paths(&self.0);
        let path = wide(&path);
        let status = current_user_sid()
            .map_err(|_| STORAGE_UNAVAILABLE)
            .and_then(|sid| Key::open_user_hive(&sid))
            .map(|hive| unsafe { RegDeleteKeyW(hive.0, path.as_ptr()) });
        if !std::thread::panicking() {
            assert!(
                matches!(status, Ok(0 | 2)),
                "delete isolated retry test state: {status:?}"
            );
        }
    }
}

fn available(pipe: Handle) -> io::Result<usize> {
    let mut count = 0;
    let result = unsafe {
        PeekNamedPipe(
            pipe,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut count,
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(count as usize)
    }
}

struct Worker {
    child: Child,
    job: Option<OwnedHandle>,
    output: String,
}

impl Worker {
    fn drain_output(&mut self) {
        let stdout = self.child.stdout.as_mut().expect("worker output");
        let mut buffer = [0u8; 1024];
        loop {
            let count = match available(stdout.as_raw_handle()) {
                Ok(0) => return,
                Ok(count) => count.min(buffer.len()),
                Err(error) if error.raw_os_error() == Some(109) => return, // Closed pipe.
                Err(error) => panic!("inspect budget worker output: {error}"),
            };
            let got = stdout
                .read(&mut buffer[..count])
                .expect("read available worker output");
            if got == 0 {
                return;
            }
            self.output
                .push_str(&String::from_utf8_lossy(&buffer[..got]));
            assert!(
                self.output.len() <= 16_384,
                "unexpected budget worker output"
            );
        }
    }

    fn wait_ready(&mut self) {
        let until = Instant::now() + TIMEOUT;
        loop {
            self.drain_output();
            if self.output.contains(READY) {
                return;
            }
            assert!(
                self.child.try_wait().expect("poll budget worker").is_none()
                    && Instant::now() < until,
                "budget worker did not reserve its slot: {}",
                self.output
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn reap(&mut self) -> ExitStatus {
        let status =
            unsafe { WaitForSingleObject(self.child.as_raw_handle(), TIMEOUT.as_millis() as u32) };
        assert_eq!(
            status, WAIT_OBJECT_0,
            "budget worker did not exit within its deadline"
        );
        self.child.wait().expect("reap budget worker")
    }

    fn expect_success(mut self) {
        let status = self.reap();
        self.drain_output();
        assert!(
            status.success() && self.output.contains(DONE),
            "budget worker did not run successfully: {status}\n{}",
            self.output
        );
    }

    fn crash(mut self) {
        const FORCED_EXIT: u32 = 73;
        let job = self.job.as_ref().expect("test job");
        assert_ne!(
            unsafe { TerminateJobObject(job.as_raw_handle(), FORCED_EXIT) },
            0,
            "terminate reservation holder: {}",
            io::Error::last_os_error()
        );
        assert_eq!(self.reap().code(), Some(FORCED_EXIT as i32));
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Closing the job terminates the tree. The direct kill also handles
        // assignment failure, when the process was still at its launch gate.
        drop(self.job.take());
        let _ = self.child.kill();
        if unsafe { WaitForSingleObject(self.child.as_raw_handle(), TIMEOUT.as_millis() as u32) }
            == WAIT_OBJECT_0
        {
            let _ = self.child.wait();
        }
    }
}

#[test]
fn budget_process_worker() {
    let Ok(nonce) = std::env::var(TEST_NONCE) else {
        return;
    };
    let fields: Vec<_> = nonce.split('-').collect();
    assert!(
        fields.len() == 3
            && fields
                .iter()
                .all(|field| !field.is_empty() && field.bytes().all(|b| b.is_ascii_digit())),
        "worker only accepts an isolated test nonce"
    );
    let stdin = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    let until = Instant::now() + TIMEOUT;
    while available(stdin).expect("worker launch gate is a pipe") == 0 {
        assert!(Instant::now() < until, "worker launch gate timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut start = [0];
    io::stdin()
        .read_exact(&mut start)
        .expect("read worker launch gate");
    assert_eq!(start, *b"1");

    let (mutex, path) = test_paths(&nonce);
    let reserve = || reserve_in(&mutex, &path, NOW_MS);
    match std::env::var(TEST_ACTION)
        .expect("test worker action")
        .as_str()
    {
        "spend" => drop(reserve().expect("spend a shared slot")),
        "refund" => reserve()
            .expect("reserve for a simulated success")
            .refund()
            .expect("refund that slot"),
        "busy" => assert_eq!(reserve().err(), Some(BUDGET_BUSY)),
        "full" => assert_eq!(reserve().err(), Some(BUDGET_FULL)),
        "hold" => {
            let _attempt = reserve().expect("reserve before simulated crash");
            println!("{READY}");
            io::stdout().flush().expect("announce reservation");
            std::thread::sleep(Duration::from_secs(60));
            panic!("test parent was expected to terminate the reservation holder");
        }
        _ => panic!("unknown retry test worker action"),
    }
    println!("{DONE}");
}

#[test]
fn separate_launches_exhaust_the_same_retry_budget() {
    let fixture = Fixture::new();
    for _ in 0..3 {
        fixture.spawn("spend").expect_success();
    }
    fixture.spawn("full").expect_success();
}

#[test]
fn a_success_refunds_only_its_slot_across_processes() {
    let fixture = Fixture::new();
    for _ in 0..2 {
        fixture.spawn("spend").expect_success();
    }
    fixture.spawn("refund").expect_success();
    fixture.spawn("spend").expect_success();
    fixture.spawn("full").expect_success();
}

#[test]
fn concurrent_launch_is_refused_and_crash_keeps_the_reserved_slot() {
    let fixture = Fixture::new();
    let mut holder = fixture.spawn("hold");
    holder.wait_ready();
    fixture.spawn("busy").expect_success();
    assert!(holder
        .child
        .try_wait()
        .expect("poll reservation holder")
        .is_none());
    holder.crash();

    for _ in 0..2 {
        fixture.spawn("spend").expect_success();
    }
    fixture.spawn("full").expect_success();
}
