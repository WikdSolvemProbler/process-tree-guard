// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The owners a run watches: the parent and ancestors, opened once, and the walk that finds the boundary above them.

use super::*;


use crate::sys::windows::{

    CloseHandle, CreateToolhelp32Snapshot, GetCurrentProcess, GetCurrentProcessId, GetLastError,

    GetProcessTimes, GetSystemWindowsDirectoryW, Handle, INVALID_HANDLE,

    NtQueryInformationProcess, NtQuerySystemInformation, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,

    Process32FirstW, Process32NextW, ProcessBasicInformation, ProcessEntry32W, ProcessIdInformation,

    QueryDosDeviceW, QueryFullProcessImageNameW, SNAPPROCESS, SYNCHRONIZE,

    SYSTEM_PROCESS_ID_INFORMATION_CLASS, os_error,

};
/// The parent's process ID straight from this process's own record; falls back
/// to a full process snapshot only if that query fails.
pub(crate) fn parent_process_id() -> Result<u32, GuardError> {
    parent_process_id_on(unsafe { GetCurrentProcess() })
}

/// `parent_process_id` on a current-process pseudo-handle the caller
/// already fetched for a creation-time read of this same process: the
/// pseudo-handle is this process's own constant handle, so fetching
/// it again for the query beside that read bought nothing — the same
/// discipline `harden_self` applies to its two uses of the handle in
/// `cli_process`.
fn parent_process_id_on(process: Handle) -> Result<u32, GuardError> {
    unsafe {
        let mut info: ProcessBasicInformation = std::mem::zeroed();
        let status = NtQueryInformationProcess(process, 0, &mut info as *mut _ as *mut c_void,
            std::mem::size_of::<ProcessBasicInformation>() as u32, std::ptr::null_mut());
        if status == 0 && info.parent_process_id != 0 {
            return Ok(info.parent_process_id as u32);
        }
    }
    parent_process_id_by_snapshot()
}

pub(super) fn parent_process_id_by_snapshot() -> Result<u32, GuardError> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE {
            return Err(os_error());
        }
        let me = GetCurrentProcessId();
        let mut entry: ProcessEntry32W = std::mem::zeroed();
        entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
        let mut found = None;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            if entry.process_id == me {
                found = Some(entry.parent_process_id);
                break;
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        found.ok_or_else(|| GuardError("cannot find the parent process".into()))
    }
}

/// The time the process behind `handle` was created, as the system counts it; the handle needs
/// the right to query limited information.
pub(crate) fn started_at(handle: Handle) -> Option<u64> {
    let (mut c, mut e, mut k, mut u) = (0u64, 0u64, 0u64, 0u64);
    let ok = unsafe { GetProcessTimes(handle, &mut c, &mut e, &mut k, &mut u) };
    if ok == 0 { None } else { Some(c) }
}

/// The programs of the desktop shell and of the system that end the walk up the parents (for the
/// refusal of a run that another guard started as well as automatic monitoring).
pub(super) const WALK_ENDS_AT: [&str; 6] = ["explorer.exe", "services.exe", "wininit.exe", "winlogon.exe", "svchost.exe", "smss.exe"];

/// The Windows folder as the system gives it, without a final separator; empty when it cannot be read.
pub(super) fn windows_folder() -> String {
    let mut buffer = [std::mem::MaybeUninit::<u16>::uninit(); 512];
    let length = unsafe { GetSystemWindowsDirectoryW(buffer.as_mut_ptr() as *mut u16, buffer.len() as u32) } as usize;
    if length == 0 || length >= buffer.len() {
        return String::new();
    }
    // The call wrote the `length` units the read below is confined to.
    // Trailing separators are popped from the converted string itself:
    // trimming into a second string would allocate again per call.
    let mut folder = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u16, length) });
    while folder.ends_with('\\') {
        folder.pop();
    }
    folder
}

