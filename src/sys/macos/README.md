<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/sys/macos

## Contents

- [src/sys/macos](#srcsysmacos)

This folder is a named place for a macOS backend. It holds this README and nothing else.

**Belongs here:** This README only.

**Does not belong here:** Code of any kind. A file here that is not this README is a defect at this version.

macOS is a named place, not a backend. The revival condition stands on the wiki Platform Support page.

## UID witness probe

XNU charges a fork to the caller's real UID before admitting the child. A dedicated-UID design can use a non-root verifier with `RLIMIT_NPROC=2` as a conservative emptiness witness, provided an exclusive lease prevents new work under that UID. [XNU fork accounting](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_fork.c)

The native arm64 probe on macOS 27.0.1 first forked and reaped a baseline child. It then held a same-UID peer alive, verified the process-local limit of two, and observed `EAGAIN` from the witness fork. The peer exited after its release pipe closed. The evidence covers rejection with a live peer. [Native probe evidence](../../../tests/evidence/macos-uid-witness-20261106.zip)

## Dedicated UID fixture

An ephemeral hosted arm64 macOS 27.0.1 VM verified a dedicated worker account and a different-UID peer. A supervisor with the worker's permanent credentials swept a detached, double-forked fixture. Each of 20 rounds captured the exact escaped process's SIGKILL exit, verified the worker UID was empty, repeated the process-capacity witness, and confirmed the peer survived. The final supervisor receipt confirmed both UIDs drained. [Hosted run](https://github.com/WikdSolvemProbler/process-tree-guard/actions/runs/42213887803) · [Transcripts and earlier failures](../../../tests/evidence/macos-dedicated-uid-20261106.zip)

The fixture parks each escaped process before the sweep. Production still needs a verified exclusive lease, authenticated admission, credential containment, and fork-race coverage.

## Detached fork churn

The fork-churn fixture compiled on the Tailscale Mac as an ordinary user. Its privileged hosted run recorded all 20 detached rounds with an empty-worker witness, native SIGKILL for the anchored process, and a live different-UID peer. Three rounds observed a successful fork entirely between sweep dispatch and reap. None recorded a successful fork entirely after the pre-kill marker. The all-round overlap gate failed, and the internal broadcast snapshot was unobserved. The parked regression passed separately. [Hosted run](https://github.com/WikdSolvemProbler/process-tree-guard/actions/runs/42228789224) · [Sources, compile and runtime receipts](../../../tests/evidence/macos-fork-churn-20261106.zip)

## Native kqueue process events

An ordinary-user probe on physical macOS ARM64 observed an ordinary fork event with a zero data field. On the same gated child, separate recursive tracking registrations using NOTE_TRACK, NOTE_TRACKERR and NOTE_CHILD each returned ENOTSUP. An exit-status subscription on that child succeeded, and its event agreed with waitpid on exit 7. The child was reaped naturally and a separate check found the process group gone. This rules out the tested kqueue registration path as a source of recursive child identity on this kernel. [Source, raw native receipts, and cleanup](../../../tests/evidence/macos-kqueue-native-20261106.zip)

## Teardown requirements

Apple's published XNU source implements broadcast signaling by collecting a PID list under the process-list lock, releasing the lock, and then signaling the listed processes. The collection skips processes in the middle of being created. These source observations describe the pinned revision below. The hosted fixture did not observe the installed kernel's internal snapshot. [Broadcast signaling](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_sig.c#L1566-L1598) · [PID collection and delivery](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_proc.c#L3853-L3901)

This permits a schedule in which a listed process creates a same-UID child after the collection and dies in the sweep. The new child can take the same role at the next collection. With at least one free process slot on each pass, any fixed sweep count admits this schedule. This is a source-based counterexample to a termination bound for repeated sweeps. It is separate from the observed hosted outcomes.

A successful fork by a trusted non-root verifier with both `RLIMIT_NPROC` limits set to two proves that the verifier and its new reservation were the only charges to the leased real UID at the accounting increment. A drained receipt also requires sealed admission, verified witness output, and evidence that all surviving workload processes and in-flight creations remain charged to that UID. A missing or failed witness leaves the lease quarantined and unavailable for reuse. The backend still needs a mechanism that prevents continued process creation during teardown and drains the existing members. [XNU fork accounting](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_fork.c#L236-L273)

Two related interfaces have narrower documented roles. Apple's descendant-scoped Endpoint Security client requires an entitlement and delivers authorization and notification events. The published coalition termination request marks the request and reports completion when the active count reaches zero. Both would require separate proof of bounded whole-tree teardown. [Endpoint Security descendants client](https://developer.apple.com/documentation/endpointsecurity/es_new_descendants_client%28_%3A_%3A%29?changes=_2_1_1&language=objc) · [XNU coalition termination request](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/kern/coalition.c#L2030-L2085) · [XNU coalition count and notification](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/kern/coalition.c#L1775-L1818)

### Retained child accounting

An ordinary-user test on physical macOS 27.0.1 ARM64 checked `fork` and `posix_spawn` at the same process-local limit before, during, and after retaining one exited child. Both calls succeeded before retention, returned `EAGAIN` while the exact child was held with `waitid(WNOWAIT)`, succeeded at the next higher limit while it was held, and succeeded again at the original limit after it was reaped. All eight trials completed. The first attempt failed calibration because it reused the inherited hard limit after XNU clamped it. The corrected attempt recorded the effective limits. Both attempts verified scratch and process group cleanup. [Sources and both native attempts](../../../tests/evidence/macos-nproc-accounting.zip)

The observed one-entry shift supports the hypothesis that a retained exit could help fence fork admission for a dedicated UID. The shared-UID test covers accounting only. Credential transfer at saturation, reserve custody, concurrent fork publication, and bounded cleanup on supervisor loss remain open. [XNU reap accounting](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_exit.c#L2593-L2617) · [XNU process limit clamping](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/kern_resource.c#L1568-L1589)

Archive SHA-256 edcf989f29c5769973ab339c9ea07eede811fe38d636245d7ceb931bec085610.

A second physical Mac experiment tested who controls a retained child. The same-UID nonparent received `ECHILD` in both cases. The exact retained child disappeared after explicit reaping in the control and after the owning parent exited in the other case. This verifies that the reservation depends on continued custody on the tested macOS build. The first native compile failed its format checks. Both attempts and independent scratch and process group cleanup receipts are preserved. The test used ordinary credentials and did not exercise privileged UID transfer or concurrent publication. [Custody protocol, sources and receipts](../../../tests/evidence/macos-reserve-custody.zip)

Archive SHA-256 470527f7e8fd5f95c74404638035da5c082fc41a775b3e0ba14c4ff19dad34eb.

Backend acceptance still requires the shared process behavior tests and 20 escape repetitions with no survivors. [Shared behavior tests](../../../tests/support/shared_process_behavior.rs) · [Issue #1](https://github.com/WikdSolvemProbler/process-tree-guard/issues/1)

### Coalition source review

Coalitions offer kernel-maintained fork grouping, but the examined XNU termination request does not kill live members or stop existing members from forking. It marks the coalition for an empty notification. Adoption refuses a terminated or reaped coalition, while termination is finalized only after the active count reaches zero. The syscall also requires privileged-coalition membership. Consequently, root or privileged coalition access alone does not establish the required bounded kill/drain fence. These source findings leave coalitions useful as a grouping component and leave other private interfaces unqualified. The dedicated-UID fork-fence and privileged-custody requirements remain open. [Termination and fork semantics](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/kern/sys_coalition.c#L88-L93) · [Adoption](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/kern/coalition.c#L1847-L1888) · [Termination implementation](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/kern/coalition.c#L2034-L2085)

This review pins public XNU revision `f6217f891ac0bb64f3d375211650a4c1ff8ca1ea`. It establishes no mapping to the installed macOS kernel and excludes no unexamined private interface. [Structured source references](../../../tests/evidence/macos-coalition-source-review.json)

**Next:** The wiki Platform Support page states the tier of each named place.

[↑ Back to Contents](#contents)
