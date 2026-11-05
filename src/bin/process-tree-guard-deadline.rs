// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! `process-tree-guard-deadline`: the deadline role. It is `process-tree-guard`
//! with `--timeout` (and `--extendable`), `extend` or automatic mode, and no
//! hook mode. The engine is in the library crate.

fn main() {
    process_tree_guard::cli_exit(process_tree_guard::Role::Deadline);
}