/// Whether the walk up the parents ends at a parent with this file name and image path: only one
/// of the six programs of the system, at its system path (explorer.exe in the Windows folder, the
/// other five in System32). A program of that name in any other folder, one under the Windows
/// folder included, is a copy that a process of the user may be able to make, and is walked past
/// like any other process; so is one whose path cannot be read, which fails toward a refusal.
pub(crate) fn ends_the_walk(name: &str, image: &str, windows: &str) -> bool {
    // The six names are plain ASCII, so comparing the name as it stands,
    // case-insensitively in place, decides exactly what comparing its
    // lowered copy decided, with no copy made.
    if windows.is_empty() || image.is_empty() || !WALK_ENDS_AT.iter().any(|end| end.eq_ignore_ascii_case(name)) {
        return false;
    }
    // The expected path — the Windows folder, `\System32` for every name
    // but the shell's, then the name — is compared against `image`
    // segment by segment, in place: building it as a string allocated
    // twice for every ancestor level the walk examines. A segment whose
    // length would split a character of `image` cannot match, exactly
    // as the built string could not.
    let Some(head) = image.get(..windows.len()) else { return false };
    if !head.eq_ignore_ascii_case(windows) {
        return false;
    }
    let mut rest = &image[windows.len()..];
    if !name.eq_ignore_ascii_case("explorer.exe") {
        let Some(system) = rest.get(.."\\System32".len()) else { return false };
        if !system.eq_ignore_ascii_case("\\System32") {
            return false;
        }
        rest = &rest["\\System32".len()..];
    }
    rest.len() == name.len() + 1 && rest.starts_with('\\') && rest[1..].eq_ignore_ascii_case(name)
}

/// One wide unit folded for ASCII-only case-insensitive comparison:
/// only `A` to `Z` fold, exactly as `char::eq_ignore_ascii_case` folds
/// only ASCII, so a non-ASCII unit matches only itself.
fn fold_wide_unit(unit: u16) -> u16 {
    if (0x41..=0x5a).contains(&unit) { unit + 0x20 } else { unit }
}

/// Whether two wide-unit runs are equal under ASCII-only
/// case-insensitive comparison, unit by unit in place.
fn wide_units_eq_ignore_ascii_case(a: &[u16], b: &[u16]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(&x, &y)| fold_wide_unit(x) == fold_wide_unit(y))
}

/// Whether a wide-unit run spells `ascii` (a pure-ASCII text) under
/// ASCII-only case-insensitive comparison: a unit matches the ASCII
/// character exactly when it is that character folded, so a non-ASCII
/// unit — a surrogate half included — never matches.
fn wide_units_eq_ascii(units: &[u16], ascii: &str) -> bool {
    units.len() == ascii.len()
        && units.iter().zip(ascii.bytes()).all(|(&unit, byte)| fold_wide_unit(unit) == fold_wide_unit(byte as u16))
}

/// Whether `units` contains an unpaired surrogate: a high surrogate
/// not followed by a low one, or a low one standing alone. Valid pairs
/// and every other unit are well-formed.
pub(crate) fn has_unpaired_surrogate(units: &[u16]) -> bool {
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if (0xd800..0xdc00).contains(&unit) {
            if index + 1 < units.len() && (0xdc00..0xe000).contains(&units[index + 1]) {
                index += 2;
            } else {
                return true;
            }
        } else if (0xdc00..0xe000).contains(&unit) {
            return true;
        } else {
            index += 1;
        }
    }
    false
}

