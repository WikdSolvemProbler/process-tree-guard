// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The hook's own paths: the program's names and family, and the automatic copy and policy file it looks for beside itself.

use super::*;

/// The three names of the one executable, the current name first. `process-guard` is the
/// earlier name, an accepted alias, and `ptg` is the short name of the full role; each
/// name also has an automatic copy, the same name with `-auto` added.
pub(crate) const NAMES: [&str; 3] = ["process-tree-guard", "process-guard", "ptg"];

/// The names in NAMES other than the one at `own`, in NAMES order.
pub(crate) fn others(own: usize) -> impl Iterator<Item = &'static str> {
    NAMES.iter().enumerate().filter(move |(index, _)| *index != own).map(|(_, name)| *name)
}

/// The folder of `exe` and the index in NAMES of its program name, when it has one.
pub(crate) fn family(exe: &str) -> Option<(&str, usize)> {
    let (dir, file) = exe.rsplit_once('/')?;
    // The stem, compared in place: every name is plain ASCII, so a
    // case-insensitive comparison needs no lowered copy of the file
    // name. The byte offset is a character boundary exactly when the
    // suffix can match (the four suffix bytes are ASCII), and `get`
    // yields None where it is not, which cannot match either.
    let stem = file
        .get(..file.len().checked_sub(4)?)
        .filter(|_| file[file.len() - 4..].eq_ignore_ascii_case(".exe"))?;
    NAMES.iter().position(|name| stem.eq_ignore_ascii_case(name)).map(|index| (dir, index))
}

/// The deadline role, which the hook role vouches for.
pub(crate) const DEADLINE: &str = "process-tree-guard-deadline";

/// The paths a guard envelope may name when the hook runs as the hook role in
/// `dir`: the programs that enforce a deadline, in `dir` and, in the install
/// layout (the last folder of `dir` is named `libexec`, in any letter case, as
/// Windows folder names are not case sensitive), in the `bin` folder beside it, with
/// the rest of the path as it is spelled. The hook role itself is never among them,
/// whatever its file name (`exe`), because it would not run the command.
#[cfg(test)]
pub(crate) fn gate_envelope_paths(exe: &str, dir: &str) -> Vec<String> {
    let mut folders = vec![dir.to_string()];
    if let Some((root, last)) = dir.rsplit_once('/') {
        if last.eq_ignore_ascii_case("libexec") {
            folders.push(format!("{}/bin", root));
        }
    }
    let mut paths = Vec::new();
    for folder in &folders {
        for name in NAMES.into_iter().chain([DEADLINE]) {
            paths.push(format!("{}/{}.exe", folder, name));
        }
    }
    // Windows paths are not case sensitive, and `exe` keeps the case it was started with.
    paths.retain(|path| !path.eq_ignore_ascii_case(exe));
    paths
}

/// The paths a guard envelope may name when this executable is `exe`: `exe` itself
/// and, when it is installed under one of the three names, the other names in the
/// same folder; for the hook role, see `gate_envelope_paths`. The other file need
/// not exist; a command that names a missing program only fails to run.
#[cfg(test)]
pub(crate) fn envelope_paths(exe: &str, role: Role) -> Vec<String> {
    if role == Role::Gate {
        return exe.rsplit_once('/').map_or_else(Vec::new, |(dir, _)| gate_envelope_paths(exe, dir));
    }
    let mut paths = vec![exe.to_string()];
    if let Some((dir, own)) = family(exe) {
        paths.extend(others(own).map(|name| format!("{}/{}.exe", dir, name)));
    }
    paths
}

/// The automatic wrapper paths beside `exe`: the wrapper of its own name first,
/// then the other names'. An executable under any other name keeps the earlier
/// behaviour: only `process-guard-auto.exe`. The hook role's own wrapper is
/// `process-tree-guard-deadline-auto.exe` in its folder, then the three names'
/// wrappers there. None when `exe` has no folder. The whole-string
/// form of `wrapper_specs`: production decisions hold the segment
/// form in their path set, and this form stays for its tests.
#[cfg(test)]
pub fn wrapper_paths(exe: &str, role: Role) -> Option<Vec<String>> {
    let (dir, _) = exe.rsplit_once('/')?;
    let auto = |name: &str| format!("{}/{}-auto.exe", dir, name);
    if role == Role::Gate {
        return Some([auto(DEADLINE)].into_iter().chain(NAMES.into_iter().map(auto)).collect());
    }
    Some(match family(exe) {
        Some((_, own)) => [auto(NAMES[own])].into_iter().chain(others(own).map(auto)).collect(),
        None => vec![auto(NAMES[1])],
    })
}

