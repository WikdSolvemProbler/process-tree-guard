# Extendable Runs
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page teaches extendable runs in process-tree-guard. A hold is an approved extension of a deadline. A process in the run asks for a hold. The guard logs every request.

## Contents

- [Start an extendable run](#start-an-extendable-run)
- [Ask for a hold](#ask-for-a-hold)
- [The log](#the-log)
- [The cap and approval](#the-cap-and-approval)
- [Nested guards](#nested-guards)
- [The owner rule](#the-owner-rule)
- [When the run ends](#when-the-run-ends)

## Start an extendable run

Start an extendable run with `--extendable`:

```
process-tree-guard --extendable --timeout SECONDS COMMAND...
```

The guard serves a private named pipe for the run. The guard passes the pipe name to the command in `PROCESS_TREE_GUARD_CHANNEL`. A run without `--extendable` has no channel. A run without `--extendable` keeps a fixed deadline.

[↑ Back to Contents](#contents)

## Ask for a hold

A process in the job asks for a hold in two ways. The process calls the library function `extend`. Or the process runs `process-tree-guard extend SECONDS`. The guard verifies the process before the guard answers. The guard keeps the hold on the handle of the process. The hold ends when the process exits.

Each hold ends at most one cap ahead. The default cap is 21600 seconds. `--max-extension` sets the cap for the run. No cap exceeds 86400 seconds. A request after the deadline fails.

[↑ Back to Contents](#contents)

## The log

The guard logs every request, grant, and refusal. Name the audit file with `--extension-log PATH`. The guard grants nothing while the audit file cannot take lines. With no audit file, standard error is the record. A grant stands only after its line stands in the record. A run with a closed standard error does not start. A run with standard error set to the null device does not start.

[↑ Back to Contents](#contents)

## The cap and approval

A cap over 21600 is a per-run exception. The guard asks the signed-in user for approval before the run starts. The default method asks for Windows Hello consent. The fallback method asks with the credential dialog on the secure desktop. The method `--authenticate console` asks on a real console. A password check allows at most 3 attempts. Windows Hello is asked once.

A run that nobody can approve does not start. A run that gets no answer in 300 seconds does not start. Both cases exit with code 125. A nested guard never asks. The contain role never asks. Automatic mode never asks.

[↑ Back to Contents](#contents)

## Nested guards

A guard inside an extendable run asks that run for its own deadline at start. The run can refuse the request. A refusal ends the inner guard with exit code 125. A library call inside the run makes one hold for the calling process. The hold ends when the call returns. Each call has a hold of its own. Holds of one process do not replace holds of the same process.

One process gets at most 12 requests in any 10 seconds. A request over that rate fails. The command of that request does not run. The contain role runs a command as a child of the extendable run. [Programs and Roles](Programs-and-Roles.md) states the contain form.

[↑ Back to Contents](#contents)

## The owner rule

An owner is a process whose exit ends a run. The guard ends the tree when the owner exits. For a program run, the owner is the process that started the guard. For a library call, the owner is the parent of the calling process. The `run_auto` form watches every live ancestor.

A caller with no live parent cannot run a command to its end. The call returns exit code 125 in that case. Start the guard program as a child when the embedder must outlive the call. [Guarantees and Limits](Guarantees-and-Limits.md) states the owner promise.

[↑ Back to Contents](#contents)

## When the run ends

The run ends at the latest live hold. The run also ends at its own `--timeout` deadline when no hold stands later. A run ends with exit code 124 at that time. A process that renews in time keeps the run alive. Each renewal is capped. Each renewal stands in the log.

[↑ Back to Contents](#contents)
