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

The native arm64 probe on macOS 27.0.1 first forked and reaped a baseline child. It then held a same-UID peer alive, verified the process-local limit of two, and observed `EAGAIN` from the witness fork. The peer exited after its release pipe closed. The evidence covers rejection with a live peer. An exclusive-UID positive witness and the backend's repeated escape tests still require an isolated administrator-provisioned lease. [Native probe evidence](../../../tests/evidence/macos-uid-witness-20261106.zip)

**Next:** The wiki Platform Support page states the tier of each named place.

[↑ Back to Contents](#contents)
