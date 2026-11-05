// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The core hook mechanism: whether a hook payload's command is exactly the guard envelope.
//!
//! This module holds the shared mechanism only: the streaming
//! scanner (`scanner`), the envelope check (`envelope`), the JSON
//! value tree (`json`), the hook's own path resolution (`paths`)
//! and the one output primitive (`say`). The vendor adapters and
//! the dispatch that selects among them live in the plugins
//! module at the crate root; every vendor family reads its
//! payload through this mechanism.

use crate::Role;
use std::io::Read;

/// The payload size limit every hook vendor reads under, in bytes.
const MAX_INPUT: usize = 16 * 1024 * 1024;

/// Print one decision line on standard output through its raw handle when
/// it is not a console (see `cli_process`); a failed write is ignored, as
/// the decision is carried by the exit code as much as by the text.
pub(crate) fn say(text: &str) {
    crate::cli_process::write_stdout_line(text);
}

mod envelope;
mod json;
mod paths;
mod scanner;

pub(crate) use envelope::*;
pub(crate) use json::*;
pub(crate) use paths::*;
pub(crate) use scanner::*;
