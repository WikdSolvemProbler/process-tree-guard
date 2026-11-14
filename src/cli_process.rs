// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Process-owning CLI policy. No supervision engine lives
//! here. Returning APIs never enter this policy or modify their host's ACLs or
//! console attachment. ProcessOwnership test scopes do not activate it.

use crate::sys::windows::{

    AttachConsole, CloseHandle, CreateFileW, FlushConsoleInputBuffer, FreeConsole,

    GetConsoleProcessList, GetProcessId, GetSystemWindowsDirectoryW, Handle, INVALID_HANDLE,

    NtQueryInformationProcess, OPEN_EXISTING, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,

    ProcessBasicInformation, QueryFullProcessImageNameW,

};
use crate::{GuardError, Role, SmallList};
use std::ffi::c_void;

mod harden;
mod stdio;

use harden::*;
pub(crate) use harden::harden_worker;
pub(crate) use stdio::*;

thread_local! {
    static ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ARGUMENTS: std::mem::ManuallyDrop<std::cell::Cell<Option<Arguments>>> =
        const { std::mem::ManuallyDrop::new(std::cell::Cell::new(None)) };
}
/// One native argument iterator serves hardening dispatch and the whole CLI.
/// Returning entry calls create a fresh iterator and never cache environment.
pub(crate) struct Arguments {
    first: Option<std::ffi::OsString>,
    second: Option<std::ffi::OsString>,
    rest: std::iter::Skip<std::env::ArgsOs>,
}
impl Arguments {
    pub(crate) fn native() -> Self { Self { first: None, second: None, rest: std::env::args_os().skip(1) } }
}
impl Iterator for Arguments {
    type Item = std::ffi::OsString;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(first) = self.first.take() { return Some(first); }
        if let Some(second) = self.second.take() { return Some(second); }
        self.rest.next()
    }
}
pub(crate) fn take_arguments() -> Arguments {
    ARGUMENTS.with(|slot| slot.take()).unwrap_or_else(Arguments::native)
}
pub(crate) fn active() -> bool { ACTIVE.with(std::cell::Cell::get) }

/// Clear any unconsumed CLI iterator if an owning adapter unwinds on a
/// surviving worker. The ordinary CLI forgets this guard only at native exit.
/// Rc in PhantomData keeps the TLS cleanup guard on its creating thread.
pub(crate) struct CachedArgumentsUnwindGuard(std::marker::PhantomData<std::rc::Rc<()>>);
impl CachedArgumentsUnwindGuard {
    pub(crate) fn new() -> Self { Self(std::marker::PhantomData) }
}
impl Drop for CachedArgumentsUnwindGuard {
    fn drop(&mut self) {
        let cached = ARGUMENTS.with(|slot| slot.take());
        drop(cached);
    }
}
/// Returning APIs use live environment and library policy even when invoked
/// by a custom cli_exit entry. Restore the prior policy on every return/unwind;
/// nested returning calls cannot accidentally re-enable its one-shot cache.
fn returning_with<T>(active: &std::cell::Cell<bool>, body: impl FnOnce() -> T) -> T {
    struct Restore<'a> { active: &'a std::cell::Cell<bool>, previous: bool }
    impl Drop for Restore<'_> {
        fn drop(&mut self) { self.active.set(self.previous); }
    }
    let _restore = Restore { active, previous: active.replace(false) };
    body()
}

pub(crate) fn returning<T>(body: impl FnOnce() -> T) -> T {
    ACTIVE.with(|active| returning_with(active, body))
}

static REAL_CLI: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn is_gate(kind: Role) -> bool { kind == Role::Gate }
fn is_full(kind: Role) -> bool { kind == Role::Full }

fn hook_entry(kind: Role, first: Option<&std::ffi::OsStr>) -> bool {
    is_gate(kind) || (is_full(kind) && first == Some(std::ffi::OsStr::new("hook")))
}

fn context_vendor(name: Option<&std::ffi::OsStr>) -> bool {
    matches!(name, Some(name) if name == "cursor-context" || name == "antigravity-context")
}

