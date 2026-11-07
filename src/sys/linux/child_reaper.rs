// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

use super::cgroup_authority::settlement::Deadline;
use super::{AsRawFd, DirectChild, OwnedFd};
use std::mem::{align_of, offset_of, size_of};

const ABI_WORDS: usize = 34;
const NATIVE_CAPACITY: usize = 512;

// Fixed C/Rust wire frame. Native siginfo_t is decoded only by its installed
// C headers in waitid_bridge.c; Rust retains its bytes without interpreting them.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct WaitFrame {
    pub(super) called: u32,
    pub(super) result: i32,
    pub(super) call_errno: i32,
    pub(super) signo: i32,
    pub(super) signal_errno: i32,
    pub(super) code: i32,
    pub(super) pid: i32,
    pub(super) uid: u32,
    pub(super) status: i32,
    pub(super) native_size: u32,
}

const FRAME_OFFSETS: [usize; 10] = [
    offset_of!(WaitFrame, called),
    offset_of!(WaitFrame, result),
    offset_of!(WaitFrame, call_errno),
    offset_of!(WaitFrame, signo),
    offset_of!(WaitFrame, signal_errno),
    offset_of!(WaitFrame, code),
    offset_of!(WaitFrame, pid),
    offset_of!(WaitFrame, uid),
    offset_of!(WaitFrame, status),
    offset_of!(WaitFrame, native_size),
];

const _: () = {
    assert!(size_of::<WaitFrame>() == 40);
    assert!(align_of::<WaitFrame>() == 4);
    let mut index = 0;
    while index < FRAME_OFFSETS.len() {
        assert!(FRAME_OFFSETS[index] == index * 4);
        index += 1;
    }
};

