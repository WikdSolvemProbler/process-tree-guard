<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# evidence

## Contents

- [evidence](#evidence)

This folder holds the recorded evidence of the suite. A test holds each record here to a new run or a new count.

**Belongs here:** The recorded battery record, the summary of the scanner set, the recorded test run, and the preserved `reduced-run` package.

**Does not belong here:** New run results. A new run writes its results in its own run folder.

## Windows correctness reruns

The full Windows x64 suite at b774403 recorded 643 passes and four skips. Both earlier full runs are included. They each reported one infrastructure failure after a Python scale fixture reached its deadline before publishing the complete process list. The final run verified the staged binaries before and after testing. [Raw logs and 647 test certificates](windows-correctness-20261106.zip)

Archive SHA-256 b6254fc68a59ac7d53aa6d4a906e1cc3cf34c72c4ef36cad176bc7358998cc55.

## Linux portable correctness

The exact 47d8f47 archive completed the portable suite on the Tailscale Linux ARM64 guest with 593 passes and 54 skips. The first run reached its guest deadline and recorded a sleeping-launcher test failure before the final traceback. The isolated unchanged test and the full rerun passed. The archive preserves both attempts and independent guest cleanup checks. [Linux correctness evidence](linux-correctness-20261106.zip)

## Owner-pricing timing checks

At `ee885c6`, all five default owner-verification timing tests passed on Windows x64, Linux ARM64, and native macOS ARM64. The class verified actual guard-clock receipts and tested the pricing calculation using the same observations. Both live methods were separately selectable.

The earlier macOS live sleeping-launcher gate failed. All 45 sleep intervals ended before their respective guard clocks opened, but the median prices from the separate conditions differed by 39.2285 ms against a 0.7020 ms bound. The inside-delay sensitivity control passed. The final default tests do not assess stability across separate samples. The live gates retain their original acceptance thresholds and remain available for quiet-host validation. [Raw results and candidate patches](owner-pricing-timing-20261106.zip) · [Live validation method](../benchmark/README.md#owner-verification-timing-checks)

Archive SHA-256 `26a8238f7753fe1ad1034f1b5d803c0c9f16076f87dbed1632397e9d8fff5065`.

## Full correctness after the owner-pricing repair

At `ee885c6`, the full Windows x64 run recorded 643 passes and four skips. The Linux ARM64 guest ran the portable suite and recorded 593 passes and 54 skips. Both counted 647 tests with zero failures and zero errors. The archive includes both sets of certificates, a Windows line-ending preflight refusal, and a Linux transfer interrupted before tests ran. The focused native macOS result is recorded above. [Full correctness evidence](correctness-ee885c6-20261106.zip)

Archive SHA-256 `f733a493ec7a4b8bbc9297b6e635e4832e26682b6d413101dd971634d86b5fb7`.

## Unsupported Linux and macOS build check

The ARM64 Linux guest and physical ARM64 Mac checked the same `6420738` source over Tailscale with Rust 1.99. Both offline Cargo checks exited 101 with exactly one compiler diagnostic, the intentional backend refusal in `src/sys/mod.rs`. Independent cleanup verified both scratch directories, the Linux user cgroup, and the macOS compiler process group were gone. This is build-refusal evidence. [Native compile and cleanup receipts](unsupported-platform-refusal-20261106.zip)

Archive SHA-256 `4b6f54d1e650a8066ae3c05468e14e203d6ca47506e42c1b9205f25487553312`.

## Correctness after the survivor-report repair

The first full runs at `88f5a6d` found three Windows and four Linux count-check failures. The corrected test kept the same scale assertions inside an existing test. Windows x64 at `7f4f32e` recorded 643 passes and four skips. Linux ARM64 at `7462953` recorded 593 passes and 54 skips. Both counted 647 cases with zero failures and zero errors. The five targeted native macOS checks also passed. [Full receipts and earlier failed attempts](survivor-report-correctness-20261106.zip)

Archive SHA-256 `29752be7051c6561dca2bb43ba4ffa8a36e35b76d0e57282863c450b3773631e`.

## Correctness after the survivor-probe and census repairs

The journalled Windows x64 run at e5c6159 recorded 643 passes, four skips, and zero failures or errors across 647 cases. It used the e73981a x64 CI images after verifying the Rust and build sources were unchanged. The per-test journal and certificates agree for every case. The earlier e73981a run exposed two stale census assertions, and the first corrected rerun reached its finite deadline during a recorded Windows sleep interval. Their receipts remain available.

The Linux ARM64 VM and physical macOS ARM64 each passed four targeted tests from b38a598 over Tailscale. The first transfer attempts ended before testing, and the smaller retry preserved all relevant tracked source bytes. Independent checks verified cleanup after both remote attempts. [Full Windows and targeted native receipts](survivor-probe-census-rerun-20261107.zip) · [Earlier Windows failures](windows-survivor-suite-count-failures-20261106.zip)

Archive SHA-256 c2e94681a5e2fb08de685e1339636e9150c1442e363006920a023a95417aa03d.

## SysPrims fast profile review

The optional `tests/campaign-sysprims-fast-warn.json` adds `--log-level=warn` to ordinary SysPrims fast launches. A no-launch census checked all 1,940 planned rows. The shipped configuration reproduced all 1,940 original commands. The optional configuration changed 198 commands by one token and preserved the other 1,742, including kill and malformed commands.

The profile checks the exact recorded argument vector against the configuration bytes whose digest was recorded at load time. A measured SysPrims fast vector that cannot be verified leaves the affected comparison unevaluated. A faster rival can still establish a loss using independently verified timing and output. Twelve guard and rival output mutations reproduced false latency losses on earlier revisions and were held unevaluated on the approved revision. Three archived reports reproduced byte for byte, and both independent Sol reviewers approved the exact patch for code readiness.

This review launched zero benchmark slots. The quiet-host campaign and speed acceptance remain open. [Review scripts, exact sources and receipts](windows-sysprims-fast-profile-review.zip)

Archive SHA-256 f3601f500f8a0e146ba9afdf00aa45d360ae793b25265d8e7b790597ce4dbc98.

## Correctness after the optional SysPrims profile

At `59cc806`, the Windows x64 suite recorded 643 passes, four skips, and zero failures or errors across 647 cases. The per-test journal matched all 647 certificates. The suite reused verified CI binaries from `e73981a` after confirming that the Rust and build inputs were unchanged. Both changed regression methods also passed from the same `59cc806` source on the Linux ARM64 VM and physical macOS ARM64 via Tailscale. Independent checks found both remote scratch directories and their containment scopes gone. [Full Windows and focused native receipts](sysprims-profile-correctness.zip)

Archive SHA-256 7ba2860543706f065009052c0763d0598757ef65bed5e3ea3392a383a8808295.

## Linux cgroup authority qualification

An ordinary-user Linux ARM64 test exercised the authority candidate’s real openat, statx and fstatfs calls. It rejected mismatched inodes, symlinks and incorrect descriptor types, and refused provisioning before creation. The first native attempt exposed incorrect ARM64 open flags. Corrected source passed, and a C program using the guest headers independently matched the selected constants. The canonical installation-path correction passed the same test. The archive preserves the failed attempt, corrected source, successful receipts and independent cleanup checks. Positive root-owned provisioning, installation provenance, generic placement and watchdog integration remain unqualified. [Authority source and raw evidence](linux-cgroup-authority.zip)

Archive SHA-256 `87901397e99a762094f561470a468adad142da62994a7c91a134cd99f61dd031`.

## Rejected exploratory laptop attempt 011

The single approved attempt stopped after 91 of 1,940 recorded slots: all 36 controls and 55 warmups, with zero measured trials. The frozen counter-order gate rejected a sample whose idle and kernel totals each decreased by 156,250 100-nanosecond ticks between immediate reads; derived busy time was unchanged. The containing Job drained from five active processes to zero. A separate native check found all 174 recorded exact process identities absent. The raw package remains provisional and incomplete, and supports no speed conclusion. The archive preserves all records, including six non-ok Nuewframe warmups, without rescoring or discarding them. [Rejected attempt and exact cleanup evidence](windows-laptop-exploratory-attempt-011.zip)

Attempt archive SHA-256 db31b44814b8e9250b8a4e88174a03da481f00212427695469859b4033421171.

## Independent Windows counter diagnostic

Two independent readers reproduced immediate GetSystemTimes component regressions on this host: native C retained 925 pairs with 84 regressions, and Python retained 940 pairs with 45. Both reached their prescribed 15-second deadline before the fixed 1,024-pair first phase finished. Neither reached the affinity phase, and no retry occurred. Derived busy time never decreased in these traces; the largest component decrements were 4,843,750 and 2,500,000 100-nanosecond ticks. These observations rule out a Python-only decoding explanation but establish neither the kernel cause nor a universal error tolerance. The archive retains every source, raw reading, compiler identity, exit and incomplete-control limitation. [Independent counter diagnostic](windows-system-counter-diagnostic.zip)

Diagnostic archive SHA-256 7969fcdca2d147984fe19e90dd672d1583fcaccde5884c0a73d3e059381a36e8.

**Next:** The [`reduced-run` folder](reduced-run/README.md) holds the preserved package.

[↑ Back to Contents](#contents)
