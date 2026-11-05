// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The approval prompt's text: how a command is shown without letting look-alike characters disguise it.

use super::*;

// ------------------------------------------------------------------ the message

/// True when `c` is shown as it is: printable basic Latin other than the square brackets that
/// frame a field. Everything else is written out, every character outside basic Latin included:
/// the letters of other scripts that look like Latin ones (a Cyrillic o or c for the Latin one),
/// the blanks and spaces of other scripts, the combining marks that stack over the text beside
/// them, and the format and control characters. A name that is written out cannot pass for another.
pub(super) fn shown_as_it_is(c: char) -> bool {
    c.is_ascii() && ((c.is_ascii_graphic() && c != '[' && c != ']') || c == ' ')
}

/// Which end of a field that is too long the prompt keeps: the start of a command line, which
/// names the program and the first arguments, and the end of a path or a name, which holds the
/// file name or the user.
#[derive(Clone, Copy)]
pub(super) enum Keep {
    Start,
    End,
}

/// The text as the prompt shows it: no more than `limit` characters, with every character that is
/// not shown as it is written as `\u{..}` so that nothing in it can start a line of its own, pass
/// for a space or for another letter, or change how the rest is shown, the square brackets that
/// frame it written the same way so that it cannot close its own frame and imitate a label, and,
/// when it was cut, the number of characters left out: `... (and N more characters)` after a kept
/// start, `(N characters before) ...` before a kept end. Not `extend::escape`: that also escapes
/// quotes and backslashes, which would double every path.
/// The length of the form `c` is shown in: itself when it is shown
/// as it is, else the written-out form `\u{..}`, whose length is
/// its fixed frame plus the digits of the value in base 16. Every
/// shown form is plain ASCII, so this length is also the form's
/// length in UTF-16 units.
fn shown_form_len(c: char, imitates: bool) -> usize {
    if shown_as_it_is(c) && !imitates {
        c.len_utf8()
    } else {
        let value = c as u32;
        let digits = match value {
            0x0..=0xF => 1,
            0x10..=0xFF => 2,
            0x100..=0xFFF => 3,
            0x1000..=0xFFFF => 4,
            0x10000..=0xFFFFF => 5,
            _ => 6,
        };
        4 + digits
    }
}

/// How many digits `value` takes in base 10.
fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

/// The length of the text `shown` would make of the same field, with
/// no string built: the same per-character length pass `shown` makes,
/// then the same choice of how much is kept under `limit`, and the
/// words that say how much was cut counted from their fixed frames
/// and the digits of the count left out. Every shown form is plain
/// ASCII, so the length counted here is the built text's length in
/// characters, in bytes and in UTF-16 units alike.
pub(super) fn shown_len(text: &str, limit: usize, keep: Keep) -> usize {
    let mut lengths = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        let imitates = c == '\\' && {
            let mut ahead = chars.clone();
            ahead.next() == Some('u') && ahead.next() == Some('{')
        };
        lengths.push(shown_form_len(c, imitates));
    }
    if lengths.is_empty() {
        return "(none)".len();
    }
    let fits = |lengths: &mut dyn Iterator<Item = &usize>| {
        let mut used = 0;
        lengths.take_while(|length| {
            used += *length;
            used <= limit
        }).count()
    };
    let kept = match keep {
        Keep::Start => fits(&mut lengths.iter()),
        Keep::End => fits(&mut lengths.iter().rev()),
    };
    let more = lengths.len() - kept;
    match keep {
        _ if more == 0 => lengths.iter().sum(),
        Keep::Start => lengths[..kept].iter().sum::<usize>() + "... (and ".len() + decimal_digits(more) + " more characters)".len(),
        Keep::End => "(".len() + decimal_digits(more) + " characters before) ...".len() + lengths[lengths.len() - kept..].iter().sum::<usize>(),
    }
}

