# Platform Support
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page is the platform support page of process-tree-guard and the public home of its tier discipline. A tier is the evidence level of a claim. The guard is a universal guard. Proof is per backend, and this page states proof honestly.

## Contents

- [The backend table](#the-backend-table)
- [The architecture table](#the-architecture-table)
- [Why the guard refuses a weak backend](#why-the-guard-refuses-a-weak-backend)
- [Named places and revival conditions](#named-places-and-revival-conditions)
- [What a tier means](#what-a-tier-means)

## The backend table

| Backend | Tier | State |
|---|---|---|
| Windows | Proven backend | One declaration module stands at `src/sys/windows.rs`. The crate refuses a build with no proven backend. |
| Linux | Theory | No backend exists. A named place stands at `src/sys/linux/`. |
| macOS | Theory | No backend exists. A named place stands at `src/sys/macos/`. |

The harness logic of the test suite has Linux proof. That proof does not raise the Linux backend tier. The tiers do not bleed into each other.

[↑ Back to Contents](#contents)

## The architecture table

| Architecture | Tier |
|---|---|
| x86-64, Windows Microsoft Visual C++ (MSVC) | Release target. Proven: the release test run on Windows 11 passed 433 of 437 in-tree tests; the 4 open tests are named on the [Testing and Benchmark](Testing-and-Benchmark.md) page. |
| aarch64, Windows MSVC | Release target. The release build and library unit tests passed natively on Windows 11 ARM64. Native integration tests, including the desktop-dependent cases, are pending. [Main CI run](https://github.com/WikdSolvemProbler/process-tree-guard/actions/runs/42166290419) · [v0.5.0 ARM64 ZIP and checksums](https://github.com/WikdSolvemProbler/process-tree-guard/releases/tag/v0.5.0) |
| i686, Windows MSVC | Type-and-borrow tier only. No execution evidence stands. This target is not a release target. |
| x86-64, Windows GNU toolchain | Type-and-borrow tier only. The build carries no version resource. This target is not a release target. |

`cargo check` passes with zero warnings for `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`, `i686-pc-windows-msvc`, and `x86_64-pc-windows-gnu`. The check passes in the debug profile and in the release profile. The check ran from a Linux aarch64 host and from a macOS arm64 host. A check is type-and-borrow evidence only.

The source carries no architecture conditionals. The declarations are width-clean. That discipline keeps the type tier true.

[↑ Back to Contents](#contents)

## Why the guard refuses a weak backend

A backend must keep the promise. The promise needs placement at creation. The promise needs an end of the whole tree on close. The promise needs a refusal of breakaway. A process group gives none of those three facts. A child leaves a process group by a new session. A double fork hides a child from a watcher. No creation step places a Unix child inside a group before its first instruction.

The guard therefore refuses a build for a target with no proven backend. The refusal text states the fact:

```text
this build has no proven backend for its target; the proven backend at this version is Windows
```

A build for a Linux target or a macOS target stops with that compile error. The proven backend at this version is Windows.

[↑ Back to Contents](#contents)

## Named places and revival conditions

A named place is a folder with one `README.md` and no code. The Linux place stands at [src/sys/linux/README.md](../src/sys/linux/README.md). The macOS place stands at [src/sys/macos/README.md](../src/sys/macos/README.md). No module declaration names those folders. No code in those folders compiles.

A future backend revives a place in one of two ways. Name a mechanism set on that system. That set must give placement at creation. That set must give an end of the whole tree on close. That set must give no breakaway. Or argue a changed promise for that backend as a product decision. Silence on both points is a refusal.

[↑ Back to Contents](#contents)

## What a tier means

Proven means the claim has execution evidence on that platform. Type-and-borrow means the code checks as types and borrows, with no execution. Theory means an argument stands and no check stands. A page that states a platform fact states its tier in the same sentence or in the same table row. A platform claim with no tier is a defect in this set.

[↑ Back to Contents](#contents)
