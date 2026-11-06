// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The streaming scanner: a payload walked through one window, its watched strings decoded straight into their checkers.

use super::*;

// ------------------------------------------------------------ streaming scan
//
// The verdict vendors (pretooluse and cursor) decide from one watched
// string inside the payload. They read the payload through a 16 KiB
// window and walk its structure without building a value tree: unwatched
// values are skipped by shape, keys are matched as they decode, and the
// watched string is decoded straight into the envelope checker below,
// which keeps only the verdict. The walk accepts exactly what the
// parser above accepts: the same depth limit, the same string escapes
// and surrogate rules, the same number shapes, nothing after the top
// value but whitespace, and the last value of a repeated key winning,
// as `Json::get` finds it.

/// One decoded character of a scanned string, delivered in order.
pub(crate) trait CharSink {
    fn push_char(&mut self, c: char);
    /// Whether the sink discards every character it is given, so a
    /// plain run of string bytes may be validated whole instead of
    /// delivered character by character. A sink that watches its
    /// characters keeps per-character delivery.
    fn takes_bulk(&self) -> bool {
        false
    }
    /// Whether the sink collects the string's text whole, so a plain
    /// run of string bytes that has been validated whole may be
    /// handed to it as one slice through `push_run` instead of being
    /// decoded character by character and pushed one at a time. The
    /// bytes of a validated run are exactly the characters the
    /// per-character delivery would have produced, encoded: a valid
    /// run starts and ends at a scalar boundary, so collecting it
    /// whole collects the same text.
    fn captures_run(&self) -> bool {
        false
    }
    /// Take one validated plain run of a string, as text. Only
    /// called on a sink whose `captures_run` holds.
    fn push_run(&mut self, _text: &str) {}
    /// Whether the sink is at the base64 payload stage of a live
    /// envelope check, so a validated plain run of string bytes may
    /// be handed to it for run processing instead of being decoded
    /// character by character and pushed one at a time. Only the
    /// envelope check at that one stage reports true; every other
    /// sink keeps per-character delivery.
    fn takes_payload_run(&self) -> bool {
        false
    }
    /// Process one validated plain run of a string at the payload
    /// stage and report how many of its leading bytes were
    /// consumed. Only the whitespace-free prefix of the run is
    /// consumed: the first decoded whitespace character and every
    /// byte after it are left unconsumed, so the caller delivers
    /// them through the per-character path exactly as before.
    /// Only called on a sink whose `takes_payload_run` holds.
    /// The run arrives as text: the caller validated it whole, as
    /// it does for the collecting form's `push_run`, so the
    /// validation stands in the caller alone and no second
    /// validation of the same run stands in the sink.
    fn push_payload_run(&mut self, _run: &str) -> usize {
        0
    }
    /// Whether the sink is at the timeout stage of a live
    /// envelope check, so a validated plain run of string bytes
    /// may be handed to it for run processing instead of being
    /// decoded character by character and pushed one at a time.
    /// Only the envelope check at that one stage reports true;
    /// every other sink keeps per-character delivery.
    fn takes_timeout_run(&self) -> bool {
        false
    }
    /// Process one validated plain run of a string at the
    /// timeout stage and report how many of its leading bytes
    /// were consumed. Only the prefix of ASCII digits and `.`
    /// is consumed: the first byte that is neither — a space, a
    /// letter, or any byte at or past 0x80 (digits and the point
    /// are ASCII, so a multi-byte character's lead byte ends the
    /// prefix on its own boundary) — and every byte after it are
    /// left unconsumed, as is everything after the feed on which
    /// the check settles, so the caller delivers the rest
    /// through the per-character and bulk paths exactly as
    /// before. Only called on a sink whose `takes_timeout_run`
    /// holds. The run arrives as text: the caller validated it
    /// whole, as it does for the payload run form, so the
    /// validation stands in the caller alone and no second
    /// validation of the same run stands in the sink.
    fn push_timeout_run(&mut self, _run: &str) -> usize {
        0
    }
    /// Whether the sink is at a stage of a live envelope check
    /// whose only act on a whitespace character is to stay in
    /// place — the leading-whitespace stage, or the payload stage
    /// once its trailing whitespace has begun — so a validated
    /// plain run of string bytes may be handed to it for run
    /// processing instead of being decoded character by character
    /// and pushed one at a time. Only the envelope check at those
    /// two stages reports true; every other sink keeps
    /// per-character delivery.
    fn takes_whitespace_run(&self) -> bool {
        false
    }
    /// Process one validated plain run of a string at a whitespace
    /// stage and report how many of its leading bytes were
    /// consumed. Only the maximal decoded-whitespace prefix is
    /// consumed: each character of the prefix is tested with
    /// `char::is_whitespace` at its own position (the full Unicode
    /// set, never a space-byte test), and the first
    /// non-whitespace character and every byte after it are left
    /// unconsumed, so the caller delivers them through the
    /// per-character path exactly as before. No consumed
    /// character changes the sink's state at either stage, so the
    /// run form records nothing per character. Only called on a
    /// sink whose `takes_whitespace_run` holds. The run arrives
    /// as text: the caller validated it whole, as it does for the
    /// payload run form, so the validation stands in the caller
    /// alone and no second validation of the same run stands in
    /// the sink.
    fn push_whitespace_run(&mut self, _run: &str) -> usize {
        0
    }
}

