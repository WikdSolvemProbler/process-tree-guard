// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! This module selects the backend; it declares nothing; a declaration found here is a defect by definition.

#[cfg(target_os = "windows")]
pub(crate) mod windows;

#[cfg(not(target_os = "windows"))]
compile_error!("this build has no proven backend for its target; the proven backend at this version is Windows");