/// Help and the context vendors decide nothing: no host acts on what they
/// return, so their answer outranks hardening and they dispatch unhardened.
/// Every decision, run, auto and extend invocation keeps the hardened path.
fn skip_hardening(kind: Role, first: Option<&std::ffi::OsStr>, second: Option<&std::ffi::OsStr>) -> bool {
    if matches!(first, Some(name) if name == "-h" || name == "--help") {
        return true;
    }
    if is_gate(kind) {
        return context_vendor(first);
    }
    is_full(kind)
        && first == Some(std::ffi::OsStr::new("hook"))
        && context_vendor(second)
}

/// Called only from the real diverging CLI adapter, before it starts workers.
/// The creator's existing process handle is unaffected by the new DACL.
pub(crate) fn enter(kind: Role) -> Result<(), (GuardError, bool)> {
    let mut rest = std::env::args_os().skip(1);
    let first = rest.next();
    let hook = hook_entry(kind, first.as_deref());
    // The vendor of a full-command hook dispatch is the second argument;
    // peek it only on that dispatch, where dispatch itself reads it next.
    let second = if is_full(kind) && first.as_deref() == Some(std::ffi::OsStr::new("hook")) {
        rest.next()
    } else {
        None
    };
    if !skip_hardening(kind, first.as_deref(), second.as_deref()) {
        harden_self(PROCESS_DENY_MASK | if hook { PROCESS_TERMINATE } else { 0 })
            .map_err(|error| (error, hook))?;
        REAL_CLI.store(true, std::sync::atomic::Ordering::Release);
    }
    ACTIVE.with(|active| active.set(true));
    // This invocation owns the process, so each standard handle's
    // console-ness is probed once, at its first use, and kept in that
    // handle's own slot: every later line write, standard-input choice
    // and receipt in this invocation reads the cached verdict of the
    // handle it uses instead of probing it again for each line, and a
    // handle the invocation never uses is never probed at all.
    let previous = ARGUMENTS.with(|slot| slot.replace(Some(Arguments { first, second, rest })));
    drop(previous);
    Ok(())
}

const CONIN_NAME: [u16; 7] = [67, 79, 78, 73, 78, 36, 0];
const CONOUT_NAME: [u16; 8] = [67, 79, 78, 79, 85, 84, 36, 0];

/// Read-only attachment probe: a pseudoconsole need not have a console window.
/// The kept device is non-inheritable; returning calls close it, while the
/// one-shot real CLI reclaims it at exit.
pub(crate) struct Console {
    pub(crate) attached: bool,
    input: Handle,
    exit_reclaim: bool,
}
impl Console {
    pub(crate) fn probe(exit_reclaim: bool) -> Self {
        unsafe {
            for (name, input) in [(&CONIN_NAME[..], true), (&CONOUT_NAME[..], false)] {
                let handle = CreateFileW(name.as_ptr(), 0xc000_0000, 3,
                    std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut());
                if !handle.is_null() && handle != INVALID_HANDLE {
                    if input { return Self { attached: true, input: handle, exit_reclaim }; }
                    if !exit_reclaim { CloseHandle(handle); }
                    return Self { attached: true, input: std::ptr::null_mut(), exit_reclaim };
                }
            }
        }
        Self { attached: false, input: std::ptr::null_mut(), exit_reclaim }
    }

