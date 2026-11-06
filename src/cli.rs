// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The command line forms: `--timeout SECONDS ...`, automatic mode, `extend` and the contain role.

use crate::auth::{Exception, Method};
use crate::base64::{base64_encode_utf16le, decode_command};
#[cfg(test)]
use crate::base64::base64_encode;
use crate::engine::{list2cmdline, run_contained_job, run_job_with, which, RunMode};
use crate::extend::{escape_into, image_of, iso8601_into, request_held, Settings, Target, DEFAULT_MAX_EXTENSION};
use crate::{GuardError, Role, Shell, MAX_SECONDS};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

mod auto;
mod help;

pub use auto::{auto, auto_slice};
pub use help::{help, usage_for, CONTAIN_USAGE, GATE_USAGE, USAGE};

/// Seconds that are finite, greater than zero and at most MAX_SECONDS.
pub fn check_seconds(seconds: f64) -> Result<f64, GuardError> {
    if crate::deadline::invalid_seconds(seconds) {
        return Err(GuardError(format!("timeout must be greater than zero and at most {} seconds", crate::engine::format_seconds(MAX_SECONDS))));
    }
    Ok(seconds)
}

/// Seconds from text: a finite number greater than zero and at most 86400.
fn duration(value: &str) -> Result<f64, GuardError> {
    let seconds: f64 = value.trim().parse().map_err(|_| GuardError("timeout must be a number".into()))?;
    check_seconds(seconds)
}

fn git_bash_host() -> String {
    git_bash_host_with(|path| path.is_file(), |name| std::env::var_os(name))
}

fn git_bash_host_with(is_file: impl Fn(&std::path::Path) -> bool, env: impl Fn(&str) -> Option<std::ffi::OsString>) -> String {
    use std::path::{Path, PathBuf};
    const STANDARD: &str = "C:/Program Files/Git/bin/bash.exe";
    if is_file(Path::new(STANDARD)) { return STANDARD.into(); }
    for (key, parts) in [("ProgramFiles", &["Git", "bin", "bash.exe"][..]),
        ("ProgramFiles(x86)", &["Git", "bin", "bash.exe"][..]),
        ("LOCALAPPDATA", &["Programs", "Git", "bin", "bash.exe"][..])] {
        if let Some(root) = env(key).filter(|root| !root.is_empty()) {
            let mut candidate = PathBuf::from(root);
            for part in parts { candidate.push(part); }
            if is_file(&candidate) { return candidate.to_string_lossy().into_owned(); }
        }
    }
    STANDARD.into()
}

/// The argument list and verbatim `cmd` tail that run `command` in `shell`. The
/// PowerShell and `cmd` programs are named as in the earlier version and found when the job starts.
pub fn shell_command(shell: Shell, command: String) -> (Vec<String>, Option<String>) {
    shell_command_with_host(shell, command, None)
}

/// As `shell_command`, with a caller-supplied PowerShell host: the PowerShell
/// branch takes `host` when it is `Some` and searches for its host only when
/// it is `None`. The Bash and Cmd branches ignore `host`.
pub fn shell_command_with_host(shell: Shell, command: String, host: Option<String>) -> (Vec<String>, Option<String>) {
    match shell {
        Shell::PowerShell => {
            let encoded = base64_encode_utf16le(&command);
            let host = host.unwrap_or_else(|| which("pwsh.exe").unwrap_or_else(|| "powershell.exe".into()));
            (vec![host, "-NoLogo".into(), "-NoProfile".into(), "-NonInteractive".into(), "-EncodedCommand".into(), encoded], None)
        }
        Shell::Bash => (vec![git_bash_host(), "--noprofile".into(), "--norc".into(), "-c".into(), command], None),
        Shell::Cmd => (vec![std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()), "/d".into(), "/s".into(), "/c".into()], Some(command)),
    }
}

struct Options {
    help: bool,
    timeout: Option<String>,
    shell: Option<String>,
    command_b64: Option<String>,
    command: Vec<String>,
    extendable: bool,
    max_extension: Option<String>,
    extension_log: Option<String>,
    authenticate: Option<String>,
}

