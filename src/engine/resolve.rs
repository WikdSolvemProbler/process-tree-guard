// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Finding the program to run: the PATH and PATHEXT search, in the order the system search uses.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The parsed PATHEXT list: the one environment text plus the byte range of
/// each extension in it, so an extension is borrowed from the text with no
/// copy of its own, exactly as a live parse borrows its slices.
struct PathextList {
    text: String,
    ranges: Vec<std::ops::Range<usize>>,
}

impl PathextList {
    fn iter(&self) -> impl Iterator<Item = &str> {
        self.ranges.iter().map(|range| &self.text[range.start..range.end])
    }
}

/// The parsed PATH search: whether the current folder is searched first,
/// then every folder of PATH in order.
struct SearchDirs {
    current: bool,
    dirs: Vec<PathBuf>,
}

/// A parsed search part, borrowed from its process-lifetime cache on a
/// process-owning CLI invocation and owned by the call otherwise: the same
/// cached/uncached split as `trace_enabled_with`. The environment of a
/// one-shot guard process does not change under it, so a CLI invocation
/// parses each part once; a returning library call reads the live
/// environment on every call and never touches the cache.
enum SearchPart<'a, T> {
    Cached(&'a T),
    Live(T),
}

impl<T> std::ops::Deref for SearchPart<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Self::Cached(value) => value,
            Self::Live(value) => value,
        }
    }
}

fn search_part<'a, T>(active: bool, cache: &'a OnceLock<T>, live: impl FnOnce() -> T) -> SearchPart<'a, T> {
    if active { SearchPart::Cached(cache.get_or_init(live)) } else { SearchPart::Live(live()) }
}

fn read_pathext() -> PathextList {
    let text = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    // `;` is one byte and cannot occur inside a multi-byte character, so the
    // ranges are character boundaries exactly where `split(';')` would cut.
    let mut ranges = Vec::new();
    let mut begin = 0usize;
    for (index, byte) in text.bytes().enumerate() {
        if byte == b';' {
            if index > begin {
                ranges.push(begin..index);
            }
            begin = index + 1;
        }
    }
    if text.len() > begin {
        ranges.push(begin..text.len());
    }
    PathextList { text, ranges }
}

fn read_search_dirs() -> SearchDirs {
    SearchDirs {
        current: std::env::var_os("NoDefaultCurrentDirectoryInExePath").is_none(),
        dirs: std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default(),
    }
}

