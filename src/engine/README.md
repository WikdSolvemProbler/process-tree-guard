<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/engine

## Contents

- [src/engine](#srcengine)

This folder holds the parts of the engine module that stand as their own files. The engine runs the command and keeps the deadline.

**Belongs here:** The owner rule, the wait, the environment sort, the name resolution, and the trace.

**Does not belong here:** Hook rules and extension rules. Each has its own module.

`environment.rs` is a pure file with one pinned exception. The criterion statement in `src/lib.rs` names the exception.

**Next:** The [src README](../README.md) states the source layout.

[↑ Back to Contents](#contents)
