// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The envelope check: whether a watched command string is exactly the guard's envelope form, checked as it decodes.

use super::*;

// ------------------------------------------------------- envelope checking
//
// The envelope verdict on one watched command string, computed as its
// decoded characters arrive. The stages walk the exact form
// `envelope_for` checks: leading whitespace, the optional `& `, one of
// the envelope paths, the timeout token, the shell name, and the base64
// payload, with trailing whitespace allowed only after the payload.
// Each stage keeps only the state it needs.

pub(crate) const TIMEOUT_PREFIX: &str = " --timeout ";
pub(crate) const SHELL_PREFIX: &str = "--shell ";
pub(crate) const PAYLOAD_PREFIX: &str = " --command-b64 ";
pub(crate) const SHELL_NAMES: [&str; 3] = ["powershell", "bash", "cmd"];

/// The fraction digits of 2^-37, the half unit in the last place above
/// 86400: a fraction past it parses to a value over the cap.
/// This midpoint alone is derived from `crate::MAX_SECONDS` at its
/// current value; the comparisons themselves read that constant.
pub(super) const TIMEOUT_MID: &[u8] = b"0000000000072759576141834259033203125";
/// The leading significant digits of 2^-1075, half of the smallest
/// subnormal above zero, whose first digit sits at fraction place 324:
/// a smaller fraction parses to zero.
pub(crate) const ZERO_MID: &[u8] = b"24703282292062327";

/// The `--timeout` token of an envelope, checked as its characters
/// arrive. A token of at most 32 bytes is kept whole and handed to
/// `f64` parsing at the end: the check the whole-string form makes.
/// A longer token is settled from counters alone: the significant
/// digits of the whole part (its first five places and a capped
/// count), where the fraction's first nonzero digit sits, and how the
/// fraction compares with the two rounding boundaries above.
pub(crate) struct TimeoutCheck {
    kept: [u8; 32],
    bytes: usize,
    bad: bool,
    dot: bool,
    whole_digits: usize,
    whole_value: u32,
    whole_significant: usize,
    whole_nonzero: bool,
    fraction_digits: usize,
    fraction_mid: i8,
    fraction_sub: i8,
}

impl TimeoutCheck {
    pub(crate) fn new() -> Self {
        TimeoutCheck {
            kept: [0; 32], bytes: 0, bad: false, dot: false,
            whole_digits: 0, whole_value: 0, whole_significant: 0, whole_nonzero: false,
            fraction_digits: 0, fraction_mid: 0, fraction_sub: 0,
        }
    }
    fn push_char(&mut self, c: char) {
        let start = self.bytes;
        self.bytes += c.len_utf8();
        if start < self.kept.len() && c.is_ascii() {
            self.kept[start] = c as u8;
        }
        match c {
            '.' if !self.dot => self.dot = true,
            '0'..='9' => {
                let digit = c as u32 - '0' as u32;
                if self.dot {
                    self.fraction_digits += 1;
                    let place = self.fraction_digits;
                    if self.fraction_mid == 0 {
                        let bound = if place <= TIMEOUT_MID.len() { (TIMEOUT_MID[place - 1] - b'0') as u32 } else { 0 };
                        if digit < bound {
                            self.fraction_mid = -1;
                        } else if digit > bound {
                            self.fraction_mid = 1;
                        }
                    }
                    if self.fraction_sub == 0 {
                        let bound = if place < 324 {
                            0
                        } else if place - 324 < ZERO_MID.len() {
                            (ZERO_MID[place - 324] - b'0') as u32
                        } else {
                            0
                        };
                        if digit < bound {
                            self.fraction_sub = -1;
                        } else if digit > bound {
                            self.fraction_sub = 1;
                        }
                    }
                } else {
                    self.whole_digits += 1;
                    if digit != 0 {
                        self.whole_nonzero = true;
                    }
                    if digit != 0 || self.whole_significant > 0 {
                        if self.whole_significant < 5 {
                            self.whole_value = self.whole_value * 10 + digit;
                        }
                        self.whole_significant = (self.whole_significant + 1).min(6);
                    }
                }
            }
            _ => self.bad = true,
        }
    }
    /// Consume the maximal prefix of `run` that the per-character
    /// feed would step through changing nothing but two counters,
    /// and report its length in bytes: `bytes`, and the one digit
    /// count of the phase. The step is taken only where that is
    /// the whole effect of every stepped character, only on a
    /// live, unsettled check, and only past the kept prefix
    /// (`bytes` above the kept length, where a per-character step
    /// writes no kept byte and `finish` reads `bytes` only
    /// against that bound), so no stepped character settles the
    /// check and the feeds' stop conditions — which test
    /// `settled_false` after each per-character feed — have
    /// nothing to test across a step:
    ///
    /// - In the fraction phase with both rounding comparisons
    ///   already decided (`fraction_mid` and `fraction_sub` both
    ///   nonzero), a digit step consults neither comparison —
    ///   each is guarded on its undecided state — and adds one
    ///   to `bytes` and one to `fraction_digits` and nothing
    ///   else; the maximal ASCII digit run is consumed whole.
    /// - In the whole phase with no significant digit yet
    ///   (`whole_significant` zero, so `whole_nonzero` stands
    ///   false and `whole_value` stands zero), a zero step adds
    ///   one to `bytes` and one to `whole_digits` and nothing
    ///   else — a nonzero digit is not of that shape, so the
    ///   maximal zero run is consumed whole.
    /// - In the fraction phase at or past place 341, both bounds
    ///   a zero step consults are zero — the midpoint bound past
    ///   its 37 places, and the subnormal bound past its last
    ///   place, 340 — and a zero digit equals a zero bound, so
    ///   the step decides neither comparison however they stand,
    ///   and adds one to `bytes` and one to `fraction_digits`
    ///   and nothing else; the maximal zero run is consumed
    ///   whole.
    ///
    /// The runs are scanned as bytes: the digits the
    /// per-character feed steps on are the characters
    /// `'0'..='9'`, which are ASCII, so a digit byte is the
    /// character it names, and a multi-byte character's bytes,
    /// all at or past 0x80, end a run on that character's own
    /// boundary.
    fn push_run(&mut self, run: &str) -> usize {
        if self.bad || self.settled_false() || self.bytes <= self.kept.len() {
            return 0;
        }
        let bytes = run.as_bytes();
        let n = if self.dot {
            if self.fraction_mid != 0 && self.fraction_sub != 0 {
                bytes.iter().position(|&b| !b.is_ascii_digit()).unwrap_or(bytes.len())
            } else if self.fraction_digits >= 341 {
                bytes.iter().position(|&b| b != b'0').unwrap_or(bytes.len())
            } else {
                return 0;
            }
        } else if self.whole_significant == 0 {
            bytes.iter().position(|&b| b != b'0').unwrap_or(bytes.len())
        } else {
            return 0;
        };
        self.bytes += n;
        if self.dot {
            self.fraction_digits += n;
        } else {
            self.whole_digits += n;
        }
        n
    }
    /// Whether the answer is already settled false whatever
    /// characters follow: `bad` is set once and never cleared while
    /// the token is fed, and `finish` refuses whenever it stands.
    /// The counter states below are settled the same way, each by
    /// the arithmetic `finish` itself applies. A whole part past
    /// the cap by a whole unit — six significant digits (a value
    /// of at least 100,000 however the fraction falls), or a
    /// five-digit value above 86400 — stays past it under every
    /// continuation: later whole digits only grow it, a point and
    /// fraction leave the whole part standing, and any other
    /// character sets `bad`. The 86400 boundary with its fraction
    /// already past the rounding bound (`fraction_mid` 1, a state
    /// that state never leaves) parses over the cap on the short
    /// path and is refused by the counter path. And a zero whole
    /// part whose fraction is already below the smallest
    /// subnormal's half (`fraction_sub` -1, likewise final, and
    /// reachable only past the 324th fraction place, so only the
    /// counter path reads it) parses to zero however it continues,
    /// the one verdict `finish` gives a token with no nonzero
    /// whole digit. An envelope checker that sees this stops
    /// feeding the token and lets the rest of the string be
    /// validated whole, as it does for its own settled stages.
    fn settled_false(&self) -> bool {
        self.bad
            || self.whole_significant >= 6
            || self.whole_value > crate::MAX_SECONDS as u32
            || (self.whole_value == crate::MAX_SECONDS as u32 && self.fraction_mid == 1)
            || (!self.whole_nonzero && self.fraction_sub == -1)
    }
    fn finish(&self) -> bool {
        // The token rules of the whole-string form: a nonempty whole
        // part of digits, and a fraction of digits when a point is
        // present (so `.5` and `5.` both fail, as does any other
        // character, `e` included).
        if self.bad || self.whole_digits == 0 || (self.dot && self.fraction_digits == 0) {
            return false;
        }
        if self.bytes <= self.kept.len() {
            let text = std::str::from_utf8(&self.kept[..self.bytes]).unwrap_or("");
            return text.parse::<f64>().is_ok_and(|t| t > 0.0 && t <= crate::MAX_SECONDS);
        }
        // A token past the kept bytes, settled from the counters. A
        // parsed value over 86400 by at least one whole unit, or under
        // it, is decided by the whole part alone; the boundaries are
        // decided by the fraction comparisons.
        if self.whole_significant >= 6 || self.whole_value > crate::MAX_SECONDS as u32 {
            return false;
        }
        if self.whole_significant == 5 && self.whole_value == crate::MAX_SECONDS as u32 {
            return self.fraction_mid <= 0;
        }
        if self.whole_nonzero {
            return true;
        }
        self.fraction_sub > 0
    }
}

