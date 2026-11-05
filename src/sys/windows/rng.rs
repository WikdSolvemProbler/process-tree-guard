// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The system random generator: `RtlGenRandom`, resolved from the system folder at first use and retained for the process.

use super::*;

/// RtlGenRandom returns a one-byte BOOLEAN, not a Win32 BOOL.
type RtlGenRandomFn = unsafe extern "system" fn(*mut u8, u32) -> u8;
const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RngLoadError {
    Provider(u32),
    Export(u32),
    Target(u32),
    NullTarget,
}

impl RngLoadError {
    fn guard_error(self) -> GuardError {
        let (stage, code) = match self {
            Self::Provider(code) => ("load the system random generator", code),
            Self::Export(code) => ("find the system random generator", code),
            Self::Target(code) => ("retain the system random generator module", code),
            Self::NullTarget => return GuardError("the system random generator module was not returned".into()),
        };
        GuardError(format!("could not {}: {}", stage, error_for(code)))
    }
}

/// Private resolver boundary. Native failures carry their immediately captured code;
/// cleanup never has to consult a possibly overwritten thread LastError.
trait RngLoader {
    fn provider(&self) -> Result<Handle, RngLoadError>;
    fn export(&self, provider: Handle) -> Result<*const c_void, RngLoadError>;
    fn target(&self, address: *const c_void) -> Result<Handle, RngLoadError>;
    fn release(&self, module: Handle) -> Result<(), u32>;
}

struct NativeRngLoader;

impl RngLoader for NativeRngLoader {
    fn provider(&self) -> Result<Handle, RngLoadError> {
        const ADVAPI: [u16; 13] = [97, 100, 118, 97, 112, 105, 51, 50, 46, 100, 108, 108, 0];
        let module = unsafe { LoadLibraryExW(ADVAPI.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
        if module.is_null() {
            let code = unsafe { GetLastError() };
            Err(RngLoadError::Provider(code))
        } else { Ok(module) }
    }

    fn export(&self, provider: Handle) -> Result<*const c_void, RngLoadError> {
        let address = unsafe { GetProcAddress(provider, c"SystemFunction036".as_ptr() as *const u8) };
        if address.is_null() {
            let code = unsafe { GetLastError() };
            Err(RngLoadError::Export(code))
        } else { Ok(address) }
    }

    fn target(&self, address: *const c_void) -> Result<Handle, RngLoadError> {
        let mut module = std::ptr::null_mut();
        let ok = unsafe { GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, address as *const u16, &mut module) };
        if ok == 0 {
            let code = unsafe { GetLastError() };
            Err(RngLoadError::Target(code))
        } else if module.is_null() {
            Err(RngLoadError::NullTarget)
        } else { Ok(module) }
    }

    fn release(&self, module: Handle) -> Result<(), u32> {
        if unsafe { FreeLibrary(module) } == 0 {
            let code = unsafe { GetLastError() };
            Err(code)
        } else { Ok(()) }
    }
}

/// Exactly one acquired module reference, even when another guard has the same
/// HMODULE value. Never used for an uncounted/borrowed module.
struct RngModule<'a, L: RngLoader> {
    module: Handle,
    loader: &'a L,
}

impl<L: RngLoader> Drop for RngModule<'_, L> {
    fn drop(&mut self) {
        // Best effort once. A failed release can leave this reference resident;
        // it must neither panic nor replace the captured primary resolver error.
        let _ = self.loader.release(self.module);
    }
}

#[derive(Clone, Copy, Debug)]
struct RngApi {
    fill: RtlGenRandomFn,
}

impl RngApi {
    fn fill_16(&self, bytes: &mut [u8; 16]) -> Result<(), GuardError> {
        if unsafe { (self.fill)(bytes.as_mut_ptr(), bytes.len() as u32) } == 0 {
            // RtlGenRandom FALSE has no documented extended-error receipt.
            Err(GuardError("the system random generator failed".into()))
        } else { Ok(()) }
    }
}

/// The target drops before the provider on a pre-publication unwind. Each guard
/// owns a distinct acquisition, including when both handles compare equal.
struct PendingRng<'a, L: RngLoader> {
    target: RngModule<'a, L>,
    provider: RngModule<'a, L>,
    fill: RtlGenRandomFn,
}