/// One automatic wrapper path as its segments — the borrowed
/// directory slice of the executable spelling and a static program
/// name — so a wrapper that is only compared against is never built
/// as a string. The wrapper the segments spell is
/// `<dir>/<name>-auto.exe`, exactly the string `wrapper_paths`
/// builds from the same directory and name.
#[derive(Clone, Copy)]
pub(crate) struct WrapperSpec<'a> {
    pub(crate) dir: &'a str,
    pub(crate) name: &'static str,
}

/// The wrapper specs of one executable, in fixed storage: there are
/// never more than four (the hook role's deadline name and the three names), so the list a
/// decision holds needs no heap vector — the vector form allocated
/// once for every call of a rewriting vendor, for a count the role
/// already bounds. The form mirrors `EnvelopePaths`: specs stand in
/// `list[..count]`, in the order they were added.
pub(crate) struct WrapperSet<'a> {
    pub(crate) list: [WrapperSpec<'a>; 4],
    pub(crate) count: usize,
}

impl<'a> WrapperSet<'a> {
    pub(crate) fn empty() -> Self {
        WrapperSet { list: [WrapperSpec { dir: "", name: "" }; 4], count: 0 }
    }
    pub(crate) fn push(&mut self, spec: WrapperSpec<'a>) {
        if self.count < self.list.len() {
            self.list[self.count] = spec;
            self.count += 1;
        }
    }
    pub(crate) fn first(&self) -> Option<&WrapperSpec<'a>> {
        self.list[..self.count].first()
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = &WrapperSpec<'a>> {
        self.list[..self.count].iter()
    }
}

/// The same wrappers `wrapper_paths` builds as strings, spelled as
/// segments, in the same order: the hook role's deadline name first,
/// then the three names; otherwise the executable's own name then the
/// others, or the earlier name alone when the executable matches no
/// family. The no-folder case yields an empty list, where
/// `wrapper_paths` yields None.
pub(crate) fn wrapper_specs<'a>(exe: &'a str, role: Role) -> WrapperSet<'a> {
    let mut specs = WrapperSet::empty();
    let (dir, _) = match exe.rsplit_once('/') {
        Some(pair) => pair,
        None => return specs,
    };
    if role == Role::Gate {
        specs.push(WrapperSpec { dir, name: DEADLINE });
        for name in NAMES {
            specs.push(WrapperSpec { dir, name });
        }
        return specs;
    }
    match family(exe) {
        Some((_, own)) => {
            specs.push(WrapperSpec { dir, name: NAMES[own] });
            for name in others(own) {
                specs.push(WrapperSpec { dir, name });
            }
        }
        None => specs.push(WrapperSpec { dir, name: NAMES[1] }),
    }
    specs
}

/// The remainder of `text` after the wrapper `spec` spells, when
/// `text` starts with exactly that wrapper: the segment walk of
/// `envelope_for_segments_trimmed` over the directory, the separator, the
/// program name and `-auto.exe`, matching character for character
/// exactly, as `strip_prefix` on the built wrapper string does.
/// A spec character can only match a whole text character, so the
/// byte index the walk leaves stands on a character boundary.
pub(crate) fn strip_wrapper_prefix<'t>(text: &'t str, spec: WrapperSpec<'_>) -> Option<&'t str> {
    // Segment by segment in slices: `starts_with` on the segment
    // decides exactly what the per-character walk decided — a
    // segment is valid UTF-8, so its byte prefix stands in `text`
    // exactly when its characters do, in order — and the index a
    // matched segment leaves stands on a character boundary, as
    // the walk's did, because the matched bytes are the segment's
    // own encoding.
    let mut index = 0usize;
    for seg in [spec.dir, "/", spec.name, "-auto.exe"] {
        if !text[index..].starts_with(seg) {
            return None;
        }
        index += seg.len();
    }
    Some(&text[index..])
}