/// The wide-unit form of `ends_the_walk`: the same test, with the
/// name, the image and the Windows folder still in their reported
/// wide units, compared in place with no string formed. The six names
/// and `\System32` are pure ASCII, matched unit by unit under
/// ASCII-only folding; the folder prefix compares unit for unit under
/// the same rule; the length arithmetic is `ends_the_walk`'s own, in
/// units. Guard: this form is only for inputs with no unpaired
/// surrogate — `String::from_utf16_lossy` maps each unpaired
/// surrogate to U+FFFD, so two different unpaired surrogates, or one
/// beside a real U+FFFD, compare equal in the string form while their
/// units differ. Valid pairs and all other units compare identically
/// under both forms (non-ASCII comparison is identity in both), so on
/// guarded inputs the verdict is `ends_the_walk`'s verdict on the
/// lossy strings. The caller (the console finish's boundary test)
/// makes the guard check and falls back to the string form for any
/// level that fails it.
pub(crate) fn ends_the_walk_wide(name: &[u16], image: &[u16], windows: &[u16]) -> bool {
    if windows.is_empty() || image.is_empty() || !WALK_ENDS_AT.iter().any(|end| wide_units_eq_ascii(name, end)) {
        return false;
    }
    if image.len() < windows.len() || !wide_units_eq_ignore_ascii_case(&image[..windows.len()], windows) {
        return false;
    }
    let mut rest = &image[windows.len()..];
    if !wide_units_eq_ascii(name, "explorer.exe") {
        let system32 = "\\System32";
        if rest.len() < system32.len() || !wide_units_eq_ascii(&rest[..system32.len()], system32) {
            return false;
        }
        rest = &rest[system32.len()..];
    }
    rest.len() == name.len() + 1 && rest[0] == b'\\' as u16 && wide_units_eq_ignore_ascii_case(&rest[1..], name)
}

// The bounded deep-ancestry policy, with room for extension wake and timer.
pub(super) const MAX_WATCHED_OWNERS: usize = 4096;
pub(super) const OWNERS_PER_WINDOW: usize = 60;
pub(crate) const MAX_ANCESTOR_LEVELS: usize = 32;
pub(crate) const MAX_CONSOLE_LEVELS: usize = 64;
const _: () = assert!(OWNERS_PER_WINDOW + 3 <= 64);

// Ordered keys avoid randomized-hasher initialization and remain O(log n)
// even for adversarial process identifiers. Insert replaces the same PID just
// as the snapshot map did; ancestry traversal keeps its independent 4096 cap.
pub(super) type ParentMap = std::collections::BTreeMap<u32, u32>;

/// A process object opened for AUTO shell selection and retained through the
/// same run's owner wait and console cleanup. Never cached across invocations.
pub(crate) struct ParentFacts {
    pub(super) handle: Handle,
    pub(super) id: u32,
    pub(super) started: u64,
    pub(super) ours: u64,
    pub(crate) image: String,
}
impl ParentFacts {
    pub(crate) fn open() -> Result<Self, GuardError> {
        let process = unsafe { GetCurrentProcess() };
        let ours = started_at(process)
            .ok_or_else(|| GuardError("this process's creation time cannot be verified".into()))?;
        let id = parent_process_id_on(process)?;
        let handle = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, id) };
        if handle.is_null() { return Err(os_error()); }
        Self::from_opened(id, ours, handle)
    }

    // Takes ownership immediately, including every failed identity query.
    pub(super) fn from_opened(id: u32, ours: u64, handle: Handle) -> Result<Self, GuardError> {
        let mut facts = Self { handle, id, started: 0, ours, image: String::new() };
        facts.started = started_at(handle).filter(|&started| started <= ours)
            .ok_or_else(|| GuardError("the parent cannot be verified and watched".into()))?;
        facts.image = image_path_of_handle(handle);
        Ok(facts)
    }
}
impl Drop for ParentFacts {
    fn drop(&mut self) { unsafe { CloseHandle(self.handle) }; }
}

/// Already-verified AUTO ancestry, with the same retained handles used to wait.
/// Console cleanup must not rediscover those objects by PID.
#[derive(Clone, Copy, Default)]
pub(crate) struct ConsoleOwner {
    pub(crate) id: u32,
    pub(crate) handle: Handle,
    pub(crate) boundary: bool,
    pub(crate) recheck_boundary: bool,
}

pub(super) struct OwnerObservation {
    pub(super) handle: Handle,
    pub(super) started: Option<u64>,
    pub(super) image: String,
    pub(super) waitable: bool,
}

