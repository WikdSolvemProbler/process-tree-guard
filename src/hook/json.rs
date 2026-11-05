// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! JSON in the hook: the value tree of the parsed verdict, and the writer the hook's answers are built with.

#[derive(Debug)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

// The accessors of the value tree are compiled for the tests only:
// they serve the tests that hold the parsed verdict (MEASUREMENT
// section 3's differential reference implementation) against the
// streaming scanner, while production code reads payloads through
// the scanner and builds its answers with `write_json`.
impl Json {
    #[cfg(test)]
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    #[cfg(test)]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
}

pub(crate) struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn ws(&mut self) {
        // The maximal prefix of the grammar's four whitespace bytes
        // is consumed whole, as the streaming scanner's skip
        // consumes it: the set is exactly those four ASCII bytes,
        // so finding the first byte outside the set with a slice
        // scan leaves `pos` exactly where the byte-at-a-time loop
        // left it, however long the run between two tokens (a
        // captured value may pad to the payload cap).
        let rest = &self.bytes[self.pos..];
        let run = rest.iter().position(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r')).unwrap_or(rest.len());
        self.pos += run;
    }
    fn eat(&mut self, b: u8) -> Option<()> {
        self.ws();
        if self.bytes.get(self.pos) == Some(&b) { self.pos += 1; Some(()) } else { None }
    }
    fn literal(&mut self, word: &str, value: Json) -> Option<Json> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) { self.pos += word.len(); Some(value) } else { None }
    }
    fn value(&mut self) -> Option<Json> {
        self.ws();
        self.depth += 1;
        if self.depth > 256 {
            return None;
        }
        let result = match *self.bytes.get(self.pos)? {
            b'{' => {
                self.pos += 1;
                let mut pairs = Vec::new();
                if self.eat(b'}').is_none() {
                    loop {
                        self.ws();
                        let key = self.string()?;
                        self.eat(b':')?;
                        pairs.push((key, self.value()?));
                        if self.eat(b',').is_some() { continue; }
                        self.eat(b'}')?;
                        break;
                    }
                }
                Json::Object(pairs)
            }
            b'[' => {
                self.pos += 1;
                let mut items = Vec::new();
                if self.eat(b']').is_none() {
                    loop {
                        items.push(self.value()?);
                        if self.eat(b',').is_some() { continue; }
                        self.eat(b']')?;
                        break;
                    }
                }
                Json::Array(items)
            }
            b'"' => Json::Str(self.string()?),
            b't' => self.literal("true", Json::Bool(true))?,
            b'f' => self.literal("false", Json::Bool(false))?,
            b'n' => self.literal("null", Json::Null)?,
            _ => {
                let start = self.pos;
                // The token is the maximal run of the token bytes,
                // consumed whole as the whitespace skip above
                // consumes its run: the byte-at-a-time loop advanced
                // `pos` past exactly this prefix and nothing else
                // reads the intermediate positions, so the slice the
                // `f64` parse is handed — and the position it starts
                // from — are the loop's.
                let rest = &self.bytes[self.pos..];
                let run = rest.iter().position(|&b| !matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')).unwrap_or(rest.len());
                self.pos += run;
                let text = std::str::from_utf8(&self.bytes[start..self.pos]).ok()?;
                Json::Number(text.parse().ok()?)
            }
        };
        self.depth -= 1;
        Some(result)
    }
    fn hex4(&mut self) -> Option<u32> {
        let text = std::str::from_utf8(self.bytes.get(self.pos..self.pos + 4)?).ok()?;
        self.pos += 4;
        u32::from_str_radix(text, 16).ok()
    }
    fn string(&mut self) -> Option<String> {
        if self.bytes.get(self.pos) != Some(&b'"') {
            return None;
        }
        self.pos += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            // Copy the plain run up to the next special byte (quote,
            // backslash or a control character) as one slice: the
            // whole-string UTF-8 validation at the closing quote
            // covers the copied bytes exactly as it covered the
            // byte-at-a-time form.
            let rest = &self.bytes[self.pos..];
            let run = rest.iter().position(|&b| matches!(b, b'"' | b'\\' | 0..=0x1f)).unwrap_or(rest.len());
            out.extend_from_slice(&rest[..run]);
            self.pos += run;
            let b = *self.bytes.get(self.pos)?;
            self.pos += 1;
            match b {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let e = *self.bytes.get(self.pos)?;
                    self.pos += 1;
                    let c = match e {
                        b'"' => '"', b'\\' => '\\', b'/' => '/', b'b' => '\u{8}', b'f' => '\u{c}',
                        b'n' => '\n', b'r' => '\r', b't' => '\t',
                        b'u' => {
                            let mut unit = self.hex4()?;
                            if (0xD800..0xDC00).contains(&unit) {
                                if self.bytes.get(self.pos..self.pos + 2)? != b"\\u" { return None; }
                                self.pos += 2;
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) { return None; }
                                unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                            }
                            char::from_u32(unit)?
                        }
                        _ => return None,
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                0..=0x1f => return None,
                _ => out.push(b),
            }
        }
    }
}

