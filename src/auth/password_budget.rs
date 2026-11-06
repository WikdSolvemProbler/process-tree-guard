// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! A password attempt is charged before it reaches Windows. The charge
//! survives process exit, including an interrupted or unanswered verification.

use super::{now_ms, PromptLock, MAX_ATTEMPTS};
use crate::sys::windows::{
    wide, Handle, RegCloseKey, RegCreateKeyExW, RegFlushKey, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW,
};

const KEY_PATH: &str = r"Software\ProcessTreeGuard\PasswordBudget";
const VALUE_NAME: &str = "Attempts";
const WINDOW_MS: u64 = 15 * 60 * 1000;
const BUDGET_FULL: &str = "three password attempts were used in the last 15 minutes";
const BUDGET_BUSY: &str =
    "another password check is running or the password budget lock is unavailable";
const STORAGE_UNAVAILABLE: &str = "the password attempt budget could not be read or saved";
const CLOCK_INVALID: &str = "the system clock precedes a recorded password attempt";

const HKEY_USERS: Handle = 0x8000_0003u32 as i32 as isize as Handle;
const KEY_QUERY_VALUE: u32 = 0x0001;
const KEY_SET_VALUE: u32 = 0x0002;
const KEY_CREATE_SUB_KEY: u32 = 0x0004;
const REG_BINARY: u32 = 3;
const ERROR_FILE_NOT_FOUND: i32 = 2;

const SLOTS: usize = MAX_ATTEMPTS as usize;
const HEADER: [u8; 4] = *b"PTG\x01";
const RECORD_SIZE: usize = HEADER.len() + SLOTS * 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Record([u64; SLOTS]);

impl Record {
    const EMPTY: Self = Self([0; SLOTS]);

    // Nonzero timestamps are sorted, with unused slots set to zero.
    fn decode(bytes: &[u8; RECORD_SIZE]) -> Result<Self, &'static str> {
        if bytes[..HEADER.len()] != HEADER {
            return Err(STORAGE_UNAVAILABLE);
        }
        let mut record = Self::EMPTY;
        let mut previous = 0;
        let mut unused = false;
        for (slot, bytes) in record
            .0
            .iter_mut()
            .zip(bytes[HEADER.len()..].chunks_exact(8))
        {
            let timestamp = u64::from_le_bytes(bytes.try_into().expect("one timestamp"));
            if timestamp == 0 {
                unused = true;
            } else if unused || timestamp < previous {
                return Err(STORAGE_UNAVAILABLE);
            } else {
                previous = timestamp;
                *slot = timestamp;
            }
        }
        Ok(record)
    }

    fn encode(self) -> [u8; RECORD_SIZE] {
        let mut bytes = [0; RECORD_SIZE];
        bytes[..HEADER.len()].copy_from_slice(&HEADER);
        for (slot, timestamp) in bytes[HEADER.len()..].chunks_exact_mut(8).zip(self.0) {
            slot.copy_from_slice(&timestamp.to_le_bytes());
        }
        bytes
    }

    // Return the pruned state before the reservation and the charged state.
    fn reserve(self, now: u64) -> Result<(Self, Self), &'static str> {
        if now == 0 || self.0.iter().any(|&timestamp| timestamp > now) {
            return Err(CLOCK_INVALID);
        }
        let mut before = Self::EMPTY;
        let mut len = 0;
        for timestamp in self.0 {
            if timestamp != 0 && now - timestamp < WINDOW_MS {
                before.0[len] = timestamp;
                len += 1;
            }
        }
        if len == SLOTS {
            return Err(BUDGET_FULL);
        }
        let mut charged = before;
        charged.0[len] = now;
        Ok((before, charged))
    }
}

struct Key(Handle);

impl Key {
    fn open_user_hive(sid: &str) -> Result<Self, &'static str> {
        // Predefined HKCU can resolve to .Default if the user's hive is unloaded.
        let sid = wide(sid);
        let mut key = std::ptr::null_mut();
        let status =
            unsafe { RegOpenKeyExW(HKEY_USERS, sid.as_ptr(), 0, KEY_CREATE_SUB_KEY, &mut key) };
        if status == 0 && !key.is_null() {
            Ok(Self(key))
        } else {
            Err(STORAGE_UNAVAILABLE)
        }
    }

    fn open(parent: Handle, path: &str) -> Result<Self, &'static str> {
        let path = wide(path);
        let mut key = std::ptr::null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                parent,
                path.as_ptr(),
                0,
                std::ptr::null_mut(),
                0,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if status == 0 && !key.is_null() {
            Ok(Self(key))
        } else {
            Err(STORAGE_UNAVAILABLE)
        }
    }

    fn read(&self) -> Result<Record, &'static str> {
        let value = wide(VALUE_NAME);
        let mut bytes = [0; RECORD_SIZE];
        let mut kind = 0;
        let mut size = bytes.len() as u32;
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                value.as_ptr(),
                std::ptr::null_mut(),
                &mut kind,
                bytes.as_mut_ptr(),
                &mut size,
            )
        };
        match status {
            ERROR_FILE_NOT_FOUND => Ok(Record::EMPTY),
            0 if kind == REG_BINARY && size as usize == bytes.len() => Record::decode(&bytes),
            _ => Err(STORAGE_UNAVAILABLE),
        }
    }

    fn write(&self, record: Record) -> Result<(), &'static str> {
        let value = wide(VALUE_NAME);
        let bytes = record.encode();
        let status = unsafe {
            RegSetValueExW(
                self.0,
                value.as_ptr(),
                0,
                REG_BINARY,
                bytes.as_ptr(),
                bytes.len() as u32,
            )
        };
        // A reservation must be durable before the password reaches Windows.
        if status != 0 || unsafe { RegFlushKey(self.0) } != 0 {
            return Err(STORAGE_UNAVAILABLE);
        }
        Ok(())
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

