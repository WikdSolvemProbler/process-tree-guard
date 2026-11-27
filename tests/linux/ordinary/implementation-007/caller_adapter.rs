// SPDX-License-Identifier: Apache-2.0
//! Private candidate adapter for the ordinary native caller, with no default dispatch.

use super::{OriginalInvocation, OwnerMode};
use std::ffi::{CString, OsStr, OsString, c_char, c_int};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::ptr;

#[derive(Debug)]
pub(super) enum OrdinaryOutcome {
    Exited(i32),
    Signaled(i32),
    TimedOut,
    OwnerEnded,
    ExecRefused(i32),
    Unconfirmed(i32),
}

#[derive(Debug)]
pub(super) struct OrdinaryRun {
    pub(super) outcome: OrdinaryOutcome,
    pub(super) evidence: Option<OwnedFd>,
}

#[repr(C)]
#[derive(Default)]
struct WaitFrame {
    called: u32,
    result: i32,
    call_errno: i32,
    signo: i32,
    signal_errno: i32,
    code: i32,
    pid: i32,
    uid: u32,
    status: i32,
    native_size: u32,
}

#[repr(C)]
#[derive(Default)]
struct NativeResult {
    outcome: c_int,
    value: c_int,
    guard_errno: c_int,
    worker_wait: WaitFrame,
    evidence_fd: c_int,
}

const _: () = assert!(std::mem::size_of::<NativeResult>() == 56);
const _: () = assert!(crate::deadline::MAX_SECONDS == 86_400.0);

#[repr(C)]
struct OwnerIdentity {
    pid: u32,
    ppid: u32,
    start_ticks: u64,
}

unsafe extern "C" {
    fn ptg220_run(
        path: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
        deadline_ns: u64,
        owners: *const c_int,
        count: u32,
        originals: *const OwnerIdentity,
        owner_mode: u32,
        result: *mut NativeResult,
    ) -> c_int;
}

pub(super) fn run_ordinary(
    resolved_program: &OsStr,
    argv: &[OsString],
    seconds: f64,
    mode: OwnerMode,
) -> Result<OrdinaryRun, String> {
    let invocation = OriginalInvocation::capture(mode, seconds).map_err(|e| format!("{e:?}"))?;
    let path = CString::new(resolved_program.as_bytes()).map_err(|e| e.to_string())?;
    let args = argv.iter()
        .map(|arg| CString::new(arg.as_os_str().as_bytes()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if args.is_empty() {
        return Err("ordinary Linux requires argv[0]".into());
    }
    let mut pointers: Vec<_> = args.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(ptr::null());
    let owners: Vec<_> = invocation.pidfds().iter().map(AsRawFd::as_raw_fd).collect();
    let identities: Vec<_> = invocation.metadata().identities.iter().map(|entry| OwnerIdentity {
        pid: entry.pid, ppid: entry.ppid, start_ticks: entry.start_ticks,
    }).collect();
    let count = u32::try_from(owners.len()).map_err(|e| e.to_string())?;
    invocation.revalidate().map_err(|e| format!("{e:?}"))?;
    let mut native = NativeResult { evidence_fd: -1, ..NativeResult::default() };
    // The immutable buffers and original OwnedFd values outlive the synchronous call.
    let code = unsafe {
        ptg220_run(path.as_ptr(), pointers.as_ptr(), ptr::null(),
                   invocation.metadata().deadline_ns, owners.as_ptr(), count, identities.as_ptr(),
                   u32::from(mode == OwnerMode::Auto), &mut native)
    };
    let evidence = if native.evidence_fd < 0 {
        None
    } else {
        // The C caller returns a new, sealed CLOEXEC descriptor owned by this result.
        Some(unsafe { OwnedFd::from_raw_fd(native.evidence_fd) })
    };
    let outcome = if code != 0 {
        OrdinaryOutcome::Unconfirmed(native.guard_errno)
    } else {
        match native.outcome {
            1 => OrdinaryOutcome::Exited(native.value),
            2 => OrdinaryOutcome::Signaled(native.value),
            3 => OrdinaryOutcome::TimedOut,
            4 => OrdinaryOutcome::OwnerEnded,
            5 => OrdinaryOutcome::ExecRefused(native.value),
            _ => return Err("ordinary Linux returned an unknown terminal outcome".into()),
        }
    };
    Ok(OrdinaryRun { outcome, evidence })
}
