<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/sys/linux

## Contents

- [src/sys/linux](#srcsyslinux)

This folder is a named place for a Linux backend. It holds this README and nothing else.

**Belongs here:** This README only.

**Does not belong here:** Code of any kind. A file here that is not this README is a defect at this version.

Linux is a named place, not a backend. The revival condition stands on the wiki Platform Support page.

## Native containment probes

An Ubuntu 24.04.5 ARM64 guest running Linux 6.8 verified atomic placement with `clone3`, refusal of a protected worker's attempt to enter a sibling cgroup, refusal of outer cgroup namespace reentry, and protection against a switch to threaded mode.

A worker in the combined user and cgroup namespaces could still kill a disposable sibling through an inherited cgroup mount alias. The guest refused the attempt to make mount propagation private with `EACCES`, so the masked workload was withheld. A Linux backend needs a verified boundary for these controller paths before executing a command.

The archive preserves the C sources, guest commands, raw outcomes, failed first attempt, and cleanup checks. The probes ran in fresh delegated scopes with independent finite deadlines. [Native probe evidence](../../../tests/evidence/linux-cgroup-native-20261106.zip)

### Root-owned supervisor probes

In a separate system scope, a supervisor atomically placed the payload in a root-owned cgroup and executed it with the caller's UID, GID, and supplementary groups, with `NoNewPrivs` enabled. The payload was denied writes to its own and its sibling's cgroup controls, and the root-owned common ancestor blocked migration into a separately writable leaf.

A one-shot watchdog drained the payload cgroup on supervisor loss, owner loss, deadline expiry, and normal completion. Held pidfds verified the original payload and its detached, double-forked descendants. The sibling survived all four cases. The deadline and owner-loss cases returned 124 and 125, and normal completion preserved the fixture's exit 23.

The root-owned control and lifecycle archive contains the source, outcomes, and independent cleanup checks. [Root-owned control and lifecycle evidence](../../../tests/evidence/linux-root-supervision-20261106.zip)

A startup-fence prototype refused late launch requests after owner loss and deadline expiry. Killing the supervisor between atomic placement and release closed the exec-permit pipe and left the child reaped without an exec marker. Two permitted commands returned 7 and 197, and both statuses were forwarded exactly. [Startup fence evidence](../../../tests/evidence/linux-startup-fence-20261106.zip)

A Rust prototype recovered from watchdog death before the atomically placed child closed inherited descriptors, while the child waited for exec permission, and after exec. In the first case the watchdog pidfd became ready while the result pipe still had a writer. The supervisor drained the cgroup using its held process identity. A missing executable returned guard status 125 with an ENOENT receipt, while a real program's exit 127 passed through. Supervisor and owner loss also passed, and the independent sibling survived all seven cases. [Rust supervision evidence](../../../tests/evidence/linux-rust-supervision-20261106.zip)

A one-shot admission fixture used kernel credentials on a private Unix socket and a held pidfd for its direct child. The supervisor launched the command with the verified UID, GID, and groups and forwarded exit 127. Seven refusal cases left the exec marker and payload cgroup absent. The existing seven lifecycle cases still passed, and the independent sibling survived. [Admission proof and raw receipts](../../../tests/evidence/linux-admission-20261106.zip)

A follow-up fixture admitted an independently launched ordinary user through a bounded one-shot root rendezvous. The accepted socket credentials and per-packet kernel credentials matched the transferred pidfd. Seven malformed or mismatched requests left no payload leaf or exec marker. The valid fixture forwarded exit 127. Caller exit before release prevented exec, and caller exit after exec drained the payload cgroup. The independent sibling survived. [External admission proof and raw receipts](../../../tests/evidence/linux-external-admission-20261106.zip)

Both admission fixtures ran on Linux ARM64 6.8. The product backend still needs an installation and activation policy, general command authorization, qualification on the supported kernel floor, and concurrent lifecycle coverage.

A shared deadline-validation prototype passed focused checks on Windows x64, Linux ARM64, and native macOS ARM64. The recovered census classified its new module as pure after the current baseline pins were reconciled. The matched Windows release changed its mapped code section, and the historical generator still emits an obsolete explanation for `environment.rs`. The prototype remains outside production pending speed qualification and a refreshed design gate. [Source, native receipts, and raw censuses](../../../tests/evidence/deadline-policy-qualification-20261106.zip)

The smaller shared numeric deadline predicate now lives in `src/deadline.rs`. The matched Windows x64 MSVC release images have identical machine code and runtime data. Native ARM64 tests verified the predicate, and the product retained its intended unsupported-backend refusal on both Linux and macOS. Linux activation and containment remain subject to the backend acceptance requirements. [Exact source, build, and census evidence](../../../tests/evidence/deadline-numeric-integration-20261106.zip)

## Generic request prototype

The one-shot admission prototypes used a fixed command. A standalone Rust prototype now decodes a bounded, versioned timeout and argument packet. It preserves native non-UTF-8 bytes and empty arguments after the executable, refuses malformed framing and embedded NULs, and prepares owned C argument buffers. It reuses the production numeric deadline predicate and keeps positive durations from becoming a disarmed timer value.

The ordinary-user Linux ARM64 6.8 VM and Windows GNU host each passed seven focused tests. The prototype does not launch a payload. Socket integration, authentication with the new packet, caller environment and stdio, native containment, and kernel-floor and concurrency qualification remain open. [Prototype source, earlier compiler failure, and native evidence](../../../tests/evidence/linux-generic-request-20261106.zip)

A native transport fixture passed all eight tests over an anonymous SOCK_SEQPACKET pair in the Linux ARM64 6.8 VM. It matched kernel socket and message credentials to a held pidfd, verified close-on-exec, and accepted a packet exactly at the 128 KiB cap. Seven kernel-backed refusals covered payload and control truncation, missing and duplicate descriptors, a descriptor of the wrong type, a malformed frame, and peer EOF. The descriptor count returned to baseline in each refusal. This fixture used one ordinary process, so a separate client and privileged activation remain unqualified. [Transport source and native evidence](../../../tests/evidence/linux-request-transport-20261106.zip)

A separate native test connected distinct ordinary-user child processes to a private SOCK_SEQPACKET listener. It admitted the live client with matching socket credentials, packet credentials, and pidfd. It refused a mismatched pidfd, a queued request from an exited client, and a request sent over an inherited endpoint with different socket and packet identities. Each original child was reaped and each lane verified EOF and descriptor cleanup. This proves those four sequential admission cases. Integration with production activation and containment remains open. [Cross-process source and native evidence](../../../tests/evidence/linux-cross-peer-20261106.zip)

The release prototype kept the authenticated owner pidfd after transport EOF and checked it alongside an absolute CLOCK_BOOTTIME timer at the permit decision. The live control received one permit and wrote a marker. Owner exit and deadline expiry each closed the permit pipe without a marker, although both had passed the earlier check. All four prior admission cases passed in the same native run. The result covers the final release observation. Production still needs the watchdog to handle owner loss or expiry after that observation and the native payload containment path. [Release-check source, native receipts, and initial compile failure](../../../tests/evidence/linux-release-lease-20261106.zip)

A production-shaped preparation prototype now holds the decoded command, verified caller identity and groups, the original transferred pidfd, and the absolute release timer together. The Linux ARM64 VM passed both ordinary-user policy lanes, retaining the original argv and owner handle on allow and closing the handle on deny. The four admission and three release-decision controls also passed. The fixture ends before cgroup allocation and payload execution. [Prepared-run source and native receipts](../../../tests/evidence/linux-prepared-run-20261107.zip)

A standalone child launch plan passed two scripted tests on the Linux ARM64 VM. It refuses release on absent placement, owner loss, or deadline expiry. It checks that each failed setup stage stops the sequence, preserves the distinct exec error, and passes the intended credential and byte-argument inputs to the scripted child operations. This probe uses the PreparedRun fixture values with simulated placement and release. Native privilege changes, cgroup activation and payload execution remain outside this evidence. [Source and native scripted test receipts](../../../tests/evidence/linux-scripted-child-launch.zip)

Archive SHA-256 51095f8862bffad087ef183d515ea3eb8ff28b2b5bc2acce97306b09d6a3d3e1.

The authenticated launch bridge consumed the verified caller identity, original argument allocation, owner pidfd, and absolute lease. Five ordinary-user Linux ARM64 lanes exercised a live scripted sequence, refusal after a scripted UID failure, native owner loss, absent placement, and policy denial. The original argument allocation reached the scripted exec boundary. The four transport and three release controls also passed. Descriptor and scratch cleanup were verified. Placement and credential operations were simulated, and the fixture executed no workload. [Bridge source and native receipts](../../../tests/evidence/linux-prepared-child-bridge.zip)

Archive SHA-256 c7acd929cc66914d16ed3ba30d662b56b71178b8ca986cf1f413d5b2c6f79f70.

A native ARM64 Linux compiler check now covers the proposed activation adapter against the actual PreparedRun and lease types. The adapter carries the original owner pidfd, absolute timer, verified identity and argv through its cgroup handoff interface. Both ordinary and test builds passed warnings-denied object generation on the Tailscale VM. The earlier metadata checks also passed, producing empty binary-crate metadata outputs. No adapter entry point or payload ran. Native provisioning, watchdog activation, permit delivery, credential changes and cleanup settlement still require runtime integration. [Exact sources, compiler receipts and independent cleanup](../../../tests/evidence/linux-native-activation-seam.zip)

Archive SHA-256 a093c2af157eb2148c241adfff6d5be540b8a2097f37e8cdaf33a8f21f9b5a32.

An ordinary-user Linux ARM64 test exercised the authority candidate’s real openat, statx and fstatfs calls. It rejected mismatched inodes, symlinks and incorrect descriptor types, and refused provisioning before creation. The first native attempt exposed incorrect ARM64 open flags. Corrected source passed, and a C program using the guest headers independently matched the selected constants. The canonical installation-path correction passed the same test. The archive preserves the failed attempt, corrected source, successful receipts and independent cleanup checks. That read-only qualification did not exercise positive root-owned provisioning, installation provenance, generic placement or watchdog integration. [Authority source and raw evidence](../../../tests/evidence/linux-cgroup-authority.zip)

The older ARM64 Rust lifecycle fixtures used the incorrect open flags. Their saved outcomes remain evidence of those executions; they do not establish no-follow path opening. The corrected authority component has the read-only scope stated above. Bounded settlement must retain or resolve created-leaf ownership on every arming error and consume the armed run through verified drainage and exact reaping before production activation.

### Native empty-domain settlement

On the Linux ARM64 VM, a hash-verified root-owned binary exercised the actual factory and bounded settlement API. The sequential fixture exclusively created its fixed test root, provisioned one empty leaf, wrote through the retained kill descriptor, verified emptiness, revalidated the original identities and removed the exact leaf and root. Both finite system services reported success. Independent checks found the fixed root, private root scratch, service cgroups and selected workers absent. The ordinary handoff was archived before removal under its original UID. The fixture executed no workload and recorded zero poll wakeups, so populated polling, child reaping, watchdog behavior and full one-shot activation remain unqualified. [Source, native receipts and cleanup](../../../tests/evidence/linux-domain-settlement.zip)

Archive SHA-256 609e1812f40f0387680f2e8e7686ddb8f1262469e47581cf752dffb0d4a69b35.

### Termination-first settlement repair

The next prototype requests termination through its original held kill descriptor before clock reads, cleanup-timer setup or pathname revalidation. The shared driver owns the actual controls and returns that custody plus the write outcome on every returned error. Failed or short writes do not enter wait setup. Removal still requires the bounded timer, validated original identities, observed emptiness, exact unlink and absence confirmation. The receipt timestamp is explicitly after the kill write returns.

One ordinary-user Linux ARM64 attempt compiled the full module graph with warnings denied and passed four selected controls. The native control wrote through a held descriptor to its own ordinary file, then exercised real expired-deadline refusal while retaining the descriptor. The other three check the shared driver's ordering and outcomes. There was no cgroup write, payload or root fixture. Remote artifacts were preserved and verified before exact identity-checked cleanup; independent checks found the original scope, cgroup path and recorded PIDs absent. [Source, native controls and cleanup receipts](../../../tests/evidence/linux-domain-termination-first.zip)

Archive SHA-256 eda65f3210dbdc12a72b9e0f9ec4ca76ab10f94aaf348b674e446d9075e5df12.

A native write can still block, so this component requires independent watchdog coverage before payload activation. It does not settle created resources after every arming failure or consume `PendingSettlement` through exact child reaping. Populated drainage, actual cgroup failure recovery and the full one-shot port remain unqualified. The production Linux backend stays disabled.

### Typed arming-failure custody

The next adapter returns owning arming failures instead of erasing them into a string. Its variants retain the actual factory failure and optional created leaf, or verified controls with any direct watchdog child. Direct child records retain both PID and pidfd. Activation errors also retain the original prepared run, including its owner handle and absolute lease.

Both ordinary and test-object builds of the complete module graph passed on Linux ARM64 with warnings denied. The objects were not linked or executed, and no activation or test harness ran. Exact sources, compiler outputs and independently checked archive-before-removal cleanup are preserved. [Typed API and compiler evidence](../../../tests/evidence/linux-arming-custody.zip)

Archive SHA-256 c19869448c84258d15f47ebef214e38c6b57d1d1817d23e89f55eaad3d65a155.

This establishes compiler acceptance of the owning API. The native arming and clone contracts are not runtime-qualified, and dropping a returned owner does not guarantee settlement. Consuming arming failures and `PendingSettlement`, exact child reaping, watchdog integration and production activation remain open.

### Retaining the drained domain until final removal

The next component separates termination and drainage from removal. A successful drain now returns an owner containing every original control, the same armed cleanup deadline and its receipts. Returned failures also retain the armed deadline once it exists. Final removal consumes that owner, reuses the deadline and preserves partial removal state on error. Its private unsafe caller contract requires actual prior reaping and no future placement; that contract is not itself a reaping proof. The old combined settlement shortcut remains only in the empty test fixture.

Both ordinary and test-object builds of the complete graph passed on Linux ARM64 with warnings denied. No test harness or component was executed. Exact objects, source continuity and independent archive-before-cleanup receipts are preserved. [Owning drain/removal source and compiler evidence](../../../tests/evidence/linux-domain-drain-ownership.zip)

Archive SHA-256 9f7532dc455e73b8b82d0f7e73fcbbea0c5b4788613d9286f4a39e48ac7051f1.

Native pidfd reaping, subreaper ownership, exec-result observation, mandatory terminal orchestration and watchdog integration remain open. The production Linux backend remains disabled.

### Direct-child reaper ABI qualification

The next component performs one nonblocking wait through an owned child pidfd and decodes native status through a C bridge built against the installed Linux headers. Pending and error paths retain ownership; a validated exit receipt retains the original pidfd and raw status. A later clock failure preserves a status already consumed. This source contract still needs runtime qualification with genuine children.

One ordinary-user Linux ARM64 attempt passed the native constant/header reporter and warnings-denied ordinary and test-object builds of the complete Rust graph. The normalized C/Rust frame was 40 bytes with alignment 4; native siginfo_t was 128 bytes with alignment 8. Only the constant reporter executed. No child was constructed, no component wait call or Rust harness ran, and no root, cgroup or payload operation occurred. Installed header identities and hashes matched before and after compilation.

All 58 remote artifacts were preserved before exact identity-checked cleanup. Independent checks found the original scope, cgroup path and 11 recorded tool/controller PIDs absent. The archive retains source, installed-header provenance, objects, reporter, raw streams and cleanup receipts. [Source and limited native qualification](../../../tests/evidence/linux-direct-child-reaper-abi.zip)

Archive SHA-256 7f890abddee02d8a0c14e9f9b2fd0ac7456667926750dfba0172263f29d3b67a.

Actual normal exit, Pending, signal termination and repeated-wait behavior remain untested. Terminal orchestration, subreaper adoption, watchdog and exec-result handling, populated settlement and production activation remain open.

### Sequential reaper attempt stopped at linking

The next ordinary-user ARM64 attempt compiled the native C bridge/fixture and both Rust objects with warnings denied, and passed the constant ABI reporter. Linking the selected Rust test failed on an unresolved stack-protection symbol, with the dynamic loader reported as missing from the link. The test was never listed or executed; zero fixture children and zero component wait calls ran. The source, failed link argv and stderr, all 81 native artifacts and earlier unexecuted preparations are preserved.

Independent verification covered every archive member and confirmed exact scratch removal, original scope/cgroup absence and all 14 recorded PIDs absent. The link log suggests an ordering problem with appended C objects; that diagnosis and a normal static-library repair still need native validation. Compiler protections remain enabled, the failed attempt stays failed, and actual reaping remains unqualified. [Complete failed attempt and cleanup](../../../tests/evidence/linux-direct-child-reaper-sequential.zip)

Archive SHA-256 a2e04ba953e7974a23b44164bc07e6ce3236d90f6abdabe811811c30588c2b14 (153 verified members).

### Actual sequential child reaping and static-link repair

A fresh ordinary Linux ARM64 attempt linked the same C object bytes through Rust's normal native static-library interface, retaining the stack-protection reference and all compiler hardening. The link map identifies the dynamic loader as the provider for that reference. This repaired the earlier link-order failure without changing the reaper or fixture code. The failed attempt remains preserved.

The selected native test passed all three sequential cases: exit 23; a genuine Pending result retaining the original PID/pidfd followed by exit 29; and pidfd-directed SIGTERM reported as signal 15. Each validated reaped receipt retained its pidfd for a separate raw wait that returned ECHILD. All native status bytes were retained. An independent [offline audit](../../../tests/evidence/audit_reaper_static_link.py) decoded those bytes using the installed-header offsets, matched the normalized fields and terminal outcomes, verified every archive member, and checked the unchanged C objects and link map.

All 103 artifacts were preserved before exact removal. The original scratch, scope/cgroup and 26 recorded PIDs were independently absent. The archive also preserves the read-only inspection's user-bus refusal, corrected inspection, unexecuted parser revisions and generation-size refusal. [Complete native qualification and cleanup](../../../tests/evidence/linux-direct-child-reaper-static-link.zip)

Archive SHA-256 f546645adef6e0b470837f5a68a124cb073f5a92c7ec8b9626fac4eeb6d7ea34 (221 verified members).

This qualifies the ordinary direct-child reaper component and link repair. Timer-expiry, cleanup-failure/quarantine and post-reap clock-error branches remain unqualified at runtime. Populated cgroup drainage, original lease/owner and exec-report supervision, watchdog/adopted-child settlement and a consuming terminal controller still require integration. The production backend stays disabled; no guard performance result follows.

### Consuming terminal-controller source and object qualification

The next concrete controller retains the original absolute lease, prearmed cleanup authority, supervisor's own pidfd, owned exec-report reader and direct children. Stop events latch a held kill before later diagnostics; drained controls remain owned through direct and adopted reaping, with removal last. An interrupted active poll conservatively cancels before later clock/allocation work. These are reviewed source contracts, not exercised runtime guarantees.

One ordinary Linux ARM64 attempt compiled the installed-header C bridge and actual full-graph normal/test Rust objects with warnings denied. All six compiler steps passed, 125 header dependencies matched and all three relocatable objects were preserved. No link, compiled test/controller/reaper/reporter, child, root operation or cgroup write ran. All 49 artifacts were verified before exact scratch removal; the original scope/cgroup and seven recorded PIDs were absent. [Complete source and compiler evidence](../../../tests/evidence/linux-terminal-controller-objects.zip) and [independent offline audit](../../../tests/evidence/audit_terminal_controller_objects.py).

Archive SHA-256 0848ccef7232bb14df277ab4e0ad4a65a637f7b020af3eb1ff1ad3da64b7c4eb (133 members). Native arming/placement, partial-arm settlement, trusted bootstrap, product entry and all terminal runtime/failure paths remain unqualified. The production backend remains disabled; no guard performance claim follows.

### Consuming arming failures and partial creation custody

The next source consumes actual activation/arming failures. It retains the original request/lease, existing cleanup timer, context, created-leaf controls and optional watchdog. Verified domains use the real held-kill-first drain/reap/remove path. A never-activated leaf is removed only after original identity/empty-domain and no-child context proof, with removal last. Ambiguous or refused creation retains the attempted held parent/name and raw outcome without assuming deletion authority. No release, signal or child exit receipt is invented.

One ordinary ARM64 attempt compiled both full-graph normal/test Rust objects with warnings denied. The Rust version query and two object builds all succeeded; no C compile, linking, compiled test/controller, child, root or cgroup operation occurred. Prior unchanged C/ABI evidence stays historical. All 37 artifacts were preserved before exact removal, and the original scope/cgroup plus four recorded PIDs were absent. [Source, objects and raw cleanup evidence](../../../tests/evidence/linux-arming-failure-controller-objects.zip) and [independent offline audit](../../../tests/evidence/audit_terminal_controller_objects.py).

Archive SHA-256 cdb04781716f2ae29b2cf5a21fa83f949ded244af3b409008abbbb7d7a56e1e0 (103 members). Native arming/placement, independent setup supervision, runtime/failure-path qualification and product integration remain open; the production backend remains disabled.

**Next:** The wiki Platform Support page states the tier of each named place.

[↑ Back to Contents](#contents)
