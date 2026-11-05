// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The test seam of the approval: the prompt and verifier a unit test substitutes, and the answer it gives.

use super::*;
use std::cell::Cell;

thread_local! {
    /// Some(true): the right password is given. Some(false): the prompt is cancelled. None: the real prompt.
    pub(crate) static ANSWER: Cell<Option<bool>> = const { Cell::new(None) };
}

pub(super) struct SeamPrompt(pub bool);

impl Prompt for SeamPrompt {
    fn begin(&self, _text: &str) -> Result<Claim, String> {
        Ok(Claim { _lock: None })
    }

    fn ask(&self, _attempt: u32, _last_error: u32, _text: &str) -> Asked {
        if self.0 { Asked::Entered(Box::new(Credentials::with("me", "", "right"))) } else { Asked::Cancelled }
    }
}

pub(super) struct SeamVerifier;

impl Verifier for SeamVerifier {
    fn verify(&self, credentials: Box<Credentials>) -> Verified {
        if credentials.password_units() == "right".encode_utf16().collect::<Vec<u16>>() { Verified::Approved } else { Verified::Wrong }
    }
}