/// Decide coverage from facts read on the handle that will be waited upon. The
/// observer keeps every opened handle in the caller's cleanup list, including
/// handles rejected here. A continuing chain past the wait budget is refused.
#[cfg(test)]
pub(super) fn collect_owner_handles(
    map: &ParentMap, child: u32, child_started: u64, windows: &str,
    observe: impl FnMut(u32) -> Result<OwnerObservation, GuardError>,
) -> Result<HandleList, GuardError> {
    collect_owner_handles_with_levels(map, child, child_started, windows, observe, &mut None)
}

pub(super) fn collect_owner_handles_with_levels(
    map: &ParentMap,
    mut child: u32,
    mut child_started: u64,
    windows: &str,
    mut observe: impl FnMut(u32) -> Result<OwnerObservation, GuardError>,
    levels: &mut Option<SmallList<ConsoleOwner, 16>>,
) -> Result<HandleList, GuardError> {
    let mut ids: SmallList<u32, 16> = SmallList::new();
    let mut owners = HandleList::new();
    loop {
        let parent = match map.get(&child) {
            Some(parent) => *parent,
            None => return Err(GuardError("cannot verify the complete ancestor chain".into())),
        };
        if parent == 0 || parent == 4 {
            break;
        }
        if ids.contains(&parent) {
            return Err(GuardError("ancestor chain contains a cycle".into()));
        }
        let facts = observe(parent)?;
        let name = crate::file_name_of(&facts.image);
        let stop = ends_the_walk(name, &facts.image, windows);
        // As before, a direct parent that is itself the desktop boundary is
        // still watched, so an automatic run never starts without an owner.
        // Above that parent a boundary ends the walk before its creation
        // time is required: a later process would end it too, and a
        // boundary this user cannot open has no creation time to read.
        if stop && !owners.is_empty() {
            break;
        }
        let started = facts.started.ok_or_else(|| GuardError("ancestor creation time cannot be verified".into()))?;
        if started > child_started {
            // The identifier belongs to a later process, so this is the end of
            // the live chain, not a handle to admit as one of its ancestors.
            break;
        }
        if !facts.waitable {
            return Err(GuardError("a live ancestor cannot be watched".into()));
        }
        if owners.len() == MAX_WATCHED_OWNERS {
            return Err(GuardError("ancestor chain continues past the owner watch limit".into()));
        }
        owners.push(facts.handle);
        if let Some(levels) = levels.as_mut().filter(|levels| levels.len() < MAX_CONSOLE_LEVELS) { levels.push(ConsoleOwner { id: parent, handle: facts.handle, boundary: stop, recheck_boundary: facts.image.is_empty() || windows.is_empty() }); }
        ids.push(parent);
        if stop {
            break;
        }
        child = parent;
        child_started = started;
    }
    if owners.is_empty() {
        return Err(GuardError("no live ancestor could be watched".into()));
    }
    Ok(owners)
}

pub(super) fn owner_parent_map() -> Result<ParentMap, GuardError> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE {
            return Err(os_error());
        }
        let mut map = ParentMap::new();
        let mut entry: ProcessEntry32W = std::mem::zeroed();
        entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            map.insert(entry.process_id, entry.parent_process_id);
            ok = Process32NextW(snapshot, &mut entry);
        }
        let error = GetLastError();
        CloseHandle(snapshot);
        // ERROR_NO_MORE_FILES is the successful end of the snapshot iteration.
        if error != 18 {
            return Err(GuardError(format!("cannot enumerate ancestors (Windows error {})", error)));
        }
        Ok(map)
    }
}

