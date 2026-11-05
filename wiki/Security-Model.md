# Security Model
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page summarizes the security model of process-tree-guard for a security reader. The repository file [SECURITY.md](../SECURITY.md) is the full text. This page guides that reader to the full text in one hop.

## Contents

- [Read the full text](#read-the-full-text)
- [What the guard hardens](#what-the-guard-hardens)
- [What the guard refuses to claim](#what-the-guard-refuses-to-claim)
- [Service mediated creation](#service-mediated-creation)
- [Prompt and approval surface](#prompt-and-approval-surface)

## Read the full text

Read [SECURITY.md](../SECURITY.md) in the repository for the full security policy. That file states the supported versions. That file states the private report route. Report a defect through the Security tab of the repo. Do not open a public issue for a defect of this kind.

[↑ Back to Contents](#contents)

## What the guard hardens

The guard hardens the run against escape from the job object. The guard hardens the hook envelope against a false pass. The guard hardens a hook denial against a stream error. The guard hardens its own files against a planted file in `PATH` or in the current folder. The program form hardens its own process against new handles that can suspend, change, or duplicate it.

[↑ Back to Contents](#contents)

## What the guard refuses to claim

The guard does not claim a boundary against a hostile process of the same user. The guard does not claim control of a service creation path. The guard does not claim resource bounds. [Guarantees and Limits](Guarantees-and-Limits.md) states each limit at full strength.

[↑ Back to Contents](#contents)

## Service mediated creation

A service creates a process outside the job. The job cannot hold that process. This fact is a limit of the mechanism, not a defect report route. A claim that a normal command escapes the job is a defect. A claim that a same-user hostile process uses a service path is known behaviour. `SECURITY.md` separates the two cases by name.

[↑ Back to Contents](#contents)

## Prompt and approval surface

A cap over the default needs a user approval. The guard asks only from a top-level program run. A nested guard never asks. A guard under another guard never asks. That rule keeps a command from summoning a prompt for its own run. [Extendable Runs](Extendable-Runs.md) states the approval rules and the refusal tests.

[↑ Back to Contents](#contents)
