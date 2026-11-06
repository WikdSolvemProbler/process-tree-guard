// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The Antigravity vendor: rewriting a run_command call into the automatic wrapper, or denying it.

use super::*;

/// The longest rewritten `CommandLine` the hook produces, in characters. Wrapping
/// grows a command by about 4/3 plus the wrapper path, and `cmd.exe` cannot run a
/// line of more than 8191 characters; the margin leaves room for the host's own
/// `cmd.exe /d /s /c "` prefix. The hook cannot know which shell the host uses, so
/// a longer command is denied with an explanation instead of failing with a shell error.
pub(super) const MAX_REWRITTEN_LINE: usize = 8000;

/// The command line that runs `command` under the automatic wrapper at `wrapper`.
pub fn antigravity_wrapped(wrapper: &str, command: &str) -> String {
    format!("{} --command-b64 {}", wrapper, crate::base64::base64_encode(command.as_bytes()))
}

/// True when `command` is exactly the wrapper form `<wrapper> --command-b64 BASE64`
/// with a payload that decodes to a valid command, or a valid guard envelope. The
/// wrapper is any name's automatic copy beside `exe`, and the envelope names
/// any name of the executable. Nothing else counts: trailing text, other
/// separators, another path or a path in another case are wrapped again.
/// The guarded check against a path set computed once by the caller
/// (see `path_set`).
pub fn antigravity_guarded(command: &str, set: &PathSet<'_>) -> bool {
    // Shells split on ASCII space, tab and newline only; trimming anything wider
    // (a no-break space, a vertical tab) would pass a line the wrapper then rejects.
    let trimmed = command.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    set.wrappers.iter().any(|spec| {
        strip_wrapper_prefix(trimmed, *spec)
            .and_then(|r| r.strip_prefix(" --command-b64 "))
            .is_some_and(crate::base64::command_text_is_valid)
    })
        // valid_envelope trims Unicode whitespace itself; refuse what it would silently strip.
        // The refusal test reads only the two end characters: a
        // Unicode trim changes the text exactly when its first or
        // its last character is whitespace, so comparing the whole
        // trimmed length walked both edges of the command for an
        // answer its ends already give — and the envelope walk then
        // takes the text as it stands, through
        // `valid_envelope_paths_trimmed`, instead of trimming it a
        // third time inside the walk. For a command that is all
        // whitespace each of those walks was a full pass.
        || (trimmed.chars().next().is_none_or(|c| !c.is_whitespace())
            && trimmed.chars().next_back().is_none_or(|c| !c.is_whitespace())
            && valid_envelope_paths_trimmed(trimmed, &set.envelope))
}

/// An Antigravity denial whose reason the caller appends to the
/// answer buffer in pieces through `build`: literal spans pushed,
/// variable spans escaped with `write_escaped`, numbers written
/// with `write!` directly into the buffer. The reason is escaped
/// straight into the answer: formatting it into its own string
/// first built that string (one allocation and a full copy of a
/// mostly literal text) only for the escaping to copy it back out
/// character by character. The pieces escape independently — the
/// escaping is per character, with no state across a boundary —
/// so the answer is byte-for-byte the one the formatted reason
/// produced.
pub(super) fn antigravity_deny_with(build: impl FnOnce(&mut String)) -> String {
    let mut out = String::with_capacity(128);
    out.push_str("{\"decision\":\"deny\",\"reason\":\"");
    out.push_str("TIMEOUT_REQUIRED: ");
    build(&mut out);
    out.push_str(" Native tool timeout alone does not guarantee child cleanup.\"}");
    out
}

pub(super) fn antigravity_deny(why: &str) -> String {
    antigravity_deny_with(|out| write_escaped(why, out))
}

