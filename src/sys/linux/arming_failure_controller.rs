// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

use super::*;
use std::result::Result;
use super::cgroup_authority::{ProvisionFailure, CreatedLeaf};
use super::cgroup_authority::settlement::{Deadline, RemovedUnactivatedLeaf, UnactivatedLeafQuarantine};
use super::terminal_controller::{ContextFailure, DedicatedSupervisor, EmptyContextReceipt,
    SettledRun, TerminalQuarantine};


use super::super::super::entry::{AuthenticatedCancellation,CancellationObservation,CancellationState};
#[derive(Clone,Copy)]
enum PeerSite{Entry,PreInternal,FinalNoInternal}
#[derive(Default)]
struct ArmingPeerFacts{
 binding_valid:bool,entry:Option<CancellationObservation>,pre_internal:Option<CancellationObservation>,final_no_internal:Option<CancellationObservation>,
 internal_requested:bool,first_nonclear:Option<(PeerSite,CancellationObservation)>,veto_peer:bool,veto_fault:bool,
 gate_required:bool,gate_previous:Option<c_int>,gate_invalid:Option<c_int>,gate_cause:Option<c_int>,pre_internal_cause:Option<c_int>,
}
impl ArmingPeerFacts{
 fn bound()->Self{Self{binding_valid:true,..Self::default()}}
 fn observe(&mut self,site:PeerSite,raw:CancellationObservation){
  let slot=match site{PeerSite::Entry=>&mut self.entry,PeerSite::PreInternal=>&mut self.pre_internal,PeerSite::FinalNoInternal=>&mut self.final_no_internal};
  if slot.is_some(){self.veto_fault=true;return;}*slot=Some(raw);
  match raw.state(){CancellationState::Clear=>{},CancellationState::Peer=>self.veto_peer=true,CancellationState::Fault=>self.veto_fault=true}
  if raw.state()!=CancellationState::Clear&&self.first_nonclear.is_none(){self.first_nonclear=Some((site,raw));}
 }
 fn gate_flag(&mut self,cause:c_int){match cause{0=>{},1=>self.veto_peer=true,_=>self.veto_fault=true}}
 fn observe_gate(&mut self,previous:Option<c_int>,invalid:Option<c_int>,cause:c_int){
  self.gate_required=true;
  if self.gate_previous.is_some()||self.gate_cause.is_some(){self.veto_fault=true;return;}
  self.gate_previous=previous;self.gate_invalid=invalid;self.gate_cause=Some(cause);
  if !previous.is_some_and(super::native_port::arming_gate_cancel_valid)||invalid.is_some(){self.veto_fault=true;}self.gate_flag(cause);
 }
 fn clear(raw:Option<CancellationObservation>)->bool{raw.is_some_and(|raw|raw.state()==CancellationState::Clear)}
 fn permits_timeout(&self)->bool{
  self.binding_valid&&!self.veto_peer&&!self.veto_fault&&Self::clear(self.entry)
   &&(!self.gate_required||(self.gate_previous.is_some_and(super::native_port::arming_gate_cancel_valid)&&self.gate_invalid.is_none()&&self.gate_cause==Some(0)))
   &&if self.internal_requested{Self::clear(self.pre_internal)&&self.pre_internal_cause==Some(0)&&self.final_no_internal.is_none()}
     else{Self::clear(self.final_no_internal)&&self.pre_internal.is_none()&&self.pre_internal_cause.is_none()}
 }
}
// The actual opaque endpoint, not a boolean/rawFD/relay, stays in every owner.
// These observations invalidate a primary timeout; they can never create one.
pub(super) struct ArmingPeerVeto{authority:Arc<AuthenticatedCancellation>,facts:ArmingPeerFacts}
impl ArmingPeerVeto{
 pub(super) fn new(authority:Arc<AuthenticatedCancellation>,prepared:&PreparedRun)->Self{
  let mut facts=ArmingPeerFacts::bound();let peer=authority.peer();
  facts.binding_valid=peer.pid==prepared.owner_pid&&peer.uid==prepared.identity.uid&&peer.gid==prepared.identity.gid;
  Self{authority,facts}
 }
 pub(super) fn bind_controls(&mut self,other:Option<&Arc<AuthenticatedCancellation>>){
  if !other.is_some_and(|other|Arc::ptr_eq(&self.authority,other)){self.facts.binding_valid=false;self.facts.veto_fault=true;}
 }
 pub(super) fn entry(&mut self){let raw=self.authority.sample();self.facts.observe(PeerSite::Entry,raw);}
 pub(super) fn gate(&mut self,previous:Option<c_int>,invalid:Option<c_int>,cause:c_int){self.facts.observe_gate(previous,invalid,cause);}
 pub(super) fn pre_internal(&mut self,other:&Arc<AuthenticatedCancellation>,raw:CancellationObservation,cause:c_int){
  self.bind_controls(Some(other));self.facts.observe(PeerSite::PreInternal,raw);self.facts.pre_internal_cause=Some(cause);self.facts.gate_flag(cause);
 }
 pub(super) fn mark_internal(&mut self){
  if self.facts.pre_internal.is_none()||self.facts.internal_requested{self.facts.veto_fault=true;}self.facts.internal_requested=true;
 }
 pub(super) fn finish_without_internal(&mut self){
  if self.facts.internal_requested{return;}let raw=self.authority.sample();self.facts.observe(PeerSite::FinalNoInternal,raw);
 }
 pub(super) fn permits_timeout(&self)->bool{self.facts.permits_timeout()}
}