/// Owns the cross-session lock through the password check and any refund.
/// Dropping without a refund leaves the durable reservation in place.
pub(super) struct Attempt {
    key: Key,
    before: Record,
    _lock: PromptLock,
}

impl Attempt {
    pub(super) fn reserve(sid: &str) -> Result<Self, &'static str> {
        let Some(suffix) = sid.strip_prefix("S-") else {
            return Err(STORAGE_UNAVAILABLE);
        };
        if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            return Err(STORAGE_UNAVAILABLE);
        }
        let lock_name = format!(r"Global\ProcessTreeGuardPassword-{sid}");
        reserve_with_clock(sid, &lock_name, KEY_PATH, now_ms)
    }

    pub(super) fn refund(self) -> Result<(), &'static str> {
        self.key.write(self.before)
    }
}

fn reserve_with_clock(
    sid: &str,
    lock_name: &str,
    key_path: &str,
    clock: impl FnOnce() -> u64,
) -> Result<Attempt, &'static str> {
    let lock = PromptLock::take_named(lock_name).map_err(|_| BUDGET_BUSY)?;
    let hive = Key::open_user_hive(sid)?;
    let key = Key::open(hive.0, key_path)?;
    let record = key.read()?;
    let (before, charged) = record.reserve(clock())?;
    key.write(charged)?;
    Ok(Attempt {
        key,
        before,
        _lock: lock,
    })
}

#[cfg(test)]
fn reserve_in(lock_name: &str, key_path: &str, now_ms: u64) -> Result<Attempt, &'static str> {
    let sid = super::current_user_sid().map_err(|_| STORAGE_UNAVAILABLE)?;
    reserve_with_clock(&sid, lock_name, key_path, || now_ms)
}

#[cfg(test)]
#[path = "password_budget/process_tests.rs"]
mod process_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unloaded_user_hive_cannot_supply_a_password_budget() {
        // The Windows null SID has no user profile hive.
        assert_eq!(
            Key::open_user_hive("S-1-0-0").err(),
            Some(STORAGE_UNAVAILABLE)
        );
    }

    #[test]
    fn budget_counts_across_reservations_and_expires_at_the_window_boundary() {
        let mut record = Record::EMPTY;
        for now in [100, 101, 102] {
            record = record.reserve(now).unwrap().1;
        }
        assert_eq!(record.reserve(100 + WINDOW_MS - 1), Err(BUDGET_FULL));
        let (before, charged) = record.reserve(100 + WINDOW_MS).unwrap();
        assert_eq!(before.0, [101, 102, 0]);
        assert_eq!(charged.0, [101, 102, 100 + WINDOW_MS]);
    }

    #[test]
    fn record_can_refund_only_the_current_reservation() {
        let old = Record([100, 110, 0]);
        let (before, charged) = old.reserve(120).unwrap();
        assert_eq!(before, old);
        assert_eq!(charged.0, [100, 110, 120]);
        assert_eq!(Record::decode(&before.encode()), Ok(old));
    }

    #[test]
    fn record_refuses_invalid_clock_and_corruption() {
        assert_eq!(Record::EMPTY.reserve(0), Err(CLOCK_INVALID));
        assert_eq!(Record([100, 101, 0]).reserve(100), Err(CLOCK_INVALID));
        let mut unknown_version = Record::EMPTY.encode();
        unknown_version[3] = 2;
        assert_eq!(Record::decode(&unknown_version), Err(STORAGE_UNAVAILABLE));
        assert_eq!(
            Record::decode(&Record([100, 0, 101]).encode()),
            Err(STORAGE_UNAVAILABLE)
        );
        assert_eq!(
            Record::decode(&Record([101, 100, 0]).encode()),
            Err(STORAGE_UNAVAILABLE)
        );
        assert_eq!(
            Record::decode(&Record([100, 100, 0]).encode()),
            Ok(Record([100, 100, 0]))
        );
    }
}
