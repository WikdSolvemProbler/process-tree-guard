// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Standard Base64 and the encoded command text the guard accepts.

use crate::GuardError;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Strict standard base64: full quartets, padding only at the end.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return None;
    }
    // Only four facts are ever read from this count — that the
    // padding is 0, 1, 2, or more than 2 (the refusal below, and the
    // body slice and the tail handling on a count of at most 2) —
    // so the count stops at 3: a trailing run of `=` past that, of
    // whatever length a value carries, was walked in full only to
    // be refused by the check below.
    let padding = bytes.iter().rev().take_while(|&&b| b == b'=').take(3).count();
    if padding > 2 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let body = &bytes[..bytes.len() - padding];
    // The value of a body byte, by range: the same mapping the
    // streaming payload checker in `hook.rs` applies per character.
    // A linear scan of the alphabet per byte cost up to 64
    // comparisons for every character decoded.
    let value = |b: u8| -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a') as u32 + 26),
            b'0'..=b'9' => Some((b - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    // Whole quartets are decoded as one group, as the streaming
    // payload checker's quartet step in `hook.rs` decodes them: the
    // four values shift into one group in order and its three bytes
    // are emitted at once, where the per-byte form shifted one value
    // at a time and tested the position modulo four on every byte
    // to find the quartet's end. A byte outside the alphabet fails
    // the whole decode in either form, and no other state is kept,
    // so which byte of a quartet is seen first decides nothing.
    let (quartets, remainder) = body.as_chunks::<4>();
    for quartet in quartets {
        let group = (value(quartet[0])? << 18)
            | (value(quartet[1])? << 12)
            | (value(quartet[2])? << 6)
            | value(quartet[3])?;
        out.extend_from_slice(&[(group >> 16) as u8, (group >> 8) as u8, group as u8]);
    }
    // The tail the quartets leave is the partial group the per-byte
    // form accumulated last: three bytes when the padding is one,
    // two when it is two, and none when there is no padding (the
    // length is in whole quartets, so those are the only shapes).
    // It is accumulated and shifted exactly as that form did.
    let mut acc: u32 = 0;
    for &b in remainder {
        acc = (acc << 6) | value(b)?;
    }
    match padding {
        // Unused trailing bits are ignored, as the reference decoder does.
        1 => { acc <<= 6; out.extend_from_slice(&[(acc >> 16) as u8, (acc >> 8) as u8]); }
        2 => { acc <<= 12; out.push((acc >> 16) as u8); }
        _ => {}
    }
    Some(out)
}

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        // A full chunk takes all four alphabet characters: the
        // per-character test of the position against the chunk's
        // length is decided for the whole group at once, as in
        // `append_group` below, instead of on every character.
        if chunk.len() == 3 {
            out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
            out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
            out.push(ALPHABET[(n >> 6 & 63) as usize] as char);
            out.push(ALPHABET[(n & 63) as usize] as char);
            continue;
        }
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Encode PowerShell's UTF-16LE text in one pass, without an intermediate
/// byte vector. The result is the same standard Base64 as base64_encode.
pub(crate) fn base64_encode_utf16le(text: &str) -> String {
    // Capacity from the byte length alone: a character contributes at
    // most one UTF-16 unit per UTF-8 byte it occupies (a four-byte
    // character contributes two units), so the unit count never exceeds
    // `text.len()` and this bound covers the exact size. Counting the
    // units first would walk the whole text a second time before the
    // encoding pass.
    let mut out = String::with_capacity((text.len() * 2).div_ceil(3) * 4);
    let mut group = [0u8; 3];
    let mut filled = 0usize;
    for unit in text.encode_utf16() {
        for byte in unit.to_le_bytes() {
            group[filled] = byte;
            filled += 1;
            if filled == 3 {
                append_group(&mut out, &group, filled);
                group = [0; 3];
                filled = 0;
            }
        }
    }
    if filled != 0 { append_group(&mut out, &group, filled); }
    out
}

fn append_group(out: &mut String, bytes: &[u8; 3], filled: usize) {
    let n = (bytes[0] as u32) << 16 | (bytes[1] as u32) << 8 | bytes[2] as u32;
    // A full group takes all four alphabet characters: the
    // per-character test of the position against `filled` is
    // decided for the whole group at once, instead of on every
    // character of every group the encoder emits.
    if filled == 3 {
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 6 & 63) as usize] as char);
        out.push(ALPHABET[(n & 63) as usize] as char);
        return;
    }
    for i in 0..4 {
        out.push(if i <= filled {
            ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char
        } else { '=' });
    }
}

/// A strict UTF-8 decoder fed one byte at a time: a completed scalar is
/// returned, an incomplete sequence returns nothing yet, and an invalid
/// byte or scalar is an error. This is the validation `String::from_utf8`
/// applies to a whole string, done in flight. It is a second, independent
/// re-implementation of the streaming scanner's decoder in `hook.rs`, so
/// the differential between the parsed and streaming verdicts keeps
/// crossing two implementations.
struct Utf8Decoder {
    pending: u8,
    value: u32,
    floor: u32,
}