/// Parse the argument list. Errors become exit 125, never 2. `-h` and `--help`
/// stop the parse and ask for the usage text. The extension options are read only
/// when `extension_options` is true (the full command and the deadline role).
fn parse(mut args: Vec<String>, extension_options: bool) -> Result<Options, GuardError> {
    let mut options = Options {
        help: false, timeout: None, shell: None, command_b64: None, command: Vec::new(),
        extendable: false, max_extension: None, extension_log: None, authenticate: None,
    };
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with('-') || arg == "--" {
            args.drain(..i);
            options.command = args;
            break;
        }
        if arg == "-h" || arg == "--help" { options.help = true; break; }
        if arg == "--extendable" && extension_options {
            options.extendable = true;
            i += 1;
            continue;
        }
        let (key, inline) = arg.split_once('=').map_or((arg.as_str(), None), |(key, value)| (key, Some(value)));
        // The byte index of the `=` when the value is inline: the
        // key is the argument up to it, so its length is the index.
        let inline_at = inline.map(|_| key.len());
        // The recognized key is static: moving the owned value never requires
        // cloning the option name or the remaining command arguments.
        let key = match key {
            "--timeout" => "--timeout",
            "--shell" => "--shell",
            "--command-b64" => "--command-b64",
            "--max-extension" if extension_options => "--max-extension",
            "--extension-log" if extension_options => "--extension-log",
            "--authenticate" if extension_options => "--authenticate",
            other => return Err(GuardError(format!("unrecognized arguments: {}", other))),
        };
        let value = match inline_at {
            Some(eq) => {
                // The inline value is the tail of an argument this
                // parse already owns: cutting the `--key=` head off
                // that argument in place and keeping its storage
                // replaces the copy `to_owned` made of the value of
                // every inline option, on every invocation. The key
                // was validated above, so error precedence is the
                // one the copying form kept.
                let mut owned = std::mem::take(&mut args[i]);
                owned.replace_range(..=eq, "");
                owned
            }
            None => {
                i += 1;
                args.get_mut(i).map(std::mem::take)
                    .ok_or_else(|| GuardError(format!("argument {}: expected one argument", key)))?
            }
        };
        match key {
            "--timeout" => options.timeout = Some(value),
            "--shell" => {
                if Shell::from_name(&value).is_none() {
                    return Err(GuardError(format!("argument --shell: invalid choice: '{}'", value)));
                }
                options.shell = Some(value);
            }
            "--command-b64" => options.command_b64 = Some(value),
            "--max-extension" => options.max_extension = Some(value),
            "--extension-log" => options.extension_log = Some(value),
            "--authenticate" => {
                if Method::from_name(&value).is_none() {
                    return Err(GuardError(format!("argument --authenticate: invalid choice: '{}' (choose from 'desktop', 'console')", value)));
                }
                options.authenticate = Some(value);
            }
            _ => unreachable!(),
        }
        i += 1;
    }
    Ok(options)
}

/// The settings of an extendable run, from its launch arguments alone: never from
/// the environment, so nothing a child sets can raise the cap.
fn extension_settings(options: &Options, timeout: f64) -> Result<Option<Settings>, GuardError> {
    if !options.extendable {
        if options.max_extension.is_some() || options.extension_log.is_some() || options.authenticate.is_some() {
            return Err(GuardError("--max-extension, --extension-log and --authenticate need --extendable".into()));
        }
        return Ok(None);
    }
    let seconds_option = |name: &str, text: &Option<String>, default: f64| -> Result<f64, GuardError> {
        match text {
            Some(text) => {
                let value: f64 = text.trim().parse().map_err(|_| GuardError(format!("{} must be a number", name)))?;
                if !value.is_finite() || value <= 0.0 || value > MAX_SECONDS {
                    return Err(GuardError(format!("{} must be greater than zero and at most {} seconds", name, crate::engine::format_seconds(MAX_SECONDS))));
                }
                Ok(value)
            }
            None => Ok(default),
        }
    };
    let max_seconds = seconds_option("--max-extension", &options.max_extension, DEFAULT_MAX_EXTENSION)?;
    // The run's own deadline is a first request: it must fit under the cap like any other.
    if timeout > max_seconds {
        return Err(GuardError(format!("--timeout ({}) is over --max-extension ({}); raise --max-extension, at most {} seconds",
            crate::engine::format_seconds(timeout), crate::engine::format_seconds(max_seconds), crate::engine::format_seconds(MAX_SECONDS))));
    }
    let log_path = match &options.extension_log {
        Some(path) if path.is_empty() => return Err(GuardError("--extension-log needs a path".into())),
        Some(path) => Some(std::sync::Arc::from(PathBuf::from(path).as_path())),
        None => None,
    };
    // A cap over the default is an exception that needs the approval of the signed-in user (Windows
    // Hello, or the password of the account the guard runs as). This function only says so: the
    // prompt is made later, when the command is known to be runnable, and never from a launch
    // argument alone. `--authenticate` is read and ignored at or below the default cap, which
    // never prompts. The command line is filled in by the caller.
    let exception = (max_seconds > DEFAULT_MAX_EXTENSION).then(|| Exception {
        method: options.authenticate.as_deref().and_then(Method::from_name).unwrap_or(Method::Desktop),
        command: String::new(),
        timeout,
    });
    Ok(Some(Settings { max_seconds, log_path, exception }))
}

