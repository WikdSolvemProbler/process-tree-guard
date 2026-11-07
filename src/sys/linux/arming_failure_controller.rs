// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

use super::*;
use std::result::Result;
use super::cgroup_authority::{ProvisionFailure, CreatedLeaf};
use super::cgroup_authority::settlement::{Deadline, RemovedUnactivatedLeaf, UnactivatedLeafQuarantine};
use super::terminal_controller::{ContextFailure, DedicatedSupervisor, EmptyContextReceipt,
    SettledRun, TerminalQuarantine};

// Preserve the real source failure; no synthetic release/exec/exit is attached.
enum BeforeChildCause {
    BeforeArm { error: String },
    BeforeProvision { error: String },
    Provision(ProvisionFailure),
}

#[must_use = "before-child custody is not automatically settled by Drop"]
pub(super) struct BeforeChildOwner {
    prepared: PreparedRun,
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
    ) -> Self {
        Self { prepared, arming: None, supervisor, context_error, cleanup, cause,
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

    fn settle(mut self) -> Result<ArmingSettled, ArmingQuarantine> {
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
            return Ok(ArmingSettled::BeforeChildren(BeforeChildSettled { owner: self, removed_leaf: None }));
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
            Ok(receipt) => Ok(ArmingSettled::BeforeChildren(BeforeChildSettled {
                owner: self, removed_leaf: Some(receipt),
            })),
            Err(leaf) => Err(ArmingQuarantine::UnactivatedRemoval { owner: self, leaf }),
        }
    }
}

impl ActivationFailure {
    pub(super) fn settle_arming_failure(self) -> Result<ArmingSettled, ArmingQuarantine> {
        match self {
            ActivationFailure::BeforeArm { prepared, supervisor, cleanup, context_error, error } => {
                BeforeChildOwner::new(prepared, supervisor, context_error, cleanup,
                    BeforeChildCause::BeforeArm { error }).settle()
            }
            ActivationFailure::Arming { prepared, supervisor, cleanup, failure } => match failure {
                ArmFailure::VerifiedDomain { controls, watchdog, error, arming } => {
                    // Concrete native held kill is the first external operation
                    // in this branch. No formatting/clock/path gate precedes it.
                    terminal_controller::settle_verified_arming_failure(
                        prepared, supervisor, cleanup, controls, watchdog, error, arming)
                        .map(ArmingSettled::VerifiedDomain)
                        .map_err(ArmingQuarantine::VerifiedDomain)
                }
                ArmFailure::BeforeProvision { error, arming } => BeforeChildOwner::new(
                    prepared, Some(supervisor), None, Some(cleanup),
                    BeforeChildCause::BeforeProvision { error }).with_arming(arming).settle(),
                ArmFailure::Provision { failure, arming } => BeforeChildOwner::new(
                    prepared, Some(supervisor), None, Some(cleanup),
                    BeforeChildCause::Provision(failure)).with_arming(arming).settle(),
            },
        }
    }
}
