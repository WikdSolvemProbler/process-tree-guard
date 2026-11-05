// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The powershell-auto vendor: rewriting a PowerShell tool call into the automatic wrapper, or denying it.

use super::*;

/// The PowerShell command line that runs `command` under the automatic guard.
pub fn powershell_wrapped(auto_exe: &str, command: &str) -> String {
    let utf8 = crate::base64::base64_encode(command.as_bytes());
    // The escaped form is built only when the path holds a quote to
    // escape: `replace` allocates a full copy of the path even when
    // there is nothing in it to replace, on every wrapped command.
    let shown: std::borrow::Cow<'_, str> = if auto_exe.contains('\'') {
        std::borrow::Cow::Owned(auto_exe.replace('\'', "''"))
    } else {
        std::borrow::Cow::Borrowed(auto_exe)
    };
    format!("& '{}' --shell powershell --command-b64 {}; exit $LASTEXITCODE", shown, utf8)
}

/// A reader between the payload source and the scanner that holds
/// only the bytes of one armed span. The facts walk arms it at a
/// value's start offset through the scanner (see
/// `Scanner::arm_capture`) and disarms it at its end. The start
/// offset is a scanner offset (`Scanner::byte_offset`), so bytes
/// of the span the scanner has already fetched into its window
/// seed the capture when it arms, the serves that follow are
/// recorded while it stays armed, and the serve past the span's
/// end is cut back by the disarm. Bytes served while disarmed are
/// not held, and no byte is copied into a side buffer before a
/// span is armed.
pub(super) struct CapturingReader<R: Read> {
    inner: R,
    armed: bool,
    start: usize,
    captured: Vec<u8>,
}

impl<R: Read> CapturingReader<R> {
    fn new(inner: R) -> Self {
        CapturingReader { inner, armed: false, start: 0, captured: Vec::new() }
    }
    /// Begin recording the span that starts at source offset
    /// `start`, seeded with `seed`: the span's already-fetched
    /// prefix, the unread bytes of the scanner's window at the
    /// moment of arming. A later span replaces the earlier one,
    /// from the new seed.
    fn arm_seeded(&mut self, start: usize, seed: &[u8]) {
        self.captured.clear();
        self.captured.extend_from_slice(seed);
        self.start = start;
        self.armed = true;
    }
    /// Stop recording at source offset `end` and hand over the
    /// span's bytes: the scanner's window may hold bytes past the
    /// span's end, served while armed, so the buffer is first cut
    /// back to the span's own length.
    fn disarm(&mut self, end: usize) -> Vec<u8> {
        self.armed = false;
        self.captured.truncate(end.saturating_sub(self.start));
        std::mem::take(&mut self.captured)
    }
}

impl<R: Read> Read for CapturingReader<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(out)?;
        if self.armed {
            self.captured.extend_from_slice(&out[..n]);
        }
        Ok(n)
    }
}

impl<R: Read> Scanner<CapturingReader<R>> {
    /// Arm the source's span capture at source offset `start`, the
    /// scanner's current offset: the span's already-fetched prefix
    /// is exactly the unread part of the current window, the source
    /// offsets `window[pos..end]` (the window holds the source
    /// offsets `[fetched - end, fetched)` and `start` is
    /// `fetched - (end - pos)`), so that prefix seeds the capture
    /// and only bytes served after this call are appended by the
    /// source itself. The seed slice is computed from the raw
    /// window storage exactly as `available` computes its slice,
    /// confined to the prefix the last fill reported, without
    /// refilling: when the window is spent (`pos == end`) the seed
    /// is empty and the span's first bytes are the next serve. A
    /// span start is the scanner's current offset by construction,
    /// so it is always inside the window or at its spent end, and
    /// the seed is always the span's true prefix regardless of how
    /// short the underlying serves were. Doing this as a `Scanner`
    /// method keeps the borrow of the window and the mutation of
    /// the source inside one function.
    fn arm_capture(&mut self, start: usize) {
        let seed: &[u8] = if self.pos < self.end {
            unsafe { std::slice::from_raw_parts(self.window.as_ptr().add(self.pos) as *const u8, self.end - self.pos) }
        } else {
            &[]
        };
        self.source.arm_seeded(start, seed);
    }
}

