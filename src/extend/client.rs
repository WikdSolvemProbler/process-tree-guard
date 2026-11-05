// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The client of an extendable run: finding its channel, connecting, and exchanging one request and one reply.

use super::*;


use crate::sys::windows::{

    CloseHandle, CreateEventW, CreateFileW, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY,

    FILE_FLAG_OVERLAPPED, GetLastError, Handle, INVALID_HANDLE, OPEN_EXISTING, Overlapped,

    PIPE_READMODE_MESSAGE, ReadFile, ResetEvent, SECURITY_SQOS_PRESENT, SetNamedPipeHandleState,

    WAIT_TIMEOUT, WaitNamedPipeW, WriteFile, checked, error_for, os_error, wide,

};
// ---------------------------------------------------------------------- the client

pub(super) fn valid_channel(name: &str) -> bool {
    name.strip_prefix(PIPE_PREFIX).is_some_and(|rest| rest.len() == 32 && rest.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// The validated channel name of a process-owning invocation, computed
/// once and reused for the invocation's life: the same premise the
/// resolver's caches and the console cache already rest on — the
/// environment of a one-shot guard process does not change under it,
/// and this process never sets the variable (the guard sets it only
/// in the child's block). The cached value is the validated `Option`
/// itself, absence included; a name that fails validation is never
/// cached, so its error is re-derived on every call, exactly as the
/// live read derives it.
static CACHED_CHANNEL: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// The channel of the enclosing run, if the environment names one. The name is
/// checked before it is opened, so a changed variable never names a file or a pipe
/// that is not a channel. On a process-owning invocation (exactly when
/// `cli_process::active()` holds) the validated name is read from the
/// cache above once it is filled; a returning library call never
/// touches the cache and reads and validates the live environment on
/// every call, on the resolver's own active/live split.
pub(super) fn channel_from_environment() -> Result<Option<std::borrow::Cow<'static, str>>, GuardError> {
    if !crate::cli_process::active() {
        return Ok(live_channel_from_environment()?.map(std::borrow::Cow::Owned));
    }
    if let Some(cached) = CACHED_CHANNEL.get() {
        // The cache is process-lifetime storage, so the validated
        // name is lent from it: cloning it on every call charged an
        // allocation and a copy of the name to every nested guard
        // start, every enclosed check and every release, for a text
        // no caller keeps past the request it makes with it.
        return Ok(cached.as_deref().map(std::borrow::Cow::Borrowed));
    }
    let name = live_channel_from_environment()?;
    // A racing first call may have filled the slot first; both calls
    // validated the same name from the same unchanging environment,
    // so the slot's copy and this one are the same name either way.
    if CACHED_CHANNEL.set(name.clone()).is_err() {
        if let Some(cached) = CACHED_CHANNEL.get() {
            return Ok(cached.as_deref().map(std::borrow::Cow::Borrowed));
        }
    }
    Ok(name.map(std::borrow::Cow::Owned))
}

/// The live read behind `channel_from_environment`: the environment
/// lookup, the conversion and the validation walk, exactly as every
/// call performed them before the cache.
fn live_channel_from_environment() -> Result<Option<String>, GuardError> {
    match std::env::var_os(CHANNEL_VARIABLE) {
        None => Ok(None),
        Some(value) if value.is_empty() => Ok(None),
        Some(value) => match value.into_string() {
            Ok(name) if valid_channel(&name) => Ok(Some(name)),
            _ => Err(GuardError(format!("{} does not name an extension channel", CHANNEL_VARIABLE))),
        },
    }
}

/// What a client says when the channel it names is not there.
pub(super) const CHANNEL_GONE: &str = "the extendable run's channel is gone";

/// The wide form of a name, in stack storage when it fits and in a
/// heap vector otherwise: a channel name is the 28 characters of
/// `PIPE_PREFIX` and 32 hex characters, 61 units with its NUL, so
/// every name `valid_channel` accepts takes the stack form and the
/// heap form is the fallback for a name of any other shape, spelled
/// exactly as `wide` spells it. The units are written one by one
/// before any read of them, so no unit needs clearing first.
pub(super) enum StackWide<const N: usize> {
    Stack([std::mem::MaybeUninit<u16>; N]),
    Heap(Vec<u16>),
}

impl<const N: usize> StackWide<N> {
    pub(super) fn as_ptr(&self) -> *const u16 {
        match self {
            Self::Stack(storage) => storage.as_ptr() as *const u16,
            Self::Heap(units) => units.as_ptr(),
        }
    }
}

pub(super) type WideName = StackWide<64>;

pub(super) fn stack_wide<const N: usize>(name: &str) -> StackWide<N> {
    let mut storage = [std::mem::MaybeUninit::<u16>::uninit(); N];
    for (count, unit) in name.encode_utf16().chain(std::iter::once(0)).enumerate() {
        if count == storage.len() {
            return StackWide::Heap(wide(name));
        }
        storage[count].write(unit);
    }
    StackWide::Stack(storage)
}

pub(super) fn wide_name(name: &str) -> WideName { stack_wide::<64>(name) }

/// Open the channel, waiting a little while its single instance is busy. None when there is no
/// such channel: the run that made it has ended.
pub(super) fn connect(name: &str) -> Result<Option<Handle>, GuardError> {
    let wide_name = wide_name(name);
    // The retry bound's clock is read only when a retry is actually
    // needed: a connect that succeeds (or fails outright) on its
    // first attempt never reads it, where reading it before the loop
    // charged every connect one clock call for a bound it never used.
    // The bound therefore runs from the first busy answer, which is
    // the first moment any waiting can begin.
    let mut began: Option<u64> = None;
    loop {
        // Anonymous impersonation level: the server never acts as this process.
        let pipe = unsafe { CreateFileW(wide_name.as_ptr(), CLIENT_ACCESS, 0, std::ptr::null_mut(), OPEN_EXISTING, SECURITY_SQOS_PRESENT | FILE_FLAG_OVERLAPPED, std::ptr::null_mut()) };
        if pipe != INVALID_HANDLE {
            return Ok(Some(pipe));
        }
        match unsafe { GetLastError() } {
            ERROR_PIPE_BUSY => {
                // One clock read settles the retry bound: the
                // first busy answer both starts the bound (the
                // moment any waiting can begin) and is measured
                // against it — reading the clock once for the
                // insertion and again for the subtraction paid two
                // `GetTickCount64` calls for the one instant the
                // bound runs from.
                let now = tick();
                let began_at = *began.get_or_insert(now);
                if now.saturating_sub(began_at) < CONNECT_MS {
                    unsafe { WaitNamedPipeW(wide_name.as_ptr(), 500) };
                } else {
                    return Err(GuardError("the extendable run's channel is busy".into()));
                }
            }
            ERROR_FILE_NOT_FOUND => return Ok(None),
            _ => return Err(os_error()),
        }
    }
}

/// What a client makes of a failed call: the run did not answer in time, or a Windows error.
pub(super) fn client_error(code: u32) -> GuardError {
    if code == WAIT_TIMEOUT {
        GuardError("the extendable run did not answer".into())
    } else {
        error_for(code)
    }
}

/// The one overlapped event of a thread's exchanges, closed when the
/// thread ends, so a host thread that ends takes no handle with it. An
/// exchange is thread-confined by construction: it is made, waited on
/// and finished inside one `exchange` call on one thread, exchanges on
/// one thread are sequential, and no path inside an exchange — the
/// write, `finish_io` and the read — calls back into an exchange, so
/// the event is never shared between concurrent exchanges and never
/// carries an operation that is still in flight when `exchange`
/// returns (`finish_io` settles its operation before it returns).
struct ExchangeEvent(Handle);

impl Drop for ExchangeEvent {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

thread_local! {
    static EXCHANGE_EVENT: std::cell::RefCell<Option<ExchangeEvent>> = const { std::cell::RefCell::new(None) };
}

/// The handle of this thread's exchange event, creating it on the
/// thread's first exchange only. The slot's borrow is released before
/// the exchange itself, so only the handle is carried into it.
fn exchange_event() -> Result<Handle, GuardError> {
    EXCHANGE_EVENT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(event) = slot.as_ref() {
            return Ok(event.0);
        }
        let event = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(os_error());
        }
        *slot = Some(ExchangeEvent(event));
        Ok(event)
    })
}