/// The base64 payload of an envelope, validated as its characters
/// arrive: the strict shape of `base64_decode` (a length in whole
/// quartets, at most two trailing `=` and only at the end), with the
/// decoded bytes checked in flight: valid UTF-8, no NUL, and not all
/// whitespace, as `decode_command` requires of the decoded text.
pub(crate) struct PayloadCheck {
    bad: bool,
    total: usize,
    body: usize,
    padding: usize,
    group: u32,
    grouped: usize,
    utf8: Utf8Decoder,
    utf8_bad: bool,
    nul: bool,
    decoded: bool,
    solid: bool,
}

impl PayloadCheck {
    pub(crate) fn new() -> Self {
        PayloadCheck {
            bad: false, total: 0, body: 0, padding: 0, group: 0, grouped: 0,
            utf8: Utf8Decoder::new(), utf8_bad: false, nul: false, decoded: false, solid: false,
        }
    }
    /// Whether the answer is already settled false whatever
    /// characters follow: each flag read here is set once and never
    /// cleared while the payload is fed, and `finish` refuses
    /// whenever any of them stands; the padding count only grows,
    /// past the most `finish` accepts. An envelope checker that
    /// sees this stops feeding the payload and lets the rest of
    /// the string be validated whole, as it does for its own
    /// settled stages.
    fn settled_false(&self) -> bool {
        self.bad || self.padding > 2 || self.utf8_bad || self.nul
    }
    fn push_char(&mut self, c: char) {
        self.total += 1;
        if c == '=' {
            self.padding += 1;
            return;
        }
        if self.padding > 0 {
            self.bad = true;
            return;
        }
        let value = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => {
                self.bad = true;
                return;
            }
        };
        self.body += 1;
        self.group = (self.group << 6) | value;
        self.grouped += 1;
        if self.grouped == 4 {
            let group = self.group;
            self.emit((group >> 16) as u8);
            self.emit((group >> 8) as u8);
            self.emit(group as u8);
            self.group = 0;
            self.grouped = 0;
        }
    }
    /// Process the whitespace-free prefix of one validated plain
    /// run, in quartets taken straight from the run's bytes, and
    /// report the prefix's length in bytes. The run is valid UTF-8
    /// as it stands (its caller validated it whole and hands it
    /// over as text, so no validation is repeated here), so its
    /// characters decode in place; the first character for which
    /// `char::is_whitespace` holds — the full Unicode set — ends
    /// the prefix, and that character and the rest of the run are
    /// left to the per-character path, so whitespace is never
    /// delivered here and never counted in `total`, exactly as the
    /// envelope stage never delivers it to `push_char`. Inside the
    /// prefix, a step of four alphabet bytes standing at a group
    /// boundary with no padding seen is taken as one quartet: the
    /// same four `push_char` steps would add four to `total` and
    /// to `body`, shift the four values into `group` in order and
    /// emit the group's three bytes, leaving `group` and `grouped`
    /// at zero — which is what the quartet step does directly,
    /// whatever `bad` already holds, because a `push_char` step
    /// for an alphabet byte performs that work whether or not an
    /// earlier byte set `bad`. Every other character of the
    /// prefix — a partial group carried across a window edge, a
    /// padding byte, a byte after padding, or a non-alphabet
    /// intrusion of any length — goes through `push_char` itself,
    /// so its `total`, `padding`, `body`, `group` and `grouped`
    /// effects are that form's by construction, and the group
    /// state those steps leave is the state the next run or the
    /// per-character tail carries on from.
    /// The prefix is not found in a separate pass: a pre-pass that
    /// decoded every character of the run with `char::is_whitespace`
    /// walked the whole run once only to walk it again here. A
    /// quartet step can never cross the prefix's end — a whitespace
    /// character holds no alphabet byte (its ASCII forms are not in
    /// the alphabet, and a multi-byte form's bytes are all at least
    /// 0x80), so a step of four alphabet bytes lies wholly inside
    /// the prefix — and every other character is decoded at its own
    /// position and tested there: the loop stops before the first
    /// whitespace character, having delivered exactly the prefix,
    /// and reports its byte length, the position it stopped at.
    fn push_payload_run(&mut self, text: &str) -> usize {
        let bytes = text.as_bytes();
        let mut index = 0usize;
        while index < bytes.len() {
            // A step that settles the answer false ends the run
            // at the step that settled it: `settled_false` states
            // are final under every continuation, the envelope
            // check that takes this run's length sets its own
            // dead flag from the same predicate when the run
            // returns (as its per-character feed does after each
            // character), and the scanner's bulk path then takes
            // the rest of the string — so feeding the rest of
            // this window's run through the quartet and character
            // steps below, emitting decoded bytes into a check
            // whose `finish` the dead flag already refuses, was
            // work no reader could observe. The state the steps
            // leave past the settling one is read by no one: the
            // check's `finish` is dead-gated, exactly as for the
            // per-character path's settling feed.
            if self.settled_false() {
                break;
            }
            if self.grouped == 0 && self.padding == 0 && index + 4 <= bytes.len() {
                let quartet = &bytes[index..index + 4];
                let value = |byte: u8| -> Option<u32> {
                    match byte {
                        b'A'..=b'Z' => Some((byte - b'A') as u32),
                        b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
                        b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
                        b'+' => Some(62),
                        b'/' => Some(63),
                        _ => None,
                    }
                };
                if let (Some(a), Some(b), Some(c), Some(d)) =
                    (value(quartet[0]), value(quartet[1]), value(quartet[2]), value(quartet[3]))
                {
                    self.total += 4;
                    self.body += 4;
                    let group = (a << 18) | (b << 12) | (c << 6) | d;
                    self.emit((group >> 16) as u8);
                    self.emit((group >> 8) as u8);
                    self.emit(group as u8);
                    index += 4;
                    continue;
                }
            }
            let c = text[index..].chars().next().unwrap_or('\0');
            if c.is_whitespace() {
                break;
            }
            let len = c.len_utf8();
            self.push_char(c);
            index += len;
        }
        index
    }
    fn emit(&mut self, byte: u8) {
        self.decoded = true;
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
    fn finish(&mut self) -> bool {
        if self.bad || self.body == 0 || !self.total.is_multiple_of(4) || self.padding > 2 {
            return false;
        }
        match (self.padding, self.grouped) {
            (0, 0) => {}
            (1, 3) => {
                let group = self.group;
                self.emit((group >> 10) as u8);
                self.emit((group >> 2) as u8);
            }
            (2, 2) => {
                let group = self.group;
                self.emit((group >> 4) as u8);
            }
            _ => return false,
        }
        !self.utf8_bad && self.utf8.complete() && !self.nul && self.decoded && self.solid
    }
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum EnvelopeStage {
    Lead,
    Amp,
    Path,
    TimeoutPrefix,
    Timeout,
    ShellPrefix,
    ShellName,
    PayloadPrefix,
    Payload,
}

/// ASCII case-insensitive character equality, the comparison the
/// path rules use: non-ASCII characters match only themselves.
pub(crate) fn chars_eq_ignore_ascii_case(a: char, b: char) -> bool {
    a.eq_ignore_ascii_case(&b)
}

/// One envelope path as successive segments — a borrowed directory
/// slice, the separator, a static program name and the extension —
/// so the verdict path never builds the path as a whole string.
#[derive(Clone, Copy)]
pub(crate) struct PathSegments<'a> {
    segs: [&'a str; 5],
    count: usize,
}

impl<'a> PathSegments<'a> {
    pub(crate) fn empty() -> Self {
        PathSegments { segs: [""; 5], count: 0 }
    }
    /// Whether the path these segments spell equals `text` under
    /// ASCII case-insensitive comparison, compared segment by
    /// segment in place.
    pub(crate) fn eq_ignore_ascii_case(&self, text: &str) -> bool {
        // Segment by segment in slices: each segment's slice of
        // `text` is compared with the same ASCII case-insensitive
        // equality the per-character walk applied. A slice whose
        // length would split a character of `text` cannot match,
        // exactly as the walk could not: matching characters have
        // the segment's byte length, so a matched segment ends on
        // a character boundary, and a non-ASCII character's bytes
        // (all at least 0x80) fold to themselves under the ASCII
        // comparison, as the character itself did.
        let mut rest = text;
        for seg in &self.segs[..self.count] {
            let Some(head) = rest.get(..seg.len()) else { return false };
            if !head.eq_ignore_ascii_case(seg) {
                return false;
            }
            rest = &rest[seg.len()..];
        }
        rest.is_empty()
    }
}

/// The envelope path set of one verdict, as segment lists borrowed
/// from the executable spelling and the static program names.
pub(crate) struct EnvelopePaths<'a> {
    pub(crate) list: [PathSegments<'a>; 8],
    pub(crate) count: usize,
}

impl EnvelopePaths<'_> {
    /// The empty set: no envelope path at all.
    pub(crate) fn empty() -> Self {
        EnvelopePaths { list: [PathSegments::empty(); 8], count: 0 }
    }
}

