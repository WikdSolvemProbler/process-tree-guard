// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! `process-tree-guard hook VENDOR`: the command envelope check for agent shell tools.
//!
//! Reads the host's pre-execution hook payload as JavaScript Object Notation
//! (JSON) on standard input and allows the command only when it is exactly the
//! guard envelope:
//!
//! ```text
//! <this executable> --timeout SECONDS --shell powershell|bash|cmd --command-b64 BASE64
//! ```
//!
//! with forward slashes in the executable path, an optional leading `& `, a
//! timeout above zero and at most 86400, and a payload that decodes to nonempty
//! UTF-8 with no NUL. Anything else is denied with exit code 2, which is how a
//! host hook blocks a tool call; this is the one place the guard uses exit 2 on purpose.
//!
//! The executable has two names, `process-tree-guard` and the earlier alias
//! `process-guard`, kept as an alias. When it is installed under either
//! name, the envelope may name either name in the same folder, and the automatic
//! wrapper is the one of the executable's own name.
//!
//! The hook role, `process-tree-guard-gate`, runs the same modes. It never runs a
//! command itself, so an envelope never names it: the envelope names a program
//! that enforces the deadline (`process-tree-guard`, `process-guard` or
//! `process-tree-guard-deadline`) in the gate's own folder or, when the gate is
//! in a folder named `libexec` (in any letter case), in the `bin` folder beside it. Its automatic
//! wrapper is `process-tree-guard-deadline-auto` in its own folder. Every path is
//! built from the gate's own location and never from `PATH`. The rules follow the
//! program, not its file name: the hook role keeps them under any file name, and
//! never accepts its own path whatever it is called, while the full command keeps
//! the rules of its name even when it is called `process-tree-guard-gate`.
//!
//! Formats, and the host each one serves:
//!
//! - `pretooluse` serves the hosts whose pre-execution hook speaks the
//!   PreToolUse protocol in this shape — Claude Code and Codex: it reads
//!   `tool_input.command` (or `tool_input.cmd`) and answers in the
//!   PreToolUse hook format: an allow is an empty JSON object with
//!   exit code 0 (no decision, which is how the format allows), and a
//!   deny is a `hookSpecificOutput` PreToolUse decision
//!   (`permissionDecision` of deny, with its reason) with exit code 2.
//!   The payload carries nothing that tells the two
//!   hosts apart, and the code does not try to: one vendor serves both.
//! - `cursor` serves Cursor: it reads the top-level `command` and
//!   answers in Cursor's `beforeShellExecution` format (a `permission`
//!   of allow or deny; the deny also carries `user_message` and
//!   `agent_message`, the allow carries neither).
//! - `cursor-context` also serves Cursor: it prints the policy as
//!   session context, in the `additional_context` form.
//!
//! `antigravity` serves Google Antigravity, and is different: it rewrites instead of only checking. It reads a
//! Google Antigravity PreToolUse payload, and for the `run_command` tool replaces
//! `CommandLine` with a call to the automatic wrapper beside this executable (the
//! wrapper of its own name), so the command runs under the guard. Every outcome, denials included, is a JSON decision
//! on standard output and the exit code is always 0. `antigravity-context` prints the
//! policy as a Google Antigravity SessionStart system message.
//!
//! `powershell-auto` serves Claude Code: it also rewrites, and reads that
//! host's PreToolUse payload for its PowerShell tool — the watched
//! `permission_mode` is Claude Code's, and `bypassPermissions` is the
//! value that makes its answer `allow` instead of `ask`. It replaces
//! `tool_input.command` with a call to the automatic wrapper (returned
//! as the decision's `updatedInput`), so the command runs under the guard.
//! A payload it cannot classify (not JSON, no `tool_input`, no string
//! `command`, or its own location unknown) is denied in the same
//! PreToolUse form, still at exit code 0: this vendor never passes a
//! payload through in silence.

use crate::Role;
use std::io::Read;

/// The fallthrough vendor the dispatch assumes when no vendor is named.
pub(crate) const DEFAULT_VENDOR: &str = "pretooluse";

mod antigravity;
mod powershell_auto;
mod verdict;

pub(crate) use crate::hook::*;
pub(crate) use antigravity::*;
pub(crate) use powershell_auto::*;
/// Run the hook as `role`: the hook role (`Role::Gate`) or any other program, whose
/// rules then follow its file name. Returns the process exit code: 0 allows, 2
/// blocks (the `antigravity` vendors and `powershell-auto` always return 0).
/// The fail-loud answer to a hook invocation that names no recognised
/// vendor (or a recognised vendor followed by a further argument, which
/// the entries in `lib.rs` refuse through this same path): the role's
/// usage text on standard error through the diagnostics queue, nothing
/// read from standard input, nothing written to standard output, and
/// the guard error code. For the full command the usage text is the
/// `hook` line of `USAGE`; for the gate it is `GATE_USAGE`.
pub(crate) fn usage_error(role: Role) -> i32 {
    let usage = match role {
        Role::Gate => crate::cli::GATE_USAGE,
        _ => crate::cli::USAGE.lines().find(|line| line.contains("process-tree-guard hook")).map_or(crate::cli::USAGE, |line| line.trim()),
    };
    crate::diagnostics::stderr_owned(usage.to_string());
    crate::EXIT_GUARD_ERROR
}

