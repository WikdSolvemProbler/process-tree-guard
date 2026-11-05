<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src

## Contents

- [src](#src)

This folder holds the product source of the guard. The library, the four programs, and every mechanism live here.

**Belongs here:** Production Rust modules, each with one subject.

**Does not belong here:** Tests that drive the built programs. Those tests live in `tests/`.

`lib.rs` states the public library surface and the short seam criterion statement.

`dispatch.rs` holds the role dispatch: the one place that knows all four roles.

**Next:** The [sys folder](sys/README.md) states the platform axis. The [plugins folder](plugins/README.md) states the integrations axis.

[↑ Back to Contents](#contents)