/// Watches the decoded characters of one object key against up to
/// two names at once, without holding the key: for each name it
/// tracks the exact match and the ASCII case-insensitive match, the
/// two spellings the Antigravity ambiguity rule keeps apart — the
/// hook reads the exact key, and a second key that is the same name
/// in any letter case makes the payload ambiguous.
pub(super) struct WatchKey {
    names: [Option<&'static str>; 2],
    pos: usize,
    exact_alive: [bool; 2],
    ci_alive: [bool; 2],
}

impl WatchKey {
    fn new(names: &[&'static str]) -> Self {
        let mut watch = WatchKey { names: [None, None], pos: 0, exact_alive: [false; 2], ci_alive: [false; 2] };
        for (slot, name) in watch.names.iter_mut().zip(names.iter()) {
            *slot = Some(name);
        }
        watch.exact_alive = [watch.names[0].is_some(), watch.names[1].is_some()];
        watch.ci_alive = watch.exact_alive;
        watch
    }
    pub(super) fn exact(&self, slot: usize) -> bool {
        self.exact_alive[slot] && self.names[slot].is_some_and(|name| self.pos == name.len())
    }
    pub(super) fn case_insensitive(&self, slot: usize) -> bool {
        self.ci_alive[slot] && self.names[slot].is_some_and(|name| self.pos == name.len())
    }
}

impl CharSink for WatchKey {
    fn takes_bulk(&self) -> bool {
        // Once every slot is dead under both spellings, no later
        // character can revive one (`push_char` only clears the
        // flags, and `exact` / `case_insensitive` read a slot only
        // through its flags, never through the position a dead
        // slot's count would have reached), so the rest of the
        // string is discarded exactly as the per-character form
        // discards it and may be validated whole.
        (0..2).all(|i| !self.exact_alive[i] && !self.ci_alive[i])
    }
    fn push_char(&mut self, c: char) {
        for i in 0..2 {
            let name = match self.names[i] {
                Some(name) => name,
                None => continue,
            };
            if self.pos < name.len() {
                let target = name.as_bytes()[self.pos] as char;
                if c != target {
                    self.exact_alive[i] = false;
                }
                if !chars_eq_ignore_ascii_case(c, target) {
                    self.ci_alive[i] = false;
                }
            } else {
                self.exact_alive[i] = false;
                self.ci_alive[i] = false;
            }
        }
        self.pos += 1;
    }
}

/// A bounded sink that reports whether the string fed to it is
/// exactly one name, holding no part of the string itself.
pub(super) struct MatchSink {
    name: &'static str,
    seen: usize,
    alive: bool,
}

impl MatchSink {
    fn new(name: &'static str) -> Self {
        MatchSink { name, seen: 0, alive: true }
    }
    fn matched(&self) -> bool {
        self.alive && self.seen == self.name.len()
    }
}

impl CharSink for MatchSink {
    fn takes_bulk(&self) -> bool {
        // A dead sink has already settled `matched` false, and no
        // later character revives it, so the rest of the string is
        // discarded exactly as the per-character form discards it
        // and may be validated whole.
        !self.alive
    }
    fn push_char(&mut self, c: char) {
        if self.alive && self.seen < self.name.len() && self.name.as_bytes()[self.seen] as char == c {
            self.seen += 1;
        } else {
            self.alive = false;
        }
    }
}

/// A sink that collects the decoded characters of one string into
/// the caller's string: the one string the Antigravity rewrite
/// needs, reused across candidate keys so only the last candidate
/// is ever held.
pub(super) struct Capture<'x> {
    text: &'x mut String,
}

impl CharSink for Capture<'_> {
    fn push_char(&mut self, c: char) {
        self.text.push(c);
    }
    fn captures_run(&self) -> bool {
        true
    }
    fn push_run(&mut self, text: &str) {
        self.text.push_str(text);
    }
}

/// What the scan has of the `name` field of a `toolCall` object: the
/// hook needs only whether the last exact `name` value is the string
/// `run_command`, some other string, not a string, or absent.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum NameValue {
    Missing,
    NotString,
    RunCommand,
    OtherString,
}

/// What the scan has of one `args` object: how many of its keys are
/// `CommandLine` in any letter case, and whether its last exact
/// `CommandLine` value is a string (held in the tool scan's text).
#[derive(Clone, Copy, Default)]
pub(super) struct ArgsScan {
    count_command_line: usize,
    has_command: bool,
    command_is_string: bool,
}

/// What the scan has of one `toolCall` object.
pub(super) struct ToolCallScan {
    count_name: usize,
    count_args: usize,
    name: NameValue,
    args_is_object: bool,
    args: ArgsScan,
    command_text: String,
}

pub(super) enum ToolCallValue {
    Missing,
    NotObject,
    Object(ToolCallScan),
}

/// What the scan has of the whole payload: how many of the top
/// object's keys are `toolCall` in any letter case, and the scan of
/// the last exact `toolCall` value.
pub(super) struct PayloadScan {
    count_tool_call: usize,
    tool_call: ToolCallValue,
}

