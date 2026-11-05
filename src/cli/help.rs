// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The command-line texts: every role's usage string, plus the one place a role is given its text.

use crate::Role;

pub const USAGE: &str = "usage: process-tree-guard [--extendable [--max-extension SECONDS] [--extension-log PATH] [--authenticate {desktop,console}]] \
    --timeout SECONDS (--shell {powershell,bash,cmd} --command-b64 BASE64 | [--] COMMAND [ARGS...])\n       process-tree-guard extend SECONDS\n       process-tree-guard hook [VENDOR]\n       process-tree-guard auto [--timeout SECONDS] [--shell {powershell,bash,cmd}] [--command-b64 BASE64 | [--] COMMAND...]";

/// What `process-tree-guard-deadline -h` prints.
pub const DEADLINE_USAGE: &str = "usage: process-tree-guard-deadline [--extendable [--max-extension SECONDS] [--extension-log PATH] [--authenticate {desktop,console}]] \
    --timeout SECONDS (--shell {powershell,bash,cmd} --command-b64 BASE64 | [--] COMMAND [ARGS...])\n       process-tree-guard-deadline extend SECONDS\n       process-tree-guard-deadline auto [--timeout SECONDS] [--shell {powershell,bash,cmd}] [--command-b64 BASE64 | [--] COMMAND...]";

/// What `process-tree-guard-contain -h` prints.
pub const CONTAIN_USAGE: &str = "usage: process-tree-guard-contain --timeout SECONDS (--shell {powershell,bash,cmd} --command-b64 BASE64 | [--] COMMAND [ARGS...])  (runs only inside an extendable run of process-tree-guard; the timeout is a request to that run)";

/// What `process-tree-guard-gate -h` prints: the gate takes no deadline, only a vendor.
pub const GATE_USAGE: &str = "usage: process-tree-guard-gate [VENDOR]  (VENDOR: pretooluse, cursor, cursor-context, antigravity, antigravity-context or powershell-auto; reads the hook payload on standard input; the default is pretooluse)";

/// Optional help is captured; mandatory protocol continues through say.
pub fn help(text: &str) {
    crate::diagnostics::stdout_line(text);
}

/// The usage text of a role: the deadline role's own
/// text, and the full text for every other role.
pub fn usage_for(role: Role) -> &'static str {
    if role == Role::Deadline { DEADLINE_USAGE } else { USAGE }
}
