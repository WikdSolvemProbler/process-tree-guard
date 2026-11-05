// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Secrets held by the approval: wiping the compiler may not drop, and the registry of where a live secret is.

use super::*;


use crate::sys::windows::{

    CREDUI_DOMAIN_CHARS, CREDUI_PASSWORD_CHARS, CREDUI_USER_CHARS, CloseHandle, CoTaskMemFreeFn,

    Handle,

};
// ------------------------------------------------------------------ secrets

/// Zero `buffer` with writes that the compiler may not drop or reorder past what follows.
pub(super) fn wipe<T: Copy + Default>(buffer: &mut [T]) {
    for unit in buffer.iter_mut() {
        // SAFETY: the pointer comes from a unique reference to an initialised element.
        unsafe { std::ptr::write_volatile(unit as *mut T, T::default()) };
    }
    compiler_fence(Ordering::SeqCst);
}

#[cfg(test)]
thread_local! {
    /// For each secret dropped on this thread: how many of its units were not zero before the
    /// wipe and after it, so that a test can see that a drop wipes on the path it takes.
    pub(super) static WIPES: std::cell::RefCell<Vec<(usize, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Where a live secret is: its address, its size in bytes and the thread that made it.
pub(super) type Placed = (usize, usize, std::thread::ThreadId);

/// The buffers that hold a secret right now. A destructor does not run when the release build
/// aborts on a panic, so the panic hook zeroes these (see `install_panic_hook`). A buffer is taken
/// out of this list before it is freed, so the hook never writes to memory that has gone.
pub(super) fn live() -> std::sync::MutexGuard<'static, Vec<Placed>> {
    static LIVE: std::sync::Mutex<Vec<Placed>> = std::sync::Mutex::new(Vec::new());
    LIVE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(super) fn register(pointer: *const u8, bytes: usize) {
    // A buffer is listed only once the hook that zeroes the list on a panic is in place, so that no path can hold a secret that
    // the abort of a release build leaves in the dump of the process (Common Weakness Enumeration, CWE-316, cleartext storage of
    // sensitive information in memory); the hook is installed once.
    install_panic_hook();
    live().push((pointer as usize, bytes, std::thread::current().id()));
}

pub(super) fn unregister(pointer: *const u8) {
    live().retain(|placed| placed.0 != pointer as usize);
}

/// The thread whose buffers the hook zeroes alone: in the unit tests the one a test has named,
/// and in a program none, so that the hook zeroes every buffer.
#[cfg(test)]
fn only_this_thread() -> Option<std::thread::ThreadId> {
    HOOK_FOR_THIS_THREAD.load(Ordering::SeqCst).then(|| std::thread::current().id())
}

#[cfg(not(test))]
fn only_this_thread() -> Option<std::thread::ThreadId> {
    None
}

/// Zero every buffer that holds a secret (those of one thread when `only` names it).
pub(super) fn wipe_live(only: Option<std::thread::ThreadId>) {
    for (address, bytes, thread) in live().iter() {
        if only.is_none_or(|id| id == *thread) {
            for offset in 0..*bytes {
                // SAFETY: the memory is still allocated, because every owner takes its buffer out
                // of the list before it frees it and the lock of the list is held here. No
                // reference to it is formed, since another owner may hold one or be writing.
                unsafe { std::ptr::write_volatile((*address as *mut u8).add(offset), 0) };
            }
        }
    }
    compiler_fence(Ordering::SeqCst);
}

/// Make a panic zero the secrets before anything else happens. The release profile aborts on a
/// panic and runs no destructor, so without this the password would stay in the dump that the
/// system writes for the process. The hook runs before the abort, then the previous hook. It is
/// installed by `register`, which every buffer that holds a secret passes through, and also by `decide`.
pub(super) fn install_panic_hook() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            wipe_live(only_this_thread());
            previous(info);
        }));
    });
}

/// A buffer that is zeroed when it goes away, on every path.
pub(super) struct Secret<B: AsMut<[u16]>>(pub(super) B);