pub(super) fn shown(text: &str, limit: usize, keep: Keep) -> String {
    fn push_form(out: &mut String, c: char, imitates: bool) {
        use std::fmt::Write as _;
        if shown_as_it_is(c) && !imitates {
            out.push(c);
        } else {
            let _ = write!(out, "\\u{{{:x}}}", c as u32);
        }
    }
    // One pass collects only the length of each character's shown
    // form — a vector of lengths, where the form this replaces built
    // one owned string per character of the field (an allocation and
    // a copy for every character, graphic ones included), held them
    // all, and then copied the kept ones again into the result with
    // `concat`. The characters themselves are walked again, from the
    // text, when the kept range is written into the result.
    let mut lengths = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        // A backslash that begins what looks like a written-out character is itself written out, so that an
        // argument cannot imitate the form; any other backslash, as in a path, is shown as it is.
        let imitates = c == '\\' && {
            let mut ahead = chars.clone();
            ahead.next() == Some('u') && ahead.next() == Some('{')
        };
        lengths.push(shown_form_len(c, imitates));
    }
    if lengths.is_empty() {
        return "(none)".to_string();
    }
    let fits = |lengths: &mut dyn Iterator<Item = &usize>| {
        let mut used = 0;
        lengths.take_while(|length| {
            used += *length;
            used <= limit
        }).count()
    };
    let kept = match keep {
        Keep::Start => fits(&mut lengths.iter()),
        Keep::End => fits(&mut lengths.iter().rev()),
    };
    let more = lengths.len() - kept;
    // Write the kept characters' forms into `out`, in their order:
    // the first `skip` characters are passed over, then `take` are
    // written. Each character's imitation test is recomputed from
    // the same lookahead the length pass used, so the forms written
    // are the forms whose lengths were counted.
    fn write_kept(out: &mut String, text: &str, skip: usize, take: usize) {
        let mut chars = text.chars();
        let mut index = 0;
        while let Some(c) = chars.next() {
            let imitates = c == '\\' && {
                let mut ahead = chars.clone();
                ahead.next() == Some('u') && ahead.next() == Some('{')
            };
            if index >= skip && index - skip < take {
                push_form(out, c, imitates);
            }
            index += 1;
        }
    }
    match keep {
        _ if more == 0 => {
            let mut out = String::with_capacity(lengths.iter().sum());
            write_kept(&mut out, text, 0, kept);
            out
        }
        Keep::Start => {
            let mut out = String::with_capacity(lengths[..kept].iter().sum::<usize>() + 32);
            write_kept(&mut out, text, 0, kept);
            out.push_str(&format!("... (and {} more characters)", more));
            out
        }
        Keep::End => {
            let mut out = format!("({} characters before) ...", more);
            write_kept(&mut out, text, more, kept);
            out
        }
    }
}

/// The command line as the prompt and the log show it: its start, cut at `COMMAND_LIMIT`.
pub(super) fn shown_command(command: &str) -> String {
    shown(command, COMMAND_LIMIT, Keep::Start)
}

/// What the prompt says, with what the person must check first: the command line as it was
/// given, the program that will run as the search found it, the account whose password it asks
/// for, and the parent process as the system reports it; then what the cap means, the deadline the run starts with and how to refuse. A dialog that
/// clips or scrolls long text loses the end first, so the end is the explanation and not a field.
/// The cap is the size of each request for time, not a bound on the run: the processes of the run
/// can renew without a total limit. The program is shown because the name in the command line is
/// looked for in the current folder first, so it can be another file than the one the user expects.
/// The parent is the one the launcher chose, so it is information, not proof of who started the
/// run. The command line keeps its start when it is cut, and the program, the account and the
/// parent keep their end, which holds the file name or the user. Each field is in square brackets, in which no bracket can appear, so that none can
/// imitate the others. How to refuse depends on the method: the desktop prompt has a cancel; the
/// console prompt is not known to have a cancel key, and is refused by not answering or by closing
/// the console.
pub(crate) fn prompt_text(subject: &Subject, exception: &Exception, seconds: f64) -> String {
    let field = |text: &str, limit| if text.is_empty() { "(unknown)".to_string() } else { shown(text, limit, Keep::End) };
    let refusal = match exception.method {
        Method::Desktop => "cancel to refuse the run".to_string(),
        Method::Console => format!("leave the prompt unanswered for {} seconds, or close this console, to refuse the run", PROMPT_TIMEOUT_MS / 1000),
    };
    // Said when a field above has a character that is written out, so that the form is not a puzzle.
    let written_out = [subject.account, subject.program, subject.parent, exception.command.as_str()].into_iter()
        .any(|text| !text.is_ascii() || text.contains("\\u{"));
    let note = if written_out { " Note: \\u{..} is a character written out; check each field letter by letter." } else { "" };
    format!("{} is asked to run: Command line: [{}] Program: [{}] Account: [{}] Parent process, as the system reports it: [{}] \
        The processes of the run may each ask to keep it alive for up to {} at a time, and may renew without a total limit. \
        The run starts with a deadline of {} seconds. Enter the password of the account above to allow it, or {}.{}",
        CAPTION, shown_command(&exception.command), shown(subject.program, PROGRAM_LIMIT, Keep::End), field(subject.account, ACCOUNT_LIMIT),
        field(subject.parent, PARENT_LIMIT), hours_phrase(seconds), format_seconds(exception.timeout), refusal, note)
}
