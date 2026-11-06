// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Runs one command under a finite deadline and ends the whole
//! process tree at the deadline.
//! The proven backend at this version is Windows. No other backend
//! is proven at this version.
//!
//! A body of code depends on the platform at exactly the places
//! where it uses a platform mechanism.
//! A platform mechanism is a capability that only one operating
//! system provides, stated in that system's own terms.
//!
//! The shape has one backend module: `sys/windows.rs`.
//! Reach is judged file by file: a file reaches a platform
//! mechanism when its code uses a declaration, or uses a thing
//! that itself uses a declaration in one step.
//! The pure set exists and is pinned by name.
//!
//! Anchor A1 is the professor test: a professor, shown the shape
//! alone, could call the shape perfect.
//! Anchor A2 is universality, answered by refusal: the guard does
//! not port to a system that cannot keep its promise, and the seam
//! is drawn so that the refusal can be proved file by file.
//!
//! The pure set is: `src/auth/hello/text.rs`,
//! `src/auth/message.rs`, `src/base64.rs`, `src/cli/help.rs`,
//! `src/engine/resolve.rs`,
//! `src/engine/trace.rs`, `src/extend/codec.rs`,
//! `src/extend/log/audit.rs`, `src/extend/log/format.rs`,
//! `src/hook/json.rs`, `src/hook/scanner.rs`.
//! The environment builder calls the Windows backend to encode OS text.
//! The earlier census pinned it as an exception before that boundary moved.
//!
//! The full criterion stands on the wiki page
//! `The-Seam-Criterion.md`.
// The backend selector also owns the unsupported-target diagnostic.
mod sys;

#[cfg(windows)]
mod auth;
mod base64;
#[cfg(windows)]
mod cli;
#[cfg(windows)]
mod cli_process;
#[cfg(windows)]
mod diagnostics;
#[cfg(windows)]
mod dispatch;

#[cfg(windows)]
pub use diagnostics::{cli_exit, take_diagnostics, DiagnosticOutput, DiagnosticStream};
#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod extend;
#[cfg(windows)]
mod hook;
#[cfg(windows)]
mod plugins;
#[cfg(windows)]
mod timing;

/// Inline storage for the ordinary handful of handles/IDs; deep ancestry
/// spills without changing handle ownership. All exposed elements are initialized.
/// The inline capacity is a parameter of the list: the engine's lists hold 16,
/// the console cleanup's flushed-process list holds 64. This is the one home
/// of the type, at the crate root both its users (the engine and the CLI
/// process policy) descend from; `sys.rs` holds Windows declarations only.
#[derive(Debug)]
pub(crate) struct SmallList<T: Copy + Default, const N: usize> {
    inline: [T; N],
    len: usize,
    spill: Vec<T>,
}
impl<T: Copy + Default, const N: usize> SmallList<T, N> {
    pub(crate) fn new() -> Self { Self { inline: [T::default(); N], len: 0, spill: Vec::new() } }
    pub(crate) fn from_slice(values: &[T]) -> Self {
        let mut list = Self::new();
        for &value in values { list.push(value); }
        list
    }
    pub(crate) fn push(&mut self, value: T) {
        if !self.spill.is_empty() { self.spill.push(value); }
        else if self.len < self.inline.len() { self.inline[self.len] = value; self.len += 1; }
        else {
            self.spill = Vec::with_capacity(64);
            self.spill.extend_from_slice(&self.inline[..self.len]);
            self.spill.push(value);
        }
    }
    pub(crate) fn extend_from_slice(&mut self, values: &[T]) { for &value in values { self.push(value); } }
}
impl<T: Copy + Default, const N: usize> std::ops::Deref for SmallList<T, N> {
    type Target = [T];
    fn deref(&self) -> &[T] { if self.spill.is_empty() { &self.inline[..self.len] } else { &self.spill } }
}
impl<T: Copy + Default + PartialEq, const N: usize> PartialEq<Vec<T>> for SmallList<T, N> {
    fn eq(&self, other: &Vec<T>) -> bool { &**self == other.as_slice() }
}

/// The file name of the program at `image`, whichever way the path is written,
/// as it stands: its callers compare it case-insensitively in place. The one
/// home of the split, at the crate root: the CLI, the CLI process policy, the
/// engine's owner walk, the extension state and the approval all read a file
/// name out of an image path, and the rule (either separator; the whole path
/// when there is none) cannot drift between them.
pub(crate) fn file_name_of(image: &str) -> &str {
    image.rsplit(['\\', '/']).next().unwrap_or("")
}

#[cfg(test)]
pub(crate) mod test_temp {
    use std::path::PathBuf;

