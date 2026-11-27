// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Opt-in ordinary Linux component for an enrolled, cooperative caller.

use crate::GuardError;
use std::ffi::{c_int, OsString};
use std::path::Path;

#[path = "invocation_native.rs"]
mod invocation_native;

mod invocation {
    use super::invocation_native as native;
    use std::io::Read;

    fn calling_umask() -> Result<u16, String> {
        // The native ordinary path inherits the calling thread's fs context;
        // the Rust umask sample is carried only in shared invocation metadata.
        let status = std::fs::File::open("/proc/thread-self/status").map_err(|error| error.to_string())?;
        let mut text = String::new();
        status.take(65537).read_to_string(&mut text).map_err(|error| error.to_string())?;
        if text.len() > 65536 {
            return Err("calling-thread status exceeds its size bound".into());
        }
        let mut fields = text.lines().filter_map(|line| line.strip_prefix("Umask:"));
        let octal = fields.next().ok_or("missing calling-thread umask")?.trim();
        if fields.next().is_some() || octal.len() != 4 || !octal.bytes().all(|byte| matches!(byte, b'0'..=b'7')) {
            return Err("invalid calling-thread umask".into());
        }
        u16::from_str_radix(octal, 8).map_err(|error| error.to_string())
    }

    include!("invocation_capture.rs");
}

use invocation::{OriginalInvocation, OwnerMode};

#[path = "../../../tests/linux/ordinary/implementation-007/caller_adapter.rs"]
mod caller_adapter;

#[link(name = "ptg_ordinary_caller", kind = "static")]
unsafe extern "C" {}

/// The ordinary component's observed native result.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The original command returned this native exit status.
    Exited(i32),
    /// The original command ended because of this native signal.
    Signaled(i32),
    /// The component selected and verified deadline settlement.
    TimedOut,
    /// The component selected and verified settlement after owner loss.
    OwnerEnded,
    /// Native execution of the original command was rejected with this error.
    ExecRefused(i32),
    /// The component could not establish a complete native outcome.
    Unconfirmed(i32),
}

/// Run one resolved Unix command under a finite Manual deadline. The original
/// owner is the calling process's parent. The caller supplies the absolute
/// executable path and the original argument vector, including its argv[0].
///
/// This experimental component requires an administratively enrolled Linux
/// caller and independently packaged native helper. It inherits the caller's
/// ordinary execution context and does not configure the host. The native
/// component refuses an ineligible caller or environment before payload launch.
///
/// # Safety
///
/// The calling process must remain single-threaded, already be a child
/// subreaper, and retain exclusive custody of its child waits. It must have
/// default SIGCHLD handling, start without existing children, and have no
/// signal handler or other callback that creates, reaps, or transfers child
/// custody while this call owns it. The caller must retain the enrolled policy
/// and descriptor context until the call returns.
pub unsafe fn run_manual(program: &Path, argv: &[OsString], seconds: f64) -> Result<Outcome, GuardError> {
    if !program.is_absolute() {
        return Err(GuardError("ordinary Linux requires an absolute executable path".into()));
    }
    let run = caller_adapter::run_ordinary(program.as_os_str(), argv, seconds, OwnerMode::Manual)
        .map_err(GuardError)?;
    drop(run.evidence);
    use caller_adapter::OrdinaryOutcome as Native;
    Ok(match run.outcome {
        Native::Exited(status) => Outcome::Exited(status),
        Native::Signaled(signal) => Outcome::Signaled(signal),
        Native::TimedOut => Outcome::TimedOut,
        Native::OwnerEnded => Outcome::OwnerEnded,
        Native::ExecRefused(error) => Outcome::ExecRefused(error),
        Native::Unconfirmed(error) => Outcome::Unconfirmed(error),
    })
}

// Match the LP64 native ABI checked by the C and Rust adapter sources.
const _: () = assert!(std::mem::size_of::<c_int>() == 4);
