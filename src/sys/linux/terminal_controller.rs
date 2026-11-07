// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

use super::*;
use super::cgroup_authority::settlement::{Deadline, DrainedDomain, KilledDomain,
    KillOutcome, RemovedDomain, SettlementQuarantine};
use super::child_reaper::{AnyFailure, AnyState, AnyWaitObservation, ExitOutcome,
    ReapFailure, ReapedChild, ReapStep};
use std::marker::PhantomData;
use std::rc::Rc;
use std::result::Result;

pub(super) const CLEANUP_GRACE_NS: u64 = 5_000_000_000;
const MAX_ANY_OBSERVATIONS: usize = 4096;
const ANY_RESERVE_CHUNK: usize = 64;
const _: () = {
    assert!(std::mem::size_of::<PollFd>() == 8);
    assert!(std::mem::offset_of!(PollFd, events) == 4);
    assert!(std::mem::offset_of!(PollFd, revents) == 6);
};

unsafe extern "C" {
    fn ptg_terminal_self(which: u32) -> c_int;
    fn ptg_terminal_self_pidfd(error: *mut c_int) -> c_int;
    fn ptg_terminal_sigchld(facts: *mut u32, error: *mut c_int) -> c_int;
    fn ptg_terminal_subreaper(operation: u32, value: *mut c_int, error: *mut c_int) -> c_int;
    fn ptg_terminal_kill(pidfd: c_int, error: *mut c_int) -> c_int;
    fn ptg_terminal_errno_kind(error: c_int) -> u32;
    fn ptg_terminal_poll(fds: *mut c_void, count: u32, timeout: c_int, error: *mut c_int) -> c_int;
    fn ptg_terminal_exec_read(fd: c_int, bytes: *mut u8, capacity: u32,
                              count: *mut c_int, error: *mut c_int) -> u32;
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct NativeCall { result: c_int, errno: c_int }

#[derive(Clone, Copy)]
pub(super) struct ContextPreflight {
    pid: c_int,
    tid: c_int,
    signal: NativeCall,
    sigchld_facts: u32,
    original_get: NativeCall,
    original_flag: c_int,
}

fn subreaper(operation: u32) -> (NativeCall, c_int) {
    let mut value = -1;
    let mut error = 0;
    let result = unsafe { ptg_terminal_subreaper(operation, &mut value, &mut error) };
    (NativeCall { result, errno: error }, value)
}

fn single_thread() -> Probe<()> {
    let mut count = 0usize;
    for entry in io_result(fs::read_dir("/proc/self/task"))? {
        io_result(entry)?;
        count += 1;
    }
    check(count == 1, "dedicated supervisor must have one native thread")
}

// This token is process-local, not Send/Sync and not a reusable server context.
// It does not prove the unsafe entry contract by type alone. The actual entry
// setup checks its thread, signal state, original flag and non-consuming ECHILD.
#[must_use = "dedicated subreaper state must remain owned through terminal settlement"]
pub(super) struct DedicatedSupervisor {
    pid: c_int,
    tid: c_int,
    self_owner: OwnedFd,
    entry_probe: AnyWaitObservation,
    sigchld_facts: u32,
    original_flag: c_int,
    last_set: NativeCall,
    last_get: NativeCall,
    last_readback: c_int,
    active: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(super) struct ContextFailure {
    pub(super) owner: Option<DedicatedSupervisor>,
    pub(super) probe: Option<AnyWaitObservation>,
    pub(super) error: String,
    pub(super) wait_error: Option<AnyFailure>,
    pub(super) preflight: ContextPreflight,
}

// Raw storage for the no-child cleanup path. None means a call did not occur;
// zero/default values are never synthesized as successful native observations.
#[derive(Default)]
pub(super) struct EmptyContextReceipt {
    preflight: Option<ContextPreflight>,
    prior_owner_state: Option<(NativeCall, NativeCall, c_int, bool)>,
    before_probe: Option<AnyWaitObservation>,
    after_probe: Option<AnyWaitObservation>,
    wait_error: Option<AnyFailure>,
    restore_set: Option<NativeCall>,
    restore_get: Option<(NativeCall, c_int)>,
    before_ns: Option<u64>,
    after_ns: Option<u64>,
    flag_at_original: bool,
    complete: bool,
}

fn before_child_preflight() -> ContextPreflight {
    let pid = unsafe { ptg_terminal_self(0) };
    let tid = unsafe { ptg_terminal_self(1) };
    let mut facts = 0;
    let mut error = 0;
    let result = unsafe { ptg_terminal_sigchld(&mut facts, &mut error) };
    let (get, flag) = subreaper(0);
    ContextPreflight { pid, tid, signal: NativeCall { result, errno: error },
        sigchld_facts: facts, original_get: get, original_flag: flag }
}

fn require_empty_probe(receipt: &mut EmptyContextReceipt, after: bool) -> Probe<()> {
    let observation = match child_reaper::peek_entry_children() {
        Ok(value) => value,
        Err(error) => {
            receipt.wait_error = Some(error);
            return Err("no-child non-consuming wait proof failed".into());
        }
    };
    let empty = observation.peek && matches!(observation.classified, Some(AnyState::NoChildren));
    if after { receipt.after_probe = Some(observation); }
    else { receipt.before_probe = Some(observation); }
    check(empty, "no-child cleanup encountered a pending/reapable or unverified child")
}

fn require_before_child_facts(facts: ContextPreflight, flag: c_int) -> Probe<()> {
    check(facts.pid > 0 && facts.tid == facts.pid, "no-child cleanup is not its dedicated main thread")?;
    single_thread()?;
    check(facts.signal.result == 0 && facts.signal.errno == 0 && facts.sigchld_facts == 7
        && facts.original_get.result == 0 && facts.original_get.errno == 0
        && facts.original_flag == flag, "no-child cleanup context state differs")
}

impl DedicatedSupervisor {
    // Before establish() was called, this checks an untouched empty context.
    // It performs no mutation, timer allocation, poll or blocking wait. The
    // unsafe dedicated one-shot entry still requires an independent finite
    // enclosing process authority, including when cleanup timer creation failed.
    pub(super) fn prove_untouched_before_arm(
        deadline: Option<&Deadline>,
        receipt: &mut EmptyContextReceipt,
    ) -> Probe<()> {
        if let Some(deadline) = deadline { receipt.before_ns = Some(deadline.check()?); }
        let facts = before_child_preflight();
        receipt.preflight = Some(facts);
        require_before_child_facts(facts, 0)?;
        require_empty_probe(receipt, false)?;
        receipt.flag_at_original = true;
        if let Some(deadline) = deadline { receipt.after_ns = Some(deadline.check()?); }
        receipt.complete = true;
        Ok(())
    }

    pub(super) fn settle_before_children(
        &mut self,
        deadline: Option<&Deadline>,
        receipt: &mut EmptyContextReceipt,
    ) -> Probe<()> {
        receipt.prior_owner_state = Some((self.last_set, self.last_get, self.last_readback, self.active));
        if let Some(deadline) = deadline { receipt.before_ns = Some(deadline.check()?); }
        check(self.original_flag == 0 && self.sigchld_facts == 7
            && self.entry_probe.peek
            && matches!(self.entry_probe.classified, Some(AnyState::NoChildren)),
            "original no-child context facts are not verified")?;
        let facts = before_child_preflight();
        receipt.preflight = Some(facts);
        check(facts.pid == self.pid && facts.tid == self.tid, "no-child owner process/thread changed")?;
        require_before_child_facts(facts, i32::from(self.active))?;
        require_empty_probe(receipt, false)?;
        if self.active {
            let (set, _) = subreaper(2);
            receipt.restore_set = Some(set);
            self.last_set = set;
            let (get, value) = subreaper(0);
            receipt.restore_get = Some((get, value));
            self.last_get = get;
            self.last_readback = value;
            if get.result == 0 && get.errno == 0 && value == self.original_flag { self.active = false; }
            check(set.result == 0 && set.errno == 0 && get.result == 0 && get.errno == 0
                && value == self.original_flag, "empty pre-child context restoration refused")?;
        }
        receipt.flag_at_original = !self.active;
        require_empty_probe(receipt, true)?;
        if let Some(deadline) = deadline { receipt.after_ns = Some(deadline.check()?); }
        receipt.complete = true;
        Ok(())
    }

    /// # Safety
    /// This is the dedicated one-shot process before any port child exists.
    /// No other thread, clone path, signal waiter, or wait caller may run now
    /// or later. The trusted port creates only the returned watchdog/payload.
    pub(super) unsafe fn establish() -> Result<Self, ContextFailure> {
        let pid = unsafe { ptg_terminal_self(0) };
        let tid = unsafe { ptg_terminal_self(1) };
        let mut facts = 0;
        let mut error = 0;
        let result = unsafe { ptg_terminal_sigchld(&mut facts, &mut error) };
        let (get, original_flag) = subreaper(0);
        let preflight = ContextPreflight { pid, tid, signal: NativeCall { result, errno: error },
            sigchld_facts: facts, original_get: get, original_flag };
        let initial = (|| -> Probe<()> {
            check(pid > 0 && tid == pid, "one-shot supervisor must be its process main thread")?;
            single_thread()?;
            check(result == 0 && error == 0 && facts == 7,
                  "SIGCHLD must be default/unblocked without SA_NOCLDWAIT")?;
            check(get.result == 0 && get.errno == 0 && original_flag == 0,
                  "preexisting or unverified subreaper context refused")
        })();
        if let Err(error) = initial {
            return Err(ContextFailure { owner: None, probe: None, error, wait_error: None, preflight });
        }
        let probe = match child_reaper::peek_entry_children() {
            Ok(value) => value,
            Err(wait_error) => return Err(ContextFailure {
                owner: None, probe: None, error: "initial non-consuming child probe refused".into(),
                wait_error: Some(wait_error),
                preflight,
            }),
        };
        if !matches!(probe.classified, Some(AnyState::NoChildren)) || !probe.peek {
            return Err(ContextFailure { owner: None, probe: Some(probe),
                error: "existing pending/reapable child refused without consuming its status".into(),
                wait_error: None, preflight });
        }
        let mut error = 0;
        let descriptor = unsafe { ptg_terminal_self_pidfd(&mut error) };
        if descriptor < 0 || error != 0 {
            return Err(ContextFailure { owner: None, probe: Some(probe),
                error: format!("opening supervisor's own pidfd failed: {error}"), wait_error: None, preflight });
        }
        // Kernel-created pidfd for getpid(), never a caller-supplied handle.
        let self_owner = unsafe { OwnedFd::from_raw_fd(descriptor) };
        if let Err(error) = native(close_on_exec(self_owner.as_raw_fd())) {
            return Err(ContextFailure { owner: None, probe: Some(probe), error, wait_error: None, preflight });
        }
        let mut owner = Self {
            pid, tid, self_owner, entry_probe: probe, sigchld_facts: facts, original_flag,
            last_set: NativeCall::default(), last_get: get, last_readback: original_flag,
            active: false, _not_send_or_sync: PhantomData,
        };
        let (set, _) = subreaper(1);
        owner.last_set = set;
        // Retain possible native state before another syscall/format can fail.
        owner.active = set.result == 0;
        let (readback, value) = subreaper(0);
        owner.last_get = readback;
        owner.last_readback = value;
        if set.result != 0 || set.errno != 0 || readback.result != 0 || value != 1 {
            return Err(ContextFailure { owner: Some(owner), probe: None,
                error: "subreaper setup/readback failed with actual context retained".into(), wait_error: None, preflight });
        }
        Ok(owner)
    }

    pub(super) fn self_pidfd(&self) -> &OwnedFd { &self.self_owner }

    fn validate(&self) -> Probe<()> {
        check(self.active && self.original_flag == 0 && self.sigchld_facts == 7,
              "dedicated subreaper token is not active")?;
        check(unsafe { ptg_terminal_self(0) } == self.pid
            && unsafe { ptg_terminal_self(1) } == self.tid, "dedicated process/thread changed")?;
        single_thread()?;
        let mut facts = 0;
        let mut error = 0;
        let result = unsafe { ptg_terminal_sigchld(&mut facts, &mut error) };
        let (get, value) = subreaper(0);
        check(result == 0 && error == 0 && facts == 7 && get.result == 0 && get.errno == 0 && value == 1,
              "dedicated SIGCHLD/subreaper state changed")
    }

    fn restore_after_empty(&mut self) -> Probe<()> {
        // Only the consuming controller calls this after actual direct reaps
        // and a valid terminal P_ALL ECHILD observation in this same context.
        self.validate()?;
        let (set, _) = subreaper(2);
        self.last_set = set;
        let (get, value) = subreaper(0);
        self.last_get = get;
        self.last_readback = value;
        if set.result == 0 && get.result == 0 && value == self.original_flag { self.active = false; }
        check(set.result == 0 && set.errno == 0 && get.result == 0 && get.errno == 0
            && value == self.original_flag, "restoring empty one-shot context failed")
    }
}

pub(super) struct DirectReapedProof<'a> {
    context: &'a DedicatedSupervisor,
    payload: Option<&'a ReapedChild>,
    watchdog: Option<&'a ReapedChild>,
}

impl DirectReapedProof<'_> {
    pub(super) fn validate(&self) -> Probe<()> {
        self.context.validate()?;
        check(self.watchdog.is_none_or(|child| child.pid() > 0)
            && self.payload.is_none_or(|child| child.pid() > 0)
            && self.payload.zip(self.watchdog).is_none_or(|(payload, watchdog)|
                payload.pid() != watchdog.pid()), "direct reap proof differs")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExecState {
    Pending,
    NoExecErrorReport,
    ReportedFailure { errno: i32 },
    Unverified,
}

#[derive(Clone, Copy, Debug, Default)]
struct ExecRead { kind: u32, count: c_int, errno: c_int }

pub(super) struct ExecReport {
    reader: OwnedFd,
    bytes: [u8; 9],
    used: usize,
    reads: [ExecRead; 16],
    read_count: usize,
    state: ExecState,
}

impl ExecReport {
    /// # Safety
    /// Sole nonblocking CLOEXEC read end for this exact trusted child. Its
    /// scrubbed, sole writer is CLOEXEC and may report only EXE1+errnoLE on
    /// bootstrap/exec failure. No other child/client inherits a writer.
    pub(super) unsafe fn from_trusted_child(reader: OwnedFd) -> Self {
        Self { reader, bytes: [0; 9], used: 0, reads: [ExecRead::default(); 16],
               read_count: 0, state: ExecState::Pending }
    }

    fn fd(&self) -> RawFd {
        if self.state == ExecState::Pending { self.reader.as_raw_fd() } else { -1 }
    }

    fn sample(&mut self) -> ExecState {
        if self.state != ExecState::Pending { return self.state; }
        if self.read_count == self.reads.len() || self.used == self.bytes.len() {
            self.state = ExecState::Unverified;
            return self.state;
        }
        // Reserve a fixed raw record before any consuming read. No allocation.
        let slot = &mut self.reads[self.read_count];
        self.read_count += 1;
        slot.kind = unsafe { ptg_terminal_exec_read(self.reader.as_raw_fd(),
            self.bytes[self.used..].as_mut_ptr(), (self.bytes.len() - self.used) as u32,
            &mut slot.count, &mut slot.errno) };
        match slot.kind {
            0 if slot.count > 0 && (slot.count as usize) <= self.bytes.len() - self.used => {
                self.used += slot.count as usize;
                if self.used >= 8 {
                    let code = i32::from_le_bytes(self.bytes[4..8].try_into().expect("four fixed bytes"));
                    self.state = if self.used == 8 && &self.bytes[..4] == b"EXE1" && (1..=4095).contains(&code) {
                        ExecState::ReportedFailure { errno: code }
                    } else { ExecState::Unverified };
                }
            }
            1 if slot.count == 0 => {
                self.state = if self.used == 0 { ExecState::NoExecErrorReport } else { ExecState::Unverified };
            }
            2 | 3 if slot.count < 0 => {}
            _ => self.state = ExecState::Unverified,
        }
        self.state
    }
}

enum ChildOwner { Live(DirectChild), Failed(ReapFailure), Reaped(ReapedChild) }

impl ChildOwner {
    fn fd(&self) -> RawFd {
        match self {
            Self::Live(child) => child.pidfd.as_raw_fd(),
            Self::Failed(ReapFailure::NotCalled { owner, .. }
                | ReapFailure::Unresolved { owner, .. }
                | ReapFailure::ConsumedInvalid { owner, .. }) => owner.pidfd(),
            Self::Failed(ReapFailure::AfterReap { .. }) | Self::Reaped(_) => -1,
        }
    }
    fn reaped(&self) -> Option<&ReapedChild> {
        match self { Self::Reaped(value) => Some(value), _ => None }
    }
}

enum DomainOwner {
    Active(DomainControls),
    Killed(KilledDomain),
    Drained(DrainedDomain),
    Quarantined(SettlementQuarantine),
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PollObservation { returned: c_int, errno: c_int, events: [c_short; 6] }

const STOP_OWNER: u32 = 1;
const STOP_DEADLINE: u32 = 2;
const STOP_PAYLOAD: u32 = 4;
const STOP_WATCHDOG: u32 = 8;
const STOP_RELEASE: u32 = 16;
const STOP_EXEC_FAILURE: u32 = 32;
const STOP_PROTOCOL: u32 = 64;
const STOP_POLL: u32 = 128;
const STOP_ARMING_FAILURE: u32 = 256;

#[derive(Clone, Copy, Debug)]
struct StopReceipt {
    reasons: u32,
    observed_poll: Option<PollObservation>,
    kill_outcome: KillOutcome,
}

pub(super) enum TerminalFault {
    Invariant,
    Poll(PollObservation),
    Deadline(String),
    Context(String),
    Domain,
    WatchdogSignal(NativeCall),
    DirectReap,
    Adopted(AnyFailure),
    RawStorage,
    ExecProtocol,
    Release,
    WatchdogEarlyExit,
    ParentSignalMask,
}

// An arming failure never attempted a release. Keep that actual origin instead
// of fabricating a release error/decision or a successful execution receipt.
pub(super) enum ExecutionOrigin {
    Release(Probe<Decision>),
    ArmingFailure { error: String },
}

pub(super) struct TerminalRun {
    arming: native_arming::ArmingReceipt,
    payload_mask: Option<native_port::ParentMaskObservation>,
    prepared: PreparedRun,
    origin: ExecutionOrigin,
    supervisor: DedicatedSupervisor,
    domain: Option<DomainOwner>,
    cleanup: Option<Deadline>,
    payload: Option<ChildOwner>,
    watchdog: Option<ChildOwner>,
    exec: Option<ExecReport>,
    stop: Option<StopReceipt>,
    latched_reasons: Option<u32>,
    last_poll: PollObservation,
    poll_observed: bool,
    interrupted_polls: u64,
    last_pending: [Option<child_reaper::RawWaitObservation>; 2],
    pending_count: [u64; 2],
    watchdog_kill: Option<NativeCall>,
    adopted: Vec<AnyWaitObservation>,
    echild_index: Option<usize>,
    semantic_fault: Option<TerminalFault>,
}

pub(super) struct SettledParts {
    arming: native_arming::ArmingReceipt,
    payload_mask: Option<native_port::ParentMaskObservation>,
    prepared: PreparedRun,
    origin: ExecutionOrigin,
    supervisor: DedicatedSupervisor,
    payload: Option<ReapedChild>,
    watchdog: Option<ReapedChild>,
    exec: Option<ExecReport>,
    stop: StopReceipt,
    watchdog_kill: Option<NativeCall>,
    adopted: Vec<AnyWaitObservation>,
    echild_index: usize,
    last_pending: [Option<child_reaper::RawWaitObservation>; 2],
    pending_count: [u64; 2],
    interrupted_polls: u64,
}

// Success means bounded settlement completed. It is NOT permit/exec success;
// original release, exec-report and actual child exit remain separate facts.
pub(super) struct SettledRun { parts: SettledParts, removed: RemovedDomain }

#[must_use = "quarantine owns actual remaining resources; dropping it is not settlement"]
pub(super) enum TerminalQuarantine {
    Running { owner: TerminalRun, fault: TerminalFault },
    Removal { parts: SettledParts, domain: SettlementQuarantine, fault: Option<TerminalFault> },
    SettledFault { owner: SettledRun, fault: TerminalFault },
}

struct ReadyToRemove { parts: SettledParts, drained: DrainedDomain, fault: Option<TerminalFault> }

impl TerminalRun {
    fn from_pending(pending: PendingSettlement) -> Self {
        let PendingSettlement { active, release } = pending;
        let ActiveRun { payload_mask, prepared, domain, child, cleanup, supervisor } = active;
        let ArmedDomain { controls, watchdog, arming } = domain;
        let mask_fault=(arming.parent_mask_uncertain() || payload_mask.is_some_and(|value|value.uncertain()))
            .then_some(TerminalFault::ParentSignalMask);
        let (payload, exec) = match child {
            Some(PayloadChild { identity, exec_report, .. }) => (Some(ChildOwner::Live(identity)), Some(exec_report)),
            None => (None, None),
        };
        Self { arming, payload_mask, prepared, origin: ExecutionOrigin::Release(release), supervisor, domain: Some(DomainOwner::Active(controls)),
            cleanup: Some(cleanup), payload, watchdog: Some(ChildOwner::Live(watchdog)), exec,
            stop: None, latched_reasons: None, last_poll: PollObservation::default(), poll_observed: false,
            interrupted_polls: 0, last_pending: [None, None], pending_count: [0, 0],
            watchdog_kill: None, adopted: Vec::new(), echild_index: None, semantic_fault: mask_fault }
    }

    fn latch_stop(&mut self, reasons: u32) -> Result<(), TerminalFault> {
        if self.stop.is_some() { return Ok(()); }
        // Latch before the syscall, not after its diagnostics or a clock read.
        self.latched_reasons = Some(reasons);
        let domain = self.domain.take().ok_or(TerminalFault::Invariant)?;
        match domain {
            DomainOwner::Active(controls) => {
                let killed = controls.terminate_owned();
                let kill_outcome = killed.outcome();
                self.domain = Some(DomainOwner::Killed(killed));
                self.stop = Some(StopReceipt { reasons,
                    observed_poll: self.poll_observed.then_some(self.last_poll), kill_outcome });
                Ok(())
            }
            other => { self.domain = Some(other); Err(TerminalFault::Invariant) }
        }
    }

    fn supervise(&mut self) -> Result<(), TerminalFault> {
        let early = match &self.origin {
            ExecutionOrigin::ArmingFailure { .. } => return Err(TerminalFault::Invariant),
            ExecutionOrigin::Release(Err(_)) => { self.semantic_fault = Some(TerminalFault::Release); Some(STOP_RELEASE) }
            ExecutionOrigin::Release(Ok(Decision::OwnerLost)) => Some(STOP_OWNER),
            ExecutionOrigin::Release(Ok(Decision::Deadline)) => Some(STOP_DEADLINE),
            ExecutionOrigin::Release(Ok(Decision::Permit)) if self.payload.is_none() || self.exec.is_none() => {
                self.semantic_fault = Some(TerminalFault::Invariant); Some(STOP_PROTOCOL)
            }
            ExecutionOrigin::Release(Ok(Decision::Permit)) => None,
        };
        if let Some(reasons) = early { return self.latch_stop(reasons); }
        loop {
            let cleanup = self.cleanup.as_ref().ok_or(TerminalFault::Invariant)?;
            let mut watched = [
                PollFd { fd: self.prepared.lease.owner.as_raw_fd(), events: POLLIN, revents: 0 },
                PollFd { fd: self.prepared.lease.timer.as_raw_fd(), events: POLLIN, revents: 0 },
                PollFd { fd: self.payload.as_ref().map_or(-1, ChildOwner::fd), events: POLLIN, revents: 0 },
                PollFd { fd: self.watchdog.as_ref().map_or(-1, ChildOwner::fd), events: POLLIN, revents: 0 },
                PollFd { fd: self.exec.as_ref().map_or(-1, ExecReport::fd), events: POLLIN, revents: 0 },
                PollFd { fd: cleanup.timer_fd(), events: POLLIN, revents: 0 },
            ];
            // Both original execution timer and separately prearmed cleanup
            // timer are real owned FDs. This is never an unarmed infinite wait.
            let mut errno = 0;
            let returned = unsafe { ptg_terminal_poll(watched.as_mut_ptr().cast(), watched.len() as u32, -1, &mut errno) };
            self.last_poll = PollObservation { returned, errno, events: watched.map(|value| value.revents) };
            self.poll_observed = true;
            if returned < 0 && unsafe { ptg_terminal_errno_kind(errno) } == 1 {
                self.interrupted_polls = self.interrupted_polls.saturating_add(1);
                // An unbounded signal stream must not bypass readable timers.
                // Cancel conservatively before any clock/diagnostic allocation;
                // the same prearmed authority bounds subsequent settlement.
                self.semantic_fault = Some(TerminalFault::Poll(self.last_poll));
                self.latch_stop(STOP_POLL)?;
                return Ok(());
            }
            let allowed = [POLLIN | POLLHUP, POLLIN, POLLIN | POLLHUP,
                           POLLIN | POLLHUP, POLLIN | POLLHUP, POLLIN];
            let invalid = returned <= 0
                || returned as usize != self.last_poll.events.iter().filter(|value| **value != 0).count()
                || self.last_poll.events.iter().zip(allowed).any(|(event, mask)| event & !mask != 0);
            if invalid {
                self.semantic_fault = Some(TerminalFault::Poll(self.last_poll));
                self.latch_stop(STOP_POLL)?;
                return Ok(());
            }
            let events = self.last_poll.events;
            let mut reasons = 0;
            if events[0] != 0 { reasons |= STOP_OWNER; }
            if events[1] != 0 || events[5] != 0 { reasons |= STOP_DEADLINE; }
            if events[2] != 0 { reasons |= STOP_PAYLOAD; }
            if events[3] != 0 { reasons |= STOP_WATCHDOG; }
            if reasons != 0 {
                if reasons & STOP_WATCHDOG != 0 && reasons & (STOP_OWNER | STOP_DEADLINE) == 0 {
                    self.semantic_fault = Some(TerminalFault::WatchdogEarlyExit);
                }
                self.latch_stop(reasons)?;
                return Ok(());
            }
            if events[4] != 0 {
                let state = self.exec.as_mut().ok_or(TerminalFault::Invariant)?.sample();
                match state {
                    ExecState::ReportedFailure { .. } => { self.latch_stop(STOP_EXEC_FAILURE)?; return Ok(()); }
                    ExecState::Unverified => {
                        self.semantic_fault = Some(TerminalFault::ExecProtocol);
                        self.latch_stop(STOP_PROTOCOL)?;
                        return Ok(());
                    }
                    ExecState::Pending | ExecState::NoExecErrorReport => {}
                }
            }
        }
    }

    fn drain(&mut self) -> Result<(), TerminalFault> {
        let domain = self.domain.take().ok_or(TerminalFault::Invariant)?;
        let killed = match domain {
            DomainOwner::Killed(value) => value,
            other => { self.domain = Some(other); return Err(TerminalFault::Invariant); }
        };
        let deadline = match self.cleanup.take() {
            Some(value) => value,
            None => { self.domain = Some(DomainOwner::Killed(killed)); return Err(TerminalFault::Invariant); }
        };
        match killed.drain(deadline, CLEANUP_GRACE_NS) {
            Ok(value) => { self.domain = Some(DomainOwner::Drained(value)); Ok(()) }
            Err(value) => { self.domain = Some(DomainOwner::Quarantined(value)); Err(TerminalFault::Domain) }
        }
    }

    fn deadline(&self) -> Result<&Deadline, TerminalFault> {
        match self.domain.as_ref() {
            Some(DomainOwner::Drained(domain)) => Ok(domain.cleanup_deadline()),
            _ => Err(TerminalFault::Invariant),
        }
    }

    fn kill_watchdog_after_empty(&mut self) -> Result<(), TerminalFault> {
        if self.watchdog_kill.is_some() { return Ok(()); }
        if !matches!(self.domain, Some(DomainOwner::Drained(_))) { return Err(TerminalFault::Invariant); }
        // Verified-domain arming may fail before any watchdog clone. Absence
        // is not a fabricated successful signal or reap.
        if self.watchdog.is_none() { return Ok(()); }
        let descriptor = self.watchdog.as_ref().map_or(-1, ChildOwner::fd);
        if descriptor < 0 { return Err(TerminalFault::Invariant); }
        let mut error = 0;
        let result = unsafe { ptg_terminal_kill(descriptor, &mut error) };
        let receipt = NativeCall { result, errno: error };
        self.watchdog_kill = Some(receipt);
        if result == 0 || (result == -1 && unsafe { ptg_terminal_errno_kind(error) } == 3) { Ok(()) }
        else { Err(TerminalFault::WatchdogSignal(receipt)) }
    }

    fn wait_cleanup(&mut self, adopted_pending: bool) -> Result<(), TerminalFault> {
        let deadline = self.deadline()?;
        deadline.check().map_err(TerminalFault::Deadline)?;
        let mut watched = [
            PollFd { fd: self.payload.as_ref().map_or(-1, ChildOwner::fd), events: POLLIN, revents: 0 },
            PollFd { fd: self.watchdog.as_ref().map_or(-1, ChildOwner::fd), events: POLLIN, revents: 0 },
            PollFd { fd: self.exec.as_ref().map_or(-1, ExecReport::fd), events: POLLIN, revents: 0 },
            PollFd { fd: deadline.timer_fd(), events: POLLIN, revents: 0 },
        ];
        // An adopted task in exit/reparent publication may be a child but not
        // yet waitable. Short bounded poll avoids a busy wait or PID lookup.
        let mut errno = 0;
        let returned = unsafe { ptg_terminal_poll(watched.as_mut_ptr().cast(), watched.len() as u32,
                                                  if adopted_pending { 10 } else { -1 }, &mut errno) };
        let events = [watched[0].revents, watched[1].revents, watched[2].revents, watched[3].revents, 0, 0];
        self.last_poll = PollObservation { returned, errno, events };
        self.poll_observed = true;
        if returned < 0 && unsafe { ptg_terminal_errno_kind(errno) } == 1 { return Ok(()); }
        let allowed = [POLLIN | POLLHUP, POLLIN | POLLHUP, POLLIN | POLLHUP, POLLIN, 0, 0];
        if returned < 0 || (returned == 0 && !adopted_pending)
            || returned as usize != events.iter().filter(|event| **event != 0).count()
            || events.iter().zip(allowed).any(|(event, mask)| event & !mask != 0) {
            return Err(TerminalFault::Poll(self.last_poll));
        }
        self.deadline()?.check().map_err(TerminalFault::Deadline)?;
        Ok(())
    }

    fn reap_directs(&mut self) -> Result<(), TerminalFault> {
        loop {
            let deadline = match self.domain.as_ref() {
                Some(DomainOwner::Drained(value)) => value.cleanup_deadline(),
                _ => return Err(TerminalFault::Invariant),
            };
            let payload_done = step_direct(&mut self.payload, &mut self.last_pending[0],
                                           &mut self.pending_count[0], deadline)?;
            let watchdog_done = step_direct(&mut self.watchdog, &mut self.last_pending[1],
                                            &mut self.pending_count[1], deadline)?;
            if let Some(exec) = self.exec.as_mut() {
                if exec.sample() == ExecState::Unverified { self.semantic_fault = Some(TerminalFault::ExecProtocol); }
            }
            if payload_done && watchdog_done { return Ok(()); }
            self.wait_cleanup(false)?;
        }
    }

    fn reap_adopted(&mut self) -> Result<(), TerminalFault> {
        loop {
            if self.adopted.len() == MAX_ANY_OBSERVATIONS { return Err(TerminalFault::RawStorage); }
            if self.adopted.len() == self.adopted.capacity() {
                let additional = ANY_RESERVE_CHUNK.min(MAX_ANY_OBSERVATIONS - self.adopted.len());
                self.adopted.try_reserve_exact(additional).map_err(|_| TerminalFault::RawStorage)?;
            }
            // Real storage exists BEFORE the consuming P_ALL wait. The slot
            // remains owned even for consumed-invalid/post-clock failures.
            self.adopted.push(AnyWaitObservation::reserved());
            let index = self.adopted.len() - 1;
            let deadline = match self.domain.as_ref() {
                Some(DomainOwner::Drained(value)) => value.cleanup_deadline(),
                _ => return Err(TerminalFault::Invariant),
            };
            let watchdog = match self.watchdog.as_ref() {
                None => None,
                Some(value) => Some(value.reaped().ok_or(TerminalFault::Invariant)?),
            };
            let payload = match self.payload.as_ref() {
                None => None,
                Some(value) => Some(value.reaped().ok_or(TerminalFault::Invariant)?),
            };
            let proof = DirectReapedProof { context: &self.supervisor, payload, watchdog };
            let state = child_reaper::poll_adopted_once(&proof, deadline, &mut self.adopted[index])
                .map_err(TerminalFault::Adopted)?;
            match state {
                AnyState::NoChildren => { self.echild_index = Some(index); return Ok(()); }
                AnyState::Child { consumed: true, .. } => {}
                AnyState::Pending => self.wait_cleanup(true)?,
                _ => return Err(TerminalFault::Invariant),
            }
        }
    }

    fn finish_exec_report(&mut self) -> Result<(), TerminalFault> {
        while let Some(exec) = self.exec.as_mut() {
            match exec.sample() {
                ExecState::Pending => self.wait_cleanup(false)?,
                ExecState::Unverified => { self.semantic_fault = Some(TerminalFault::ExecProtocol); return Ok(()); }
                ExecState::NoExecErrorReport | ExecState::ReportedFailure { .. } => return Ok(()),
            }
        }
        Ok(())
    }

    fn fail(mut self, fault: TerminalFault) -> TerminalQuarantine {
        if self.stop.is_none() { let _ = self.latch_stop(STOP_PROTOCOL); }
        // Never terminate the outside watchdog until original-domain empty is
        // proven. A secondary signal error remains in watchdog_kill raw state.
        if matches!(self.domain, Some(DomainOwner::Drained(_))) && self.watchdog_kill.is_none() {
            let _ = self.kill_watchdog_after_empty();
        }
        TerminalQuarantine::Running { owner: self, fault }
    }

    fn ready(mut self) -> Result<ReadyToRemove, TerminalQuarantine> {
        if self.echild_index.is_none() || self.supervisor.active || self.cleanup.is_some() {
            return Err(self.fail(TerminalFault::Invariant));
        }
        let watchdog = match self.watchdog.take() {
            None => None,
            Some(ChildOwner::Reaped(value)) => Some(value),
            other => { self.watchdog = other; return Err(self.fail(TerminalFault::Invariant)); }
        };
        let payload = match self.payload.take() {
            None => None,
            Some(ChildOwner::Reaped(value)) => Some(value),
            other => {
                self.payload = other; self.watchdog = watchdog.map(ChildOwner::Reaped);
                return Err(self.fail(TerminalFault::Invariant));
            }
        };
        let drained = match self.domain.take() {
            Some(DomainOwner::Drained(value)) => value,
            other => {
                self.domain = other; self.payload = payload.map(ChildOwner::Reaped);
                self.watchdog = watchdog.map(ChildOwner::Reaped);
                return Err(self.fail(TerminalFault::Invariant));
            }
        };
        let stop = match self.stop {
            Some(value) => value,
            None => {
                self.domain = Some(DomainOwner::Drained(drained)); self.payload = payload.map(ChildOwner::Reaped);
                self.watchdog = watchdog.map(ChildOwner::Reaped);
                return Err(self.fail(TerminalFault::Invariant));
            }
        };
        let watchdog_kill = match (watchdog.is_some(), self.watchdog_kill) {
            (true, Some(value)) => Some(value),
            (false, None) => None,
            _ => {
                self.domain = Some(DomainOwner::Drained(drained)); self.payload = payload.map(ChildOwner::Reaped);
                self.watchdog = watchdog.map(ChildOwner::Reaped);
                return Err(self.fail(TerminalFault::Invariant));
            }
        };
        let parts = SettledParts {
            arming: self.arming, payload_mask: self.payload_mask, prepared: self.prepared, origin: self.origin, supervisor: self.supervisor,
            payload, watchdog, exec: self.exec, stop, watchdog_kill, adopted: self.adopted,
            echild_index: self.echild_index.expect("checked actual ECHILD receipt"),
            last_pending: self.last_pending, pending_count: self.pending_count,
            interrupted_polls: self.interrupted_polls,
        };
        Ok(ReadyToRemove { parts, drained, fault: self.semantic_fault })
    }
}

fn step_direct(
    actor: &mut Option<ChildOwner>,
    last_pending: &mut Option<child_reaper::RawWaitObservation>,
    pending_count: &mut u64,
    deadline: &Deadline,
) -> Result<bool, TerminalFault> {
    let value = match actor.take() { Some(value) => value, None => return Ok(true) };
    let child = match value {
        ChildOwner::Live(value) => value,
        ChildOwner::Reaped(value) => { *actor = Some(ChildOwner::Reaped(value)); return Ok(true); }
        ChildOwner::Failed(value) => { *actor = Some(ChildOwner::Failed(value)); return Err(TerminalFault::DirectReap); }
    };
    match child_reaper::poll_reap(child, deadline) {
        Ok(ReapStep::Pending { child, observation, .. }) => {
            *actor = Some(ChildOwner::Live(child));
            *last_pending = Some(observation);
            *pending_count = pending_count.saturating_add(1);
            Ok(false)
        }
        Ok(ReapStep::Reaped(value)) => { *actor = Some(ChildOwner::Reaped(value)); Ok(true) }
        Err(value) => { *actor = Some(ChildOwner::Failed(value)); Err(TerminalFault::DirectReap) }
    }
}

impl ReadyToRemove {
    fn remove_last(self) -> Result<SettledRun, TerminalQuarantine> {
        let Self { parts, drained, fault } = self;
        // This private type can only be built after direct receipts, actual
        // same-context adopted ECHILD, closed permit, and restored empty context.
        // No future placement or clone path is called after that proof.
        match unsafe { drained.remove_after_reaping() } {
            Ok(removed) => {
                let owner = SettledRun { parts, removed };
                match fault {
                    None => Ok(owner),
                    Some(fault) => Err(TerminalQuarantine::SettledFault { owner, fault }),
                }
            }
            Err(domain) => Err(TerminalQuarantine::Removal { parts, domain, fault }),
        }
    }
}

pub(super) struct TerminalFacts<'a> {
    pub(super) release: Option<&'a Probe<Decision>>,
    pub(super) arming_failure: Option<&'a str>,
    pub(super) exec_report: Option<ExecState>,
    pub(super) payload_exit: Option<ExitOutcome>,
    pub(super) watchdog_exit: Option<ExitOutcome>,
    pub(super) stop_reasons: u32,
    pub(super) removed: &'a RemovedDomain,
}

impl SettledRun {
    pub(super) fn facts(&self) -> TerminalFacts<'_> {
        let (release, arming_failure) = match &self.parts.origin {
            ExecutionOrigin::Release(value) => (Some(value), None),
            ExecutionOrigin::ArmingFailure { error } => (None, Some(error.as_str())),
        };
        TerminalFacts { release, arming_failure,
            exec_report: self.parts.exec.as_ref().map(|report| report.state),
            payload_exit: self.parts.payload.as_ref().map(ReapedChild::outcome),
            watchdog_exit: self.parts.watchdog.as_ref().map(ReapedChild::outcome), stop_reasons: self.parts.stop.reasons,
            removed: &self.removed }
    }
}

impl TerminalRun {
    fn settle_stopped(mut owner: Self) -> Result<SettledRun, TerminalQuarantine> {
        if let Err(fault) = owner.drain() { return Err(owner.fail(fault)); }
        if let Err(fault) = owner.kill_watchdog_after_empty() { return Err(owner.fail(fault)); }
        if let Err(fault) = owner.reap_directs() { return Err(owner.fail(fault)); }
        if let Err(fault) = owner.reap_adopted() { return Err(owner.fail(fault)); }
        if let Err(fault) = owner.finish_exec_report() { return Err(owner.fail(fault)); }
        if let Err(error) = owner.deadline().and_then(|deadline| deadline.check().map_err(TerminalFault::Deadline)) {
            return Err(owner.fail(error));
        }
        if let Err(error) = owner.supervisor.restore_after_empty() { return Err(owner.fail(TerminalFault::Context(error))); }
        owner.ready()?.remove_last()
    }
}

impl PendingSettlement {
    pub(super) fn run_to_terminal(self) -> Result<SettledRun, TerminalQuarantine> {
        let mut owner = TerminalRun::from_pending(self);
        if let Err(fault) = owner.supervise() { return Err(owner.fail(fault)); }
        TerminalRun::settle_stopped(owner)
    }
}

pub(super) fn settle_verified_arming_failure(
    prepared: PreparedRun,
    supervisor: DedicatedSupervisor,
    cleanup: Deadline,
    controls: DomainControls,
    watchdog: Option<DirectChild>,
    error: String,
    arming: native_arming::ArmingReceipt,
) -> Result<SettledRun, TerminalQuarantine> {
    // Vec::new allocates no storage. No clock, formatting, pathname validation
    // or fresh allocation occurs before the original held kill below.
    let mask_fault=arming.parent_mask_uncertain().then_some(TerminalFault::ParentSignalMask);
    let mut owner = TerminalRun {
        arming, payload_mask: None, prepared, origin: ExecutionOrigin::ArmingFailure { error }, supervisor,
        domain: Some(DomainOwner::Active(controls)), cleanup: Some(cleanup),
        payload: None, watchdog: watchdog.map(ChildOwner::Live), exec: None,
        stop: None, latched_reasons: None, last_poll: PollObservation::default(), poll_observed: false,
        interrupted_polls: 0, last_pending: [None, None], pending_count: [0, 0],
        watchdog_kill: None, adopted: Vec::new(), echild_index: None, semantic_fault: mask_fault,
    };
    if let Err(fault) = owner.latch_stop(STOP_ARMING_FAILURE) { return Err(owner.fail(fault)); }
    TerminalRun::settle_stopped(owner)
}
