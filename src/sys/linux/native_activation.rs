// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

use super::*;

const CLONE_PIDFD: u64 = 0x0000_1000;
const CLONE_INTO_CGROUP: u64 = 0x2_0000_0000;
const SIGCHLD: u64 = 17;

struct LeaseWatch<'a> {
    owner_pid: c_int,
    owners: &'a Arc<VerifiedInvocation>,
    timer: &'a OwnedFd,
    deadline_ns: u64,
    supervisor: &'a OwnedFd,
    cancellation:&'a Arc<super::super::entry::AuthenticatedCancellation>,
}

impl<'a> LeaseWatch<'a> {
    fn original(prepared: &'a PreparedRun, supervisor: &'a terminal_controller::DedicatedSupervisor,cancellation:&'a Arc<super::super::entry::AuthenticatedCancellation>) -> Self {
        Self {
            owner_pid: prepared.owner_pid,
            owners: &prepared.lease.invocation,
            timer: &prepared.lease.timer,
            deadline_ns: prepared.lease.deadline_ns,
            supervisor: supervisor.self_pidfd(),
            cancellation,
        }
    }
}

// The trusted provisioner supplies these from one verified fresh cgroup2
// domain. These are owned descriptors, never paths from the request.
struct DomainControls {
    cancellation:Option<Arc<super::super::entry::AuthenticatedCancellation>>,
    peer_cancel:std::cell::Cell<Option<(c_int,c_int)>>,
    gate: Option<native_port::SharedGate>,
    gate_cancel: std::cell::Cell<Option<c_int>>,
    gate_cancel_invalid: std::cell::Cell<Option<c_int>>,
    gate_release: Option<(c_int,c_int)>,
    directory: OwnedFd,
    watchdog_kill: OwnedFd,
    supervisor_kill: OwnedFd,
    events: OwnedFd,
    provenance: cgroup_authority::LeafProvenance,
}

struct ArmedDomain {
    controls: DomainControls,
    watchdog: DirectChild,
    arming: native_arming::ArmingReceipt,
}

// These variants retain the actual factory/domain authorities. They describe
// the trusted port's obligations; native_arming supplies the watchdog edge only.
enum ArmFailure {
    // Contract: no domain, watchdog or payload resource has been created.
    BeforeProvision { error: String, arming: native_arming::ArmingReceipt },
    // Retains the factory's actual optional CreatedLeaf after a partial mkdir.
    Provision { failure: cgroup_authority::ProvisionFailure, arming: native_arming::ArmingReceipt },
    // A watchdog may be created only after verified controls exist. If one
    // exists, its direct-child ownership must be returned even on readiness error.
    VerifiedDomain {
        controls: DomainControls,
        watchdog: Option<DirectChild>,
        error: String,
        arming: native_arming::ArmingReceipt,
    },
}

impl ArmFailure{
    fn has_primary_deadline(&self)->bool{match self{
        Self::BeforeProvision{arming,..}|Self::Provision{arming,..}|Self::VerifiedDomain{arming,..}=>arming.has_primary_deadline(),
    }}
}

struct ChildPlan<'a> {
    directory: &'a OwnedFd,
    identity: &'a VerifiedIdentity,
    exec: &'a request::ExecPlan,
}

struct DirectChild {
    pid: c_int,
    pidfd: OwnedFd,
}

impl DirectChild {
    /// # Safety
    /// The trusted native port must supply the positive PID and newly owned
    /// pidfd from the same successful direct-child clone, with exclusive wait
    /// ownership. No caller-supplied PID/descriptor may enter this constructor.
    /// This record must remain owned until exact reaping; dropping it does not
    /// terminate or reap the child. native_arming supplies the trusted watchdog clone;
    /// native_port supplies the actual CLONE_INTO_CGROUP payload edge.
    unsafe fn from_successful_clone(pid: c_int, pidfd: OwnedFd) -> Self {
        Self { pid, pidfd }
    }
}

