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

## Linux termination-first settlement repair

The next prototype requests termination through its original held kill descriptor before clock reads, cleanup-timer setup or pathname revalidation. The shared driver owns the actual controls and returns that custody plus the write outcome on every returned error. Failed or short writes do not enter wait setup. Removal still requires the bounded timer, validated original identities, observed emptiness, exact unlink and absence confirmation. The timestamp explicitly records an observation after the kill write returns.

One ordinary-user Linux ARM64 attempt compiled the full module graph with warnings denied and passed all four selected controls. Three check the driver's order and outcomes; the fourth performed a real held write to its own ordinary file, then required the real expired-deadline path to refuse while retaining the file descriptor. It did not open or write a cgroup, execute a payload or run the ignored root fixture. Independent checks found the original scope, cgroup path and recorded worker PIDs absent. All 26 remote artifacts were downloaded and hash-verified before exact identity-checked removal of the private scratch.

This qualification does not establish populated drainage, native cgroup error recovery, reaping or watchdog behavior. A native write can block; independent watchdog custody remains required before production activation. Typed arming failures and consuming terminal settlement are still open, and the Linux backend remains disabled. The prior empty-domain proof and all earlier source are preserved. [Repaired source, native controls and cleanup](linux-domain-termination-first.zip)

Archive SHA-256 eda65f3210dbdc12a72b9e0f9ec4ca76ab10f94aaf348b674e446d9075e5df12.

## Typed Linux arming-failure custody

The next adapter returns actual factory failures or verified controls and direct watchdog ownership instead of erasing arming failures into strings. Activation errors retain the original prepared run and its owner handle and absolute timer. Direct child records carry both PID and pidfd. One ordinary Linux ARM64 attempt compiled the full graph into ordinary and test objects with warnings denied. Neither object was linked or executed; no activation, test harness, root operation or payload ran.

All 24 remote artifacts were downloaded and verified before exact ordinary-user cleanup. Independent checks found the original scope, cgroup path and recorded compiler/controller PIDs absent. This is compiler evidence for the ownership API, not proof of native arming-error behavior, mandatory settlement on drop or a complete backend. Consuming terminal settlement and native watchdog/reaping integration remain open. [Source, relocatable objects and cleanup](linux-arming-custody.zip)

Archive SHA-256 c19869448c84258d15f47ebef214e38c6b57d1d1817d23e89f55eaad3d65a155.

## Linux owning drain/removal split

The next component retains the actual controls, same armed cleanup deadline and receipts after successful drainage. Final removal is separate and consuming, preserves partial removal state on failure and creates no replacement timer. A private unsafe caller contract requires completed native reaping and prevention of further placement; no reaper or runtime controller is implied. The old combined shortcut exists only in the empty test fixture.

One ordinary Linux ARM64 attempt compiled the complete graph into ordinary and test objects with warnings denied. Neither object was linked or executed. All 24 remote files were preserved and verified before exact ordinary-user cleanup; independent checks found the original scope, cgroup path and recorded compiler/controller PIDs absent. This establishes compiler acceptance of the owning phases, not populated drainage, post-arm runtime recovery, child reaping or a production backend. [Sources, objects and cleanup](linux-domain-drain-ownership.zip)

Archive SHA-256 9f7532dc455e73b8b82d0f7e73fcbbea0c5b4788613d9286f4a39e48ac7051f1.

## Prospective laptop attempt 013

Attempt013 is frozen under manifest SHA-256 313bf9796153453177636419137275ebd0f5374d71c359138c8001455a6e81ae. Both independent Sol reviewers approved exactly one full exploratory 1,940-slot attempt after public preregistration, with zero unresolved objections. All seven measurement modules are byte-identical to native-qualified012. The driver changes only the run ID and its derived destination; the method records the current chronology and binds the original qualification receipts.

The package links the original 40-sample lifecycle, six held-process edge windows and 31 focused checks by source and raw-file hashes. It preserves the earlier metadata and the resolved chronology objection. The rejected full011 and012 captures remain rejected. Charger confirmation does not replace prospective CPU, power, memory, clock, census and cleanup admission. No full013 attempt had started when this archive was produced, and strict quiet-host completion remains open. [Frozen013 inputs, qualification links and specification](windows-laptop-exploratory-protocol-013.zip)