/// `--timeout SECONDS (--shell SHELL --command-b64 BASE64 | [--] COMMAND [ARGS...])`
/// for the contain role, with `args` the arguments after the program name. The
/// contain role takes no extension options: its deadline is a request to the
/// run that encloses it, and its run cannot carry an exception, so it takes
/// the plain run path.
pub fn run_contain_args(args: Vec<String>) -> Result<u32, GuardError> {
    let options = parse(args, false)?;
    if options.help {
        help(CONTAIN_USAGE);
        return Ok(0);
    }
    let launch = plan(options)?;
    run_contained_job(&launch.child, launch.seconds, launch.raw_tail.as_deref())
}

/// `[--extendable ...] --timeout SECONDS (--shell SHELL --command-b64 BASE64 | [--] COMMAND [ARGS...])`,
/// with `args` the arguments after the program name. The contain role takes no
/// extension options: its deadline is a request to the run that encloses it.
pub fn run_args(args: Vec<String>, role: Role) -> Result<u32, GuardError> {
    if role == Role::Contain {
        return run_contain_args(args);
    }
    let options = parse(args, true)?;
    if options.help {
        help(usage_for(role));
        return Ok(0);
    }
    let launch = plan(options)?;
    run_job_with(&launch.child, launch.seconds, launch.raw_tail.as_deref(), false, &RunMode { extendable: launch.extendable, require_channel: false, library: false })
}

/// What a command line asks to run.
struct Launch {
    child: Vec<String>,
    seconds: f64,
    raw_tail: Option<String>,
    extendable: Option<Settings>,
}

/// Check the options and work out the command: the errors in the order they are reported (the
/// timeout and the cap, then the command), with no prompt and no system call.
fn plan(mut options: Options) -> Result<Launch, GuardError> {
    let timeout = options.timeout.as_deref().ok_or_else(|| GuardError("the following arguments are required: --timeout".into()))?;
    let seconds = duration(timeout)?;
    let mut extendable = extension_settings(&options, seconds)?;
    let mut raw_tail: Option<String> = None;
    // The text the prompt of an exception shows: the command of a shell, else the command line.
    let needs_shown = extendable.as_ref().is_some_and(|settings| settings.exception.is_some());
    let mut shown = None;
    let child: Vec<String> = if let Some(shell) = options.shell {
        let encoded = match (&options.command_b64, options.command.is_empty()) {
            (Some(b64), true) => b64,
            _ => return Err(GuardError("--shell requires --command-b64 and no additional arguments".into())),
        };
        let command = decode_command(encoded)?;
        let shell = Shell::from_name(&shell).expect("the shell was checked when the arguments were parsed");
        if needs_shown { shown = Some(command.clone()); }
        let (child, tail) = shell_command(shell, command);
        raw_tail = tail;
        child
    } else {
        if options.command_b64.is_some() {
            return Err(GuardError("--command-b64 requires --shell".into()));
        }
        let after_dashes = options.command.first().map(String::as_str) == Some("--");
        if after_dashes { options.command.remove(0); }
        let child = options.command;
        if child.is_empty() {
            return Err(GuardError(if after_dashes { "a command is required after --" } else { "a command is required" }.into()));
        }
        if needs_shown { shown = Some(list2cmdline(&child)); }
        child
    };
    if let Some(exception) = extendable.as_mut().and_then(|settings| settings.exception.as_mut()) {
        exception.command = shown.expect("exception command was rendered");
    }
    Ok(Launch { child, seconds, raw_tail, extendable })
}