/// Send `request` on the open channel and read the reply, each step for at most
/// `REPLY_MS`: a run that no longer answers is refused here, not waited for.
pub(super) fn exchange(pipe: Handle, request: &[u8]) -> Result<Reply, GuardError> {
    unsafe {
        let mut mode = PIPE_READMODE_MESSAGE;
        checked(SetNamedPipeHandleState(pipe, &mut mode, std::ptr::null_mut(), std::ptr::null_mut()))?;
        let event = exchange_event()?;
        // A completed read of an earlier exchange on this thread leaves
        // the manual-reset event signalled, where a fresh event starts
        // unsignalled, so it is reset before the write as well as
        // between the write and the read.
        ResetEvent(event);
        let mut overlapped = Overlapped::with_event(event);
        let mut written = 0u32;
        let started = WriteFile(pipe, request.as_ptr() as *const c_void, request.len() as u32, &mut written, &mut overlapped) != 0;
        finish_io(pipe, &mut overlapped, started, written, &[event], REPLY_MS).map_err(client_error)?;
        ResetEvent(event);
        let mut overlapped = Overlapped::with_event(event);
        let mut reply = [0u8; REPLY_SIZE];
        let mut read = 0u32;
        let started = ReadFile(pipe, reply.as_mut_ptr() as *mut c_void, reply.len() as u32, &mut read, &mut overlapped) != 0;
        let length = finish_io(pipe, &mut overlapped, started, read, &[event], REPLY_MS).map_err(client_error)?;
        decode_reply(&reply[..length as usize]).ok_or_else(not_understood)
    }
}