    /// Best effort, after Job cleanup only, and only for a real CLI invocation.
    /// User type-ahead is deliberately discarded with child-injected input.
    pub(crate) fn finish(&self, first_owner: Option<Handle>, levels: Option<&[crate::engine::ConsoleOwner]>, windows: Option<&str>) {
        if !active() { return; }
        unsafe {
            // The Windows folder string of the whole call: the run's
            // own string when the run computed one (the ancestors
            // route hands it in), and otherwise — the parent-only
            // route, which never computed one — the one query this
            // call has always made. Every level below compares its
            // image against that one string in place instead of
            // re-querying the folder per ancestor. A failed query
            // yields the empty string on either route, and every
            // boundary test below fails on it, exactly as before.
            // The folder is held once in its wide units beside the
            // string — the queried buffer's own units, or the
            // handed-in string encoded once — and every level below
            // compares its reported image units against those units.
            // The units sit in stack storage: a folder name is
            // bounded by the 512-unit query buffer that produced
            // every string handed in here, so collecting them into
            // a heap vector copied a bounded buffer per finish for
            // storage the caller's frame already holds. A handed-in
            // string past that bound — no route in this tree builds
            // one — takes the heap form, encoded as it was before.
            let folder_units = match windows {
                Some(windows) => FolderUnits::encoded(windows),
                None => windows_directory(),
            };
            let windows_wide: &[u16] = folder_units.as_slice();
            // Inline storage of 64 identifiers, spilling only past that:
            // an ordinary finish allocates nothing for this list.
            let mut flushed = FlushedConsoles::new();
            if self.attached {
                if !self.input.is_null() { FlushConsoleInputBuffer(self.input); }
                else { flush_attached(); }
                note_console_members(&mut flushed);
                FreeConsole();
            }
            if let Some(levels) = levels {
                flush_verified_owners(levels, |handle| boundary(handle, windows_wide), |pid| {
                    if !flushed.can_skip(pid) && AttachConsole(pid) != 0 {
                        flush_attached();
                        note_console_members(&mut flushed);
                        FreeConsole();
                    }
                });
                return;
            }
            let Some(first) = first_owner else { return };
            let mut current = first;
            let mut owned = false;
            let mut current_started = None;
            // The finite ancestry-flush bound: 64 levels. Every newly opened
            // parent must predate its retained child; no PID-only replacement.
            for _ in 0..crate::engine::MAX_CONSOLE_LEVELS {
                if boundary(current, windows_wide) { break; }
                let pid = GetProcessId(current);
                if pid == 0 || pid == 4 { break; }
                if !flushed.can_skip(pid) && AttachConsole(pid) != 0 {
                    flush_attached();
                    note_console_members(&mut flushed);
                    FreeConsole();
                }
                let mut basic: ProcessBasicInformation = std::mem::zeroed();
                let next = if NtQueryInformationProcess(current, 0, &mut basic as *mut _ as *mut c_void,
                    std::mem::size_of::<ProcessBasicInformation>() as u32, std::ptr::null_mut()) == 0 {
                    basic.parent_process_id as u32
                } else { 0 };
                let parent = if next != 0 && next != 4 && next != pid {
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, next)
                } else { std::ptr::null_mut() };
                if parent.is_null() {
                    if owned { CloseHandle(current); }
                    return;
                }
                let verified = advance_creation_time(&mut current_started, current, parent, crate::engine::started_at);
                if owned { CloseHandle(current); }
                if !verified {
                    CloseHandle(parent);
                    return;
                }
                current = parent;
                owned = true;
            }
            if owned { CloseHandle(current); }
        }
    }
}
impl Drop for Console {
    fn drop(&mut self) { if !self.exit_reclaim && !self.input.is_null() { unsafe { CloseHandle(self.input) }; } }
}
/// Keep the creation time from the freshly opened retained parent for its
/// next loop iteration. Both original creation-time checks and ordering remain.
fn advance_creation_time(
    current_started: &mut Option<u64>, current: Handle, parent: Handle,
    mut read: impl FnMut(Handle) -> Option<u64>,
) -> bool {
    let child = (*current_started).or_else(|| read(current));
    let next = read(parent);
    if matches!((next, child), (Some(p), Some(c)) if p <= c) {
        *current_started = next;
        true
    } else {
        false
    }
}

/// Reuse only the current run's held ancestry; no image/PBI re-query or
/// PID reopen for levels already verified during AUTO monitoring.
fn flush_verified_owners(
    levels: &[crate::engine::ConsoleOwner],
    mut retry_boundary: impl FnMut(Handle) -> bool,
    mut flush: impl FnMut(u32),
) {
    for level in levels.iter().take(crate::engine::MAX_CONSOLE_LEVELS) {
        if level.boundary || (level.recheck_boundary && retry_boundary(level.handle)) { break; }
        flush(level.id);
    }
}

/// A complete census permits duplicate suppression; any uncertain census
/// disables it for this entire finish, even after a later complete query.
struct FlushedConsoles {
    members: SmallList<u32, 64>,
    reliable: bool,
}
impl FlushedConsoles {
    fn new() -> Self { Self { members: SmallList::new(), reliable: true } }
    fn can_skip(&self, pid: u32) -> bool { self.reliable && self.members.contains(&pid) }

    /// # Safety
    /// When count is in 1..=list.len(), that prefix must be initialized by the
    /// query. Invalid counts require no initialized entries and read none.
    unsafe fn record_query(&mut self, count: usize, list: &[std::mem::MaybeUninit<u32>]) {
        if count == 0 || count > list.len() {
            self.reliable = false;
            return;
        }
        // The native query initialized exactly this returned prefix. No slice
        // is formed on its failure or overflow paths above.
        for &pid in unsafe { std::slice::from_raw_parts(list.as_ptr() as *const u32, count) } {
            if !self.members.contains(&pid) { self.members.push(pid); }
        }
    }
}

