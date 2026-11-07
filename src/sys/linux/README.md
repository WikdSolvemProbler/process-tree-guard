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

**Next:** The wiki Platform Support page states the tier of each named place.

[↑ Back to Contents](#contents)
