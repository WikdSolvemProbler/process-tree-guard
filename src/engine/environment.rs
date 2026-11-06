// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The environment block a run's child is created with: this process's environment with one entry set, sorted as `CreateProcessW` takes it.

// The sort key of one entry of an environment block: the entry's name in
// upper case, ordered as `String` orders that same text. A name that a
// byte-wise ASCII test shows is already its own upper case — pure ASCII
// with no lower-case byte — is unchanged by upper-casing (ASCII upper case
// maps only `a`..=`z`), so its key is the name itself and no upper-case
// `String` is built for it: an environment entry in that case is valid
// text, so its key is read from the entry's own held `OsString`, and the
// entry being set reads from the `OsString` built from its name. Every
// other name, non-ASCII ones included, takes the owned fallback: the
// upper case of its lossy text, exactly as before (a name that is not
// valid text at all takes an owned copy of that lossy text, whose
// ordering is the one the lossy form decided before).
// Either way the key's text is held once, formed before the sort,
// and the comparison reads it in place: forming the lossy text at
// every comparison re-scanned the whole name for validity once per
// comparison the sort made, on a sort that makes a comparison count
// of the order of the entry count times its logarithm. No second
// key row is formed for the sort either: `str` ordering is the
// ordering of its bytes, so the comparison reads the owned key's
// bytes, or — for a name with no owned key, which the pass that
// built the rows showed to be valid text — the entry name's encoded
// bytes, which are that text's bytes. Both are the bytes of the
// very text the key row form compared as `str`.

/// Whether `text` is already its own upper case under the byte-wise ASCII
/// test: pure ASCII with no lower-case byte.
fn already_upper_ascii(text: &str) -> bool {
    // One byte pass settles both halves: a byte at or past 0x80
    // (every byte of a multi-byte character) fails the ASCII half,
    // and an ASCII lower-case byte fails the upper-case half, so
    // the fused refusal test decides exactly what `is_ascii` and
    // the lower-case scan decided in their two passes.
    !text.bytes().any(|byte| !byte.is_ascii() || byte.is_ascii_lowercase())
}

/// The environment of this process with `name` set to `value`, as the block
/// `CreateProcessW` takes: sorted by name (compared in upper case, the name being the
/// text before the first `=` after the first character), each entry ended by NUL, then
/// one more NUL.
pub(super) fn environment_block(name: &str, value: &str) -> Vec<u16> {
    use crate::sys::windows::encode_wide;
    // The entries are held as `vars_os` yielded them, which already
    // exist, so no entry is encoded to be held. Beside them, aligned
    // by index, stands the owned sort text of exactly the entries
    // whose key cannot be read from their entry's own text: the
    // entries whose name is not already its own upper case, and the
    // ones whose name is not valid text at all.
    // An entry whose name is already upper-case ASCII — the ordinary
    // shape of a Windows variable name — builds no sort string at
    // all, where the table form built the lossy text of every name
    // into an owned key string for the sort.
    let mut entries: Vec<(std::ffi::OsString, std::ffi::OsString)> = Vec::new();
    let mut owned_keys: Vec<Option<String>> = Vec::new();
    // An upper bound on the block's size in units, accumulated from
    // byte lengths in the same pass: every UTF-16 unit of a name or
    // a value costs at least one byte of its encoded form (an ASCII
    // unit one byte, a surrogate pair four bytes for two units, an
    // unpaired surrogate three bytes for one), so a text's unit
    // count never exceeds its byte count. The bound is a capacity
    // only — the writes below are the block's contents whatever the
    // capacity — and it replaces a sizing pass that walked every
    // entry's units a second time only to count them, before the
    // writing pass walked them a third time to encode them.
    // The block's final NUL; each entry below adds its byte length,
    // its `=` and its own NUL.
    let mut capacity = 1usize;
    for (key, entry_value) in std::env::vars_os() {
        // Whether the entry is the one being set, and which sort key
        // it takes, are both read from the entry's name as text. A
        // name that is valid text — the ordinary case — is read
        // through the one validity check `to_str` makes: forming the
        // lossy text first validated the same bytes a second time,
        // and asking `to_str` again inside the upper-case branch
        // validated them a third time, for answers the first check
        // already settled. A name that is not valid text takes the
        // lossy walk exactly as before, whose text decides both
        // answers there.
        let (skip, owned_key) = match key.to_str() {
            Some(text) => {
                let skip = text.eq_ignore_ascii_case(name);
                let owned_key = if already_upper_ascii(text) { None } else { Some(text.to_uppercase()) };
                (skip, owned_key)
            }
            None => {
                let lossy = key.to_string_lossy();
                let skip = lossy.eq_ignore_ascii_case(name);
                let owned_key = if already_upper_ascii(&lossy) {
                    Some(lossy.into_owned())
                } else {
                    Some(lossy.to_uppercase())
                };
                (skip, owned_key)
            }
        };
        if skip {
            continue;
        }
        capacity += key.len() + 1 + entry_value.len() + 1;
        entries.push((key, entry_value));
        owned_keys.push(owned_key);
    }
    // The entry being set is appended as an owned pair built from
    // `name` and `value`, like the rest; its wide units are the
    // `encode_utf16` units of that text, as the arena form wrote.
    capacity += name.len() + 1 + value.len() + 1;
    entries.push((std::ffi::OsString::from(name), std::ffi::OsString::from(value)));
    owned_keys.push(if already_upper_ascii(name) { None } else { Some(name.to_uppercase()) });
    // The entries are sorted by index, so sorting moves indices and
    // key rows are never moved either; the sort is stable, as the
    // table form's was, so entries whose keys compare equal keep the
    // order `vars_os` gave them, the entry being set last among them.
    // The comparison reads each key's bytes in place (see the sort
    // key note above): an owned key's own bytes, or — for an entry
    // with no owned key, which the pass above has shown to be valid
    // text in exactly that case — its name's encoded bytes.
    let key_bytes = |index: usize| -> &[u8] {
        match &owned_keys[index] {
            Some(text) => text.as_bytes(),
            None => entries[index].0.as_os_str().as_encoded_bytes(),
        }
    };
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&a, &b| key_bytes(a).cmp(key_bytes(b)));
    let mut block = Vec::with_capacity(capacity);
    for &index in &order {
        let (entry_name, entry_value) = &entries[index];
        block.extend(encode_wide(entry_name));
        block.push(b'=' as u16);
        block.extend(encode_wide(entry_value));
        block.push(0);
    }
    block.push(0);
    block
}