Archive SHA-256 77ef0d6eca6835f087b77114c871373684134b6b4464b35a0f3e37e52427722a.

## Rejected exploratory laptop attempt 013

The single approved attempt refused its first preflight sample and launched zero benchmark slots. All 33 inspected process rows were permitted, but the complete sample-to-inventory interval took 1,107,467,900 ns against the unchanged 50,000,000 ns budget. The counter read itself took 892,900 ns. The saved generic error combines census-state and timing predicates; independent inspection identifies a timing-budget failure here, not a detected competing process. Available memory was 4,585,127,936 bytes, with AC connected and battery saver off. The archive preserves the exact source, manifest, preparations, raw refusal and original terminal reason. A separate native check found the recorded exact enclosing-guard identity absent. The observer had no saved creation identity, so no exact-identity cleanup claim is made for it. There was no campaign worker, slot journal or measured result. [Rejected preflight and exact guard check](windows-laptop-exploratory-attempt-013.zip)

Attempt archive SHA-256 3686ef75656a67fbe7dc27dc4e873d32326440b33112c5a8d471aad48c0206ac.

## Observer inventory cost diagnostic

Three fixed no-workload snapshots instrumented the frozen013 observer pipeline. Their complete brackets were 785.412, 487.709 and 802.415 ms. Repeated image-path metadata and open operations accounted for the largest measured exclusive costs; content hashing took 17.409–36.953 ms. The first two snapshots inspected 22 external candidates covering four distinct file identities. The profiler adds CPU-clock queries and bookkeeping inside the spans, so its 49.257–69.829 ms snapshot-API spans do not isolate uninstrumented API latency. These observations cannot reconstruct the stage costs of the original refusal or establish that any proposed optimization meets the 50 ms gate.

The third snapshot retained an outside pinned guard and a transient image-query error. These diagnostic classifications do not explain the original013 capture, whose rows were all permitted. The first generated profiler failed before sampling; its source, error and exit are preserved alongside the corrected source, all three snapshots, raw telemetry, analysis and unchanged frozen inputs. No production guard source, benchmark gate or failed capture was changed. [Diagnostic sources and raw stage evidence](windows-observer-inventory-cost.zip)

Diagnostic archive SHA-256 f092428723c692770f1977c6e95a9f4b6c1164842a84b7f9bd7f3c46522be413.

## Low-overhead observer calibration

One fixed three-snapshot control measured the same frozen013 pipeline with QPC boundary reads and no added per-API CPU-clock queries. Complete brackets were 244.1854, 397.7710 and 334.8737 ms; all exceeded the unchanged 50 ms budget. The native snapshot calls took 21.3224, 25.6743 and 22.7875 ms. These observations show that Toolhelp alone did not exceed 50 ms in this control, while the remaining pipeline still requires repair. The populations were 37, 38 and 36 inspected candidates; the middle snapshot retained one unverified process-open failure. Exit zero records diagnostic completion, not benchmark admission.

The archive preserves the corrected source, raw streams, three observations, unchanged frozen inputs, 45-second whole-tree bound and 35-second internal budget. It also preserves the hash-matching unexecuted pre-review source, reconstructed by reverting the two reviewed deadline checks; only the corrected source ran. Different times, populations and instrumentation prevent a paired speed comparison with the earlier diagnostic or original refusal. No benchmark slots ran and no guard source or gate changed. [Calibrated observer source and raw evidence](windows-observer-inventory-calibration.zip)

Calibration archive SHA-256 e0a812fae07dfc176231c5e4bda54ed152441e41b3eeea6a2f27b226868703cb.

## Failed native image-reader qualification

A candidate replaces Python file-metadata calls with direct Windows queries while retaining independent per-row inspection, full 128-bit file identity, four named/held signatures and read-only access that shares reads, writes and deletes. Ten unit tests passed, and an executable compiled against the installed Windows SDK confirmed the structure sizes and offsets. The compiler metadata probe's missing-source diagnostic is retained alongside the successful actual build.