pub fn parse_json(text: &str) -> Option<Json> {
    let mut parser = Parser { bytes: text.as_bytes(), pos: 0, depth: 0 };
    let value = parser.value()?;
    parser.ws();
    if parser.pos == parser.bytes.len() { Some(value) } else { None }
}

/// Append the compact JSON text of `value` to `out`, in one pass: no
/// intermediate text is built for any part of the value.
pub(crate) fn write_json(value: &Json, out: &mut String) {
    use std::fmt::Write as _;
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Number(n) if n.is_finite() => {
            let _ = write!(out, "{}", n);
        }
        Json::Number(_) => out.push_str("null"),
        Json::Str(s) => write_quote(s, out),
        Json::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(item, out);
            }
            out.push(']');
        }
        Json::Object(pairs) => {
            out.push('{');
            for (i, (k, v)) in pairs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_quote(k, out);
                out.push(':');
                write_json(v, out);
            }
            out.push('}');
        }
    }
}

/// The whole-string form of `write_quote`, retained for the tests
/// that hold answers against it; production answers escape straight
/// into their buffers with `write_quote`.
#[cfg(test)]
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    write_quote(text, &mut out);
    out
}

/// Append `text` to `out` as a quoted JSON string.
pub(crate) fn write_quote(text: &str, out: &mut String) {
    out.push('"');
    write_escaped(text, out);
    out.push('"');
}

/// Append `text` to `out` escaped as the body of a JSON string,
/// without the surrounding quotes: the per-character work of
/// `write_quote`, shared with a caller that assembles a quoted
/// string from several pieces in the one buffer it returns.
pub(crate) fn write_escaped(text: &str, out: &mut String) {
    use std::fmt::Write as _;
    // The escape decision is byte-decidable: every byte that needs
    // an escape is ASCII below 0x20 or one of the two ASCII marks,
    // and every other byte — a plain ASCII byte, 0x7f included, or
    // any byte of a multi-byte character, whose bytes are all at
    // least 0x80 and so decode to a character above 0x20 that is
    // neither mark — is copied as it stands, exactly as the
    // per-character form pushed its character. A maximal plain run
    // is therefore copied as one slice of the text instead of being
    // decoded character by character and re-encoded per character:
    // the answers this builds (a rewritten command, a whole
    // re-serialised `tool_input`) are mostly plain runs. Every
    // special byte is itself a whole character, so the run slices
    // stand on character boundaries.
    let bytes = text.as_bytes();
    let mut run_start = 0usize;
    for (index, &byte) in bytes.iter().enumerate() {
        let escape = match byte {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0..=0x1f => {
                out.push_str(&text[run_start..index]);
                // Written into the buffer, not formatted into a
                // throwaway string per control character and copied
                // back out of it. The byte is the character: both
                // are the ASCII value below 0x20.
                let _ = write!(out, "\\u{:04x}", byte as u32);
                run_start = index + 1;
                continue;
            }
            _ => continue,
        };
        out.push_str(&text[run_start..index]);
        out.push_str(escape);
        run_start = index + 1;
    }
    out.push_str(&text[run_start..]);
}
