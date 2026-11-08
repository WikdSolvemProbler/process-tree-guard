# Linux staged entry: ordinary arming repair checks passed

This Rust-only successor of published `254700bbb631e5c2e01105d985fd80d4767ec9b5` records typed original-deadline evidence at eight exact watchdog-arming refusal sites using unchanged ABI4 receipts. Raw poll counts, untouched fields, messages, masks and timestamps must match the actual branch. Every arming outcome retains the same authenticated cancellation endpoint and sticky entry/pre-local-shutdown or final-no-internal observations; genuine peer cancellation or a poll fault vetoes124 even after generic-first gate cancellation. Failed kills, independent faults and quarantine remain125. The private deadline proof is non-Copy/non-Clone and borrowed; the first invalid result from every Rust gate-cancel attempt remains a sticky arming veto. Existing focused regression logic exercises the production gate reducer without new test names or native effects. Exact implementation `40a9010d0b2b32f2ef08780b8bff838d2ea8a0630ca0eb68208732eb328c7dda` passed all11 ordinary build/link steps and12 tests (0 failed/ignored), including the production gate reducer and pure arming-proof/peer-veto tables. All90 artifacts were verified;2 exact scratch roots were removed and original scopes were absent. Native arming/payload/release/EXE1 and privileged lifecycle behavior were not executed. Earlier build014 preparation failures and denied evaluations remain preserved and are not passed runs.

The last tested predecessor `4a9fd73f868cb93b60c41a6ca1e4574f8da7d735792568699eb1487ed76fadf6` passed all11 ordinary compile/link steps and12 tests, with0 failed/ignored and no timeout. The existing gate test verified schema4/spec160/27-word constants, prlimit64=261 and CORE guard literal1 through the same production C ABI query/validator;17 IPC refusals settled descriptors and3 gate mappings were destroyed. All90 artifacts were verified before exact removal of2 owned scratch roots; original scopes were absent. These checks performed no actual CORE limit/credential/core-window/real-ELF-policy or privileged lifecycle operation. No global sysctl, caller/parent limit or runtime001 input changed; public dispatch and full caller-context inheritance remain open.

An earlier tested predecessor `78c81cf7ce646ada8137a259a35f37d913eab014f7bed5586a6f578f22133784` passed all eleven ordinary-user compile/link steps and all twelve tests, with no failure, ignored test or timeout. Checks cover policy grammar/default refusal and ordinary parser/IPC/gate/classifier behavior; they do not open a real /etc grant policy or execute privileged enforcement. All17 IPC refusals settled descriptors and three gate mappings were destroyed. All90 artifacts were verified before exact removal of two owned scratch roots; original scopes were absent. No service is installed or activated, and default Linux dispatch remains disabled.

Transparent `run_library` refuses125 before capture, settings or activation. The distinct `run_library_delegated(command,seconds,mode,SeparateService::Acknowledged)` and standalone first flag `--separate-service-context` explicitly disclose a different service authority/resource context. Acknowledgment grants no privilege. The library still requires existing NNP1 without changing its host; only a dedicated acknowledged CLI can set its own NNP.

The administrator policy is fixed at `/etc/process-tree-guard/delegation.policy`: at most4096 bytes, exact LF header `PTG-DELEGATION-1\ncontext separate-service\n`, then0..64 strictly increasing canonical decimal `uid N\n` lines for1..4294967294. Missing/invalid files or an empty/unmatched UID set deny125. Protected root-owned nonwritable nofollow ancestry, type/mount/name identities, the held file and exact bytes remain owned through outcomes. Actual kernel-authenticated requester UID—not a request UID—selects the grant before RuntimeMax extension, private listener or provisioning. The same snapshot is revalidated before provision, payload creation and permit. This is admission-snapshot semantics, not immediate active-run revocation.

PTGEX6 requires acknowledgment byte7=1 in both immutable authenticated requests; its40-byte header, mask at38, original owner capabilities, IO/CWD/environment and absolute BOOTTIME cutoff are retained. Ten existing test names/assertions are retained with explicit-acknowledgment adaptations and two missing/invalid-ack IPC refusals. The two added grammar and early-refusal tests also passed. These tests do not read a real privileged policy, create a service or execute a payload. All C/H, service/socket template, selector and Windows bytes remain unchanged.

The previous d0 attempt remains negative history: build11 passed but tests9 passed/1 failed because a protected-root inode check rejected a legitimate caller-owned proc directory. Repair033 retained genuine procfs/type/mount/name/thread identity checks while separating kernel proc-child ownership from protected installation/cgroup ownership. Tested d6 then passed11 build steps and10 tests,15 descriptor-settled IPC refusals, manual2/auto7 original owners and the exact60.25-second interval, plus three parent-only gate mappings destroyed. All89 artifacts were verified before exact removal of two owned scratch roots; original scopes were absent.

