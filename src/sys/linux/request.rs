// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Owned command packet for the Linux one-shot admission boundary.

use crate::deadline;

use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::time::Duration;

const MAGIC: &[u8; 6] = b"PTGEX2";
const HEADER_SIZE: usize = 16;
pub const MAX_PACKET_BYTES: usize = 128 * 1024;
const MAX_ARGS: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    PacketSize,
    Header,
    Version,
    Timeout,
    ArgumentCount,
    ArgumentLength,
    EmptyProgram,
    ArgumentNul,
    TrailingBytes,
}

/// The argument strings own their buffers for the lifetime of the pointer array.
/// Moving the plan does not move any of those buffers.
#[derive(Debug)]
pub struct ExecPlan {
    duration: Duration,
    arguments: Vec<CString>,
    pointers: Vec<*const c_char>,
}

impl ExecPlan {
    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn program(&self) -> &CStr {
        &self.arguments[0]
    }

    pub fn arguments(&self) -> &[CString] {
        &self.arguments
    }

    /// The final pointer is null. Borrow the plan until a prospective exec call returns.
    pub fn argv(&self) -> &[*const c_char] {
        &self.pointers
    }
}

fn duration(seconds: f64) -> Result<Duration, Refusal> {
    if deadline::invalid_seconds(seconds) {
        return Err(Refusal::Timeout);
    }
    // A positive timeout must never turn into an all-zero timerfd setting.
    Ok(Duration::try_from_secs_f64(seconds)
        .map_err(|_| Refusal::Timeout)?
        .max(Duration::from_nanos(1)))
}

pub fn encode(seconds: f64, arguments: &[&[u8]]) -> Result<Vec<u8>, Refusal> {
    duration(seconds)?;
    if arguments.is_empty() || arguments.len() > MAX_ARGS {
        return Err(Refusal::ArgumentCount);
    }
    if arguments[0].is_empty() {
        return Err(Refusal::EmptyProgram);
    }
    let mut size = HEADER_SIZE;
    for argument in arguments {
        size = size
            .checked_add(4)
            .and_then(|bytes| bytes.checked_add(argument.len()))
            .filter(|bytes| *bytes <= MAX_PACKET_BYTES)
            .ok_or(Refusal::PacketSize)?;
        if argument.contains(&0) {
            return Err(Refusal::ArgumentNul);
        }
    }

    let mut packet = Vec::with_capacity(size);
    packet.extend_from_slice(MAGIC);
    packet.extend_from_slice(&seconds.to_le_bytes());
    packet.extend_from_slice(&(arguments.len() as u16).to_le_bytes());
    for argument in arguments {
        packet.extend_from_slice(&(argument.len() as u32).to_le_bytes());
        packet.extend_from_slice(argument);
    }
    Ok(packet)
}

