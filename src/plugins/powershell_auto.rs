// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The powershell-auto vendor: rewriting a PowerShell tool call into the automatic wrapper, or denying it.

use super::*;

/// PowerShell treats smart apostrophes as quote delimiters too. This
/// wrapper supports ASCII apostrophe escaping, so reject other delimiters.
fn wrapper_path_supported(path: &str) -> bool {
    !path.contains(['\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}'])
}

/// The PowerShell command line for a supported automatic wrapper path.
fn powershell_wrapped(auto_exe: &str, command: &str) -> String {
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
            &self.window[self.pos..self.end]
        } else {
            &[]
        };
        self.source.arm_seeded(start, seed);
    }
}

/// Capture the single `tool_input` value and check for
/// `permission_mode: "bypassPermissions"`. Reject ambiguous spellings
/// of either key so the host and the hook cannot select different values.
/// Unwatched values are skipped without retaining their bytes.
pub(super) fn powershell_payload_facts<R: Read>(source: R) -> Option<(Option<Vec<u8>>, bool)> {
    let mut scan = Scanner::new(CapturingReader::new(source));
    let mut span: Option<Vec<u8>> = None;
    let mut seen_permission_mode = false;
    let mut bypass = false;
    let walked = (|| -> Option<()> {
        scan.ws();
        if scan.peek() == Some(b'{') {
            scan.bump();
            if scan.eat(b'}').is_none() {
                loop {
                    scan.ws();
                    let key = scan_watch_key(&mut scan, &["tool_input", "permission_mode"])?;
                    scan.eat(b':')?;
                    if key.case_insensitive(0) {
                        if !key.exact(0) || span.is_some() {
                            return None;
                        }
                        scan.ws();
                        let start = scan.byte_offset();
                        scan.arm_capture(start);
                        scan.value(2)?;
                        let end = scan.byte_offset();
                        span = Some(scan.source.disarm(end));
                    } else if key.case_insensitive(1) {
                        if !key.exact(1) || seen_permission_mode {
                            return None;
                        }
                        seen_permission_mode = true;
                        scan.ws();
                        if scan.peek() == Some(b'"') {
                            let mut probe = KeyProbe::new(&["bypassPermissions"]);
                            scan.string(&mut probe)?;
                            bypass = probe.matched() == Some(0);
                        } else {
                            scan.value(2)?;
                        }
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
        Some(())
    })();
    if walked.is_none() || scan.failed {
        return None;
    }
    Some((span, bypass))
}

/// The host blocks on exit 2. If the decision cannot be delivered,
/// returning success could let the host run the original tool call.
fn reply(text: &str) -> i32 {
    if crate::cli_process::try_write_stdout_line(text) {
        0
    } else {
        crate::cli_process::write_stderr_line("The process guard could not write the PowerShell hook decision.");
        crate::EXIT_HOOK_DENY
    }
}

/// Deny a payload that cannot be classified or safely rewritten.
pub(super) fn powershell_deny(why: &str) -> i32 {
    let mut out = String::with_capacity(why.len() + 112);
    out.push_str("{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":");
    write_quote(why, &mut out);
    out.push_str("}}");
    reply(&out)
}

fn command_slot(tool_input: &mut Json) -> Result<&mut String, &'static str> {
    let Json::Object(pairs) = tool_input else {
        return Err("the hook payload's tool_input is not an object.");
    };
    let mut commands = pairs.iter_mut().filter(|(key, _)| key.eq_ignore_ascii_case("command"));
    let first = commands.next();
    if commands.next().is_some() {
        return Err("the hook payload's tool_input has ambiguous command keys.");
    }
    match first {
        Some((key, Json::Str(command))) if key == "command" => Ok(command),
        _ => Err("the hook payload's tool_input has no string command."),
    }
}

/// `hook powershell-auto`: rewrite a PowerShell tool call so the automatic
/// guard runs it. An exactly guarded command is left as is.
pub fn powershell_auto(role: Role) -> i32 {
    // Validate the tool call before resolving paths. Only the captured
    // `tool_input` subtree is parsed into a value tree for rewriting.
    let (captured, bypass) = match powershell_payload_facts(crate::cli_process::standard_input()) {
        Some(facts) => facts,
        None => return powershell_deny("the hook payload is missing, malformed, or ambiguous."),
    };
    let captured = match captured { Some(captured) => captured, None => return powershell_deny("the hook payload has no tool_input, so it is not a recognised PowerShell tool payload.") };
    let mut tool_input = match std::str::from_utf8(&captured).ok().and_then(parse_json) { Some(value) => value, None => return powershell_deny("the hook payload's tool_input is not valid JSON.") };
    let slot = match command_slot(&mut tool_input) {
        Ok(slot) => slot,
        Err(why) => return powershell_deny(why),
    };
    let command = std::mem::take(slot);
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
    if !wrapper_path_supported(auto_exe) {
        return powershell_deny("the process guard's wrapper path contains a smart apostrophe.");
    }
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
        return reply("{}");
    }
    *slot = powershell_wrapped(auto_exe, &command);
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
    reply(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ambiguous_payload_keys() {
        for payload in [
            r#"{"tool_input":{},"tool_input":{}}"#,
            r#"{"Tool_Input":{},"tool_input":{}}"#,
            r#"{"tool_input":{},"permission_mode":"default","permission_mode":"bypassPermissions"}"#,
            r#"{"tool_input":{},"PERMISSION_MODE":"default","permission_mode":"bypassPermissions"}"#,
        ] {
            assert!(powershell_payload_facts(payload.as_bytes()).is_none(), "{payload}");
        }
        let payload = br#"{"tool_input":{"command":"echo hi"},"permission_mode":"bypassPermissions"}"#;
        assert!(powershell_payload_facts(&payload[..]).unwrap().1);
    }

    #[test]
    fn requires_one_exact_command_key() {
        for payload in [
            r#"{"command":"one","command":"two"}"#,
            r#"{"Command":"one","command":"two"}"#,
            r#"{"command":"one","comm\u0061nd":"two"}"#,
        ] {
            assert!(command_slot(&mut parse_json(payload).unwrap()).is_err(), "{payload}");
        }
        let mut input = parse_json(r#"{"command":"echo hi","other":true}"#).unwrap();
        assert_eq!(command_slot(&mut input).unwrap().as_str(), "echo hi");
    }

    #[test]
    fn rejects_smart_apostrophes_in_wrapper_paths() {
        for quote in ['\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}'] {
            assert!(!wrapper_path_supported(&format!("C:/a{quote}; whoami; #/ptg-auto.exe")));
        }
        assert!(wrapper_path_supported("C:/a'b/ptg-auto.exe"));
        assert!(powershell_wrapped("C:/a'b/ptg-auto.exe", "echo hi").starts_with("& 'C:/a''b/ptg-auto.exe' "));
    }
}