fn note_console_members(members: &mut FlushedConsoles) {
    let mut list = [std::mem::MaybeUninit::<u32>::uninit(); 1024];
    let count = unsafe { GetConsoleProcessList(list.as_mut_ptr() as *mut u32, list.len() as u32) } as usize;
    // The API initializes the returned prefix only when the count fits.
    unsafe { members.record_query(count, &list) };
}
fn flush_attached() {
    let handle = unsafe { CreateFileW(CONIN_NAME.as_ptr(), 0xc000_0000, 3,
        std::ptr::null_mut(), OPEN_EXISTING, 0, std::ptr::null_mut()) };
    if !handle.is_null() && handle != INVALID_HANDLE {
        unsafe { FlushConsoleInputBuffer(handle); CloseHandle(handle); }
    }
}
/// The folder's wide units held once for a `Console::finish` call:
/// stack storage carrying exactly the units, or — only for a
/// handed-in string longer than the stack form holds — a heap
/// vector carrying them. The one slice form below is what every
/// boundary test reads, so the two forms cannot differ in a test.
#[allow(clippy::large_enum_variant)]
enum FolderUnits {
    Stack { storage: [std::mem::MaybeUninit<u16>; 512], len: usize },
    Heap(Vec<u16>),
}
impl FolderUnits {
    fn as_slice(&self) -> &[u16] {
        match self {
            // The writers below filled exactly the first `len`
            // units of the storage the read is confined to.
            FolderUnits::Stack { storage, len } => unsafe {
                std::slice::from_raw_parts(storage.as_ptr() as *const u16, *len)
            },
            FolderUnits::Heap(units) => units,
        }
    }
    /// The handed-in string's units, encoded once: into the stack
    /// storage while they fit it, and into the heap form — the
    /// one collecting encode this call always paid — only for a
    /// string past the stack form's bound.
    fn encoded(text: &str) -> FolderUnits {
        let mut storage = [std::mem::MaybeUninit::<u16>::uninit(); 512];
        let mut len = 0usize;
        for unit in text.encode_utf16() {
            if len == storage.len() {
                return FolderUnits::Heap(text.encode_utf16().collect());
            }
            storage[len].write(unit);
            len += 1;
        }
        FolderUnits::Stack { storage, len }
    }
}

/// The Windows folder as the system gives it, from the one query a
/// `Console::finish` call makes when the run handed it no string of
/// its own, in its reported wide units only, held in the query's own
/// buffer. Empty when it cannot be read, which makes every boundary
/// test below fail, exactly as a failed query inside the test itself
/// did. The lossy string form is not built here: the boundary test
/// compares in wide units, and the one branch that needs the string
/// (an unpaired surrogate in either input) builds it there, from
/// these same units, so an ordinary finish pays no conversion or
/// allocation for a fallback it does not take.
fn windows_directory() -> FolderUnits {
    let mut storage = [std::mem::MaybeUninit::<u16>::uninit(); 512];
    let reported = unsafe { GetSystemWindowsDirectoryW(storage.as_mut_ptr() as *mut u16, storage.len() as u32) } as usize;
    let len = if reported == 0 || reported >= storage.len() { 0 } else { reported };
    FolderUnits::Stack { storage, len }
}