/// The same paths `envelope_paths` builds as strings, spelled as
/// components: the directory is a slice of `exe`, the program names
/// are the static ones, and the gate's sibling `bin` folder is the
/// parent slice plus one more segment. No path string is built.
pub(crate) fn envelope_path_specs<'a>(exe: &'a str, role: Role) -> EnvelopePaths<'a> {
    let mut specs = EnvelopePaths { list: [PathSegments::empty(); 8], count: 0 };
    fn add<'a>(specs: &mut EnvelopePaths<'a>, segs: [&'a str; 5], count: usize) {
        if specs.count < specs.list.len() {
            specs.list[specs.count] = PathSegments { segs, count };
            specs.count += 1;
        }
    }
    if role == Role::Gate {
        let (dir, _) = match exe.rsplit_once('/') {
            Some(pair) => pair,
            None => return specs,
        };
        let mut folders: [[&'a str; 2]; 2] = [[dir, ""], ["", ""]];
        let mut folder_counts = [1usize, 0usize];
        let mut folder_total = 1usize;
        if let Some((root, last)) = dir.rsplit_once('/') {
            if last.eq_ignore_ascii_case("libexec") {
                folders[1] = [root, "/bin"];
                folder_counts[1] = 2;
                folder_total = 2;
            }
        }
        for folder_index in 0..folder_total {
            let folder = &folders[folder_index];
            for name in [NAMES[0], NAMES[1], DEADLINE] {
                let mut segs = [""; 5];
                let mut n = 0usize;
                // The folder's own segments come first.
                for seg in folder.iter().take(folder_counts[folder_index]) {
                    segs[n] = seg;
                    n += 1;
                }
                segs[n] = "/";
                segs[n + 1] = name;
                segs[n + 2] = ".exe";
                let spec = PathSegments { segs, count: n + 3 };
                if !spec.eq_ignore_ascii_case(exe) {
                    add(&mut specs, segs, n + 3);
                }
            }
        }
        return specs;
    }
    add(&mut specs, [exe, "", "", "", ""], 1);
    if let Some((dir, own)) = family(exe) {
        add(&mut specs, [dir, "/", NAMES[1 - own], ".exe", ""], 4);
    }
    specs
}

/// The executable spelling behind one verdict scan, resolved on
/// first demand. The envelope paths are read only at the moment the
/// first watched string begins, so a payload with no watched string
/// (not an object, no `tool_input`, no string `command`/`cmd`,
/// malformed) is decided without the module-name query and its
/// per-character decode walk ever being paid. The spelling is held
/// in cells shared by the whole scan, so the first demand fills
/// them through interior mutability and every later demand — a
/// second watched string, or the deny reason `main` builds after
/// the scan — reads the one spelling already resolved: exactly one
/// query on every route that reaches a watched string, and on a
/// route that reaches none the single query is the deny's own. A
/// spelling supplied whole (the tests, which scan under spellings
/// no running executable carries) is held as `fixed` and read the
/// same way, with no query at all.
pub(crate) struct LazyPaths<'a> {
    role: Role,
    stack: std::cell::OnceCell<Option<StackSpelling>>,
    owned: std::cell::OnceCell<String>,
    fixed: Option<&'a str>,
}

