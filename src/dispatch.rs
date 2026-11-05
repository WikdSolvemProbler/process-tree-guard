// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Role dispatch: the choice of which role behaviour a process performs, from its arguments or its own file name.

use crate::{cli, cli_process, diagnostics, engine, plugins, GuardError, Role, EXIT_GUARD_ERROR};
use std::ffi::OsString;

/// The arguments after the first `skip` as text, or an error when one is not Unicode.
fn text_args(args: impl Iterator<Item = OsString>) -> Result<Vec<String>, GuardError> {
    args
        .map(|arg| arg.into_string().map_err(|_: OsString| GuardError("arguments must be valid Unicode".into())))
        .collect()
}

/// Report an outcome as the exit code of the process.
pub(crate) fn finish(result: Result<u32, GuardError>) -> i32 {
    match result {
        Ok(code) => code as i32,
        Err(error) => {
            // Preserve exact CLI text as owned output; only cli_exit starts its writer.
            diagnostics::stderr_owned(format!("PROCESS_GUARD_ERROR: {}", error));
            EXIT_GUARD_ERROR
        }
    }
}

pub(crate) fn entry_gate() -> i32 {
    engine::trace("start");
    let mut args = cli_process::take_arguments();
    let first = args.next().and_then(|v| v.into_string().ok());
    if matches!(first.as_deref(), Some("-h" | "--help")) {
        cli::help(cli::GATE_USAGE);
        return 0;
    }
    // A further argument after the vendor is refused through the same
    // fail-loud path as an unrecognised vendor, instead of being
    // discarded in silence.
    if args.next().is_some() {
        return plugins::usage_error(Role::Gate);
    }
    // The default vendor is a static text: borrowing it (or the
    // argument's own text) names it with no string built for it,
    // where materialising the default allocated on every gate call
    // that named no vendor.
    plugins::main(first.as_deref().unwrap_or(crate::plugins::DEFAULT_VENDOR), Role::Gate)
}

pub(crate) fn entry_contain() -> i32 {
    engine::trace("start");
    let args = cli_process::take_arguments();
    finish(text_args(args).and_then(cli::run_contain_args))
}

// The `-auto` suffix test runs on the stem's encoded bytes in place: the
// suffix is plain ASCII, so no lossy conversion or lowered copy of the
// file name is allocated for a check every run-mode call pays.
fn auto_by_name_via_std() -> bool {
    std::env::current_exe().ok().is_some_and(|p| {
        p.file_stem().is_some_and(|stem| {
            let bytes = stem.as_encoded_bytes();
            bytes.len() >= 5 && bytes[bytes.len() - 5..].eq_ignore_ascii_case(b"-auto")
        })
    })
}

// The same suffix test on the image name as `GetModuleFileNameW` writes it:
// the stem is the part of the file name before its final dot (a name that is
// exactly `..`, or whose only dot leads it, is its own stem, as `file_stem`
// has it), and its last five units are compared in place. The suffix is
// plain ASCII, so a unit matches exactly when the stem's encoded byte at
// that place would: an ASCII character is one unit and one byte in both
// forms, and a non-ASCII unit is neither.
fn stem_ends_with_auto(units: &[u16]) -> bool {
    let start = units.iter().rposition(|&unit| unit == b'\\' as u16 || unit == b'/' as u16)
        .map_or(0, |separator| separator + 1);
    let name = &units[start..];
    if name.is_empty() {
        return false;
    }
    let stem = if name == [b'.' as u16, b'.' as u16] {
        name
    } else {
        match name.iter().rposition(|&unit| unit == b'.' as u16) {
            Some(0) | None => name,
            Some(dot) => &name[..dot],
        }
    };
    stem.len() >= 5 && stem[stem.len() - 5..].iter()
        .zip(b"-auto").all(|(&unit, &byte)| unit < 128 && (unit as u8).eq_ignore_ascii_case(&byte))
}

fn auto_by_name() -> bool {
    // The image name is read into stack storage: only the units the call
    // reports are read, so the storage needs no clearing first. A name that
    // does not fit (the call then reports the size it was given) or cannot
    // be read at all takes the std call above, whose growth loop settles
    // both cases exactly as it did before.
    let mut buffer = [std::mem::MaybeUninit::<u16>::uninit(); 1024];
    let units = match crate::sys::windows::image_name_units(&mut buffer) {
        Some(units) => units,
        None => return auto_by_name_via_std(),
    };
    stem_ends_with_auto(units)
}

pub(crate) fn entry_full() -> i32 {
    engine::trace("start");
    let mut args = cli_process::take_arguments();
    let first = args.next();
    // Explicit hook mode wins over an automatic filename.
    if first.as_deref() == Some(std::ffi::OsStr::new("hook")) {
        let vendor = args.next().and_then(|v| v.into_string().ok());
        // A further argument after the vendor is refused through the
        // same fail-loud path as an unrecognised vendor, instead of
        // being discarded in silence.
        if args.next().is_some() {
            return plugins::usage_error(Role::Full);
        }
        return plugins::main(vendor.as_deref().unwrap_or(crate::plugins::DEFAULT_VENDOR), Role::Full);
    }
    // Explicit `auto` and `extend` win over an automatic filename, as
    // `hook` does above: a *-auto file invoked with `auto` or `extend` as
    // its first argument takes the subcommand, not automatic mode.
    match first.as_deref().and_then(|a| a.to_str()) {
        Some("auto") => return finish(text_args(args).and_then(|a| cli::auto(a, Role::Full))),
        Some("extend") => return finish(text_args(args).and_then(|args| cli::extend_command(args, Role::Full))),
        _ => {}
    }
    if auto_by_name() {
        return finish(text_args(first.into_iter().chain(args)).and_then(|a| cli::auto(a, Role::Full)));
    }
    finish(text_args(first.into_iter().chain(args)).and_then(|args| cli::run_args(args, Role::Full)))
}

pub(crate) fn entry_deadline() -> i32 {
    engine::trace("start");
    let mut args = cli_process::take_arguments();
    let first = args.next();
    // Explicit `auto` and `extend` win over an automatic filename: a
    // *-auto file invoked with `auto` or `extend` as its first argument
    // takes the subcommand, not automatic mode.
    match first.as_deref().and_then(|a| a.to_str()) {
        Some("auto") => return finish(text_args(args).and_then(|a| cli::auto(a, Role::Deadline))),
        Some("extend") => return finish(text_args(args).and_then(|args| cli::extend_command(args, Role::Deadline))),
        _ => {}
    }
    if auto_by_name() {
        return finish(text_args(first.into_iter().chain(args)).and_then(|a| cli::auto(a, Role::Deadline)));
    }
    finish(text_args(first.into_iter().chain(args)).and_then(|args| cli::run_args(args, Role::Deadline)))
}