    fn candidate(prefix: &str, stamp: u128, attempt: u32) -> PathBuf {
        std::env::temp_dir().join(format!("{prefix}-{}-{stamp}-{attempt}", std::process::id()))
    }

    fn fresh_dir_at(prefix: &str, stamp: u128) -> PathBuf {
        assert!(!prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        for attempt in 0..128 {
            let path = candidate(prefix, stamp, attempt);
            match std::fs::create_dir(&path) {
                Ok(()) => return path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
                Err(error) => panic!("create exclusive test folder {}: {error}", path.display()),
            }
        }
        panic!("128 exclusive test-folder candidates were already present for {prefix}");
    }

    pub(crate) fn fresh_dir(prefix: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_nanos();
        fresh_dir_at(prefix, stamp)
    }

    #[test]
    fn a_collision_keeps_the_sentinel_and_selects_a_different_empty_folder() {
        let prefix = "ptg-exclusive-dir-collision";
        let start = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_nanos();
        let (stamp, first) = (0..128).find_map(|offset| {
            let stamp = start + offset;
            let first = candidate(prefix, stamp, 0);
            match std::fs::create_dir(&first) {
                Ok(()) => Some((stamp, first)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => panic!("make collision fixture {}: {error}", first.display()),
            }
        }).expect("128 collision-fixture timestamps were already present");
        let sentinel = first.join("sentinel.txt");
        std::fs::write(&sentinel, b"keep original").expect("write sentinel");
        let selected = fresh_dir_at(prefix, stamp);
        assert_ne!(selected, first);
        assert!(std::fs::read_dir(&selected).unwrap().next().is_none(), "new folder starts empty");
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"keep original");
        std::fs::remove_dir(&selected).expect("remove empty selected folder");
        std::fs::remove_file(&sentinel).expect("remove our sentinel");
        std::fs::remove_dir(&first).expect("remove empty collision fixture");
    }
}

#[cfg(windows)]
pub use extend::{Grant, CHANNEL_VARIABLE, DEFAULT_MAX_EXTENSION};

#[cfg(windows)]
pub(crate) use dispatch::{entry_contain, entry_deadline, entry_full, entry_gate};
use std::fmt;

/// The longest deadline the guard accepts, in seconds (24 hours).
pub const MAX_SECONDS: f64 = 86400.0;

/// The exit code of a run whose deadline passed: the run's own result
/// when its process tree was terminated at the deadline, and the code
/// the job's members are terminated with.
pub(crate) const EXIT_TIMEOUT: i32 = 124;

/// The exit code of a guard error, and of a run ended because the
/// process that launched the guard exited.
pub(crate) const EXIT_GUARD_ERROR: i32 = 125;

/// The exit code of a hook denial: the one code with which a host
/// hook blocks a tool call.
pub(crate) const EXIT_HOOK_DENY: i32 = 2;

/// A guard error: invalid arguments, a program that is not found, or a failed
/// system call. The command line programs report it on standard error as
/// `PROCESS_GUARD_ERROR: MESSAGE` and exit with 125.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardError(String);

impl GuardError {
    /// The error text, without the `PROCESS_GUARD_ERROR` prefix.
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GuardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GuardError {}

/// A shell the guard can run an encoded command line in. PowerShell and `cmd` are
/// found as the `PATH` search for a command finds them, as in the earlier version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Shell {
    /// `pwsh.exe` when the `PATH` search finds one, otherwise `powershell.exe`.
    PowerShell,
    /// Git Bash at its standard or Program Files/per-user installation.
    Bash,
    /// The program named by `COMSPEC`, otherwise `cmd.exe`.
    Cmd,
}

impl Shell {
    /// The name used on the command line: `powershell`, `bash` or `cmd`.
    pub fn name(self) -> &'static str {
        match self {
            Shell::PowerShell => "powershell",
            Shell::Bash => "bash",
            Shell::Cmd => "cmd",
        }
    }

    /// The shell with this command line name, if any.
    pub fn from_name(name: &str) -> Option<Shell> {
        [Shell::PowerShell, Shell::Bash, Shell::Cmd].into_iter().find(|shell| shell.name() == name)
    }
}

/// Run `command` (a program and its arguments) in a job object and end the
/// whole process tree after `seconds`. `seconds` must be finite, greater than
/// zero and at most [`MAX_SECONDS`]. Returns the command's exit code, 124 when
/// the deadline passed, or 125 when the owner exited: the owner is the calling
/// process's parent (see the owner rule in the crate documentation). When that
/// parent has already exited the call is an error, or returns 125 at once. Inside an
/// extendable run the call asks the run for `seconds` first (an error if the run refuses),
/// and the hold it makes is ended when the call returns; see the crate documentation.
#[cfg(windows)]
pub fn run(command: &[String], seconds: f64) -> Result<u32, GuardError> {
    crate::cli_process::returning(|| {
        let _diagnostics = diagnostics::Invocation::begin();
        cli::check_seconds(seconds)?;
        if command.is_empty() {
            return Err(GuardError("a command is required".into()));
        }
        engine::run_library_job(command, seconds, None, false)
    })
}