impl<L: RngLoader> PendingRng<'_, L> {
    fn retain_for_process(self) -> RngApi {
        let Self { target, provider, fill } = self;
        // Intentionally retain both counted references for every cached call,
        // across returning APIs and host-thread exits. No fallible work follows.
        std::mem::forget(target);
        std::mem::forget(provider);
        RngApi { fill }
    }
}

/// # Safety
/// A successful loader export must be callable with RtlGenRandomFn's exact ABI.
/// Provider/target successes must each own one nonnull counted reference and
/// release must not panic. The native loader and private test loaders meet this.
unsafe fn prepare_rng<L: RngLoader>(loader: &L) -> Result<PendingRng<'_, L>, RngLoadError> {
    let provider = RngModule { module: loader.provider()?, loader };
    let address = loader.export(provider.module)?;
    let target = RngModule { module: loader.target(address)?, loader };
    // Do not convert, publish or call the export before retaining its actual
    // containing module, which may differ from the named provider on forwarding.
    let fill = std::mem::transmute::<*const c_void, RtlGenRandomFn>(address);
    Ok(PendingRng { target, provider, fill })
}

fn cached_rng(cache: &std::sync::OnceLock<Result<RngApi, RngLoadError>>,
    initialize: impl FnOnce() -> Result<RngApi, RngLoadError>) -> Result<&RngApi, RngLoadError> {
    cache.get_or_init(initialize).as_ref().map_err(|error| *error)
}

/// Only Extension::start's channel name calls this. An ordinary fixed run neither
/// initializes this cache nor resolves the optional RNG export.
pub(crate) fn secure_random_16(bytes: &mut [u8; 16]) -> Result<(), GuardError> {
    static API: std::sync::OnceLock<Result<RngApi, RngLoadError>> = std::sync::OnceLock::new();
    let api = cached_rng(&API, || {
        let loader = NativeRngLoader;
        unsafe { prepare_rng(&loader) }.map(PendingRng::retain_for_process)
    }).map_err(RngLoadError::guard_error)?;
    api.fill_16(bytes)
}

