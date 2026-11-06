// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Hardening a process-owning invocation: the deny-only access lists on its process and its threads.

use super::*;


use crate::sys::windows::{

    DACL_SECURITY_INFORMATION, GetCurrentProcess, GetCurrentThread, GetSecurityInfo, Handle,

    LocalFree, SetSecurityInfo, WRITE_DAC, error_for, raw_handle,

};
/// The creating thread already owns this native handle. Apply the worker ACL
/// before any payload exists; no thread-ID reopen or process-wide enumeration.
pub(crate) fn harden_worker<T>(thread: &std::thread::JoinHandle<T>) -> Result<(), GuardError> {
    if REAL_CLI.load(std::sync::atomic::Ordering::Acquire) {
        harden_thread(raw_handle(thread))?;
    }
    Ok(())
}

pub(super) const ACL_REVISION: u32 = 2;
pub(super) const SE_KERNEL_OBJECT: i32 = 6;
const PROCESS_DUP_HANDLE: u32 = 0x0040;
const PROCESS_SUSPEND_RESUME: u32 = 0x0800;
const PROCESS_VM_OPERATION: u32 = 0x0008;
const PROCESS_VM_WRITE: u32 = 0x0020;
const PROCESS_VM_READ: u32 = 0x0010;
const PROCESS_SET_INFORMATION: u32 = 0x0200;
const PROCESS_SET_QUOTA: u32 = 0x0100;
const PROCESS_CREATE_PROCESS: u32 = 0x0080;
const PROCESS_CREATE_THREAD: u32 = 0x0002;
const PROCESS_SET_LIMITED_INFORMATION: u32 = 0x2000;
pub(super) const PROCESS_TERMINATE: u32 = 0x0001;
const WRITE_OWNER: u32 = 0x0008_0000;
pub(super) const PROCESS_DENY_MASK: u32 = PROCESS_DUP_HANDLE | PROCESS_SUSPEND_RESUME | PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ
    | PROCESS_SET_INFORMATION | PROCESS_SET_QUOTA | PROCESS_CREATE_PROCESS | PROCESS_CREATE_THREAD
    | PROCESS_SET_LIMITED_INFORMATION | WRITE_DAC | WRITE_OWNER;
const THREAD_SUSPEND_RESUME: u32 = 0x0002;
const THREAD_TERMINATE: u32 = 0x0001;
const THREAD_SET_INFORMATION: u32 = 0x0020;
const THREAD_SET_LIMITED_INFORMATION: u32 = 0x0400;
const THREAD_SET_CONTEXT: u32 = 0x0010;
const THREAD_GET_CONTEXT: u32 = 0x0008;
const THREAD_SET_THREAD_TOKEN: u32 = 0x0080;
const THREAD_DENY_MASK: u32 = THREAD_SUSPEND_RESUME | THREAD_TERMINATE
    | THREAD_SET_INFORMATION | THREAD_SET_LIMITED_INFORMATION | THREAD_SET_CONTEXT | THREAD_GET_CONTEXT
    | THREAD_SET_THREAD_TOKEN | WRITE_DAC | WRITE_OWNER;
pub(super) const GENERIC_ALL: u32 = 0x1000_0000;
/// S-1-1-0, Everyone: revision 1, one subauthority, the world authority,
/// the world RID. Built literally so no well-known-SID lookup is needed.
pub(super) const EVERYONE_SID: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
/// S-1-5-18, Local System: revision 1, one subauthority, the NT
/// authority (5), the system RID (18). Built literally, as above.
const SYSTEM_SID: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];
/// S-1-3-4, Owner Rights: revision 1, one subauthority, the creator
/// authority (3), the owner-rights RID (4). Built literally, as above.
const OWNER_RIGHTS_SID: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 3, 4, 0, 0, 0];

#[repr(C, align(4))]
pub(super) struct AclStack([std::mem::MaybeUninit<u8>; 1024]);