pub fn decode(packet: &[u8]) -> Result<ExecPlan, Refusal> {
    if packet.len() > MAX_PACKET_BYTES {
        return Err(Refusal::PacketSize);
    }
    let (header, mut rest) = packet
        .split_first_chunk::<HEADER_SIZE>()
        .ok_or(Refusal::Header)?;
    let (magic, header) = header.split_first_chunk::<6>().ok_or(Refusal::Header)?;
    if magic != MAGIC {
        return Err(Refusal::Version);
    }
    let (seconds, count) = header.split_first_chunk::<8>().ok_or(Refusal::Header)?;
    let duration = duration(f64::from_le_bytes(*seconds))?;
    let count = usize::from(u16::from_le_bytes(
        count.try_into().map_err(|_| Refusal::Header)?,
    ));
    if count == 0 || count > MAX_ARGS || count > rest.len() / 4 {
        return Err(Refusal::ArgumentCount);
    }

    let mut arguments = Vec::with_capacity(count);
    for index in 0..count {
        let (length, tail) = rest
            .split_first_chunk::<4>()
            .ok_or(Refusal::ArgumentLength)?;
        let length =
            usize::try_from(u32::from_le_bytes(*length)).map_err(|_| Refusal::ArgumentLength)?;
        let (argument, tail) = tail
            .split_at_checked(length)
            .ok_or(Refusal::ArgumentLength)?;
        if index == 0 && argument.is_empty() {
            return Err(Refusal::EmptyProgram);
        }
        arguments.push(CString::new(argument).map_err(|_| Refusal::ArgumentNul)?);
        rest = tail;
    }
    if !rest.is_empty() {
        return Err(Refusal::TrailingBytes);
    }

    let mut pointers = Vec::with_capacity(count + 1);
    pointers.extend(arguments.iter().map(|argument| argument.as_ptr()));
    pointers.push(ptr::null());
    Ok(ExecPlan {
        duration,
        arguments,
        pointers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_native_bytes_empty_arguments_and_null_terminated_argv() {
        let arguments: &[&[u8]] = &[b"/tmp/command \xff", b"", b"a b", b"\x80\xfe", b""];
        let packet = encode(2.25, arguments).unwrap();
        let plan = decode(&packet).unwrap();
        drop(packet);
        assert_eq!(plan.duration(), Duration::from_millis(2250));
        assert_eq!(plan.program().to_bytes(), arguments[0]);
        assert_eq!(
            plan.arguments()
                .iter()
                .map(|arg| arg.to_bytes())
                .collect::<Vec<_>>(),
            arguments
        );
        let pointers = plan
            .arguments()
            .iter()
            .map(|arg| arg.as_ptr())
            .chain(std::iter::once(ptr::null()))
            .collect::<Vec<_>>();
        assert_eq!(plan.argv(), pointers);
    }

    #[test]
    fn matches_numeric_policy_without_disarming_the_timer() {
        for (seconds, nanos) in [
            (f64::from_bits(1), 1),
            (f64::MIN_POSITIVE, 1),
            (0.25e-9, 1),
            (0.5, 500_000_000),
            (deadline::MAX_SECONDS, 86_400_000_000_000),
        ] {
            let plan = decode(&encode(seconds, &[b"fixture"]).unwrap()).unwrap();
            assert_eq!(plan.duration(), Duration::from_nanos(nanos));
        }

        for seconds in [
            0.0,
            -0.0,
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::from_bits(deadline::MAX_SECONDS.to_bits() + 1),
        ] {
            assert_eq!(encode(seconds, &[b"fixture"]), Err(Refusal::Timeout));
            let mut packet = encode(1.0, &[b"fixture"]).unwrap();
            packet[6..14].copy_from_slice(&seconds.to_le_bytes());
            assert_eq!(decode(&packet).unwrap_err(), Refusal::Timeout);
        }
    }

    #[test]
    fn rejects_malformed_or_mismatched_framing() {
        let packet = encode(1.0, &[b"one", b"two"]).unwrap();
        for len in 0..packet.len() {
            assert!(
                decode(&packet[..len]).is_err(),
                "accepted truncation at {len}"
            );
        }

        let mut changed = packet.clone();
        changed[5] = b'1';
        assert_eq!(decode(&changed).unwrap_err(), Refusal::Version);

        let mut changed = packet.clone();
        changed[14..16].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(decode(&changed).unwrap_err(), Refusal::TrailingBytes);

        let mut changed = packet.clone();
        changed[14..16].copy_from_slice(&3u16.to_le_bytes());
        assert!(decode(&changed).is_err());

        let mut changed = packet.clone();
        changed[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode(&changed).unwrap_err(), Refusal::ArgumentLength);

        let mut changed = packet;
        changed.push(0);
        assert_eq!(decode(&changed).unwrap_err(), Refusal::TrailingBytes);
    }

    #[test]
    fn rejects_empty_program_and_embedded_nul() {
        assert_eq!(encode(1.0, &[]), Err(Refusal::ArgumentCount));
        assert_eq!(encode(1.0, &[b"", b"argument"]), Err(Refusal::EmptyProgram));
        assert_eq!(encode(1.0, &[b"bad\0name"]), Err(Refusal::ArgumentNul));
        assert_eq!(
            encode(1.0, &[b"name", b"bad\0argument"]),
            Err(Refusal::ArgumentNul)
        );

        let mut packet = encode(1.0, &[b"x", b"y"]).unwrap();
        packet[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode(&packet).unwrap_err(), Refusal::EmptyProgram);

        let mut packet = encode(1.0, &[b"x", b"y"]).unwrap();
        packet[25] = 0;
        assert_eq!(decode(&packet).unwrap_err(), Refusal::ArgumentNul);
    }

    #[test]
    fn enforces_exact_packet_and_argument_bounds() {
        let argument = vec![b'x'; MAX_PACKET_BYTES - HEADER_SIZE - 4];
        let mut packet = encode(1.0, &[&argument]).unwrap();
        assert_eq!(packet.len(), MAX_PACKET_BYTES);
        assert_eq!(decode(&packet).unwrap().program().to_bytes(), argument);
        packet.push(b'x');
        assert_eq!(decode(&packet).unwrap_err(), Refusal::PacketSize);
        assert_eq!(encode(1.0, &[&argument, b""]), Err(Refusal::PacketSize));

        let mut arguments = vec![b"".as_slice(); MAX_ARGS];
        arguments[0] = b"x";
        let mut packet = encode(1.0, &arguments).unwrap();
        assert_eq!(decode(&packet).unwrap().arguments().len(), MAX_ARGS);
        arguments.push(b"");
        assert_eq!(encode(1.0, &arguments), Err(Refusal::ArgumentCount));
        packet[14..16].copy_from_slice(&((MAX_ARGS + 1) as u16).to_le_bytes());
        assert_eq!(decode(&packet).unwrap_err(), Refusal::ArgumentCount);
    }
}