/// `extend SECONDS`: ask the enclosing extendable run to hold the deadline of the
/// process that ran this program (its parent, since this program ends at once)
/// for SECONDS from now. Prints the deadline that was granted and the process held.
/// The number is sent as it is: the run refuses, and logs, one it will not grant.
pub fn extend_command(args: Vec<String>, role: Role) -> Result<u32, GuardError> {
    if matches!(args.first().map(String::as_str), Some("-h" | "--help")) {
        help(usage_for(role));
        return Ok(0);
    }
    let usage = if role == Role::Deadline { "usage: process-tree-guard-deadline extend SECONDS" } else { "usage: process-tree-guard extend SECONDS" };
    let [seconds] = <[String; 1]>::try_from(args).map_err(|_| GuardError(usage.into()))?;
    let seconds: f64 = seconds.trim().parse().map_err(|_| GuardError("extend: SECONDS must be a number".into()))?;
    let (grant, held) = request_held(Target::Parent, 0, seconds)?;
    let until = grant.until.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    // The receipt is assembled in its one buffer: the deadline text
    // and the escaped image were each built as their own string
    // first and copied into the formatted line, three allocations
    // beside the line's own for text the buffer holds as well in place.
    use std::fmt::Write as _;
    let mut line = String::with_capacity(96);
    line.push_str("PROCESS_TREE_GUARD_GRANTED: seconds=");
    crate::engine::format_seconds_into(grant.seconds, &mut line);
    line.push_str(" deadline=");
    iso8601_into(until, &mut line);
    let _ = write!(line, " held={}", held);
    // The receipt names the holder by pid and image on every output, a
    // console or a pipe alike: the line ends with `held=ID image="NAME"`.
    line.push_str(" image=\"");
    escape_into(&image_of(held), &mut line);
    line.push('"');
    // The receipt is mandatory output, written synchronously through the raw-handle line writer.
    crate::cli_process::write_stdout_line(&line);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_discovery_uses_standard_then_program_files_then_user_roots_without_a_global_cache() {
        use std::path::Path;
        let user = Path::new(r"C:\Users\test\AppData\Local").join("Programs").join("Git").join("bin").join("bash.exe");
        let result = git_bash_host_with(|path| path == user, |key| {
            (key == "LOCALAPPDATA").then(|| std::ffi::OsString::from(r"C:\Users\test\AppData\Local"))
        });
        assert_eq!(Path::new(&result), user);
        assert_eq!(git_bash_host_with(|_| false, |_| None), "C:/Program Files/Git/bin/bash.exe");
        assert_eq!(git_bash_host_with(|_| true, |_| panic!("standard path is checked first")), "C:/Program Files/Git/bin/bash.exe");
    }

    #[test]
    fn duration_bounds() {
        assert!(duration(".3").is_ok());
        assert!(duration("86400").is_ok());
        for bad in ["0", "-1", "nan", "inf", "86401", "x", ""] {
            assert!(duration(bad).is_err(), "{}", bad);
        }
        assert!(check_seconds(0.5).is_ok());
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 86401.0] {
            assert!(check_seconds(bad).is_err(), "{}", bad);
        }
    }

    #[test]
    fn the_envelope_cap_is_max_seconds() {
        // The envelope checker's `TimeoutCheck` in src/hook/envelope.rs
        // compares against `crate::MAX_SECONDS` itself; only its
        // `TIMEOUT_MID` midpoint is derived from the value above it,
        // so a change to `crate::MAX_SECONDS` must re-derive that
        // midpoint and this test then fails until it is re-derived.
        assert_eq!(crate::MAX_SECONDS, 86400.0);
        assert!(duration("86400").is_ok());
        assert!(duration("86401").is_err());
    }

    #[test]
    fn argument_errors_never_exit_two() {
        assert!(parse(vec!["--bogus".into(), "1".into()], true).is_err());
        assert!(matches!(parse(vec!["--bogus".into()], true), Err(GuardError(m)) if m.starts_with("unrecognized")));
        assert!(parse(vec!["--shell".into(), "zsh".into()], true).is_err());
        assert!(parse(vec!["--timeout".into()], true).is_err());
        let ok = parse(vec!["--timeout=5".into(), "--".into(), "x".into()], true).ok().unwrap();
        assert_eq!(ok.timeout.as_deref(), Some("5"));
        assert_eq!(ok.command, vec!["--".to_string(), "x".to_string()]);
        assert!(parse(vec!["--help".into()], true).ok().unwrap().help);
        assert!(parse(vec!["-h".into(), "--bogus".into()], true).ok().unwrap().help);
    }

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn extension_options_are_read_from_the_arguments_only() {
        let ok = parse(words("--extendable --max-extension 60 --extension-log a.log --timeout 5 -- x"), true).ok().unwrap();
        assert!(ok.extendable);
        assert_eq!(ok.max_extension.as_deref(), Some("60"));
        assert_eq!(ok.extension_log.as_deref(), Some("a.log"));
        assert_eq!(extension_settings(&ok, 5.0).ok().flatten().map(|s| s.max_seconds), Some(60.0));
        // An option after the command belongs to the command.
        let after = parse(words("--timeout 5 -- x --extendable"), true).ok().unwrap();
        assert!(!after.extendable);
        // Not for the contain role, and not without --extendable.
        for line in ["--extendable --timeout 5 x", "--max-extension 5 --timeout 5 x", "--extension-log a --timeout 5 x"] {
            assert!(parse(words(line), false).is_err(), "{}", line);
        }
        for line in ["--max-extension 60 --timeout 5 x", "--extension-log a.log --timeout 5 x"] {
            let options = parse(words(line), true).ok().unwrap();
            assert!(extension_settings(&options, 5.0).is_err(), "{}", line);
        }
    }

    #[test]
    fn the_cap_has_a_default_and_bounds_and_covers_the_run_itself() {
        let settings = |line: &str, timeout: f64| extension_settings(&parse(words(line), true).ok().unwrap(), timeout);
        assert_eq!(settings("--extendable", 5.0).ok().flatten().map(|s| s.max_seconds), Some(DEFAULT_MAX_EXTENSION));
        assert_eq!(settings("--extendable --max-extension 86400", 86400.0).ok().flatten().map(|s| s.max_seconds), Some(86400.0));
        for bad in ["0", "-1", "nan", "inf", "86401", "x", ""] {
            assert!(settings(&format!("--extendable --max-extension={}", bad), 5.0).is_err(), "{}", bad);
        }
        assert!(settings("--extendable", 21600.0).is_ok());
        assert!(settings("--extendable", 21601.0).is_err(), "the run's own deadline is over the default cap");
        assert!(settings("--extendable --max-extension 10", 10.5).is_err());
        assert!(settings("--extendable --extension-log=", 5.0).is_err());
    }

    #[test]
    fn there_is_no_option_for_a_total_lifetime() {
        // The only limit on a request is the cap; a run that is renewed within it is renewed for as long as it is.
        assert!(parse(words("--extendable --max-total 600 --timeout 5 x"), true).is_err());
        assert!(parse(words("--max-total=600 --timeout 5 x"), true).is_err());
    }

    #[test]
    fn a_cap_over_the_default_is_an_exception_and_nothing_else_is() {
        let settings = |line: &str| extension_settings(&parse(words(line), true).ok().unwrap(), 5.0);
        for line in ["--extendable", "--extendable --max-extension 21600", "--extendable --max-extension 10",
            "--extendable --max-extension 21600 --authenticate console", "--extendable --max-extension 10 --authenticate desktop"] {
            let exception = settings(line).ok().flatten().expect("an extendable run").exception;
            assert!(exception.is_none(), "{} is at or below the default cap and never prompts", line);
        }
        let over_settings = settings("--extendable --max-extension 21601").ok().flatten().expect("over the cap");
        let over = over_settings.exception.as_ref().expect("over the cap");
        assert_eq!(over.method, Method::Desktop, "the desktop prompt is the default");
        assert!(over_settings.max_seconds == 21601.0 && over.command.is_empty(), "the exception has no cap of its own: the prompt asks for the one that is enforced");
        assert_eq!(over.timeout, 5.0, "the exception carries the deadline the run starts with, for the prompt to show");
        let day_settings = settings("--extendable --max-extension 86400 --authenticate console").ok().flatten().expect("the largest cap");
        let day = day_settings.exception.as_ref().expect("the largest cap");
        assert_eq!((day.method, day_settings.max_seconds), (Method::Console, 86400.0));
        let named = settings("--extendable --max-extension 86400 --authenticate desktop").ok().flatten().and_then(|s| s.exception).expect("desktop");
        assert_eq!(named.method, Method::Desktop);
        // Neither the cap nor the method means anything without --extendable.
        assert!(settings("--max-extension 86400").is_err());
        assert!(matches!(settings("--authenticate console"), Err(GuardError(m)) if m.contains("need --extendable")));
    }

    #[test]
    fn authenticate_takes_desktop_or_console_and_nothing_else() {
        for bad in ["", "Console", "DESKTOP", "hello", "console ", "windows-hello", "none"] {
            assert!(parse(vec!["--authenticate".into(), bad.into(), "--timeout".into(), "5".into()], true).is_err(), "{:?}", bad);
            assert!(parse(vec![format!("--authenticate={}", bad)], true).is_err(), "{:?}", bad);
        }
        for good in ["desktop", "console"] {
            assert_eq!(parse(vec![format!("--authenticate={}", good)], true).ok().unwrap().authenticate.as_deref(), Some(good));
        }
        assert!(parse(vec!["--authenticate".into()], true).is_err(), "a value is required");
        // The contain role takes no extension options, so no prompt either.
        assert!(matches!(parse(words("--authenticate console --timeout 5 x"), false), Err(GuardError(m)) if m.starts_with("unrecognized")));
        // After the command it is the command's own argument.
        assert!(parse(words("--timeout 5 -- x --authenticate bogus"), true).ok().unwrap().authenticate.is_none());
    }

    #[test]
    fn the_prompt_is_given_the_command_that_will_run() {
        let launch = |line: &str| plan(parse(words(line), true).ok().unwrap()).ok().unwrap();
        let shown = |launch: Launch| launch.extendable.and_then(|s| s.exception).map(|e| e.command);
        assert_eq!(shown(launch("--extendable --max-extension 86400 --timeout 5 -- tool.exe a b")), Some("tool.exe a b".to_string()));
        assert_eq!(shown(launch("--extendable --max-extension 86400 --timeout 5 tool.exe")), Some("tool.exe".to_string()));
        // The command line as it will be started: the boundaries of the arguments are kept, so `a b` and `a`, `b` do not look the same.
        let with = |args: &[&str]| {
            let mut line = words("--extendable --max-extension 86400 --timeout 5 --");
            line.extend(args.iter().map(|arg| arg.to_string()));
            shown(plan(parse(line, true).ok().unwrap()).ok().unwrap())
        };
        assert_eq!(with(&["tool.exe", "a b"]), Some("tool.exe \"a b\"".to_string()));
        assert_eq!(with(&["tool.exe", "a", "b"]), Some("tool.exe a b".to_string()));
        assert_eq!(with(&["tool.exe", "say \"hi\" there"]), Some("tool.exe \"say \\\"hi\\\" there\"".to_string()));
        assert_eq!(with(&["tool.exe", ""]), Some("tool.exe \"\"".to_string()), "an empty argument is shown");
        let b64 = base64_encode(b"echo hello");
        assert_eq!(shown(launch(&format!("--extendable --max-extension 86400 --shell cmd --command-b64 {} --timeout 5", b64))), Some("echo hello".to_string()),
            "a shell run shows the decoded text, not the encoded argument");
        assert_eq!(shown(launch("--extendable --timeout 5 -- tool.exe")), None, "a run at the default cap has no exception to show it in");
        // The largest deadline under the largest cap passes the argument checks: it is the only way to a 24 hour first deadline.
        assert!(plan(parse(words("--extendable --max-extension 86400 --authenticate console --timeout 86400 -- tool.exe"), true).ok().unwrap()).is_ok());
        // The cap and the timeout are checked before the command is looked at, and none of it prompts.
        let error = plan(parse(words("--extendable --max-extension 86400 --timeout 90000"), true).ok().unwrap()).err().expect("a bad timeout");
        assert!(error.message().contains("timeout"), "{}", error);
        assert!(plan(parse(words("--extendable --max-extension 86400 --timeout 5"), true).ok().unwrap()).is_err(), "no command");
    }

    #[test]
    fn shell_commands_name_the_shell_host() {
        let (cmd, tail) = shell_command(Shell::Cmd, "echo x".into());
        assert_eq!(cmd[1..], ["/d", "/s", "/c"]);
        assert_eq!(cmd[0], std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()));
        assert_eq!(tail.as_deref(), Some("echo x"));
        let (bash, tail) = shell_command(Shell::Bash, "echo x".into());
        assert_eq!(bash.last().map(String::as_str), Some("echo x"));
        assert!(tail.is_none());
        let (ps, _) = shell_command(Shell::PowerShell, "echo x".into());
        assert_eq!(ps[0], which("pwsh.exe").unwrap_or_else(|| "powershell.exe".into()));
        assert_eq!(ps[ps.len() - 2], "-EncodedCommand");
        assert_eq!(ps[ps.len() - 1], base64_encode(&"echo x".encode_utf16().flat_map(|u| u.to_le_bytes()).collect::<Vec<u8>>()));
    }
}
