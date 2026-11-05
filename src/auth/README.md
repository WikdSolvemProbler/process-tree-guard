<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/auth

## Contents

- [src/auth](#srcauth)

This folder holds the parts of the approval module that stand as their own files. The module guards each hold that passes the default cap.

**Belongs here:** The consent surfaces, the refusal form, the names, the secrets handling, and the verify step.

**Does not belong here:** Deadline rules. The engine owns the deadline. Approval only guards the cap.

`tests.rs` holds the module tests for this group. The tests sit with the module they test.

**Next:** The [hello folder](hello/README.md) states the Windows Hello part.

[↑ Back to Contents](#contents)