/// Locate an executable as the PATH search does, honouring PATHEXT. This finds
/// the program the caller asked to run and the shell the guard chooses, as in the earlier version;
/// the guard's own files are never found this way.
pub fn which(name: &str) -> Option<String> {
    use std::path::Path;
    let active = crate::cli_process::active();
    static PATHEXT: OnceLock<PathextList> = OnceLock::new();
    // The extensions are borrowed slices of the one environment read, compared
    // case-insensitively in place: no lowered copies of the extensions or of
    // the name, and no candidate list built before the first probe.
    let pathext = search_part(active, &PATHEXT, read_pathext);
    let named = pathext.iter().any(|e| name.len() >= e.len()
        && name.get(name.len() - e.len()..).is_some_and(|tail| tail.eq_ignore_ascii_case(e)));
    // One candidate buffer for the whole search, its capacity kept across
    // folders and extensions: a candidate is built in it by pushing the
    // folder, then the name, then the extension onto the name, and between
    // candidates the buffer is truncated back to the folder alone (and
    // cleared between folders), so a search allocates the buffer once and
    // the hit's result string once instead of a fresh string and path for
    // every candidate probed. The probes themselves are unchanged: the
    // same candidates in the same order, each tested with `is_file`.
    let mut candidate = PathBuf::new();
    // One pass decides whether the name carries a separator: `/`
    // and `\` are ASCII, so in text they are single bytes that
    // never occur inside a multi-byte character (whose bytes are
    // all at least 0x80) — the byte test settles exactly what the
    // two `contains` scans settled, without walking the name twice
    // for it (the same fusion as `needs_quote` below).
    if name.bytes().any(|byte| byte == b'/' || byte == b'\\') {
        if named {
            return Path::new(name).is_file().then(|| name.to_string());
        }
        for extension in pathext.iter() {
            candidate.clear();
            candidate.push(name);
            candidate.as_mut_os_string().push(extension);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        return None;
    }
    let mut in_dir = |dir: &Path| -> Option<String> {
        candidate.clear();
        candidate.push(dir);
        let dir_len = candidate.as_os_str().len();
        if named {
            candidate.push(name);
            return candidate.is_file().then(|| candidate.to_string_lossy().into_owned());
        }
        for extension in pathext.iter() {
            candidate.push(name);
            candidate.as_mut_os_string().push(extension);
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
            while candidate.as_os_str().len() > dir_len {
                if !candidate.pop() {
                    break;
                }
            }
        }
        None
    };
    static SEARCH_DIRS: OnceLock<SearchDirs> = OnceLock::new();
    let search_dirs = search_part(active, &SEARCH_DIRS, read_search_dirs);
    // The folders are walked in order, current folder first, so the search
    // stops at the folder that holds the program instead of probing every
    // PATH folder (and every candidate) once a folder has matched.
    if search_dirs.current {
        if let Some(found) = in_dir(Path::new(".")) {
            return Some(found);
        }
    }
    for dir in &search_dirs.dirs {
        if let Some(found) = in_dir(dir) {
            return Some(found);
        }
    }
    None
}

/// Quote arguments into one command line exactly as the C runtime parses it back.
pub(crate) fn list2cmdline(args: &[String]) -> String {
    command_line(args.iter().map(String::as_str))
}

/// Whether an argument needs quoting: empty, or holding a space
/// or a tab. One pass decides it: space and tab are ASCII, so in
/// text they are single bytes that never occur inside a multi-byte
/// character (whose bytes are all at least 0x80), and in wide units
/// they are single units that are no half of a surrogate pair — so
/// the byte (or unit) test settles exactly what the two `contains`
/// scans settled, without walking the argument twice for it before
/// the rendering walk walks it a third time.
fn needs_quote(text: &str) -> bool {
    text.is_empty() || text.bytes().any(|byte| byte == b' ' || byte == b'\t')
}

fn needs_quote_units(units: &[u16]) -> bool {
    units.is_empty() || units.iter().any(|&unit| unit == b' ' as u16 || unit == b'\t' as u16)
}

/// Writable UTF-16 command line. The common short invocation stays in this stack
/// buffer; longer commands spill without changing CreateProcessW's mutable input.
/// The stack storage is uninitialised: `push` writes the unit at `len` before
/// advancing it, and every read (`as_slice`, the spill copy, the raw pointers
/// handed to CreateProcessW behind the pushed NUL) stays within that prefix.
pub(super) struct WideLine<const N: usize> {
    pub(super) stack: [std::mem::MaybeUninit<u16>; N],
    pub(super) len: usize,
    pub(super) heap: Option<Vec<u16>>,
}

impl<const N: usize> WideLine<N> {
    pub(super) fn new() -> Self { Self { stack: [std::mem::MaybeUninit::uninit(); N], len: 0, heap: None } }

    pub(super) fn push(&mut self, unit: u16) {
        if let Some(heap) = &mut self.heap {
            heap.push(unit);
        } else if self.len < self.stack.len() {
            self.stack[self.len].write(unit);
            self.len += 1;
        } else {
            let mut heap = Vec::with_capacity(self.len * 2);
            // The copied prefix is exactly the units `push` has written.
            heap.extend_from_slice(unsafe { std::slice::from_raw_parts(self.stack.as_ptr() as *const u16, self.len) });
            heap.push(unit);
            self.heap = Some(heap);
        }
    }

    pub(super) fn extend_text(&mut self, text: &str) {
        for unit in text.encode_utf16() { self.push(unit); }
    }

    pub(super) fn as_mut_ptr(&mut self) -> *mut u16 {
        match &mut self.heap {
            Some(heap) => heap.as_mut_ptr(),
            None => self.stack.as_mut_ptr() as *mut u16,
        }
    }

    pub(super) fn as_ptr(&self) -> *const u16 {
        match &self.heap {
            Some(heap) => heap.as_ptr(),
            None => self.stack.as_ptr() as *const u16,
        }
    }

    pub(super) fn as_slice(&self) -> &[u16] {
        match &self.heap {
            Some(heap) => heap,
            // `len` counts exactly the units `push` has written.
            None => unsafe { std::slice::from_raw_parts(self.stack.as_ptr() as *const u16, self.len) },
        }
    }
}

/// Keep the explicit application pointer while avoiding a heap allocation for
/// ordinary short executable paths. Embedded NUL units retain wide()'s bytes.
pub(super) fn application_wide(text: &str) -> WideLine<260> {
    let mut result = WideLine::<260>::new();
    result.extend_text(text);
    result.push(0);
    result
}

/// Render one already-encoded UTF-16 argument. ASCII quote and backslash
/// units cannot be part of a surrogate pair, so the CRT rules are identical
/// to applying them before UTF-16 encoding.
pub(super) fn append_wide_arg(result: &mut WideLine<1024>, units: impl IntoIterator<Item = u16>, quote: bool) {
    if result.len != 0 || result.heap.is_some() { result.push(' ' as u16); }
    if quote { result.push('"' as u16); }
    let mut backslashes = 0usize;
    for unit in units {
        match unit {
            0x5c => backslashes += 1,
            0x22 => {
                for _ in 0..backslashes * 2 + 1 { result.push('\\' as u16); }
                backslashes = 0;
                result.push('"' as u16);
            }
            _ => {
                for _ in 0..backslashes { result.push('\\' as u16); }
                backslashes = 0;
                result.push(unit);
            }
        }
    }
    for _ in 0..if quote { backslashes * 2 } else { backslashes } { result.push('\\' as u16); }
    if quote { result.push('"' as u16); }
}

pub(super) fn finish_wide_line(result: &mut WideLine<1024>, raw_tail: Option<&str>) {
    if let Some(tail) = raw_tail {
        result.push(' ' as u16);
        result.push('"' as u16);
        result.extend_text(tail);
        result.push('"' as u16);
    }
    result.push(0);
}

/// Writable UTF-16 command line with the old independent first-argument path.
/// Retained as the parity reference for the fused production renderer.
#[cfg(test)]
pub(super) fn command_line_wide<'a>(args: impl Iterator<Item = &'a str>, raw_tail: Option<&str>) -> WideLine<1024> {
    let mut result = WideLine::<1024>::new();
    for arg in args {
        let quote = needs_quote(arg);
        append_wide_arg(&mut result, arg.encode_utf16(), quote);
    }
    finish_wide_line(&mut result, raw_tail);
    result
}

