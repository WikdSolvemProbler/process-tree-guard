// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Explicit separate-service disclosure and strict deny-default policy grammar.
//! Parsed bytes are policy data, never a privileged grant without held authority.
pub(super) const MAX_BYTES:usize=4096;
const MAX_UIDS:usize=64;
const HEADER:&[u8]=b"PTG-DELEGATION-1\ncontext separate-service\n";
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) enum SeparateService { Acknowledged }
#[derive(Debug,PartialEq,Eq)]
pub(super) enum Refusal { Size,Header,Line,Uid,Order,Count }
#[derive(Debug)]
pub(super) struct Policy { uids:Vec<u32> }
impl Policy {
    pub(super) fn permits(&self,uid:u32)->bool {uid!=0&&uid!=u32::MAX&&self.uids.binary_search(&uid).is_ok()}
}
pub(super) fn parse(bytes:&[u8])->Result<Policy,Refusal>{
    if bytes.len()>MAX_BYTES{return Err(Refusal::Size);}
    let rest=bytes.strip_prefix(HEADER).ok_or(Refusal::Header)?;
    if !rest.is_empty()&&rest.last()!=Some(&b'\n'){return Err(Refusal::Line);}
    let mut uids=Vec::new();
    for line in rest.split_inclusive(|b|*b==b'\n'){
        if uids.len()==MAX_UIDS{return Err(Refusal::Count);}
        let digits=line.strip_suffix(b"\n").and_then(|line|line.strip_prefix(b"uid ")).ok_or(Refusal::Line)?;
        if digits.is_empty()||digits.len()>10||digits[0]==b'0'||!digits.iter().all(u8::is_ascii_digit){return Err(Refusal::Uid);}
        let uid=digits.iter().try_fold(0u32,|value,b|value.checked_mul(10)?.checked_add(u32::from(*b-b'0'))).ok_or(Refusal::Uid)?;
        if uid==0||uid==u32::MAX{return Err(Refusal::Uid);}
        if uids.last().is_some_and(|previous|*previous>=uid){return Err(Refusal::Order);}
        uids.push(uid);
    }
    Ok(Policy{uids})
}
