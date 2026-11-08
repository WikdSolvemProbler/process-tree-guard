// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller
//! Staged standalone Linux client; the default cross-platform selector is unchanged.
#[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
#[path="../../deadline.rs"]mod deadline;
#[cfg(all(target_os="linux",any(target_arch="aarch64",target_arch="x86_64")))]
#[path="mod.rs"]mod linux;
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
