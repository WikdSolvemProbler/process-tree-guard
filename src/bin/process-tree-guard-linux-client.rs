// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Opt-in Cargo entry for the acknowledged separate-service client component.
#![allow(dead_code)]

#[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
#[path="../deadline.rs"]mod deadline;
#[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
#[path="../sys/linux/mod.rs"]mod linux;
#[cfg(all(target_os="linux",target_env="gnu",any(target_arch="aarch64",target_arch="x86_64")))]
#[link(name="ptg_linux_native",kind="static")]
unsafe extern "C" {}

fn main(){
 #[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]{
  let result=linux::run_cli(std::env::args_os().skip(1).collect());
  // Error detail is retained by ClientOutcome. The dedicated CLI uses a bounded
  // readiness-checked stderr write under its exit timer before _exit; no destructor/output tail is trusted.
  linux::client_exit(result);
 }
 #[cfg(not(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64"))))]
 std::process::exit(125);
}
