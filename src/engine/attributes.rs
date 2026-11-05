// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The creation attributes of a run: the attribute list that places the child in its job and limits the handles it inherits.

use super::*;


use crate::sys::windows::{

    DeleteProcThreadAttributeList, GetLastError, Handle, InitializeProcThreadAttributeList,

    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST, UpdateProcThreadAttribute,

    checked,

};
// Native attribute storage is opaque. Its allocation is explicitly aligned and
// remains initialized and alive through DeleteProcThreadAttributeList.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub(super) struct AttributeBlock(pub(super) [u8; 16]);

enum AttributeStorage<'a> {
    Stack(&'a mut [AttributeBlock; 8]),
    Heap(Vec<AttributeBlock>),
}

pub(super) struct JobAttributeList<'a> {
    storage: AttributeStorage<'a>,
    // Windows retains lpValue until Delete: borrow the caller's stable array,
    // rather than an inline array that would move when this wrapper is returned.
    jobs: &'a mut [Handle; 1],
    inherited: Option<&'a mut [Handle]>,
    initialized: bool,
    // A process-owning CLI invocation exits natively right after its one run:
    // the list's backing is the caller's own storage and process exit
    // reclaims the bookkeeping, so the Delete call is skipped there. A
    // returning library call lives on, so it still deletes its list.
    delete_on_drop: bool,
}

impl<'a> JobAttributeList<'a> {
    pub(super) fn new(stack: &'a mut [AttributeBlock; 8], jobs: &'a mut [Handle; 1], inherited: Option<&'a mut [Handle]>, delete_on_drop: bool) -> Result<Self, GuardError> {
        let count = 1 + u32::from(inherited.as_ref().is_some_and(|handles| !handles.is_empty()));
        let mut bytes = std::mem::size_of_val(stack);
        // The ordinary one-call stack fast path, with explicit alignment.
        let ready = unsafe { InitializeProcThreadAttributeList(stack.as_mut_ptr() as *mut c_void, count, 0, &mut bytes) } != 0;
        let storage = if ready {
            AttributeStorage::Stack(stack)
        } else {
            bytes = 0;
            let query = unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), count, 0, &mut bytes) };
            let error = unsafe { GetLastError() };
            if query != 0 || bytes == 0 || bytes > 1024 * 1024 {
                return Err(GuardError(format!("cannot size process creation attributes (Windows error {})", error)));
            }
            let blocks = bytes.div_ceil(std::mem::size_of::<AttributeBlock>());
            let mut heap = Vec::new();
            heap.try_reserve_exact(blocks).map_err(|_| GuardError("cannot allocate process creation attributes".into()))?;
            heap.resize_with(blocks, || AttributeBlock([0; 16]));
            AttributeStorage::Heap(heap)
        };
        let mut attributes = Self { storage, jobs, inherited, initialized: ready, delete_on_drop };
        unsafe {
            if !ready {
                checked(InitializeProcThreadAttributeList(attributes.as_mut_ptr(), count, 0, &mut bytes))?;
                attributes.initialized = true;
            }
            checked(UpdateProcThreadAttribute(attributes.as_mut_ptr(), 0, PROC_THREAD_ATTRIBUTE_JOB_LIST,
                attributes.jobs.as_mut_ptr() as *mut c_void, std::mem::size_of::<Handle>(),
                std::ptr::null_mut(), std::ptr::null_mut()))?;
            let list = attributes.as_mut_ptr();
            if let Some(handles) = attributes.inherited.as_deref_mut().filter(|handles| !handles.is_empty()) {
                checked(UpdateProcThreadAttribute(list, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                    handles.as_mut_ptr() as *mut c_void, std::mem::size_of_val(handles),
                    std::ptr::null_mut(), std::ptr::null_mut()))?;
            }
        }
        Ok(attributes)
    }

    pub(super) fn as_mut_ptr(&mut self) -> *mut c_void {
        match &mut self.storage {
            AttributeStorage::Stack(stack) => stack.as_mut_ptr() as *mut c_void,
            AttributeStorage::Heap(heap) => heap.as_mut_ptr() as *mut c_void,
        }
    }
}

impl Drop for JobAttributeList<'_> {
    fn drop(&mut self) {
        if self.initialized && self.delete_on_drop {
            unsafe { DeleteProcThreadAttributeList(self.as_mut_ptr()) };
        }
    }
}
