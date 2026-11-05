# Architecture
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page teaches the architecture of process-tree-guard: the shape of its source tree. The tree has three axes: the core, the `sys/` platform axis, and the `plugins/` integrations axis. A reader can learn the shape from this page before the source.

## Contents

- [The three axes](#the-three-axes)
- [The variance test](#the-variance-test)
- [The core](#the-core)
- [The platform axis](#the-platform-axis)
- [The integrations axis](#the-integrations-axis)
- [File existence rules](#file-existence-rules)
- [The tools fence and the proportion](#the-tools-fence-and-the-proportion)

## The three axes

The core holds the logic that varies with no platform and no host. The platform axis holds the declarations of one operating system. The integrations axis holds one adapter per vendor family. A file belongs to exactly one axis. A file earns its axis by the variance test in the next section.

[↑ Back to Contents](#contents)

## The variance test

Change one value of an axis. Ask one question. Does the change move code to a different file? A level passes only when the answer is yes. The code in that answer must stand today. A level that fails the test is a label. A label belongs in documentation, not in the folder tree.

[↑ Back to Contents](#contents)

## The core

The core holds the engine, the command line, extendable runs, the hook scan, and the approval surface. One file states one subject. The pure set stands in the core and in the integrations axis. The pure set is the set of files with no platform reach. [The Seam Criterion](The-Seam-Criterion.md) defines reach.

The generated map of the tree stands in [Dependency Graph](Dependency-Graph.md). That map gives the line count of each file.

[↑ Back to Contents](#contents)

## The platform axis

`src/sys/` holds one folder level for the operating system. `src/sys/mod.rs` selects the backend and refuses other targets. That module declares nothing else. `src/sys/windows.rs` is the one declaration module. `src/sys/windows/rng.rs` holds the one logic file of that leaf. [src/sys/linux/](../src/sys/linux/README.md) and [src/sys/macos/](../src/sys/macos/README.md) are named places with one README each.

No silicon level stands in the tree. No architecture level stands in the tree. Width changes no file placement today, so width fails the variance test.

[↑ Back to Contents](#contents)

## The integrations axis

`src/plugins/` is flat by vendor family. Three family files stand today. `verdict.rs` serves `pretooluse`, `cursor`, and `cursor-context`. `antigravity.rs` serves `antigravity` and `antigravity-context`. `powershell_auto.rs` serves `powershell-auto`. No host-kind level stands in the tree. Host kind predicts no code fact. Protocol kind predicts the code shape.

Wiring examples stand outside the source, at [integrations/](../integrations/README.md). Each vendor folder holds `hooks.json`, `hooks-gate.json`, and a README.

[↑ Back to Contents](#contents)

## File existence rules

A new file must pass these rules. F-R1: one file states one subject in one sentence with no "and". F-R2: the file name states that subject. F-R3: a file over the review line carries a one-subject defence. F-R4: no file carries a banned name. F-R5: no empty or whitespace-only source file stands. F-R6: a pure claim for a new file needs the revival run of [The Seam Criterion](The-Seam-Criterion.md). F-R7: a test stands with the code that the test covers, under the placement rule of the tree.

[↑ Back to Contents](#contents)

## The tools fence and the proportion

Product code stands in the tree. Development tools stand outside the product claim. The development tools do not ship in this product. [The Seam Criterion](The-Seam-Criterion.md) states how the recorded seam pin is checked again.

The generated map stands in [Dependency Graph](Dependency-Graph.md). That map shows each module and each edge.

[↑ Back to Contents](#contents)
