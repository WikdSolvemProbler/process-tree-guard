# Home
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page is the door to the process-tree-guard wiki. The guard runs one command under a finite deadline. The guard ends the whole process tree at the deadline.

## Contents

- [What the guard is](#what-the-guard-is)
- [Who the guard is for](#who-the-guard-is-for)
- [A first run in one line](#a-first-run-in-one-line)
- [Backend tiers](#backend-tiers)
- [Map of the wiki](#map-of-the-wiki)
- [Repository files](#repository-files)
- [Sponsors](#sponsors)
- [Link colors](#link-colors)

## What the guard is

The guard is a universal process-tree guard. A process tree is a process and every process that starts from it. The guard holds one run inside one job object on the proven backend. The job object ends every process in the tree when the job closes.

The guard is a library and four programs. The current version is 0.5.0. The guard has no third-party dependencies. The license is the Apache License, Version 2.0.

[↑ Back to Contents](#contents)

## Who the guard is for

Use the guard when a command can outlive its purpose. A test run can hang. A server can stay in the background. A child process can stay after its owner ends.

An artificial intelligence (AI) coding agent starts many such commands. A Continuous Integration (CI) job starts many such commands. The guard gives each command a deadline that ends the whole tree.

Start with [Getting Started](Getting-Started.md) for a first run. Start with [Hook Integrations](Hook-Integrations.md) to wire a host.

[↑ Back to Contents](#contents)

## A first run in one line

```
process-tree-guard --timeout 60 -- my-command arg1 arg2
```

The command runs for at most 60 seconds. Exit code 124 means the deadline ended the tree. Exit code 125 means a guard error, or the owner exited first. Any other code is the code of the command. [Command Reference](Command-Reference.md) lists every form.

[↑ Back to Contents](#contents)

## Backend tiers

One word, tier, names the evidence level of a claim. This table states every backend tier in one place.

| Backend | Tier |
|---|---|
| Windows | Proven backend. The guard builds and runs on Windows. Execution evidence for the final version comes from the release test run. |
| Linux | Theory. No Linux backend exists. A build for a Linux target is refused. The test harness logic has Linux proof. That proof is not backend proof. |
| macOS | Theory. No macOS backend exists. A build for a macOS target is refused. No Mac execution path exists today. |

[Platform Support](Platform-Support.md) owns the full tier discipline.

[↑ Back to Contents](#contents)

## Map of the wiki

| Group | Page | What the page answers |
|---|---|---|
| Use | [Getting Started](Getting-Started.md) | How do I get, verify and run the guard? |
| Use | [Programs and Roles](Programs-and-Roles.md) | Which of the four programs do I install? |
| Use | [Command Reference](Command-Reference.md) | What does every form, option and exit code mean? |
| Use | [Extendable Runs](Extendable-Runs.md) | How does work outlive one deadline under a cap? |
| Use | [Hook Integrations](Hook-Integrations.md) | How do I wire an agent host to the guard? |
| Use | [Policy File and Layout](Policy-File-and-Layout.md) | Where do the programs and the policy file stand? |
| Understand | [Guarantees and Limits](Guarantees-and-Limits.md) | What does the guard promise, and what does it not? |
| Understand | [Security Model](Security-Model.md) | What does the guard harden, and how do I report a defect? |
| Understand | [Platform Support](Platform-Support.md) | Which platform has which evidence tier? |
| Understand | [Architecture](Architecture.md) | What is the shape of the source tree? |
| Understand | [Dependency Graph](Dependency-Graph.md) | Which module uses which module? |
| Understand | [The Seam Criterion](The-Seam-Criterion.md) | Which files reach the platform, and why? |
| Understand | [Decision Records](Decision-Records.md) | Why is each design choice the way it is? |
| Understand | [Glossary](Glossary.md) | What does each term mean? |
| Prove | [Testing and Benchmark](Testing-and-Benchmark.md) | How do I run the suite and read a result? |
| Prove | [Measurement](Measurement.md) | What does each path cost, in counted calls? |
| Build | [Building From Source](Building-From-Source.md) | How do I build and check the programs? |

[↑ Back to Contents](#contents)

## Repository files

These files stand in the repository, outside the wiki:

- [README.md](../README.md): the front page of the project.
- [SECURITY.md](../SECURITY.md): the full security policy and the private report route.
- [POLICY.md](../POLICY.md): the policy file that the programs install and print.
- [CONTRIBUTING.md](../CONTRIBUTING.md): how to contribute.
- [CHANGELOG.md](../CHANGELOG.md): the record of each version.
- [MEASUREMENT.md](../MEASUREMENT.md): the full measurement record.
- [integrations/](../integrations/README.md): one wiring example per hook vendor.

[↑ Back to Contents](#contents)

## Sponsors

process-tree-guard was built at the Meta Global AI Developer Hackathon, sponsored by [Cerebral Valley](https://cerebralvalley.ai), [Meta](https://www.meta.com), [Baseten](https://www.baseten.co), [Fireworks AI](https://fireworks.ai) and [Modal](https://modal.com). We thank them for their support.

[↑ Back to Contents](#contents)

## Link colors

Internal links go to a section on the same page. External links go to a different page or file. The convention states internal links in red and external links in blue. The rule is `a[href^="#"] { color: #C0392B; }` for internal links. The rule is `a:not([href^="#"]) { color: #1A5276; }` for external links. GitHub removes inline styles on its pages. The convention therefore does not apply on github.com today. The set states the convention honestly and does not fake it.

[↑ Back to Contents](#contents)
