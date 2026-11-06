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

**Next:** The wiki Platform Support page states the tier of each named place.

[↑ Back to Contents](#contents)