/// Run the Base64 encoded UTF-8 command text `command_b64` in `shell`, under the
/// same deadline and owner rules as [`run`]: the owner is the calling process's
/// parent, and when that parent has already exited the call is an error, or returns
/// 125 at once. The text must decode to a nonempty command with no NUL character.
#[cfg(windows)]
pub fn run_shell(shell: Shell, command_b64: &str, seconds: f64) -> Result<u32, GuardError> {
    crate::cli_process::returning(|| {
        let _diagnostics = diagnostics::Invocation::begin();
        cli::check_seconds(seconds)?;
        let command = base64::decode_command(command_b64)?;
        let (argv, tail) = cli::shell_command(shell, command);
        engine::run_library_job(&argv, seconds, tail.as_deref(), false)
    })
}

/// Automatic mode. `args` are the arguments after the program name (or after the
/// `auto` subcommand): `[--timeout SECONDS] [--shell powershell|bash|cmd]
/// [--command-b64 BASE64 | [--] COMMAND...]`. The deadline defaults to `PROCESS_TREE_GUARD_AUTO_TIMEOUT`, else
/// `PROCESS_GUARD_AUTO_TIMEOUT`, else `HARNESS_AUTO_TIMEOUT`, else two hours. The
/// owners are every live ancestor of the calling process, not only its parent: the
/// tree ends, with 125, when any of them exits. When the parent has already exited
/// the call is an error, or returns 125 at once.
#[cfg(windows)]
pub fn run_auto(args: &[String]) -> Result<u32, GuardError> {
    crate::cli_process::returning(|| {
        let _diagnostics = diagnostics::Invocation::begin();
        cli::auto_slice(args, true)
    })
}

/// Ask the enclosing extendable run to hold the deadline of the calling process
/// for `seconds` from now. The run is the one named by `PROCESS_TREE_GUARD_CHANNEL`,
/// which an extendable `process-tree-guard` sets for its command. `seconds` is sent
/// as it is: the run refuses, and logs, a number that is not finite, not greater
/// than zero, over [`MAX_SECONDS`] or over its own cap. Returns the grant, or a
/// [`GuardError`] with the reason when the run refuses (the calling process is not
/// in its job, the request is invalid or over its cap, a rate limit, the run is
/// over), when it does not answer within a few seconds, or when there is no run to
/// ask. The hold ends when the calling process exits, or when the seconds run out.
#[cfg(windows)]
pub fn extend(seconds: f64) -> Result<Grant, GuardError> {
    crate::cli_process::returning(|| {
        let _diagnostics = diagnostics::Invocation::begin();
        extend::request(extend::Target::Caller, seconds)
    })
}

/// A hook mode of the full command: `pretooluse`, `cursor`, `cursor-context`,
/// `antigravity`, `antigravity-context` or `powershell-auto`; any other name is
/// refused with the usage text on standard error and the guard error code,
/// without reading standard input. Reads the host's payload on standard input and writes the host's
/// answer on standard output. The payload is read under a 16 MiB cap: decided on its first 16 MiB, with bytes past them never read, and a payload whose deciding text is not complete within them denied. Returns the exit code: 0 allows, 2 blocks (the
/// `antigravity` vendors and `powershell-auto` always return 0). The files it looks for (the automatic
/// copy, `POLICY.md`) and the envelope it accepts follow the file name of the
/// running program, as for `process-tree-guard hook VENDOR`.
#[cfg(windows)]
pub fn run_hook(vendor: &str) -> i32 {
    crate::cli_process::returning(|| {
        let _diagnostics = diagnostics::Invocation::begin();
        plugins::main(vendor, Role::Full)
    })
}

/// Which program is running; each role has its own set of modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Role {
    /// The full command: every mode (see [`main_full`]).
    Full,
    /// The deadline role: the `--timeout` form, `extend` and automatic mode, and no hook mode (see [`main_deadline`]).
    Deadline,
    /// The hook role: it answers hook payloads and never runs a command (see [`main_gate`]).
    Gate,
    /// The contain role: a `--timeout` run inside an extendable run, as a request to it (see [`main_contain`]).
    #[cfg(windows)]
    Contain,
}