// Preserve the real source failure; no synthetic release/exec/exit is attached.
enum BeforeChildCause {
    BeforeArm { error: String, decision:Option<Decision> },
    BeforeProvision { error: String },
    Provision(ProvisionFailure),
}

#[must_use = "before-child custody is not automatically settled by Drop"]
pub(super) struct BeforeChildOwner {
    prepared: PreparedRun,
    peer_veto: ArmingPeerVeto,
    arming: Option<native_arming::ArmingReceipt>,
    supervisor: Option<DedicatedSupervisor>,
    context_error: Option<ContextFailure>,
    cleanup: Option<Deadline>,
    cause: BeforeChildCause,
    empty_context: EmptyContextReceipt,
    leaf_empty_checked_ns: Option<u64>,
}

pub(super) enum BeforeChildFault {
    ConflictingContexts,
    ParentSignalMask,
    MissingContextOwner,
    AmbiguousCreate,
    UnanchoredLeaf,
    MissingCleanupAuthority,
    Deadline(String),
    Context(String),
    LeafValidation(String),
}

// This is successful settlement of a FAILED request, never execution success.
pub(super) struct BeforeChildSettled {
    owner: BeforeChildOwner,
    removed_leaf: Option<RemovedUnactivatedLeaf>,
}

pub(super) enum ArmingSettled {
    BeforeChildren(BeforeChildSettled),
    VerifiedDomain(SettledRun),
}

#[must_use = "failure quarantine owns every remaining authority and raw observation"]
pub(super) enum ArmingQuarantine {
    BeforeChildren { owner: BeforeChildOwner, fault: BeforeChildFault },
    DetachedCreatedLeaf { owner: BeforeChildOwner, created: CreatedLeaf, fault: BeforeChildFault },
    UnactivatedRemoval { owner: BeforeChildOwner, leaf: UnactivatedLeafQuarantine },
    VerifiedDomain(TerminalQuarantine),
}

impl BeforeChildOwner {
    fn new(
        prepared: PreparedRun,
        supervisor: Option<DedicatedSupervisor>,
        context_error: Option<ContextFailure>,
        cleanup: Option<Deadline>,
        cause: BeforeChildCause,
        cancellation:Arc<AuthenticatedCancellation>,
    ) -> Self {
        let peer_veto=ArmingPeerVeto::new(cancellation,&prepared);
        Self { prepared, peer_veto, arming: None, supervisor, context_error, cleanup, cause,
            empty_context: EmptyContextReceipt::default(), leaf_empty_checked_ns: None }
    }

    fn with_arming(mut self, receipt:native_arming::ArmingReceipt)->Self {
        self.arming=Some(receipt); self
    }

    fn leaf(&self) -> Result<Option<&CreatedLeaf>, BeforeChildFault> {
        match &self.cause {
            BeforeChildCause::Provision(failure) => {
                // A negative mkdir result must not be laundered into no leaf
                // or promoted to the identity of a possibly existing name.
                if failure.refused_create.is_some() { return Err(BeforeChildFault::AmbiguousCreate); }
                Ok(failure.created.as_ref())
            }
            _ => Ok(None),
        }
    }