impl<'a> LazyPaths<'a> {
    pub(crate) fn new(role: Role) -> Self {
        LazyPaths { role, stack: std::cell::OnceCell::new(), owned: std::cell::OnceCell::new(), fixed: None }
    }
    #[cfg(test)]
    pub(crate) fn fixed(exe: &'a str, role: Role) -> Self {
        LazyPaths { role, stack: std::cell::OnceCell::new(), owned: std::cell::OnceCell::new(), fixed: Some(exe) }
    }
    /// The spelling, resolved on this demand when it was not
    /// supplied whole: the stack form first, the standard call
    /// taken directly as its fallback, as in the other vendors.
    pub(crate) fn exe(&'a self) -> &'a str {
        if let Some(fixed) = self.fixed {
            return fixed;
        }
        if let Some(owned) = self.owned.get() {
            return owned.as_str();
        }
        match self.stack.get_or_init(stack_self_path) {
            Some(spelling) => spelling.as_str(),
            None => self.owned.get_or_init(|| self_path().unwrap_or_default()).as_str(),
        }
    }
    /// The envelope paths of the spelling, built into `slot` on
    /// first demand and read back from it on every later one: the
    /// lazily-filled path slot of a verdict scan.
    pub(crate) fn paths<'s>(&'a self, slot: &'s mut Option<EnvelopePaths<'a>>) -> &'s EnvelopePaths<'a> {
        slot.get_or_insert_with(|| envelope_path_specs(self.exe(), self.role))
    }
}

/// One envelope path being matched segment by segment: the current
/// segment's character iterator plus the one character peeked from
/// it, advancing across segment boundaries in place.
pub(crate) struct PathCursor<'a> {
    spec: PathSegments<'a>,
    seg: usize,
    chars: std::str::Chars<'a>,
    next: Option<char>,
}

impl<'a> PathCursor<'a> {
    pub(crate) fn empty() -> Self {
        PathCursor { spec: PathSegments::empty(), seg: 0, chars: "".chars(), next: None }
    }
    pub(crate) fn new(spec: PathSegments<'a>) -> Self {
        let mut cursor = PathCursor { spec, seg: 0, chars: "".chars(), next: None };
        cursor.refill();
        cursor
    }
    pub(crate) fn refill(&mut self) {
        loop {
            if let Some(c) = self.chars.next() {
                self.next = Some(c);
                return;
            }
            if self.seg >= self.spec.count {
                self.next = None;
                return;
            }
            self.chars = self.spec.segs[self.seg].chars();
            self.seg += 1;
        }
    }
    pub(crate) fn advance(&mut self) {
        self.refill();
    }
}

/// The parked marker of the shared-first-segment grouping in
/// `EnvelopeCheck`: a parked cursor's segment slot holds this base
/// plus its group leader's index, a value no live cursor's segment
/// index (at most five segments) can reach, so the grouping rides
/// in the cursor storage the checker already holds and adds no
/// state of its own.
pub(crate) const GROUP_PARKED: usize = usize::MAX - 8;