/// Open once with both rights, then verify on that held process object. A
/// query-only fallback can identify an excluded system boundary, but can never
/// silently remove a required live ancestor from the wait set.
pub(super) fn observed_owner(id: u32, kept: Option<Handle>, cleanup: &mut HandleList) -> Result<OwnerObservation, GuardError> {
    let (handle, waitable) = match kept {
        Some(handle) => (handle, true),
        None => unsafe {
            let handle = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, id);
            if !handle.is_null() {
                (handle, true)
            } else {
                let query = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, id);
                if query.is_null() {
                    // A process of another account that this user cannot
                    // open at all (the system service host above
                    // sihost.exe, the parent of packaged desktop apps) can
                    // still be the boundary of the walk: its path is read
                    // by identifier, with no handle. Only a boundary at
                    // its system path is reported, never waitable and with
                    // no creation time, so the walk ends there and cannot
                    // admit it; any other such process keeps the refusal.
                    let error = os_error();
                    let image = image_path_of_unopened(id);
                    if !ends_the_walk(crate::file_name_of(&image), &image, &windows_folder()) {
                        return Err(error);
                    }
                    return Ok(OwnerObservation { handle: std::ptr::null_mut(), started: None, image, waitable: false });
                }
                (query, false)
            }
        },
    };
    if kept.is_none() {
        cleanup.push(handle);
    }
    Ok(OwnerObservation { handle, started: started_at(handle), image: image_path_of_handle(handle), waitable })
}

/// Prefer the retained-handle parent chain to an O(system-process-count)
/// Toolhelp snapshot. Native link-query failure takes the existing verified
/// snapshot fallback; required identity/access failures remain fail-closed.
pub(super) fn direct_owner_chain(ours: u64, kept: &[Handle], cleanup: &mut HandleList, seed: Option<&ParentFacts>, levels: &mut Option<SmallList<ConsoleOwner, 16>>, windows: &mut Option<String>, process: Option<Handle>) -> Result<Option<HandleList>, GuardError> {
    let parent = match seed {
        Some(seed) => seed.id,
        None => match process {
            Some(handle) => parent_process_id_on(handle)?,
            None => parent_process_id()?,
        },
    };
    // The one Windows folder string of the run is built here and moved
    // into the caller's slot at once, so every return path below —
    // success, refusal and the snapshot fallback alike — leaves it
    // there for the fallback and for the console finish to reuse
    // instead of querying and converting the folder again.
    *windows = Some(windows_folder());
    let windows = windows.as_deref().unwrap_or("");
    direct_owner_chain_with(ours, parent, kept, seed, levels, windows, (
        |id, kept| observed_owner(id, kept, cleanup),
        |handle| unsafe { WaitForSingleObject(handle, 0) == 0 },
        |handle| {
            let mut basic: ProcessBasicInformation = unsafe { std::mem::zeroed() };
            if unsafe { NtQueryInformationProcess(handle, 0, &mut basic as *mut _ as *mut c_void,
                std::mem::size_of::<ProcessBasicInformation>() as u32, std::ptr::null_mut()) } != 0 {
                None
            } else {
                Some(basic.parent_process_id as u32)
            }
        },
    ))
}

