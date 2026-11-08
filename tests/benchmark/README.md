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

The archive contains the original report, all observations and ProcessKit event logs, the audit and host telemetry, the exact configuration, and both earlier stopped attempts. The capture process exited 1 after finalizing the package because its redirected Windows console could not encode a report character. A later display-only fix replayed the report under that code page without modifying the package. The archive index binds each file by SHA-256. [Download the evidence — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 `ef653c4fb2921adc9e466f767225ba257b26e0b9adecf354b8000f30ccf751bd`.

An offline replay corrected 32 survivor labels to name the scale path, case kind, and variant. All 44 standing losses, the incomplete verdict, and every other report line matched the baseline replay. The original capture and reports remain in the archive above. [Replayed reports and regression receipts — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 `03d40c2964ee48306489117daaab66b0b43a25f2b9879b0888d045086fc8f7ea`.

The archived SysPrims arm recorded 180 measured and 18 warmup fast slots. All 198 exited 0 and matched stdout. Every slot recorded 334 stderr bytes and failed the expected-stderr check. The invocations used the default CLI log settings. The published v0.2.4 source defaults to INFO-level stderr tracing and emits messages on successful runs, consistent with the captured mismatch. The freeze rule requires a successful fast-path median for each rival, so SysPrims left the startup comparison unevaluated in both contexts. Host quietness is a separate admission requirement. [SysPrims log defaults](https://github.com/3leaps/sysprims/blob/v0.2.4/crates/sysprims-cli/src/main.rs#L77-L85) · [SysPrims stderr writer](https://github.com/3leaps/sysprims/blob/v0.2.4/crates/sysprims-cli/src/main.rs#L691-L715)

### SysPrims output qualification

Two independent Sol reviewers approved the same bounded eight-slot Windows diagnostic. The pinned SysPrims v0.2.4 default produced 334 stderr bytes and failed the relay's byte oracle in both console contexts. The explicit top-level --log-level=warn option preserved the exact 18-byte stdout and 13-byte stderr in both contexts. All four kill controls returned 124 with no observed marked descendant and natural Job drainage. Default kill logging retained the INFO reliability diagnostic, which the WARN control filtered.

The result qualifies the tested WARN fast-path output. It supplies no comparative timing or protection-equivalence verdict. A fresh campaign can review WARN for fast-path relay slots while retaining default logging for kill, scaling, and malformed slots. The original campaign keeps its recorded verdict. The archive includes both approvals, the original and revised diagnostic designs, all raw streams and Job receipts, and a replay that reproduced the same result from a fresh extraction. [Correctness evidence and replay — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

### CPU counter sensitivity

All 1,940 stored arm-process CPU readings fell on a 15.625 ms grid. The original comparator reproduced 30 failing workload comparisons and 77 failing ranks. Adding 15.625 ms to each rival CPU reading left nine comparisons and 14 ranks failing. Adding 31.25 ms instead left four comparisons and eight ranks failing. The remaining comparisons were the empty-command and over-long-argument cases against nuewframe and SysPrims.

These post-hoc arithmetic allowances favor the guard. The strict verdict uses the original readings, and the grid supplies no measurement error bound. [Exact analysis, affected cases, and replay script](../evidence/windows-cpu-accounting-audit-20261106.zip) · [Windows CPU counter documentation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)

### CPU counter calibration

Independent replays on Linux ARM64 and native macOS ARM64 reproduced the Windows CPU audit byte-for-byte after line-ending normalization.

The first bounded Windows calibration refused an elapsed pulse that exceeded its preregistered limit. A separate fixed-work calibration then compared two idle controls with 8,192 and 32,768 iterations. Raw process cycles distinguished the work levels in all 12 measured quartets. `GetProcessTimes` passed zero of the same 12. The result is specific to the synthetic workload and observed host. Cycles remain in raw units, and the original campaign verdict retains its original CPU-time readings. [Both calibration receipts and the cross-platform replay — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence) · [Process cycle counter](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-queryprocesscycletime)

Archive SHA-256 `0e6dae0c86b1e37ada4d5598d094d926382c236441f85a591a24b7242d808a17`.

## Windows comparison with repaired Wake, 6 November 2026

The observed-load x64 MSVC run compared consolidated commit 8bf828a with Windows-repaired Wake commit 9c6f0df. The matched builds used the same release profile and toolchain. Two independent reviewers approved the exact manifest. The run completed 2,967 slots, including 2,880 measured trials, 72 warmups, and 15 controls.

| Manual startup | Median | 90th percentile |
| --- | ---: | ---: |
| Consolidated guard | 84.2891 ms | 107.1302 ms |
| Repaired Wake | 84.2844 ms | 99.2223 ms |
| Bare fixture | 54.4989 ms | 64.9063 ms |

The consolidated guard was slower on one of the 16 prespecified contrasts. The manual-startup 90th percentile was **7.9079 ms slower**, with a primary interval of **[2.6032, 17.0082] ms**. The estimate was positive in both chronological halves and all six launch orders. The other 15 contrasts were unresolved. The nominal intervals assume stationary weak dependence across chronological blocks. The separate quiet-host campaign and the speed acceptance requirement remain open.

The archive contains all raw slots and terminal receipts, the failed zero-row attempt, the matched-build inputs and Windows-repaired Wake source, and the exact analysis. A fresh extraction reproduced all 16 contrasts and the original analysis file byte for byte. [Download the evidence and Windows replay package — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 56d7e8282ce249dc5bfc3ab984ea03e97d99b463bb85e96db1684a7992f47038.

## Windows startup phase diagnostic, 6 November 2026

All 324 trials completed under the separately reviewed phase protocol. The original fixture produced a +2.6335 ms manual-startup p90 contrast, below the preregistered +3.9079 to +11.9079 ms anchor range. Its second-half contrast was −2.6100 ms. Five per-arm p90 comparisons between fixture conditions also exceeded the 2 ms threshold. Seven of 19 calibration checks failed, leaving phase localization inconclusive.

The earlier +7.9079 ms contrast remains the result of the larger directional comparison. The archive preserves the preregistration, all phase trials, the initial zero-row admission refusal caused by a missing protocol hash, and the replay. A fresh extraction reproduced the exact analysis bytes using matching local Python and PowerShell executables. [Phase diagnostic evidence and replay — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 f13a78229c9b66c963dca3748c32fdbd813a6dfcbd95eb0e1c5bf37973c802a0.

## Windows Job event feasibility

A private completion port delivered fixture PIDs from an otherwise empty Job. The observer opened each process, verified its image and Job membership while live, and read the terminal timestamps from the same held handle. Both the synthetic testkit lane and the nested launcher lane passed all 28 cases. Each lane included 24 immediate exits and four 50 ms waits. Every Job drained, and a synthetic packet naming an outsider was refused.

Job completion messages can be lost. Any future phase experiment must account for missing observations and calibrate the observer against the uninstrumented workload. This proof covers fixture observation only. [Raw feasibility evidence — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence) · [Microsoft Job notification documentation](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_associate_completion_port)

## Windows standalone manual-startup replication

The standalone run retained the original manual-startup order within 40 blocks, all 72 warmups, and all 15 controls. It completed 807 slots, including 240 measured triplets. The binaries, original fixture, native timing bracket, and guard settings matched the earlier comparison. The measured sequence omitted the intervening AUTO and wait workloads.

| Manual startup | Median | 90th percentile |
| --- | ---: | ---: |
| Consolidated guard | 37.9071 ms | 52.1849 ms |
| Repaired Wake | 37.0708 ms | 49.3178 ms |
| Bare fixture | 23.7398 ms | 30.1599 ms |

The p90 contrast was +2.8671 ms with a primary interval of [−5.5511, +8.1285] ms. Both halves were positive and one launch order was negative. The standalone contrast was unresolved and failed the preregistered calibration gate for phase work. The previous mixed-workload result and the quiet-host issue retain their separate scopes. A fresh archive extraction reproduced the exact analysis bytes. [Raw replication and offline replay — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

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

The archive contains the protocol, source and executable identities, candidate patch, prior regression evidence, all raw slots and cleanup receipts, the approval and bounded invocation, and the replay. Archive SHA-256 is e7bdc5ff85068e8ed03176628db5a4ad60014cbc9973577b1891f381404c1293. [Diagnostic evidence and replay — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)


Independent offline replays of all 300 Windows launch records ran on Linux ARM64 and macOS ARM64. Each reproduced the Windows analysis byte for byte after line-ending normalization. Independent post-run checks found no replay scratch directories, macOS replay process group, or Linux replay scope. [Native replay and cleanup evidence — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

The earlier directional comparison built both images with rust-lld.exe. The resolver baseline and candidate used Microsoft link.exe 14.51.36260.0, while repaired Wake retained its exact directional image. The baseline and candidate therefore share a build recipe, and their comparisons against Wake include a linker difference. The archived image audit also records a larger consolidated text section and one additional direct DLL import. No phase timing was measured by the audit. [Build records and PE replay — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

## Quiet-host telemetry qualification

A one-minute Windows x64 observation recorded 12 five-second windows with median CPU at 2.25%, maximum CPU at 7.85%, and more than 6.7 GiB of available physical memory throughout. Those windows met the draft preflight thresholds. This establishes the observed minute's conditions. Each full campaign needs its own admission and postflight.

A native nested-Job calibration matched the enclosing Job's CPU deltas to the sum of two exact process-handle readings over three windows. Both processes were active in the first two windows and had exited by the third. The Job drained naturally. A separate settled fixture and five offline controls also passed. The archive retains the initial clock-binding failure. These checks launched zero benchmark slots. The estimate bounds the bracketed counter readings, with quantization and accounting lag limiting its interpretation as physical CPU activity.

The archive binds the draft source and binary inputs. A full run still requires a reviewed launcher with continuous phase coverage, process and power checks, and approval on the same frozen manifest. [Sampler, staged input identities and raw receipts — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 de38e7ba32962d6850060c57d54865e1fcd33019d112b02631370140ccd66a11.

### Sealed launcher qualification

The candidate launcher binds the source and binary inventory, effective environment digest, run ID, configuration, seeded order, raw host samples, process inventory, and complete campaign package. Its offline audit accepted a current-format replay of all 1,940 historical observations, retained the 44 standing losses, bound 473 files, and passed seven refusal controls. It requires the ProcessKit event files named by each slot and explicitly records the parser refusals that produce no event. It does not equate a completed package with a passing campaign.

Six additional native windows compared enclosing Job CPU deltas with the exact retained process handles when nested fixture children exited before and after a sampling boundary. All differences were zero. The final median and coverage use only scheduled complete five-second windows. Terminal intervals still undergo contamination checks. Twelve synthetic tests passed, including an idle-tail case that had changed a failing median into a passing one in the earlier draft.

Both independent Sol reviewers approved exact revision 007 with no unresolved implementation objections, conditional on a successful natural lifecycle control before the full campaign. Three revision-007 lifecycle attempts refused in preflight after observing external Git timeout or Python runtimes. They launched zero benchmark slots. Earlier drafts and their failed checks remain preserved.

The strict census classifies generic external runtime names as competing work. Both reviewers identified that this is broader than evidence of an actual competing test. A refined census would need its own frozen definition and review. The strict quiet-host issue and speed acceptance remain open. [Exact sources, manifest, controls, refusals and replay — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 22297e02b935643b11d4a58fdd01895021f4b3e41edd77541c35fcc30e9c9de6.

### Exploratory laptop protocol 011

One prospective 1,940-slot exploratory capture is preregistered with manifest SHA-256 fa64410ae847be10735cdf5a69ce71acb7213a2d9bbe3b7695c83a8f15df454a. Both independent Sol reviewers approved the same source and bounded command. This protocol retains the numerical CPU, memory, power, clock, observer, timing and cleanup gates. Its scoped image census vetoes identified compilers and pinned benchmark content outside the owned Job. Other inspected runtimes are recorded as unclassified background and their CPU remains in the residual. The archive states the sampled-name and on-disk identity limitations. This scope is exploratory, and the strict quiet-host issue remains open.

The native lifecycle control completed preflight, a 62.4-second sleeper, natural Job drainage, postflight and raw telemetry replay. Its final inventory verifier exposed a reused local creation-time variable. The corrected verifier accepts the immutable native receipts and rejects forged identities. All 26 synthetic and saved-receipt checks passed. The collector, sampler, gates and raw replay were unchanged by that correction.

The preregistration archive preserves the reviewed source, exact current binaries, all staged input hashes, native control receipts, earlier refusals, the metadata and timestamp compatibility repairs, and both approvals. The full capture had executed zero benchmark slots when this archive was produced. Its complete package, actual gate values, losses and exclusions require separate publication. A completed exploratory package will not establish final certification or whole-run absence of interference. [Frozen protocol, artifacts and controls — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 f8770781e11b87c2ed592ab8e56ae6487a86c56e9ce11c4de738965bb53cf5a0.

## Owner-verification timing checks

The default suite verifies the placement of the production guard clock on real launches. It tests the price calculation on the same observations, changing only the recorded outer time or removing the measured delay from the priced interval.

The live stability and sensitivity gates compare independent conditions in five interleaved sets of nine pairs. Use a sampler-verified quiet host and retain telemetry and all outcomes. From the suite's `tests/tests` directory, run this command under a finite whole-process-tree deadline.

```text
python -m unittest -v test_campaign_logic.OwnerVerificationTimingTests.run_live_sleeping_launcher_gate test_campaign_logic.OwnerVerificationTimingTests.run_live_inside_delay_gate
```

The live stability gate uses the plain condition's median absolute deviation for both the pair and set components. The sensitivity gate requires a price increase greater than 125 ms and a difference beyond that bound. These two live gates are selected explicitly, outside default suite discovery.


The single approved attempt stopped after 91 of 1,940 recorded slots: all 36 controls and 55 warmups, with zero measured trials. The frozen counter-order gate rejected a sample whose idle and kernel totals each decreased by 156,250 100-nanosecond ticks between immediate reads; derived busy time was unchanged. The containing Job drained from five active processes to zero. A separate native check found all 174 recorded exact process identities absent. The raw package remains provisional and incomplete, and supports no speed conclusion. The archive preserves all records, including six non-ok Nuewframe warmups, without rescoring or discarding them. [Rejected attempt and exact cleanup evidence — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Attempt archive SHA-256 db31b44814b8e9250b8a4e88174a03da481f00212427695469859b4033421171.

Two independent readers reproduced immediate GetSystemTimes component regressions on this host: native C retained 925 pairs with 84 regressions, and Python retained 940 pairs with 45. Both reached their prescribed 15-second deadline before the fixed 1,024-pair first phase finished. Neither reached the affinity phase, and no retry occurred. Derived busy time never decreased in these traces; the largest component decrements were 4,843,750 and 2,500,000 100-nanosecond ticks. These observations rule out a Python-only decoding explanation but establish neither the kernel cause nor a universal error tolerance. The archive retains every source, raw reading, compiler identity, exit and incomplete-control limitation. [Independent counter diagnostic — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Diagnostic archive SHA-256 7969fcdca2d147984fe19e90dd672d1583fcaccde5884c0a73d3e059381a36e8.

### Exploratory laptop protocol 012

The replacement prospective capture is frozen under manifest SHA-256 b73713eb726622e167d14b2381c3756a118e37ed7fc0d01fe28b169c1426fd12. The observer takes one system CPU reading between two cumulative Job readings and subtracts only the inner Job difference. Its denominator is the smaller inner capacity from the native awake clock and QPC. It refuses every negative residual, malformed native-width value and inconsistent call bracket. The CPU thresholds remain 10%, 15% and 25% for their original conditions. This estimates observed accounting with conservative call-positioning bounds; it establishes no bound on unknown system-counter error.

All 31 synthetic checks passed. Six native nested-Job windows matched the held-process CPU sums exactly at the recorded reads. The full lifecycle passed preflight, a 62.4-second sleeper, natural drainage, postflight, independent replay and all 40 inventory samples. All nine source hashes remained unchanged. Both Sol reviewers independently recomputed the native arithmetic and approved exactly one full exploratory 1,940-slot attempt after public preregistration, with zero unresolved objections in this scope.

No full 012 attempt had started when this archive was produced. Attempt 011 remains rejected, and the strict quiet-host issue remains open. The package includes exact inputs, binaries, source, the original benchmark specification, tests and raw qualification receipts. Its chronology distinguishes the frozen method’s earlier draft statement from the later manifest and native qualification. [Prospective protocol and qualification — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 45520e84d07098dc814a449772b2583c76051c95b271aaa2ff2627412940e582.

The single approved 012 attempt stopped after 76 of 1,940 records: all 36 controls and 40 warmups, with zero measured trials. The sampled AC line changed from 1 to 0 and the frozen power-state gate rejected the run. Independent replay reproduced all 26 scored windows with no native counter, ordering or clock-bound errors. The last two residual estimates, 18.3020% and 19.6067%, also exceeded the adjacent-window limit; restoring AC alone would not establish a valid run. The containing Job drained from five active processes to zero, and a separate native check found all 113 recorded exact identities absent. The archive preserves the provisional metadata, unfinished slot, four non-ok Nuewframe warmups and all other records. No recorded guard trial was non-ok, and no speed result is claimed. [Rejected attempt and exact cleanup receipts — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Attempt archive SHA-256 766f52661cd8362d32e15bde3d890c17cdcf56418c99d7718fed6de384c683d7.

### Prospective laptop attempt 013

Attempt013 is frozen under manifest SHA-256 313bf9796153453177636419137275ebd0f5374d71c359138c8001455a6e81ae. Both independent Sol reviewers approved exactly one full exploratory 1,940-slot attempt after public preregistration, with zero unresolved objections. All seven measurement modules are byte-identical to native-qualified012. The driver changes only the run ID and its derived destination; the method records the current chronology and binds the original qualification receipts.

The package links the original 40-sample lifecycle, six held-process edge windows and 31 focused checks by source and raw-file hashes. It preserves the earlier metadata and the resolved chronology objection. The rejected full011 and012 captures remain rejected. Charger confirmation does not replace prospective CPU, power, memory, clock, census and cleanup admission. No full013 attempt had started when this archive was produced, and strict quiet-host completion remains open. [Frozen013 inputs, qualification links and specification — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 77ef0d6eca6835f087b77114c871373684134b6b4464b35a0f3e37e52427722a.

### Rejected attempt 013 and observer diagnosis

The single approved attempt refused its first preflight sample and launched zero benchmark slots. All 33 inspected process rows were permitted, but the complete sample-to-inventory interval took 1,107,467,900 ns against the unchanged 50,000,000 ns budget. The counter read itself took 892,900 ns. The saved generic error combines census-state and timing predicates; independent inspection identifies a timing-budget failure here, not a detected competing process. Available memory was 4,585,127,936 bytes, with AC connected and battery saver off. The archive preserves the exact source, preparations, raw refusal and terminal reason. A separate native check found the recorded exact enclosing-guard identity absent. No campaign worker or measured result existed, and the observer had no saved creation identity for an exact cleanup claim. [Rejected preflight and exact guard check — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Attempt archive SHA-256 3686ef75656a67fbe7dc27dc4e873d32326440b33112c5a8d471aad48c0206ac.

Three fixed no-workload snapshots instrumented the frozen013 observer pipeline. Their complete brackets were 785.412, 487.709 and 802.415 ms. Repeated image-path metadata and open operations accounted for the largest measured exclusive costs; content hashing took 17.409–36.953 ms. The first two snapshots inspected 22 external candidates covering four distinct file identities. CPU-clock queries and bookkeeping are inside the measured spans, so the recorded 49.257–69.829 ms snapshot-API spans do not establish uninstrumented API latency. The third snapshot's outside pinned guard and transient image-query error remain in the raw evidence. They were absent from the original013 refusal. The failed first profiler source and exit are also retained. No guard source, gate or prior capture was changed, and these diagnostics support no speed or parity conclusion. [Diagnostic sources and raw stage evidence — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Diagnostic archive SHA-256 f092428723c692770f1977c6e95a9f4b6c1164842a84b7f9bd7f3c46522be413.

One fixed three-snapshot calibration then measured the same frozen013 pipeline with QPC boundary reads and no added per-API CPU-clock queries. Complete brackets were 244.1854, 397.7710 and 334.8737 ms, all over the unchanged 50 ms budget. The native snapshot calls took 21.3224, 25.6743 and 22.7875 ms. Toolhelp alone did not exceed 50 ms in this control, but the remaining pipeline still requires repair. Candidate populations were 37, 38 and 36; the middle snapshot retained one unverified process-open failure. Exit zero records diagnostic completion, not benchmark admission. All nine frozen source hashes were unchanged, and zero slots ran.

The package retains the raw streams and exact corrected source, plus the hash-matching unexecuted pre-review source reconstructed by reverting its two deadline edits. Only the corrected source executed, under 45-second outer and 35-second internal bounds. Different timestamps, populations and instrumentation prevent a paired speed claim against earlier diagnostics or the original refusal. [Calibrated observer evidence — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Calibration archive SHA-256 e0a812fae07dfc176231c5e4bda54ed152441e41b3eeea6a2f27b226868703cb.

The proposed direct Windows file reader passed ten unit tests and the independent SDK layout check, then failed its single native mutation control. All four pinned images matched the manifest and original013 digests. A same-length edit after the content read was accepted, so qualification exited one; replacement, growth and truncation never ran. Handles returned to baseline167 and the owned fixture was absent. The candidate remains unadopted, with no full-pipeline or speed result.

The record lacks the accepted edit's returned metadata and cannot establish the cause or whether013 shares the limitation. Both readers make final metadata observations before content-handle closure; a preserved correction withdraws an incorrect draft sequencing claim. The archive includes that correction, raw failure, exact code, passed controls, the compiler metadata probe's diagnostic and the unexecuted unsafe-cleanup preparation. No production guard or gate changed. [Failed native qualification — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Qualification archive SHA-256 989ad9d5d57887db6af8d78ea909d71baf884b7e931dbf5d19c0289a0e8afa8f.

A separate fixed two-file diagnostic retained every metadata query and independent post-close bytes. The013 case refused its edit;014 accepted the pre-edit digest despite changed final bytes and unchanged recorded native and Python metadata. Distinct files, observation times and instrumentation prevent a universal API or timestamp-cause inference. Both helpers stayed unchanged. [Mutation observations — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Observation archive SHA-256 b9cf0c747815c29f409ced6f7772c8328a5e11d10faf615e4f928ad99e9f9993.

A separate fixed-two-read candidate passed thirteen unit tests, matched four pinned images, and correctly refused the persistent in-place edit between reads with both differing digests recorded. Its next control's `os.replace` failed with WinError5 before demonstrating a replacement. Qualification exited one, and growth/truncation were not reached. This OS refusal does not qualify replacement detection. The unexpected replacement error's partial helper evidence was not emitted by the outer logger; that limitation is preserved. Handles returned to baseline167 and the owned fixture was absent. No full-pipeline timing, 50 ms compliance, atomic immutability or loaded-image identity claim follows. [Two-read candidate and incomplete native qualification — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Qualification archive SHA-256 e02164c5d081970bf833ecf0fe60ff07a63b6f8ac2057e35d048626a2669e0ef.

Independent follow-up controls qualified growth/truncation against their exact second-read errors and expected bytes. An actual replacement between the first named handle's closure and content open produced the expected full-identity mismatch and was rejected before any hash call. A separate OS diagnostic observed successful closed-target replacement and WinError5 while the shared-read handle was held; this does not establish detection in that blocked window. Every case restored its handle baseline and removed its owned fixture. Earlier failures remain unchanged, and no full-pipeline timing or gate compliance is claimed. [Follow-up native controls — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Controls archive SHA-256 72d2a97b6fdd6e863f63df93ca174156ed47fb7f94cf22930c6ff5b8c5591a33.

The complete two-read observer pipeline then failed all three unchanged 50 ms checks: 252.5741, 351.2949 and 341.4947 ms. The middle snapshot also retained a transient process-open failure and invalid creation evidence. Diagnostic exit zero did not mean admission: `observer_pipeline_qualified` was false and zero benchmark slots ran. The intrinsic file envelopes totaled 208.903–287.333 ms, while both content passes used 17.463–24.110 ms; the specific open/metadata/Python cost remains unisolated. Diagnostic observer CPU deltas of 234.375 and 328.125 ms include post-cutoff validation and emission and would also exceed the observer ceiling if treated as phase windows. No campaign phase was scored.

All raw data and exact bindings are retained, including the failed first unit import and subsequent seven passing strict-evidence checks. No guard source, gate or prior capture changed. [Failed pipeline qualification — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Pipeline archive SHA-256 045952810870af384f49a7d9872135a70e63ec358a33b59da54cde860609baaa.

A subsequent four-image diagnostic measured the existing file APIs with QPC-based nanosecond call brackets. The observed Python312 inspection took 11.524 ms, with open/close envelopes totaling 8.7641 ms. All four images matched their pinned digests on both reads and closed their owned handles. These call envelopes include Python/ctypes boundary costs and establish neither an OS/filter cause nor full-census performance. No collector or gate changed. [Exact API-cost source and observations — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Diagnostic archive SHA-256 73420df6a90917ef7d63e026f8a8bb1662bcfa0af14a64a89d4e64717427a40d.

An independent C control subsequently matched all four pins on both complete reads and all four native signatures. The Python312 target's open/close envelopes also remained relatively costly in that process, totaling 5.5454 ms. This does not support a solely ctypes explanation, establish the OS/filter cause or prove parity with the separately timed Python control. Raw QPC ticks/frequency, native statuses, exact build/source pins and all preparation corrections are preserved. No collector or numerical gate changed. [Independent C diagnostic — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Diagnostic archive SHA-256 df75877895e00a5271d3ea1e7bcc70289054b95b2cc80a090365fdf44ffd9f9e.

The separate by-name metadata investigation was not adopted. After preserving the refused SDK-path preparation and correcting a malformed parser, syntax, eight path controls and actual SDK-header checks passed. The native probe attempted only Python: identity, volume, core fields and both hashes agreed, but access time changed across observations and the strict mapping control exited one. The other three images were not attempted. The single observed by-name and legacy named-query envelopes did not establish a clear census-speed benefit. All handles closed; deletion/type/reparse equivalence remains unqualified. No collector or gate changed. [Preserved preparations and partial diagnostic — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Diagnostic archive SHA-256 52c85969003200e5f8c19b44db9e8428c8b8a5a821333afc1a40c2fc7a4769a1.

The prospective grouped census retains all process rows and checks while sharing one file observation for each exact raw image-path string. This is a changed sampling schedule with fewer independent content observations, not proven equivalence to the former per-row schedule. Twenty-seven mocked controls passed; two independent Sol reviewers approved its source. The diagnostic driver replays emitted census and telemetry bytes and requires both original and stricter output-inclusive 50 ms spans. Native controls and the final receipt-sealed plan are still pending, so no native grouped performance or admission result exists. Unexecuted drafts and passed mock attempts are preserved. [Source checkpoint and declared limits — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Preparation archive SHA-256 24ac9a06a804e7d229fb5eae7a0cdd59a3208d3d133ab3ffad9fdfdc757896d2.

The grouped method then passed six genuine native file/member component controls, with handles 170 to 170 and all fixtures removed. Its one approved three-snapshot observer diagnostic still failed: complete output-inclusive spans were 88.4836/82.8714/62.2483 ms, all above 50 ms. Original sample-to-return spans also failed. All emitted census and telemetry replays were valid, but the two observed windows failed memory, observer CPU or background CPU criteria and lacked full phase coverage. Diagnostic exit zero is not admission; qualification remained false and zero slots ran. Independent raw arithmetic and original observer/guard absence checks passed. No comparative speed conclusion or old-schedule equivalence follows. [Native controls, failed pipeline and measured cost decomposition — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 34fa7babbdba6d5124fd88ea434cc9b4845e64700b9924f92579c794138b24d2.

The subsequent native bulk-enumeration component compiled and passed 27 Python/C controls. One capture validated all 524 returned rows and held controller identities, with handles 165 to 165 and successful module cleanup. Its complete snapshot took 36.9241 ms, including 17.1389 ms after the native call. A separate saved-input byte-decoder experiment preserved identical full output across eight calls, all sixteen adversarial outcomes and eleven tests; descriptive old/new wall times were 10.3–14.8 versus 1.0–3.0 ms. This phase used a Python memory shim, not actual enumeration. Neither experiment establishes full-census admission or guard-versus-competitor speed; zero qualified measured campaign comparisons exist. Exact sources, raw output, drafts and an independent offline audit are public. No guard guarantee/default or numeric gate changed. [Native component and saved-data diagnostic — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 c4dcc4578071029c9da39a02fae4091eb3929b5296e0a30a2aad96f99cf289b6.

The byte decoder then passed one actual native capture of 503 unique rows, with valid held controller anchors and handles restored. Its complete snapshot took 24.3351 ms, including 5.7321 ms after the native call. This capture and the earlier 524-row observation differ in time/population and do not form a paired speed comparison. All reference/tool pins and raw row/cleanup facts passed independent audit. Full census and phase/campaign admission remain unqualified, and zero benchmark slots ran. [Actual native decoder component — raw archive withdrawn](../evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 5bf81a8a64698206faa48b60e7d55c7b21721ad0c25c91baa2f7f06e99a46b12.

**Next:** The [competitors-001 folder](competitors-001/README.md) holds the set itself.

[↑ Back to Contents](#contents)


## Prospective observed-accounting campaign017

This preregisters one attempt of the existing 1,940-slot Windows campaign: 36 controls, 224 warmups and 1,680 measured trials, with seed 20261029. The seven measurement modules, 20 qualified inputs, guard image `8fa4e58b`, SlotJob `94f31bff`, rivals, workloads, console contexts and numerical gates remain unchanged from campaign016. Only the run identity and destination change. The attempt retains the 6,900-second campaign deadline, 6,920-second outer envelope and 6,600-second observation limit. It may start only after this preregistration is pushed, with other test workers stopped throughout capture.

Exact manifest SHA-256: `01ce85eb03ecdd6790378da3aa9f6ee413900acc912459bcc677fa76ca9737f2`. Source snapshot: `f86a918ac26e984954a4d74c74be2795ae74ae6e006e2f6564dc5916bd47b5ae`. Both independent Sol reviewers approved the same manifest with zero unresolved objections: reviewer2 receipt `97bd1b430b8c5db0abceec7ccea48c97482fa99f2daa2acdc6c98633c171a663`; critic receipt `f86158f67caa5722aad88ceb74144a19070553e756b503345ab3a1225d5eb112`.

Earlier campaign015 and campaign016 failures remain preserved; campaign016 recorded no measured trials. This authorizes one fresh attempt, with no automatic retry, overwritten capture, waived gate or rescoring of earlier data. It is an exploratory observed-accounting run. This preregistration establishes no performance result, equivalence, leadership or strict issue #7 completion. Raw machine and transport records remain local under the publication privacy policy; hashes do not substitute for independently accessible evidence.


**Recorded outcome:** The single attempt refused admission before the campaign worker started. Its two initial inventory brackets took 44.4429 ms and 93.4519 ms; the second exceeded the unchanged 50 ms limit. Both inventories contained only permitted classifications, so the refusal does not demonstrate a competing workload. There were zero completed resource windows and zero controls, warmups or measured trials. Windows also reported battery power and 3.6173–3.6418 GiB available RAM; those observations were not separately scored before the inventory veto.

The incomplete capture and all earlier failures remain preserved. Terminal receipt SHA-256: `fd6011b92206bb95a3b5637dc8d869e83916ca1e20eadcabef07ffa88f795338`; telemetry: `26710049c379cce83b853501d8f8f730bae02542e05faac04f30ff555f25c4e9`; inventory: `493cfa5a65dda90ced71ed1d315c13187d4e33ff9aad4eba48b75b753a5a724b`. Both independent reviewers verified this scoped refusal. This attempt establishes no guard failure, competitive comparison, speed ranking, equivalence or issue #7 completion. It is consumed and will not be retried automatically.
