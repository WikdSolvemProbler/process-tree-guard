# Guarantees and Limits
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page states the guarantees and limits of process-tree-guard. It states the guarantees at full strength. It states the limits at the same strength. Every guarantee names its backend tier. The proven tier in this page is the Windows backend tier.

## Contents

- [The promise](#the-promise)
- [Born into containment](#born-into-containment)
- [Kill on close and no breakaway](#kill-on-close-and-no-breakaway)
- [Pass through](#pass-through)
- [Time only, no resource caps](#time-only-no-resource-caps)
- [Same user boundary](#same-user-boundary)
- [Remote and container boundary](#remote-and-container-boundary)
- [Scope of a run](#scope-of-a-run)

## The promise

The guard runs one command under a finite deadline. The guard ends the whole process tree at the deadline. A successful result requires confirmed cleanup; uncertainty is reported as a guard error, not a successful run. Tier: Windows, proven backend. Execution evidence for the final version comes from the release test run.

[↑ Back to Contents](#contents)

## Born into containment

The guard creates the command inside the job object at creation. The guard uses `PROC_THREAD_ATTRIBUTE_JOB_LIST` for that placement. The guard creates the command in a suspended state. The guard resumes the command after placement. No gap stands between creation and placement. Tier: Windows, proven backend.

A system that cannot place a process at creation gets no backend. [Platform Support](Platform-Support.md) states that refusal.

[↑ Back to Contents](#contents)

## Kill on close and no breakaway

The job object carries the kill on close flag. The close of the last job handle ends every process in the job. The job refuses breakaway. A child that asks to leave the job gets a refusal. Tier: Windows, proven backend.

The guard uses one bounded 5-second cleanup budget to confirm that the job is empty and its original child has exited. An unconfirmed Job, failed or skipped retained-member wait, or unconfirmed original-child wait is a guard error (exit 125), even if the child returned zero or the deadline was reached. Kill on close remains the fallback; a termination request alone is not reported as confirmed completion. Windows termination can depend on outstanding I/O finishing or being canceled, so a guard error does not certify that every process has exited.

[↑ Back to Contents](#contents)

## Pass through

The guard passes standard input, standard output, and standard error through unchanged. The guard passes the exit code of the command through unchanged. The owner rule ends the run early when the owner exits. That end gives exit code 125. The deadline end gives exit code 124. Tier: Windows, proven backend.

[↑ Back to Contents](#contents)

## Time only, no resource caps

The guard bounds time only. The job sets no cap on process count. The job sets no cap on memory use. The job sets no cap on central processing unit (CPU) rate. A cap of that kind can end correct work. A cap end and a deadline end look the same in the exit surface. A missed containment costs seconds, because the deadline still ends the tree. A false end costs correct work. That price split is the reason for this limit.

[↑ Back to Contents](#contents)

## Same user boundary

Containment is a process lifetime guard. Containment is not a full boundary against a hostile process of the same user. A service can create a process outside the job. Windows Management Instrumentation (WMI), Task Scheduler, and a Component Object Model (COM) server are examples of such a service. A process with a powerful handle from before the run stands outside this protection. A process with administrator rights stands outside this protection. [Security Model](Security-Model.md) and [SECURITY.md](../SECURITY.md) state the full boundary.

[↑ Back to Contents](#contents)

## Remote and container boundary

The guard ends the local process that starts remote work. The guard does not end processes on a remote machine. The guard does not end processes inside a container. The guard does not end processes inside Windows Subsystem for Linux. Each such place needs a guard inside that place.

[↑ Back to Contents](#contents)

## Scope of a run

A run without `--extendable` ends by its fixed deadline. That deadline is at most 86400 seconds. Long services stand outside that form by design. An extendable run bounds how far ahead its deadline stands at each moment. An extendable run has no bound on total life when its processes renew in time. Each renewal is capped and stands in the log. Commands that never pass through the guard stand outside the guard.

[↑ Back to Contents](#contents)
