// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The desktop approval prompt: presence on an interactive desktop, and the credential dialog on the secure desktop.

use super::*;


use crate::sys::windows::{

    CREDUIWIN_ENUMERATE_CURRENT_USER, CREDUIWIN_SECURE_PROMPT, CREDUI_DOMAIN_CHARS,

    CREDUI_MAX_MESSAGE_LENGTH, CREDUI_PASSWORD_CHARS, CREDUI_USER_CHARS, CRED_UNPROTECTED,

    CredUiInfoW, DesktopApi, ERROR_CANCELLED, GetLastError, UOI_FLAGS, UserObjectFlags,

    WSF_VISIBLE, desktop_api, error_for, wide, window_station_api,

};
// ------------------------------------------------------------------ presence

/// Whether a prompt can be shown to someone: not in session 0, which has no desktop, and with
/// an interactive (visible) window station. The reason is the distinct text for each case.
pub(crate) fn present(session: Option<u32>, flags: Option<u32>) -> Result<(), &'static str> {
    match (session, flags) {
        (None, _) => Err("the session of the process cannot be read, so nobody can be asked"),
        (Some(0), _) => Err("the process runs in session 0, which has no desktop to show the prompt on"),
        (_, None) => Err("the window station of the process cannot be read, so nobody can be asked"),
        (_, Some(flags)) if flags & WSF_VISIBLE == 0 => Err("the process has no interactive window station to show the prompt on"),
        _ => Ok(()),
    }
}

/// The session of this process and the flags of its window station, when they can be read.
pub(super) fn detect() -> (Option<u32>, Option<u32>) {
    let session = own_session();
    let flags = window_station_api().and_then(|api| unsafe {
        // The handle belongs to the process and is not closed.
        let station = (api.station)();
        if station.is_null() {
            return None;
        }
        let mut info = UserObjectFlags::default();
        let mut needed = 0u32;
        let read = (api.information)(station, UOI_FLAGS, &mut info as *mut _ as *mut c_void, std::mem::size_of::<UserObjectFlags>() as u32, &mut needed);
        (read != 0).then_some(info.flags)
    });
    (session, flags)
}

// ------------------------------------------------------------ the desktop prompt

/// The credential dialog of the desktop method, used where Windows Hello is not available:
/// `CredUIPromptForWindowsCredentialsW` showing only the accounts of the current user, then
/// `CredUnPackAuthenticationBufferW`. The user whose sign-in is a Personal Identification Number
/// (PIN) gets the password in a protected form, which is unprotected (`CredUnprotectW`) into a
/// buffer that is wiped like any other secret. It is refused without showing anything when the
/// process has no interactive window station or runs in session 0.
pub(crate) struct DesktopPrompt<'a> {
    owners: &'a Owners,
    /// The thread that makes every call of the prompt of this run, made when the first is asked.
    worker: std::cell::RefCell<Option<PromptWorker>>,
}

impl<'a> DesktopPrompt<'a> {
    pub(crate) fn new(owners: &'a Owners) -> DesktopPrompt<'a> {
        DesktopPrompt { owners, worker: std::cell::RefCell::new(None) }
    }
}

impl Prompt for DesktopPrompt<'_> {
    fn begin(&self, text: &str) -> Result<Claim, String> {
        self.ready(text)?;
        self.seat(text)
    }

    fn seat(&self, _text: &str) -> Result<Claim, String> {
        if window_station_api().is_none() {
            return Err(NOT_AVAILABLE.into());
        }
        let (session, flags) = detect();
        present(session, flags)?;
        Ok(Claim { _lock: Some(PromptLock::take()?) })
    }

    fn ready(&self, text: &str) -> Result<(), String> {
        if desktop_api().is_none() || window_station_api().is_none() {
            return Err(NOT_AVAILABLE.into());
        }
        // The field limits keep the message under the most the dialog takes; a message that is not
        // refuses the run rather than reach a dialog that would clip the way to refuse it.
        if text.encode_utf16().count() >= CREDUI_MAX_MESSAGE_LENGTH {
            return Err("the message of the prompt is longer than the dialog takes".into());
        }
        Ok(())
    }

    fn ask(&self, _attempt: u32, last_error: u32, text: &str) -> Asked {
        let Some(api) = desktop_api() else { return Asked::NotShown(NOT_AVAILABLE.into()) };
        let (message, caption) = (wide(text), wide(CAPTION));
        let mut slot = self.worker.borrow_mut();
        if slot.is_none() {
            *slot = PromptWorker::start();
        }
        let Some(worker) = slot.as_ref() else { return Asked::NotShown("the prompt could not be started".into()) };
        worker.ask(self.owners, Duration::from_millis(PROMPT_TIMEOUT_MS), move || desktop_dialog(api, &message, &caption, last_error))
    }
}

pub(super) fn desktop_dialog(api: &DesktopApi, message: &[u16], caption: &[u16], last_error: u32) -> Asked {
    let info = CredUiInfoW {
        size: std::mem::size_of::<CredUiInfoW>() as u32,
        parent: std::ptr::null_mut(),
        message: message.as_ptr(),
        caption: caption.as_ptr(),
        banner: std::ptr::null_mut(),
    };
    let (mut package, mut save) = (0u32, 0);
    let mut out = OutBuffer { pointer: std::ptr::null_mut(), size: 0, free: api.free };
    let status = unsafe {
        (api.prompt)(&info, last_error, &mut package, std::ptr::null(), 0,
            &mut out.pointer, &mut out.size, &mut save, CREDUIWIN_SECURE_PROMPT | CREDUIWIN_ENUMERATE_CURRENT_USER)
    };
    match status {
        0 => {}
        ERROR_CANCELLED => return Asked::Cancelled,
        code => return Asked::Unavailable(format!("the prompt failed: {}", error_for(code).message())),
    }
    if !out.pointer.is_null() && out.size > 0 {
        register(out.pointer as *const u8, out.size as usize);
    }
    read_credentials(api, out.pointer, out.size)
}