/// The path sets of one hook decision, computed once: the envelope
/// paths a command may name, and the automatic wrapper paths beside
/// the executable. A decision that needs both sets — the rewriting
/// vendors — computes them here, once, and threads the one set
/// through its checks instead of rebuilding either list per check.
/// Both halves are the borrowed segment form (see
/// `envelope_path_specs` and `wrapper_specs`): the rewriting vendors
/// only ever compare against them, so they are never built as path
/// strings. The one wrapper string a decision does need — the first
/// wrapper, which is interpolated into the rewritten command,
/// probed for beside the executable and named in a denial — is
/// materialised once as `first_wrapper`.
pub(crate) struct PathSet<'a> {
    pub(crate) envelope: EnvelopePaths<'a>,
    pub(crate) wrappers: WrapperSet<'a>,
    pub(crate) first_wrapper: Option<String>,
}

/// The one computation of both path sets for executable `exe`.
pub(crate) fn path_set<'a>(exe: &'a str, role: Role) -> PathSet<'a> {
    let wrappers = wrapper_specs(exe, role);
    let first_wrapper = wrappers.first().map(|spec| format!("{}/{}-auto.exe", spec.dir, spec.name));
    PathSet { envelope: envelope_path_specs(exe, role), wrappers, first_wrapper }
}

/// True when the already-trimmed `command` is exactly the guard
/// envelope naming one of the envelope paths in `envelope` (see
/// `envelope_path_specs`): the segment form of the set
/// `envelope_paths` builds as strings, giving the same verdict for
/// every input (see `envelope_for_segments_trimmed`). Each caller
/// trims once for the whole set — `powershell_auto` on the text it
/// already trimmed, `antigravity_guarded` on a text it has proved
/// stable under a Unicode trim — and this walk shares that one
/// trim across every spec, instead of re-scanning the command's
/// leading and trailing whitespace once per spec (up to five on
/// the gate role) for a result the first trim already settled.
/// Trimming is idempotent, so the one trim and a per-spec trim
/// decide identically for every input.
pub(crate) fn valid_envelope_paths_trimmed(command: &str, envelope: &EnvelopePaths) -> bool {
    envelope.list[..envelope.count].iter().any(|spec| envelope_for_segments_trimmed(command, *spec))
}

/// The executable spelling held in stack storage: the UTF-8 bytes of
/// the path as `GetModuleFileNameW` reports it, with forward slashes.
/// A name of at most 1,023 UTF-16 units spells at most three bytes
/// per unit, so this storage always suffices for a name that fits.
pub(crate) struct StackSpelling {
    // Uninitialised storage: the decode loop in `stack_self_path`
    // writes each byte of the `len` prefix before any read of it
    // (`as_str` and the prefix strip are confined to that prefix),
    // so no byte is read before the write that produced it and none
    // of the 3,072 bytes needs clearing first — the same discipline
    // as the scanner window above and the line storage in
    // `cli_process`.
    bytes: [std::mem::MaybeUninit<u8>; 3072],
    len: usize,
}

impl StackSpelling {
    pub(crate) fn as_str(&self) -> &str {
        // The bytes were encoded from decoded characters as they were
        // written, so they are valid UTF-8 by construction. The read
        // is confined to the written prefix.
        let bytes = unsafe { std::slice::from_raw_parts(self.bytes.as_ptr() as *const u8, self.len) };
        std::str::from_utf8(bytes).unwrap_or("")
    }
}

/// Spell this executable's path into stack storage, exactly as
/// `self_path` spells it (forward slashes, the `//?/` prefix
/// removed): the module name is read into a 1,024-unit stack buffer
/// and decoded in place. A name that does not fit, cannot be read,
/// or does not decode returns None and the caller takes the standard
/// call, whose growth loop settles those cases as it did before.
pub(crate) fn stack_self_path() -> Option<StackSpelling> {
    let mut buffer = [std::mem::MaybeUninit::<u16>::uninit(); 1024];
    let units = crate::sys::windows::image_name_units(&mut buffer)?;
    let mut spelling = StackSpelling { bytes: [std::mem::MaybeUninit::uninit(); 3072], len: 0 };
    for decoded in char::decode_utf16(units.iter().map(|&unit| if unit == b'\\' as u16 { b'/' as u16 } else { unit })) {
        let c = decoded.ok()?;
        let mut buf = [0u8; 4];
        let text = c.encode_utf8(&mut buf);
        if spelling.len + text.len() > spelling.bytes.len() {
            return None;
        }
        // Each byte of the character's encoding is written into its
        // slot before `len` advances past it.
        for (slot, &byte) in spelling.bytes[spelling.len..spelling.len + text.len()].iter_mut().zip(text.as_bytes()) {
            slot.write(byte);
        }
        spelling.len += text.len();
    }
    // The `//?/` prefix is tested on the written bytes themselves:
    // the prefix is plain ASCII, so its encoding is itself, and the
    // bytes are valid UTF-8 by construction (the same fact `as_str`
    // rests on), so byte equality here is the string test. Asking
    // `as_str` for it validated the whole spelling — a full scan of
    // up to 3,072 bytes — to decide a four-byte prefix, on every
    // call that resolves the spelling.
    let has_prefix = spelling.len >= 4
        && (0..4).all(|i| unsafe { spelling.bytes[i].assume_init_read() } == b"//?/"[i]);
    if has_prefix {
        // The strip moves the written prefix only: bytes `4..len`
        // land on `0..len - 4`, every one of them written above.
        spelling.bytes.copy_within(4..spelling.len, 0);
        spelling.len -= 4;
    }
    Some(spelling)
}

