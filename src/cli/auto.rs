// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Automatic mode: the wrapper a host puts in front of every shell command.

use super::{duration, help, shell_command_with_host, usage_for};
use crate::base64::decode_command;
use crate::engine::{parent_image_path, run_auto_job, ParentFacts};
use crate::{GuardError, Role, Shell};

/// Default hard deadline for `auto` mode: two hours, the longest any host shell call may run.
const AUTO_DEFAULT_SECONDS: f64 = 7200.0;

/// The environment variables that set the automatic deadline, in the order they
/// are read: the current name, the earlier name (accepted as an alias), then the legacy
/// host name. The first one that is set is used.
const AUTO_TIMEOUT_VARIABLES: [&str; 3] = ["PROCESS_TREE_GUARD_AUTO_TIMEOUT", "PROCESS_GUARD_AUTO_TIMEOUT", "HARNESS_AUTO_TIMEOUT"];

/// The value of the first of AUTO_TIMEOUT_VARIABLES that `lookup` finds.
fn auto_timeout_setting(lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    AUTO_TIMEOUT_VARIABLES.iter().find_map(|name| lookup(name))
}

/// `auto [--timeout SECONDS] [--shell powershell|bash|cmd] [--command-b64 BASE64 | [--] COMMAND...]`, with
/// `args` the arguments after the program name (or after the `auto` subcommand).
///
/// The automatic wrapper a host puts in front of every shell command. One
/// remaining argument is a command line for a shell: the shell named by
/// --shell, otherwise the same kind of shell that launched the guard (Git
/// Bash when unknown). Several remaining arguments are a program and its
/// arguments. The deadline is --timeout, else the first of
/// AUTO_TIMEOUT_VARIABLES that is set, else two hours. The host's own
/// per-call timeout still applies: when the host kills the process that
/// launched the guard, the guard kills the whole tree.
pub fn auto(args: Vec<String>, role: Role) -> Result<u32, GuardError> {
    if matches!(args.first().map(String::as_str), Some("-h" | "--help")) {
        help(usage_for(role));
        return Ok(0);
    }
    auto_with(args, false)
}

/// `auto`, for the program (`library` false) or for the library call `run_auto` (`library`
/// true), whose process goes on when the call returns.
pub fn auto_with(args: Vec<String>, library: bool) -> Result<u32, GuardError> {
    auto_slice(&args, library)
}

/// The slice form of `auto_with`: the prefix options borrow their values
/// from the argument list and the remaining arguments dispatch as a slice,
/// so a caller that holds no vector of its own (the library call
/// `run_auto`) pays no whole-vector clone before dispatch — at most the
/// one clone of a lone shell command, where the owned form moved that
/// element instead. The checks keep the owned form's order exactly: the
/// prefix loop first (the `--command-b64` decode and the
/// never-extendable refusal at their loop positions), then the deadline,
/// then the remaining-argument checks, then the single command's checks.
pub fn auto_slice(args: &[String], library: bool) -> Result<u32, GuardError> {
    let mut timeout: Option<&str> = None;
    let mut shell: Option<&str> = None;
    let mut encoded: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "--" { i += 1; break; }
        // The prefix options take the same two spellings as in `parse`:
        // `--key value` and `--key=value`. A recognised key with no value
        // is an error, never the command: before this, `--timeout` alone
        // (and likewise `--shell`, `--command-b64`, and every `--key=value`
        // spelling) fell through to the command text, so a mistyped
        // automatic call ran its own option line in a shell under the
        // default deadline instead of failing.
        let (key, inline) = arg.split_once('=').map_or((arg, None), |(key, value)| (key, Some(value)));
        match key {
            "--timeout" | "--shell" | "--command-b64" => {
                let value = match inline {
                    Some(value) => value,
                    None => {
                        i += 1;
                        args.get(i).map(String::as_str)
                            .ok_or_else(|| GuardError(format!("argument {}: expected one argument", key)))?
                    }
                };
                match key {
                    "--timeout" => timeout = Some(value),
                    "--shell" => shell = Some(value),
                    _ => encoded = Some(decode_command(value)?),
                }
                i += 1;
            }
            "--extendable" | "--max-extension" | "--extension-log" | "--authenticate" => {
                return Err(GuardError("auto: automatic mode is never extendable".into()));
            }
            _ => break,
        }
    }
    let rest = &args[i..];
    let seconds = if let Some(t) = timeout {
        duration(t)?
    } else if let Some(t) = auto_timeout_setting(|name| std::env::var(name).ok()) {
        duration(&t)?
    } else {
        AUTO_DEFAULT_SECONDS
    };
    let run = |argv: &[String], tail: Option<&str>, parent: Option<&ParentFacts>| {
        run_auto_job(argv, seconds, tail, library, parent)
    };
    // A command that arrived as `--command-b64` was already put
    // through exactly the nonempty-and-no-NUL test below by
    // `decode_command` when it was decoded, in the prefix loop
    // above: performing the trim and the NUL scan a second time on
    // the same text, on every encoded automatic call, settled
    // nothing the decode had not. Only a command taken from the
    // remaining arguments, which no decode has seen, takes the
    // test here.
    let (command, decoded) = match encoded {
        Some(command) => {
            if !rest.is_empty() {
                return Err(GuardError("auto: --command-b64 takes no further arguments".into()));
            }
            (command, true)
        }
        None => {
            if rest.is_empty() {
                return Err(GuardError("auto: a command is required".into()));
            }
            if rest.len() > 1 {
                return run(rest, None, None);
            }
            (rest[0].clone(), false)
        }
    };
    // The acceptance is the one fused pass `decode_command` makes
    // (see `nonempty_without_nul`): the trim emptiness test and the
    // NUL scan as separate calls walked this text twice.
    if !decoded && !crate::base64::nonempty_without_nul(&command) {
        return Err(GuardError("command must be nonempty and contain no NUL".into()));
    }
    // Explicit bash/cmd cannot use a parent image. Default selection and
    // PowerShell handover share their original object with owner monitoring.
    let needs_parent = shell_needs_parent_image(shell);
    let parent = if needs_parent && !library && crate::cli_process::active() {
        Some(ParentFacts::open()?)
    } else { None };
    let image = match parent.as_ref() {
        Some(parent) => std::borrow::Cow::Borrowed(parent.image.as_str()),
        None if needs_parent => std::borrow::Cow::Owned(parent_image_path()),
        None => std::borrow::Cow::Borrowed(""),
    };
    let (child, tail) = auto_shell_command(shell, image, command, shell_command_with_host)?;
    run(&child, tail.as_deref(), parent.as_ref())
}