/// The direct walk, with lazy native operations shared by the production caller and tests.
pub(super) fn direct_owner_chain_with<O, E, P>(
    ours: u64, mut parent: u32, kept: &[Handle], seed: Option<&ParentFacts>,
    levels: &mut Option<SmallList<ConsoleOwner, 16>>, windows: &str, operations: (O, E, P),
) -> Result<Option<HandleList>, GuardError>
where
    O: FnMut(u32, Option<Handle>) -> Result<OwnerObservation, GuardError>,
    E: FnMut(Handle) -> bool,
    P: FnMut(Handle) -> Option<u32>,
{
    let (mut observe, mut is_exited, mut parent_of) = operations;
    let mut child_started = ours;
    let mut ids: SmallList<u32, 16> = SmallList::new();
    let mut owners = HandleList::new();
    while parent != 0 && parent != 4 {
        if ids.contains(&parent) { return Err(GuardError("ancestor chain contains a cycle".into())); }
        let first = if owners.is_empty() { seed } else { None };
        let facts = match first {
            Some(seed) => OwnerObservation { handle: seed.handle, started: Some(seed.started), image: String::new(), waitable: true },
            None => observe(parent, if owners.is_empty() { kept.first().copied() } else { None })?,
        };
        let image = first.map_or(facts.image.as_str(), |seed| seed.image.as_str());
        let name = crate::file_name_of(image);
        let stop = ends_the_walk(name, image, windows);
        // A boundary above the direct parent ends the walk before its
        // creation time is required, as in the snapshot walk.
        if stop && !owners.is_empty() { break; }
        let started = facts.started.ok_or_else(|| GuardError("ancestor creation time cannot be verified".into()))?;
        if started > child_started { break; }
        if !facts.waitable { return Err(GuardError("a live ancestor cannot be watched".into())); }
        // Every admitted level spends the traversal budget, including exited ancestors.
        if ids.len() == MAX_WATCHED_OWNERS {
            return Err(GuardError("ancestor chain continues past the owner watch limit".into()));
        }
        // Only live ancestors are watched. Above the direct parent, an
        // ancestor that has already exited (the intermediate a POSIX
        // layer forks and leaves behind when it runs a program, held open
        // by its own parent) is passed through to its parent, which is
        // read from its record as for any other level; watching it would
        // end the run at once as if its owner had just exited. A direct
        // parent that has exited is still watched, and ends the run.
        let exited = !owners.is_empty() && is_exited(facts.handle);
        if !exited {
            owners.push(facts.handle);
            if let Some(levels) = levels.as_mut().filter(|levels| levels.len() < MAX_CONSOLE_LEVELS) { levels.push(ConsoleOwner { id: parent, handle: facts.handle, boundary: stop, recheck_boundary: image.is_empty() || windows.is_empty() }); }
        }
        ids.push(parent);
        if stop { break; }
        parent = match parent_of(facts.handle) {
            Some(parent) => parent,
            None => return Ok(None),
        };
        child_started = started;
    }
    if owners.is_empty() { return Err(GuardError("no live ancestor could be watched".into())); }
    Ok(Some(owners))
}

pub(super) fn watched_owner_handles(watch_ancestors: bool, kept: &[Handle], cleanup: &mut HandleList, seed: Option<&ParentFacts>, levels: &mut Option<SmallList<ConsoleOwner, 16>>, windows: &mut Option<String>) -> Result<HandleList, GuardError> {
    // The pseudo-handle behind this run's own creation-time read is
    // kept in `process` and handed to the parent query below (and,
    // on the watched route, to the direct chain's parent query):
    // one fetch serves every read of this process in this call.
    // A seeded call already holds its facts and fetches nothing here.
    let mut process: Option<Handle> = None;
    let ours = match seed {
        Some(seed) => seed.ours,
        None => {
            let handle = unsafe { GetCurrentProcess() };
            process = Some(handle);
            started_at(handle)
                .ok_or_else(|| GuardError("this process's creation time cannot be verified".into()))?
        }
    };
    if !watch_ancestors {
        let parent = match process {
            Some(handle) => parent_process_id_on(handle)?,
            None => parent_process_id()?,
        };
        let handle = match kept.first().copied() {
            Some(handle) => handle,
            None => {
                let handle = unsafe { OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, parent) };
                if handle.is_null() { return Err(os_error()); }
                cleanup.push(handle);
                handle
            }
        };
        if !matches!(started_at(handle), Some(started) if started <= ours) {
            return Err(GuardError("the parent cannot be verified and watched".into()));
        }
        // No image or whole-system process snapshot is needed for one parent.
        return Ok(HandleList::from_slice(&[handle]));
    }

    if let Some(owners) = direct_owner_chain(ours, kept, cleanup, seed, levels, windows, process)? { return Ok(owners); }
    // The fallback must describe its own verified chain, not a partial direct walk.
    if let Some(levels) = levels.as_mut() { *levels = SmallList::new(); }
    let map = owner_parent_map()?;
    let me = unsafe { GetCurrentProcessId() };
    let first = map.get(&me).copied();
    if seed.is_some_and(|seed| first != Some(seed.id)) {
        return Err(GuardError("the parent chain changed before it could be verified".into()));
    }
    // The direct chain has already placed the run's one Windows folder
    // string in the slot; the fallback reads it from there instead of
    // querying and converting the folder a second time.
    collect_owner_handles_with_levels(&map, me, ours, windows.as_deref().unwrap_or(""), |id| {
        let retained = if Some(id) == first { seed.map(|seed| seed.handle).or_else(|| kept.first().copied()) } else { None };
        observed_owner(id, retained, cleanup)
    }, levels)
}