/// The executable path exactly as the envelope must spell it: forward
/// slashes, by the standard call. A caller that has already tried the
/// stack spelling takes this directly for a name that does not fit
/// it: a combined entry that retried the stack spelling first would
/// run the module-name query and its decode walk a second time, to
/// fail the same way, before the standard call runs.
pub fn self_path() -> Option<String> {
    let path = std::env::current_exe().ok()?;
    let mut text = path.to_str()?.replace('\\', "/");
    // The prefix is removed in place: copying the stripped form into a
    // second string would allocate on a path every hook call pays.
    if text.starts_with("//?/") {
        text.drain(..4);
    }
    Some(text)
}

/// The executable spelling a caller uses:
/// the stack spelling when it was taken, else
/// the standard call's owned string, held in
/// the caller's `owned` slot — filled on this
/// arm alone, as `LazyPaths::exe` fills its own.
pub(crate) fn exe_spelling<'a>(
    stack: &'a Option<StackSpelling>,
    owned: &'a mut Option<String>,
) -> &'a str {
    match stack {
        Some(spelling) => spelling.as_str(),
        None => owned
            .get_or_insert_with(|| self_path().unwrap_or_default())
            .as_str(),
    }
}

/// The policy file of the executable at `exe`: `POLICY.md` in the folder above the
/// executable's own folder, whatever those folders are called.
pub(crate) fn policy_path(exe: &str) -> Option<String> {
    let (dir, _) = exe.rsplit_once('/')?;
    let (root, _) = dir.rsplit_once('/')?;
    Some(format!("{}/POLICY.md", root))
}

/// The text written when no readable policy file is found.
pub(crate) const POLICY_NOT_FOUND: &str = "POLICY.md not found beside the installed process guard.";

/// Append the policy text found relative to this executable alone to
/// `out`, as a quoted JSON string escaped directly into the buffer
/// being built. The file bytes come through a raw handle that is
/// never closed (see `cli_process::read_file_bytes`) and are
/// validated as UTF-8 here, with exactly the strictness the standard
/// string read applied: any failure — no executable path, no policy
/// path, a missing file, a read error, invalid UTF-8 — writes the
/// fallback text instead, as the standard read's failure did.
pub(crate) fn write_policy(out: &mut String) {
    // The stack spelling first, as in the vendors of `main`: this
    // function's only use of the executable path is to derive the
    // policy path from it, and `self_path` copies the spelling the
    // stack form already holds into a fresh string for that one
    // derivation, on every context-vendor call. The allocating form
    // is the fallback for a name that does not fit the stack storage.
    let stack = stack_self_path();
    let mut owned = None;
    let exe = exe_spelling(&stack, &mut owned);
    let bytes = policy_path(exe)
        .and_then(|path| crate::cli_process::read_file_bytes(&path));
    let text = bytes.as_deref().and_then(|raw| std::str::from_utf8(raw).ok()).unwrap_or(POLICY_NOT_FOUND);
    write_quote(text, out);
}

/// True when every character of `path` is in `[A-Za-z0-9:/._-]`, so the path can
/// be written without quoting and parse the same in PowerShell, cmd and Git Bash.
pub(crate) fn plain_path(path: &str) -> bool {
    path.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'/' | b'.' | b'_' | b'-'))
}
