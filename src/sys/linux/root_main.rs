// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Dedicated installed root service binary; no CLI payload/installation role.
#[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
#[path="../../deadline.rs"]mod deadline;
#[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
#[path="mod.rs"]mod linux;
fn main(){
 #[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
 linux::root_service_main();
 #[cfg(not(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64"))))]
 std::process::exit(125);
}