The single native control matched four pinned image digests against both the manifest and frozen013 reader. It then accepted a same-length edit made after its content read and correctly failed qualification with exit one. Replacement, growth and truncation controls did not run. The process handle count returned to its baseline of 167, and the owned fixture was absent. The candidate has not been adopted into a full pipeline, and no speed result exists.

The failed control did not retain the accepted edit's returned metadata or digest, so it does not establish the cause or a regression relative to013. An early derived receipt incorrectly placed013's final metadata query after content-handle closure. A separate correction preserves and withdraws that claim: both readers query final metadata inside the open-handle context. The archive retains the failed source, raw streams, correction, unexecuted unsafe-cleanup draft and corrected ownership tests. No production guard source or gate changed. [Component source, passed controls and failed native qualification](windows-native-image-qualification-001.zip)

Qualification archive SHA-256 989ad9d5d57887db6af8d78ea909d71baf884b7e931dbf5d19c0289a0e8afa8f.

A separate fixed two-file diagnostic retained every metadata query, returned digest and independent post-close content read. The013 case rejected its edit; the014 case accepted the pre-edit digest even though the final bytes differed and its recorded native and Python metadata remained unchanged. The files and observation times differed, and instrumentation affected timing. These two outcomes establish neither a universal API difference nor the timestamp mechanism. Both helpers stayed unchanged. [Matched mutation observations](windows-image-mutation-observation.zip)

Observation archive SHA-256 b9cf0c747815c29f409ced6f7772c8328a5e11d10faf615e4f928ad99e9f9993.

## Two-read image verification: qualification incomplete

A separate candidate always reads the same held file twice, with a checked 64-bit seek to zero between the complete size-bounded reads. It requires both digests to agree and preserves all prior metadata and process checks. Thirteen unit tests passed. The native control matched four pinned images and correctly refused the persistent in-place edit between passes, retaining both differing digests and refusal evidence.

The next control's `os.replace` operation returned PermissionError/WinError5 before a pathname replacement was demonstrated. The control exited one; growth and truncation were not reached. This OS refusal does not prove replacement detection, and its cause is unresolved. The outer error logger omitted the unexpected error's attached partial evidence; that missing evidence is explicitly recorded. Handles returned to baseline167 and the owned fixture was absent. The original failed reader and all records remain unchanged. Two-read consistency is not atomic immutability or loaded-image authentication, and neither full-pipeline timing nor the 50 ms gate has been qualified. [Two-read source, passed in-place control and incomplete native qualification](windows-two-read-image-qualification-001.zip)

Qualification archive SHA-256 e02164c5d081970bf833ecf0fe60ff07a63b6f8ac2057e35d048626a2669e0ef.

Independent follow-up controls successfully grew and truncated their own files and required the exact second-read extent errors, expected final bytes and sizes, and correct first-pass evidence. A separate case successfully replaced the named file after the initial named handle closed and before content open. Full 128-bit identities showed the new held file matched the replacement, and the helper rejected the exact binding mismatch before any hash call. Every case restored its handle baseline and removed its owned fixture.

The separate OS diagnostic confirmed that ordinary replacement succeeded with a closed target but returned WinError5 with the shared-read handle held. That observation does not explain the OS mechanism or qualify detection in the blocked window. The earlier incomplete qualification remains incomplete in its original record. No helper, gate or sharing policy changed, and no full pipeline ran. [Follow-up controls and complete raw evidence](windows-two-read-image-controls-002.zip)

Controls archive SHA-256 72d2a97b6fdd6e863f63df93ca174156ed47fb7f94cf22930c6ff5b8c5591a33.

## Failed native-reader pipeline qualification

The fixed two-read collector was tested in the complete observer pipeline for three snapshots spaced five seconds apart. All three exceeded the unchanged 50 ms budget: 252.5741, 351.2949 and 341.4947 ms. The middle snapshot also retained a transient process-open failure and invalid creation evidence; the other two passed independent file-evidence validation. The process completed with exit zero while `observer_pipeline_qualified` remained false. Zero benchmark slots ran, and no retry occurred.