/// The envelope checker: one watched string in, one verdict kept.
/// Each envelope path is held as a segment cursor plus the one
/// character peeked from it, so feeding a character compares against
/// the peeked character and advances in place: no full path string
/// and no per-path character vector is built for a verdict, and
/// stepping stays O(1) amortised per character.
pub(crate) struct EnvelopeCheck<'a> {
    path_cursors: [PathCursor<'a>; 8],
    path_count: usize,
    stage: EnvelopeStage,
    dead: bool,
    path_alive: [bool; 8],
    lit: usize,
    timeout: TimeoutCheck,
    shell_seen: [usize; 3],
    shell_alive: [bool; 3],
    payload: PayloadCheck,
    payload_ws: bool,
}

impl<'a> EnvelopeCheck<'a> {
    pub(crate) fn new(paths: &EnvelopePaths<'a>) -> Self {
        let mut check = EnvelopeCheck {
            path_cursors: std::array::from_fn(|i| {
                if i < paths.count { PathCursor::new(paths.list[i]) } else { PathCursor::empty() }
            }),
            path_count: paths.count.min(8),
            stage: EnvelopeStage::Lead,
            dead: paths.count > 8,
            path_alive: [false; 8],
            lit: 0,
            timeout: TimeoutCheck::new(),
            shell_seen: [0; 3],
            shell_alive: [true; 3],
            payload: PayloadCheck::new(),
            payload_ws: false,
        };
        for alive in check.path_alive.iter_mut().take(paths.count) {
            *alive = true;
        }
        // Group the specs by their identical first segment: every
        // spec after the first that shares a first segment (compared
        // as the exact segment text) is parked behind that first
        // spec, its group leader, by writing the leader's index into
        // the parked cursor's segment slot, so `feed_path` steps the
        // shared segment once, through the leader alone. A spec
        // whose first segment is empty shares no character worth
        // stepping — its cursor already stands past that segment —
        // and is left to step on its own, as every leader is.
        for i in 0..check.path_count {
            let first = check.path_cursors[i].spec.segs[0];
            if first.is_empty() || check.path_cursors[i].spec.count == 0 {
                continue;
            }
            for leader in 0..i {
                if check.path_cursors[leader].spec.segs[0] == first {
                    check.path_cursors[i].seg = GROUP_PARKED + leader;
                    check.path_cursors[i].chars = "".chars();
                    check.path_cursors[i].next = None;
                    break;
                }
            }
        }
        check
    }
    fn step_literal(&mut self, c: char, text: &str, next: EnvelopeStage) {
        if c != text.as_bytes()[self.lit] as char {
            self.dead = true;
            return;
        }
        self.lit += 1;
        if self.lit == text.len() {
            self.lit = 0;
            self.stage = next;
        }
    }
    fn feed_path(&mut self, c: char) {
        let mut completed = false;
        // The group leaders whose shared first segment this character
        // completes: their fan-out runs after the stepping loop, so
        // a member fanned out at this character is not stepped on it
        // a second time — the character is the shared segment's
        // last, and the member stands at its second segment for the
        // next character, exactly where the per-spec form stands
        // after stepping the same character.
        let mut fan_mask: u8 = 0;
        for i in 0..self.path_count {
            if !self.path_alive[i] {
                continue;
            }
            if self.path_cursors[i].seg >= GROUP_PARKED {
                // Parked behind its group leader: the leader's
                // stepping of the shared first segment is this
                // spec's stepping too, character for character.
                continue;
            }
            if self.path_cursors[i].next == Some(c) {
                // A cursor consuming its first segment holds segment
                // index 1 (the refill that loaded that segment has
                // not loaded another); crossing to a higher index on
                // this character is that segment's completion.
                let in_first = self.path_cursors[i].seg == 1;
                self.path_cursors[i].advance();
                if self.path_cursors[i].next.is_none() {
                    completed = true;
                } else if in_first && self.path_cursors[i].seg > 1 {
                    fan_mask |= 1 << i;
                }
            } else {
                self.path_alive[i] = false;
                // A leader that dies on this character takes its
                // parked members with it: their shared segment holds
                // the same next character, so this is the character
                // the per-spec form kills each of them on.
                for j in 0..self.path_count {
                    if self.path_cursors[j].seg == GROUP_PARKED + i {
                        self.path_alive[j] = false;
                    }
                }
            }
        }
        while fan_mask != 0 {
            let leader = fan_mask.trailing_zeros() as usize;
            fan_mask &= fan_mask - 1;
            if self.fan_out(leader) {
                completed = true;
            }
        }
        if completed {
            // No envelope path is a strict prefix of another: the
            // folder is shared, and the program names in it diverge
            // before any of them ends. The first path completed is
            // therefore the only one that can govern the rest.
            self.stage = EnvelopeStage::TimeoutPrefix;
            self.lit = 0;
        } else if !self.path_alive.iter().take(self.path_count).any(|alive| *alive) {
            self.dead = true;
        }
    }
    /// Fan one group out at its shared first segment's completion:
    /// each parked member is placed directly past that segment —
    /// its segment slot reset to the second segment and refilled in
    /// place, without stepping the shared segment's characters
    /// again — so it stands exactly where the per-spec form's
    /// stepping of the same characters left it: peeked on its second
    /// segment's first character, or complete when nothing but empty
    /// segments follows it, which is reported so the caller settles
    /// the completion on this same character.
    fn fan_out(&mut self, leader: usize) -> bool {
        let mut completed = false;
        for j in 0..self.path_count {
            let member = &mut self.path_cursors[j];
            if member.seg != GROUP_PARKED + leader {
                continue;
            }
            member.seg = 1;
            member.chars = "".chars();
            member.next = None;
            member.refill();
            if member.next.is_none() {
                completed = true;
            }
        }
        completed
    }
    fn feed_shell(&mut self, c: char) {
        let mut completed = false;
        for (i, name) in SHELL_NAMES.iter().enumerate() {
            if !self.shell_alive[i] {
                continue;
            }
            if self.shell_seen[i] < name.len() && name.as_bytes()[self.shell_seen[i]] as char == c {
                self.shell_seen[i] += 1;
                if self.shell_seen[i] == name.len() {
                    completed = true;
                }
            } else {
                self.shell_alive[i] = false;
            }
        }
        if completed {
            self.stage = EnvelopeStage::PayloadPrefix;
            self.lit = 0;
        } else if !self.shell_alive.iter().any(|alive| *alive) {
            self.dead = true;
        }
    }
    fn finish(&mut self) -> bool {
        !self.dead && self.stage == EnvelopeStage::Payload && self.payload.finish()
    }
}

