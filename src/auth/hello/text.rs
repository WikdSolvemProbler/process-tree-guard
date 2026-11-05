// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The counted text of the request for Windows Hello: the fixed words and the fields, with its length in UTF-16 units counted as it is built.

use super::*;

/// The fixed words of the message for Windows Hello, in the order they stand in it, between the
/// caption, the four fields, the cap, the deadline and the note. They are plain ASCII, so the
/// byte length of each is also its length in UTF-16 units, which is how the length of the whole
/// message is counted before it is made (`hello_len`).
const HELLO_SEGMENTS: [&str; 8] = [
    " asks Windows Hello to allow this run; cancel to refuse it. Command line: [",
    "] Program: [",
    "] Account: [",
    "] Parent process, as reported: [",
    "] Allows a cap of ",
    ", renewable without a total limit; the run starts with a deadline of ",
    " seconds.",
    " Cancel to refuse it.",
];

/// Said in the message when a field has a character that is written out, so that the form is not
/// a puzzle. Plain ASCII, as the segments are.
const HELLO_NOTE: &str = " Note: \\u{..} is a character written out; check each field letter by letter.";

/// Whether a field of the message has a character that is written out.
fn hello_written_out(subject: &Subject, exception: &Exception) -> bool {
    [subject.account, subject.program, subject.parent, exception.command.as_str()].into_iter()
        .any(|text| !text.is_ascii() || text.contains("\\u{"))
}

/// A text assembled into one buffer, with the count of its UTF-16 units accumulated as each
/// piece is appended, so that the count of the finished text is known without a further walk
/// over it.
struct CountedText {
    text: String,
    units: usize,
}

impl CountedText {
    fn push(&mut self, piece: &str) {
        self.text.push_str(piece);
        self.units += piece.encode_utf16().count();
    }
}

/// The length, in UTF-16 units, of the message `hello_made_counted` would make at `percent`, counted
/// before anything is built: a length pass over each field in its shown form (`shown_len`, no
/// string built), the fixed words and the share-independent pieces (the cap, the deadline and
/// the note, each counted once by the caller) by their lengths. Every piece counted here is
/// plain ASCII — the shown forms by construction, the rest as noted at their definitions — so
/// each length counted is the piece's length in UTF-16 units.
fn hello_len(subject: &Subject, exception: &Exception, percent: usize, hours_units: usize, deadline_units: usize, note_units: usize) -> usize {
    let field_len = |text: &str, limit: usize| if text.is_empty() { "(unknown)".len() } else { shown_len(text, limit, Keep::End) };
    let at = |limit: usize| limit * percent / 100;
    CAPTION.len() + HELLO_SEGMENTS.iter().map(|segment| segment.len()).sum::<usize>()
        + shown_len(&exception.command, at(HELLO_COMMAND_LIMIT), Keep::Start)
        + shown_len(subject.program, at(HELLO_PROGRAM_LIMIT), Keep::End)
        + field_len(subject.account, at(HELLO_ACCOUNT_LIMIT))
        + field_len(subject.parent, at(HELLO_PARENT_LIMIT))
        + hours_units + deadline_units + note_units
}

/// What the request for Windows Hello says, with the same fields as the credential dialog, cut shorter: it starts with how to
/// refuse, so that a clipped end still shows it, and names the run (the command line first); then the cap and the deadline the run
/// starts with, and it ends with how to refuse again, as the credential dialog does. The limits of the fields
/// (`HELLO_COMMAND_LIMIT` and the others) are those of a text that is as long as the message may be; a text that is longer once
/// the characters are written out is measured with a smaller share of each, down to none, and the chosen share is the only one
/// built, so that it fits `HELLO_MESSAGE_LIMIT` whatever the fields hold (what is left out is counted, as in the credential dialog).
#[cfg(test)]
pub(in crate::auth) fn hello_text(subject: &Subject, exception: &Exception, seconds: f64) -> String {
    hello_text_counted(subject, exception, seconds).0
}

/// `hello_text` with the count of the built message's UTF-16 units, accumulated as the message
/// was appended (`hello_made_counted`), so that a caller that must check the count against a
/// limit does not walk the finished text again.
pub(in crate::auth) fn hello_text_counted(subject: &Subject, exception: &Exception, seconds: f64) -> (String, usize) {
    hello_made_counted(subject, exception, seconds, hello_share(subject, exception, seconds))
}

/// The share, in percent, of the limits of the fields that the message for Windows Hello is made with: the largest of 100, 75, 50
/// and 25 that keeps it within `HELLO_MESSAGE_LIMIT`, and 0 when none does. The search measures each share's message
/// (`hello_len`: length passes only, nothing built) and builds none of them, so a caller that then makes the text — and the
/// record of a run that Windows Hello decided, which cuts the command line and the starter by the same share, so that it never
/// shows more than the user was shown — pays for arithmetic, not for the builds of the shares it did not choose.
pub(in crate::auth) fn hello_share(subject: &Subject, exception: &Exception, seconds: f64) -> usize {
    // The share-independent pieces, counted once: the words of the cap and the deadline are
    // plain ASCII (figures, a point, spaces and the words for hours and seconds), as is the note.
    let hours_units = hours_phrase(seconds).len();
    let deadline_units = format_seconds(exception.timeout).len();
    let note_units = if hello_written_out(subject, exception) { HELLO_NOTE.len() } else { 0 };
    [100, 75, 50, 25].into_iter()
        .find(|percent| hello_len(subject, exception, *percent, hours_units, deadline_units, note_units) <= HELLO_MESSAGE_LIMIT)
        .unwrap_or(0)
}

/// The message for Windows Hello with the limits of its fields at `percent` of their defaults, and the count of its
/// UTF-16 units, accumulated piece by piece as the message is appended into its one buffer. The parent keeps the words
/// "as reported" of the credential dialog: the launcher chooses it, so it is information and not proof of who started the run
/// (Common Weakness Enumeration, CWE-451, user interface misrepresentation of critical information).
pub(in crate::auth) fn hello_made_counted(subject: &Subject, exception: &Exception, seconds: f64, percent: usize) -> (String, usize) {
    let field = |text: &str, limit: usize| if text.is_empty() { "(unknown)".to_string() } else { shown(text, limit, Keep::End) };
    let note = if hello_written_out(subject, exception) { HELLO_NOTE } else { "" };
    let at = |limit: usize| limit * percent / 100;
    let mut made = CountedText { text: String::new(), units: 0 };
    made.push(CAPTION);
    made.push(HELLO_SEGMENTS[0]);
    made.push(&shown(&exception.command, at(HELLO_COMMAND_LIMIT), Keep::Start));
    made.push(HELLO_SEGMENTS[1]);
    made.push(&shown(subject.program, at(HELLO_PROGRAM_LIMIT), Keep::End));
    made.push(HELLO_SEGMENTS[2]);
    made.push(&field(subject.account, at(HELLO_ACCOUNT_LIMIT)));
    made.push(HELLO_SEGMENTS[3]);
    made.push(&field(subject.parent, at(HELLO_PARENT_LIMIT)));
    made.push(HELLO_SEGMENTS[4]);
    made.push(&hours_phrase(seconds));
    made.push(HELLO_SEGMENTS[5]);
    made.push(&format_seconds(exception.timeout));
    made.push(HELLO_SEGMENTS[6]);
    made.push(note);
    made.push(HELLO_SEGMENTS[7]);
    (made.text, made.units)
}
