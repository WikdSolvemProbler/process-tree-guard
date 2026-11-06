// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Native functions used only by Windows test fixtures.

use super::{Bool, Handle};
use std::ffi::c_void;

#[link(name = "advapi32")]
extern "system" {
    pub(crate) fn ConvertStringSidToSidW(text: *const u16, sid: *mut *mut c_void) -> Bool;
    pub(crate) fn CredProtectW(as_self: Bool, credentials: *const u16, chars: u32, protected: *mut u16, protected_chars: *mut u32, kind: *mut i32) -> Bool;
    pub(crate) fn IsValidAcl(acl: *const c_void) -> Bool;
    pub(crate) fn LookupAccountSidW(system: *const u16, sid: *mut c_void, name: *mut u16, name_chars: *mut u32, domain: *mut u16, domain_chars: *mut u32, kind: *mut u32) -> Bool;
    pub(crate) fn RegDeleteKeyW(key: Handle, subkey: *const u16) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    pub(crate) fn CreatePipe(read: *mut Handle, write: *mut Handle, attributes: *mut c_void, size: u32) -> Bool;
}

#[link(name = "user32")]
extern "system" {
    pub(crate) fn CreateDesktopW(name: *const u16, device: *const u16, devmode: *mut c_void, flags: u32, access: u32, attributes: *mut c_void) -> Handle;
    pub(crate) fn CreateWindowStationW(name: *const u16, flags: u32, access: u32, attributes: *mut c_void) -> Handle;
    pub(crate) fn PostMessageW(window: Handle, message: u32, w: usize, l: isize) -> Bool;
    pub(crate) fn PostThreadMessageW(thread: u32, message: u32, w: usize, l: isize) -> Bool;
    pub(crate) fn SetProcessWindowStation(station: Handle) -> Bool;
    pub(crate) fn SetThreadDesktop(desktop: Handle) -> Bool;
}