impl CharSink for EnvelopeCheck<'_> {
    fn takes_payload_run(&self) -> bool {
        // The run capability is confined to the payload stage of
        // a live check that has seen no payload whitespace: at
        // every other stage, after whitespace (whose tail the
        // per-character form settles through `payload_ws` and
        // `dead`), and once dead (where the bulk path above takes
        // the rest), delivery is the existing path unchanged.
        self.stage == EnvelopeStage::Payload && !self.payload_ws && !self.dead
    }
    fn push_payload_run(&mut self, text: &str) -> usize {
        let consumed = self.payload.push_payload_run(text);
        if self.payload.settled_false() {
            self.dead = true;
        }
        consumed
    }
    fn takes_timeout_run(&self) -> bool {
        // The run capability is confined to the timeout stage
        // of a live check: at every other stage, and once dead
        // (where the bulk path above takes the rest), delivery
        // is the existing path unchanged.
        self.stage == EnvelopeStage::Timeout && !self.dead
    }
    fn push_timeout_run(&mut self, text: &str) -> usize {
        // Feed the run's digit-and-point prefix through the
        // same `TimeoutCheck::push_char` the per-character path
        // feeds: digits and the point are ASCII, so each byte
        // of the prefix is the character it names, delivered in
        // order, and the check's counters take exactly the steps
        // they would have taken one by one. The prefix ends at
        // the first byte that is neither a digit nor the point
        // — a multi-byte character's lead byte is at or past
        // 0x80, so the prefix ends on that character's boundary
        // and the character itself is left to the per-character
        // path — or just after the feed on which the check
        // settles false: those are states `finish` refuses on
        // under every continuation, the per-character feed sets
        // the check's dead flag on the same feed, and the bulk
        // path takes the rest of the string from there. At the
        // head of each turn the rest of the run is first offered
        // to the check's own bulk step (`TimeoutCheck::push_run`),
        // which consumes a run of digits (or of zeros, in the
        // states it names) that the per-character feed would step
        // through changing nothing but two counters; a step
        // settles nothing, so the settling test below stands
        // exactly where it stood, on the per-character feeds.
        let bytes = text.as_bytes();
        let mut index = 0usize;
        while index < bytes.len() {
            let consumed = self.timeout.push_run(&text[index..]);
            if consumed > 0 {
                index += consumed;
                continue;
            }
            let byte = bytes[index];
            if !matches!(byte, b'0'..=b'9' | b'.') {
                break;
            }
            self.timeout.push_char(byte as char);
            index += 1;
            if self.timeout.settled_false() {
                self.dead = true;
                break;
            }
        }
        index
    }
    fn takes_whitespace_run(&self) -> bool {
        // The run capability is confined to the two stages of a
        // live check whose only act on a whitespace character is
        // to stay in place: the leading stage, where a whitespace
        // character returns from `push_char` at once with nothing
        // moved, and the payload stage once its trailing
        // whitespace has begun, where a whitespace character
        // leaves `payload_ws` standing (it already stands) and
        // moves nothing else. At every other stage, at the
        // payload stage before its whitespace, and once dead
        // (where the bulk path above takes the rest), delivery
        // is the existing path unchanged.
        (self.stage == EnvelopeStage::Lead && !self.dead)
            || (self.stage == EnvelopeStage::Payload && self.payload_ws && !self.dead)
    }
    fn push_whitespace_run(&mut self, text: &str) -> usize {
        // Report the maximal decoded-whitespace prefix of the
        // run and consume nothing else: each character of the
        // prefix is decoded at its own position and tested with
        // `char::is_whitespace` there (the full Unicode set,
        // never a space-byte test), and the loop stops before
        // the first non-whitespace character, leaving that
        // character and every byte after it to the per-character
        // path, which settles them exactly as before (the
        // leading stage dispatches on it; the payload stage
        // kills the check on it). No state is recorded per
        // consumed character: in the leading stage a whitespace
        // character changes nothing, and at the payload stage
        // with `payload_ws` standing a whitespace character
        // changes nothing either, so the prefix's length is the
        // whole effect of the run.
        let mut index = 0usize;
        while index < text.len() {
            let c = text[index..].chars().next().unwrap_or('\0');
            if !c.is_whitespace() {
                break;
            }
            index += c.len_utf8();
        }
        index
    }
    fn takes_bulk(&self) -> bool {
        // A dead check discards every later character: `push_char`
        // returns at once and `finish` is already settled false, so
        // the rest of the string may be validated whole like a
        // discarding sink's, instead of being decoded character by
        // character into a verdict that cannot change. A live check
        // watches its characters and keeps per-character delivery.
        self.dead
    }
    fn push_char(&mut self, c: char) {
        if self.dead {
            return;
        }
        match self.stage {
            EnvelopeStage::Lead => {
                if c.is_whitespace() {
                    return;
                }
                if c == '&' {
                    self.stage = EnvelopeStage::Amp;
                    return;
                }
                self.stage = EnvelopeStage::Path;
                self.feed_path(c);
            }
            EnvelopeStage::Amp => {
                self.stage = EnvelopeStage::Path;
                if c != ' ' {
                    // Not the `& ` marker: the `&` is the first
                    // character of the path itself.
                    self.push_char('&');
                    self.push_char(c);
                }
            }
            EnvelopeStage::Path => self.feed_path(c),
            EnvelopeStage::TimeoutPrefix => self.step_literal(c, TIMEOUT_PREFIX, EnvelopeStage::Timeout),
            EnvelopeStage::Timeout => {
                if c == ' ' {
                    if self.timeout.finish() {
                        self.stage = EnvelopeStage::ShellPrefix;
                    } else {
                        self.dead = true;
                    }
                } else {
                    self.timeout.push_char(c);
                    if self.timeout.settled_false() {
                        self.dead = true;
                    }
                }
            }
            EnvelopeStage::ShellPrefix => self.step_literal(c, SHELL_PREFIX, EnvelopeStage::ShellName),
            EnvelopeStage::ShellName => self.feed_shell(c),
            EnvelopeStage::PayloadPrefix => self.step_literal(c, PAYLOAD_PREFIX, EnvelopeStage::Payload),
            EnvelopeStage::Payload => {
                if c.is_whitespace() {
                    // Whitespace here is the string's trailing
                    // whitespace, which the whole-string form trims;
                    // anything but whitespace after it is payload
                    // content the form would reject.
                    self.payload_ws = true;
                    return;
                }
                if self.payload_ws {
                    self.dead = true;
                    return;
                }
                self.payload.push_char(c);
                if self.payload.settled_false() {
                    self.dead = true;
                }
            }
        }
    }
}

