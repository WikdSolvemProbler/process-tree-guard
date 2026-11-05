// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! `ptg`: the short name of the full role, the same program as `process-tree-guard`.
//! The engine is in the library crate.

fn main() {
    process_tree_guard::cli_exit(process_tree_guard::Role::Full);
}
