// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

use super::*;

const AT_REMOVEDIR: c_int = 0x200;
const POLLPRI: c_short = 0x2;
const POLLERR: c_short = 0x8;
const ENOENT: c_int = 2;

unsafe extern "C" {
    #[link_name = "write"]
    fn held_write(fd: c_int, bytes: *const c_void, size: usize) -> isize;
    #[link_name = "pread"]
    fn held_pread(fd: c_int, bytes: *mut c_void, size: usize, offset: i64) -> isize;
    #[link_name = "unlinkat"]
    fn held_unlinkat(parent: c_int, name: *const c_char, flags: c_int) -> c_int;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum SettlementStage {
    ArmDeadline,
    Validate,
    Kill,
    Drain,
    Remove,
    Confirm,
}

#[derive(Debug)]
pub(in super::super) struct DrainReceipt {
    pub(in super::super) name: String,
    identity: Identity,
    // Clock observation after the kill write returned; never a pre-kill stamp.
    pub(in super::super) after_kill_ns: u64,
    pub(in super::super) kill_outcome: KillOutcome,
    pub(in super::super) empty_ns: u64,
    pub(in super::super) poll_wakeups: usize,
    pub(in super::super) cleanup_deadline_ns: u64,
}

pub(in super::super) struct DrainedDomain {
    controls: DomainControls,
    deadline: Deadline,
    receipt: DrainReceipt,
}

// Created only by the real held kill syscall below. The original controls and
// raw outcome remain owned before any clock, allocation or path observation.
pub(in super::super) struct KilledDomain {
    controls: DomainControls,
    kill_outcome: KillOutcome,
}

#[derive(Debug)]
pub(in super::super) struct RemovedDomain {
    pub(in super::super) drain: DrainReceipt,
    pub(in super::super) removed_ns: u64,
    pub(in super::super) gate_release: Option<(c_int,c_int)>,
}

#[derive(Debug)]
pub(in super::super) struct RemovedUnactivatedLeaf {
    pub(in super::super) name: String,
    identity: Identity,
    pub(in super::super) empty_checked_ns: u64,
    pub(in super::super) removed_ns: u64,
    pub(in super::super) cleanup_deadline_ns: u64,
}

#[must_use = "an unactivated leaf failure retains its actual anchor and cleanup authority"]
pub(in super::super) struct UnactivatedLeafQuarantine {
    pub(in super::super) created: CreatedLeaf,
    pub(in super::super) deadline: Deadline,
    pub(in super::super) stage: SettlementStage,
    pub(in super::super) error: String,
    pub(in super::super) removal_succeeded: bool,
    pub(in super::super) empty_checked_ns: Option<u64>,
}

#[must_use = "settlement failure retains the domain for quarantine or explicit recovery"]
pub(in super::super) struct SettlementQuarantine {
    pub(in super::super) controls: DomainControls,
    pub(in super::super) stage: SettlementStage,
    pub(in super::super) error: String,
    pub(in super::super) removal_succeeded: bool,
    pub(in super::super) kill_outcome: KillOutcome,
    pub(in super::super) cleanup_deadline: Option<Deadline>,
    pub(in super::super) after_kill_ns: Option<u64>,
    pub(in super::super) drain_receipt: Option<DrainReceipt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum KillOutcome {
    Written,
    Short { bytes: usize },
    Failed { errno: Option<c_int> },
}

fn request_kill(kill: &OwnedFd) -> KillOutcome {
    // DomainControls can only be constructed with the original trusted held
    // authority. Clock/timer allocation and current names do not gate this
    // cancellation attempt. The syscall itself can block: independent watchdog
    // coverage remains necessary for the eventual full one-shot port.
    let written = unsafe { held_write(kill.as_raw_fd(), b"1".as_ptr().cast(), 1) };
    if written == 1 {
        KillOutcome::Written
    } else if written < 0 {
        KillOutcome::Failed {
            errno: io::Error::last_os_error().raw_os_error(),
        }
    } else {
        KillOutcome::Short {
            bytes: written as usize,
        }
    }
}

#[derive(Debug)]
struct TerminationFailure<C> {
    custody: C,
    kill_outcome: KillOutcome,
    error: String,
}

fn terminate_then<C, T>(
    custody: C,
    kill: impl FnOnce(&C) -> KillOutcome,
    after_kill: impl FnOnce(&C, KillOutcome) -> Probe<T>,
) -> std::result::Result<(C, T), TerminationFailure<C>> {
    let outcome = kill(&custody);
    let result = if outcome == KillOutcome::Written {
        after_kill(&custody, outcome)
    } else {
        Err(format!("held cgroup.kill request refused: {outcome:?}"))
    };
    match result {
        Ok(receipt) => Ok((custody, receipt)),
        Err(error) => Err(TerminationFailure {
            custody,
            kill_outcome: outcome,
            error,
        }),
    }
}

pub(in super::super) struct Deadline {
    timer: OwnedFd,
    until_ns: u64,
    armed_ceiling_ns: u64,
    last_narrow: Option<NarrowObservation>,
}

#[derive(Clone, Copy, Debug)]
pub(in super::super) struct NarrowObservation {
    pub(in super::super) previous_until_ns: u64,
    pub(in super::super) requested_until_ns: u64,
    pub(in super::super) result: c_int,
    pub(in super::super) errno: Option<c_int>,
}

impl Deadline {
    fn arm(until_ns: u64) -> Probe<Self> {
        check(until_ns > 0, "settlement deadline cannot disarm the timer")?;
        check(
            boottime_ns()? < until_ns,
            "settlement deadline already expired",
        )?;
        let timer =
            fd_from_call(unsafe { timerfd_create(CLOCK_BOOTTIME, O_CLOEXEC | O_NONBLOCK) })?;
        native(close_on_exec(timer.as_raw_fd()))?;
        let value = ItimerSpec {
            interval: Timespec::default(),
            value: Timespec {
                seconds: (until_ns / NS_PER_SECOND) as i64,
                nanoseconds: (until_ns % NS_PER_SECOND) as i64,
            },
        };
        native(os_result(unsafe {
            timerfd_settime(
                timer.as_raw_fd(),
                TFD_TIMER_ABSTIME,
                &value,
                ptr::null_mut(),
            )
        } as isize))?;
        // The caller stores this owned armed authority before the post-arm
        // check, so expiry/clock errors cannot discard it on returned failure.
        Ok(Self { timer, until_ns, armed_ceiling_ns: until_ns, last_narrow: None })
    }

    pub(in super::super) fn arm_before_placement(until_ns: u64) -> Probe<Self> {
        Self::arm(until_ns)
    }

    pub(in super::super) fn armed_ceiling_ns(&self) -> u64 { self.armed_ceiling_ns }
    pub(in super::super) fn effective_until_ns(&self) -> u64 { self.until_ns }
    pub(in super::super) fn narrow_observation(&self) -> Option<NarrowObservation> { self.last_narrow }

    fn narrow_to(&mut self, requested_until_ns: u64) -> Probe<()> {
        check(requested_until_ns > 0 && requested_until_ns <= self.until_ns,
              "cleanup authority may only move earlier, never disarm or extend")?;
        if requested_until_ns == self.until_ns { return Ok(()); }
        let value = ItimerSpec {
            interval: Timespec::default(),
            value: Timespec {
                seconds: (requested_until_ns / NS_PER_SECOND) as i64,
                nanoseconds: (requested_until_ns % NS_PER_SECOND) as i64,
            },
        };
        let result = unsafe { timerfd_settime(self.timer.as_raw_fd(), TFD_TIMER_ABSTIME,
                                             &value, ptr::null_mut()) };
        let errno = if result < 0 { io::Error::last_os_error().raw_os_error() } else { None };
        self.last_narrow = Some(NarrowObservation {
            previous_until_ns: self.until_ns, requested_until_ns, result, errno,
        });
        // Preserve the actual successful retarget before any later fallible
        // clock check. On error the same old owner and attempt stay available;
        // callers quarantine it and do not enter an uncertain wait.
        if result == 0 { self.until_ns = requested_until_ns; }
        check(result == 0, "narrowing prearmed cleanup timer failed")
    }

    pub(in super::super) fn check(&self) -> Probe<u64> {
        let now = boottime_ns()?;
        check(now < self.until_ns, "settlement absolute deadline expired")?;
        Ok(now)
    }

    // Borrowed descriptor only. A future native reaper must retain this owner,
    // check its deadline and never rearm or disarm the cleanup timer.
    pub(in super::super) fn timer_fd(&self) -> RawFd {
        self.timer.as_raw_fd()
    }

    // Test fixture uses the real prearmed cleanup authority, not a mock clock.
    #[cfg(test)]
    pub(in super::super) fn fixture_arm(until_ns: u64) -> Probe<Self> {
        Self::arm(until_ns)
    }
}

fn populated(events: &OwnedFd) -> Probe<bool> {
    // pread leaves the shared open-file offset intact for the watchdog.
    let mut bytes = [0u8; 4096];
    let count = native(os_result(unsafe {
        held_pread(
            events.as_raw_fd(),
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            0,
        )
    }))?;
    check(
        count < bytes.len(),
        "cgroup.events exceeded the bounded read",
    )?;
    let contents = std::str::from_utf8(&bytes[..count])
        .map_err(|_| "cgroup.events contains non-UTF-8 text")?;
    let mut field = contents
        .lines()
        .filter_map(|line| line.strip_prefix("populated "));
    let value = field.next().ok_or("cgroup.events omits populated")?;
    check(field.next().is_none(), "cgroup.events duplicates populated")?;
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err("cgroup.events has invalid populated value".into()),
    }
}

fn wait_empty(events: &OwnedFd, deadline: &Deadline) -> Probe<(u64, usize)> {
    let mut wakeups = 0;
    loop {
        deadline.check()?;
        if !populated(events)? {
            return Ok((deadline.check()?, wakeups));
        }
        let mut descriptors = [
            PollFd {
                fd: events.as_raw_fd(),
                events: POLLPRI | POLLERR,
                revents: 0,
            },
            PollFd {
                fd: deadline.timer.as_raw_fd(),
                events: POLLIN,
                revents: 0,
            },
        ];
        native(os_result(
            unsafe { poll(descriptors.as_mut_ptr(), descriptors.len(), -1) } as isize,
        ))?;
        deadline.check()?;
        check(
            descriptors[0].revents & !(POLLPRI | POLLERR) == 0 && descriptors[1].revents == 0,
            "unexpected settlement poll event",
        )?;
        wakeups += 1;
    }
}

fn require_name_absent(parent: &OwnedFd, name: &CStr) -> Probe<()> {
    let mut unused = Statx::default();
    let result = unsafe {
        fd_statx(
            parent.as_raw_fd(),
            name.as_ptr(),
            AT_SYMLINK_NOFOLLOW,
            STATX_BASIC_STATS | STATX_MNT_ID,
            &mut unused,
        )
    };
    check(
        result == -1 && io::Error::last_os_error().raw_os_error() == Some(ENOENT),
        "removed cgroup name is not verifiably absent",
    )
}

fn unlink_exact(
    parent: &OwnedFd,
    name: &CStr,
    directory: &OwnedFd,
    expected: Identity,
) -> Probe<()> {
    require_cgroup2(parent)?;
    require_cgroup2(directory)?;
    let parent_id = require_root_mode(&descriptor_stat(parent)?, S_IFDIR)?;
    let held = require_root_mode(&descriptor_stat(directory)?, S_IFDIR)?;
    check(
        held == expected && held.same_mount(parent_id),
        "removal identity or mount changed",
    )?;
    require_name_matches(parent.as_raw_fd(), name, directory)?;
    native(os_result(
        unsafe { held_unlinkat(parent.as_raw_fd(), name.as_ptr(), AT_REMOVEDIR) } as isize,
    ))?;
    Ok(())
}

impl DomainControls {
    pub(in super::super) fn terminate_owned(self) -> KilledDomain {
        self.cancel_release();
        let kill_outcome = request_kill(&self.supervisor_kill);
        KilledDomain { controls: self, kill_outcome }
    }

    // Historical empty-domain component fixture only. Production controller
    // must bring a separately prearmed cleanup owner into KilledDomain::drain.
    #[cfg(test)]
    pub(in super::super) fn terminate_and_drain(
        self,
        absolute_deadline_ns: u64,
    ) -> std::result::Result<DrainedDomain, SettlementQuarantine> {
        let mut stage = SettlementStage::Kill;
        let mut cleanup_deadline = None;
        let mut after_kill_ns = None;
        let result = terminate_then(
            self,
            |controls| { controls.cancel_release(); request_kill(&controls.supervisor_kill) },
            |controls, kill_outcome| -> Probe<(Deadline, DrainReceipt)> {
                stage = SettlementStage::ArmDeadline;
                let observed_after_kill = boottime_ns()?;
                after_kill_ns = Some(observed_after_kill);
                cleanup_deadline = Some(Deadline::arm(absolute_deadline_ns)?);
                let deadline = cleanup_deadline
                    .as_ref()
                    .ok_or("armed cleanup deadline was not retained")?;
                deadline.check()?;
                stage = SettlementStage::Validate;
                controls.revalidate()?;
                require_cgroup2(&controls.provenance.creation.parent.directory)?;
                check(
                    Identity::from(&descriptor_stat(&controls.watchdog_kill)?)
                        == Identity::from(&descriptor_stat(&controls.supervisor_kill)?),
                    "settlement kill descriptors name different inodes",
                )?;
                deadline.check()?;
                stage = SettlementStage::Drain;
                let (empty_ns, poll_wakeups) = wait_empty(&controls.events, deadline)?;
                let receipt = DrainReceipt {
                    name: controls.provenance.creation.name.to_string_lossy().into_owned(),
                    identity: controls.provenance.identity,
                    after_kill_ns: observed_after_kill,
                    kill_outcome,
                    empty_ns,
                    poll_wakeups,
                    cleanup_deadline_ns: deadline.until_ns,
                };
                let deadline = cleanup_deadline
                    .take()
                    .ok_or("successful drain lost its cleanup deadline")?;
                Ok((deadline, receipt))
            },
        );
        match result {
            Ok((controls, (deadline, receipt))) => Ok(DrainedDomain {
                controls,
                deadline,
                receipt,
            }),
            Err(failure) => Err(SettlementQuarantine {
                controls: failure.custody,
                stage,
                error: failure.error,
                removal_succeeded: false,
                kill_outcome: failure.kill_outcome,
                cleanup_deadline,
                after_kill_ns,
                drain_receipt: None,
            }),
        }
    }
}

impl KilledDomain {
    pub(in super::super) fn outcome(&self) -> KillOutcome { self.kill_outcome }

    pub(in super::super) fn drain(
        self,
        mut deadline: Deadline,
        cleanup_grace_ns: u64,
    ) -> std::result::Result<DrainedDomain, SettlementQuarantine> {
        let Self { controls, kill_outcome } = self;
        let mut stage = SettlementStage::Kill;
        let mut after_kill_ns = None;
        let result = (|| -> Probe<DrainReceipt> {
            check(kill_outcome == KillOutcome::Written, "original held cgroup.kill request failed")?;
            stage = SettlementStage::ArmDeadline;
            let observed = boottime_ns()?;
            after_kill_ns = Some(observed);
            let earlier = observed.checked_add(cleanup_grace_ns).ok_or("cleanup cutoff overflow")?
                .min(deadline.effective_until_ns());
            deadline.narrow_to(earlier)?;
            deadline.check()?;
            stage = SettlementStage::Validate;
            controls.revalidate()?;
            require_cgroup2(&controls.provenance.creation.parent.directory)?;
            check(Identity::from(&descriptor_stat(&controls.watchdog_kill)?)
                == Identity::from(&descriptor_stat(&controls.supervisor_kill)?),
                "settlement kill descriptors name different inodes")?;
            deadline.check()?;
            stage = SettlementStage::Drain;
            let (empty_ns, poll_wakeups) = wait_empty(&controls.events, &deadline)?;
            Ok(DrainReceipt {
                name: controls.provenance.creation.name.to_string_lossy().into_owned(),
                identity: controls.provenance.identity,
                after_kill_ns: observed,
                kill_outcome,
                empty_ns,
                poll_wakeups,
                cleanup_deadline_ns: deadline.effective_until_ns(),
            })
        })();
        match result {
            Ok(receipt) => Ok(DrainedDomain { controls, deadline, receipt }),
            Err(error) => Err(SettlementQuarantine {
                controls, stage, error, removal_succeeded: false, kill_outcome,
                cleanup_deadline: Some(deadline), after_kill_ns, drain_receipt: None,
            }),
        }
    }
}

impl DrainedDomain {
    pub(in super::super) fn peer_cancelled(&self)->c_int{self.controls.peer_cancelled()}
    pub(in super::super) fn receipt(&self) -> &DrainReceipt {
        &self.receipt
    }

    pub(in super::super) fn cleanup_deadline(&self) -> &Deadline {
        &self.deadline
    }

    /// # Safety
    /// The trusted terminal owner must already have reaped every owned payload,
    /// watchdog and adopted child, closed the permit, and prevented any future
    /// placement. This method name is not proof of those obligations. The native
    /// consuming controller seals these obligations with its private ready
    /// state after actual direct/adopted reaps. The historical empty fixture
    /// also calls this method where no child or permit ever existed. Neither
    /// source path is a production arming/placement implementation.
    pub(in super::super) unsafe fn remove_after_reaping(
        self,
    ) -> std::result::Result<RemovedDomain, SettlementQuarantine> {
        let Self { mut controls, deadline, receipt } = self;
        let mut stage = SettlementStage::Remove;
        let mut removal_succeeded = false;
        let result = (|| -> Probe<u64> {
            deadline.check()?;
            controls.revalidate()?;
            deadline.check()?;
            let creation = &controls.provenance.creation;
            unlink_exact(
                &creation.parent.directory,
                &creation.name,
                &controls.directory,
                controls.provenance.identity,
            )?;
            removal_succeeded = true;
            stage = SettlementStage::Confirm;
            require_name_absent(&creation.parent.directory, &creation.name)?;
            // All exact direct/adopted reaps preceded this consuming edge.
            // Failed unmap retains the original mapping even if unlink succeeded.
            if let Some(gate)=controls.gate.take() {
                match unsafe{gate.destroy()} {
                    Ok(())=>controls.gate_release=Some((0,0)),
                    Err((gate,result,error))=>{
                        controls.gate=Some(gate);controls.gate_release=Some((result,error));
                        return Err(format!("settled shared gate unmap refused: result={result} errno={error}"));
                    }
                }
            }
            deadline.check()
        })();
        match result {
            Ok(removed_ns) => Ok(RemovedDomain { drain: receipt, removed_ns, gate_release:controls.gate_release }),
            Err(error) => Err(SettlementQuarantine {
                controls,
                stage,
                error,
                removal_succeeded,
                kill_outcome: receipt.kill_outcome,
                cleanup_deadline: Some(deadline),
                after_kill_ns: Some(receipt.after_kill_ns),
                drain_receipt: Some(receipt),
            }),
        }
    }
}

impl CreatedLeaf {
    /// # Safety
    /// Only the trusted no-payload provisioning-failure path may call this.
    /// No watchdog or payload was ever created for this leaf, actual dedicated
    /// context ECHILD and restoration have completed, and placement is closed.
    /// A bare mkdir/name token is insufficient: the original directory anchor
    /// and current exact empty-domain/name/mount provenance must all validate.
    /// This path reports no invented kill, signal or reap receipt.
    pub(in super::super) unsafe fn remove_unactivated_after_empty(
        self,
        deadline: Deadline,
    ) -> std::result::Result<RemovedUnactivatedLeaf, UnactivatedLeafQuarantine> {
        let mut stage = SettlementStage::Validate;
        let mut removal_succeeded = false;
        let mut empty_checked_ns = None;
        let result = (|| -> Probe<(u64, Identity)> {
            deadline.check()?;
            self.validate_unactivated()?;
            empty_checked_ns = Some(deadline.check()?);
            let anchor = self.anchor.as_ref().ok_or("unactivated anchor missing before removal")?;
            stage = SettlementStage::Remove;
            unlink_exact(&self.parent.directory, &self.name, &anchor.directory, anchor.identity)?;
            removal_succeeded = true;
            stage = SettlementStage::Confirm;
            require_name_absent(&self.parent.directory, &self.name)?;
            Ok((deadline.check()?, anchor.identity))
        })();
        match result {
            Ok((removed_ns, identity)) => Ok(RemovedUnactivatedLeaf {
                name: self.name.to_string_lossy().into_owned(), identity,
                empty_checked_ns: empty_checked_ns.expect("successful validation recorded its clock"),
                removed_ns, cleanup_deadline_ns: deadline.effective_until_ns(),
            }),
            Err(error) => Err(UnactivatedLeafQuarantine {
                created: self, deadline, stage, error, removal_succeeded, empty_checked_ns,
            }),
        }
    }
}