/// Live ancestors of this process, nearest first, each with the file name
/// the snapshot gave for it: each one must have started
/// before its child (so a reused process ID is never mistaken for an ancestor),
/// and the authentication image search stops only at verified system paths.
/// Launch monitoring uses collect_owner_handles so its verified handles remain pinned.
pub(super) fn ancestor_ids() -> Vec<(u32, String)> {
    let windows = windows_folder();
    // One arena holds every name the snapshot gives, in the order the
    // snapshot gave them; the map holds, beside each process's parent,
    // the (offset, length) range of its name in that arena. The snapshot
    // pass therefore makes one growing allocation for all of its names
    // instead of one string per process, and the walk below reads a
    // name from its range and clones only the ancestors' names out.
    let mut arena = String::new();
    let mut map: std::collections::BTreeMap<u32, (u32, (usize, usize))> = std::collections::BTreeMap::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(SNAPPROCESS, 0);
        if snapshot != INVALID_HANDLE {
            let mut entry: ProcessEntry32W = std::mem::zeroed();
            entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
            let mut ok = Process32FirstW(snapshot, &mut entry);
            while ok != 0 {
                let len = entry.exe_file.iter().position(|&c| c == 0).unwrap_or(260);
                let offset = arena.len();
                for decoded in std::char::decode_utf16(entry.exe_file[..len].iter().copied()) {
                    match decoded {
                        Ok(character) => arena.push(character),
                        Err(_) => arena.push('\u{FFFD}'),
                    }
                }
                map.insert(entry.process_id, (entry.parent_process_id, (offset, arena.len() - offset)));
                ok = Process32NextW(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
        }
    }
    let mut out = Vec::new();
    let mut child = unsafe { GetCurrentProcessId() };
    // This process's own creation time is read from its pseudo-handle,
    // the one-call form the owner walk above already uses for the
    // same fact, not by opening this process by its identifier.
    let mut child_started = started_at(unsafe { GetCurrentProcess() }).unwrap_or(u64::MAX);
    for _ in 0..MAX_ANCESTOR_LEVELS {
        let parent = match map.get(&child) { Some((p, _)) => *p, None => break };
        let name = match map.get(&parent) { Some((_, (offset, length))) => &arena[*offset..*offset + *length], None => break };
        if parent == 0 || parent == 4 || out.iter().any(|(id, _)| *id == parent) {
            break;
        }
        // One open of the parent carries both facts this iteration
        // reads from it: its creation time, and — only when the
        // snapshot name is one the walk ends at — its image path for
        // the boundary test. The one right both former opens used
        // covers both queries, and the handle is closed once. A
        // failed open fails both facts, which breaks the walk
        // exactly as the former pair's failed creation-time open did
        // after its failed image open had already refused the
        // boundary test on an empty path.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, parent) };
        if handle.is_null() {
            break;
        }
        let started = started_at(handle);
        let boundary = WALK_ENDS_AT.iter().any(|end| end.eq_ignore_ascii_case(name))
            && ends_the_walk(name, &image_path_of_handle(handle), &windows);
        unsafe { CloseHandle(handle) };
        if boundary {
            break;
        }
        let started = match started { Some(t) => t, None => break };
        if started > child_started {
            break;
        }
        out.push((parent, name.to_owned()));
        child = parent;
        child_started = started;
    }
    if out.is_empty() {
        if let Ok(parent) = parent_process_id() {
            let name = map.get(&parent).map(|(_, (offset, length))| arena[*offset..*offset + *length].to_owned()).unwrap_or_default();
            out.push((parent, name));
        }
    }
    out
}

/// The path of the file of the process behind `handle` as the system gives it, letter case
/// included; empty when it cannot be read. The handle needs the right to query limited information.
pub(crate) fn image_path_of_handle(handle: Handle) -> String {
    let mut buffer = [std::mem::MaybeUninit::<u16>::uninit(); 1024];
    let mut size = buffer.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr() as *mut u16, &mut size) } == 0 {
        return String::new();
    }
    // On success the call sets `size` to the unit count it wrote; the
    // read is confined to that prefix.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u16, size as usize) })
}