struct PlacedChild {
    identity: DirectChild,
    ready_permit: Probe<OwnedFd>,
    // The same child's trusted CLOEXEC writer reports a fixed exec-error frame.
    // Reader remains owned even when readiness/permit fails. EOF alone is not
    // proof that the permit completed execution.
    exec_report: terminal_controller::ExecReport,
}

struct PayloadChild {
    identity: DirectChild,
    exec_report: terminal_controller::ExecReport,
}

// native_port implements real factory/watchdog, atomic payload placement and
// held-to-released bootstrap. Product selection still awaits qualification.
// The port is inside the privileged trust boundary. It is not request input.
trait OneShotPort {
    fn caller_cancellation(&self)->&Arc<super::super::entry::AuthenticatedCancellation>;
    // Apply the command policy to the original decoded plan before arming.
    // This check must create no domain or child resources.
    // The first native proof may authorize only the pinned absolute fixture.
    fn authorize(&mut self, identity: &VerifiedIdentity, exec: &request::ExecPlan) -> Probe<()>;

    // Arm the proof's watchdog with the original owner, immutable timer and
    // exact supervisor pidfd opened by the supervisor BEFORE watchdog clone.
    // Supervisor death must cancel even while the original owner remains live;
    // never open a numeric getppid() afterward (reparent/reuse race). The
    // watchdog must not consume/disarm/retarget the original timer. Return
    // only once both watchdog and independent supervisor kill paths own access
    // to the same verified domain. No payload may exist during arm. Every Err
    // retains any actual CreatedLeaf/controls and direct watchdog child. A bare
    // BeforeProvision relies on its no-resource contract; Provision retains
    // the factory's exact result, including whether a leaf was created.
    fn arm(
        &mut self,
        watch: LeaseWatch<'_>,
    ) -> std::result::Result<ArmedDomain, ArmFailure>;

