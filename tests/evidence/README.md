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

An ordinary-user Linux ARM64 test exercised the authority candidate’s real openat, statx and fstatfs calls. It rejected mismatched inodes, symlinks and incorrect descriptor types, and refused provisioning before creation. The first native attempt exposed incorrect ARM64 open flags. Corrected source passed, and a C program using the guest headers independently matched the selected constants. The canonical installation-path correction passed the same test. The archive preserves the failed attempt, corrected source, successful receipts and independent cleanup checks. That read-only qualification did not exercise positive root-owned provisioning, installation provenance, generic placement or watchdog integration. [Authority source and raw evidence](linux-cgroup-authority.zip)

Archive SHA-256 `87901397e99a762094f561470a468adad142da62994a7c91a134cd99f61dd031`.

## Rejected exploratory laptop attempt 011

The single approved attempt stopped after 91 of 1,940 recorded slots: all 36 controls and 55 warmups, with zero measured trials. The frozen counter-order gate rejected a sample whose idle and kernel totals each decreased by 156,250 100-nanosecond ticks between immediate reads; derived busy time was unchanged. The containing Job drained from five active processes to zero. A separate native check found all 174 recorded exact process identities absent. The raw package remains provisional and incomplete, and supports no speed conclusion. The archive preserves all records, including six non-ok Nuewframe warmups, without rescoring or discarding them. [Rejected attempt and exact cleanup evidence](windows-laptop-exploratory-attempt-011.zip)

Attempt archive SHA-256 db31b44814b8e9250b8a4e88174a03da481f00212427695469859b4033421171.

## Independent Windows counter diagnostic

Two independent readers reproduced immediate GetSystemTimes component regressions on this host: native C retained 925 pairs with 84 regressions, and Python retained 940 pairs with 45. Both reached their prescribed 15-second deadline before the fixed 1,024-pair first phase finished. Neither reached the affinity phase, and no retry occurred. Derived busy time never decreased in these traces; the largest component decrements were 4,843,750 and 2,500,000 100-nanosecond ticks. These observations rule out a Python-only decoding explanation but establish neither the kernel cause nor a universal error tolerance. The archive retains every source, raw reading, compiler identity, exit and incomplete-control limitation. [Independent counter diagnostic](windows-system-counter-diagnostic.zip)

Diagnostic archive SHA-256 7969fcdca2d147984fe19e90dd672d1583fcaccde5884c0a73d3e059381a36e8.

## Exploratory laptop protocol 012

The replacement prospective capture is frozen under manifest SHA-256 b73713eb726622e167d14b2381c3756a118e37ed7fc0d01fe28b169c1426fd12. The observer takes one system CPU reading between two cumulative Job readings and subtracts only the inner Job difference. Its denominator is the smaller inner capacity from the native awake clock and QPC. It refuses every negative residual, malformed native-width value and inconsistent call bracket. The CPU thresholds remain 10%, 15% and 25% for their original conditions. This estimates observed accounting with conservative call-positioning bounds; it establishes no bound on unknown system-counter error.

All 31 synthetic checks passed. Six native nested-Job windows matched the held-process CPU sums exactly at the recorded reads. The full lifecycle passed preflight, a 62.4-second sleeper, natural drainage, postflight, independent replay and all 40 inventory samples. All nine source hashes remained unchanged. Both Sol reviewers independently recomputed the native arithmetic and approved exactly one full exploratory 1,940-slot attempt after public preregistration, with zero unresolved objections in this scope.

No full 012 attempt had started when this archive was produced. Attempt 011 remains rejected, and the strict quiet-host issue remains open. The package includes exact inputs, binaries, source, the original benchmark specification, tests and raw qualification receipts. Its chronology distinguishes the frozen method’s earlier draft statement from the later manifest and native qualification. [Prospective protocol and qualification](windows-laptop-exploratory-protocol-012.zip)

