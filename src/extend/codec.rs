// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The wire form of an extension exchange: the fixed-size request and reply messages of the channel.

use super::*;

pub(crate) enum Reply {
    /// `held` is the process the hold is on when it is not the one that asked (0 otherwise).
    Granted { seconds: f64, until_unix_ms: u64, held: u32 },
    Refused { refusal: Refusal, cap: f64 },
}

pub(super) fn word(bytes: &[u8], index: usize) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[index * 8..index * 8 + 8]);
    u64::from_le_bytes(raw)
}

/// Which hold of a process a request is for. A process has one hold for each token: the
/// calls of the library that overlap in one process (threads of a host) each have a token
/// of their own, so that one call's grant and release never touch another call's hold.
/// Token 0 is the hold of the process itself (the `extend` call, the `extend` program and
/// a guard program). A token is 56 bits: it shares a word of the request with the target.
pub(crate) type Token = u64;

pub(super) const TOKEN_BITS: u32 = 56;

/// A token no other call of this process has used: the next number, from 1.
pub(crate) fn next_token() -> Token {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::SeqCst) & ((1 << TOKEN_BITS) - 1)
}

pub(super) fn encode_request(target: Target, token: Token, seconds: f64) -> [u8; REQUEST_SIZE] {
    let mut out = [0u8; REQUEST_SIZE];
    let kind = target as u64 | ((token & ((1 << TOKEN_BITS) - 1)) << 8);
    for (index, value) in [REQUEST_MAGIC, kind, seconds.to_bits()].into_iter().enumerate() {
        out[index * 8..index * 8 + 8].copy_from_slice(&value.to_le_bytes());
    }
    out
}

pub(super) fn decode_request(bytes: &[u8]) -> Option<(Target, Token, f64)> {
    if bytes.len() != REQUEST_SIZE || word(bytes, 0) != REQUEST_MAGIC {
        return None;
    }
    let kind = word(bytes, 1);
    let target = match kind & 0xff {
        1 => Target::Caller,
        2 => Target::Parent,
        3 => Target::Release,
        _ => return None,
    };
    Some((target, kind >> 8, f64::from_bits(word(bytes, 2))))
}

pub(super) fn encode_reply(reply: &Reply) -> [u8; REPLY_SIZE] {
    let words = match reply {
        Reply::Granted { seconds, until_unix_ms, held } => [REPLY_MAGIC, 0, *held as u64, seconds.to_bits(), *until_unix_ms],
        Reply::Refused { refusal, cap } => [REPLY_MAGIC, 1, *refusal as u64, cap.to_bits(), 0],
    };
    let mut out = [0u8; REPLY_SIZE];
    for (index, value) in words.into_iter().enumerate() {
        out[index * 8..index * 8 + 8].copy_from_slice(&value.to_le_bytes());
    }
    out
}

/// The latest time a reply can name, the end of the year 9999, in milliseconds since 1970.
pub(super) const LATEST_UNTIL_MS: u64 = 253_402_300_799_999;

/// A reply as the client reads it. The client does not know who serves the channel, so it
/// accepts nothing out of range: the seconds (and the cap, in a refusal) are finite numbers
/// from 0 to `MAX_SECONDS`, and the time is one a clock can show.
pub(super) fn decode_reply(bytes: &[u8]) -> Option<Reply> {
    if bytes.len() != REPLY_SIZE || word(bytes, 0) != REPLY_MAGIC {
        return None;
    }
    let seconds = f64::from_bits(word(bytes, 3));
    if !(seconds.is_finite() && (0.0..=crate::MAX_SECONDS).contains(&seconds)) {
        return None;
    }
    match word(bytes, 1) {
        0 if word(bytes, 4) <= LATEST_UNTIL_MS => Some(Reply::Granted { seconds, until_unix_ms: word(bytes, 4), held: u32::try_from(word(bytes, 2)).ok()? }),
        1 => Some(Reply::Refused { refusal: Refusal::from_code(word(bytes, 2))?, cap: seconds }),
        _ => None,
    }
}

pub(super) fn not_understood() -> GuardError {
    GuardError("the extendable run sent a reply that is not understood".into())
}