impl Utf8Decoder {
    fn new() -> Self {
        Utf8Decoder { pending: 0, value: 0, floor: 0 }
    }
    fn feed(&mut self, byte: u8) -> Option<Option<char>> {
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
    fn complete(&self) -> bool {
        self.pending == 0
    }
}

/// The in-flight verdict state of `command_text_is_valid`: the decoded
/// bytes are checked as they are produced, and no decoded byte is kept.
struct CommandCheck {
    utf8: Utf8Decoder,
    utf8_bad: bool,
    nul: bool,
    solid: bool,
}

impl CommandCheck {
    fn new() -> Self {
        CommandCheck { utf8: Utf8Decoder::new(), utf8_bad: false, nul: false, solid: false }
    }
    fn emit(&mut self, byte: u8) {
        if byte == 0 {
            self.nul = true;
        }
        match self.utf8.feed(byte) {
            Some(Some(c)) => {
                if !c.is_whitespace() {
                    self.solid = true;
                }
            }
            Some(None) => {}
            None => self.utf8_bad = true,
        }
    }
}

/// Whether `value` is a command text `decode_command` accepts, decided
/// with no owned storage: the strict quartet and padding walk of
/// `base64_decode` runs in place, each decoded byte is fed to the
/// incremental UTF-8 validator above as it is produced, and the only
/// verdict state kept is that validator, a NUL flag, and whether any
/// decoded character is not whitespace. The acceptance is exactly
/// `decode_command`'s for every input: an empty value or a length not
/// in whole quartets fails, padding is at most two `=` and only at the
/// end (a body byte outside the alphabet fails, `=` in the body
/// included), the unused trailing bits of the last group are ignored,
/// the decoded bytes must be valid UTF-8 ending at a scalar boundary,
/// no decoded byte may be NUL, and the decoded text must be neither
/// empty nor all whitespace (the `trim().is_empty()` test, taken per
/// decoded character through `char::is_whitespace`). Callers that need
/// the decoded text itself keep using `decode_command`.
pub fn command_text_is_valid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return false;
    }
    // The padding count is capped at 3, as in `base64_decode`:
    // only 0, 1, 2 and more-than-2 are distinguished.
    let padding = bytes.iter().rev().take_while(|&&b| b == b'=').take(3).count();
    if padding > 2 {
        return false;
    }
    let mut check = CommandCheck::new();
    let body = &bytes[..bytes.len() - padding];
    let digit = |b: u8| -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a') as u32 + 26),
            b'0'..=b'9' => Some((b - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    // Whole quartets are decoded as one group, as in
    // `base64_decode` above and in the streaming payload checker's
    // quartet step: the four values shift into one group in order
    // and its three bytes are emitted at once, where the per-byte
    // form shifted one value at a time and tested the position
    // modulo four on every byte to find the quartet's end. The
    // settled-answer test below runs at the same points as that
    // form's — after each whole quartet's three emitted bytes, and
    // never inside a quartet — so it returns at exactly the same
    // quartets. A byte outside the alphabet fails the whole value
    // in either form.
    let (quartets, remainder) = body.as_chunks::<4>();
    for quartet in quartets {
        let Some(group) = (match (digit(quartet[0]), digit(quartet[1]), digit(quartet[2]), digit(quartet[3])) {
            (Some(a), Some(b), Some(c), Some(d)) => Some((a << 18) | (b << 12) | (c << 6) | d),
            _ => None,
        }) else {
            return false;
        };
        check.emit((group >> 16) as u8);
        check.emit((group >> 8) as u8);
        check.emit(group as u8);
        // A byte the decoder refused, or a NUL byte, settles
        // the answer false for the whole value: both flags are
        // set once and never cleared, the result below is their
        // negation among its conjuncts, and the check has no
        // other effect, so the rest of the body is not walked.
        // (The alphabet test above already returns at once in
        // the same way.)
        if check.utf8_bad || check.nul {
            return false;
        }
    }
    // The tail the quartets leave is the partial group the
    // per-byte form accumulated last (three bytes when the
    // padding is one, two when it is two, none when there is no
    // padding), accumulated and shifted exactly as that form did.
    let mut acc: u32 = 0;
    for &b in remainder {
        let Some(value) = digit(b) else { return false };
        acc = (acc << 6) | value;
    }
    match padding {
        // Unused trailing bits are ignored, as in `base64_decode`.
        1 => {
            acc <<= 6;
            check.emit((acc >> 16) as u8);
            check.emit((acc >> 8) as u8);
        }
        2 => {
            acc <<= 12;
            check.emit((acc >> 16) as u8);
        }
        _ => {}
    }
    !check.utf8_bad && check.utf8.complete() && !check.nul && check.solid
}

/// Whether `text` passes the command acceptance the callers share:
/// nonempty after trimming, and no NUL anywhere. One pass settles
/// both halves: a character that is not whitespace (the end-trim
/// emptiness test — the trim strips exactly the
/// `char::is_whitespace` characters) and a NUL anywhere. The two
/// tests as separate calls walked the text twice — the trim from
/// both ends and the NUL search over the whole text. A NUL ends
/// the walk: the refusal is the same whichever half refuses, as
/// the combined condition's refusal was.
/// The pass ends at the first solid character, not at the text's
/// end: once a non-whitespace character stands, the emptiness half
/// is settled true for every continuation, and the only fact the
/// rest of the text can still decide is a NUL in it. A NUL is the
/// single byte 0x00, and no multi-byte character contains a 0x00
/// byte (its bytes are all at least 0x80), so that one remaining
/// test is a byte scan of the rest, exact where a per-character
/// walk re-decoded every character and re-tested its whitespace
/// for an answer already settled — a command may run to the
/// payload cap, so the settled tail is the long part.
pub fn nonempty_without_nul(text: &str) -> bool {
    for (index, c) in text.char_indices() {
        if c == '\0' {
            return false;
        }
        if !c.is_whitespace() {
            return !text.as_bytes()[index + c.len_utf8()..].contains(&0);
        }
    }
    false
}

/// The command text a `--command-b64` value holds: nonempty UTF-8 with no NUL.
pub fn decode_command(value: &str) -> Result<String, GuardError> {
    let bytes = base64_decode(value).ok_or_else(|| GuardError("--command-b64 is not valid base64".into()))?;
    let command = String::from_utf8(bytes).map_err(|_| GuardError("--command-b64 is not valid UTF-8".into()))?;
    if !nonempty_without_nul(&command) {
        return Err(GuardError("command must be nonempty and contain no NUL".into()));
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_utf16le_stream_matches_byte_encoder() {
        for text in ["", "echo x", "a\\b \"quoted\"", "É中😀", "\0", &"💬/\\".repeat(400)] {
            let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
            assert_eq!(base64_encode_utf16le(text), base64_encode(&bytes), "{text:?}");
        }
    }

    #[test]
    fn base64_round_trip_and_strictness() {
        for text in ["", "a", "ab", "abc", "abcd", "Write-Output 'guard-ok'"] {
            let encoded = base64_encode(text.as_bytes());
            if !text.is_empty() {
                assert_eq!(base64_decode(&encoded).unwrap(), text.as_bytes());
            }
        }
        assert_eq!(base64_encode(b"Write-Output 'guard-ok'"), "V3JpdGUtT3V0cHV0ICdndWFyZC1vayc=");
        for bad in ["////=", "abc", "ab=c", "a===", "Zm9v!", "Zg=x"] {
            assert!(base64_decode(bad).is_none(), "{}", bad);
        }
        // The allocation-free validator holds equal to the decoding
        // check on every shape below: valid texts, padding shapes
        // (canonical and not, the unused trailing bits ignored),
        // split and invalid scalars (truncated sequences, overlong
        // forms, surrogate encodings, a scalar past the range),
        // all-whitespace and NUL payloads, alphabet violations, and
        // lengths outside whole quartets.
        let mut corpus: Vec<String> = [
            "", "=", "==", "===", "====", "AAA=", "AA==", "A===", "AB==", "ABC=", "ABCD",
            "////=", "abc", "ab=c", "a===", "Zm9v!", "Zg=x", "Zg==", "Zh==", "Zg=", "Zg===",
            "Zm9v", "Zm9vYg==", "Zm9vYg=", "Zm9vYg", "AAAA", "AAA", "AA", "A", "===A", "A=AA",
            "AA=A", "Zm9v Zg==", "Zm9v\nZg==", "éééé", "😀😀", "AB+/",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for text in [
            "a", "ab", "abc", "Write-Output 'guard-ok'", " ", "  \t\r\n ", "\u{a0}\u{3000}",
            "\0", "a\0b", "é", "É中😀", "e\u{301}", " x ", "\u{2003}command\u{2003}",
        ] {
            corpus.push(base64_encode(text.as_bytes()));
        }
        for bytes in [
            &[0xc3][..], &[0xe2, 0x82][..], &[0xf0, 0x9f, 0x98][..], &[0x80][..], &[0xbf, 0x80][..],
            &[0xc0, 0x80][..], &[0xc1, 0xbf][..], &[0xe0, 0x80, 0x80][..], &[0xed, 0xa0, 0x80][..],
            &[0xed, 0xbf, 0xbf][..], &[0xf0, 0x80, 0x80, 0x80][..], &[0xf4, 0x90, 0x80, 0x80][..],
            &[0xf5, 0x80, 0x80, 0x80][..], &[0x00][..], &[0x20, 0x00][..], &[0xff][..], &[0x61, 0xc3][..],
        ] {
            corpus.push(base64_encode(bytes));
        }
        for candidate in &corpus {
            assert_eq!(command_text_is_valid(candidate), decode_command(candidate).is_ok(), "{candidate:?}");
        }
    }
}
