// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Original public invocation: captured before helpers, with no numeric-PID reopen
//! after transfer. Raw wire metadata is not privileged timestamp attestation.
use super::{Credentials,SeparateService};
use std::mem::size_of;
use super::invocation_native as native;

#[repr(C)]
#[derive(Clone,Copy)]
pub(super) struct OwnerWatch {pub(super) fds:*const c_int,pub(super) count:u32,pub(super) reserved:u32}
const _:()={assert!(size_of::<OwnerWatch>()==16);assert!(std::mem::offset_of!(OwnerWatch,count)==8);};

fn calling_umask() -> std::result::Result<u16,String> {
    super::supervisor::native_activation::capture_calling_umask()
}

include!("invocation_capture.rs");

impl InvocationError {
    pub(crate) fn exit_code(&self)->i32 {if *self==Self::Expired {124}else{125}}
}
impl OriginalInvocation {
    pub(super) fn acknowledge_separate_service(mut self,_acknowledgment:SeparateService)->Self {self.metadata.separate_service=true;self}
}

#[must_use="verified original owner set must remain held through terminal/quarantine"]
#[derive(Debug)]
pub(super) struct VerifiedInvocation {metadata:Metadata,pidfds:Vec<OwnedFd>,numbers:Vec<c_int>}
impl VerifiedInvocation {
    pub(super) fn receive(metadata:Metadata,pidfds:Vec<OwnedFd>,peer:Credentials)->CaptureResult<Self> {
        metadata.validate()?;
        if metadata.identities[0].pid!=peer.pid as u32{return Err(InvocationError::IdentityChanged);}
        if namespaces(peer.pid as u32)?!=namespaces(std::process::id())?{return Err(InvocationError::Namespace);}
        verify_chain(&metadata,&pidfds)?;
        validate_time(&metadata,clock()?)?;
        let numbers=pidfds.iter().map(AsRawFd::as_raw_fd).collect();
        Ok(Self{metadata,pidfds,numbers})
    }
    pub(super) fn requester(&self)->&OwnedFd {&self.pidfds[0]}
    pub(super) fn metadata(&self)->&Metadata {&self.metadata}
    pub(super) fn owners(&self)->&[OwnedFd] {&self.pidfds}
    pub(super) fn numbers(&self)->&[c_int] {&self.numbers}
    pub(super) fn watch(&self)->OwnerWatch {OwnerWatch{fds:self.numbers.as_ptr(),count:self.numbers.len() as u32,reserved:0}}
    pub(super) fn check_live(&self)->CaptureResult<()> {live_set(&self.pidfds)}
    pub(super) fn check_time(&self)->CaptureResult<()> {validate_time(&self.metadata,clock()?)}
}