/// A sink that discards the characters: the string is still validated.
pub(crate) struct Skip;

impl CharSink for Skip {
    fn push_char(&mut self, _: char) {}
    fn takes_bulk(&self) -> bool {
        true
    }
}

/// Watches the decoded characters of one object key against up to two
/// names, without holding the key: a name matches when every character
/// matched and the key ends there. Escaped spellings decode first, so a
/// key written with escapes matches the name it decodes to.
pub(crate) struct KeyProbe {
    names: [Option<&'static str>; 2],
    seen: [usize; 2],
    alive: [bool; 2],
}

impl KeyProbe {
    pub(crate) fn new(names: &[&'static str]) -> Self {
        let mut probe = KeyProbe { names: [None, None], seen: [0, 0], alive: [false, false] };
        for (slot, name) in probe.names.iter_mut().zip(names.iter()) {
            *slot = Some(name);
        }
        probe.alive = [probe.names[0].is_some(), probe.names[1].is_some()];
        probe
    }
    pub(crate) fn matched(&self) -> Option<usize> {
        (0..2).find(|&i| self.alive[i] && self.names[i].is_some_and(|name| self.seen[i] == name.len()))
    }
}

impl CharSink for KeyProbe {
    fn push_char(&mut self, c: char) {
        for i in 0..2 {
            if !self.alive[i] {
                continue;
            }
            let name = self.names[i].unwrap_or("");
            if self.seen[i] < name.len() && name.as_bytes()[self.seen[i]] as char == c {
                self.seen[i] += 1;
            } else {
                self.alive[i] = false;
            }
        }
    }
    fn takes_bulk(&self) -> bool {
        // A probe with no names discards like `Skip` does; a probe
        // that matches names needs every character while any name
        // can still match. Once every named slot is dead, no later
        // character can revive one (`push_char` only clears `alive`,
        // and `matched` reads only live slots), so the rest of the
        // string is discarded exactly as the per-character form
        // discards it and may be validated whole like `Skip`'s.
        (0..2).all(|i| self.names[i].is_none() || !self.alive[i])
    }
}

/// A strict UTF-8 decoder fed one byte at a time: a completed scalar is
/// returned, an incomplete sequence returns nothing yet, and an invalid
/// byte or scalar is an error. This is the validation `String::from_utf8`
/// applies to a whole string, done in flight.
pub(crate) struct Utf8Decoder {
    pending: u8,
    value: u32,
    floor: u32,
}

impl Utf8Decoder {
    pub(crate) fn new() -> Self {
        Utf8Decoder { pending: 0, value: 0, floor: 0 }
    }
    pub(crate) fn feed(&mut self, byte: u8) -> Option<Option<char>> {
        if self.pending == 0 {
            match byte {
                0x00..=0x7f => return Some(Some(byte as char)),
                0xc2..=0xdf => {
                    self.value = (byte & 0x1f) as u32;
                    self.floor = 0x80;
                    self.pending = 1;
                }
                0xe0..=0xef => {
                    self.value = (byte & 0x0f) as u32;
                    self.floor = 0x800;
                    self.pending = 2;
                }
                0xf0..=0xf4 => {
                    self.value = (byte & 0x07) as u32;
                    self.floor = 0x10000;
                    self.pending = 3;
                }
                _ => return None,
            }
            return Some(None);
        }
        if !(0x80..=0xbf).contains(&byte) {
            return None;
        }
        self.value = (self.value << 6) | (byte & 0x3f) as u32;
        self.pending -= 1;
        if self.pending > 0 {
            return Some(None);
        }
        let value = self.value;
        if value < self.floor || value > 0x10ffff || (0xd800..0xe000).contains(&value) {
            return None;
        }
        char::from_u32(value).map(Some)
    }
    pub(crate) fn complete(&self) -> bool {
        self.pending == 0
    }
}

/// The payload window: at most 16 KiB is held, refilled as the cursor
/// advances, and cut off at the payload size limit, exactly where a
/// capped read of the whole payload would end. A read failure ends the
/// window and marks the scan failed.
pub(crate) struct Scanner<R: Read> {
    pub(crate) source: R,
    // Read::read accepts initialized bytes and may inspect its input buffer.
    pub(crate) window: [u8; 16384],
    pub(crate) pos: usize,
    pub(crate) end: usize,
    pub(crate) eof: bool,
    pub(crate) failed: bool,
    pub(crate) budget: usize,
    pub(crate) fetched: usize,
}

impl<R: Read> Scanner<R> {
    pub(crate) fn new(source: R) -> Self {
        Scanner { source, window: [0; 16384], pos: 0, end: 0, eof: false, failed: false, budget: MAX_INPUT, fetched: 0 }
    }
    pub(crate) fn peek(&mut self) -> Option<u8> {
        if self.pos == self.end && !self.eof {
            if self.budget == 0 {
                self.eof = true;
            } else {
                let want = self.budget.min(self.window.len());
                match self.source.read(&mut self.window[..want]) {
                    Ok(0) => self.eof = true,
                    Ok(n) if n <= want => {
                        self.pos = 0;
                        self.end = n;
                        self.budget -= n;
                        self.fetched += n;
                    }
                    Ok(_) | Err(_) => {
                        self.eof = true;
                        self.failed = true;
                    }
                }
            }
        }
        // `pos < end`, and `end` is the length the last fill reported,
        // so this byte was written by that fill.
        if self.pos < self.end { Some(self.window[self.pos]) } else { None }
    }
    pub(crate) fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }
    /// The number of source bytes consumed so far: the bytes the
    /// fills have fetched, less the ones still unread in the window.
    /// When the source is a held payload this is the offset of the
    /// next byte in it, so a value walked between two readings is
    /// exactly the source span between them.
    pub(crate) fn byte_offset(&self) -> usize {
        self.fetched - (self.end - self.pos)
    }
    /// The unread bytes of the current window, refilling it first
    /// when it is spent: exactly the bytes `peek` and `bump` would
    /// serve, in order. The slice is confined to the prefix the last
    /// fill reported, so no byte is read before the fill that wrote
    /// it, and no byte past the payload budget is ever served.
    pub(crate) fn available(&mut self) -> &[u8] {
        self.peek();
        if self.pos < self.end {
            &self.window[self.pos..self.end]
        } else {
            &[]
        }
    }
    pub(crate) fn ws(&mut self) {
        // The whitespace set is the grammar's own four ASCII bytes,
        // so the skip is byte-exact: each window's maximal prefix of
        // those bytes is consumed whole, and the loop refills only
        // when a whole window was whitespace. The per-byte form
        // consumed the same prefix byte by byte through `peek`;
        // `available` peeks (and fills) on the same boundaries, so
        // the fills, the budget spent and the end-of-input and
        // failure states the skip leaves are the per-byte form's.
        loop {
            let (run, len) = {
                let window = self.available();
                (window.iter().position(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r')).unwrap_or(window.len()), window.len())
            };
            self.pos += run;
            if run < len || len == 0 {
                return;
            }
        }
    }
    pub(crate) fn eat(&mut self, byte: u8) -> Option<()> {
        self.ws();
        if self.peek() == Some(byte) {
            self.pos += 1;
            Some(())
        } else {
            None
        }
    }
    pub(crate) fn literal(&mut self, word: &str) -> Option<()> {
        for &byte in word.as_bytes() {
            if self.bump() != Some(byte) {
                return None;
            }
        }
        Some(())
    }
    pub(crate) fn hex4(&mut self) -> Option<u32> {
        // The same four bytes the whole-string parser hands to
        // `u32::from_str_radix` in base 16: hexadecimal digits, with
        // one leading `+` allowed.
        let mut value: u32 = 0;
        for i in 0..4 {
            let byte = self.bump()?;
            let digit = match byte {
                b'0'..=b'9' => (byte - b'0') as u32,
                b'a'..=b'f' => (byte - b'a') as u32 + 10,
                b'A'..=b'F' => (byte - b'A') as u32 + 10,
                b'+' if i == 0 => continue,
                _ => return None,
            };
            value = value * 16 + digit;
        }
        Some(value)
    }
    /// Scan one string, whose opening quote is the next byte, delivering
    /// its decoded characters to `sink`. The escapes, surrogate pairs,
    /// control characters and UTF-8 validity are the string rules of
    /// the parser above; a string that parser rejects rejects this scan.
    pub(crate) fn string<S: CharSink>(&mut self, sink: &mut S) -> Option<()> {
        if self.bump() != Some(b'"') {
            return None;
        }
        let mut utf8 = Utf8Decoder::new();
        loop {
            // A sink that discards its characters — an unwatched
            // string, or a key matched against no name — need not see
            // them one by one. When the decoder sits at a scalar
            // boundary, the maximal run of non-special bytes in the
            // current window (no quote, backslash or control byte) is
            // consumed whole if it is valid UTF-8 as it stands: a
            // valid run then ends at a boundary by construction, so
            // the decoder state is unchanged. A run the whole-run
            // validation refuses — an invalid byte in it, or a scalar
            // split by the window edge — falls back to the per-byte
            // path below without consuming anything, as do escapes,
            // control bytes and a decoder mid-scalar; the per-byte
            // path decides those exactly as it always has.
            if sink.takes_bulk() && utf8.complete() {
                let (run, valid) = {
                    let window = self.available();
                    let run = window.iter().position(|&b| matches!(b, b'"' | b'\\' | 0x00..=0x1f)).unwrap_or(window.len());
                    (run, std::str::from_utf8(&window[..run]).is_ok())
                };
                if run > 0 && valid {
                    self.pos += run;
                    continue;
                }
            }
            // A sink that collects the text whole takes the same
            // validated run as one slice, on the same terms: the
            // run is consumed only when it validates as it stands,
            // and a run it refuses falls back to the per-byte path
            // below without consuming anything, exactly as the
            // discarding form's refusal does.
            if sink.captures_run() && utf8.complete() {
                let (run, text) = {
                    let window = self.available();
                    let run = window.iter().position(|&b| matches!(b, b'"' | b'\\' | 0x00..=0x1f)).unwrap_or(window.len());
                    (run, std::str::from_utf8(&window[..run]).ok())
                };
                if run > 0 {
                    if let Some(text) = text {
                        sink.push_run(text);
                        self.pos += run;
                        continue;
                    }
                }
            }
            // A sink at the payload stage of a live envelope check
            // takes the same validated run for run processing, on
            // the same terms as the forms above: the run is handed
            // over only when it validates as UTF-8 as it stands,
            // and a run that validation refuses — an invalid byte
            // in it, or a scalar split by the window edge — falls
            // back to the per-byte path below without consuming
            // anything, so an invalid byte is rejected there
            // exactly as before. The run form consumes only the
            // run's whitespace-free prefix; when it consumes
            // nothing (the run begins with a whitespace character)
            // or less than the whole run, the unconsumed rest —
            // beginning with that whitespace character — is
            // delivered by the per-byte path below and by the
            // loop's later turns, unchanged.
            if sink.takes_payload_run() && utf8.complete() {
                let consumed = {
                    let window = self.available();
                    let run = window.iter().position(|&b| matches!(b, b'"' | b'\\' | 0x00..=0x1f)).unwrap_or(window.len());
                    // The validated text itself is handed over, as
                    // the collecting form above hands its text over:
                    // the sink receives the one validation's result
                    // instead of validating the same run a second
                    // time to recover it.
                    if run > 0 {
                        match std::str::from_utf8(&window[..run]) {
                            Ok(text) => sink.push_payload_run(text),
                            Err(_) => 0,
                        }
                    } else {
                        0
                    }
                };
                if consumed > 0 {
                    self.pos += consumed;
                    continue;
                }
            }
            // A sink at the timeout stage of a live envelope
            // check takes the same validated run for run
            // processing, on the same terms as the payload form
            // above: the run is handed over only when it
            // validates as UTF-8 as it stands, and a run that
            // validation refuses — an invalid byte in it, or a
            // scalar split by the window edge — falls back to
            // the per-byte path below without consuming
            // anything, so an invalid byte is rejected there
            // exactly as before. The run form consumes only
            // the run's digit-and-point prefix; when it consumes
            // nothing (the run begins with a byte that is
            // neither a digit nor the point) or less than the
            // whole run, the unconsumed rest is delivered by
            // the per-byte path below and by the loop's later
            // turns, unchanged. When it stops because the check
            // settled, the check is dead and the bulk path
            // above takes the rest of the string.
            if sink.takes_timeout_run() && utf8.complete() {
                let consumed = {
                    let window = self.available();
                    let run = window.iter().position(|&b| matches!(b, b'"' | b'\\' | 0x00..=0x1f)).unwrap_or(window.len());
                    // The validated text itself is handed over, as
                    // the payload form above hands its text over:
                    // the sink receives the one validation's result
                    // instead of validating the same run a second
                    // time to recover it.
                    if run > 0 {
                        match std::str::from_utf8(&window[..run]) {
                            Ok(text) => sink.push_timeout_run(text),
                            Err(_) => 0,
                        }
                    } else {
                        0
                    }
                };
                if consumed > 0 {
                    self.pos += consumed;
                    continue;
                }
            }
            // A sink at a whitespace stage of a live envelope
            // check takes the same validated run for run
            // processing, on the same terms as the payload form
            // above: the run is handed over only when it
            // validates as UTF-8 as it stands, and a run that
            // validation refuses — an invalid byte in it, or a
            // scalar split by the window edge — falls back to
            // the per-byte path below without consuming
            // anything, so an invalid byte is rejected there
            // exactly as before. The run form consumes only
            // the run's decoded-whitespace prefix; when it
            // consumes nothing (the run begins with a
            // non-whitespace character) or less than the whole
            // run, the unconsumed rest — beginning with that
            // character — is delivered by the per-byte path
            // below and by the loop's later turns, unchanged.
            if sink.takes_whitespace_run() && utf8.complete() {
                let consumed = {
                    let window = self.available();
                    let run = window.iter().position(|&b| matches!(b, b'"' | b'\\' | 0x00..=0x1f)).unwrap_or(window.len());
                    // The validated text itself is handed over, as
                    // the payload form above hands its text over:
                    // the sink receives the one validation's result
                    // instead of validating the same run a second
                    // time to recover it.
                    if run > 0 {
                        match std::str::from_utf8(&window[..run]) {
                            Ok(text) => sink.push_whitespace_run(text),
                            Err(_) => 0,
                        }
                    } else {
                        0
                    }
                };
                if consumed > 0 {
                    self.pos += consumed;
                    continue;
                }
            }
            let byte = self.bump()?;
            match byte {
                b'"' => return if utf8.complete() { Some(()) } else { None },
                b'\\' => {
                    let escape = self.bump()?;
                    let c = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let mut unit = self.hex4()?;
                            if (0xd800..0xdc00).contains(&unit) {
                                if self.bump() != Some(b'\\') || self.bump() != Some(b'u') {
                                    return None;
                                }
                                let low = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&low) {
                                    return None;
                                }
                                unit = 0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00);
                            }
                            char::from_u32(unit)?
                        }
                        _ => return None,
                    };
                    // The decoded character is delivered itself,
                    // not re-encoded and fed byte by byte through
                    // the decoder to be re-derived: a character's
                    // own encoding fed to a decoder at a scalar
                    // boundary produces exactly that character and
                    // leaves the decoder at a boundary, which is
                    // the whole effect the feed had. Mid-scalar it
                    // produced nothing at all but a rejection: the
                    // encoding's first byte is an ASCII or lead
                    // byte, never the continuation byte the decoder
                    // waits for, so its first feed already failed —
                    // the rejection the completeness test makes
                    // here directly.
                    if !utf8.complete() {
                        return None;
                    }
                    sink.push_char(c);
                }
                0x00..=0x1f => return None,
                _ => {
                    if let Some(done) = utf8.feed(byte)? {
                        sink.push_char(done);
                    }
                }
            }
        }
    }
    /// Scan one object key and report which of `names` it decodes to.
    pub(crate) fn key(&mut self, names: &[&'static str]) -> Option<Option<usize>> {
        let mut probe = KeyProbe::new(names);
        self.string(&mut probe)?;
        Some(probe.matched())
    }
    /// Scan one number token. The whole-string parser takes every byte
    /// of `-+.eE0-9` and hands the run to `f64` parsing, so the token
    /// is consumed whole here and its shape checked by the grammar
    /// that parsing accepts: an optional sign, digits with an optional
    /// fraction (or a fraction alone), and an optional exponent.
    pub(crate) fn number(&mut self) -> Option<()> {
        #[derive(PartialEq)]
        enum Shape {
            Start,
            Sign,
            Whole,
            WholeDot,
            Dot,
            Fraction,
            Exponent,
            ExponentSign,
            ExponentDigits,
            Dead,
        }
        let mut shape = Shape::Start;
        let mut any = false;
        while let Some(byte) = self.peek() {
            if !matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                break;
            }
            // In a digit-stable shape a digit is a self-loop:
            // `Whole`, `Fraction` and `ExponentDigits` each step
            // to themselves on a digit and change on nothing a
            // digit can do, and each is reached only by consuming
            // a digit (from `Start` or `Sign`, from `Dot` or
            // `WholeDot`, and from `Exponent` or `ExponentSign`
            // respectively), so `any` already stands in all
            // three. The maximal ASCII digit prefix of the
            // current window is therefore consumed whole —
            // digits are ASCII bytes that genuinely are the
            // characters — refilling only when a whole window
            // was digits (`available` peeks and fills on the
            // same boundaries the per-byte loop peeked on). The
            // first byte that is not a digit is not consumed
            // here: when the prefix is short of the window, or
            // empty because the peeked byte is not a digit, the
            // per-byte path below classifies that byte with its
            // transitions exactly as before.
            // In the dead shape every token byte is a self-loop:
            // the shape is absorbing (every transition out of it
            // lands back in it), so the maximal token-byte prefix
            // of the current window is consumed whole, on the same
            // terms as the digit runs below — `available` peeks
            // (and fills) on the same boundaries the per-byte loop
            // peeked on, so the consumed prefix, the fills, the
            // budget spent and the end-of-input and failure states
            // left behind are the per-byte form's, and the first
            // byte that is not a token byte is left to the loop's
            // own test above. `any` already stands in this shape:
            // it is reached only by consuming a token byte, which
            // the per-byte path marks as it consumes it.
            if shape == Shape::Dead {
                let run = {
                    let window = self.available();
                    window.iter().position(|&b| !matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')).unwrap_or(window.len())
                };
                if run > 0 {
                    self.pos += run;
                    any = true;
                    continue;
                }
            }
            if matches!(shape, Shape::Whole | Shape::Fraction | Shape::ExponentDigits) {
                let run = {
                    let window = self.available();
                    window.iter().position(|&b| !b.is_ascii_digit()).unwrap_or(window.len())
                };
                if run > 0 {
                    self.pos += run;
                    any = true;
                    continue;
                }
            }
            self.pos += 1;
            any = true;
            shape = match (shape, byte) {
                (Shape::Start, b'0'..=b'9') => Shape::Whole,
                (Shape::Start, b'+' | b'-') => Shape::Sign,
                (Shape::Start, b'.') => Shape::Dot,
                (Shape::Sign, b'0'..=b'9') => Shape::Whole,
                (Shape::Sign, b'.') => Shape::Dot,
                (Shape::Dot, b'0'..=b'9') => Shape::Fraction,
                (Shape::Whole, b'0'..=b'9') => Shape::Whole,
                (Shape::Whole, b'.') => Shape::WholeDot,
                (Shape::Whole, b'e' | b'E') => Shape::Exponent,
                (Shape::WholeDot, b'0'..=b'9') => Shape::Fraction,
                (Shape::WholeDot, b'e' | b'E') => Shape::Exponent,
                (Shape::Fraction, b'0'..=b'9') => Shape::Fraction,
                (Shape::Fraction, b'e' | b'E') => Shape::Exponent,
                (Shape::Exponent, b'0'..=b'9') => Shape::ExponentDigits,
                (Shape::Exponent, b'+' | b'-') => Shape::ExponentSign,
                (Shape::ExponentSign, b'0'..=b'9') => Shape::ExponentDigits,
                (Shape::ExponentDigits, b'0'..=b'9') => Shape::ExponentDigits,
                _ => Shape::Dead,
            };
        }
        if any && matches!(shape, Shape::Whole | Shape::WholeDot | Shape::Fraction | Shape::ExponentDigits) {
            Some(())
        } else {
            None
        }
    }
    /// Skip one whole value by structure, at nesting depth `depth`:
    /// the same shapes and the same depth limit as the parser above.
    pub(crate) fn value(&mut self, depth: usize) -> Option<()> {
        self.ws();
        if depth > 256 {
            return None;
        }
        match self.peek()? {
            b'{' => {
                self.pos += 1;
                if self.eat(b'}').is_none() {
                    loop {
                        self.ws();
                        self.key(&[])?;
                        self.eat(b':')?;
                        self.value(depth + 1)?;
                        if self.eat(b',').is_some() {
                            continue;
                        }
                        self.eat(b'}')?;
                        break;
                    }
                }
                Some(())
            }
            b'[' => {
                self.pos += 1;
                if self.eat(b']').is_none() {
                    loop {
                        self.value(depth + 1)?;
                        if self.eat(b',').is_some() {
                            continue;
                        }
                        self.eat(b']')?;
                        break;
                    }
                }
                Some(())
            }
            b'"' => self.string(&mut Skip),
            b't' => self.literal("true"),
            b'f' => self.literal("false"),
            b'n' => self.literal("null"),
            _ => self.number(),
        }
    }
}