Those ordinary checks did not execute payload/child-umask/watchdog/terminal/root-client lifecycles. Privileged activation, actual delegation enforcement, cancellation/natural-stop/cleanup transitions, native timeout124 cause, Linux5.14 coverage, Cargo/public wiring and comparative performance remain open. Full transparent preservation of caller seccomp/LSM/Landlock/rlimits/cgroup ancestry remains an independent unmet requirement; this explicit administrative mode is not a replacement claim.

Exact predecessors and failed attempts remain preserved locally, including proposal002 and its objections. No raw archive, private machine/account/route or transport commands are published. PROVENANCE.json separates the historical tested4a9 checkpoint from this uncompiled arming repair. Native refusal-boundary qualification and full payload/release/EXE1 timeout124 parity remain open.

---
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/sys/linux

## Contents

- [src/sys/linux](#srcsyslinux)

The following records describe predecessor-component checkpoints. Their then-current limitations are historical; the staged implementation and remaining enabling conditions are stated above. Original sources, failed attempts and raw evidence remain preserved. Linux is still not an enabled backend.

## Native containment probes

An Ubuntu 24.04.5 ARM64 guest running Linux 6.8 verified atomic placement with `clone3`, refusal of a protected worker's attempt to enter a sibling cgroup, refusal of outer cgroup namespace reentry, and protection against a switch to threaded mode.

A worker in the combined user and cgroup namespaces could still kill a disposable sibling through an inherited cgroup mount alias. The guest refused the attempt to make mount propagation private with `EACCES`, so the masked workload was withheld. A Linux backend needs a verified boundary for these controller paths before executing a command.

The archive preserves the C sources, guest commands, raw outcomes, failed first attempt, and cleanup checks. The probes ran in fresh delegated scopes with independent finite deadlines. [Native probe evidence](../../../tests/evidence/linux-cgroup-native-20261106.zip)

### Root-owned supervisor probes

In a separate system scope, a supervisor atomically placed the payload in a root-owned cgroup and executed it with the caller's UID, GID, and supplementary groups, with `NoNewPrivs` enabled. The payload was denied writes to its own and its sibling's cgroup controls, and the root-owned common ancestor blocked migration into a separately writable leaf.

A one-shot watchdog drained the payload cgroup on supervisor loss, owner loss, deadline expiry, and normal completion. Held pidfds verified the original payload and its detached, double-forked descendants. The sibling survived all four cases. The deadline and owner-loss cases returned 124 and 125, and normal completion preserved the fixture's exit 23.

The root-owned control and lifecycle archive contains the source, outcomes, and independent cleanup checks. [Root-owned control and lifecycle evidence](../../../tests/evidence/linux-root-supervision-20261106.zip)

A startup-fence prototype refused late launch requests after owner loss and deadline expiry. Killing the supervisor between atomic placement and release closed the exec-permit pipe and left the child reaped without an exec marker. Two permitted commands returned 7 and 197, and both statuses were forwarded exactly. [Startup fence evidence](../../../tests/evidence/linux-startup-fence-20261106.zip)

A Rust prototype recovered from watchdog death before the atomically placed child closed inherited descriptors, while the child waited for exec permission, and after exec. In the first case the watchdog pidfd became ready while the result pipe still had a writer. The supervisor drained the cgroup using its held process identity. A missing executable returned guard status 125 with an ENOENT receipt, while a real program's exit 127 passed through. Supervisor and owner loss also passed, and the independent sibling survived all seven cases. [Rust supervision evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

A one-shot admission fixture used kernel credentials on a private Unix socket and a held pidfd for its direct child. The supervisor launched the command with the verified UID, GID, and groups and forwarded exit 127. Seven refusal cases left the exec marker and payload cgroup absent. The existing seven lifecycle cases still passed, and the independent sibling survived. [Admission proof and raw receipts — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

A follow-up fixture admitted an independently launched ordinary user through a bounded one-shot root rendezvous. The accepted socket credentials and per-packet kernel credentials matched the transferred pidfd. Seven malformed or mismatched requests left no payload leaf or exec marker. The valid fixture forwarded exit 127. Caller exit before release prevented exec, and caller exit after exec drained the payload cgroup. The independent sibling survived. [External admission proof and raw receipts — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Both admission fixtures ran on Linux ARM64 6.8. The product backend still needs an installation and activation policy, general command authorization, qualification on the supported kernel floor, and concurrent lifecycle coverage.

A shared deadline-validation prototype passed focused checks on Windows x64, Linux ARM64, and native macOS ARM64. The recovered census classified its new module as pure after the current baseline pins were reconciled. The matched Windows release changed its mapped code section, and the historical generator still emits an obsolete explanation for `environment.rs`. The prototype remains outside production pending speed qualification and a refreshed design gate. [Source, native receipts, and raw censuses — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

The smaller shared numeric deadline predicate now lives in `src/deadline.rs`. The matched Windows x64 MSVC release images have identical machine code and runtime data. Native ARM64 tests verified the predicate, and the product retained its intended unsupported-backend refusal on both Linux and macOS. Linux activation and containment remain subject to the backend acceptance requirements. [Exact source, build, and census evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

## Generic request prototype

The one-shot admission prototypes used a fixed command. A standalone Rust prototype now decodes a bounded, versioned timeout and argument packet. It preserves native non-UTF-8 bytes and empty arguments after the executable, refuses malformed framing and embedded NULs, and prepares owned C argument buffers. It reuses the production numeric deadline predicate and keeps positive durations from becoming a disarmed timer value.

The ordinary-user Linux ARM64 6.8 VM and Windows GNU host each passed seven focused tests. The prototype does not launch a payload. Socket integration, authentication with the new packet, caller environment and stdio, native containment, and kernel-floor and concurrency qualification remain open. [Prototype source, earlier compiler failure, and native evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

A native transport fixture passed all eight tests over an anonymous SOCK_SEQPACKET pair in the Linux ARM64 6.8 VM. It matched kernel socket and message credentials to a held pidfd, verified close-on-exec, and accepted a packet exactly at the 128 KiB cap. Seven kernel-backed refusals covered payload and control truncation, missing and duplicate descriptors, a descriptor of the wrong type, a malformed frame, and peer EOF. The descriptor count returned to baseline in each refusal. This fixture used one ordinary process, so a separate client and privileged activation remain unqualified. [Transport source and native evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

A separate native test connected distinct ordinary-user child processes to a private SOCK_SEQPACKET listener. It admitted the live client with matching socket credentials, packet credentials, and pidfd. It refused a mismatched pidfd, a queued request from an exited client, and a request sent over an inherited endpoint with different socket and packet identities. Each original child was reaped and each lane verified EOF and descriptor cleanup. This proves those four sequential admission cases. Integration with production activation and containment remains open. [Cross-process source and native evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

The release prototype kept the authenticated owner pidfd after transport EOF and checked it alongside an absolute CLOCK_BOOTTIME timer at the permit decision. The live control received one permit and wrote a marker. Owner exit and deadline expiry each closed the permit pipe without a marker, although both had passed the earlier check. All four prior admission cases passed in the same native run. The result covers the final release observation. Production still needs the watchdog to handle owner loss or expiry after that observation and the native payload containment path. [Release-check source, native receipts, and initial compile failure — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

A production-shaped preparation prototype now holds the decoded command, verified caller identity and groups, the original transferred pidfd, and the absolute release timer together. The Linux ARM64 VM passed both ordinary-user policy lanes, retaining the original argv and owner handle on allow and closing the handle on deny. The four admission and three release-decision controls also passed. The fixture ends before cgroup allocation and payload execution. [Prepared-run source and native receipts — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

A standalone child launch plan passed two scripted tests on the Linux ARM64 VM. It refuses release on absent placement, owner loss, or deadline expiry. It checks that each failed setup stage stops the sequence, preserves the distinct exec error, and passes the intended credential and byte-argument inputs to the scripted child operations. This probe uses the PreparedRun fixture values with simulated placement and release. Native privilege changes, cgroup activation and payload execution remain outside this evidence. [Source and native scripted test receipts — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 51095f8862bffad087ef183d515ea3eb8ff28b2b5bc2acce97306b09d6a3d3e1.

The authenticated launch bridge consumed the verified caller identity, original argument allocation, owner pidfd, and absolute lease. Five ordinary-user Linux ARM64 lanes exercised a live scripted sequence, refusal after a scripted UID failure, native owner loss, absent placement, and policy denial. The original argument allocation reached the scripted exec boundary. The four transport and three release controls also passed. Descriptor and scratch cleanup were verified. Placement and credential operations were simulated, and the fixture executed no workload. [Bridge source and native receipts — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 c7acd929cc66914d16ed3ba30d662b56b71178b8ca986cf1f413d5b2c6f79f70.

A native ARM64 Linux compiler check now covers the proposed activation adapter against the actual PreparedRun and lease types. The adapter carries the original owner pidfd, absolute timer, verified identity and argv through its cgroup handoff interface. Both ordinary and test builds passed warnings-denied object generation on Linux ARM64. The earlier metadata checks also passed, producing empty binary-crate metadata outputs. No adapter entry point or payload ran. Native provisioning, watchdog activation, permit delivery, credential changes and cleanup settlement still require runtime integration. [Exact sources, compiler receipts and independent cleanup — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 a093c2af157eb2148c241adfff6d5be540b8a2097f37e8cdaf33a8f21f9b5a32.

An ordinary-user Linux ARM64 test exercised the authority candidate’s real openat, statx and fstatfs calls. It rejected mismatched inodes, symlinks and incorrect descriptor types, and refused provisioning before creation. The first native attempt exposed incorrect ARM64 open flags. Corrected source passed, and a C program using the guest headers independently matched the selected constants. The canonical installation-path correction passed the same test. The archive preserves the failed attempt, corrected source, successful receipts and independent cleanup checks. That read-only qualification did not exercise positive root-owned provisioning, installation provenance, generic placement or watchdog integration. [Authority source and raw evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

The older ARM64 Rust lifecycle fixtures used the incorrect open flags. Their saved outcomes remain evidence of those executions; they do not establish no-follow path opening. The corrected authority component has the read-only scope stated above. Bounded settlement must retain or resolve created-leaf ownership on every arming error and consume the armed run through verified drainage and exact reaping before production activation.

### Native empty-domain settlement

On the Linux ARM64 VM, a hash-verified root-owned binary exercised the actual factory and bounded settlement API. The sequential fixture exclusively created its fixed test root, provisioned one empty leaf, wrote through the retained kill descriptor, verified emptiness, revalidated the original identities and removed the exact leaf and root. Both finite system services reported success. Independent checks found the fixed root, private root scratch, service cgroups and selected workers absent. The ordinary handoff was archived before removal under its original UID. The fixture executed no workload and recorded zero poll wakeups, so populated polling, child reaping, watchdog behavior and full one-shot activation remain unqualified. [Source, native receipts and cleanup — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 609e1812f40f0387680f2e8e7686ddb8f1262469e47581cf752dffb0d4a69b35.

### Termination-first settlement repair

The next prototype requests termination through its original held kill descriptor before clock reads, cleanup-timer setup or pathname revalidation. The shared driver owns the actual controls and returns that custody plus the write outcome on every returned error. Failed or short writes do not enter wait setup. Removal still requires the bounded timer, validated original identities, observed emptiness, exact unlink and absence confirmation. The receipt timestamp is explicitly after the kill write returns.

One ordinary-user Linux ARM64 attempt compiled the full module graph with warnings denied and passed four selected controls. The native control wrote through a held descriptor to its own ordinary file, then exercised real expired-deadline refusal while retaining the descriptor. The other three check the shared driver's ordering and outcomes. There was no cgroup write, payload or root fixture. Remote artifacts were preserved and verified before exact identity-checked cleanup; independent checks found the original scope, cgroup path and recorded PIDs absent. [Source, native controls and cleanup receipts — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 eda65f3210dbdc12a72b9e0f9ec4ca76ab10f94aaf348b674e446d9075e5df12.

A native write can still block, so this component requires independent watchdog coverage before payload activation. It does not settle created resources after every arming failure or consume `PendingSettlement` through exact child reaping. Populated drainage, actual cgroup failure recovery and the full one-shot port remain unqualified. The production Linux backend stays disabled.

### Typed arming-failure custody

The next adapter returns owning arming failures instead of erasing them into a string. Its variants retain the actual factory failure and optional created leaf, or verified controls with any direct watchdog child. Direct child records retain both PID and pidfd. Activation errors also retain the original prepared run, including its owner handle and absolute lease.

Both ordinary and test-object builds of the complete module graph passed on Linux ARM64 with warnings denied. The objects were not linked or executed, and no activation or test harness ran. Exact sources, compiler outputs and independently checked archive-before-removal cleanup are preserved. [Typed API and compiler evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 c19869448c84258d15f47ebef214e38c6b57d1d1817d23e89f55eaad3d65a155.

This establishes compiler acceptance of the owning API. The native arming and clone contracts are not runtime-qualified, and dropping a returned owner does not guarantee settlement. Consuming arming failures and `PendingSettlement`, exact child reaping, watchdog integration and production activation remain open.

### Retaining the drained domain until final removal

The next component separates termination and drainage from removal. A successful drain now returns an owner containing every original control, the same armed cleanup deadline and its receipts. Returned failures also retain the armed deadline once it exists. Final removal consumes that owner, reuses the deadline and preserves partial removal state on error. Its private unsafe caller contract requires actual prior reaping and no future placement; that contract is not itself a reaping proof. The old combined settlement shortcut remains only in the empty test fixture.

Both ordinary and test-object builds of the complete graph passed on Linux ARM64 with warnings denied. No test harness or component was executed. Exact objects, source continuity and independent archive-before-cleanup receipts are preserved. [Owning drain/removal source and compiler evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 9f7532dc455e73b8b82d0f7e73fcbbea0c5b4788613d9286f4a39e48ac7051f1.

Native pidfd reaping, subreaper ownership, exec-result observation, mandatory terminal orchestration and watchdog integration remain open. The production Linux backend remains disabled.

### Direct-child reaper ABI qualification

The next component performs one nonblocking wait through an owned child pidfd and decodes native status through a C bridge built against the installed Linux headers. Pending and error paths retain ownership; a validated exit receipt retains the original pidfd and raw status. A later clock failure preserves a status already consumed. This source contract still needs runtime qualification with genuine children.

One ordinary-user Linux ARM64 attempt passed the native constant/header reporter and warnings-denied ordinary and test-object builds of the complete Rust graph. The normalized C/Rust frame was 40 bytes with alignment 4; native siginfo_t was 128 bytes with alignment 8. Only the constant reporter executed. No child was constructed, no component wait call or Rust harness ran, and no root, cgroup or payload operation occurred. Installed header identities and hashes matched before and after compilation.

All 58 remote artifacts were preserved before exact identity-checked cleanup. Independent checks found the original scope, cgroup path and 11 recorded tool/controller PIDs absent. The archive retains source, installed-header provenance, objects, reporter, raw streams and cleanup receipts. [Source and limited native qualification — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 7f890abddee02d8a0c14e9f9b2fd0ac7456667926750dfba0172263f29d3b67a.

Actual normal exit, Pending, signal termination and repeated-wait behavior remain untested. Terminal orchestration, subreaper adoption, watchdog and exec-result handling, populated settlement and production activation remain open.

### Sequential reaper attempt stopped at linking

The next ordinary-user ARM64 attempt compiled the native C bridge/fixture and both Rust objects with warnings denied, and passed the constant ABI reporter. Linking the selected Rust test failed on an unresolved stack-protection symbol, with the dynamic loader reported as missing from the link. The test was never listed or executed; zero fixture children and zero component wait calls ran. The source, failed link argv and stderr, all 81 native artifacts and earlier unexecuted preparations are preserved.

Independent verification covered every archive member and confirmed exact scratch removal, original scope/cgroup absence and all 14 recorded PIDs absent. The link log suggests an ordering problem with appended C objects; that diagnosis and a normal static-library repair still need native validation. Compiler protections remain enabled, the failed attempt stays failed, and actual reaping remains unqualified. [Complete failed attempt and cleanup — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 a2e04ba953e7974a23b44164bc07e6ce3236d90f6abdabe811811c30588c2b14 (153 verified members).

### Actual sequential child reaping and static-link repair

A fresh ordinary Linux ARM64 attempt linked the same C object bytes through Rust's normal native static-library interface, retaining the stack-protection reference and all compiler hardening. The link map identifies the dynamic loader as the provider for that reference. This repaired the earlier link-order failure without changing the reaper or fixture code. The failed attempt remains preserved.

The selected native test passed all three sequential cases: exit 23; a genuine Pending result retaining the original PID/pidfd followed by exit 29; and pidfd-directed SIGTERM reported as signal 15. Each validated reaped receipt retained its pidfd for a separate raw wait that returned ECHILD. All native status bytes were retained. An independent [offline audit](../../../tests/evidence/audit_reaper_static_link.py) decoded those bytes using the installed-header offsets, matched the normalized fields and terminal outcomes, verified every archive member, and checked the unchanged C objects and link map.

All 103 artifacts were preserved before exact removal. The original scratch, scope/cgroup and 26 recorded PIDs were independently absent. The archive also preserves the read-only inspection's user-bus refusal, corrected inspection, unexecuted parser revisions and generation-size refusal. [Complete native qualification and cleanup — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence)

Archive SHA-256 f546645adef6e0b470837f5a68a124cb073f5a92c7ec8b9626fac4eeb6d7ea34 (221 verified members).

This qualifies the ordinary direct-child reaper component and link repair. Timer-expiry, cleanup-failure/quarantine and post-reap clock-error branches remain unqualified at runtime. Populated cgroup drainage, original lease/owner and exec-report supervision, watchdog/adopted-child settlement and a consuming terminal controller still require integration. The production backend stays disabled; no guard performance result follows.

### Consuming terminal-controller source and object qualification

The next concrete controller retains the original absolute lease, prearmed cleanup authority, supervisor's own pidfd, owned exec-report reader and direct children. Stop events latch a held kill before later diagnostics; drained controls remain owned through direct and adopted reaping, with removal last. An interrupted active poll conservatively cancels before later clock/allocation work. These are reviewed source contracts, not exercised runtime guarantees.

One ordinary Linux ARM64 attempt compiled the installed-header C bridge and actual full-graph normal/test Rust objects with warnings denied. All six compiler steps passed, 125 header dependencies matched and all three relocatable objects were preserved. No link, compiled test/controller/reaper/reporter, child, root operation or cgroup write ran. All 49 artifacts were verified before exact scratch removal; the original scope/cgroup and seven recorded PIDs were absent. [Complete source and compiler evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence) and [independent offline audit](../../../tests/evidence/audit_terminal_controller_objects.py).

Archive SHA-256 0848ccef7232bb14df277ab4e0ad4a65a637f7b020af3eb1ff1ad3da64b7c4eb (133 members). Native arming/placement, partial-arm settlement, trusted bootstrap, product entry and all terminal runtime/failure paths remain unqualified. The production backend remains disabled; no guard performance claim follows.

### Consuming arming failures and partial creation custody

The next source consumes actual activation/arming failures. It retains the original request/lease, existing cleanup timer, context, created-leaf controls and optional watchdog. Verified domains use the real held-kill-first drain/reap/remove path. A never-activated leaf is removed only after original identity/empty-domain and no-child context proof, with removal last. Ambiguous or refused creation retains the attempted held parent/name and raw outcome without assuming deletion authority. No release, signal or child exit receipt is invented.

One ordinary ARM64 attempt compiled both full-graph normal/test Rust objects with warnings denied. The Rust version query and two object builds all succeeded; no C compile, linking, compiled test/controller, child, root or cgroup operation occurred. Prior unchanged C/ABI evidence stays historical. All 37 artifacts were preserved before exact removal, and the original scope/cgroup plus four recorded PIDs were absent. [Source, objects and raw cleanup evidence — raw archive withdrawn](../../../tests/evidence/README.md#withdrawn-private-evidence) and [independent offline audit](../../../tests/evidence/audit_terminal_controller_objects.py).

Archive SHA-256 cdb04781716f2ae29b2cf5a21fa83f949ded244af3b409008abbbb7d7a56e1e0 (103 members). Native arming/placement, independent setup supervision, runtime/failure-path qualification and product integration remain open; the production backend remains disabled.

**Next:** The wiki Platform Support page states the tier of each named place.

[↑ Back to Contents](#contents)

## Private entry deadline timer

`entry_timer.c` and `entry_timer.h` provide an opaque owner for an absolute `CLOCK_BOOTTIME` timer that delivers `SIGKILL` to its own process. Callers must establish cleanup coverage before retiring the startup timer. The cleanup timer rejects deletion. Ownership survives partial creation or observation failure, and the 96-byte receipt retains the native call results and the two clock observations around timer readback. No timer handle representation crosses the C boundary.

A hardened GCC C11 build linked with `librt` passed three ordinary-user checks on Linux 6.8 ARM64: startup expiry, cleanup-deletion refusal with the timer still active, and handoff that survived the old startup cutoff before cleanup expiry. All three ended with the expected signal, with zero parent fallback kills or timeouts. Fifteen artifacts were verified locally before the exact scratch directory was removed; the original execution scope was absent. Both independent reviewers cleared the exact component and bounded test scope.

This qualifies the timer component only. It establishes no whole-tree containment, precise expiry bound, performance result, supported-kernel-floor runtime result, or public Linux activation. Raw host and transport receipts remain local for publication privacy; this summary does not replace publicly accessible raw evidence. Trusted service entry remains under implementation and the public backend selector remains disabled.

## Checked product entry source (qualification pending)

The staged Linux tree now contains actual ordinary-client and root-service main sources, a root-created activation/private-listener exchange, protected-file and kernel namespace checks, authenticated PID1 unit/property queries, and consuming scope construction. The client captures the original full owner set and absolute BOOTTIME cutoff before activation. The library requires existing NNP1 without modifying its host; the dedicated CLI sets NNP only for itself. The service retains all four root UID/GID identities, creates no child before checked recursive unit authority, and feeds the authenticated original IO/CWD/environment and lease into the existing NativePort/controllers.

The actual root/client binaries and fixed socket/template sources are described in [deployment/README.md](deployment/README.md). They are not installed or enabled by this source change. Default selector, Windows, Cargo and build-script wiring remain unchanged. No fake origin constructor, user-supplied service snapshot, sudo prefix or daemon is used.

Root outcome frames retain bounded raw native timer evidence and distinguish payload exit125 from guard/exec failure. A complete result remains provisional through exact service termination, original unit empty/typed retirement proof and retained manager invocation/ExecMainPID/code/status. The retirement inference is restricted to the originally live held unit, raw original events ENODEV and exact same-parent/name ENOENT; replacement/other errors refuse. It proves no live tasks or online children in the original retired cgroup, not process reaping or release of all kernel references.

The full entry sources have since passed fresh native compilation/linking and the eight existing ordinary tests; see the [checked service-entry source checkpoint](#checked-service-entry-source-checkpoint). Temporary bounded deployment, privileged lifecycle/failure cases and kernel-floor qualification remain pending. Native timer-refusal124 cause classification remains open; generic failures and owning quarantine remain125. Raw public archives remain withdrawn under the privacy policy; private connection details are not published.

At the prior847 checkpoint, the private library client had no post-presentation cancellation/join and returned incomplete125 quarantine. Cancellation v2 below implements that missing path in source; full source/build/native qualification is still pending. Standalone client process exit remains independently watched by the original requester pidfd.

## Checked service-entry source checkpoint

Build candidate `84762a4132e57a670d99a096899c7509de40140840b31d9caf87ebd90744594c` passed all eleven ordinary-user compilation/link steps on Linux 6.8 ARM64: six fresh hardened C objects, their archive, the full normal Rust object, the existing test harness, and both actual root-service and client executable sources. All eight existing serial parser/numeric/IPC tests passed, with zero failures or ignored tests. Their bodies are unchanged from the preceding owner/deadline qualification. The IPC test retained the original owner chain, 60.25-second absolute interval, IO/CWD/environment and fourteen refusal cases with descriptor settlement.

Review corrected the final-status/cgroup-pruning race using a typed original-object retirement receipt; protected the public system-bus endpoint before trusting its replies; retained exact synchronous socket cleanup and verified per-service PID1 RuntimeDirectory cleanup policy; and stopped diagnostics from changing inherited stderr flags. The compiler also found and corrected a clashing write declaration and shared-API reexports. Earlier failed transfer/build attempts are preserved locally. All 89 build/test artifacts were checked before exact removal of both scratch roots; both original execution scopes were absent.

The generated root/client executables were linked but never run. These checks execute no privileged entry, payload, watchdog or terminal controller. PID1 directory cleanup and actual cgroup retirement remain runtime-unqualified. Post-presentation library errors still return an explicit finite nonsettled quarantine; authenticated per-call cancellation and bounded join remain required before enablement. Public Linux dispatch, kernel-floor coverage and comparative performance remain open. Raw machine/transport receipts stay local; this summary does not substitute for public raw evidence.

## Authenticated caller cancellation source and ordinary checks

The actual final accepted private connection now mints an opaque cancellation capability bound to the same authenticated requester. It remains owned through launch, native arming, terminal/failure owners and early outcomes; no optional replacement owner or extra wire right is used. The watchdog arming ABI is4 (21 words), readiness is112 bytes with seven logical poll slots, and terminal capacity is255. The current payload-spec size remains152 bytes; PTGEX5 is described below.

The same connection is polled directly by the controller. Recognized peer cancellation first latches the shared peer cause before the held domain kill. Generic cancellation cannot relabel an earlier cause. Unknown events and every independent mask, ABI, exec, poll, kill and cleanup fault remain faults. Watchdog exit125 is always a native fault despite later peer/owner/deadline events; exit124 alone never creates a timeout result. Expected cancellation still requires the actual first-cause/same-FD evidence and a natural clean trusted watchdog outcome, plus the existing complete drain/reap/remove receipts.

On a post-presentation client error, the client records shutdown(SHUT_WR) and attempts a join under one fixed5s cap narrowed by the original observation cutoff. A valid already-consumed root frame and its actual root code are reused. The original client error remains125. Missing/invalid native settlement, failed join or deadline exhaustion retains an explicit owning quarantine; empty-domain observations alone do not fabricate a reaping receipt. The source makes no instantaneous no-exec-after-shutdown claim.

The prior eight test names are retained with the PTGEX5 adaptations described below. Two focused regressions cover three actual native shared-gate operation sequences and pure watchdog125/fault-preservation classification. All ten ordinary tests now pass; the whole cancellation lifecycle remains unexecuted. No new framework, helper or daemon was added.

## Caller thread umask and watchdog cause source repairs

PTGEX5 replaces the old reserved LE16 header field at38 with a validated mask0..0777; the header remains40 bytes. OriginalInvocation captures that value once before activation from a genuine descriptor-bound procfs calling-thread status chain. No host umask(0)/restore is performed, and no TGID/main-thread mask is substituted. Both authenticated requests compare the complete immutable metadata. Like caller environment/cwd, this is validated caller-context data rather than a privileged TID attestation.

Payload schema3 uses an explicit u32 mask at92, formerly padding; total size152 and existing pointer offsets stay unchanged. Its18-word handshake declares the field and limit. The contained C child applies/readbacks the mask beforeREADY/exec; the root service retains0077. Existing codec/transport tests are adapted in place for version5, mask roundtrips/bounds and an actual invalid-mask refusal; prior owner/time/IO/framing/numeric assertions remain. These sources now compile and the ordinary capture/codec/refusal tests pass; contained-child mask application/readback remains unexercised.

A WATCHDOG primary stop with no genuinely established owner/deadline/caller-cancellation cause now returns guard125/unknown; a later lease sample never turns a guard-killed payload137 into an ordinary command result or invents timeout124. The intentional-signal race is addressed by the source-only natural-stop repair below. Explicit administrative execution-delegation/confinement policy remains open; this is not transparent caller-sandbox or resource-limit inheritance and is not enabled.

## Natural watchdog stop source and handoff

After the genuine primary stop/gate latch and confirmed payload-domain emptiness, the controller records pre-local caller evidence and an explicit INTERNAL stop request before server-side SHUT_RD. The same endpoint remains available for result transmission. Readiness induced by this local request is never promoted to a new caller STOP_CANCEL. The watchdog must report a natural clean124 with valid actual original/peer/internal evidence. Every125 or forced signal remains a fault. A natural reap is capped by min(existing cleanup bound,now+1s), including post-reap time validation; fallback kill/reap is finite but explicitly uncertain/quarantined. No execution lease is extended and no new timer/helper/framework is added.

The combined cancellation/PTGEX5/umask/natural-stop d0 predecessor subsequently passed all eleven ordinary-user compilation/link steps. Its ten-test run failed: nine passed and native_seqpacket_admission_and_refusals refused during OriginalInvocation::capture because a generic protected-root inode check was applied to an ordinary caller's proc directory. That failed attempt remains preserved. Neither the compiled entry nor native payload/watchdog lifecycle was executed. Administrative delegation policy, caller confinement/resource-context semantics, native timeout124 classification and supported-kernel-floor qualification remain unresolved.

## Procfs caller-directory repair and ordinary validation

The failure above is repaired by a procfs-only child-directory opener for TGID, ns, task and TID traversal. It retains descriptor-relative O_PATH/directory/nofollow/CLOEXEC opens, genuine procfs checks, the same held parent mount, and exact name-to-held-inode binding. Kernel-generated proc children may be caller-owned or change ownership with dumpability; their authority does not require UID0 or a caller-UID match. The physical /proc root and all generic protected installation/cgroup directory checks remain unchanged. Calling-thread identity, thread-self/TGID validation and the read-only umask snapshot are preserved.

The exact repair changed only service_scope.rs executable bytes; all ten test bodies and C/H files remained unchanged. Tested d6 subsequently passed all eleven ordinary compile/link steps and all ten tests, including genuine calling-thread umask capture and three parent-only gate mappings. The earlier d0 result remains9 passed/1 failed in preserved negative history. Payload/child-umask/watchdog/terminal/root-client lifecycles, privileged activation, context inheritance, native timeout124, kernel-floor, public wiring and performance remain open. Raw evidence and pre-repair bytes remain local under the privacy policy.

## Protected host dumpability2 contract (native qualification pending)

Host mode is selected only from the privileged read-only activation receipt, never a request or private wire field. Mode0 keeps its previous child path and CORE limits. Mode2 requires explicit reviewed Linux5.14.x or6.8.x family compatibility, the actual held protected current LP64 ELF and a readable current hard RLIMIT_CORE of at least1; mode1, unknown families/images and hard0 refuse. A release-family classification is not kernel attestation or runtime qualification.

In the contained child, the reviewed contract captures CORE and temporarily sets soft1 with the same hard limit while all IDs are root. It retains real/saved UID0 while effective/filesystem UID and GIDs become the target and capability P/E/I sets are cleared; NNP remains1. With that credential mismatch still protecting against same-UID remote prlimit, the child establishes/readbacks dumpability0 and restores the exact child-inherited service CORE limits. Only then does the final real/saved-only UID drop occur, with effective/filesystem UID and cleared capability sets unchanged. Final IDs, caps and dumpability0 must be verified before READY; no saved root reaches READY or exec. This does not claim the capability bounding set is all zero.

Payload schema4 appends host_dumpability at152 and reserved0 at156 (total160, gate144 unchanged); the27-word ABI also declares CORE layout, constant and architecture syscall. Spawn/readiness/commit and the16-byte private READY use schema4. Existing ordinary test names stay at12; the early-admission test adds only pure mode/family/hard-limit/ELF refusal assertions, not native CORE/credential/child operations. The existing native gate regression first runs the same production27-word ABI query/validator (27 foreign-C scalar constant reads), then its original three gate mappings. It performs no CORE, credential or child operation. The full source now compiles and these12 ordinary checks pass. Actual CORE/credential/image-policy/core-window/READY and privileged lifecycle behavior remain unexercised and require separate qualification. Rejected soft-only and hard-clamp proposals remain preserved privately.
