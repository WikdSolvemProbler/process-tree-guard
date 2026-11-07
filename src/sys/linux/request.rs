// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Private PTGEX3 packet: one authenticated caller's argv/environment snapshot.
//! The separately transferred five rights are owner pidfd, stdin/out/err and CWD.
use crate::deadline;
use std::ffi::{c_char, CStr, CString};
use std::ptr;
use std::time::Duration;

const MAGIC: &[u8; 6] = b"PTGEX3";
const HEADER_SIZE: usize = 20;
pub const MAX_PACKET_BYTES: usize = 128 * 1024;
const MAX_NATIVE_BYTES: usize = 128 * 1024;
const MAX_ARGS: usize = 4096;
const MAX_ENV: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    PacketSize, Header, Version, Timeout, ArgumentCount, ArgumentLength,
    EmptyProgram, ArgumentNul, EnvironmentCount, EnvironmentEntry,
    NativeBytes, TrailingBytes,
}

// All strings own their native bytes. Moving a plan never moves its buffers.
#[derive(Debug)]
pub struct ExecPlan {
    duration: Duration,
    arguments: Vec<CString>,
    pointers: Vec<*const c_char>,
    environment: Vec<CString>,
}
impl ExecPlan {
    pub fn duration(&self) -> Duration { self.duration }
    pub fn program(&self) -> &CStr { &self.arguments[0] }
    pub fn arguments(&self) -> &[CString] { &self.arguments }
    pub fn environment(&self) -> &[CString] { &self.environment }
    pub fn argv(&self) -> &[*const c_char] { &self.pointers }
}
fn duration(seconds: f64) -> Result<Duration, Refusal> {
    if deadline::invalid_seconds(seconds) { return Err(Refusal::Timeout); }
    Ok(Duration::try_from_secs_f64(seconds).map_err(|_| Refusal::Timeout)?
        .max(Duration::from_nanos(1)))
}
fn environment_entry(value: &[u8]) -> Result<(), Refusal> {
    if value.contains(&0) || !value.iter().position(|byte| *byte == b'=').is_some_and(|at| at > 0) {
        return Err(Refusal::EnvironmentEntry);
    }
    Ok(())
}
fn sizes(arguments: &[&[u8]], environment: &[&[u8]]) -> Result<usize, Refusal> {
    if arguments.is_empty() || arguments.len() > MAX_ARGS { return Err(Refusal::ArgumentCount); }
    if environment.len() > MAX_ENV { return Err(Refusal::EnvironmentCount); }
    if arguments[0].is_empty() { return Err(Refusal::EmptyProgram); }
    let mut packet = HEADER_SIZE;
    // Unchanged native bridge counts the program plus argv plus envp, including
    // every terminator. Account for that exact aggregate before any arming.
    let mut native = arguments[0].len().checked_add(1).ok_or(Refusal::NativeBytes)?;
    for value in arguments.iter().chain(environment) {
        packet = packet.checked_add(4).and_then(|n| n.checked_add(value.len()))
            .filter(|n| *n <= MAX_PACKET_BYTES).ok_or(Refusal::PacketSize)?;
        native = native.checked_add(value.len()).and_then(|n| n.checked_add(1))
            .filter(|n| *n <= MAX_NATIVE_BYTES).ok_or(Refusal::NativeBytes)?;
    }
    for value in arguments { if value.contains(&0) { return Err(Refusal::ArgumentNul); } }
    for value in environment { environment_entry(value)?; }
    Ok(packet)
}
pub fn encode(seconds: f64, arguments: &[&[u8]], environment: &[&[u8]]) -> Result<Vec<u8>, Refusal> {
    duration(seconds)?;
    let size = sizes(arguments, environment)?;
    let mut packet = Vec::with_capacity(size);
    packet.extend_from_slice(MAGIC);
    packet.extend_from_slice(&seconds.to_le_bytes());
    packet.extend_from_slice(&(arguments.len() as u16).to_le_bytes());
    packet.extend_from_slice(&(environment.len() as u16).to_le_bytes());
    packet.extend_from_slice(&0u16.to_le_bytes());
    for value in arguments.iter().chain(environment) {
        packet.extend_from_slice(&(value.len() as u32).to_le_bytes());
        packet.extend_from_slice(value);
    }
    Ok(packet)
}
fn take_string<'a>(rest: &mut &'a [u8]) -> Result<&'a [u8], Refusal> {
    let (length, tail) = rest.split_first_chunk::<4>().ok_or(Refusal::ArgumentLength)?;
    let length = usize::try_from(u32::from_le_bytes(*length)).map_err(|_| Refusal::ArgumentLength)?;
    let (value, tail) = tail.split_at_checked(length).ok_or(Refusal::ArgumentLength)?;
    *rest = tail;
    Ok(value)
}
pub fn decode(packet: &[u8]) -> Result<ExecPlan, Refusal> {
    if packet.len() > MAX_PACKET_BYTES { return Err(Refusal::PacketSize); }
    let (header, mut rest) = packet.split_first_chunk::<HEADER_SIZE>().ok_or(Refusal::Header)?;
    if &header[..6] != MAGIC { return Err(Refusal::Version); }
    if header[18..20] != [0, 0] { return Err(Refusal::Header); }
    let duration = duration(f64::from_le_bytes(header[6..14].try_into().map_err(|_| Refusal::Header)?))?;
    let argc = usize::from(u16::from_le_bytes(header[14..16].try_into().map_err(|_| Refusal::Header)?));
    let envc = usize::from(u16::from_le_bytes(header[16..18].try_into().map_err(|_| Refusal::Header)?));
    if argc == 0 || argc > MAX_ARGS { return Err(Refusal::ArgumentCount); }
    if envc > MAX_ENV { return Err(Refusal::EnvironmentCount); }
    if argc.checked_add(envc).is_none_or(|n| n > rest.len() / 4) { return Err(Refusal::ArgumentLength); }
    let mut arguments = Vec::with_capacity(argc);
    let mut environment = Vec::with_capacity(envc);
    for index in 0..argc {
        let value = take_string(&mut rest)?;
        if index == 0 && value.is_empty() { return Err(Refusal::EmptyProgram); }
        arguments.push(CString::new(value).map_err(|_| Refusal::ArgumentNul)?);
    }
    for _ in 0..envc {
        let value = take_string(&mut rest)?;
        environment_entry(value)?;
        environment.push(CString::new(value).map_err(|_| Refusal::EnvironmentEntry)?);
    }
    if !rest.is_empty() { return Err(Refusal::TrailingBytes); }
    let argv_bytes: Vec<_> = arguments.iter().map(|v| v.as_bytes()).collect();
    let env_bytes: Vec<_> = environment.iter().map(|v| v.as_bytes()).collect();
    sizes(&argv_bytes, &env_bytes)?;
    let pointers = arguments.iter().map(|v| v.as_ptr()).chain(std::iter::once(ptr::null())).collect();
    Ok(ExecPlan { duration, arguments, pointers, environment })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_native_bytes_empty_arguments_and_null_terminated_argv() {
        let arguments: &[&[u8]] = &[b"/tmp/command \xff", b"", b"a b", b"\x80\xfe", b""];
        let environment: &[&[u8]] = &[b"KEY=\xff a b", b"EMPTY=", b"KEY=second", b"\xfe=\x80"];
        let packet = encode(2.25, arguments, environment).unwrap();
        let plan = decode(&packet).unwrap();
        drop(packet);
        assert_eq!(plan.duration(), Duration::from_millis(2250));
        assert_eq!(plan.program().to_bytes(), arguments[0]);
        assert_eq!(plan.arguments().iter().map(|v| v.to_bytes()).collect::<Vec<_>>(), arguments);
        assert_eq!(plan.environment().iter().map(|v| v.to_bytes()).collect::<Vec<_>>(), environment);
        let pointers = plan.arguments().iter().map(|v| v.as_ptr()).chain(std::iter::once(ptr::null())).collect::<Vec<_>>();
        assert_eq!(plan.argv(), pointers);
    }
    #[test]
    fn matches_numeric_policy_without_disarming_the_timer() {
        for (seconds, nanos) in [(f64::from_bits(1),1),(f64::MIN_POSITIVE,1),(0.25e-9,1),(0.5,500_000_000),(deadline::MAX_SECONDS,86_400_000_000_000)] {
            let plan = decode(&encode(seconds,&[b"fixture"],&[]).unwrap()).unwrap();
            assert_eq!(plan.duration(),Duration::from_nanos(nanos));
        }
        for seconds in [0.0,-0.0,-1.0,f64::NAN,f64::INFINITY,f64::NEG_INFINITY,f64::from_bits(deadline::MAX_SECONDS.to_bits()+1)] {
            assert_eq!(encode(seconds,&[b"fixture"],&[]),Err(Refusal::Timeout));
            let mut packet=encode(1.0,&[b"fixture"],&[]).unwrap();
            packet[6..14].copy_from_slice(&seconds.to_le_bytes());
            assert_eq!(decode(&packet).unwrap_err(),Refusal::Timeout);
        }
    }
    #[test]
    fn rejects_malformed_or_mismatched_framing() {
        let packet=encode(1.0,&[b"one",b"two"],&[]).unwrap();
        for len in 0..packet.len() {assert!(decode(&packet[..len]).is_err(),"accepted truncation at {len}");}
        let mut changed=packet.clone();changed[5]=b'2';
        assert_eq!(decode(&changed).unwrap_err(),Refusal::Version);
        let mut changed=packet.clone();changed[14..16].copy_from_slice(&1u16.to_le_bytes());
        assert_eq!(decode(&changed).unwrap_err(),Refusal::TrailingBytes);
        let mut changed=packet.clone();changed[14..16].copy_from_slice(&3u16.to_le_bytes());
        assert!(decode(&changed).is_err());
        let env_packet=encode(1.0,&[b"one",b"two"],&[b"A=B"]).unwrap();
        for len in 0..env_packet.len() {assert!(decode(&env_packet[..len]).is_err(),"accepted env truncation at {len}");}
        let mut changed=packet.clone();changed[16..18].copy_from_slice(&2u16.to_le_bytes());
        assert!(decode(&changed).is_err());
        let mut changed=packet.clone();changed[18]=1;
        assert_eq!(decode(&changed).unwrap_err(),Refusal::Header);
        let mut changed=packet.clone();changed[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode(&changed).unwrap_err(),Refusal::ArgumentLength);
        let mut changed=packet;changed.push(0);
        assert_eq!(decode(&changed).unwrap_err(),Refusal::TrailingBytes);
    }
    #[test]
    fn rejects_empty_program_and_embedded_nul() {
        assert_eq!(encode(1.0,&[],&[]),Err(Refusal::ArgumentCount));
        assert_eq!(encode(1.0,&[b"",b"argument"],&[]),Err(Refusal::EmptyProgram));
        assert_eq!(encode(1.0,&[b"bad\0name"],&[]),Err(Refusal::ArgumentNul));
        assert_eq!(encode(1.0,&[b"name",b"bad\0argument"],&[]),Err(Refusal::ArgumentNul));
        for env in [b"no_equal".as_slice(),b"=empty_name",b"A=bad\0value"] {
            assert_eq!(encode(1.0,&[b"x"],&[env]),Err(Refusal::EnvironmentEntry));
        }
        let mut packet=encode(1.0,&[b"x"],&[b"A=B"]).unwrap();packet[20..24].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(decode(&packet).unwrap_err(),Refusal::EmptyProgram);
        let mut packet=encode(1.0,&[b"x",b"y"],&[]).unwrap();*packet.last_mut().unwrap()=0;
        assert_eq!(decode(&packet).unwrap_err(),Refusal::ArgumentNul);
        let mut packet=encode(1.0,&[b"x"],&[b"A=B"]).unwrap();*packet.last_mut().unwrap()=0;
        assert_eq!(decode(&packet).unwrap_err(),Refusal::EnvironmentEntry);
    }
    #[test]
    fn enforces_exact_packet_and_argument_bounds() {
        let argument=vec![b'x';MAX_PACKET_BYTES-HEADER_SIZE-9];
        let mut packet=encode(1.0,&[b"x",&argument],&[]).unwrap();
        assert_eq!(packet.len(),MAX_PACKET_BYTES);
        assert_eq!(decode(&packet).unwrap().arguments()[1].as_bytes(),argument);
        packet.push(b'x');assert_eq!(decode(&packet).unwrap_err(),Refusal::PacketSize);
        let exact_program=vec![b'x';MAX_NATIVE_BYTES/2-1];
        assert_eq!(decode(&encode(1.0,&[&exact_program],&[]).unwrap()).unwrap().program().to_bytes(),exact_program);
        let combined_program=vec![b'p';32];
        let mut combined_env=vec![b'v';MAX_NATIVE_BYTES-2*(combined_program.len()+1)-1];
        combined_env[..2].copy_from_slice(b"A=");
        assert_eq!(decode(&encode(1.0,&[&combined_program],&[&combined_env]).unwrap()).unwrap().environment()[0].as_bytes(),combined_env);
        combined_env.push(b'x');
        assert_eq!(encode(1.0,&[&combined_program],&[&combined_env]),Err(Refusal::NativeBytes));
        let program=vec![b'x';MAX_NATIVE_BYTES/2];
        assert_eq!(encode(1.0,&[&program],&[]),Err(Refusal::NativeBytes));
        let mut arguments=vec![b"".as_slice();MAX_ARGS];arguments[0]=b"x";
        assert_eq!(decode(&encode(1.0,&arguments,&[]).unwrap()).unwrap().arguments().len(),MAX_ARGS);
        let mut bad_count=encode(1.0,&arguments,&[]).unwrap();
        bad_count[14..16].copy_from_slice(&((MAX_ARGS+1) as u16).to_le_bytes());
        assert_eq!(decode(&bad_count).unwrap_err(),Refusal::ArgumentCount);
        arguments.push(b"");assert_eq!(encode(1.0,&arguments,&[]),Err(Refusal::ArgumentCount));
        let env=vec![b"A=".as_slice();MAX_ENV];
        assert_eq!(decode(&encode(1.0,&[b"x"],&env).unwrap()).unwrap().environment().len(),MAX_ENV);
        let mut env=env;env.push(b"B=");assert_eq!(encode(1.0,&[b"x"],&env),Err(Refusal::EnvironmentCount));
        let mut packet=encode(1.0,&[b"x"],&[]).unwrap();packet[16..18].copy_from_slice(&((MAX_ENV+1) as u16).to_le_bytes());
        assert_eq!(decode(&packet).unwrap_err(),Refusal::EnvironmentCount);
    }
}