fn shell_needs_parent_image(requested: Option<&str>) -> bool {
    matches!(requested, None | Some("powershell"))
}

/// Classify only the borrowed basename. The parent path is an execution
/// identity, so its original native spelling must never be lowercased.
fn auto_shell_command(
    requested: Option<&str>, parent: std::borrow::Cow<'_, str>, command: String,
    render: impl FnOnce(Shell, String, Option<String>) -> (Vec<String>, Option<String>),
) -> Result<(Vec<String>, Option<String>), GuardError> {
    let name = crate::file_name_of(&parent).as_bytes();
    let parent_is_powershell = [b"pwsh".as_slice(), b"powershell".as_slice()].iter()
        .any(|prefix| name.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix)));
    let shell = match requested {
        Some(name) => Shell::from_name(name)
            .ok_or_else(|| GuardError(format!("argument --shell: invalid choice: '{}'", name)))?,
        None if parent_is_powershell => Shell::PowerShell,
        None if name.eq_ignore_ascii_case(b"cmd.exe") => Shell::Cmd,
        None => Shell::Bash,
    };
    let handover = shell == Shell::PowerShell && parent_is_powershell && !parent.is_empty();
    let host = if handover { Some(parent.clone().into_owned()) } else { None };
    let (mut child, tail) = render(shell, command, host);
    if handover && child.first().map(String::as_str) != Some(parent.as_ref()) {
        // The render was handed the parent path as its host, so its
        // first element already is that path: assigning it again
        // built a second owned copy of the same text only to drop
        // the first. The assignment stands for a render whose first
        // element is anything else.
        child[0] = parent.into_owned();
    }
    Ok((child, tail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::help::{DEADLINE_USAGE, USAGE};

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn explicit_non_powershell_shells_do_not_request_parent_images() {
        for requested in [Some("bash"), Some("cmd"), Some("invalid"), Some("PowerShell")] {
            assert!(!shell_needs_parent_image(requested));
        }
        assert!(shell_needs_parent_image(None));
        assert!(shell_needs_parent_image(Some("powershell")));
        for requested in ["bash", "cmd"] {
            let (child, tail) = auto_shell_command(Some(requested), "".into(), "literal".into(), |shell, command, _host| {
                assert_eq!(shell, Shell::from_name(requested).unwrap());
                (vec!["selected.exe".into(), command], None)
            }).unwrap();
            assert_eq!(child, ["selected.exe", "literal"]);
            assert!(tail.is_none());
        }
    }

    #[test]
    fn automatic_powershell_handover_preserves_the_exact_parent_path() {
        for (requested, parent, expected_shell, use_parent) in [
            (None, r"C:\CaseSensitive\PowerShell\PwSh.EXE", Shell::PowerShell, true),
            (Some("powershell"), r"C:\OtherCASE\PowerShell.EXE", Shell::PowerShell, true),
            (None, r"\\server\CaseSensitive\PowerShell.EXE", Shell::PowerShell, true),
            (None, r"C:\ÄCase\PaTh\pWsH.EXE", Shell::PowerShell, true),
            (None, r"C:\Windows\SYSTEM32\CMD.EXE", Shell::Cmd, false),
            (Some("powershell"), r"C:\Tools\CMD.EXE", Shell::PowerShell, false),
            (Some("bash"), r"C:\ExactCase\PwSh.EXE", Shell::Bash, false),
            (None, r"C:\Tools\other.EXE", Shell::Bash, false),
        ] {
            let (child, tail) = auto_shell_command(requested, parent.into(), "unchanged command".into(), |shell, command, _host| {
                assert_eq!(shell, expected_shell);
                (vec!["default-host.exe".into(), "unchanged-flag".into(), command], Some("unchanged tail".into()))
            }).expect("select the ordinary automatic shell");
            assert_eq!(child[0], if use_parent { parent } else { "default-host.exe" });
            assert_eq!(&child[1..], &["unchanged-flag", "unchanged command"]);
            assert_eq!(tail.as_deref(), Some("unchanged tail"));
        }
        assert!(auto_shell_command(Some("PowerShell"), String::new().into(), "command".into(), |_, _, _| {
            (vec!["unused".into()], None)
        }).is_err(), "explicit shell names retain their case-sensitive public syntax");
    }

    #[test]
    fn automatic_deadline_variables_are_read_in_order() {
        let set = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
        };
        assert_eq!(auto_timeout_setting(set(&[])), None);
        assert_eq!(auto_timeout_setting(set(&[("HARNESS_AUTO_TIMEOUT", "3")])).as_deref(), Some("3"));
        assert_eq!(auto_timeout_setting(set(&[("HARNESS_AUTO_TIMEOUT", "3"), ("PROCESS_GUARD_AUTO_TIMEOUT", "2")])).as_deref(), Some("2"));
        assert_eq!(auto_timeout_setting(set(&[("PROCESS_GUARD_AUTO_TIMEOUT", "2"), ("HARNESS_AUTO_TIMEOUT", "3"), ("PROCESS_TREE_GUARD_AUTO_TIMEOUT", "1")])).as_deref(), Some("1"));
        assert_eq!(auto_timeout_setting(set(&[("PROCESS_TREE_GUARD_AUTO_TIMEOUT", ""), ("PROCESS_GUARD_AUTO_TIMEOUT", "2")])).as_deref(), Some(""));
    }

    #[test]
    fn automatic_help_prints_the_role_usage_and_runs_nothing() {
        for (role, usage) in [(Role::Full, USAGE), (Role::Deadline, DEADLINE_USAGE)] {
            for flag in ["--help", "-h"] {
                assert_eq!(auto(vec![flag.into()], role), Ok(0));
                assert_eq!(crate::take_diagnostics(), vec![crate::DiagnosticOutput {
                    stream: crate::DiagnosticStream::Stdout,
                    text: format!("{}\n", usage),
                }]);
            }
        }
    }

    #[test]
    fn automatic_prefix_options_fail_like_the_run_parser() {
        // A recognised prefix key with no value is an argument error, in
        // both spellings, exactly as `parse` has it; it is never taken
        // for the command text and run.
        for line in ["--timeout", "--shell", "--command-b64", "--timeout 5 --shell"] {
            let error = auto_with(words(line), false).expect_err("a valueless prefix option");
            assert!(error.message().contains("expected one argument"), "{}: {}", line, error);
        }
        // The `--key=value` spelling is read as the option it names: with
        // no command after it, the failure is the missing command, not a
        // shell run of the option line under the default deadline.
        let error = auto_with(words("--timeout=5"), false).expect_err("no command");
        assert!(error.message().contains("a command is required"), "{}", error);
        // The never-extendable refusal also fires in the inline spelling.
        for line in ["--max-extension=5 echo x", "--authenticate=console echo x", "--extension-log=a.log echo x", "--extendable=1 echo x"] {
            let error = auto_with(words(line), false).expect_err("automatic mode is never extendable");
            assert!(error.message().contains("never extendable"), "{}: {}", line, error);
        }
    }

    #[test]
    fn automatic_mode_never_takes_authenticate() {
        for line in ["--authenticate console echo x", "--extendable --authenticate console echo x", "--extendable --max-extension 86400 --authenticate console echo x"] {
            let error = auto_with(words(line), false).expect_err("automatic mode is never extendable");
            assert!(error.message().contains("never extendable"), "{}: {}", line, error);
        }
    }
}