#[allow(clippy::large_enum_variant)]
pub(super) enum AclOut {
    /// Uninitialised storage with a DWORD-aligned payload. The builder
    /// writes every byte of the ACL's logical length — header, ACEs and
    /// the copied tail — before the pointer reaches the kernel, so no
    /// byte is read before it is written and none needs pre-zeroing.
    Stack(AclStack),
    // u32 storage guarantees DWORD alignment even though ACLs are written
    // byte by byte; the allocator need not over-align a Vec<u8>.
    Heap(Vec<std::mem::MaybeUninit<u32>>),
}

impl AclOut {
    pub(super) fn new(total: usize) -> Self {
        if total <= 1024 {
            Self::Stack(AclStack([std::mem::MaybeUninit::uninit(); 1024]))
        } else {
            let words = total / 4 + usize::from(!total.is_multiple_of(4));
            Self::Heap(vec![std::mem::MaybeUninit::uninit(); words])
        }
    }

    pub(super) fn as_mut_ptr(&mut self) -> *mut c_void {
        match self {
            AclOut::Stack(bytes) => bytes.0.as_mut_ptr() as *mut c_void,
            AclOut::Heap(vec) => vec.as_mut_ptr() as *mut c_void,
        }
    }

    pub(super) fn bytes_mut(&mut self, total: usize) -> &mut [std::mem::MaybeUninit<u8>] {
        let capacity = match self {
            Self::Stack(_) => 1024,
            Self::Heap(words) => words.len() * 4,
        };
        assert!(total <= capacity);
        unsafe { std::slice::from_raw_parts_mut(self.as_mut_ptr() as *mut std::mem::MaybeUninit<u8>, total) }
    }
}