/// Whether the process behind `handle` is one of the six system
/// boundary programs at its verified system path: the engine's
/// in-place test, run on the image's reported wide units against the
/// folder's units, so an ordinary level forms no string at all.
fn boundary(handle: Handle, windows_wide: &[u16]) -> bool {
    // The same uninitialised buffer and the same one query
    // `image_path_of_handle` makes: the call writes the prefix whose
    // length it reports through `size`, and every read below is
    // confined to that prefix. A failed query reported no image, and
    // the string form's test fails on an empty image, as this does.
    let mut buffer = [std::mem::MaybeUninit::<u16>::uninit(); 1024];
    let mut size = buffer.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr() as *mut u16, &mut size) } == 0 {
        return false;
    }
    let image = unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u16, size as usize) };
    // Guard: if any unit of the reported image or of the folder is an
    // unpaired surrogate, this level takes the string form below.
    // Lossy conversion maps each unpaired surrogate to U+FFFD, which
    // can make different reported units compare equal as strings; the
    // wide comparison would keep them apart and could change the
    // verdict. With no unpaired surrogate in either input, valid
    // pairs and all other units compare identically under both
    // forms, so the wide verdict is the string form's verdict.
    if !crate::engine::has_unpaired_surrogate(image) && !crate::engine::has_unpaired_surrogate(windows_wide) {
        let name_start = image.iter().rposition(|&unit| unit == b'\\' as u16 || unit == b'/' as u16).map_or(0, |at| at + 1);
        return crate::engine::ends_the_walk_wide(&image[name_start..], image, windows_wide);
    }
    let image = String::from_utf16_lossy(image);
    let name = crate::file_name_of(&image);
    // The folder's string form is built only on this fallback
    // branch, from the same reported units the wide form compared:
    // its lossy text is exactly the string the finish used to build
    // for every call, so the verdict is unchanged.
    let windows = String::from_utf16_lossy(windows_wide);
    crate::engine::ends_the_walk(name, &image, &windows)
}

#[cfg(test)]
mod tests {
    #[test]
    fn console_creation_time_carry_reads_each_retained_object_once() {
        let mut cached = None;
        let reads = std::cell::RefCell::new(Vec::new());
        let mut read = |handle: Handle| {
            reads.borrow_mut().push(handle as usize);
            Some(match handle as usize { 11 => 30, 12 => 20, 13 => 10, _ => 99 })
        };
        assert!(advance_creation_time(&mut cached, 11usize as Handle, 12usize as Handle, &mut read));
        assert!(advance_creation_time(&mut cached, 12usize as Handle, 13usize as Handle, &mut read));
        assert_eq!(*reads.borrow(), [11, 12, 13]);
        assert_eq!(cached, Some(10));
        assert!(!advance_creation_time(&mut cached, 13usize as Handle, 14usize as Handle, &mut read));
        assert_eq!(*reads.borrow(), [11, 12, 13, 14]);
        assert_eq!(cached, Some(10), "a rejected parent never becomes the cached child");
        assert!(!advance_creation_time(&mut None, 11usize as Handle, 12usize as Handle, |_| None));
    }

    use super::*;


    use crate::sys::windows::{

        CreateJobObjectW, DACL_SECURITY_INFORMATION, SetSecurityInfo, WRITE_DAC,

    };
    #[test]
    fn cli_arguments_slot_uses_no_drop_tls_and_moves_only_once() {
        type Payload = std::cell::Cell<Option<Arguments>>;
        assert!(std::mem::needs_drop::<Payload>());
        assert!(!std::mem::needs_drop::<std::mem::ManuallyDrop<Payload>>());
        std::thread::spawn(|| {
            assert!(ARGUMENTS.with(|slot| slot.take()).is_none());
            let prior = ARGUMENTS.with(|slot| slot.replace(Some(Arguments {
                first: Some(std::ffi::OsString::from("cached first")),
                second: None,
                rest: std::env::args_os().skip(1),
            })));
            assert!(prior.is_none());
            let mut moved = take_arguments();
            assert_eq!(moved.next(), Some(std::ffi::OsString::from("cached first")));
            drop(moved);
            assert!(ARGUMENTS.with(|slot| slot.take()).is_none());
            let fresh = take_arguments();
            drop(fresh);
            assert!(ARGUMENTS.with(|slot| slot.take()).is_none());
        }).join().unwrap();
    }

    #[test]
    fn cell_cache_replacement_and_take_drop_owned_values_normally() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        struct Probe(Arc<AtomicUsize>);
        impl Drop for Probe { fn drop(&mut self) { self.0.fetch_add(1, Ordering::SeqCst); } }
        let drops = Arc::new(AtomicUsize::new(0));
        let slot = std::mem::ManuallyDrop::new(std::cell::Cell::new(Some(Probe(drops.clone()))));
        let replaced = slot.replace(Some(Probe(drops.clone())));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(replaced);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        let moved = slot.take();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        drop(moved);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn unwind_guard_clears_cached_iterator_on_a_surviving_worker() {
        std::thread::spawn(|| {
            assert!(!active());
            assert!(ARGUMENTS.with(|slot| slot.take()).is_none());
            let caught = std::panic::catch_unwind(|| {
                let _guard = CachedArgumentsUnwindGuard::new();
                let prior = ARGUMENTS.with(|slot| slot.replace(Some(Arguments {
                    first: Some(std::ffi::OsString::from("unconsumed")),
                    second: None,
                    rest: std::env::args_os().skip(1),
                })));
                drop(prior);
                panic!("simulate custom entry unwind without calling cli_exit or hardening");
            });
            assert!(caught.is_err());
            assert!(ARGUMENTS.with(|slot| slot.take()).is_none());
            assert!(!active(), "the test did not enter owning CLI policy");
        }).join().unwrap();
    }

