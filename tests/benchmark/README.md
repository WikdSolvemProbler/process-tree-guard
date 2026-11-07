<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# benchmark

## Contents

- [benchmark](#benchmark)

This folder holds the preserved competitor set of the suite. The campaign runs against these competitor programs as found.

**Belongs here:** The preserved set, and the results record of the benchmark.

**Does not belong here:** Run output of a campaign. A run writes its output in its own run folder, not here.

`results-record.json` states the outcomes, the SHA record, and the competitor programs of the benchmark.

The [benchmark specification](../../SPEC_Benchmark.md) records the evaluation contract, review requirements, and freeze gate.

## Windows capture, 6 November 2026

The campaign recorded all 1,940 planned slots in 56 minutes 53 seconds. The seeded order, 1,680 measured rows, and 1,940 successful slot Job drain receipts verified. The harness reported an incomplete certification with 44 standing losses. The startup comparison was unevaluated under its full eligibility rules, and resource and scaling gates failed. Manual usability remained unevaluated. Host CPU before launch was 22.7–27.8%, so this capture does not establish a quiet-host speed record.

The guard's measured fast-path medians were 65.0 ms in `NO_WINDOW` and 74.3 ms in `DETACHED`. ProcessKit measured 86.9 ms and 546.4 ms, and nuewframe measured 69.9 ms and 507.3 ms. These are whole-slot medians under the observed host conditions. SysPrims' default stderr did not match the relay workload. The guard left no observed survivors in 204 measured kill and scaling slots. Nuewframe left survivors in all 204 of its corresponding slots. The harness recorded these observations before draining the slot Jobs.

The archive contains the original report, all observations and ProcessKit event logs, the audit and host telemetry, the exact configuration, and both earlier stopped attempts. The capture process exited 1 after finalizing the package because its redirected Windows console could not encode a report character. A later display-only fix replayed the report under that code page without modifying the package. The archive index binds each file by SHA-256. [Download the evidence](../evidence/windows-campaign-20261106.zip)

Archive SHA-256 `ef653c4fb2921adc9e466f767225ba257b26e0b9adecf354b8000f30ccf751bd`.

An offline replay corrected 32 survivor labels to name the scale path, case kind, and variant. All 44 standing losses, the incomplete verdict, and every other report line matched the baseline replay. The original capture and reports remain in the archive above. [Replayed reports and regression receipts](../evidence/windows-survivor-report-replay-20261106.zip)

Archive SHA-256 `03d40c2964ee48306489117daaab66b0b43a25f2b9879b0888d045086fc8f7ea`.

The archived SysPrims arm recorded 180 measured and 18 warmup fast slots. All 198 exited 0 and matched stdout. Every slot recorded 334 stderr bytes and failed the expected-stderr check. The invocations used the default CLI log settings. The published v0.2.4 source defaults to INFO-level stderr tracing and emits messages on successful runs, consistent with the captured mismatch. The freeze rule requires a successful fast-path median for each rival, so SysPrims left the startup comparison unevaluated in both contexts. Host quietness is a separate admission requirement. [SysPrims log defaults](https://github.com/3leaps/sysprims/blob/v0.2.4/crates/sysprims-cli/src/main.rs#L77-L85) · [SysPrims stderr writer](https://github.com/3leaps/sysprims/blob/v0.2.4/crates/sysprims-cli/src/main.rs#L691-L715)

### SysPrims output qualification

Two independent Sol reviewers approved the same bounded eight-slot Windows diagnostic. The pinned SysPrims v0.2.4 default produced 334 stderr bytes and failed the relay's byte oracle in both console contexts. The explicit top-level --log-level=warn option preserved the exact 18-byte stdout and 13-byte stderr in both contexts. All four kill controls returned 124 with no observed marked descendant and natural Job drainage. Default kill logging retained the INFO reliability diagnostic, which the WARN control filtered.

The result qualifies the tested WARN fast-path output. It supplies no comparative timing or protection-equivalence verdict. A fresh campaign can review WARN for fast-path relay slots while retaining default logging for kill, scaling, and malformed slots. The original campaign keeps its recorded verdict. The archive includes both approvals, the original and revised diagnostic designs, all raw streams and Job receipts, and a replay that reproduced the same result from a fresh extraction. [Correctness evidence and replay](../evidence/windows-sysprims-output-20261106.zip)

### CPU counter sensitivity

All 1,940 stored arm-process CPU readings fell on a 15.625 ms grid. The original comparator reproduced 30 failing workload comparisons and 77 failing ranks. Adding 15.625 ms to each rival CPU reading left nine comparisons and 14 ranks failing. Adding 31.25 ms instead left four comparisons and eight ranks failing. The remaining comparisons were the empty-command and over-long-argument cases against nuewframe and SysPrims.

These post-hoc arithmetic allowances favor the guard. The strict verdict uses the original readings, and the grid supplies no measurement error bound. [Exact analysis, affected cases, and replay script](../evidence/windows-cpu-accounting-audit-20261106.zip) · [Windows CPU counter documentation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)

### CPU counter calibration

Independent replays on Linux ARM64 and native macOS ARM64 reproduced the Windows CPU audit byte-for-byte after line-ending normalization.

The first bounded Windows calibration refused an elapsed pulse that exceeded its preregistered limit. A separate fixed-work calibration then compared two idle controls with 8,192 and 32,768 iterations. Raw process cycles distinguished the work levels in all 12 measured quartets. `GetProcessTimes` passed zero of the same 12. The result is specific to the synthetic workload and observed host. Cycles remain in raw units, and the original campaign verdict retains its original CPU-time readings. [Both calibration receipts and the cross-platform replay](../evidence/windows-cpu-calibration-20261106.zip) · [Process cycle counter](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-queryprocesscycletime)

Archive SHA-256 `0e6dae0c86b1e37ada4d5598d094d926382c236441f85a591a24b7242d808a17`.

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

## Windows valid-launch resolver diagnostic

The proposed empty-executable fix rejected a malformed command that otherwise reached Windows executable-image validation. The controlled regression passed on the candidate and failed on its matched baseline. The timing diagnostic separately exercised valid launches from the baseline, candidate, and exact Windows-repaired Wake build on this laptop under observed load.

Two independent Sol reviewers approved the same frozen manifest. All 12 controls and 288 measured launches completed, with exact exit and stream checks and natural slot Job drainage. The primary and backup guards both returned zero. No recorded process instance remained in the wrapper's post-run check. A fresh archive extraction reproduced the analysis byte for byte.

| Windows context | Fixture argument | Paired median candidate minus baseline | Paired median candidate minus repaired Wake |
| --- | --- | ---: | ---: |
| NO_WINDOW | Absolute path | -7.557 ms | +2.367 ms |
| NO_WINDOW | Bare name | +3.183 ms | +7.017 ms |
| DETACHED | Absolute path | +2.487 ms | +6.798 ms |
| DETACHED | Bare name | -3.999 ms | +3.884 ms |

The median paired lookup-form contrast was +4.721 ms in NO_WINDOW and -0.171 ms in DETACHED. Those contrasts describe end-to-end elapsed time. The candidate's observed paired medians were above Wake's in each cell. Its changes relative to the matched baseline had mixed signs, and these data do not isolate a consistent resolver cost. The candidate remains unmerged and the separate speed acceptance requirement remains open.

The archive contains the protocol, source and executable identities, candidate patch, prior regression evidence, all raw slots and cleanup receipts, the approval and bounded invocation, and the replay. Archive SHA-256 is e7bdc5ff85068e8ed03176628db5a4ad60014cbc9973577b1891f381404c1293. [Diagnostic evidence and replay](../evidence/windows-resolver-valid-launch-20261106.zip)


Independent offline replays of all 300 Windows launch records ran through Tailscale SSH on Linux ARM64 and physical macOS ARM64. Each reproduced the Windows analysis byte for byte after line-ending normalization. Independent post-run checks found no replay scratch directories, Mac replay process group, or Linux replay scope. [Native replay and cleanup evidence](../evidence/resolver-native-replay-20261106.zip)

The earlier directional comparison built both images with rust-lld.exe. The resolver baseline and candidate used Microsoft link.exe 14.51.36260.0, while repaired Wake retained its exact directional image. The baseline and candidate therefore share a build recipe, and their comparisons against Wake include a linker difference. The archived image audit also records a larger consolidated text section and one additional direct DLL import. No phase timing was measured by the audit. [Build records and PE replay](../evidence/windows-wake-build-audit-20261106.zip)

## Quiet-host telemetry qualification

A one-minute Windows x64 observation recorded 12 five-second windows with median CPU at 2.25%, maximum CPU at 7.85%, and more than 6.7 GiB of available physical memory throughout. Those windows met the draft preflight thresholds. This establishes the observed minute's conditions. Each full campaign needs its own admission and postflight.

A native nested-Job calibration matched the enclosing Job's CPU deltas to the sum of two exact process-handle readings over three windows. Both processes were active in the first two windows and had exited by the third. The Job drained naturally. A separate settled fixture and five offline controls also passed. The archive retains the initial clock-binding failure. These checks launched zero benchmark slots. The estimate bounds the bracketed counter readings, with quantization and accounting lag limiting its interpretation as physical CPU activity.

The archive binds the draft source and binary inputs. A full run still requires a reviewed launcher with continuous phase coverage, process and power checks, and approval on the same frozen manifest. [Sampler, staged input identities and raw receipts](../evidence/windows-quiet-host-sampler.zip)

Archive SHA-256 de38e7ba32962d6850060c57d54865e1fcd33019d112b02631370140ccd66a11.

### Sealed launcher qualification

The candidate launcher binds the source and binary inventory, effective environment digest, run ID, configuration, seeded order, raw host samples, process inventory, and complete campaign package. Its offline audit accepted a current-format replay of all 1,940 historical observations, retained the 44 standing losses, bound 473 files, and passed seven refusal controls. It requires the ProcessKit event files named by each slot and explicitly records the parser refusals that produce no event. It does not equate a completed package with a passing campaign.

Six additional native windows compared enclosing Job CPU deltas with the exact retained process handles when nested fixture children exited before and after a sampling boundary. All differences were zero. The final median and coverage use only scheduled complete five-second windows. Terminal intervals still undergo contamination checks. Twelve synthetic tests passed, including an idle-tail case that had changed a failing median into a passing one in the earlier draft.

Both independent Sol reviewers approved exact revision 007 with no unresolved implementation objections, conditional on a successful natural lifecycle control before the full campaign. Three revision-007 lifecycle attempts refused in preflight after observing external Git timeout or Python runtimes. They launched zero benchmark slots. Earlier drafts and their failed checks remain preserved.

The strict census classifies generic external runtime names as competing work. Both reviewers identified that this is broader than evidence of an actual competing test. A refined census would need its own frozen definition and review. The strict quiet-host issue and speed acceptance remain open. [Exact sources, manifest, controls, refusals and replay](../evidence/windows-quiet-launch-qualification.zip)

Archive SHA-256 22297e02b935643b11d4a58fdd01895021f4b3e41edd77541c35fcc30e9c9de6.

### Exploratory laptop protocol 011

One prospective 1,940-slot exploratory capture is preregistered with manifest SHA-256 fa64410ae847be10735cdf5a69ce71acb7213a2d9bbe3b7695c83a8f15df454a. Both independent Sol reviewers approved the same source and bounded command. This protocol retains the numerical CPU, memory, power, clock, observer, timing and cleanup gates. Its scoped image census vetoes identified compilers and pinned benchmark content outside the owned Job. Other inspected runtimes are recorded as unclassified background and their CPU remains in the residual. The archive states the sampled-name and on-disk identity limitations. This scope is exploratory, and the strict quiet-host issue remains open.

The native lifecycle control completed preflight, a 62.4-second sleeper, natural Job drainage, postflight and raw telemetry replay. Its final inventory verifier exposed a reused local creation-time variable. The corrected verifier accepts the immutable native receipts and rejects forged identities. All 26 synthetic and saved-receipt checks passed. The collector, sampler, gates and raw replay were unchanged by that correction.

The preregistration archive preserves the reviewed source, exact current binaries, all staged input hashes, native control receipts, earlier refusals, the metadata and timestamp compatibility repairs, and both approvals. The full capture had executed zero benchmark slots when this archive was produced. Its complete package, actual gate values, losses and exclusions require separate publication. A completed exploratory package will not establish final certification or whole-run absence of interference. [Frozen protocol, artifacts and controls](../evidence/windows-laptop-exploratory-protocol-011.zip)

Archive SHA-256 f8770781e11b87c2ed592ab8e56ae6487a86c56e9ce11c4de738965bb53cf5a0.

## Owner-verification timing checks

The default suite verifies the placement of the production guard clock on real launches. It tests the price calculation on the same observations, changing only the recorded outer time or removing the measured delay from the priced interval.

The live stability and sensitivity gates compare independent conditions in five interleaved sets of nine pairs. Use a sampler-verified quiet host and retain telemetry and all outcomes. From the suite's `tests/tests` directory, run this command under a finite whole-process-tree deadline.

```text
python -m unittest -v test_campaign_logic.OwnerVerificationTimingTests.run_live_sleeping_launcher_gate test_campaign_logic.OwnerVerificationTimingTests.run_live_inside_delay_gate
```

The live stability gate uses the plain condition's median absolute deviation for both the pair and set components. The sensitivity gate requires a price increase greater than 125 ms and a difference beyond that bound. These two live gates are selected explicitly, outside default suite discovery.

**Next:** The [competitors-001 folder](competitors-001/README.md) holds the set itself.

[↑ Back to Contents](#contents)

The single approved attempt stopped after 91 of 1,940 recorded slots: all 36 controls and 55 warmups, with zero measured trials. The frozen counter-order gate rejected a sample whose idle and kernel totals each decreased by 156,250 100-nanosecond ticks between immediate reads; derived busy time was unchanged. The containing Job drained from five active processes to zero. A separate native check found all 174 recorded exact process identities absent. The raw package remains provisional and incomplete, and supports no speed conclusion. The archive preserves all records, including six non-ok Nuewframe warmups, without rescoring or discarding them. [Rejected attempt and exact cleanup evidence](../evidence/windows-laptop-exploratory-attempt-011.zip)

Attempt archive SHA-256 db31b44814b8e9250b8a4e88174a03da481f00212427695469859b4033421171.

Two independent readers reproduced immediate GetSystemTimes component regressions on this host: native C retained 925 pairs with 84 regressions, and Python retained 940 pairs with 45. Both reached their prescribed 15-second deadline before the fixed 1,024-pair first phase finished. Neither reached the affinity phase, and no retry occurred. Derived busy time never decreased in these traces; the largest component decrements were 4,843,750 and 2,500,000 100-nanosecond ticks. These observations rule out a Python-only decoding explanation but establish neither the kernel cause nor a universal error tolerance. The archive retains every source, raw reading, compiler identity, exit and incomplete-control limitation. [Independent counter diagnostic](../evidence/windows-system-counter-diagnostic.zip)

Diagnostic archive SHA-256 7969fcdca2d147984fe19e90dd672d1583fcaccde5884c0a73d3e059381a36e8.