/// The entry point of `process-tree-guard`, the full command: `--timeout` (with
/// `--extendable`), the `extend`, `auto` and `hook` subcommands and automatic mode
/// by a file name ending in `-auto`. Returns the exit code; optional output is retrieved with [`take_diagnostics`].
/// --extendable requires calling this entry point through [`cli_exit`]; otherwise
/// it is refused before audit sinks or a child are started.
#[cfg(windows)]
pub fn main_full() -> i32 {
    let _diagnostics = diagnostics::Invocation::begin();
    let code = entry_full();
    // What the trace kept and no run wrote: an argument error, or a hook.
    engine::flush_trace(None);
    code
}

/// The entry point of `process-tree-guard-deadline`, the deadline role: the full
/// command with `--timeout` (and `--extendable`), `extend` or automatic mode, and
/// no hook mode. Returns the exit code; optional output is retrieved with [`take_diagnostics`].
/// --extendable requires the process-owning [`cli_exit`] adapter.
#[cfg(windows)]
pub fn main_deadline() -> i32 {
    let _diagnostics = diagnostics::Invocation::begin();
    let code = entry_deadline();
    // What the trace kept and no run wrote: an argument error, or a hook.
    engine::flush_trace(None);
    code
}

/// The entry point of `process-tree-guard-gate`, the hook role:
/// `process-tree-guard-gate VENDOR` takes the vendors and formats of
/// `process-tree-guard hook VENDOR`, with `pretooluse` when no vendor is given.
/// It differs in the envelope set it accepts, the automatic copy it rewrites to,
/// and `-h` and `--help`: as its first argument, either prints the usage and
/// returns 0 without reading standard input, and the gate has no other options.
/// The program is the hook role whatever its file name: it keeps the gate's
/// envelope and wrapper rules and never accepts an envelope that names its own
/// path, in whatever letter case it was started. Returns the exit code; optional
/// output is retrieved with [`take_diagnostics`].
#[cfg(windows)]
pub fn main_gate() -> i32 {
    let _diagnostics = diagnostics::Invocation::begin();
    let code = entry_gate();
    // What the trace kept and no run wrote: an argument error, or a hook.
    engine::flush_trace(None);
    code
}

/// The entry point of `process-tree-guard-contain`, the contain role:
/// `process-tree-guard-contain --timeout SECONDS (--shell {powershell,bash,cmd} --command-b64 BASE64 | [--] COMMAND...)` runs the command in a
/// job of its own as a child of an extendable run. Its `--timeout` is a request to
/// the run that encloses it, which is refused (exit code 125) when it is over the
/// run's cap; with no such run it refuses to start, with 125, because it never
/// runs a command without a deadline that something finite holds. It takes no
/// extension options and has no automatic or hook mode, whatever its file name.
/// Returns the exit code; optional output is retrieved with [`take_diagnostics`].
#[cfg(windows)]
pub fn main_contain() -> i32 {
    let _diagnostics = diagnostics::Invocation::begin();
    let code = entry_contain();
    // What the trace kept and no run wrote: an argument error, or a hook.
    engine::flush_trace(None);
    code
}

#[cfg(all(test, windows))]
mod diagnostic_api_tests {
    use super::*;
    use crate::dispatch::finish;

    #[test]
    fn returning_main_dispatch_refuses_extension_and_cli_owned_dispatch_remains_available() {
        let args = || ["--extendable", "--timeout", "1", "--", r"?:\missing.exe"].iter().map(|arg| arg.to_string()).collect();
        let error = cli::run_args(args(), Role::Full).expect_err("returning main dispatch must refuse before launching");
        assert!(error.message().contains("requires the process-owning cli_exit adapter"));
        assert_eq!(finish(Err(error)), 125);
        assert!(take_diagnostics().iter().any(|record| record.text.contains("requires the process-owning cli_exit adapter")));
        {
            let _owner = diagnostics::ProcessOwnership::begin();
            let error = cli::run_args(args(), Role::Full).expect_err("the intentionally invalid executable is not found");
            assert!(error.message().starts_with("executable not found:"), "owned CLI dispatch passes the ownership check: {}", error);
        }
        assert!(!diagnostics::owns_process());
    }

    #[test]
    fn returning_finish_owns_exact_error_without_a_writer_or_host_exit() {
        let _call = diagnostics::Invocation::begin();
        assert_eq!(finish(Err(GuardError("missing é.exe".into()))), 125);
        assert_eq!(take_diagnostics(), vec![DiagnosticOutput {
            stream: DiagnosticStream::Stderr,
            text: "PROCESS_GUARD_ERROR: missing é.exe\n".into(),
        }]);
    }

    #[test]
    fn returning_validation_keeps_the_public_result_error() {
        let error = run(&[], 1.0).expect_err("an empty command is refused");
        assert_eq!(error.message(), "a command is required");
        assert!(take_diagnostics().is_empty(), "the caller owns its Result error; no writer is started");
    }
}
