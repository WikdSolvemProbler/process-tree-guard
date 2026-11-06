<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# benchmark

## Contents

- [benchmark](#benchmark)

This folder holds the preserved competitor set of the suite. The campaign runs against these competitor programs as found.

**Belongs here:** The preserved set, and the results record of the benchmark.

**Does not belong here:** Run output of a campaign. A run writes its output in its own run folder, not here.

`results-record.json` states the outcomes, the SHA record, and the competitor programs of the benchmark.

## Windows capture, 6 November 2026

The campaign recorded all 1,940 planned slots in 56 minutes 53 seconds. The seeded order, 1,680 measured rows, and 1,940 successful slot Job drain receipts verified. The harness reported an incomplete certification with 44 standing losses. The startup comparison was unevaluated under its full eligibility rules, and resource and scaling gates failed. Manual usability remained unevaluated. Host CPU before launch was 22.7–27.8%, so this capture does not establish a quiet-host speed record.

The guard's measured fast-path medians were 65.0 ms in `NO_WINDOW` and 74.3 ms in `DETACHED`. ProcessKit measured 86.9 ms and 546.4 ms, and nuewframe measured 69.9 ms and 507.3 ms. These are whole-slot medians under the observed host conditions. SysPrims' default stderr did not match the relay workload. The guard left no observed survivors in 204 measured kill and scaling slots. Nuewframe left survivors in all 204 of its corresponding slots. The harness recorded these observations before draining the slot Jobs.

The archive contains the original report, all observations and ProcessKit event logs, the audit and host telemetry, the exact configuration, and both earlier stopped attempts. The capture process exited 1 after finalizing the package because its redirected Windows console could not encode a report character. A later display-only fix replayed the report under that code page without modifying the package. The archive index binds each file by SHA-256. [Download the evidence](../evidence/windows-campaign-20261106.zip)

Archive SHA-256 `ef653c4fb2921adc9e466f767225ba257b26e0b9adecf354b8000f30ccf751bd`.

## Windows comparison with repaired Wake, 6 November 2026

The observed-load x64 MSVC run compared consolidated commit 8bf828a with Windows-repaired Wake commit 9c6f0df. The matched builds used the same release profile and toolchain. Two independent reviewers approved the exact manifest. The run completed 2,967 slots, including 2,880 measured trials, 72 warmups, and 15 controls.

| Manual startup | Median | 90th percentile |
| --- | ---: | ---: |
| Consolidated guard | 84.2891 ms | 107.1302 ms |
| Repaired Wake | 84.2844 ms | 99.2223 ms |
| Bare fixture | 54.4989 ms | 64.9063 ms |

The consolidated guard was slower on one of the 16 prespecified contrasts. The manual-startup 90th percentile was **7.9079 ms slower**, with a primary interval of **[2.6032, 17.0082] ms**. The estimate was positive in both chronological halves and all six launch orders. The other 15 contrasts were unresolved. The nominal intervals assume stationary weak dependence across chronological blocks. The separate quiet-host campaign and the speed acceptance requirement remain open.

The archive contains all raw slots and terminal receipts, the failed zero-row attempt, the matched-build inputs and Windows-repaired Wake source, and the exact analysis. A fresh extraction reproduced all 16 contrasts and the original analysis file byte for byte. [Download the evidence and Windows replay package](../evidence/windows-wake-directional-20261106.zip)

Archive SHA-256 56d7e8282ce249dc5bfc3ab984ea03e97d99b463bb85e96db1684a7992f47038.

## Windows startup phase diagnostic, 6 November 2026

All 324 trials completed under the separately reviewed phase protocol. The original fixture produced a +2.6335 ms manual-startup p90 contrast, below the preregistered +3.9079 to +11.9079 ms anchor range. Its second-half contrast was −2.6100 ms. Five per-arm p90 comparisons between fixture conditions also exceeded the 2 ms threshold. Seven of 19 calibration checks failed, leaving phase localization inconclusive.

The earlier +7.9079 ms contrast remains the result of the larger directional comparison. The archive preserves the preregistration, all phase trials, the initial zero-row admission refusal caused by a missing protocol hash, and the replay. A fresh extraction reproduced the exact analysis bytes using matching local Python and PowerShell executables. [Phase diagnostic evidence and replay](../evidence/windows-wake-phase-20261106.zip)

Archive SHA-256 f13a78229c9b66c963dca3748c32fdbd813a6dfcbd95eb0e1c5bf37973c802a0.

## Windows Job event feasibility

A private completion port delivered fixture PIDs from an otherwise empty Job. The observer opened each process, verified its image and Job membership while live, and read the terminal timestamps from the same held handle. Both the synthetic testkit lane and the nested launcher lane passed all 28 cases. Each lane included 24 immediate exits and four 50 ms waits. Every Job drained, and a synthetic packet naming an outsider was refused.

Job completion messages can be lost. Any future phase experiment must account for missing observations and calibrate the observer against the uninstrumented workload. This proof covers fixture observation only. [Raw feasibility evidence](../evidence/windows-job-events-20261106.zip) · [Microsoft Job notification documentation](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_associate_completion_port)

## Windows standalone manual-startup replication

The standalone run retained the original manual-startup order within 40 blocks, all 72 warmups, and all 15 controls. It completed 807 slots, including 240 measured triplets. The binaries, original fixture, native timing bracket, and guard settings matched the earlier comparison. The measured sequence omitted the intervening AUTO and wait workloads.

| Manual startup | Median | 90th percentile |
| --- | ---: | ---: |
| Consolidated guard | 37.9071 ms | 52.1849 ms |
| Repaired Wake | 37.0708 ms | 49.3178 ms |
| Bare fixture | 23.7398 ms | 30.1599 ms |

The p90 contrast was +2.8671 ms with a primary interval of [−5.5511, +8.1285] ms. Both halves were positive and one launch order was negative. The standalone contrast was unresolved and failed the preregistered calibration gate for phase work. The previous mixed-workload result and the quiet-host issue retain their separate scopes. A fresh archive extraction reproduced the exact analysis bytes. [Raw replication and offline replay](../evidence/windows-wake-anchor-20261106.zip)

Archive SHA-256 d6427f0343c829dff858cf850fee22b86cf8352e3047c7946c79ea8f2b33cf4c.

**Next:** The [competitors-001 folder](competitors-001/README.md) holds the set itself.

[↑ Back to Contents](#contents)