    fn validate_unactivated_leaf(&mut self) -> Result<(), BeforeChildFault> {
        let Some(leaf) = self.leaf()? else { return Ok(()); };
        if !leaf.has_anchor() { return Err(BeforeChildFault::UnanchoredLeaf); }
        let deadline = self.cleanup.as_ref().ok_or(BeforeChildFault::MissingCleanupAuthority)?;
        deadline.check().map_err(BeforeChildFault::Deadline)?;
        leaf.validate_unactivated().map_err(BeforeChildFault::LeafValidation)?;
        self.leaf_empty_checked_ns = Some(deadline.check().map_err(BeforeChildFault::Deadline)?);
        Ok(())
    }

    fn settle_context(&mut self) -> Result<(), BeforeChildFault> {
        let deadline = self.cleanup.as_ref();
        match (self.supervisor.as_mut(), self.context_error.as_mut()) {
            (Some(_), Some(_)) => Err(BeforeChildFault::ConflictingContexts),
            (Some(owner), None) => owner.settle_before_children(deadline, &mut self.empty_context)
                .map_err(BeforeChildFault::Context),
            (None, Some(failure)) => match failure.owner.as_mut() {
                Some(owner) => owner.settle_before_children(deadline, &mut self.empty_context)
                    .map_err(BeforeChildFault::Context),
                // Setup may have refused an existing/unknown context. Never
                // restore a flag or claim empty custody without its real owner.
                None => Err(BeforeChildFault::MissingContextOwner),
            },
            (None, None) => DedicatedSupervisor::prove_untouched_before_arm(deadline, &mut self.empty_context)
                .map_err(BeforeChildFault::Context),
        }
    }

    fn fail(self, fault: BeforeChildFault) -> ArmingQuarantine {
        ArmingQuarantine::BeforeChildren { owner: self, fault }
    }

    fn settled(mut self,removed_leaf:Option<RemovedUnactivatedLeaf>)->ArmingSettled{
        self.peer_veto.finish_without_internal();
        ArmingSettled::BeforeChildren(BeforeChildSettled{owner:self,removed_leaf})
    }

    fn settle(mut self) -> Result<ArmingSettled, ArmingQuarantine> {
        self.peer_veto.entry();
        if let Err(fault) = self.validate_unactivated_leaf() { return Err(self.fail(fault)); }
        if let Err(fault) = self.settle_context() { return Err(self.fail(fault)); }
        if self.arming.as_ref().is_some_and(|receipt|receipt.parent_mask_uncertain()) {
            return Err(self.fail(BeforeChildFault::ParentSignalMask));
        }
        let created = match &mut self.cause {
            BeforeChildCause::Provision(failure) => failure.created.take(),
            _ => None,
        };
        let Some(created) = created else {
            return Ok(self.settled(None));
        };
        let deadline = match self.cleanup.take() {
            Some(value) => value,
            None => {
                return Err(ArmingQuarantine::DetachedCreatedLeaf {
                    owner: self, created, fault: BeforeChildFault::MissingCleanupAuthority,
                });
            }
        };
        // The private path just proved native no-child state/restoration and
        // an anchored never-activated leaf. Revalidate/unlink only that leaf
        // LAST, using the SAME original prearmed timer. No kill is invented.
        match unsafe { created.remove_unactivated_after_empty(deadline) } {
            Ok(receipt) => Ok(self.settled(Some(receipt))),
            Err(leaf) => Err(ArmingQuarantine::UnactivatedRemoval { owner: self, leaf }),
        }
    }
}

impl ActivationFailure {
    pub(super) fn settle_arming_failure(self,cancellation:Arc<AuthenticatedCancellation>) -> Result<ArmingSettled, ArmingQuarantine> {
        match self {
            ActivationFailure::BeforeArm { prepared, supervisor, cleanup, context_error, error, decision } => {
                BeforeChildOwner::new(prepared, supervisor, context_error, cleanup,
                    BeforeChildCause::BeforeArm { error, decision },cancellation).settle()
            }
            ActivationFailure::Arming { prepared, supervisor, cleanup, failure } => match failure {
                ArmFailure::VerifiedDomain { controls, watchdog, error, arming } => {
                    // Concrete native held kill is the first external operation
                    // in this branch. No formatting/clock/path gate precedes it.
                    terminal_controller::settle_verified_arming_failure(
                        prepared, supervisor, cleanup, controls, watchdog, error, arming,cancellation)
                        .map(ArmingSettled::VerifiedDomain)
                        .map_err(ArmingQuarantine::VerifiedDomain)
                }
                ArmFailure::BeforeProvision { error, arming } => BeforeChildOwner::new(
                    prepared, Some(supervisor), None, Some(cleanup),
                    BeforeChildCause::BeforeProvision { error },cancellation).with_arming(arming).settle(),
                ArmFailure::Provision { failure, arming } => BeforeChildOwner::new(
                    prepared, Some(supervisor), None, Some(cleanup),
                    BeforeChildCause::Provision(failure),cancellation).with_arming(arming).settle(),
            },
        }
    }
}

