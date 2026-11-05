// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The verdict family: the check-only hook vendors — one scan, one verdict, answered in each vendor's shape.

use super::*;

/// The family's one scan: the payload on standard input, walked
/// by the core scanner, its watched command checked against the
/// envelope rule. `cursor` selects the payload field the scan
/// watches. The scan mechanism stays in the core
/// (`crate::hook::scan_verdict`); its call stands here, at the
/// family's home (section 2.4: one scan, one verdict computation).
pub fn check(cursor: bool, lazy: &LazyPaths) -> bool {
    scan_verdict(crate::cli_process::standard_input(), cursor, lazy)
}

/// `hook cursor-context`: the policy as session context, in the
/// `additional_context` form. The context form carries no decision
/// logic of its own: it prints the installed policy through the
/// shared core mechanism (`write_policy`).
pub fn cursor_context() -> i32 {
    let mut out = String::with_capacity(128);
    out.push_str("{\"additional_context\":");
    write_policy(&mut out);
    out.push('}');
    say(&out);
    0
}

/// The allow answer of the verdict vendors, in the vendor's shape:
/// Cursor's `beforeShellExecution` form names the permission; the
/// PreToolUse form allows by saying nothing (an empty object).
pub fn allow_answer(cursor: bool) -> &'static str {
    if cursor { "{\"permission\":\"allow\"}" } else { "{}" }
}

/// The deny reason of the verdict vendors, spelled from the
/// executable's own path as the scan resolved it through `lazy`.
pub fn deny_reason(lazy: &LazyPaths) -> String {
    let exe = lazy.exe();
    // The policy path is spelled from the executable's two folder
    // splits in place, as two borrowed pieces of the one format:
    // building it as its own string first (`policy_path`) allocated
    // for a value the format copied out of and dropped, on every
    // denial. The pieces spell exactly what `policy_path` builds —
    // the root folder, a separator and the leaf name — and the bare
    // leaf when the executable's spelling has no two folders.
    let (root, leaf) = match exe.rsplit_once('/').and_then(|(dir, _)| dir.rsplit_once('/')) {
        Some((root, _)) => (root, "/POLICY.md"),
        None => ("", "POLICY.md"),
    };
    format!("TIMEOUT_REQUIRED: execute exactly the process-guard envelope with an explicit finite timeout. \
        No unwrapped commands, prefixes, suffixes, or polling-timeout substitutes. See {}{}. \
        Native tool timeout alone does not guarantee child cleanup.", root, leaf)
}

/// The deny answer of the verdict vendors, built in the vendor's
/// shape around `reason`. The decision is the exit code the caller
/// returns; this is only its text.
pub fn deny_answer(cursor: bool, reason: &str) -> String {
    // The reason is escaped straight into the answer buffer, as
    // `write_policy` escapes the policy into its buffer: quoting it
    // into its own string first built that string only for the
    // format to copy it back out (twice, for cursor).
    if cursor {
        // The reason is quoted once and the quoted form copied into
        // both fields: escaping it twice walked the whole reason a
        // second time for text the first walk had already produced,
        // where copying the quoted form is a plain copy of bytes
        // already escaped.
        let mut quoted = String::with_capacity(reason.len() + 2);
        write_quote(reason, &mut quoted);
        let mut out = String::with_capacity(quoted.len() * 2 + 48);
        out.push_str("{\"permission\":\"deny\",\"user_message\":");
        out.push_str(&quoted);
        out.push_str(",\"agent_message\":");
        out.push_str(&quoted);
        out.push('}');
        out
    } else {
        let mut out = String::with_capacity(reason.len() + 112);
        out.push_str("{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":");
        write_quote(reason, &mut out);
        out.push_str("}}");
        out
    }
}