/// What the scan has of one watched field: nothing seen, a value that
/// was not a string, or the envelope checker of its string.
pub(crate) enum Watched<'a> {
    Absent,
    NotString,
    Text(EnvelopeCheck<'a>),
}

impl Watched<'_> {
    pub(crate) fn verdict(&mut self) -> bool {
        match self {
            Watched::Text(check) => check.finish(),
            _ => false,
        }
    }
}

/// The watched fields of the last `tool_input` object: `command`, and
/// `cmd` for when no `command` key is present at all.
pub(crate) struct ToolInput<'a> {
    command: Watched<'a>,
    cmd: Watched<'a>,
    has_command: bool,
}

impl<'a> ToolInput<'a> {
    pub(crate) fn new() -> Self {
        ToolInput { command: Watched::Absent, cmd: Watched::Absent, has_command: false }
    }
}

/// Scan the members of a `tool_input` object, whose opening brace is
/// the next byte, into `tool`.
pub(crate) fn scan_tool_object<'a, R: Read>(scan: &mut Scanner<R>, tool: &mut ToolInput<'a>, paths: &mut Option<EnvelopePaths<'a>>, lazy: &'a LazyPaths<'a>) -> Option<()> {
    scan.bump();
    if scan.eat(b'}').is_some() {
        return Some(());
    }
    loop {
        scan.ws();
        let matched = scan.key(&["command", "cmd"])?;
        scan.eat(b':')?;
        match matched {
            Some(0) => {
                tool.has_command = true;
                scan.watched(&mut tool.command, 3, paths, lazy)?;
            }
            Some(1) => scan.watched(&mut tool.cmd, 3, paths, lazy)?,
            _ => scan.value(3)?,
        }
        if scan.eat(b',').is_some() {
            continue;
        }
        scan.eat(b'}')?;
        return Some(());
    }
}