unsafe extern "C" {
    fn ptg_waitid_abi_word(field: u32) -> u64;
    fn ptg_waitid_once(
        pidfd: i32,
        out: *mut WaitFrame,
        native_bytes: *mut u8,
        native_capacity: u32,
    );
    fn ptg_terminal_wait_any(peek: u32, out: *mut WaitFrame,
                             native_bytes: *mut u8, native_capacity: u32);
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AbiReport {
    pub(super) words: [u64; ABI_WORDS],
}

pub(super) struct AbiFailure {
    pub(super) report: AbiReport,
    pub(super) reason: &'static str,
}

struct WaitAbi {
    report: AbiReport,
    native_size: usize,
    sigchld: i32,
    cld_exited: i32,
    cld_killed: i32,
    cld_dumped: i32,
    nsig: i32,
    echild: i32,
}

fn verified_abi() -> Result<WaitAbi, AbiFailure> {
    let mut report = AbiReport { words: [0; ABI_WORDS] };
    for (index, value) in report.words.iter_mut().enumerate() {
        // Scalar-only ABI query precedes every C write into a Rust frame.
        *value = unsafe { ptg_waitid_abi_word(index as u32) };
    }
    let failure = |reason| AbiFailure { report, reason };
    if report.words[0] != 1
        || report.words[1] != size_of::<WaitFrame>() as u64
        || report.words[2] != align_of::<WaitFrame>() as u64
        || FRAME_OFFSETS.iter().enumerate().any(|(index, value)| {
            report.words[index + 3] != *value as u64
        })
    {
        return Err(failure("normalized C/Rust frame ABI differs"));
    }
    let native_size = report.words[13];
    if !(4..=NATIVE_CAPACITY as u64).contains(&native_size)
        || !report.words[14].is_power_of_two()
        || report.words[15..21].iter().any(|offset| *offset > native_size - 4)
        || report.words[21..].iter().any(|value| *value == 0 || *value > i32::MAX as u64)
    {
        return Err(failure("installed-header ABI report is invalid or exceeds storage"));
    }
    let sigchld = report.words[24] as i32;
    let cld_exited = report.words[25] as i32;
    let cld_killed = report.words[26] as i32;
    let cld_dumped = report.words[27] as i32;
    let nsig = report.words[28] as i32;
    if nsig <= sigchld
        || cld_exited == cld_killed
        || cld_exited == cld_dumped
        || cld_killed == cld_dumped
    {
        return Err(failure("installed-header signal classifications differ"));
    }
    Ok(WaitAbi {
        report,
        native_size: native_size as usize,
        sigchld,
        cld_exited,
        cld_killed,
        cld_dumped,
        nsig,
        echild: report.words[29] as i32,
    })
}

pub(super) struct RawWaitObservation {
    pub(super) frame: WaitFrame,
    pub(super) native_bytes: [u8; NATIVE_CAPACITY],
    pub(super) abi: AbiReport,
    pub(super) expected_pid: i32,
    pub(super) target_pidfd: i32,
    pub(super) before_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExitOutcome {
    Exited { code: u8 },
    Signaled { signal: i32, core_dumped: bool },
}

// Only the validated positive branch below can create this receipt. The
// original pidfd remains owned; there is no conversion back to DirectChild.
pub(super) struct ReapedChild {
    pid: i32,
    pidfd: OwnedFd,
    outcome: ExitOutcome,
    observation: RawWaitObservation,
    after_ns: Option<u64>,
}

impl ReapedChild {
    pub(super) fn pid(&self) -> i32 { self.pid }
    pub(super) fn outcome(&self) -> ExitOutcome { self.outcome }
    pub(super) fn observation(&self) -> &RawWaitObservation { &self.observation }
}

pub(super) enum ReapStep {
    Pending {
        child: DirectChild,
        observation: RawWaitObservation,
        after_ns: u64,
    },
    Reaped(ReapedChild),
}

pub(super) enum NotCalledObservation {
    DeadlineBeforeAbi { error: String },
    Abi(AbiFailure),
    DeadlineAfterAbi { error: String, report: AbiReport },
}

pub(super) enum ObservationProblem {
    BridgeRefusedOrMalformed,
    WaitError { errno: i32, is_echild: bool },
    InvalidZeroOrReturn,
    DeadlineAfterPending,
}

// Error custody is opaque: only Pending returns a reusable DirectChild.
// In particular, a consumed-but-invalid observation cannot be retried by
// extracting a normal owner through this API.
pub(super) struct QuarantinedChild {
    child: DirectChild,
}

impl QuarantinedChild {
    pub(super) fn pid(&self) -> i32 { self.child.pid }
    pub(super) fn pidfd(&self) -> i32 { self.child.pidfd.as_raw_fd() }
}

// These variants retain the same original ownership or a validated reaped
// receipt. Errors are never converted into Pending or silently retried.
pub(super) enum ReapFailure {
    NotCalled {
        owner: QuarantinedChild,
        observation: NotCalledObservation,
    },
    Unresolved {
        owner: QuarantinedChild,
        observation: RawWaitObservation,
        reason: ObservationProblem,
        post_deadline: Result<u64, String>,
    },
    // A positive consuming wait was reported but cannot establish an exact
    // valid reap. It is not an available child/status to retry as Pending.
    ConsumedInvalid {
        owner: QuarantinedChild,
        observation: RawWaitObservation,
        reason: &'static str,
        post_deadline: Result<u64, String>,
    },
    // A later clock/deadline error must not erase the already validated event.
    AfterReap {
        reaped: ReapedChild,
        deadline_error: String,
    },
}

enum Classification {
    Pending,
    Reaped(ExitOutcome),
    Unresolved(ObservationProblem),
    ConsumedInvalid(&'static str),
}

fn classify(observation: &RawWaitObservation, abi: &WaitAbi) -> Classification {
    let f = &observation.frame;
    if f.called != 1 {
        return Classification::Unresolved(ObservationProblem::BridgeRefusedOrMalformed);
    }
    if f.result == -1 {
        return Classification::Unresolved(ObservationProblem::WaitError {
            errno: f.call_errno,
            is_echild: f.call_errno == abi.echild,
        });
    }
    if f.result != 0 || f.pid < 0 {
        return Classification::Unresolved(ObservationProblem::InvalidZeroOrReturn);
    }
    if f.pid == 0 {
        return if f.native_size as usize == abi.native_size
            && f.signo == 0 && f.signal_errno == 0 && f.code == 0 && f.uid == 0 && f.status == 0
        {
            Classification::Pending
        } else {
            Classification::Unresolved(ObservationProblem::InvalidZeroOrReturn)
        };
    }
    if f.native_size as usize != abi.native_size
        || f.pid != observation.expected_pid
        || f.signo != abi.sigchld
        || f.signal_errno != 0
    {
        return Classification::ConsumedInvalid("positive wait identity or frame is invalid");
    }
    if f.code == abi.cld_exited && (0..=255).contains(&f.status) {
        return Classification::Reaped(ExitOutcome::Exited { code: f.status as u8 });
    }
    if (f.code == abi.cld_killed || f.code == abi.cld_dumped) && (1..abi.nsig).contains(&f.status) {
        return Classification::Reaped(ExitOutcome::Signaled {
            signal: f.status,
            core_dumped: f.code == abi.cld_dumped,
        });
    }
    Classification::ConsumedInvalid("positive wait exit classification is invalid")
}

pub(super) fn poll_reap(
    child: DirectChild,
    deadline: &Deadline,
) -> Result<ReapStep, ReapFailure> {
    if let Err(error) = deadline.check() {
        return Err(ReapFailure::NotCalled {
            owner: QuarantinedChild { child },
            observation: NotCalledObservation::DeadlineBeforeAbi { error },
        });
    }
    let abi = match verified_abi() {
        Ok(abi) => abi,
        Err(failure) => return Err(ReapFailure::NotCalled {
            owner: QuarantinedChild { child },
            observation: NotCalledObservation::Abi(failure),
        }),
    };
    let before_ns = match deadline.check() {
        Ok(value) => value,
        Err(error) => return Err(ReapFailure::NotCalled {
            owner: QuarantinedChild { child },
            observation: NotCalledObservation::DeadlineAfterAbi { error, report: abi.report },
        }),
    };
    let mut observation = RawWaitObservation {
        frame: WaitFrame::default(),
        native_bytes: [0; NATIVE_CAPACITY],
        abi: abi.report,
        expected_pid: child.pid,
        target_pidfd: child.pidfd.as_raw_fd(),
        before_ns,
    };
    // One fixed WEXITED|WNOHANG call, with no retry or PID lookup. C validates
    // buffer capacity and decodes its own installed-header native siginfo_t.
    unsafe {
        ptg_waitid_once(
            observation.target_pidfd,
            &mut observation.frame,
            observation.native_bytes.as_mut_ptr(),
            NATIVE_CAPACITY as u32,
        );
    }
    // Consumption is classified before any post-call deadline validation.
    match classify(&observation, &abi) {
        Classification::Pending => match deadline.check() {
            Ok(after_ns) => Ok(ReapStep::Pending { child, observation, after_ns }),
            Err(error) => Err(ReapFailure::Unresolved {
                owner: QuarantinedChild { child }, observation,
                reason: ObservationProblem::DeadlineAfterPending,
                post_deadline: Err(error),
            }),
        },
        Classification::Reaped(outcome) => {
            let DirectChild { pid, pidfd } = child;
            let mut reaped = ReapedChild { pid, pidfd, outcome, observation, after_ns: None };
            match deadline.check() {
                Ok(after_ns) => {
                    reaped.after_ns = Some(after_ns);
                    Ok(ReapStep::Reaped(reaped))
                }
                Err(deadline_error) => Err(ReapFailure::AfterReap { reaped, deadline_error }),
            }
        }
        Classification::Unresolved(reason) => Err(ReapFailure::Unresolved {
            owner: QuarantinedChild { child }, observation, reason, post_deadline: deadline.check(),
        }),
        Classification::ConsumedInvalid(reason) => Err(ReapFailure::ConsumedInvalid {
            owner: QuarantinedChild { child }, observation, reason, post_deadline: deadline.check(),
        }),
    }
}

const _: fn(DirectChild, &Deadline) -> Result<ReapStep, ReapFailure> = poll_reap;

#[derive(Clone, Copy, Debug)]
pub(super) enum AnyState {
    Pending,
    Child { pid: i32, outcome: ExitOutcome, consumed: bool },
    NoChildren,
    InvalidPositive { consumed: bool },
    Refused,
}

pub(super) struct AnyWaitObservation {
    pub(super) peek: bool,
    pub(super) frame: WaitFrame,
    pub(super) native_bytes: [u8; NATIVE_CAPACITY],
    pub(super) abi: AbiReport,
    pub(super) before_ns: Option<u64>,
    pub(super) classified: Option<AnyState>,
    pub(super) after_ns: Option<u64>,
}

impl AnyWaitObservation {
    pub(super) fn reserved() -> Self {
        Self { peek: false, frame: WaitFrame::default(), native_bytes: [0; NATIVE_CAPACITY],
               abi: AbiReport { words: [0; ABI_WORDS] }, before_ns: None,
               classified: None, after_ns: None }
    }
}

pub(super) enum AnyFailure {
    Abi(AbiFailure),
    Context(String),
    Deadline(String),
    ObservedRefusal,
}

fn observe_any(peek: bool, slot: &mut AnyWaitObservation) -> Result<AnyState, AnyFailure> {
    let abi = verified_abi().map_err(AnyFailure::Abi)?;
    slot.peek = peek;
    slot.abi = abi.report;
    unsafe { ptg_terminal_wait_any(u32::from(peek), &mut slot.frame,
                                  slot.native_bytes.as_mut_ptr(), NATIVE_CAPACITY as u32); }
    // Preserve consumption classification in caller-owned pre-reserved storage
    // before any post-call context or clock operation can fail.
    let f = &slot.frame;
    let state = if f.called != 1 {
        AnyState::Refused
    } else if f.result == -1 && f.call_errno == abi.echild && f.native_size as usize == abi.native_size {
        AnyState::NoChildren
    } else if f.result != 0 || f.pid < 0 {
        AnyState::Refused
    } else if f.pid == 0 {
        if f.native_size as usize == abi.native_size && f.signo == 0
            && f.signal_errno == 0 && f.code == 0 && f.uid == 0 && f.status == 0 {
            AnyState::Pending
        } else { AnyState::Refused }
    } else if f.native_size as usize == abi.native_size && f.signo == abi.sigchld && f.signal_errno == 0 {
        let outcome = if f.code == abi.cld_exited && (0..=255).contains(&f.status) {
            Some(ExitOutcome::Exited { code: f.status as u8 })
        } else if (f.code == abi.cld_killed || f.code == abi.cld_dumped)
            && (1..abi.nsig).contains(&f.status) {
            Some(ExitOutcome::Signaled { signal: f.status, core_dumped: f.code == abi.cld_dumped })
        } else { None };
        match outcome {
            Some(outcome) => AnyState::Child { pid: f.pid, outcome, consumed: !peek },
            None => AnyState::InvalidPositive { consumed: !peek },
        }
    } else { AnyState::InvalidPositive { consumed: !peek } };
    slot.classified = Some(state);
    Ok(state)
}

// The initial entry probe can never consume an existing child's status.
pub(super) fn peek_entry_children() -> Result<AnyWaitObservation, AnyFailure> {
    let mut slot = AnyWaitObservation::reserved();
    let _ = observe_any(true, &mut slot)?;
    Ok(slot)
}

pub(super) fn poll_adopted_once(
    proof: &super::terminal_controller::DirectReapedProof<'_>,
    deadline: &Deadline,
    slot: &mut AnyWaitObservation,
) -> Result<AnyState, AnyFailure> {
    proof.validate().map_err(AnyFailure::Context)?;
    slot.before_ns = Some(deadline.check().map_err(AnyFailure::Deadline)?);
    let state = observe_any(false, slot)?;
    proof.validate().map_err(AnyFailure::Context)?;
    slot.after_ns = Some(deadline.check().map_err(AnyFailure::Deadline)?);
    match state {
        AnyState::Pending | AnyState::Child { consumed: true, .. } | AnyState::NoChildren => Ok(state),
        _ => Err(AnyFailure::ObservedRefusal),
    }
}