/// Reuse the exact application spelling for argv[0], excluding only its final
/// terminator. Embedded NUL units remain in both native buffers as before.
pub(super) fn command_line_wide_from_application<'a>(application: &WideLine<260>, args: impl Iterator<Item = &'a str>, raw_tail: Option<&str>) -> WideLine<1024> {
    let mut result = WideLine::<1024>::new();
    let spelling = application.as_slice();
    let first = &spelling[..spelling.len() - 1];
    let quote = needs_quote_units(first);
    append_wide_arg(&mut result, first.iter().copied(), quote);
    for arg in args {
        let quote = needs_quote(arg);
        append_wide_arg(&mut result, arg.encode_utf16(), quote);
    }
    finish_wide_line(&mut result, raw_tail);
    result
}

pub(super) fn command_line<'a>(args: impl Iterator<Item = &'a str>) -> String {
    let mut result = String::new();
    for arg in args {
        if !result.is_empty() {
            result.push(' ');
        }
        let quote = needs_quote(arg);
        if quote {
            result.push('"');
        }
        // The pending backslash run is a count, written straight
        // into the result when it ends — the wide renderer's form
        // (`append_wide_arg`). The string this replaces held the
        // run in an auxiliary string built per argument and built
        // a `repeat` temporary per embedded quote; the counts are
        // the same ones (each held character was one backslash, so
        // the held length was the count), and the written text is
        // unchanged: 2n+1 backslashes before an embedded quote,
        // the run itself before any other character, and the run
        // doubled ahead of the closing quote when quoting.
        let mut backslashes = 0usize;
        for c in arg.chars() {
            match c {
                '\\' => backslashes += 1,
                '"' => {
                    for _ in 0..backslashes * 2 + 1 {
                        result.push('\\');
                    }
                    backslashes = 0;
                    result.push('"');
                }
                _ => {
                    for _ in 0..backslashes {
                        result.push('\\');
                    }
                    backslashes = 0;
                    result.push(c);
                }
            }
        }
        for _ in 0..backslashes {
            result.push('\\');
        }
        if quote {
            for _ in 0..backslashes {
                result.push('\\');
            }
            result.push('"');
        }
    }
    result
}