    #[test]
    fn verified_console_levels_skip_requery_and_retry_only_the_original_unknown_object() {
        use crate::engine::ConsoleOwner;
        let levels = [
            ConsoleOwner { id: 11, handle: 111usize as Handle, boundary: false, recheck_boundary: false },
            ConsoleOwner { id: 12, handle: 112usize as Handle, boundary: false, recheck_boundary: true },
            ConsoleOwner { id: 13, handle: 113usize as Handle, boundary: true, recheck_boundary: false },
            ConsoleOwner { id: 14, handle: 114usize as Handle, boundary: false, recheck_boundary: false },
        ];
        let mut queried = Vec::new();
        let mut flushed = Vec::new();
        flush_verified_owners(&levels, |handle| { queried.push(handle); false }, |pid| flushed.push(pid));
        assert_eq!(queried, vec![112usize as Handle]);
        assert_eq!(flushed, vec![11, 12]);
        queried.clear(); flushed.clear();
        flush_verified_owners(&levels, |handle| { queried.push(handle); true }, |pid| flushed.push(pid));
        assert_eq!(queried, vec![112usize as Handle]);
        assert_eq!(flushed, vec![11]);
        // Valid censuses still suppress duplicates. A failed or overflowing
        // census must invalidate earlier PID membership for the whole finish;
        // a later successful query cannot restore that lost authority.
        let dedupe_levels: Vec<_> = (11..=13).map(|id| ConsoleOwner {
            id, handle: std::ptr::null_mut(), boundary: false, recheck_boundary: false,
        }).collect();
        let initial = [std::mem::MaybeUninit::new(12)];
        let later = [std::mem::MaybeUninit::new(13)];
        let mut known = FlushedConsoles::new();
        unsafe { known.record_query(1, &initial) };
        let mut attempts = Vec::new();
        flush_verified_owners(&dedupe_levels, |_| panic!("no boundary requery"), |pid| {
            if !known.can_skip(pid) {
                attempts.push(pid);
                unsafe { known.record_query(1, &later) };
            }
        });
        assert_eq!(attempts, vec![11], "complete censuses preserve duplicate suppression");
        for invalid_count in [0, 1025] {
            let unread = [std::mem::MaybeUninit::<u32>::uninit(); 1024];
            let mut known = FlushedConsoles::new();
            unsafe { known.record_query(1, &initial) };
            attempts.clear();
            flush_verified_owners(&dedupe_levels, |_| panic!("no boundary requery"), |pid| {
                if !known.can_skip(pid) {
                    attempts.push(pid);
                    if pid == 11 { unsafe { known.record_query(invalid_count, &unread) }; }
                    if pid == 12 { unsafe { known.record_query(1, &later) }; }
                }
            });
            assert_eq!(attempts, vec![11, 12, 13],
                "count {invalid_count} invalidates old membership and later success cannot heal it");
        }
        let many: Vec<_> = (0..100).map(|id| ConsoleOwner { id, handle: std::ptr::null_mut(), boundary: false, recheck_boundary: false }).collect();
        flushed.clear();
        flush_verified_owners(&many, |_| panic!("verified levels never need a native boundary query"), |pid| flushed.push(pid));
        assert_eq!(flushed, (0..64).collect::<Vec<_>>(), "the console scope retains its original finite bound");
    }