Archive SHA-256 45520e84d07098dc814a449772b2583c76051c95b271aaa2ff2627412940e582.

## Rejected exploratory laptop attempt 012

The single approved 012 attempt stopped after 76 of 1,940 records: all 36 controls and 40 warmups, with zero measured trials. The sampled AC line changed from 1 to 0 and the frozen power-state gate rejected the run. Independent replay reproduced all 26 scored windows with no native counter, ordering or clock-bound errors. The last two residual estimates, 18.3020% and 19.6067%, also exceeded the adjacent-window limit; restoring AC alone would not establish a valid run. The containing Job drained from five active processes to zero, and a separate native check found all 113 recorded exact identities absent. The archive preserves the provisional metadata, unfinished slot, four non-ok Nuewframe warmups and all other records. No recorded guard trial was non-ok, and no speed result is claimed. [Rejected attempt and exact cleanup receipts](windows-laptop-exploratory-attempt-012.zip)

Attempt archive SHA-256 766f52661cd8362d32e15bde3d890c17cdcf56418c99d7718fed6de384c683d7.

## Native Linux empty-domain settlement

On the Linux ARM64 VM, a hash-verified root-owned binary exercised the actual factory and bounded settlement API. The sequential fixture exclusively created its fixed test root, provisioned one empty leaf, wrote through the retained kill descriptor, verified emptiness, revalidated the original identities and removed the exact leaf and root. Both finite system services reported success. Independent checks found the fixed root, private root scratch, service cgroups and selected workers absent. The ordinary handoff was archived before removal under its original UID. The fixture executed no workload and recorded zero poll wakeups, so populated polling, child reaping, watchdog behavior and full one-shot activation remain unqualified. [Source, native receipts and cleanup](linux-domain-settlement.zip)

Archive SHA-256 609e1812f40f0387680f2e8e7686ddb8f1262469e47581cf752dffb0d4a69b35.

## Prospective laptop attempt 013

Attempt013 is frozen under manifest SHA-256 313bf9796153453177636419137275ebd0f5374d71c359138c8001455a6e81ae. Both independent Sol reviewers approved exactly one full exploratory 1,940-slot attempt after public preregistration, with zero unresolved objections. All seven measurement modules are byte-identical to native-qualified012. The driver changes only the run ID and its derived destination; the method records the current chronology and binds the original qualification receipts.

The package links the original 40-sample lifecycle, six held-process edge windows and 31 focused checks by source and raw-file hashes. It preserves the earlier metadata and the resolved chronology objection. The rejected full011 and012 captures remain rejected. Charger confirmation does not replace prospective CPU, power, memory, clock, census and cleanup admission. No full013 attempt had started when this archive was produced, and strict quiet-host completion remains open. [Frozen013 inputs, qualification links and specification](windows-laptop-exploratory-protocol-013.zip)

Archive SHA-256 77ef0d6eca6835f087b77114c871373684134b6b4464b35a0f3e37e52427722a.

## Deferred macOS administrator validation

The corrected fixed-M8 credential/quota control compiled on the physical macOS ARM64 host with an explicit SDK. The earlier header failure and incorrect zero-group-readback source are preserved; that source never ran as root. The reviewed corrected invocation stopped at sudo authentication before either the privileged controller or native control started. Administrator-only tests are deferred at the user’s request and tracked in [issue10](https://github.com/WikdSolvemProbler/process-tree-guard/issues/10). The archive preserves sources, binaries, exact plans and refusal logs. Ordinary-user checks and identity-verified cleanup completed after preservation. There is no privileged native result, fork-fence proof or custody qualification from this preparation. [Prepared inputs and refusal evidence](macos-admin-validation-preparation.zip)

Archive SHA-256 487ec5a2899c4312ea2a15c0cc35ade3bcc6ece106ddd5cee310bf80690459bd.

**Next:** The [`reduced-run` folder](reduced-run/README.md) holds the preserved package.

[↑ Back to Contents](#contents)
