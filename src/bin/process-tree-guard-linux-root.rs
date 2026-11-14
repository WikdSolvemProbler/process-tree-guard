// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Opt-in Cargo entry for the acknowledged separate-service root component.
#![allow(dead_code)]

#[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
#[path="../deadline.rs"]mod deadline;
#[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
#[path="../sys/linux/mod.rs"]mod linux;
#[cfg(all(target_os="linux",target_env="gnu",any(target_arch="aarch64",target_arch="x86_64")))]
#[link(name="ptg_linux_native",kind="static")]
unsafe extern "C" {}

fn main(){
 #[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
 linux::root_service_main();
 #[cfg(not(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64"))))]
 std::process::exit(125);
}