pub(super) fn deny_acl(object: Handle, mask: u32) -> Result<AclOut, GuardError> {
    let mut old_dacl: *mut c_void = std::ptr::null_mut();
    let mut descriptor: *mut c_void = std::ptr::null_mut();
    let err = unsafe { GetSecurityInfo(object, SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION,
        std::ptr::null_mut(), std::ptr::null_mut(), &mut old_dacl, std::ptr::null_mut(), &mut descriptor) };
    if err != 0 {
        return Err(error_for(err));
    }
    let old_size: usize;
    let old_count: u16;
    if old_dacl.is_null() {
        old_size = 0;
        old_count = 0;
    } else {
        let header = unsafe { std::slice::from_raw_parts(old_dacl as *const u8, 8) };
        old_size = u16::from_le_bytes([header[2], header[3]]) as usize;
        old_count = u16::from_le_bytes([header[4], header[5]]);
        if old_size < 8 {
            if !descriptor.is_null() {
                unsafe { LocalFree(descriptor) };
            }
            return Err(GuardError("existing DACL header is too short".into()));
        }
    }
    // Two deny ACEs — Everyone and SYSTEM (the two built-in
    // SIDs are the same 12-byte length each) — plus the
    // OWNER RIGHTS allow ACE (mask 0) that governs the owner's
    // implicit rights. The guard's own user needs no deny ACE
    // of its own: every access token carries Everyone, so the
    // Everyone deny at this same mask already refuses the
    // guard's own user — a user ACE beside it denied nobody
    // the Everyone ACE did not.
    let deny_size = (8 + EVERYONE_SID.len()) + (8 + SYSTEM_SID.len()) + (8 + OWNER_RIGHTS_SID.len());
    let total = if old_dacl.is_null() { 8 + deny_size + (8 + EVERYONE_SID.len()) } else { 8 + deny_size + (old_size - 8) };
    if total > u16::MAX as usize {
        if !descriptor.is_null() {
            unsafe { LocalFree(descriptor) };
        }
        return Err(GuardError("hardened DACL would exceed the ACL size limit".into()));
    }
    let mut out = AclOut::new(total);
    let acl = out.bytes_mut(total);
    if !old_dacl.is_null() {
        // The old ACE bytes move straight from the fetched
        // descriptor into their place at the new ACL's tail —
        // one copy, no intermediate buffer — while the
        // descriptor that owns them is still alive.
        let at = 8 + deny_size;
        // This copy writes the tail range in full before any read of it.
        unsafe { std::ptr::copy_nonoverlapping((old_dacl as *const u8).add(8), (acl.as_mut_ptr() as *mut u8).add(at), old_size - 8) };
    }
    if !descriptor.is_null() { unsafe { LocalFree(descriptor) }; }
    // The ACL is written byte by byte instead of through the
    // advapi32 formatting helpers (InitializeAcl and the
    // Add*Ace calls): those perform no kernel work — each one
    // appends a documented fixed-layout record to this same
    // buffer — and there were five of them per object. The
    // layout is the documented one, and this function's own
    // size arithmetic above already depends on it exactly:
    // an 8-byte header (revision, total size, ACE count) and,
    // per ACE, a header (type — 0 allowed, 1 denied — flags 0,
    // record size), the mask, and the SID bytes, in the order
    // the helper calls appended them: user deny, Everyone
    // deny, SYSTEM deny, OWNER RIGHTS allow, then the old
    // ACEs (already copied to the tail) or, for a null DACL,
    // the Everyone GENERIC_ALL allow that replaces them. The
    // validation the helpers performed is not lost: every SID
    // here is kernel-supplied or a compile-time constant, and
    // SetSecurityInfo validates the finished DACL before
    // applying it, so a malformed result still fails closed.
    fn write_bytes(acl: &mut [std::mem::MaybeUninit<u8>], at: usize, bytes: &[u8]) {
        for (slot, &byte) in acl[at..at + bytes.len()].iter_mut().zip(bytes) {
            slot.write(byte);
        }
    }
    fn write_ace(acl: &mut [std::mem::MaybeUninit<u8>], at: usize, kind: u8, ace_mask: u32, sid_bytes: &[u8]) -> usize {
        acl[at].write(kind);
        acl[at + 1].write(0);
        write_bytes(acl, at + 2, &((8 + sid_bytes.len()) as u16).to_le_bytes());
        write_bytes(acl, at + 4, &ace_mask.to_le_bytes());
        write_bytes(acl, at + 8, sid_bytes);
        at + 8 + sid_bytes.len()
    }
    let ace_count = if old_dacl.is_null() { 4 } else { 3 + old_count };
    acl[0].write(ACL_REVISION as u8);
    // Write the reserved header fields explicitly: the storage is
    // uninitialised, so every header byte is written here, not inherited.
    acl[1].write(0);
    acl[6].write(0);
    acl[7].write(0);
    write_bytes(acl, 2, &(total as u16).to_le_bytes());
    write_bytes(acl, 4, &ace_count.to_le_bytes());
    let mut at = 8;
    at = write_ace(&mut *acl, at, 1, mask, &EVERYONE_SID);
    at = write_ace(&mut *acl, at, 1, mask, &SYSTEM_SID);
    at = write_ace(&mut *acl, at, 0, 0, &OWNER_RIGHTS_SID);
    if old_dacl.is_null() {
        write_ace(&mut *acl, at, 0, GENERIC_ALL, &EVERYONE_SID);
    }
    Ok(out)
}

pub(super) fn harden_self(process_mask: u32) -> Result<(), GuardError> {
    // The pseudo-handle is fetched once and used for both the fetch of
    // the current access list and the application of the hardened one:
    // fetching it again between the two bought nothing, as the value
    // is this process's own constant handle.
    let process = unsafe { GetCurrentProcess() };
    let mut process_acl = deny_acl(process, process_mask)?;
    let err = unsafe { SetSecurityInfo(process, SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION,
        std::ptr::null_mut(), std::ptr::null_mut(), process_acl.as_mut_ptr(), std::ptr::null_mut()) };
    if err != 0 {
        return Err(error_for(err));
    }
    harden_thread(unsafe { GetCurrentThread() })
}

fn harden_thread(thread: Handle) -> Result<(), GuardError> {
    let mut thread_acl = deny_acl(thread, THREAD_DENY_MASK)?;
    let err = unsafe { SetSecurityInfo(thread, SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION,
        std::ptr::null_mut(), std::ptr::null_mut(), thread_acl.as_mut_ptr(), std::ptr::null_mut()) };
    if err != 0 { return Err(error_for(err)); }
    Ok(())
}
