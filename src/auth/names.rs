// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Account names for the approval: the domain and user a logon checks, and the names a token reports.

use super::*;


use crate::sys::windows::{GetLastError, NAME_SAM_COMPATIBLE, logon_api, name_api, wide};
// --------------------------------------------------------------------- names

/// The domain and the user for `LogonUserW`. A name with a backslash is split at the first
/// one; otherwise a name with an at sign is a user principal name, which takes no domain
/// (None); otherwise the domain the prompt returned, and when there is none the local
/// account database (`.`). The prompt does not need `CredUIParseUserNameW` for this: the
/// forms it returns are these.
pub(super) fn split_name(name: &str, domain: &str) -> (Option<String>, String) {
    if let Some((domain, user)) = name.split_once('\\') {
        (Some(domain.to_string()), user.to_string())
    } else if name.contains('@') {
        (None, name.to_string())
    } else if !domain.is_empty() {
        (Some(domain.to_string()), name.to_string())
    } else {
        (Some(".".to_string()), name.to_string())
    }
}

/// The name as `LookupAccountNameW` takes it: `DOMAIN\user`, a user principal name as it is, and
/// a name in the local database as it is.
pub(super) fn full_name(domain: Option<&str>, user: &str) -> String {
    match domain {
        Some(domain) if domain != "." => format!("{}\\{}", domain, user),
        _ => user.to_string(),
    }
}

/// The name of the account this process runs as, `DOMAIN\user`, as the system writes it for the
/// account that is signed in (`GetUserNameExW`, not a look up of the security identifier, which
/// can ask a domain controller); None when it cannot be read.
pub(super) fn own_account_name() -> Option<String> {
    let api = name_api()?;
    let mut name = [std::mem::MaybeUninit::<u16>::uninit(); 1024];
    let mut chars = name.len() as u32;
    // The size is in characters, and on success it is the length without the end.
    if unsafe { (api.name)(NAME_SAM_COMPATIBLE, name.as_mut_ptr() as *mut u16, &mut chars) } == 0 || chars == 0 || chars as usize >= name.len() {
        return None;
    }
    // The call wrote the `chars` units the read is confined to.
    Some(String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(name.as_ptr() as *const u16, chars as usize) }))
}

/// What a failed look up of an account comes to: no such account is a name that is not accepted;
/// anything else is a failure of the system.
pub(super) fn classify_lookup_error(code: u32) -> Verified {
    if code == ERROR_NONE_MAPPED { Verified::Wrong } else { Verified::Failed(code) }
}

/// Before any logon is tried with `name`: the verdict when it cannot be the account the guard
/// runs as, None when it can. Every logon with a wrong password counts toward the lockout of the
/// account it was tried for, so a name that is another account's, or no account's, is turned away
/// here and `LogonUserW` is not called for it. The security identifier of the token of a logon
/// that is made is still compared afterwards: this look up only decides what is not tried.
pub(super) fn account_check(name: &str) -> Option<Verified> {
    if own_account_name().is_some_and(|own| own.eq_ignore_ascii_case(name)) {
        return None;
    }
    let Some(api) = logon_api() else { return Some(Verified::Failed(UNIDENTIFIED)) };
    let text = wide(name);
    let (mut sid_size, mut domain_chars, mut kind) = (0u32, 0u32, 0u32);
    unsafe {
        (api.lookup)(std::ptr::null(), text.as_ptr(), std::ptr::null_mut(), &mut sid_size, std::ptr::null_mut(), &mut domain_chars, &mut kind);
        let code = GetLastError();
        if code != ERROR_INSUFFICIENT_BUFFER {
            return Some(classify_lookup_error(code));
        }
        let mut sid = vec![0u64; (sid_size as usize).div_ceil(8) + 1];
        let mut domain = vec![0u16; domain_chars as usize + 1];
        if (api.lookup)(std::ptr::null(), text.as_ptr(), sid.as_mut_ptr() as *mut c_void, &mut sid_size, domain.as_mut_ptr(), &mut domain_chars, &mut kind) == 0 {
            return Some(classify_lookup_error(GetLastError()));
        }
        match (sid_text(sid.as_mut_ptr() as *mut c_void), current_user_sid()) {
            (Ok(theirs), Ok(ours)) if !theirs.is_empty() && theirs == ours => None,
            (Ok(_), Ok(_)) => Some(Verified::OtherUser),
            _ => Some(Verified::Failed(UNIDENTIFIED)),
        }
    }
}