The native snapshot calls took 21.0799–27.1834 ms. Intrinsic file-inspection envelopes totaled 208.903–287.333 ms, with both content passes accounting for 17.463–24.110 ms. The remaining envelope combines open/metadata calls, Python work and buffer handling; it does not identify a specific API cause. Raw observer CPU deltas were 234.375 and 328.125 ms over roughly five-second intervals on 16 logical processors. These diagnostic intervals include independent validation and emission outside the inventory cutoff. They would exceed the standing 0.1% observer ceiling if scored as phase windows, but are not executed campaign-phase verdicts.

The archive retains exact source/tool bindings, raw counters, census evidence, offline decomposition and preparation history. The first unit attempt failed on a missing policy import; corrected source also rejects zero creation times, directory inconsistencies and policy conflicts, and all seven focused checks then passed. No guard source or numerical gate changed, and the pipeline remains unqualified. [Complete failed diagnostic and controls](windows-native-image-pipeline-001.zip)

Pipeline archive SHA-256 045952810870af384f49a7d9872135a70e63ec358a33b59da54cde860609baaa.

## Native file API cost diagnostic

One fixed four-image diagnostic placed only `perf_counter_ns` brackets around the existing native calls, preserving arguments, sharing and read checks. Both reads matched every pinned digest, all owned handles closed and all bound sources stayed unchanged. The Python312 image inspection took 11.524 ms: three open-call envelopes totaled 6.1374 ms and three closes 2.6267 ms. Its metadata-query envelopes totaled 1.5434 ms, reads 0.4589 ms and seek 0.003 ms; the remaining 0.7546 ms included wrapper, hashing and unattributed work. The other three complete image inspections took 2.976, 9.4611 and 1.3173 ms.

This localizes the large observed Python-path cost to open/close call envelopes, which still include the Python/ctypes boundary. It does not establish an OS/filter cause or a universal path cost. The control inspected each image once, not a full census; projecting its cost across rows would not be a measured pipeline result. The source, raw API returns and errors, exact argument classes and closure checks are retained. A publication-metadata correction explicitly notes that this Python control recorded nanosecond intervals but no raw QPC frequency. No collector source or gate changed. [API-level diagnostic](windows-native-image-api-cost-001.zip)

Diagnostic archive SHA-256 73420df6a90917ef7d63e026f8a8bb1662bcfa0af14a64a89d4e64717427a40d.

## Independent C file API diagnostic

A separate native C control used the same four pinned paths, native file arguments, four full signatures and two complete size-bounded reads with checked rewind. CNG SHA-256 matched each pin on both passes. One warnings-denied MSVC build and one native invocation succeeded. All tracked file/hash handles and the provider closed, sources and binary references stayed unchanged, and no emergency cleanup call was omitted from the log.

The Python312 target's C open/close envelopes totaled 5.5454 ms, compared with 8.7641 ms in the earlier separate Python observation. The relatively expensive path operations therefore also occurred outside ctypes. Different observation times, executable contexts and CNG hashing prevent an end-to-end parity claim or attribution to a specific OS/filter mechanism. The archive retains raw QPC ticks and frequency, every API status and argument, exact compiler/source/binary pins, and unexecuted preparations including the corrected sticky clock-error handling. No collector source, gate or guard behavior changed. [Independent native source and raw observations](windows-native-c-image-api-cost-001.zip)

Diagnostic archive SHA-256 df75877895e00a5271d3ea1e7bcc70289054b95b2cc80a090365fdf44ffd9f9e.

## Partial by-name metadata diagnostic

The documented by-name API was investigated separately; it was not adopted into the collector. The first preparation compiled but refused its SDK trace because repeated separators defeated the path gate, so its binary never ran. A malformed generated parser was preserved unexecuted. Corrected literal source passed syntax checks and eight path controls, and the fresh build verified all four exact reported SDK header paths and hashes.