#[cfg(test)]
mod rng_tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::sync::{Arc, Barrier, OnceLock};
    use std::sync::atomic::{AtomicUsize, Ordering};

    thread_local! {
        static FILL_CALLS: Cell<usize> = const { Cell::new(0) };
        static FILL_LENGTH: Cell<u32> = const { Cell::new(0) };
    }

    unsafe extern "system" fn fill_255(bytes: *mut u8, length: u32) -> u8 {
        FILL_CALLS.with(|calls| calls.set(calls.get() + 1));
        FILL_LENGTH.with(|size| size.set(length));
        for index in 0..length { bytes.add(index as usize).write(index as u8); }
        255
    }

    unsafe extern "system" fn fill_zero(_bytes: *mut u8, length: u32) -> u8 {
        FILL_CALLS.with(|calls| calls.set(calls.get() + 1));
        FILL_LENGTH.with(|size| size.set(length));
        0
    }

    #[derive(Clone, Copy)]
    enum Stop { Never, Provider, Export, Target, NullTarget }

    struct FakeLoader {
        stop: Stop,
        provider: usize,
        target: usize,
        events: RefCell<Vec<&'static str>>,
        releases: RefCell<Vec<usize>>,
        last_error: Cell<u32>,
        release_fails: bool,
    }

    impl FakeLoader {
        fn new(stop: Stop, same_module: bool, release_fails: bool) -> Self {
            Self { stop, provider: 0x1000, target: if same_module { 0x1000 } else { 0x2000 },
                events: RefCell::new(Vec::new()), releases: RefCell::new(Vec::new()),
                last_error: Cell::new(0), release_fails }
        }
    }

    impl RngLoader for FakeLoader {
        fn provider(&self) -> Result<Handle, RngLoadError> {
            self.events.borrow_mut().push("provider");
            if matches!(self.stop, Stop::Provider) {
                self.last_error.set(126);
                Err(RngLoadError::Provider(self.last_error.get()))
            } else { Ok(self.provider as Handle) }
        }

        fn export(&self, provider: Handle) -> Result<*const c_void, RngLoadError> {
            self.events.borrow_mut().push("export");
            // Report a mismatched provider as a typed failure outside any FFI.
            if provider as usize != self.provider { return Err(RngLoadError::Export(6)); }
            if matches!(self.stop, Stop::Export) {
                self.last_error.set(127);
                Err(RngLoadError::Export(self.last_error.get()))
            } else { Ok(fill_255 as *const () as *const c_void) }
        }

        fn target(&self, address: *const c_void) -> Result<Handle, RngLoadError> {
            self.events.borrow_mut().push("target");
            if address != fill_255 as *const () as *const c_void { return Err(RngLoadError::Target(87)); }
            if matches!(self.stop, Stop::Target) {
                self.last_error.set(487);
                Err(RngLoadError::Target(self.last_error.get()))
            } else if matches!(self.stop, Stop::NullTarget) {
                Err(RngLoadError::NullTarget)
            } else { Ok(self.target as Handle) }
        }

        fn release(&self, module: Handle) -> Result<(), u32> {
            self.events.borrow_mut().push("release");
            self.releases.borrow_mut().push(module as usize);
            self.last_error.set(999); // A release cannot replace the earlier code.
            if self.release_fails { Err(999) } else { Ok(()) }
        }
    }

    #[test]
    fn rng_loader_failures_keep_the_original_stage_code_and_release_only_owned_refs() {
        for release_fails in [false, true] {
            for (stop, expected, steps, released) in [
                (Stop::Provider, RngLoadError::Provider(126), vec!["provider"], vec![]),
                (Stop::Export, RngLoadError::Export(127), vec!["provider", "export", "release"], vec![0x1000]),
                (Stop::Target, RngLoadError::Target(487), vec!["provider", "export", "target", "release"], vec![0x1000]),
                (Stop::NullTarget, RngLoadError::NullTarget, vec!["provider", "export", "target", "release"], vec![0x1000]),
            ] {
                FILL_CALLS.with(|calls| calls.set(0));
                let loader = FakeLoader::new(stop, false, release_fails);
                let error = unsafe { prepare_rng(&loader) }.err().expect("a typed resolver failure");
                assert_eq!(error, expected);
                assert_eq!(*loader.events.borrow(), steps);
                assert_eq!(*loader.releases.borrow(), released);
                assert_eq!(FILL_CALLS.with(Cell::get), 0, "failure never calls the export");
                match expected {
                    RngLoadError::NullTarget => assert_eq!(error.guard_error().message(), "the system random generator module was not returned"),
                    RngLoadError::Provider(code) | RngLoadError::Export(code) | RngLoadError::Target(code) =>
                        assert!(error.guard_error().message().contains(&format!("[WinError {}]", code))),
                }
            }
        }
    }

    #[test]
    fn rng_prepublication_drop_releases_both_counted_acquisitions_even_for_equal_handles() {
        for same_module in [false, true] {
            let loader = FakeLoader::new(Stop::Never, same_module, false);
            FILL_CALLS.with(|calls| calls.set(0));
            let pending = unsafe { prepare_rng(&loader) }.expect("a valid prepared export");
            assert_eq!(*loader.events.borrow(), ["provider", "export", "target"]);
            assert!(loader.releases.borrow().is_empty());
            assert_eq!(FILL_CALLS.with(Cell::get), 0, "retaining the target is not an RNG call");
            drop(pending);
            assert_eq!(*loader.releases.borrow(), [loader.target, loader.provider]);
        }
    }

    #[test]
    fn rng_prepublication_unwind_attempts_both_releases_without_publishing_or_calling() {
        for same_module in [false, true] {
            let loader = FakeLoader::new(Stop::Never, same_module, true);
            FILL_CALLS.with(|calls| calls.set(0));
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _pending = unsafe { prepare_rng(&loader) }.expect("both counted references");
                panic!("deliberate unwind before RNG publication");
            }));
            assert!(unwind.is_err());
            assert_eq!(*loader.releases.borrow(), [loader.target, loader.provider]);
            assert_eq!(FILL_CALLS.with(Cell::get), 0);
        }
    }

    #[test]
    fn rng_committed_api_retains_both_refs_and_treats_boolean_255_as_success() {
        let loader = FakeLoader::new(Stop::Never, false, false);
        let api = unsafe { prepare_rng(&loader) }.expect("both references").retain_for_process();
        assert!(loader.releases.borrow().is_empty(), "commit intentionally retains both refs");
        FILL_CALLS.with(|calls| calls.set(0));
        let mut bytes = [0xa5; 16];
        api.fill_16(&mut bytes).expect("nonzero BOOLEAN is success");
        assert_eq!(bytes, std::array::from_fn(|index| index as u8));
        assert_eq!(FILL_LENGTH.with(Cell::get), 16);
        assert_eq!(FILL_CALLS.with(Cell::get), 1);
        assert!(loader.releases.borrow().is_empty());
    }

    #[test]
    fn rng_false_keeps_the_existing_error_and_cached_api_without_retry_or_fallback() {
        let cache = OnceLock::new();
        let initialized = AtomicUsize::new(0);
        FILL_CALLS.with(|calls| calls.set(0));
        for _ in 0..2 {
            let api = cached_rng(&cache, || {
                initialized.fetch_add(1, Ordering::SeqCst);
                Ok(RngApi { fill: fill_zero })
            }).expect("lookup succeeded");
            let mut bytes = [0xa5; 16];
            assert_eq!(api.fill_16(&mut bytes).unwrap_err().message(), "the system random generator failed");
            assert_eq!(bytes, [0xa5; 16]);
        }
        assert_eq!(initialized.load(Ordering::SeqCst), 1);
        assert_eq!(FILL_LENGTH.with(Cell::get), 16);
        assert_eq!(FILL_CALLS.with(Cell::get), 2, "one native call per request, no fallback");
    }

    #[test]
    fn rng_local_cache_is_lazy_and_preserves_the_first_loader_failure_without_retry() {
        let cache = OnceLock::new();
        let initialized = AtomicUsize::new(0);
        assert!(cache.get().is_none());
        assert_eq!(initialized.load(Ordering::SeqCst), 0);
        for _ in 0..3 {
            let failure = cached_rng(&cache, || {
                initialized.fetch_add(1, Ordering::SeqCst);
                Err(RngLoadError::Export(127))
            }).unwrap_err();
            assert_eq!(failure, RngLoadError::Export(127));
        }
        assert_eq!(initialized.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn rng_cache_initializes_once_across_concurrent_returning_threads_for_success_and_failure() {
        for succeeds in [false, true] {
            let cache = Arc::new(OnceLock::new());
            let initialized = Arc::new(AtomicUsize::new(0));
            let barrier = Arc::new(Barrier::new(8));
            let mut threads = Vec::new();
            for _ in 0..8 {
                let (cache, initialized, barrier) = (cache.clone(), initialized.clone(), barrier.clone());
                threads.push(std::thread::spawn(move || {
                    barrier.wait();
                    for _ in 0..3 {
                        let result = cached_rng(&cache, || {
                            initialized.fetch_add(1, Ordering::SeqCst);
                            if succeeds { Ok(RngApi { fill: fill_255 }) } else { Err(RngLoadError::Target(487)) }
                        });
                        if succeeds {
                            let mut bytes = [0xa5; 16];
                            result.expect("shared success").fill_16(&mut bytes).expect("same BOOLEAN API");
                            assert_eq!(bytes, std::array::from_fn(|index| index as u8));
                        } else { assert_eq!(result.unwrap_err(), RngLoadError::Target(487)); }
                    }
                }));
            }
            for thread in threads { thread.join().expect("returning host thread"); }
            assert_eq!(initialized.load(Ordering::SeqCst), 1);
            // The cached record remains valid after all initializing/calling threads exit.
            let result = cached_rng(&cache, || panic!("must not initialize again after thread exit"));
            if succeeds {
                let mut bytes = [0; 16];
                result.expect("process-scoped API").fill_16(&mut bytes).unwrap();
                assert_eq!(bytes[15], 15);
            } else { assert_eq!(result.unwrap_err(), RngLoadError::Target(487)); }
        }
    }
}
