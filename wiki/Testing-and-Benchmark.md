# Testing and Benchmark
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page guides the testing and benchmark proof for process-tree-guard. The suite is the evidence of the guard. A benchmark is a test. A result that a reader cannot read is evidence with no use.

## Contents

- [What the suite is](#what-the-suite-is)
- [Build and run](#build-and-run)
- [Read a campaign result](#read-a-campaign-result)
- [Counts](#counts)
- [The resource_note vocabulary tie](#the-resource_note-vocabulary-tie)

## What the suite is

The suite holds in-tree Rust tests and one campaign suite. The in-tree tests cover the guard code in the tree. The campaign suite builds the four programs and runs a campaign against competitor programs as found. A competitor loss is a work order, not a footnote. The suite repairs a general defect and runs the campaign again after a loss.

[↑ Back to Contents](#contents)

## Build and run

Build the suite package on a Windows laptop with one command:

```powershell
.\build.ps1
```

Run the suite tests with one command from the tests folder:

```
python3 run_tests.py
```

Run the in-tree tests from the tree root with one command:

```
cargo test
```

Give every command a finite deadline. The suite commands carry deadlines of their own. The suite stands in [tests/](../tests/README.md). Run `build.ps1` and `run_tests.py` from that folder.

[↑ Back to Contents](#contents)

## Read a campaign result

A campaign result names each cell as measured, as a standing loss, or as not measured with a reason. A standing loss is a cell that the campaign counts as a loss until a repair closes it. Read the standing losses first. A complete campaign with standing losses is an honest result. A campaign that hides a loss is not a result at all.

The completeness accounting maps each branch of the failure tree to a named test. The accounting also maps each documentation claim to a named test. A branch with no test stands as a tier-labelled non-execution with a revival path.

[↑ Back to Contents](#contents)

## Counts

Results of the release test run for 0.5.0, Wednesday, 04NOV2026. The logs are attached to the v0.5.0 release.

| Run | Where | Result |
|---|---|---|
| In-tree tests, `cargo test --release --no-fail-fast` | Windows 11, x86-64, Microsoft Visual C++ (MSVC) toolchain | 437 tests: 433 passed, 4 failed, 0 ignored |
| Library unit tests | same run | 264 of 264 passed |
| Automatic mode tests (`tests/auto.rs`) | same run | 21 of 21 passed |
| Hook program tests (`tests/plugins_antigravity.rs`) | same run | 21 of 21 passed |
| Battery (`tests/battery.rs`) | same run | 49 of 49 passed |
| Guard tests (`tests/guard.rs`) | same run | 78 of 82 passed |
| Harness logic, `python3 tests/run_tests.py` | Ubuntu 24.04, aarch64 | 647 tests: 592 passed, 54 skipped, 1 failed (a timing gate defined for a sampler-verified quiet host; it varies on a virtual machine) |
| Type and borrow check, debug and release, zero warnings | Linux aarch64 and macOS arm64 hosts | x86-64, aarch64 and i686 Windows MSVC, and x86-64 Windows GNU: all pass |
| Check with the declared minimum Rust, 1.88 | Linux aarch64 host | passes |

Open at this version, each a test that needs redesign rather than a product change: G62, G72, G80 and G83 suspend the guard through a new handle to it, which the guard denies by design (see [SECURITY.md](../SECURITY.md)); they can pass only for an elevated caller. One library test, `engine::tests::deep_owner_expiry_after_existing_timer_preserves_native_last_error`, is timing dependent: it failed in one of three full runs on the final tree, when the whole suite ran in parallel, and passed in five of five runs on its own.

The benchmark campaign did not complete for this version: run-20261104T230944Z-s20261029 recorded 922 of 1,940 slots and was stopped. It is not a speed result, and this release makes no speed claim.

The counts for this version come from the release test run on Windows. The full record stands in the suite accounting at [tests/COMPLETENESS.md](../tests/COMPLETENESS.md).

The final accounting states run, skipped, and failed counts in one line. No count in this set counts one test twice across two harnesses. A count from the harness logic on Linux is a harness fact, not a Windows backend fact.

[↑ Back to Contents](#contents)

## The resource_note vocabulary tie

The named open item stands here in its permanent home. Name: resource_note vocabulary tie, converse completeness unproven. The property is this: no code path in the suite writes a `resource_note` value outside the approved vocabulary. The covered set stands in the suite accounting. The escape classes stand in a list. The classes are call-spelled writes and a cross-module helper write. The classes are a dict-merge rebind under a variable key. The classes are indirection that hides the executing callable. Indirection covers helper pass-throughs, alias chains, inheritance, and `__code__` replacement.

Two closure criteria stand. Give a completeness argument that survives adversarial review. Or restructure the write sites so that the property holds by construction, in the choke-point form.

This item is about the bookkeeping of the suite itself. It does not change the guard, the method of measurement, or any measured result. The item stands as carried, not closed.

[↑ Back to Contents](#contents)
