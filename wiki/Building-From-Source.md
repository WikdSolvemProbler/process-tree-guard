# Building From Source
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page guides building from source for process-tree-guard with the Microsoft Visual C++ (MSVC) target. The instructions use the Windows backend. Windows is the proven tier for these instructions. Execution evidence for the final version comes from the release test run.

## Contents

- [Prerequisites](#prerequisites)
- [Build](#build)
- [Version resource behaviour](#version-resource-behaviour)
- [The extra target checks](#the-extra-target-checks)
- [Place the programs](#place-the-programs)

## Prerequisites

Install a stable Rust toolchain for the MSVC target. The Rust version floor is 1.88. The edition is 2021. Install the MSVC build tools. The release build needs no resource compiler and no crates. The build script writes the version resource in code.

[↑ Back to Contents](#contents)

## Build

Run one command at the tree root:

```
cargo build --release
```

The four programs stand in `target\release\`. The names are `process-tree-guard.exe`, `process-tree-guard-deadline.exe`, `process-tree-guard-gate.exe`, and `process-tree-guard-contain.exe`. The test helper `testkit` is not part of a normal build. A test run builds `testkit` through the `testkit` feature.

[↑ Back to Contents](#contents)

## Version resource behaviour

Each program carries its own Windows version resource. The resource holds the description of the program. The resource holds the product name `Process Tree Guard`. The resource holds the file version and the product version. The resource holds the copyright notice and the original file name. The resource sets no company name.

The resource links only for the MSVC target. A GNU target build carries no resource. That target is not a supported target.

[↑ Back to Contents](#contents)

## The extra target checks

Check the extra Windows targets as types only. Add each target with `rustup target add` first:

```
cargo check --target aarch64-pc-windows-msvc
cargo check --target i686-pc-windows-msvc
cargo check --target x86_64-pc-windows-gnu
cargo check --release --target aarch64-pc-windows-msvc
```

These checks pass with zero warnings, in the debug profile and in the release profile. The checks also run from a Linux aarch64 host and from a macOS arm64 host. A pass proves the type-and-borrow tier only. A pass proves no execution. No extra target is a release target.

A check for a Linux target or a macOS target stops with the compile error `this build has no proven backend for its target`. [Platform Support](Platform-Support.md) states the reason.

[↑ Back to Contents](#contents)

## Place the programs

Place the built programs in one of the two layouts after the build. [Policy File and Layout](Policy-File-and-Layout.md) states both layouts. Copy the full program to its `-auto` name in the same folder.

[↑ Back to Contents](#contents)