The single native probe attempted only the Python image. SDK layout and export availability passed, full identity/volume and core fields agreed, and both pinned hashes matched. Contemporaneous by-name and held comparisons agreed on every mapped field. Access time changed across the before/after observations, causing the declared strict control to exit one; the other three images were not attempted. This is neither an API-defect finding nor a rescored success. The two by-name envelopes were 3.7437 and 2.2684 ms, versus 2.5943 and 3.7125 ms for the corresponding legacy named-open/metadata/close brackets in this one observation. No clear full-census speed benefit was established.

All handles closed. Missing deletion/type properties and reparse equivalence remain unqualified, and no collector behavior or gate changed. Both preparations, binaries, exact header evidence, malformed source, corrections and partial raw output are retained. [Complete preparation and partial probe](windows-image-by-name-diagnostic.zip)

Diagnostic archive SHA-256 52c85969003200e5f8c19b44db9e8428c8b8a5a821333afc1a40c2fc7a4769a1.

## Prospective grouped census: source and mock checkpoint

The next observer method retains every relevant process row and its native checks, but shares one two-read file observation among rows with the exact same raw image-path string. This changes the observation schedule: it provides fewer independent content observations than the earlier per-row method. Its METHOD records the transient/ABA, process freshness and loaded-image limits. Twenty-seven fake/mocked controls passed, and two independent Sol reviewers approved the collector source with no unresolved source objections. No native grouped control, census or pipeline had run at this checkpoint.

The separate diagnostic driver replays actual emitted census and telemetry bytes, retains the original timing span and adds a stricter output-inclusive span, and keeps the numerical gates unchanged. Its required native-control receipt remains unset, so it refuses before sampling. The archive preserves earlier unexecuted clock/accounting drafts and both successful mock attempts. The second independent driver source approval arrived after the archived review snapshot; the final receipt-sealed launch plan still needs review. No guard source, protection, default or performance result changed. [Reviewed source and complete preparation](windows-grouped-census-preparation.zip)

Archive SHA-256 24ac9a06a804e7d229fb5eae7a0cdd59a3208d3d133ab3ffad9fdfdc757896d2 (43 verified members).

## Linux direct-child reaper: ABI and object builds

The concrete reaper source uses a single nonblocking held-pidfd wait and a native C status bridge. One ordinary-user ARM64 VM attempt passed the installed-header ABI reporter and both warnings-denied Rust object builds. The C/Rust normalized frame matched at 40 bytes/alignment 4, with native siginfo_t observed at 128 bytes/alignment 8. Header dependency identities and hashes matched before and after compilation.

Only the no-child constant reporter executed. No reaper wait, child fixture, Rust harness, root, cgroup or payload operation ran. These results establish ABI and compiler acceptance, not actual child reaping or backend readiness. Exact sources, objects, binaries and raw evidence were preserved before identity-checked cleanup; the original scope and 11 recorded controller/tool PIDs were independently absent. [Complete limited qualification](linux-direct-child-reaper-abi.zip)

Archive SHA-256 7f890abddee02d8a0c14e9f9b2fd0ac7456667926750dfba0172263f29d3b67a (112 verified members).

## Deferred macOS administrator validation

The corrected fixed-M8 credential/quota control compiled on the physical macOS ARM64 host with an explicit SDK. The earlier header failure and incorrect zero-group-readback source are preserved; that source never ran as root. The reviewed corrected invocation stopped at sudo authentication before either the privileged controller or native control started. Administrator-only tests are deferred at the user’s request and tracked in [issue10](https://github.com/WikdSolvemProbler/process-tree-guard/issues/10). The archive preserves sources, binaries, exact plans and refusal logs. Ordinary-user checks and identity-verified cleanup completed after preservation. There is no privileged native result, fork-fence proof or custody qualification from this preparation. [Prepared inputs and refusal evidence](macos-admin-validation-preparation.zip)

Archive SHA-256 487ec5a2899c4312ea2a15c0cc35ade3bcc6ece106ddd5cee310bf80690459bd.

**Next:** The [`reduced-run` folder](reduced-run/README.md) holds the preserved package.

[↑ Back to Contents](#contents)