    #[test]
    fn returning_scope_reads_live_trace_after_either_cli_cache_value() {
        use std::cell::Cell;
        use std::sync::OnceLock;
        // Model a real CLI policy using a local cell: no process hardening,
        // global environment writes, global cache resets or spawned payloads.
        for initial in [false, true] {
            let active = Cell::new(true);
            let live = Cell::new(initial);
            let cache = OnceLock::new();
            assert_eq!(crate::engine::trace_enabled_with(active.get(), &cache, || live.get()), initial);
            live.set(!initial);
            let result: Result<(), ()> = returning_with(&active, || {
                assert!(!active.get());
                assert_eq!(crate::engine::trace_enabled_with(active.get(), &cache, || live.get()), !initial,
                    "a returning call must not inherit the warmed CLI value");
                returning_with(&active, || {
                    live.set(initial);
                    assert_eq!(crate::engine::trace_enabled_with(active.get(), &cache, || live.get()), initial);
                });
                assert!(!active.get(), "a nested call must not restore the outer CLI policy early");
                live.set(!initial);
                assert_eq!(crate::engine::trace_enabled_with(active.get(), &cache, || live.get()), !initial);
                Err(())
            });
            assert_eq!(result, Err(()));
            assert!(active.get(), "an error return restores the actual caller policy");
            assert_eq!(crate::engine::trace_enabled_with(active.get(), &cache, || live.get()), initial,
                "the CLI cache itself is unchanged");
        }
    }

    #[test]
    fn simulated_process_ownership_does_not_activate_process_policy() {
        assert!(!active());
        let _owner = crate::diagnostics::ProcessOwnership::begin();
        assert!(!active(), "only the real cli_exit entry changes the CLI policy");
    }
    #[test]
    fn hook_role_selection_preserves_explicit_hook_dispatch() {
        assert!(hook_entry(Role::Gate, None));
        assert!(hook_entry(Role::Full, Some(std::ffi::OsStr::new("hook"))));
        assert!(!hook_entry(Role::Deadline, None));
        assert!(!hook_entry(Role::Full, Some(std::ffi::OsStr::new("auto"))));
    }
    #[test]
    fn acl_stack_and_heap_are_accepted_by_windows() {
        use crate::sys::windows::test_api::IsValidAcl;
        struct TestJob(Handle);
        impl Drop for TestJob {
            fn drop(&mut self) { unsafe { CloseHandle(self.0); } }
        }
        // One ACE fits the stack; 51 ACEs exceed 1024 bytes and force heap
        // storage both here and when deny_acl prepends its three ACEs.
        for count in [1usize, 51] {
            let total = 8 + 20 * count;
            let mut original = AclOut::new(total);
            let bytes = original.bytes_mut(total);
            bytes.fill(std::mem::MaybeUninit::new(0));
            // The fill wrote every byte, so the storage can be viewed as bytes.
            let bytes = unsafe { &mut *(bytes as *mut [std::mem::MaybeUninit<u8>] as *mut [u8]) };
            bytes[0] = ACL_REVISION as u8;
            bytes[2..4].copy_from_slice(&(total as u16).to_le_bytes());
            bytes[4..6].copy_from_slice(&(count as u16).to_le_bytes());
            for n in 0..count {
                let at = 8 + 20 * n;
                bytes[at + 2..at + 4].copy_from_slice(&20u16.to_le_bytes());
                bytes[at + 4..at + 8].copy_from_slice(&GENERIC_ALL.to_le_bytes());
                bytes[at + 8..at + 20].copy_from_slice(&EVERYONE_SID);
            }
            assert_ne!(unsafe { IsValidAcl(original.as_mut_ptr()) }, 0);
            let job = TestJob(unsafe { CreateJobObjectW(std::ptr::null_mut(), std::ptr::null()) });
            assert!(!job.0.is_null());
            let err = unsafe { SetSecurityInfo(job.0, SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(), std::ptr::null_mut(), original.as_mut_ptr(), std::ptr::null_mut()) };
            assert_eq!(err, 0, "install fixture ACL with {count} ACEs");
            let mut hardened = deny_acl(job.0, WRITE_DAC)
                .unwrap_or_else(|e| panic!("build hardened ACL: {}", e.0));
            assert_eq!(matches!(&hardened, AclOut::Stack(_)), count == 1);
            assert_eq!((hardened.as_mut_ptr() as usize) % 4, 0);
            assert_ne!(unsafe { IsValidAcl(hardened.as_mut_ptr()) }, 0);
            let err = unsafe { SetSecurityInfo(job.0, SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(), std::ptr::null_mut(), hardened.as_mut_ptr(), std::ptr::null_mut()) };
            assert_eq!(err, 0, "apply hardened ACL with {count} preserved ACEs");
        }
    }

}
