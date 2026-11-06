# The Seam Criterion
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page states the seam criterion of process-tree-guard in full. The criterion draws one line. Code on one side of the line reaches a platform mechanism. Code on the other side reaches no platform mechanism.

## Contents

- [The first principle](#the-first-principle)
- [Terms](#terms)
- [Mint and census](#mint-and-census)
- [The pure set and the pinned exception](#the-pure-set-and-the-pinned-exception)
- [The two anchors](#the-two-anchors)
- [Payload steps and the ring](#payload-steps-and-the-ring)
- [Recorded limits](#recorded-limits)
- [What the shipped gate holds and does not hold](#what-the-shipped-gate-holds-and-does-not-hold)
- [Judged pairs and census results](#judged-pairs-and-census-results)

## The first principle

A platform mechanism is a capability that only one operating system provides. State a platform mechanism in the terms of that system. A job object is a platform mechanism. A body of code depends on the platform at the places where the code uses a platform mechanism. Every definition in this page comes from that principle.

[↑ Back to Contents](#contents)

## Terms

A declaration is a name that the platform module states for a platform mechanism or its data. Declarations state. Declarations do not decide. `sys` is the one folder that holds the backends. A backend module is the one module that holds the declarations of one operating system.

Reach is a fact about code. A file reaches a platform mechanism when its code uses a declaration. A file also reaches a platform mechanism in one more case. Its code uses a thing. That thing itself uses a declaration in one step. A file that talks about a mechanism does not reach that mechanism by talk alone.

A wrapper is a function or a method outside the backend module that itself reaches a platform mechanism. One step of reach makes a wrapper. Carriage is a wrapper inside a value from the construction of that value. A call through the held position is a use of the wrapper.

[↑ Back to Contents](#contents)

## Mint and census

The mint is the closed set of names from which reach is judged. The mint holds the declarations of the backend. The mint also holds every name that comes from one of those names. A name can come by alias, by initialiser, or by re-export. Follow that derivation to the point where no new name joins. A name outside every such chain stands outside the mint.

The census is the counting that tests reach. For each production file, the census counts the ways the code of that file can reach a platform mechanism. The channels stand in a list. One channel is a backend reference. One channel is a wrapper use or a wrapper-type naming. One channel is a derived name. One channel is a platform or architecture path in the standard library. One channel is an external declaration of the file itself. One channel is a refusal for a channel that no counting can see.

[↑ Back to Contents](#contents)

## The pure set and the pinned exception

A pure file is a production file with a census count of zero on every channel. The earlier census pinned 12 files, including one exception for the Windows UTF-16 encoding in `src/engine/environment.rs`.

The environment builder now calls the encoder in `src/sys/windows/encoding.rs`. The earlier pin records its own revision. A fresh graph census is pending for the changed boundary.

[↑ Back to Contents](#contents)

## The two anchors

Anchor A1 is the professor test. Ask whether a professor, shown the shape alone, can call that shape perfect. The shape is the object of the test. Term readability is the instrument of the test, not a substitute for the test.

Anchor A2 is universality, answered by refusal. The guard does not move to a system that cannot keep its promise. The seam stands so that a reader can prove that refusal file by file. [Platform Support](Platform-Support.md) states the refusal in product language.

[↑ Back to Contents](#contents)

## Payload steps and the ring

A payload step is a step on a `Result` value or an `Option` value. The step passes the value inside the container onward. The step can pass that value out itself, by reference, as a copy, or under the sibling container. A step that acts on the container and passes nothing is not a payload step. The ring is the closed list of payload steps that the census reads. The ring is closed under one condition. Every stable step of the two container surfaces stands in the list. Or a step stands outside the list under a named exclusion with its reason.

[↑ Back to Contents](#contents)

## Recorded limits

The census does not count a receiver call whose type arrives through a generic parameter. The census does not count a receiver call whose type arrives through a trait object. The census does not count a bare name with more than one production definition. The re-export chase has a bound of 8 hops and 16 module-path variants. A chain over a bound fails closed by name. The macro channel stands in refusal form. Each limit stands here as a present fact about the breadth of the computation.

[↑ Back to Contents](#contents)

## What the shipped gate holds and does not hold

The shipped gate holds pin-match. The gate checks that the pinned files match the recorded pin. The gate checks that each production import list equals its pinned list. The gate does not recompute derivation. A change in an intermediate file can change derivation with no list change. Such a change is an initialiser change, an alias change, or a re-export change. The shipped gate cannot see that class of change. That loss stands by name as L-T1.

Revival has one form. Restore the full census computation from its recorded commit. Use the commit that the graph records: `d7d64f6516df9f6df98fdfe9b531e2799ec5ae85`, the head of the `ptg-gauntlet-instruments` branch. That branch is kept outside this repository. Run that computation against the current tree. Revival is mandatory before a change that reopens the shape. Revival is also mandatory before a pure claim for a file outside the recorded pin.

The numeric deadline extraction revived the exact historical census against a reconciled current baseline and the candidate. Its only added pure member was `src/deadline.rs`. The evidence preserves the raw graphs and a separate prose-only rendering that distinguishes current derivation from the historical gate. The older D43 occurrence pins still require reconciliation. [Current census and exact source pins](../tests/evidence/deadline-numeric-integration-20261106.zip)

[↑ Back to Contents](#contents)

## Judged pairs and census results

The D45 judged pairs stand in summary here: 77 (struct, field) pairs. Open the dropdown for the full list. The impl-census totals stand at their stated breadth: Option 30 and Result 16 in all. The `IsZero`-for-Option count is 5 headers over 29 implementations. By tree, core holds Option 24 and Result 13 over 288 files. The `alloc` tree holds Option 5 and Result 0 over 93 files. The `std` tree holds Option 1 and Result 3 over 646 files.

<details>
<summary>The full D45 judged-pairs list stands in this dropdown.</summary>

- `AssociateCompletionPort.key`
- `AssociateCompletionPort.port`
- `BasicLimitInformation.affinity`
- `BasicLimitInformation.maximum_working_set_size`
- `BasicLimitInformation.minimum_working_set_size`
- `Com.0`
- `ConsoleApi.expunge`
- `ConsoleApi.handler`
- `ConsoleApi.prompt`
- `CredUiInfoW.banner`
- `CredUiInfoW.caption`
- `CredUiInfoW.message`
- `CredUiInfoW.parent`
- `DesktopApi.free`
- `DesktopApi.prompt`
- `DesktopApi.protected`
- `DesktopApi.unpack`
- `DesktopApi.unprotect`
- `ExtendedLimitInformation.job_memory_limit`
- `ExtendedLimitInformation.peak_job_memory_used`
- `ExtendedLimitInformation.peak_process_memory_used`
- `ExtendedLimitInformation.process_memory_limit`
- `HString.delete`
- `HString.raw`
- `HelloApi.create`
- `HelloApi.delete`
- `HelloApi.factory`
- `HelloApi.initialize`
- `HelloApi.uninitialize`
- `LogonApi.logon`
- `LogonApi.lookup`
- `Msg.l`
- `Msg.w`
- `Msg.window`
- `NameApi.name`
- `Overlapped.event`
- `Overlapped.internal`
- `Overlapped.internal_high`
- `ProcessBasicInformation.affinity`
- `ProcessBasicInformation.parent_process_id`
- `ProcessBasicInformation.peb`
- `ProcessBasicInformation.unique_process_id`
- `ProcessEntry32W.default_heap_id`
- `ProcessInformation.process`
- `ProcessInformation.thread`
- `SecurityAttributes.descriptor`
- `StartupInfoExW.attribute_list`
- `StartupInfoW.desktop`
- `StartupInfoW.reserved`
- `StartupInfoW.reserved2`
- `StartupInfoW.std_error`
- `StartupInfoW.std_input`
- `StartupInfoW.std_output`
- `StartupInfoW.title`
- `WindowApi.create`
- `WindowApi.default_procedure`
- `WindowApi.destroy`
- `WindowApi.dispatch`
- `WindowApi.peek`
- `WindowApi.register`
- `WindowApi.unregister`
- `WindowApi.wait`
- `WindowStationApi.close_desktop`
- `WindowStationApi.information`
- `WindowStationApi.input_desktop`
- `WindowStationApi.station`
- `WindowStationApi.thread_desktop`
- `WndClassExW.background`
- `WndClassExW.class_name`
- `WndClassExW.cursor`
- `WndClassExW.icon`
- `WndClassExW.icon_small`
- `WndClassExW.instance`
- `WndClassExW.menu`
- `WndClassExW.procedure`
- `WtsApi.free`
- `WtsApi.query`

</details>

[↑ Back to Contents](#contents)
