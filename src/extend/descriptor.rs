// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The channel's security descriptor: the current user's identifier, in the text form the pipe's access list is built from.

use super::*;


use crate::sys::windows::{

    CloseHandle, ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,

    CreateNamedPipeW, DACL_SECURITY_INFORMATION, FILE_FLAG_FIRST_PIPE_INSTANCE,

    FILE_FLAG_OVERLAPPED, GetCurrentProcess, GetLastError, GetTokenInformation, Handle,

    INVALID_HANDLE, LocalFree, OpenProcessToken, PIPE_ACCESS_DUPLEX, SecurityAttributes,

    SetKernelObjectSecurity, TOKEN_QUERY, TOKEN_USER_CLASS, WRITE_DAC, checked, error_for,

    os_error, wide,

};
/// The text form of the security identifier of the user this process runs as.
pub(crate) fn current_user_sid() -> Result<String, GuardError> {
    unsafe {
        let mut token: Handle = std::ptr::null_mut();
        checked(OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token))?;
        let sid = token_user_sid(token);
        CloseHandle(token);
        sid
    }
}

/// The text form of the security identifier of the user of `token`, which stays open.
pub(crate) fn token_user_sid(token: Handle) -> Result<String, GuardError> {
    unsafe {
        // The kernel writes a TOKEN_USER into the buffer given: a
        // pointer and attributes (16 bytes) followed by the security
        // identifier itself, whose largest form is an 8-byte header
        // and fifteen subauthorities — 84 bytes in all. A 128-byte
        // stack buffer is therefore answered in one call. The sizing
        // call this replaces paid a second GetTokenInformation, and a
        // zero-filled heap buffer, on every token read; a report
        // larger than the stack form (no valid token produces one)
        // takes that heap form, sized as the sizing call sized it.
        let mut stack = [std::mem::MaybeUninit::<u64>::uninit(); 16];
        let mut needed = 0u32;
        if GetTokenInformation(token, TOKEN_USER_CLASS, stack.as_mut_ptr() as *mut c_void, (stack.len() * 8) as u32, &mut needed) != 0 {
            // The call wrote the structure the pointer is read from.
            // TOKEN_USER starts with a pointer to the security identifier.
            return sid_text(*(stack.as_ptr() as *const *mut c_void));
        }
        let failure = GetLastError();
        if (needed as usize) <= stack.len() * 8 {
            return Err(error_for(failure));
        }
        let mut buffer = vec![0u64; (needed as usize).div_ceil(8) + 1];
        checked(GetTokenInformation(token, TOKEN_USER_CLASS, buffer.as_mut_ptr() as *mut c_void, (buffer.len() * 8) as u32, &mut needed))?;
        // TOKEN_USER starts with a pointer to the security identifier.
        sid_text(*(buffer.as_ptr() as *const *mut c_void))
    }
}

/// The text form of the security identifier `sid`.
pub(crate) fn sid_text(sid: *mut c_void) -> Result<String, GuardError> {
    unsafe {
        let mut text: *mut u16 = std::ptr::null_mut();
        checked(ConvertSidToStringSidW(sid, &mut text))?;
        let mut length = 0;
        while *text.add(length) != 0 {
            length += 1;
        }
        let sid_text = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
        LocalFree(text as *mut c_void);
        Ok(sid_text)
    }
}

/// The text of one pipe security descriptor, in stack storage: the
/// descriptor is the fixed frame `D:P(A;;` and `;;;` and `)` around
/// the rights text and the current user's security identifier, whose
/// text form is at most some 176 characters (fifteen subauthorities
/// of up to ten digits), so the whole descriptor is under 197 bytes
/// and a 256-byte buffer covers every descriptor `create_pipes`
/// builds. The bytes are written one by one before any read of them,
/// so no byte needs clearing first.
pub(super) struct DescriptorText {
    bytes: [std::mem::MaybeUninit<u8>; 256],
    len: usize,
}

impl DescriptorText {
    pub(super) fn new() -> DescriptorText {
        DescriptorText { bytes: [std::mem::MaybeUninit::uninit(); 256], len: 0 }
    }

    pub(super) fn as_str(&self) -> &str {
        // The fragments written through `core::fmt::Write` are whole
        // text fragments, so the written prefix is valid text by
        // construction. The read is confined to that prefix.
        let bytes = unsafe { std::slice::from_raw_parts(self.bytes.as_ptr() as *const u8, self.len) };
        std::str::from_utf8(bytes).unwrap_or("")
    }
}

impl core::fmt::Write for DescriptorText {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        if self.len + text.len() > self.bytes.len() {
            return Err(core::fmt::Error);
        }
        for (slot, &byte) in self.bytes[self.len..self.len + text.len()].iter_mut().zip(text.as_bytes()) {
            slot.write(byte);
        }
        self.len += text.len();
        Ok(())
    }
}