/// Scan one object key and report how it stands against `names`.
pub(super) fn scan_watch_key<R: Read>(scan: &mut Scanner<R>, names: &[&'static str]) -> Option<WatchKey> {
    let mut watch = WatchKey::new(names);
    scan.string(&mut watch)?;
    Some(watch)
}

/// Scan the value of the exact `name` key of a `toolCall` object,
/// at value depth 3.
pub(super) fn scan_name_value<R: Read>(scan: &mut Scanner<R>) -> Option<NameValue> {
    scan.ws();
    if scan.peek() == Some(b'"') {
        let mut sink = MatchSink::new("run_command");
        scan.string(&mut sink)?;
        Some(if sink.matched() { NameValue::RunCommand } else { NameValue::OtherString })
    } else {
        scan.value(3)?;
        Some(NameValue::NotString)
    }
}

/// Scan the value of the exact `args` key of a `toolCall` object,
/// at value depth 3: Some scan when it is an object, None when it
/// is any other value. The last exact `CommandLine` string of the
/// object is collected into `command_text`.
pub(super) fn scan_args_value<R: Read>(scan: &mut Scanner<R>, command_text: &mut String) -> Option<Option<ArgsScan>> {
    command_text.clear();
    scan.ws();
    if scan.peek() != Some(b'{') {
        scan.value(3)?;
        return Some(None);
    }
    scan.bump();
    let mut args = ArgsScan::default();
    if scan.eat(b'}').is_some() {
        return Some(Some(args));
    }
    loop {
        scan.ws();
        let key = scan_watch_key(scan, &["CommandLine"])?;
        scan.eat(b':')?;
        if key.case_insensitive(0) {
            args.count_command_line += 1;
        }
        if key.exact(0) {
            args.has_command = true;
            scan.ws();
            if scan.peek() == Some(b'"') {
                command_text.clear();
                let mut capture = Capture { text: &mut *command_text };
                scan.string(&mut capture)?;
                args.command_is_string = true;
            } else {
                scan.value(4)?;
                args.command_is_string = false;
                command_text.clear();
            }
        } else {
            scan.value(4)?;
        }
        if scan.eat(b',').is_some() {
            continue;
        }
        scan.eat(b'}')?;
        return Some(Some(args));
    }
}

/// Scan the value of the exact `toolCall` key, at value depth 2.
pub(super) fn scan_tool_call_value<R: Read>(scan: &mut Scanner<R>) -> Option<ToolCallValue> {
    scan.ws();
    if scan.peek() != Some(b'{') {
        scan.value(2)?;
        return Some(ToolCallValue::NotObject);
    }
    scan.bump();
    let mut tool = ToolCallScan {
        count_name: 0,
        count_args: 0,
        name: NameValue::Missing,
        args_is_object: false,
        args: ArgsScan::default(),
        command_text: String::new(),
    };
    if scan.eat(b'}').is_some() {
        return Some(ToolCallValue::Object(tool));
    }
    loop {
        scan.ws();
        let key = scan_watch_key(scan, &["name", "args"])?;
        scan.eat(b':')?;
        if key.case_insensitive(0) {
            tool.count_name += 1;
        }
        if key.case_insensitive(1) {
            tool.count_args += 1;
        }
        if key.exact(0) {
            tool.name = scan_name_value(scan)?;
        } else if key.exact(1) {
            match scan_args_value(scan, &mut tool.command_text)? {
                Some(args) => {
                    tool.args = args;
                    tool.args_is_object = true;
                }
                None => {
                    tool.args = ArgsScan::default();
                    tool.args_is_object = false;
                }
            }
        } else {
            scan.value(3)?;
        }
        if scan.eat(b',').is_some() {
            continue;
        }
        scan.eat(b'}')?;
        return Some(ToolCallValue::Object(tool));
    }
}

/// Scan an Antigravity payload from `source`: the top object's
/// `toolCall` keys are counted as they stream past and the last
/// exact `toolCall` value is scanned for the fields the decision
/// reads; every other value is skipped by shape. When `budget` is
/// given it replaces the scanner's payload cap — a caller that
/// already holds the whole payload as text passes its length, so
/// the scan covers exactly the text it was given. None when the
/// payload is not exactly one JSON value or a read failed: the
/// cases the whole-string parse rejects.
pub(super) fn antigravity_scan<R: Read>(source: R, budget: Option<usize>) -> Option<PayloadScan> {
    let mut scan = Scanner::new(source);
    if let Some(budget) = budget {
        scan.budget = budget;
    }
    let walked = (|| -> Option<PayloadScan> {
        let mut payload = PayloadScan { count_tool_call: 0, tool_call: ToolCallValue::Missing };
        scan.ws();
        if scan.peek() == Some(b'{') {
            scan.bump();
            if scan.eat(b'}').is_none() {
                loop {
                    scan.ws();
                    let key = scan_watch_key(&mut scan, &["toolCall"])?;
                    scan.eat(b':')?;
                    if key.case_insensitive(0) {
                        payload.count_tool_call += 1;
                    }
                    if key.exact(0) {
                        payload.tool_call = scan_tool_call_value(&mut scan)?;
                    } else {
                        scan.value(2)?;
                    }
                    if scan.eat(b',').is_some() {
                        continue;
                    }
                    scan.eat(b'}')?;
                    break;
                }
            }
        } else {
            scan.value(1)?;
        }
        scan.ws();
        if scan.peek().is_some() {
            return None;
        }
        Some(payload)
    })();
    if walked.is_none() || scan.failed {
        return None;
    }
    walked
}

/// The one-line JSON decision for a Google Antigravity PreToolUse
/// payload, from its streaming scan (see `antigravity_scan`): the
/// payload itself is never materialised, and the only string held
/// is the one `CommandLine` the rewrite needs. `set` holds the path
/// sets of this executable, computed once by the caller;
/// `wrapper_exists` says whether the automatic wrapper of its own
/// name is present beside it.
pub(super) fn antigravity_decision(set: &PathSet<'_>, scanned: Option<PayloadScan>, wrapper_exists: bool, command_prechecked: bool) -> std::borrow::Cow<'static, str> {
    // The borrowed form covers the constant answers (the allow of
    // another tool and of an already-guarded command): building
    // those into a fresh string allocated for a text that stands
    // in the binary, on the path of every such payload. Built
    // answers (denials and rewrites) are owned, as before, and
    // their bytes are the same either way.
    let payload = match scanned {
        Some(p) => p,
        None => return antigravity_deny("the hook payload is missing or is not valid JSON.").into(),
    };
    // The hook reads exact keys. A host that matches keys without regard to case, or lets
    // another spelling or a repeated key win, could act on a different tool or command than
    // the one checked, so any second candidate for a key the hook reads is denied.
    if payload.count_tool_call > 1 {
        return antigravity_deny("the hook payload holds ambiguous toolCall keys (more than one key that is toolCall in any letter case).").into();
    }
    let tool = match &payload.tool_call {
        ToolCallValue::Object(tool) => {
            if let Some(key) = ["name", "args"].into_iter().find(|k| match *k {
                "name" => tool.count_name > 1,
                _ => tool.count_args > 1,
            }) {
                return antigravity_deny_with(|out| {
                    out.push_str("the toolCall object holds ambiguous ");
                    out.push_str(key);
                    out.push_str(" keys (more than one key that is ");
                    out.push_str(key);
                    out.push_str(" in any letter case).");
                }).into();
            }
            tool
        }
        _ => return antigravity_deny("the hook payload has no toolCall object with a string name, so it is not a recognised Antigravity PreToolUse payload.").into(),
    };
    // A payload without a toolCall object that names its tool is not the documented
    // shape (the schema changed or the hook is miswired); it is denied, not waved through.
    match tool.name {
        NameValue::RunCommand => {}
        NameValue::OtherString => return "{\"decision\":\"allow\"}".into(),
        _ => return antigravity_deny("the hook payload has no toolCall object with a string name, so it is not a recognised Antigravity PreToolUse payload.").into(),
    }
    // The hook rewrites only the exact key `CommandLine`, under the same rule.
    if tool.args_is_object && tool.args.count_command_line > 1 {
        return antigravity_deny("run_command arguments hold ambiguous CommandLine keys (more than one key that is CommandLine in any letter case).").into();
    }
    // `command_prechecked` is the entry's word that it has already
    // made exactly this command test — the nonempty-trim and the
    // NUL scan below — when it settled whether the path set was
    // needed at all: a caller that prechecked pays neither scan a
    // second time for the same text (the command can run to the
    // payload cap, so the NUL scan is a full pass over it). A
    // caller that did not precheck takes the test here, unchanged.
    let command = if tool.args_is_object && tool.args.has_command && tool.args.command_is_string
        && (command_prechecked || crate::base64::nonempty_without_nul(&tool.command_text))
    {
        tool.command_text.as_str()
    } else {
        return antigravity_deny("run_command has no CommandLine, or it is empty or contains NUL.").into();
    };
    let wrapper = match set.first_wrapper.as_deref() {
        Some(path) => path,
        None => return antigravity_deny("the location of the process guard is unknown.").into(),
    };
    if !wrapper_exists {
        return antigravity_deny_with(|out| {
            out.push_str("the automatic wrapper ");
            write_escaped(wrapper, out);
            out.push_str(" is missing beside the process guard.");
        }).into();
    }
    if !plain_path(wrapper) {
        return antigravity_deny("the process guard must be installed in a path with only the characters A-Z, a-z, 0-9, colon, slash, period, underscore and hyphen, so the rewritten command parses the same in every shell.").into();
    }
    if antigravity_guarded(command, set) {
        return "{\"decision\":\"allow\"}".into();
    }
    // The wrapped line's length is known before it is built: the
    // wrapper, the fixed ` --command-b64 ` span, and four Base64
    // characters per three command bytes (the encoding of a nonempty
    // command — this point is past the empty-command denial above).
    // A command past the limit is denied from that count alone:
    // building the wrapped line first encoded the whole command into
    // a string (and built the empty wrap a second time for its fixed
    // part) only for the denial to read the string's length and drop
    // it.
    let wrapped_len = wrapper.len() + " --command-b64 ".len() + command.len().div_ceil(3) * 4;
    if wrapped_len > MAX_REWRITTEN_LINE {
        let fixed = wrapper.len() + " --command-b64 ".len();
        return antigravity_deny_with(|out| {
            use std::fmt::Write as _;
            out.push_str("the command is too long to run under the guard: wrapping it makes a line of ");
            let _ = write!(out, "{}", wrapped_len);
            out.push_str(" characters and the limit is ");
            let _ = write!(out, "{}", MAX_REWRITTEN_LINE);
            out.push_str(", which allows a command of about ");
            let _ = write!(out, "{}", MAX_REWRITTEN_LINE.saturating_sub(fixed) / 4 * 3);
            out.push_str(" bytes here. Write the long content to a file and run the file.");
        }).into();
    }
    let rewritten = antigravity_wrapped(wrapper, command);
    // Both quoted fields are escaped straight into the answer buffer,
    // as in `antigravity_deny`: quoting each into its own string first
    // built two strings only for the format to copy them back out.
    let mut out = String::with_capacity(rewritten.len() + 160);
    out.push_str("{\"decision\":\"allow\",\"reason\":");
    write_quote("Command runs under the process guard (automatic deadline and process tree cleanup).", &mut out);
    out.push_str(",\"overwrite\":{\"CommandLine\":");
    write_quote(&rewritten, &mut out);
    out.push_str("}}");
    out.into()
}

/// `hook antigravity`: rewrite a Google Antigravity `run_command` call so the
/// automatic guard runs it. Every outcome is a JSON decision and the exit code is 0.
/// The executable is resolved only when the payload needs it. The path set — the executable query, the
/// envelope and wrapper path sets built from it, and the probe of the
/// wrapper beside it — is read by exactly one branch of the
/// decision: a payload whose tool is `run_command`. Every other
/// payload the host sends through this hook (every other tool, and
/// every malformed one) is decided from its scan alone, so it no
/// longer pays for path work whose result it never reads.
pub fn antigravity_entry(role: Role) -> i32 {
    let scanned = antigravity_scan(crate::cli_process::standard_input(), None);
    // The decision reads the path set at one point only: after the
    // ambiguity counts, the tool name, and the command itself have
    // all passed. This condition is that point, mirrored — and it
    // is the one performance of the command test (the nonempty
    // trim and the NUL scan): the decision is told this route
    // prechecked it and does not perform either scan again.
    let needs_paths = matches!(&scanned, Some(scan) if scan.count_tool_call <= 1
        && matches!(&scan.tool_call, ToolCallValue::Object(tool) if tool.count_name <= 1 && tool.count_args <= 1
            && tool.name == NameValue::RunCommand
            && tool.args_is_object && tool.args.count_command_line <= 1
            && tool.args.has_command && tool.args.command_is_string
            && crate::base64::nonempty_without_nul(&tool.command_text)));
    let line = if needs_paths {
        // The stack spelling first, as in `main`: the standard call
        // is the fallback for a name that does not fit it.
        let stack = stack_self_path();
        let mut owned = None;
        let exe = exe_spelling(&stack, &mut owned);
        let set = path_set(exe, role);
        let wrapper_exists = set.first_wrapper.as_deref().is_some_and(|path| std::path::Path::new(path).is_file());
        antigravity_decision(&set, scanned, wrapper_exists, true)
    } else {
        antigravity_decision(&PathSet { envelope: EnvelopePaths::empty(), wrappers: WrapperSet::empty(), first_wrapper: None }, scanned, false, false)
    };
    say(&line);
    0
}

/// `hook antigravity-context`: the policy as a SessionStart system message.
pub fn antigravity_context() -> i32 {
    let mut out = String::with_capacity(128);
    out.push_str("{\"injectSteps\":[{\"systemMessage\":{\"systemMessage\":");
    write_policy(&mut out);
    out.push_str("}}]}");
    say(&out);
    0
}