/// The two facts `powershell_auto` reads outside the last
/// `tool_input` value, found by walking the payload's top level as
/// it streams from `source` through the scanner, as
/// `scan_top_object` walks it for the verdict vendors: the captured
/// bytes of the last `tool_input` value, and whether the last
/// `permission_mode` value is the string `bypassPermissions`,
/// watched as it decodes with a key probe against that one name.
/// Every other value — and every earlier value of either watched
/// key — is skipped by shape (`Scanner::value`), so no value tree
/// is built for it and its bytes are never held, and a repeated
/// watched key re-arms the capture from empty in place of the
/// earlier one, as the last value won in the whole-payload tree.
/// The scanner's payload cap is the limit the whole-string read
/// applied: bytes past it are never served, so a payload over the
/// cap settles exactly as that capped read settled it. None when
/// the payload is not exactly one JSON value, nothing after it but
/// whitespace, or a read of it failed: the rejections the
/// whole-payload parse gave.
pub(super) fn powershell_payload_facts<R: Read>(source: R) -> Option<(Option<Vec<u8>>, bool)> {
    let mut scan = Scanner::new(CapturingReader::new(source));
    let mut span: Option<Vec<u8>> = None;
    let mut bypass = false;
    let walked = (|| -> Option<()> {
        scan.ws();
        if scan.peek() == Some(b'{') {
            scan.bump();
            if scan.eat(b'}').is_none() {
                loop {
                    scan.ws();
                    let matched = scan.key(&["tool_input", "permission_mode"])?;
                    scan.eat(b':')?;
                    match matched {
                        Some(0) => {
                            scan.ws();
                            let start = scan.byte_offset();
                            scan.arm_capture(start);
                            scan.value(2)?;
                            let end = scan.byte_offset();
                            span = Some(scan.source.disarm(end));
                        }
                        Some(1) => {
                            scan.ws();
                            if scan.peek() == Some(b'"') {
                                let mut probe = KeyProbe::new(&["bypassPermissions"]);
                                scan.string(&mut probe)?;
                                bypass = probe.matched() == Some(0);
                            } else {
                                scan.value(2)?;
                                bypass = false;
                            }
                        }
                        _ => scan.value(2)?,
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
        Some(())
    })();
    if walked.is_none() || scan.failed {
        return None;
    }
    Some((span, bypass))
}

/// The denial `powershell_auto` answers a payload it cannot
/// classify with: the PreToolUse decision form this hook builds
/// for a denial (see `main`) and this vendor's own `ask` / `allow`
/// answers use, with `permissionDecision` of deny and no
/// `updatedInput`, written as one line through `say`, at exit
/// code 0 — the exit code this vendor always uses (see the
/// module doc). A payload that cannot be classified is denied,
/// never passed through in silence: silence is no decision, and
/// no decision is how this format lets the command run as
/// written, without the guard.
pub(super) fn powershell_deny(why: &str) -> i32 {
    let mut out = String::with_capacity(why.len() + 112);
    out.push_str("{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":");
    write_quote(why, &mut out);
    out.push_str("}}");
    say(&out);
    0
}

/// `hook powershell-auto`: rewrite a PowerShell tool call so the automatic
/// guard runs it. A command that already starts with a guard path is left as is.
pub fn powershell_auto(role: Role) -> i32 {
    // The payload is walked as it streams, and its command taken,
    // before any path is built: a payload that is not a tool call
    // with a command string (unreadable, not JSON, no `tool_input`,
    // no string `command`) is denied at 0 without the wrapper and
    // envelope strings — and the one path set the checks below share
    // is computed once, where the wrapper list and the envelope list
    // were each built separately before. The walk builds no tree and
    // holds no whole-payload string: only the last `tool_input`
    // value's captured bytes are parsed into one, because the
    // rewrite below refills its command slot and re-serialises that
    // subtree.
    let (captured, bypass) = match powershell_payload_facts(crate::cli_process::standard_input()) {
        Some(facts) => facts,
        None => return powershell_deny("the hook payload is missing or is not valid JSON."),
    };
    let captured = match captured { Some(captured) => captured, None => return powershell_deny("the hook payload has no tool_input, so it is not a recognised PowerShell tool payload.") };
    let mut tool_input = match std::str::from_utf8(&captured).ok().and_then(parse_json) { Some(value) => value, None => return powershell_deny("the hook payload's tool_input is not valid JSON.") };
    // The command string is taken out of the `tool_input` value, not
    // cloned: the value is owned, and the slot it leaves is exactly
    // the one the rewrite below fills (its last `command` pair, as
    // the read here selected: the last pair wins, as in `Json::get`).
    // An already-guarded command returns before the slot is read again.
    let command = match &mut tool_input {
        Json::Object(pairs) => match pairs.iter_mut().rev().find(|(k, _)| k == "command") {
            Some((_, Json::Str(text))) => std::mem::take(text),
            _ => return powershell_deny("the hook payload's tool_input has no string command."),
        },
        _ => return powershell_deny("the hook payload's tool_input is not an object."),
    };
    // The spelling is resolved here — after the captured command
    // has been extracted and before the path set is built from it,
    // mirroring `antigravity_entry`'s placement: every payload that
    // is not a tool call with a string command (every return above)
    // pays no module-name query and no decode walk for a path set
    // it never builds. The stack form first, the standard call
    // taken directly as its fallback, as in the other vendors.
    let stack = stack_self_path();
    let mut owned = None;
    let exe = exe_spelling(&stack, &mut owned);
    let set = path_set(exe, role);
    let auto_exe = match set.first_wrapper.as_deref() {
        Some(path) => path,
        None => return powershell_deny("the location of the process guard is unknown."),
    };
    // Pass through unchanged only what is already exactly guarded: this hook's
    // own wrapping (with any name's wrapper), or a valid envelope. Anything
    // else, including a command that merely starts with a guard path, is wrapped
    // again (a double guard is harmless).
    // The prefix is matched in segments, in place: building it as a
    // formatted string per wrapper — with a second, escaping copy of
    // the wrapper inside it — allocated twice for every wrapper the
    // check tried (three on the gate role), for a comparison that
    // reads the command and the wrapper where they already stand.
    // A wrapper whose folder holds a quote takes the formatted
    // form for that wrapper, whose escaping the segment form would
    // have to reproduce; the quote can only sit in the folder, as
    // the program names and `-auto.exe` hold none.
    // The command is trimmed once, before the per-spec check: the
    // closure trimmed it again for every wrapper it tried (three
    // on the gate role), re-scanning the same leading and trailing
    // whitespace per spec for the one trimmed text they all read.
    let trimmed = command.trim();
    let wrapped_already = |spec: &WrapperSpec<'_>| {
        let rest = if spec.dir.contains('\'') {
            let wrapper = format!("{}/{}-auto.exe", spec.dir, spec.name);
            trimmed.strip_prefix(&format!("& '{}' --shell powershell --command-b64 ", wrapper.replace('\'', "''")))
        } else {
            trimmed.strip_prefix("& '")
                .and_then(|r| strip_wrapper_prefix(r, *spec))
                .and_then(|r| r.strip_prefix("' --shell powershell --command-b64 "))
        };
        rest.and_then(|r| r.strip_suffix("; exit $LASTEXITCODE"))
            .is_some_and(crate::base64::command_text_is_valid)
    };
    // The envelope walk takes the text trimmed above: routing the
    // untrimmed command through the trimming entry re-scanned its
    // edges for a result `trimmed` already holds (trimming is
    // idempotent, so the walk reads byte-for-byte the same text).
    let already = valid_envelope_paths_trimmed(trimmed, &set.envelope) || set.wrappers.iter().any(wrapped_already);
    if already {
        say("{}");
        return 0;
    }
    // The wrapped line is rendered once, from the extracted (last)
    // command, and moved into the first
    // `command` slot (an object with the key repeated gets a copy in
    // each later slot, as the per-slot rendering produced before).
    let wrapped = powershell_wrapped(auto_exe, &command);
    if let Json::Object(pairs) = &mut tool_input {
        let mut wrapped = Some(wrapped);
        for (k, v) in pairs.iter_mut() {
            if k == "command" {
                *v = Json::Str(match wrapped.take() {
                    Some(text) => text,
                    None => powershell_wrapped(auto_exe, &command),
                });
            }
        }
    }
    let updated = &tool_input;
    // The answer is serialized in one pass into a buffer reserved for
    // it: the wrapped command grows the captured subtree by about a third.
    let mut out = String::with_capacity(captured.len() + captured.len() / 3 + 512);
    out.push_str("{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":");
    write_quote(if bypass { "allow" } else { "ask" }, &mut out);
    out.push_str(",\"permissionDecisionReason\":");
    write_quote("Command runs under the process guard (automatic deadline and process tree cleanup).", &mut out);
    out.push_str(",\"updatedInput\":");
    write_json(updated, &mut out);
    out.push_str("}}");
    say(&out);
    0
}