impl ArmingSettled{
 pub(super) fn permits_timeout(&self)->bool{match self{
  Self::BeforeChildren(value)=>value.owner.peer_veto.permits_timeout()&&match &value.owner.cause{
   BeforeChildCause::BeforeArm{decision:Some(Decision::Deadline),..}=>value.owner.arming.is_none(),
   BeforeChildCause::BeforeProvision{..}=>value.owner.arming.as_ref().is_some_and(native_arming::ArmingReceipt::permits_timeout),
   _=>false,
  },
  Self::VerifiedDomain(run)=>run.arming_timeout_eligible(),
 }}
}

#[cfg(test)]pub(super) fn peer_veto_regression(){
 let clear=CancellationObservation{returned:0,errno:0,revents:0};let peer=CancellationObservation{returned:1,errno:0,revents:1};
 let complete=|first:CancellationObservation,last:CancellationObservation|{let mut facts=ArmingPeerFacts::bound();facts.observe(PeerSite::Entry,first);facts.observe(PeerSite::FinalNoInternal,last);facts};
 assert!(complete(clear,clear).permits_timeout());assert!(!ArmingPeerFacts::bound().permits_timeout());
 for raw in [peer,CancellationObservation{returned:-1,errno:4,revents:0},CancellationObservation{returned:2,errno:0,revents:1},CancellationObservation{returned:0,errno:0,revents:1},CancellationObservation{returned:1,errno:0,revents:8}]{
  let first=complete(raw,clear);assert!(!first.permits_timeout());let (_,saved)=first.first_nonclear.expect("first nonclear raw fact retained");assert_eq!((saved.returned,saved.errno,saved.revents),(raw.returned,raw.errno,raw.revents));
  assert!(!complete(clear,raw).permits_timeout());
 }
 let mut generic_first=complete(peer,clear);generic_first.observe_gate(Some(2),None,0);assert!(!generic_first.permits_timeout());
 let mut internal=ArmingPeerFacts::bound();internal.observe(PeerSite::Entry,clear);internal.observe_gate(Some(2),None,0);internal.observe(PeerSite::PreInternal,clear);internal.pre_internal_cause=Some(0);internal.internal_requested=true;
 assert!(internal.permits_timeout());assert!(internal.final_no_internal.is_none());
 internal.observe(PeerSite::FinalNoInternal,peer);assert!(!internal.permits_timeout()); // impossible self-induced observation cannot grant eligibility
 let mut cancelled=ArmingPeerFacts::bound();cancelled.observe(PeerSite::Entry,clear);cancelled.observe_gate(Some(2),None,0);cancelled.observe(PeerSite::PreInternal,peer);cancelled.pre_internal_cause=Some(0);cancelled.internal_requested=true;assert!(!cancelled.permits_timeout());
 for previous in [None,Some(-1),Some(1)]{let mut raw=complete(clear,clear);raw.observe_gate(previous,None,0);assert!(!raw.permits_timeout());}
 for cause in [-1,1,2]{let mut raw=complete(clear,clear);raw.observe_gate(Some(2),None,cause);assert!(!raw.permits_timeout());}
 for invalid in [1,-1,9]{let mut raw=complete(clear,clear);raw.observe_gate(Some(0),Some(invalid),0);assert!(!raw.permits_timeout());assert_eq!(raw.gate_invalid,Some(invalid));}
 let mut wrong=complete(clear,clear);wrong.binding_valid=false;assert!(!wrong.permits_timeout());
 let mut missing=ArmingPeerFacts::bound();missing.observe(PeerSite::Entry,clear);assert!(!missing.permits_timeout());missing.internal_requested=true;assert!(!missing.permits_timeout());
 println!("arming_peer_veto all_outcome_shapes=true generic_first_peer_veto=true sticky_fault=true no_capability_mint_or_native_calls=true");
}
