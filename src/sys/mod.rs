// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! This module selects the backend; it declares nothing; a declaration found here is a defect by definition.

#[cfg(target_os = "windows")]
pub(crate) mod windows;

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu", ptg_linux_ordinary_build))]
#[path = "linux/ordinary.rs"]
pub mod ordinary_linux;

#[cfg(all(not(target_os = "windows"), not(ptg_linux_separate_service_build), not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu", ptg_linux_ordinary_build))))]
compile_error!("this build has no proven backend for its target; the proven backend at this version is Windows");