    // Place atomically and retain the kernel-returned pidfd. An outer Err
    // means no clone succeeded. After a clone, return the exact child even
    // when readiness fails. A ready permit certifies trusted child readiness,
    // FD scrubbing and credential drop. A readiness error closes the writer.
    fn place_held(&mut self, domain: &ArmedDomain, child: ChildPlan<'_>) -> Probe<PlacedChild>;

    // Use the proof's held-to-released shared gate and its single permit byte.
    // An independent cancellation can win at either side of this observation.
    // Do not duplicate or retain the borrowed permit writer.
    fn payload_parent_mask(&self)->Option<native_port::ParentMaskObservation>;

    fn commit_permit(
        &mut self,
        domain: &ArmedDomain,
        child: &DirectChild,
        permit_writer: &OwnedFd,
    ) -> Probe<()>;
}

struct ActiveRun {
    cancellation:Arc<super::super::entry::AuthenticatedCancellation>,
    payload_mask: Option<native_port::ParentMaskObservation>,
    prepared: PreparedRun,
    domain: ArmedDomain,
    child: Option<PayloadChild>,
    cleanup: cgroup_authority::settlement::Deadline,
    supervisor: terminal_controller::DedicatedSupervisor,
}

impl ActiveRun {
    fn place_and_sample_release(&mut self, port: &mut impl OneShotPort) -> Probe<Decision> {
        self.domain.controls.revalidate()?;
        let decision = self.prepared.lease.sample()?.decision;
        if decision != Decision::Permit {
            return Ok(decision);
        }

        let placed = port.place_held(
            &self.domain,
            ChildPlan {
                directory: &self.domain.controls.directory,
                identity: &self.prepared.identity,
                exec: &self.prepared.plan,
            },
        );
        // Record the last attempted raw mask even when clone did NOT succeed.
        self.payload_mask=port.payload_parent_mask();
        let PlacedChild { identity, ready_permit, exec_report }=placed?;
        let child = self.child.insert(PayloadChild { identity, exec_report });
        let permit_writer = ready_permit?;

        // The original timer and owner remain live in the watchdog while the
        // supervisor samples them again at the last release boundary.
        let decision = self.prepared.lease.sample()?.decision;
        if decision == Decision::Permit {
            self.domain.controls.revalidate()?;
            port.commit_permit(&self.domain, &child.identity, &permit_writer)?;
        }
        drop(permit_writer);
        Ok(decision)
    }
}

#[must_use = "an armed activation requires watchdog settlement or supervisor fallback"]
struct PendingSettlement {
    active: ActiveRun,
    // Permit records gate release only. Exec and terminal outcome are independent.
    release: Probe<Decision>,
}

// The original request/identity/owner/timer remain owned on returned activation
// errors. Do not Debug/Display this value: it contains caller argv and identity.
// There is no automatic Drop settlement or claim of complete native custody.
enum ActivationFailure {
    BeforeArm {
        prepared: PreparedRun,
        supervisor: Option<terminal_controller::DedicatedSupervisor>,
        cleanup: Option<cgroup_authority::settlement::Deadline>,
        context_error: Option<terminal_controller::ContextFailure>,
        error: String,
        decision: Option<Decision>,
    },
    Arming {
        prepared: PreparedRun,
        supervisor: terminal_controller::DedicatedSupervisor,
        cleanup: cgroup_authority::settlement::Deadline,
        failure: ArmFailure,
    },
}

// Only this actual refusal-boundary observation creates a typed pre-arm stop.
// Later diagnostic samples cannot replace a primary ABI/policy/context error.
fn require_original_live(prepared:&PreparedRun,error:&str)->std::result::Result<(),(String,Option<Decision>)> {
    let decision=prepared.lease.sample().map_err(|error|(error,None))?.decision;
    if decision==Decision::Permit{Ok(())}else{Err((error.into(),Some(decision)))}
}

/// # Safety
/// Caller is the dedicated one-shot supervisor entry point. No foreign child,
/// concurrent clone/waiter or thread may be introduced for its entire lifetime.
/// The trusted port may create only the returned watchdog/payload children.
/// An independent finite enclosing process authority must already cover setup
/// and failure cleanup, including failure to create the cleanup timer itself.
/// This adapter remains unconnected to the product entry point.
unsafe fn activate(
    prepared: PreparedRun,
    port: &mut impl OneShotPort,
) -> std::result::Result<PendingSettlement, ActivationFailure> {
    let before_arm=(||->std::result::Result<(),(String,Option<Decision>)>{
        port.authorize(&prepared.identity,&prepared.plan).map_err(|error|(error,None))?;
        require_original_live(&prepared,"owner or deadline inactive before cgroup arming")
    })();
    if let Err((error,decision))=before_arm {
        return Err(ActivationFailure::BeforeArm{prepared,supervisor:None,cleanup:None,context_error:None,error,decision});
    }
    let supervisor = match unsafe { terminal_controller::DedicatedSupervisor::establish() } {
        Ok(value) => value,
        Err(failure) => return Err(ActivationFailure::BeforeArm {
            prepared, supervisor: None, cleanup: None, context_error: Some(failure),
            error: "dedicated supervisor setup refused".into(), decision:None,
        }),
    };
    let cleanup_until = match prepared.lease.deadline_ns.checked_add(terminal_controller::CLEANUP_GRACE_NS) {
        Some(value) => value,
        None => return Err(ActivationFailure::BeforeArm {
            prepared, supervisor: Some(supervisor), cleanup: None, context_error: None,
            error: "cleanup hard cutoff overflow".into(), decision:None,
        }),
    };
    let cleanup = match cgroup_authority::settlement::Deadline::arm_before_placement(cleanup_until) {
        Ok(value) => value,
        Err(error) => return Err(ActivationFailure::BeforeArm {
            prepared, supervisor: Some(supervisor), cleanup: None, context_error: None, error, decision:None,
        }),
    };
    let final_before_arm=(||->std::result::Result<(),(String,Option<Decision>)>{
        cleanup.check().map_err(|error|(error,None))?;
        require_original_live(&prepared,"original owner/deadline inactive before native arming")
    })();
    if let Err((error,decision))=final_before_arm {
        return Err(ActivationFailure::BeforeArm{prepared,supervisor:Some(supervisor),cleanup:Some(cleanup),context_error:None,error,decision});
    }
    // Original execution owner/timer/deadline are never retargeted. The cleanup
    // timer exists before arm can create a watchdog or place any payload.
    let cancellation=Arc::clone(port.caller_cancellation());
    let domain = match port.arm(LeaseWatch::original(&prepared, &supervisor,&cancellation)) {
        Ok(domain) => domain,
        Err(failure) => return Err(ActivationFailure::Arming {
            prepared, supervisor, cleanup, failure,
        }),
    };
    let mut active = ActiveRun {
        cancellation,
        payload_mask: None,
        prepared,
        domain,
        child: None,
        cleanup,
        supervisor,
    };
    let release = active.place_and_sample_release(port);
    // Even a placement, observation or permit error retains the original
    // lease and armed authorities for bounded drain and reap by the caller.
    Ok(PendingSettlement { active, release })
}

#[path = "cgroup_authority.rs"]
mod cgroup_authority;

#[path = "child_reaper.rs"]
mod child_reaper;

#[path = "terminal_controller.rs"]
mod terminal_controller;

#[path = "arming_failure_controller.rs"]
mod arming_failure_controller;

#[path = "native_arming.rs"]
mod native_arming;

#[path="native_port.rs"]
mod native_port;
pub(super) use native_port::LaunchContext;
pub(crate) use native_port::RunOutcome;
pub(super) use native_port::run_admitted;
pub(super) use native_port::before_activation;
pub(super) use native_port::{before_admission,before_admission_cause};

pub(crate) use cgroup_authority::service_scope::VerifiedServiceScope;
pub(crate) fn capture_service_scope()->Probe<VerifiedServiceScope> {
    VerifiedServiceScope::capture_checked_entry()
}

pub(crate) use cgroup_authority::service_scope::EntryKernelAuthority;
pub(crate) type ProtectedElfObservation=cgroup_authority::service_scope::ProtectedElfObservation;
pub(crate) fn capture_entry_kernel_authority()->Probe<EntryKernelAuthority>{EntryKernelAuthority::capture()}
pub(crate) fn require_initial_client_mapping()->Probe<()>{cgroup_authority::service_scope::require_initial_mappings()}

pub(crate) fn client_installation()->Probe<Vec<OwnedFd>>{cgroup_authority::service_scope::client_installation()}
pub(crate) use cgroup_authority::service_scope::{ClientCompletion,RetiredUnit};
pub(crate) fn client_events(fd:&OwnedFd,group:&str)->Probe<ClientCompletion>{cgroup_authority::service_scope::client_events(fd,group)}
#[cfg(test)]pub(crate) fn completion_read_regression(fd:RawFd){cgroup_authority::service_scope::completion_read_regression(fd)}


pub(crate) fn verify_runtime_parent(fds:&[OwnedFd])->Probe<()>{cgroup_authority::service_scope::verify_runtime_parent(fds)}

pub(crate) use cgroup_authority::service_scope::PublicBusEndpoint;

pub(crate) use cgroup_authority::service_scope::{PrivateRuntimeDirectory,PendingNotifyAuthority};

#[cfg(test)]pub(crate) fn gate_first_cause_regression(){native_port::gate_first_cause_regression()}

#[cfg(test)]pub(crate) fn watchdog_fault_regression(){terminal_controller::watchdog_fault_regression();native_port::watchdog_mapping_regression();native_arming::deadline_refusal_regression();arming_failure_controller::peer_veto_regression()}

pub(crate) fn capture_calling_umask()->Probe<u16>{cgroup_authority::service_scope::capture_calling_umask()}

#[cfg(test)]pub(crate) fn mode2_host_policy_regression(){native_port::mode2_host_policy_regression()}