impl<B: AsMut<[u16]>> Drop for Secret<B> {
    fn drop(&mut self) {
        unregister(self.0.as_mut().as_ptr() as *const u8);
        #[cfg(test)]
        let before = self.0.as_mut().iter().filter(|unit| **unit != 0).count();
        wipe(self.0.as_mut());
        #[cfg(test)]
        {
            let after = self.0.as_mut().iter().filter(|unit| **unit != 0).count();
            let _ = WIPES.try_with(|wipes| wipes.borrow_mut().push((before, after)));
        }
    }
}

pub(super) fn zeroed(chars: usize) -> Secret<Box<[u16]>> {
    let secret = Secret(vec![0u16; chars].into_boxed_slice());
    register(secret.0.as_ptr() as *const u8, chars * std::mem::size_of::<u16>());
    secret
}

/// What the sign-in prompt returns. The buffers are boxed, so a move copies a pointer and not
/// the secret, and the system call fills them where they stand. They are never grown: a name or
/// password that does not fit is a failure.
pub(crate) struct Credentials {
    pub(super) user: Secret<Box<[u16]>>,
    pub(super) domain: Secret<Box<[u16]>>,
    pub(super) password: Secret<Box<[u16]>>,
}

pub(super) fn text_before_nul(units: &[u16]) -> Option<String> {
    let end = units.iter().position(|unit| *unit == 0)?;
    Some(String::from_utf16_lossy(&units[..end]))
}

impl Credentials {
    pub(super) fn new() -> Credentials {
        Credentials { user: zeroed(CREDUI_USER_CHARS), domain: zeroed(CREDUI_DOMAIN_CHARS), password: zeroed(CREDUI_PASSWORD_CHARS) }
    }

    /// The user name and the domain, which are not secret; None when one has no end.
    pub(super) fn names(&self) -> Option<(String, String)> {
        Some((text_before_nul(&self.user.0)?, text_before_nul(&self.domain.0)?))
    }

    /// True when the password has its end inside the buffer.
    pub(super) fn password_ends(&self) -> bool {
        self.password.0.contains(&0)
    }

    /// True when the password is empty. An empty password is never a secret: an account that has
    /// none could be approved by pressing Enter, and so by whoever drives the console.
    pub(super) fn password_is_empty(&self) -> bool {
        self.password.0.first() == Some(&0)
    }

    pub(super) fn password_pointer(&self) -> *const u16 {
        self.password.0.as_ptr()
    }

    /// Credentials for the tests of the attempt loop, with a password that is typed here.
    #[cfg(test)]
    pub(super) fn with(user: &str, domain: &str, password: &str) -> Credentials {
        let mut credentials = Credentials::new();
        for (buffer, text) in [(&mut credentials.user, user), (&mut credentials.domain, domain), (&mut credentials.password, password)] {
            for (slot, unit) in buffer.0.iter_mut().zip(text.encode_utf16()) {
                *slot = unit;
            }
        }
        credentials
    }

    /// The password units up to the end, for the stub that checks them.
    #[cfg(test)]
    pub(super) fn password_units(&self) -> Vec<u16> {
        self.password.0.iter().copied().take_while(|unit| *unit != 0).collect()
    }
}

/// The authentication buffer that the prompt returns: zeroed over its whole size, then freed
/// with the function that the system gave it by (`CoTaskMemFree`).
pub(super) struct OutBuffer {
    pub(super) pointer: *mut c_void,
    pub(super) size: u32,
    pub(super) free: CoTaskMemFreeFn,
}

impl Drop for OutBuffer {
    fn drop(&mut self) {
        unregister(self.pointer as *const u8);
        if !self.pointer.is_null() && self.size > 0 {
            // SAFETY: the system allocated `size` bytes at `pointer` and nothing else holds them.
            wipe(unsafe { std::slice::from_raw_parts_mut(self.pointer as *mut u8, self.size as usize) });
        }
        if !self.pointer.is_null() {
            unsafe { (self.free)(self.pointer) };
        }
    }
}

/// A handle that is closed when it goes away.
pub(super) struct Token(pub(super) Handle);

impl Drop for Token {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}