/// Read the name, the domain and the password out of the authentication buffer that the dialog returned. The password buffer
/// is the size of a password; one that is not large enough holds a password in a protected form, which is longer than the
/// password it holds, and the unpacking is made again with a buffer of the size the system asked for (at most
/// `PROTECTED_LIMIT`). A password in a protected form is then unprotected into a buffer of the size of a password. Every buffer
/// is a registered secret, wiped when it goes, on every path.
pub(super) fn read_credentials(api: &DesktopApi, buffer: *const c_void, size: u32) -> Asked {
    let mut credentials = Box::new(Credentials::new());
    let (mut user_chars, mut domain_chars, mut password_chars) = (CREDUI_USER_CHARS as u32, CREDUI_DOMAIN_CHARS as u32, CREDUI_PASSWORD_CHARS as u32);
    let mut unpacked = unsafe {
        (api.unpack)(0, buffer, size,
            credentials.user.0.as_mut_ptr(), &mut user_chars,
            credentials.domain.0.as_mut_ptr(), &mut domain_chars,
            credentials.password.0.as_mut_ptr(), &mut password_chars)
    };
    let mut code = if unpacked == 0 { unsafe { GetLastError() } } else { 0 };
    if unpacked == 0 && code == ERROR_INSUFFICIENT_BUFFER && password_chars as usize > CREDUI_PASSWORD_CHARS {
        if password_chars as usize > PROTECTED_LIMIT {
            return Asked::Rejected("the password is longer than the guard takes".into());
        }
        // The old buffer is wiped and taken out of the list as the new one takes its place.
        credentials.password = zeroed(password_chars as usize);
        (user_chars, domain_chars, password_chars) = (CREDUI_USER_CHARS as u32, CREDUI_DOMAIN_CHARS as u32, credentials.password.0.len() as u32);
        unpacked = unsafe {
            (api.unpack)(0, buffer, size,
                credentials.user.0.as_mut_ptr(), &mut user_chars,
                credentials.domain.0.as_mut_ptr(), &mut domain_chars,
                credentials.password.0.as_mut_ptr(), &mut password_chars)
        };
        code = if unpacked == 0 { unsafe { GetLastError() } } else { 0 };
    }
    if unpacked == 0 {
        // The prompt was answered with something that is not a name and a password (a smart card may be so), or with a name or
        // password that is too long: an attempt used, with no logon tried, and the user is asked again. The buffers are wiped
        // when `credentials` goes.
        return Asked::Rejected(if code == ERROR_INSUFFICIENT_BUFFER {
            "the name or password is longer than the prompt allows".into()
        } else {
            format!("the answer of the prompt could not be read as a user name and password: {}", error_for(code).message())
        });
    }
    // A password that the system hands back in a protected (encrypted) form is not the text that was typed: sent to `LogonUserW`
    // it would fail as a wrong password and count toward the lockout of the account. It is unprotected first; one that cannot
    // be uses an attempt of its own, with a reason of its own, and no logon is tried for it.
    if password_is_protected(api, &mut credentials.password.0) {
        match unprotect(api, &mut credentials.password.0) {
            // The protected form is wiped as the clear one takes its place.
            Ok(clear) => credentials.password = clear,
            Err(reason) => return Asked::Rejected(reason),
        }
    }
    Asked::Entered(credentials)
}

/// The clear text of `protected`, a password in a protected form that ends with a NUL inside the slice (`CredUnprotectW`, for
/// the user whose sign-in it is), in a registered buffer of the size of a password. The size is asked for first, and a password
/// that would not fit is refused. The clear password is in memory from here until the credentials are dropped.
pub(super) fn unprotect(api: &DesktopApi, protected: &mut [u16]) -> Result<Secret<Box<[u16]>>, String> {
    let failed = |code: u32| format!("the protected form of the password could not be unprotected: {}", error_for(code).message());
    let Some(end) = protected.iter().position(|unit| *unit == 0) else { return Err(failed(ERROR_INSUFFICIENT_BUFFER)) };
    // The length counts the NUL, as the call that makes a protected form counts it.
    let length = end as u32 + 1;
    let mut needed = 0u32;
    let sized = unsafe { (api.unprotect)(0, protected.as_mut_ptr(), length, std::ptr::null_mut(), &mut needed) };
    let code = unsafe { GetLastError() };
    if sized != 0 || code != ERROR_INSUFFICIENT_BUFFER {
        return Err(failed(if sized != 0 { UNIDENTIFIED } else { code }));
    }
    if needed as usize > CREDUI_PASSWORD_CHARS {
        return Err("the password is longer than the guard takes".into());
    }
    let mut clear = zeroed(CREDUI_PASSWORD_CHARS);
    let mut chars = CREDUI_PASSWORD_CHARS as u32;
    if unsafe { (api.unprotect)(0, protected.as_mut_ptr(), length, clear.0.as_mut_ptr(), &mut chars) } == 0 {
        return Err(failed(unsafe { GetLastError() }));
    }
    Ok(clear)
}

/// True when `password`, which ends with a NUL inside the slice (the unpacking that filled it
/// succeeded), is in a protected form (`CredIsProtectedW`) and not clear text.
pub(super) fn password_is_protected(api: &DesktopApi, password: &mut [u16]) -> bool {
    let mut kind = CRED_UNPROTECTED;
    unsafe { (api.protected)(password.as_mut_ptr(), &mut kind) != 0 && kind != CRED_UNPROTECTED }
}