pub fn main(vendor: &str, role: Role) -> i32 {
    // A vendor name that is none of the six GATE_USAGE names is a wiring
    // error, not a payload: refuse it loudly instead of answering it in
    // the `pretooluse` format, which a host that speaks another format
    // would not understand. The check stands before every read of
    // standard input, so an unrecognised name reaches no scan.
    if !matches!(vendor, "pretooluse" | "cursor" | "cursor-context" | "antigravity" | "antigravity-context" | "powershell-auto") {
        return usage_error(role);
    }
    if vendor == "powershell-auto" {
        // No spelling is resolved here: `powershell_auto` resolves
        // it itself, after the payload's command has been extracted
        // and before its path set is built, so a payload that is not
        // a tool call with a string command pays nothing for it.
        return powershell_auto(role);
    }
    if vendor == "antigravity" {
        return antigravity_entry(role);
    }
    if vendor == "antigravity-context" {
        return antigravity_context();
    }
    if vendor == "cursor-context" {
        return verdict::cursor_context();
    }
    // The verdict vendors resolve no spelling ahead of the payload
    // walk: the scan behind `lazy` resolves the executable's
    // spelling on first demand, at the first watched string, and a
    // payload with no watched string is denied without it. An
    // allowed verdict always scanned a watched string, so its one
    // query is already paid here; the deny path below resolves the
    // spelling through the same slot when the scan never demanded
    // it, once, for its reason.
    let lazy = LazyPaths::new(role);
    let allowed = verdict::check(vendor == "cursor", &lazy);
    if allowed {
        say(verdict::allow_answer(vendor == "cursor"));
        return 0;
    }
    // The deny text is built only here, on the deny path: the allow path and
    // the rewriting and context vendors above never read it, and building it
    // cost a policy-path string and a formatted string on every call.
    let reason = verdict::deny_reason(&lazy);
    // The decision is the exit code, and a host that blocks on exit 2 must get it whether or not
    // either stream can be written: with panic = abort a failed `println!` or `eprintln!` ends the
    // process with a crash code, and a host that reads any other code as a non-blocking error lets
    // the command run (Common Weakness Enumeration, CWE-636).
    say(&verdict::deny_answer(vendor == "cursor", &reason));
    // The deny reason is mandatory protocol output, not an optional
    // diagnostic: write it to standard error synchronously here instead of
    // queueing it for the diagnostic writer thread.
    crate::cli_process::write_stderr_line(&reason);
    crate::EXIT_HOOK_DENY
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The role a program has when it is called by `exe`: the hook role only under its own file name.
    fn role_of(exe: &str) -> Role {
        if exe.ends_with("/process-tree-guard-gate.exe") { Role::Gate } else { Role::Full }
    }

    // The rules below are written for programs under their own file names, so these
    // shadow the functions that take the role explicitly; the checks of a renamed
    // program call them as `super::...` with the role.
    fn valid_envelope(command: &str, exe: &str) -> bool {
        super::valid_envelope(command, exe, role_of(exe))
    }

    fn envelope_paths(exe: &str) -> Vec<String> {
        super::envelope_paths(exe, role_of(exe))
    }

    fn wrapper_paths(exe: &str) -> Option<Vec<String>> {
        super::wrapper_paths(exe, role_of(exe))
    }

    fn antigravity_guarded(command: &str, exe: &str) -> bool {
        super::antigravity_guarded(command, &super::path_set(exe, role_of(exe)))
    }

    fn path_set(exe: &str) -> PathSet<'_> {
        super::path_set(exe, role_of(exe))
    }

    fn antigravity_decision(set: &PathSet<'_>, input: Option<&str>, wrapper_exists: bool) -> String {
        let scanned = input.and_then(|text| super::antigravity_scan(text.as_bytes(), Some(text.len())));
        super::antigravity_decision(set, scanned, wrapper_exists, false).into_owned()
    }

    const EXE: &str = "C:/Tools/process-tree-guard/bin/process-guard.exe";

    #[test]
    fn envelope_accepts_only_the_exact_form() {
        let b64 = "ZWNobyBPSw=="; // "echo OK"
        let good = format!("{} --timeout 60 --shell powershell --command-b64 {}", EXE, b64);
        assert!(valid_envelope(&good, EXE));
        assert!(valid_envelope(&format!("& {}", good), EXE));
        assert!(valid_envelope(&format!("  {}  ", good), EXE));
        assert!(valid_envelope(&good.replace("60", "0.5"), EXE));
        for bad in [
            "echo bad".to_string(),
            format!("{}; echo bypass", good),
            format!("{}\necho bypass", good),
            format!("echo {}", good),
            good.replace("--timeout 60", "--timeout 0"),
            good.replace("--timeout 60", "--timeout 86401"),
            good.replace("--timeout 60", "--timeout .5"),
            good.replace("--timeout 60", "--timeout 1e3"),
            good.replace("--timeout 60", "--timeout -1"),
            good.replace("--timeout 60 ", ""),
            good.replace(b64, "////"),
            good.replace(b64, "AA=="),
            good.replace(b64, "IA=="),
            good.replace(b64, "ZWNobyBPSw"),
            good.replace(b64, "ZWNobyBPSw==="),
            good.replace("powershell", "zsh"),
            good.replace(EXE, "C:/Tools/process-tree-guard/bin/process-guard.exe.evil"),
            good.replace(EXE, "c:/tools/process-tree-guard/bin/process-guard.exe"),
            good.replace("--shell powershell", "--shell  powershell"),
            format!("{} | more", good),
            format!("{} --extra", good),
        ] {
            assert!(!valid_envelope(&bad, EXE), "{}", bad);
        }
    }

    const WRAPPER: &str = "C:/Tools/process-tree-guard/bin/process-guard-auto.exe";

    #[test]
    fn base64_encoder_round_trips_through_the_decoder() {
        assert_eq!(crate::base64::base64_encode(b"f"), "Zg==");
        assert_eq!(crate::base64::base64_encode(b"fo"), "Zm8=");
        assert_eq!(crate::base64::base64_encode(b"foo"), "Zm9v");
        assert_eq!(crate::base64::base64_encode(b"foob"), "Zm9vYg==");
        for text in ["a", "ab", "abc", "abcd", "echo \"q\" 'r' \\ s\nt","echo \u{fc}\u{4e2d}\u{1f600}"] {
            assert_eq!(crate::base64::decode_command(&crate::base64::base64_encode(text.as_bytes())).ok().as_deref(), Some(text), "{:?}", text);
        }
    }

    #[test]
    fn antigravity_guarded_matches_only_exact_forms() {
        let good = antigravity_wrapped(WRAPPER, "echo OK");
        assert_eq!(good, format!("{} --command-b64 ZWNobyBPSw==", WRAPPER));
        assert!(antigravity_guarded(&good, EXE));
        assert!(antigravity_guarded(&format!("  {}\n", good), EXE));
        let envelope = format!("{} --timeout 60 --shell powershell --command-b64 ZWNobyBPSw==", EXE);
        assert!(antigravity_guarded(&envelope, EXE));
        for bad in [
            "echo bad".to_string(),
            format!("{}; evil", good),
            format!("{} && evil", good),
            format!("{} | more", good),
            format!("{}\nevil", good),
            format!("echo {}", good),
            format!("{}  --command-b64 ZWNobyBPSw==", WRAPPER),
            good.replace(WRAPPER, "c:/tools/process-tree-guard/bin/process-guard-auto.exe"),
            good.replace(WRAPPER, "C:/Tools/process-tree-guard/bin/other-auto.exe"),
            good.replace(WRAPPER, &format!("{}.evil", WRAPPER)),
            good.replace("ZWNobyBPSw==", "ZWNobyBPSw"),
            good.replace("ZWNobyBPSw==", "////"),
            good.replace("ZWNobyBPSw==", "IA=="),
            good.replace("ZWNobyBPSw==", ""),
            format!("{} --command-b64 ", WRAPPER),
            format!("{} --shell bash --command-b64 ZWNobyBPSw==", WRAPPER),
            format!("{}; evil", envelope),
            // Only ASCII space, tab, CR and LF are separators to a shell.
            format!("{}\u{a0}", good),
            format!("\u{a0}{}", good),
            format!("{}\u{b}", good),
            format!("{}\u{2028}", good),
            format!("{}\u{a0}", envelope),
            format!("\u{b}{}", envelope),
        ] {
            assert!(!antigravity_guarded(&bad, EXE), "{:?}", bad);
        }
    }

    const TREE_EXE: &str = "C:/Tools/process-tree-guard/bin/process-tree-guard.exe";
    const TREE_WRAPPER: &str = "C:/Tools/process-tree-guard/bin/process-tree-guard-auto.exe";

    #[test]
    fn both_names_are_one_family() {
        assert_eq!(wrapper_paths(EXE), Some(vec![WRAPPER.to_string(), TREE_WRAPPER.to_string()]));
        assert_eq!(wrapper_paths(TREE_EXE), Some(vec![TREE_WRAPPER.to_string(), WRAPPER.to_string()]));
        // Another name keeps the earlier name's wrapper alone; a path without a folder has no wrapper.
        assert_eq!(wrapper_paths("C:/Tools/pg/bin/other.exe"), Some(vec!["C:/Tools/pg/bin/process-guard-auto.exe".to_string()]));
        assert_eq!(wrapper_paths("process-guard.exe"), None);
        assert_eq!(envelope_paths(EXE), vec![EXE.to_string(), TREE_EXE.to_string()]);
        assert_eq!(envelope_paths(TREE_EXE), vec![TREE_EXE.to_string(), EXE.to_string()]);
        assert_eq!(envelope_paths("C:/Tools/pg/bin/other.exe"), vec!["C:/Tools/pg/bin/other.exe".to_string()]);
        // The automatic copies are not envelope names.
        assert_eq!(envelope_paths(WRAPPER).len(), 1);
    }

    #[test]
    fn envelope_accepts_either_name_in_the_same_folder_only() {
        let b64 = "ZWNobyBPSw==";
        for (exe, other) in [(EXE, TREE_EXE), (TREE_EXE, EXE)] {
            assert!(valid_envelope(&format!("{} --timeout 60 --shell bash --command-b64 {}", exe, b64), exe));
            assert!(valid_envelope(&format!("& {} --timeout 60 --shell bash --command-b64 {}", other, b64), exe));
            for bad in [
                other.replace("/bin/", "/other/"),
                other.to_uppercase(),
                other.replace(".exe", "-auto.exe"),
                format!("{}.evil", other),
                other.replace("process-", "xprocess-"),
            ] {
                let line = format!("{} --timeout 60 --shell bash --command-b64 {}", bad, b64);
                assert!(!valid_envelope(&line, exe), "{}", line);
            }
        }
        // An executable under any other name allows only its own path.
        let own = "C:/Tools/pg/bin/other.exe";
        assert!(valid_envelope(&format!("{} --timeout 5 --shell cmd --command-b64 {}", own, b64), own));
        assert!(!valid_envelope(&format!("C:/Tools/pg/bin/process-guard.exe --timeout 5 --shell cmd --command-b64 {}", b64), own));
    }

    const GATE_EXE: &str = "C:/Tools/ptg/libexec/process-tree-guard-gate.exe";
    const FLAT_GATE_EXE: &str = "C:/Tools/ptg/bin/process-tree-guard-gate.exe";

    #[test]
    fn the_gate_vouches_for_the_programs_that_enforce_a_deadline() {
        // Install layout: its own folder, then the bin folder beside libexec. Never itself.
        let paths = envelope_paths(GATE_EXE);
        assert_eq!(paths, [
            "C:/Tools/ptg/libexec/process-tree-guard.exe",
            "C:/Tools/ptg/libexec/process-guard.exe",
            "C:/Tools/ptg/libexec/process-tree-guard-deadline.exe",
            "C:/Tools/ptg/bin/process-tree-guard.exe",
            "C:/Tools/ptg/bin/process-guard.exe",
            "C:/Tools/ptg/bin/process-tree-guard-deadline.exe",
        ]);
        // Flat layout: its own folder only.
        assert_eq!(envelope_paths(FLAT_GATE_EXE).len(), 3);
        assert!(envelope_paths(FLAT_GATE_EXE).iter().all(|p| p.starts_with("C:/Tools/ptg/bin/")));
        for exe in [GATE_EXE, FLAT_GATE_EXE] {
            assert!(!envelope_paths(exe).contains(&exe.to_string()));
            assert!(!valid_envelope(&format!("{} --timeout 60 --shell bash --command-b64 ZWNobyBPSw==", exe), exe));
        }
        let b64 = "ZWNobyBPSw==";
        let line = |path: &str| format!("{} --timeout 60 --shell bash --command-b64 {}", path, b64);
        assert!(valid_envelope(&line("C:/Tools/ptg/bin/process-tree-guard.exe"), GATE_EXE));
        assert!(valid_envelope(&line("C:/Tools/ptg/libexec/process-tree-guard-deadline.exe"), GATE_EXE));
        for bad in [
            "C:/Tools/ptg/bin/process-tree-guard-auto.exe",
            "C:/Tools/ptg/other/process-tree-guard.exe",
            "C:/Tools/ptg/bin/PROCESS-TREE-GUARD.EXE",
            "C:/Tools/ptg/libexec/process-tree-guard-deadline-auto.exe",
            "C:/Tools/ptg/bin/process-tree-guard.exe.evil",
            "C:/Tools/other/bin/process-tree-guard.exe",
        ] {
            assert!(!valid_envelope(&line(bad), GATE_EXE), "{}", bad);
        }
        // The flat gate does not reach into a sibling bin folder.
        assert!(!valid_envelope(&line("C:/Tools/ptg/libexec/process-tree-guard.exe"), FLAT_GATE_EXE));
        // Windows folder names are not case sensitive: libexec is recognised in any letter case, and
        // the rest of the path is kept as the gate was started.
        for folder in ["LIBEXEC", "LibExec", "Libexec"] {
            let gate = format!("C:/Tools/ptg/{}/process-tree-guard-gate.exe", folder);
            assert_eq!(envelope_paths(&gate).len(), 6, "{}", gate);
            assert!(valid_envelope(&line("C:/Tools/ptg/bin/process-tree-guard.exe"), &gate), "{}", gate);
            assert!(valid_envelope(&line(&format!("C:/Tools/ptg/{}/process-tree-guard-deadline.exe", folder)), &gate), "{}", gate);
        }
        // A folder that only contains or starts or ends with the word is not libexec.
        for folder in ["libexec2", "xlibexec", "my-libexec", "libexe", "libexec "] {
            let gate = format!("C:/Tools/ptg/{}/process-tree-guard-gate.exe", folder);
            assert_eq!(envelope_paths(&gate).len(), 3, "{}", gate);
            assert!(!valid_envelope(&line("C:/Tools/ptg/bin/process-tree-guard.exe"), &gate), "{}", gate);
        }
        // Another file name keeps its own path only, as before.
        assert_eq!(envelope_paths("C:/Tools/ptg/libexec/other.exe"), vec!["C:/Tools/ptg/libexec/other.exe".to_string()]);
    }

    #[test]
    fn the_rules_follow_the_role_not_the_file_name() {
        let b64 = "ZWNobyBPSw==";
        let line = |path: &str| format!("{} --timeout 60 --shell bash --command-b64 {}", path, b64);
        // The hook role under other names, one of them a name of the full command: still the gate's
        // rules, and never itself.
        for renamed in [
            "C:/Tools/ptg/libexec/hookgate.exe",
            "C:/Tools/ptg/libexec/process-guard.exe",
            "C:/Tools/ptg/libexec/process-tree-guard-deadline.exe",
            "C:/Tools/ptg/libexec/process-tree-guard-gate-auto.exe",
        ] {
            let paths = super::envelope_paths(renamed, Role::Gate);
            assert!(!paths.contains(&renamed.to_string()), "{}", renamed);
            assert!(!super::valid_envelope(&line(renamed), renamed, Role::Gate), "{}", renamed);
            assert!(super::valid_envelope(&line("C:/Tools/ptg/bin/process-tree-guard.exe"), renamed, Role::Gate), "{}", renamed);
            assert_eq!(super::wrapper_paths(renamed, Role::Gate).and_then(|w| w.into_iter().next()).as_deref(),
                Some("C:/Tools/ptg/libexec/process-tree-guard-deadline-auto.exe"), "{}", renamed);
        }
        // Started under another letter case, the hook role still never vouches for its own path.
        let cased = "C:/Tools/ptg/libexec/Process-Tree-Guard-Deadline.exe";
        assert!(!super::valid_envelope(&line("C:/Tools/ptg/libexec/process-tree-guard-deadline.exe"), cased, Role::Gate));
        assert!(!super::valid_envelope(&line(cased), cased, Role::Gate));
        assert!(super::valid_envelope(&line("C:/Tools/ptg/bin/process-tree-guard.exe"), cased, Role::Gate));
        // The deadline role's path is among the others, and only the hook role's own path leaves the list.
        assert_eq!(super::envelope_paths("C:/Tools/ptg/libexec/process-tree-guard-deadline.exe", Role::Gate).len(), 5);
        assert_eq!(super::envelope_paths("C:/Tools/ptg/libexec/hookgate.exe", Role::Gate).len(), 6);
        // A program that is not the hook role keeps the rules of its name, even under the hook role's file name.
        assert_eq!(super::envelope_paths(GATE_EXE, Role::Full), vec![GATE_EXE.to_string()]);
        assert!(super::valid_envelope(&line(GATE_EXE), GATE_EXE, Role::Full));
        assert_eq!(super::wrapper_paths(GATE_EXE, Role::Full), Some(vec!["C:/Tools/ptg/libexec/process-guard-auto.exe".to_string()]));
        // No folder: the hook role vouches for nothing.
        assert!(super::envelope_paths("gate.exe", Role::Gate).is_empty());
    }

    #[test]
    fn the_policy_file_is_one_folder_above_the_executables_folder() {
        for (exe, policy) in [
            ("C:/Tools/ptg/bin/process-tree-guard.exe", Some("C:/Tools/ptg/POLICY.md")),
            ("C:/Tools/ptg/libexec/process-tree-guard-gate.exe", Some("C:/Tools/ptg/POLICY.md")),
            // A folder called bin or libexec earlier in the path does not move the policy file.
            ("C:/Tools/bin/ptg/libexec/process-tree-guard-gate.exe", Some("C:/Tools/bin/ptg/POLICY.md")),
            ("C:/Tools/libexec/ptg/bin/process-tree-guard.exe", Some("C:/Tools/libexec/ptg/POLICY.md")),
            // A folder with neither name.
            ("C:/Tools/ptg/flat/process-tree-guard-gate.exe", Some("C:/Tools/ptg/POLICY.md")),
            ("C:/gate.exe", None),
            ("gate.exe", None),
        ] {
            assert_eq!(policy_path(exe).as_deref(), policy, "{}", exe);
        }
    }

    #[test]
    fn the_gate_rewrites_to_the_automatic_deadline_role_in_its_own_folder() {
        assert_eq!(wrapper_paths(GATE_EXE), Some(vec![
            "C:/Tools/ptg/libexec/process-tree-guard-deadline-auto.exe".to_string(),
            "C:/Tools/ptg/libexec/process-tree-guard-auto.exe".to_string(),
            "C:/Tools/ptg/libexec/process-guard-auto.exe".to_string(),
        ]));
        let own = "C:/Tools/ptg/libexec/process-tree-guard-deadline-auto.exe";
        let input = call(&quote("echo hi"));
        let d = parse_json(&antigravity_decision(&path_set(GATE_EXE), Some(&input), true)).expect("one JSON object");
        let line = d.get("overwrite").and_then(|o| o.get("CommandLine")).and_then(Json::as_str).unwrap();
        assert_eq!(line, antigravity_wrapped(own, "echo hi"));
        // The wrapper forms of all three families pass through unchanged.
        for wrapper in wrapper_paths(GATE_EXE).unwrap() {
            let input = call(&quote(&antigravity_wrapped(&wrapper, "echo hi")));
            let d = parse_json(&antigravity_decision(&path_set(GATE_EXE), Some(&input), true)).unwrap();
            assert_eq!(d.get("decision").and_then(Json::as_str), Some("allow"), "{}", wrapper);
            assert!(d.get("overwrite").is_none(), "{}", wrapper);
        }
        // A missing wrapper is reported by the gate's own wrapper name.
        let missing = parse_json(&antigravity_decision(&path_set(GATE_EXE), Some(&call("\"echo hi\"")), false)).unwrap();
        assert_eq!(missing.get("decision").and_then(Json::as_str), Some("deny"));
        assert!(missing.get("reason").and_then(Json::as_str).map_or(false, |r| r.contains("process-tree-guard-deadline-auto.exe")));
    }

    #[test]
    fn antigravity_guarded_accepts_both_names_wrappers_and_envelopes() {
        let b64 = "ZWNobyBPSw==";
        for exe in [EXE, TREE_EXE] {
            for wrapper in [WRAPPER, TREE_WRAPPER] {
                assert!(antigravity_guarded(&format!("{} --command-b64 {}", wrapper, b64), exe), "{} {}", exe, wrapper);
            }
            for path in [EXE, TREE_EXE] {
                assert!(antigravity_guarded(&format!("{} --timeout 60 --shell powershell --command-b64 {}", path, b64), exe), "{} {}", exe, path);
            }
            for bad in [
                format!("{} --command-b64 {}", WRAPPER.to_uppercase(), b64),
                format!("{} --command-b64 {}", TREE_WRAPPER.replace("/bin/", "/other/"), b64),
                format!("{}.evil --command-b64 {}", TREE_WRAPPER, b64),
                format!("{} --command-b64 {}; evil", TREE_WRAPPER, b64),
            ] {
                assert!(!antigravity_guarded(&bad, exe), "{}", bad);
            }
        }
    }

    #[test]
    fn antigravity_decision_uses_the_wrapper_of_its_own_name() {
        let decide_for = |exe: &str, original: &str| {
            let input = call(&quote(original));
            parse_json(&antigravity_decision(&path_set(exe), Some(&input), true)).expect("one JSON object")
        };
        for (exe, wrapper) in [(EXE, WRAPPER), (TREE_EXE, TREE_WRAPPER)] {
            let d = decide_for(exe, "echo hi");
            let line = d.get("overwrite").and_then(|o| o.get("CommandLine")).and_then(Json::as_str).unwrap();
            assert_eq!(line, antigravity_wrapped(wrapper, "echo hi"));
            // The other name's wrapper passes through unchanged.
            let other = if wrapper == WRAPPER { TREE_WRAPPER } else { WRAPPER };
            let d = decide_for(exe, &antigravity_wrapped(other, "echo hi"));
            assert_eq!(d.get("decision").and_then(Json::as_str), Some("allow"));
            assert!(d.get("overwrite").is_none());
        }
        // A missing wrapper is reported by its own name.
        let missing = parse_json(&antigravity_decision(&path_set(TREE_EXE), Some(&call("\"echo hi\"")), false)).unwrap();
        assert_eq!(missing.get("decision").and_then(Json::as_str), Some("deny"));
        assert!(missing.get("reason").and_then(Json::as_str).map_or(false, |r| r.contains("process-tree-guard-auto.exe")));
    }

    fn call(command_json: &str) -> String {
        format!("{{\"toolCall\":{{\"name\":\"run_command\",\"args\":{{\"CommandLine\":{}}}}},\"stepIdx\":1}}", command_json)
    }

    fn decide(input: &str) -> Json {
        parse_json(&antigravity_decision(&path_set(EXE), Some(input), true)).expect("one JSON object")
    }

    #[test]
    fn antigravity_decision_rewrites_allows_and_denies() {
        // A plain command is rewritten; the original is recoverable from the payload exactly.
        let original = "echo \"q\" 'r' \\ s\n\u{fc}";
        let d = decide(&call(&quote(original)));
        assert_eq!(d.get("decision").and_then(Json::as_str), Some("allow"));
        let rewritten = d.get("overwrite").and_then(|o| o.get("CommandLine")).and_then(Json::as_str).unwrap();
        let b64 = rewritten.strip_prefix(&format!("{} --command-b64 ", WRAPPER)).expect("wrapper form");
        assert_eq!(crate::base64::decode_command(b64).ok().as_deref(), Some(original));
        assert!(matches!(d.get("overwrite"), Some(Json::Object(pairs)) if pairs.len() == 1));
        // Fed back, the wrapper form passes unchanged.
        let again = decide(&call(&quote(rewritten)));
        assert_eq!(again.get("decision").and_then(Json::as_str), Some("allow"));
        assert!(again.get("overwrite").is_none());
        // A tool with another string name is not this hook's business.
        for input in [r#"{"toolCall":{"name":"view_file","args":{}}}"#, r#"{"toolCall":{"name":"run_command ","args":{}}}"#, r#"{"toolCall":{"name":""}}"#] {
            let d = decide(input);
            assert_eq!(d.get("decision").and_then(Json::as_str), Some("allow"), "{}", input);
            assert!(d.get("overwrite").is_none(), "{}", input);
        }
        // Everything that cannot be wrapped safely, or is not a recognised payload, is denied.
        for input in [
            "", "not json", "{",
            "{}", "[]", "null", r#"{"toolCall":null}"#, r#"{"toolCall":[]}"#, r#"{"toolCall":"run_command"}"#,
            r#"{"toolCall":{"name":7}}"#, r#"{"toolCall":{"name":null}}"#,
            r#"{"toolCall":{"args":{"CommandLine":"echo a"}}}"#,
            r#"{"toolCall":{"name":"run_command","args":{}}}"#,
            r#"{"toolCall":{"name":"run_command"}}"#,
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":""}}}"#,
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":"  \n"}}}"#,
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":7}}}"#,
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":"a\u0000b"}}}"#,
            // A second key that is CommandLine in any case, or a repeated key, is ambiguous.
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":"echo hi","commandline":"evil"}}}"#,
            r#"{"toolCall":{"name":"run_command","args":{"COMMANDLINE":"evil","CommandLine":"echo hi"}}}"#,
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":"echo hi","CommandLine":"evil"}}}"#,
            // The same holds for the toolCall, name and args keys, whatever the tool.
            r#"{"toolCall":{"name":"view_file","args":{}},"toolcall":{"name":"run_command","args":{"CommandLine":"evil"}}}"#,
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":"evil"}},"toolCall":{"name":"view_file","args":{}}}"#,
            r#"{"toolCall":{"name":"view_file","Name":"run_command","args":{"CommandLine":"evil"}}}"#,
            r#"{"toolCall":{"name":"run_command","name":"view_file","args":{"CommandLine":"evil"}}}"#,
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":"echo hi"},"ARGS":{"CommandLine":"evil"}}}"#,
        ] {
            let d = decide(input);
            assert_eq!(d.get("decision").and_then(Json::as_str), Some("deny"), "{}", input);
            assert!(d.get("reason").and_then(Json::as_str).map_or(false, |r| r.starts_with("TIMEOUT_REQUIRED")), "{}", input);
        }
        let denied = |exe: &str, exists: bool| {
            parse_json(&antigravity_decision(&path_set(exe), Some(&call("\"echo hi\"")), exists)).and_then(|d| d.get("decision").and_then(Json::as_str).map(str::to_string))
        };
        // The longest command that fits is wrapped; one more group of three bytes is denied with a reason.
        let room = (MAX_REWRITTEN_LINE - antigravity_wrapped(WRAPPER, "").len()) / 4 * 3;
        let fits = decide(&call(&quote(&"a".repeat(room))));
        assert_eq!(fits.get("decision").and_then(Json::as_str), Some("allow"));
        assert!(fits.get("overwrite").is_some());
        let over = decide(&call(&quote(&"a".repeat(room + 1))));
        assert_eq!(over.get("decision").and_then(Json::as_str), Some("deny"));
        assert!(over.get("overwrite").is_none());
        assert!(over.get("reason").and_then(Json::as_str).map_or(false, |r| r.contains("too long") && r.contains("file")));
        assert_eq!(denied(EXE, false).as_deref(), Some("deny"));
        assert_eq!(denied("C:/Program Files/pg/bin/process-guard.exe", true).as_deref(), Some("deny"));
        assert_eq!(denied("", true).as_deref(), Some("deny"));
        assert_eq!(parse_json(&antigravity_decision(&path_set(EXE), None, true)).and_then(|d| d.get("decision").and_then(Json::as_str).map(str::to_string)).as_deref(), Some("deny"));
    }

    #[test]
    fn json_parsing() {
        let v = parse_json(r#"{"tool_input":{"command":"a\"b\\c\u00fc\ud83d\ude00","n":[1,2.5,true,null]}}"#).unwrap();
        assert_eq!(v.get("tool_input").and_then(|t| t.get("command")).and_then(Json::as_str), Some("a\"b\\cü😀"));
        for bad in ["", "{", "{\"a\":}", "{\"a\":1} x", "\"\\ud800\"", "\"a\nb\"", "[1,]"] {
            assert!(parse_json(bad).is_none(), "{:?}", bad);
        }
        let deep = "[".repeat(10000);
        assert!(parse_json(&deep).is_none());
        assert_eq!(quote("a\"\\\n\u{1}"), "\"a\\\"\\\\\\n\\u0001\"");
    }

    /// The verdict the whole-string form gives, for comparison with the scan.
    fn dom_verdict(input: &str, cursor: bool, exe: &str) -> bool {
        parse_json(input).map_or(false, |payload| {
            let command = if cursor {
                payload.get("command")
            } else {
                payload.get("tool_input").and_then(|t| t.get("command").or_else(|| t.get("cmd")))
            };
            command.and_then(Json::as_str).map_or(false, |c| !exe.is_empty() && valid_envelope(c, exe))
        })
    }

    fn scanned(input: &[u8], cursor: bool, exe: &str) -> bool {
        super::scan_verdict(input, cursor, &super::LazyPaths::fixed(exe, role_of(exe)))
    }

    #[test]
    fn streaming_verdict_matches_the_parsed_verdict() {
        let good = format!("{} --timeout 60 --shell powershell --command-b64 ZWNobyBPSw==", EXE);
        let payloads: Vec<String> = vec![
            format!("{{\"tool_input\":{{\"command\":{}}}}}", quote(&good)),
            format!("{{\"tool_input\":{{\"cmd\":{}}}}}", quote(&good)),
            format!("{{\"tool_input\":{{\"command\":7,\"cmd\":{}}}}}", quote(&good)),
            format!("{{\"tool_input\":{{\"cmd\":{}}},\"tool_input\":{{\"command\":\"echo no\"}}}}", quote(&good)),
            format!("{{\"tool_input\":{{\"command\":\"echo no\"}},\"tool_input\":{{\"cmd\":{}}}}}", quote(&good)),
            format!("{{\"tool_input\":{{\"command\":\"echo no\",\"command\":{}}}}}", quote(&good)),
            format!("{{\"tool_input\":{{\"command\":{},\"command\":\"echo no\"}}}}", quote(&good)),
            format!("{{\"command\":{}}}", quote(&good)),
            format!("{{\"tool_input\":{{\"command\":{}}}}}", quote(&format!("& {}", good))),
            format!("{{\"tool_input\":{{\"command\":{}}}}}", quote(&format!("  {} \n\t", good))),
            "{\"tool_input\":null}".into(),
            "{\"tool_input\":[]}".into(),
            "{\"tool_input\":{}}".into(),
            format!("{{\"other\":{{\"command\":{}}}}}", quote(&good)),
            format!("{{\"extra\":{{\"deep\":[[[[\"x\"]]]]}},\"tool_input\":{{\"command\":{}}}}}", quote(&good)),
            "{}".into(),
            "[]".into(),
            "null".into(),
            "\"x\"".into(),
            "".into(),
            "{".into(),
            "{\"tool_input\":{\"command\":\"a\",} garbage}".into(),
            "{\"tool_input\":{\"command\":\"a\",}}".into(),
            "{\"a\":1} trailing".into(),
            "{\"a\":1.}".into(),
            "{\"a\":.5}".into(),
            "{\"a\":1e}".into(),
            "{\"a\":1.e5}".into(),
            "{\"a\":-.5e-2}".into(),
            "{\"a\":+1}".into(),
            "{\"a\":1+2}".into(),
            "{\"a\":truex}".into(),
            "{\"a\":\"bad\\ud800\"}".into(),
            "{\"a\":\"bad\\ud800\\u0041\"}".into(),
            "{\"a\":\"ok\\ud83d\\ude00\"}".into(),
            "{\"a\":\"bad\\udc00\"}".into(),
            "{\"a\":\"bad\\u+123\"}".into(),
            "{\"tool\\u005finput\":{\"command\":\"echo no\"}}".into(),
            "{\"tool_input\":{\"comm\\u0061nd\":\"echo no\"}}".into(),
        ];
        for input in &payloads {
            for exe in [EXE, TREE_EXE, GATE_EXE, ""] {
                for cursor in [false, true] {
                    assert_eq!(scanned(input.as_bytes(), cursor, exe), dom_verdict(input, cursor, exe),
                        "exe={} cursor={} input={:?}", exe, cursor, input);
                }
            }
        }
        // Invalid UTF-8 denies, as a failed whole-string read denies.
        assert!(!scanned(&[b'{', 0xff, b'}'], false, EXE));
        assert!(!scanned(b"{\"a\":\"\xc3\"}", false, EXE));
    }

    #[test]
    fn streaming_verdict_keeps_the_payload_cap() {
        let good = format!("{} --timeout 60 --shell powershell --command-b64 ZWNobyBPSw==", EXE);
        // A whole payload padded with spaces past the cap decides as
        // its first 16 MiB decide: allowed.
        let mut padded = format!("{{\"tool_input\":{{\"command\":{}}}}}", quote(&good)).into_bytes();
        padded.resize(16 * 1024 * 1024 + 4096, b' ');
        assert!(scanned(&padded, false, EXE));
        // An unterminated payload past the cap does not.
        let mut unterminated = b"{\"tool_input\":{\"command\":\"".to_vec();
        unterminated.resize(16 * 1024 * 1024 + 4096, b'a');
        assert!(!scanned(&unterminated, false, EXE));
        // A watched string past several window fills still decides.
        let mut crossed = b"{\"x\":\"".to_vec();
        crossed.extend(std::iter::repeat(b'a').take(100_000));
        crossed.extend(format!("\",\"tool_input\":{{\"command\":{}}}}}", quote(&good)).into_bytes());
        assert!(scanned(&crossed, false, EXE));
        assert_eq!(scanned(&crossed, false, EXE), dom_verdict(std::str::from_utf8(&crossed).unwrap(), false, EXE));
    }

    #[test]
    fn streaming_verdict_with_multibyte_paths_and_window_edges() {
        // An envelope path with multibyte characters is matched
        // character by character, exactly as the parsed form matches
        // it: the scan holds each path as a character iterator, not
        // as bytes.
        let exe = "C:/Töols/pg/bin/process-guard.exe";
        let good = format!("{} --timeout 60 --shell bash --command-b64 ZWNobyBPSw==", exe);
        let input = format!("{{\"tool_input\":{{\"command\":{}}}}}", quote(&good));
        for cursor in [false, true] {
            assert_eq!(scanned(input.as_bytes(), cursor, exe), dom_verdict(&input, cursor, exe), "cursor={}", cursor);
        }
        assert!(scanned(input.as_bytes(), false, exe));
        // Unwatched strings bulk-consume plain runs per window fill.
        // Multibyte scalars straddling fill edges, escapes at edges
        // and long plain runs must all decide as the parsed form
        // decides.
        let mut filler = String::new();
        filler.push_str(&"a".repeat(16_380));
        filler.push_str("ü€😀");
        filler.push_str(&"b".repeat(40_000));
        filler.push_str("\\u00fc\\n\\\"");
        filler.push_str(&"c".repeat(40_000));
        filler.push_str(&"d".repeat(16_383));
        filler.push('é');
        filler.push_str(&"e".repeat(100));
        let crossed = format!("{{\"x\":{},\"tool_input\":{{\"command\":{}}}}}", quote(&filler), quote(&good));
        assert!(scanned(crossed.as_bytes(), false, exe));
        assert_eq!(scanned(crossed.as_bytes(), false, exe), dom_verdict(&crossed, false, exe));
        // The same shapes in a watched string and in keys, and byte
        // sequences no string may hold, keep the scan's verdict.
        let watched = format!("{{\"tool_input\":{{\"command\":{}}}}}", quote(&filler));
        assert_eq!(scanned(watched.as_bytes(), false, EXE), dom_verdict(&watched, false, EXE));
        for raw in [
            b"{\"x\":\"\xc3\"}".as_slice(),
            b"{\"x\":\"\xc0\xaf\"}".as_slice(),
            b"{\"x\":\"\xed\xa0\x80\"}".as_slice(),
            b"{\"x\":\"\xf4\x90\x80\x80\"}".as_slice(),
            b"{\"x\":\"ok\x80\"}".as_slice(),
            b"{\"x\":\"\xe2\x82\"}".as_slice(),
        ] {
            assert!(!scanned(raw, false, EXE), "{:?}", raw);
        }
    }

    #[test]
    fn streaming_verdict_timeout_tokens_short_and_long() {
        let with_token = |token: &str| {
            let command = format!("{} --timeout {} --shell bash --command-b64 ZWNobyBPSw==", EXE, token);
            format!("{{\"tool_input\":{{\"command\":{}}}}}", quote(&command))
        };
        let cases: Vec<(String, bool)> = vec![
            ("60".into(), true),
            ("0.5".into(), true),
            ("86400".into(), true),
            ("86401".into(), false),
            ("0".into(), false),
            (".5".into(), false),
            ("5.".into(), false),
            ("1e3".into(), false),
            // Tokens past the kept 32 bytes are settled by their digits.
            (format!("{}1", "0".repeat(100)), true),
            (format!("86400.{}1", "0".repeat(100)), true),
            (format!("0.{}1", "0".repeat(400)), false),
            ("9".repeat(100), false),
        ];
        for (token, want) in &cases {
            let input = with_token(token);
            assert_eq!(scanned(input.as_bytes(), false, EXE), *want, "token {}", token);
            assert_eq!(dom_verdict(&input, false, EXE), *want, "parsed token {}", token);
        }
    }
}