/// Ask the enclosing extendable run to hold the deadline of `target` for `seconds`
/// from now. Refused with the reason when the run says no. The seconds are sent as
/// they are, whatever they are: the run refuses an invalid or excessive request and
/// logs it, so that every request that reaches a run is in its log.
pub(crate) fn request(target: Target, seconds: f64) -> Result<Grant, GuardError> {
    request_held(target, 0, seconds).map(|(grant, _)| grant)
}

/// `request` for the hold `token` of the process, and the process the hold is on when it is
/// not the one that asked (the parent of the `extend` program), or 0.
pub(crate) fn request_held(target: Target, token: Token, seconds: f64) -> Result<(Grant, u32), GuardError> {
    let name = channel_from_environment()?.ok_or_else(|| GuardError(format!("no extendable run encloses this process ({} is not set)", CHANNEL_VARIABLE)))?;
    ask(&name, target, token, seconds)?.ok_or_else(|| GuardError(CHANNEL_GONE.into()))
}

/// Send one request to the channel `name`. None when the channel is not there.
pub(super) fn ask(name: &str, target: Target, token: Token, seconds: f64) -> Result<Option<(Grant, u32)>, GuardError> {
    let Some(pipe) = connect(name)? else { return Ok(None) };
    let outcome = exchange(pipe, &encode_request(target, token, seconds));
    unsafe { CloseHandle(pipe) };
    match outcome? {
        Reply::Granted { seconds, until_unix_ms, held } => {
            // `decode_reply` has kept the time in range; the sum is checked all the same, since a panic here is a crash.
            let until = UNIX_EPOCH.checked_add(Duration::from_millis(until_unix_ms)).ok_or_else(not_understood)?;
            Ok(Some((Grant { seconds, until }, held)))
        }
        Reply::Refused { refusal, cap } => Err(GuardError(format!("extension refused: {}", refusal.describe(cap)))),
    }
}

/// The request a guard makes when it starts inside an extendable run: its deadline
/// is a request for that many seconds. True when the request was made and granted, and
/// false when no run encloses the guard: the variable is not set, does not name a channel,
/// or names one that is gone. Such a value is no run, so the guard keeps its own fixed
/// deadline, as it did before extendable runs; a value that names a channel that is
/// there, and a run that refuses or does not answer properly, end the guard with an error.
/// `required` is the contain role, which has no use for a run that is not there and never
/// runs a command without one: for it each of those cases is an error. The hold is the
/// process's hold `token`: a call of the library has one of its own.
pub(crate) fn register_start(seconds: f64, required: bool, token: Token) -> Result<bool, GuardError> {
    let none = || if required {
        Err(GuardError(format!("this program runs inside an extendable run of process-tree-guard, and none encloses it ({} is not set)", CHANNEL_VARIABLE)))
    } else {
        Ok(false)
    };
    let name = match channel_from_environment() {
        Ok(Some(name)) => name,
        Ok(None) => return none(),
        Err(error) => return if required { Err(error) } else { Ok(false) },
    };
    match ask(&name, Target::Caller, token, seconds)? {
        Some(_) => Ok(true),
        None if required => Err(GuardError(CHANNEL_GONE.into())),
        None => none(),
    }
}

/// Whether an extendable run encloses this process: the environment names a channel and the
/// channel is there. Nothing is sent to it, so it places no hold on the run, and a name that is
/// not a channel, or one that is gone, is no run (as for `register_start`). A channel that is
/// busy is there.
pub(crate) fn enclosed() -> bool {
    let Ok(Some(name)) = channel_from_environment() else { return false };
    let wide_name = wide_name(&name);
    unsafe { WaitNamedPipeW(wide_name.as_ptr(), 1) != 0 || GetLastError() != ERROR_FILE_NOT_FOUND }
}

/// End the hold `token` of the calling process on the enclosing run: what a library call
/// does when it returns, since the process that made it goes on and the hold was for the
/// call. Its other holds, the ones of calls that overlap it among them, stand. It moves
/// the deadline earlier or not at all.
pub(crate) fn release(token: Token) -> Result<(), GuardError> {
    request_held(Target::Release, token, 0.0).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_wide_matches_wide_at_both_capacities() {
        fn check<const N: usize>(text: &str) {
            let expected = wide(text);
            let widened = stack_wide::<N>(text);
            let units = unsafe { std::slice::from_raw_parts(widened.as_ptr(), expected.len()) };
            assert_eq!(units, expected.as_slice());
        }
        for length in [0, 1, 63, 64, 65, 255, 256, 257] {
            let text = "a".repeat(length);
            check::<64>(&text);
            check::<256>(&text);
        }
        let text = "héllo wörld 🌍";
        check::<64>(text);
        check::<256>(text);
    }
}