/// Scan the members of the top object, whose opening brace is the next
/// byte. For cursor the watched field is the top `command`; otherwise
/// it is inside the last `tool_input` value.
pub(crate) fn scan_top_object<'a, R: Read>(
    scan: &mut Scanner<R>,
    cursor: bool,
    paths: &mut Option<EnvelopePaths<'a>>,
    lazy: &'a LazyPaths<'a>,
    top: &mut Watched<'a>,
    tool: &mut ToolInput<'a>,
) -> Option<()> {
    scan.bump();
    if scan.eat(b'}').is_some() {
        return Some(());
    }
    loop {
        scan.ws();
        let matched = if cursor { scan.key(&["command"])? } else { scan.key(&["tool_input"])? };
        scan.eat(b':')?;
        if cursor {
            if matched == Some(0) {
                scan.watched(top, 2, paths, lazy)?;
            } else {
                scan.value(2)?;
            }
        } else if matched == Some(0) {
            *tool = ToolInput::new();
            scan.ws();
            if scan.peek() == Some(b'{') {
                scan_tool_object(scan, tool, paths, lazy)?;
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
        return Some(());
    }
}

/// Decide the verdict vendors from the payload read from `source`:
/// true only when the payload is one JSON value, nothing follows it
/// but whitespace, and its watched command string is exactly the
/// guard envelope for the spelling `lazy` resolves on first demand.
pub(crate) fn scan_verdict<'a, R: Read>(source: R, cursor: bool, lazy: &'a LazyPaths<'a>) -> bool {
    // The envelope path set is computed once for the whole verdict
    // and threaded through the scan as borrowed path components —
    // the executable's directory slice plus the static program
    // names — so no full path string is built on the verdict path.
    // It sits in a lazily-filled slot: the spelling behind it is
    // resolved only when the first watched string is scanned, and
    // a payload with no watched string never resolves it at all.
    let mut paths: Option<EnvelopePaths<'a>> = None;
    let mut scan = Scanner::new(source);
    let mut top = Watched::Absent;
    let mut tool = ToolInput::new();
    let walked = (|| -> Option<()> {
        scan.ws();
        if scan.peek() == Some(b'{') {
            scan_top_object(&mut scan, cursor, &mut paths, lazy, &mut top, &mut tool)?;
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
        return false;
    }
    if cursor {
        top.verdict()
    } else if tool.has_command {
        tool.command.verdict()
    } else {
        tool.cmd.verdict()
    }
}

impl<R: Read> Scanner<R> {
    /// Scan the value of a watched key into `slot`: a string feeds a
    /// fresh envelope checker; any other value is skipped by structure
    /// and recorded as not a string. The envelope paths are demanded
    /// only here, at the moment a watched string begins: `paths` is
    /// the scan's lazily-filled slot and `lazy` the spelling behind
    /// it, so a value that is not a string — and a payload with no
    /// watched string at all — never resolves the spelling.
    fn watched<'a>(&mut self, slot: &mut Watched<'a>, depth: usize, paths: &mut Option<EnvelopePaths<'a>>, lazy: &'a LazyPaths<'a>) -> Option<()> {
        self.ws();
        if self.peek() == Some(b'"') {
            if depth > 256 {
                return None;
            }
            let mut check = EnvelopeCheck::new(lazy.paths(paths));
            self.string(&mut check)?;
            *slot = Watched::Text(check);
            Some(())
        } else {
            self.value(depth)?;
            *slot = Watched::NotString;
            Some(())
        }
    }
}

/// True when `command` is exactly the guard envelope for executable `exe`, or for
/// the same executable under its other name (see `envelope_paths`). The
/// whole-string reference form: the verdict path streams its scan instead,
/// and the tests hold the two to the same answers.
#[cfg(test)]
pub fn valid_envelope(command: &str, exe: &str, role: Role) -> bool {
    envelope_paths(exe, role).iter().any(|path| envelope_for(command, path))
}

/// True when `command` is exactly the guard envelope naming the executable `exe`.
#[cfg(test)]
pub(crate) fn envelope_for(command: &str, exe: &str) -> bool {
    let mut rest = command.trim();
    if let Some(r) = rest.strip_prefix("& ") {
        rest = r;
    }
    let rest = match rest.strip_prefix(exe).and_then(|r| r.strip_prefix(" --timeout ")) {
        Some(r) => r,
        None => return false,
    };
    envelope_remainder(rest)
}

/// True when the already-trimmed `rest` is exactly the guard
/// envelope naming the path `spec` spells: `envelope_for` with its
/// `strip_prefix(exe)` step replaced by a segment walk over the
/// same trimmed, optionally `& `-stripped command. The one caller
/// (`valid_envelope_paths_trimmed`) trims once for the whole spec
/// set and walks every spec over that one text, instead of paying
/// a trim per spec. The verdict is the string form's for every
/// input. A spec spells exactly the string `envelope_paths` builds
/// for the same executable and role — the executable itself, or its
/// folder, the separator, a static program name and the extension —
/// in the same folder-major, name order, with the gate's own path
/// left out by the same check that leaves it out of the string set
/// (the spec's own `eq_ignore_ascii_case(exe)` test in
/// `envelope_path_specs`, as `gate_envelope_paths` retains against
/// it) — so the two sets hold the same paths in the same order.
/// The walk matches character for character exactly, as
/// `strip_prefix` does: a path spelled in another letter case is
/// not the envelope path, as the string form's tests pin. And a
/// spec character can only match a whole command character — the
/// walk advances past a command character only when it equals the
/// spec character whole, so the byte index it leaves stands on a
/// character boundary and cannot split one, as in
/// `PathSegments::eq_ignore_ascii_case`. On spec exhaustion the
/// remainder is `rest[index..]`, which must take the same
/// ` --timeout ` strip, and every later step is the shared
/// `envelope_remainder`.
pub(crate) fn envelope_for_segments_trimmed(rest: &str, spec: PathSegments) -> bool {
    let mut rest = rest;
    if let Some(r) = rest.strip_prefix("& ") {
        rest = r;
    }
    let mut index = 0usize;
    for seg in &spec.segs[..spec.count] {
        // The segment is matched as one slice, as in
        // `strip_wrapper_prefix`: byte-prefix equality with a
        // valid UTF-8 segment is character-for-character equality
        // with it, so the walk still matches exactly, and the
        // index a matched segment leaves is a character boundary.
        if !rest[index..].starts_with(seg) {
            return false;
        }
        index += seg.len();
    }
    let rest = match rest[index..].strip_prefix(" --timeout ") {
        Some(r) => r,
        None => return false,
    };
    envelope_remainder(rest)
}

/// The steps of an envelope check after the path and its
/// ` --timeout ` prefix: the timeout token, the shell name and the
/// base64 payload. Shared by the string form and the segment form,
/// so the two forms cannot drift in the tail.
pub(crate) fn envelope_remainder(rest: &str) -> bool {
    let (timeout, rest) = match rest.split_once(' ') {
        Some(pair) => pair,
        None => return false,
    };
    // The token is settled by the streaming checker's own counter
    // form, fed once: the digit walks over the whole and fraction
    // parts and the `f64` parse that followed them walked the token
    // again, and a token past the kept bytes — a long run of digits
    // a command may carry — was parsed in full for a verdict the
    // counters settle in the same pass that reads it. `finish`
    // states the whole-string rules themselves (a nonempty whole
    // part of digits, a fraction of digits when a point is present,
    // the parsed value in range), parsing only the kept prefix of
    // a short token, as the streaming form does.
    let mut timeout_check = TimeoutCheck::new();
    let mut index = 0usize;
    while index < timeout.len() {
        // A run the check's own bulk step names
        // (`TimeoutCheck::push_run`: digits with both fraction
        // comparisons decided, zeros before the first significant
        // whole digit, zeros past the fraction bounds' last place)
        // is consumed whole: the per-character feed would step
        // through it changing nothing but two counters, and no
        // character of it settles the check, so the settling test
        // below has nothing to test across it. The index advances
        // by the step's ASCII bytes or by one whole character, so
        // it always stands on a character boundary.
        let consumed = timeout_check.push_run(&timeout[index..]);
        if consumed > 0 {
            index += consumed;
            continue;
        }
        let c = timeout[index..].chars().next().unwrap();
        timeout_check.push_char(c);
        index += c.len_utf8();
        // A token whose answer is already settled false refuses
        // at the settling character: `settled_false` states are
        // final under every continuation (the streaming checker's
        // feeds stop on the same states, and its `finish` is
        // dead-gated there), so feeding the rest of the token —
        // a token may run to the input cap — could only feed a
        // checker whose `finish` refuses however it continues.
        if timeout_check.settled_false() {
            return false;
        }
    }
    if !timeout_check.finish() {
        return false;
    }
    let rest = match rest.strip_prefix("--shell ") {
        Some(r) => r,
        None => return false,
    };
    let rest = SHELL_NAMES.iter().find_map(|s| rest.strip_prefix(s)).and_then(|r| r.strip_prefix(" --command-b64 "));
    let payload = match rest {
        Some(p) => p,
        None => return false,
    };
    // The trailing `=` count is capped at 3, as in `base64_decode`
    // and `command_text_is_valid`: only 0, 1, 2 and more-than-2 are
    // read from it (the refusal here, and the empty-body test,
    // which a count past 2 has already refused unless the payload
    // is the run itself, whose length the capped count still
    // reaches when it is at most 2), so a trailing run of any
    // length is refused after three steps instead of a walk of
    // the whole run `trim_end_matches` made here.
    let padding = payload.as_bytes().iter().rev().take_while(|&&b| b == b'=').take(3).count();
    if padding > 2 || payload.len() == padding {
        return false;
    }
    // No alphabet walk stands here: `command_text_is_valid` makes
    // the same decision on every body byte itself — a byte outside
    // the Base64 alphabet, an interior `=` included, fails its
    // quartet walk — so walking the whole payload once here only
    // to walk it again there decided nothing the second walk does
    // not. The two checks above stay: they are the cheap end of
    // the payload (the trailing `=` count and the empty body),
    // which the validator also reaches, kept so a payload that
    // fails them is refused before the decode walk starts.
    crate::base64::command_text_is_valid(payload)
}