/// The path of the file of process `id` as the kernel records it, read by
/// identifier with no handle, with the device of the Windows folder's drive
/// written back as that drive's letter; empty when it cannot be read or the
/// file is on another device. For a process that this user cannot open, this
/// is the only path there is to tell a boundary of the walk by.
pub(super) fn image_path_of_unopened(id: u32) -> String {
    let mut buffer = [0u16; 1024];
    let mut info = ProcessIdInformation {
        process_id: id as usize, length: 0, maximum_length: (buffer.len() * 2) as u16, buffer: buffer.as_mut_ptr(),
    };
    if unsafe { NtQuerySystemInformation(SYSTEM_PROCESS_ID_INFORMATION_CLASS, &mut info as *mut _ as *mut c_void,
        std::mem::size_of::<ProcessIdInformation>() as u32, std::ptr::null_mut()) } != 0 {
        return String::new();
    }
    let native = String::from_utf16_lossy(&buffer[..(info.length as usize / 2).min(buffer.len())]);
    // The native path names a device (`\Device\HarddiskVolume3\...`);
    // only the device behind the Windows folder's drive is mapped back.
    let windows = windows_folder();
    let Some(drive) = windows.get(..2).filter(|drive| drive.ends_with(':')) else { return String::new() };
    let wide: Vec<u16> = drive.encode_utf16().chain(Some(0)).collect();
    let mut target = [0u16; 512];
    if unsafe { QueryDosDeviceW(wide.as_ptr(), target.as_mut_ptr(), target.len() as u32) } == 0 {
        return String::new();
    }
    let end = target.iter().position(|&unit| unit == 0).unwrap_or(target.len());
    let device = String::from_utf16_lossy(&target[..end]);
    match native.get(..device.len()) {
        Some(head) if !device.is_empty() && head.eq_ignore_ascii_case(&device) && native[device.len()..].starts_with('\\') => {
            format!("{}{}", drive, &native[device.len()..])
        }
        _ => String::new(),
    }
}

pub(super) fn image_path_of_id(id: u32) -> String {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, id);
        if handle.is_null() {
            return String::new();
        }
        let path = image_path_of_handle(handle);
        CloseHandle(handle);
        path
    }
}

/// The path of the file of the process that launched the guard, as the system gives it;
/// empty when it cannot be read.
pub(crate) fn parent_image_path() -> String {
    parent_process_id().map_or_else(|_| String::new(), image_path_of_id)
}

/// Image file name of the process that launched the guard, lower case.
/// Retained for the unit tests, which pin this lower-case contract;
/// production callers use `parent_image_path` directly.
#[cfg(test)]
pub fn parent_image() -> String {
    // The lower case form this returns is its contract, so it is kept; the
    // lowering now happens in place on the path already read, not on a copy.
    let mut image = parent_image_path();
    image.make_ascii_lowercase();
    image
}

/// The file names of every live ancestor of this process, nearest first (see
/// `ancestor_ids`): the names the walk's snapshot already holds, so no ancestor
/// is queried again for its image; a name the snapshot did not hold is empty.
pub(super) fn ancestor_images() -> Vec<String> {
    ancestor_ids().into_iter().map(|(_, name)| name).collect()
}
