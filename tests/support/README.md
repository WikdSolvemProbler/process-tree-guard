<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# tests/support

## Contents

- [tests/support](#testssupport)

This folder holds the test helper program and its support code. The helper stands in for every child process the tests need.

**Belongs here:** The `testkit` helper source.

**Does not belong here:** Product code. The helper is built only for a test run and never ships as a program.

Cargo builds the helper only when a test run turns its feature on. `cargo build` and `cargo install` never build it.

**Next:** The [tests README](../README.md) states the test layout.

[↑ Back to Contents](#contents)