/// The wide form of a pipe security descriptor, in stack storage
/// when it fits and in a heap vector otherwise, of the `WideName`
/// kind: the descriptor text is plain ASCII, one unit per byte, so
/// a text that fits the 256-byte stack form above fits this 256-unit
/// form with its terminator in every case the bound above covers,
/// and the heap form is the fallback for a text of any other shape,
/// spelled exactly as `wide` spells it. The units are written one
/// by one before any read of them, so no unit needs clearing first.
pub(super) fn descriptor_wide(text: &str) -> super::client::StackWide<256> { super::client::stack_wide::<256>(text) }

/// Convert the wide text at `text` to a security descriptor (to be
/// freed with `LocalFree`): the one conversion both descriptor
/// entries below hand their text to.
pub(super) fn descriptor_from_wide(text: *const u16) -> Result<*mut c_void, GuardError> {
    let mut descriptor: *mut c_void = std::ptr::null_mut();
    checked(unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(text, 1, &mut descriptor, std::ptr::null_mut()) })?;
    Ok(descriptor)
}

/// An access list, in the text form, as a security descriptor (to be freed with `LocalFree`).
pub(super) fn security_descriptor(sddl: &str) -> Result<*mut c_void, GuardError> {
    let text = wide(sddl);
    descriptor_from_wide(text.as_ptr())
}

/// The security descriptor of one pipe of the channel, for the
/// rights `rights` and the current user `sid`: the descriptor text
/// is the same `D:P(A;;{};;;{})` text `create_pipes` has always
/// built, assembled into the stack form above with
/// `core::fmt::Write` and converted through its stack wide form,
/// so an extension start pays no heap allocation for either of its
/// two descriptors. Text that exceeds the stack buffer takes the
/// heap route unchanged — the same `format!` text converted by the
/// same `wide()` in `security_descriptor` — so the descriptor text
/// is byte-identical either way and the resulting access lists
/// cannot differ.
pub(super) fn pipe_security_descriptor(rights: &str, sid: &str) -> Result<*mut c_void, GuardError> {
    use core::fmt::Write as _;
    let mut text = DescriptorText::new();
    if write!(text, "D:P(A;;{};;;{})", rights, sid).is_err() {
        return security_descriptor(&format!("D:P(A;;{};;;{})", rights, sid));
    }
    let wide_text = descriptor_wide(text.as_str());
    descriptor_from_wide(wide_text.as_ptr())
}

/// The open mode of instance `index` of the channel: duplex and overlapped, with the right to
/// change the access list, and, for the first only, the first-instance flag.
pub(super) fn instance_open_mode(index: u32) -> u32 {
    let first = if index == 0 { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
    PIPE_ACCESS_DUPLEX | first | WRITE_DAC | FILE_FLAG_OVERLAPPED
}

/// Make the instances of the pipe `name`. The first is made with the first-instance flag,
/// so the name is not one that somebody else already holds. All `INSTANCES` the pipe allows
/// are made here, before the command starts, so nobody else can make another. The access
/// list is for the current user only and protected, so nothing is inherited into it. Making
/// the later instances takes the right to make an instance, so the list has it while they are
/// made and loses it at once after: the list the command's processes see does not give it.
pub(super) fn create_pipes(name: &str) -> Result<[Handle; INSTANCES as usize], GuardError> {
    let sid = current_user_sid()?;
    let making = pipe_security_descriptor(PIPE_RIGHTS_WHILE_MAKING, &sid)?;
    let settled = match pipe_security_descriptor(PIPE_RIGHTS, &sid) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            unsafe { LocalFree(making) };
            return Err(error);
        }
    };
    unsafe {
        let mut attributes = SecurityAttributes { length: std::mem::size_of::<SecurityAttributes>() as u32, descriptor: making, inherit: 0 };
        // The client's stack form: a channel name is 61 units with
        // its terminator and never reaches the heap form.
        let wide_name = wide_name(name);
        // The instance count is the constant `INSTANCES`, so the
        // handles live in fixed stack storage: the vector form grew
        // one heap allocation for a count already bounded here.
        // Slots past `made` were never written and hold null.
        let mut pipes = [std::ptr::null_mut(); INSTANCES as usize];
        let mut made = 0usize;
        let mut failure = None;
        for index in 0..INSTANCES {
            let pipe = CreateNamedPipeW(wide_name.as_ptr(), instance_open_mode(index),
                CHANNEL_PIPE_MODE, INSTANCES, OUT_BUFFER, 512, 0, &mut attributes);
            if pipe == INVALID_HANDLE {
                failure = Some(os_error());
                break;
            }
            pipes[made] = pipe;
            made += 1;
        }
        if failure.is_none() {
            for pipe in &pipes {
                if SetKernelObjectSecurity(*pipe, DACL_SECURITY_INFORMATION, settled) == 0 {
                    failure = Some(os_error());
                    break;
                }
            }
        }
        LocalFree(making);
        LocalFree(settled);
        match failure {
            Some(error) => {
                for pipe in pipes.into_iter().take(made) {
                    CloseHandle(pipe);
                }
                Err(error)
            }
            None => Ok(pipes),
        }
    }
}
