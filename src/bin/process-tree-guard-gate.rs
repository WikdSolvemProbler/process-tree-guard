// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! `process-tree-guard-gate`: the hook role. It takes the vendors and formats
//! of `process-tree-guard hook VENDOR`, with its own envelope set, automatic
//! copy and help handling. The engine is in the library crate.

fn main() {
    process_tree_guard::cli_exit(process_tree_guard::Role::Gate);
}
