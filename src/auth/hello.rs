// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Windows Hello approval: the consent of the signed-in user, asked through the Windows Runtime.

use super::*;


use crate::sys::windows::{

    CheckAvailabilityAsyncFn, Com, DESKTOP_READOBJECTS, E_NOTIMPL, GetCurrentProcessId,

    GetCurrentThreadId, HString, Handle, IID_AVAILABILITY_OPERATION, IID_CONSENT_INTEROP,

    IID_CONSENT_STATICS, IID_RESULT_OPERATION, Msg, ProcessIdToSessionId,

    RequestVerificationForWindowAsyncFn, Sleep, UOI_NAME, WM_SYSTIMER, WM_TIMER, WTS_DOMAIN_NAME,

    WTS_USER_NAME, WindowStationApi, failure_of, hello_api, slot, window_api, window_station_api,

    wts_api,

};
mod operation;
mod text;

pub(super) use operation::*;
pub(super) use text::*;

// --------------------------------------------------------------- Windows Hello

/// The word the log gives for a run that was approved or refused by the consent of Windows Hello.
pub(super) const HELLO_METHOD: &str = "hello";

/// Why the consent of Windows Hello is not asked for where its libraries are missing.
pub(super) const HELLO_MISSING: &str = "the Windows Runtime is not available on this system";

/// Whether Windows Hello can verify the user (`UserConsentVerifierAvailability`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Availability {
    Available,
    DeviceNotPresent,
    NotConfiguredForUser,
    DisabledByPolicy,
    DeviceBusy,
    Unknown(i32),
}

impl Availability {
    pub(crate) fn from_code(code: i32) -> Availability {
        match code {
            0 => Availability::Available,
            1 => Availability::DeviceNotPresent,
            2 => Availability::NotConfiguredForUser,
            3 => Availability::DisabledByPolicy,
            4 => Availability::DeviceBusy,
            other => Availability::Unknown(other),
        }
    }

    /// The name the record gives it.
    pub(crate) fn name(self) -> String {
        match self {
            Availability::Available => "Available".into(),
            Availability::DeviceNotPresent => "DeviceNotPresent".into(),
            Availability::NotConfiguredForUser => "NotConfiguredForUser".into(),
            Availability::DisabledByPolicy => "DisabledByPolicy".into(),
            Availability::DeviceBusy => "DeviceBusy".into(),
            Availability::Unknown(code) => format!("Unknown({})", code),
        }
    }
}

/// What a request for the consent of Windows Hello was answered with (`UserConsentVerificationResult`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Consent {
    Verified,
    DeviceNotPresent,
    NotConfiguredForUser,
    DisabledByPolicy,
    DeviceBusy,
    RetriesExhausted,
    Canceled,
    Unknown(i32),
}

impl Consent {
    pub(crate) fn from_code(code: i32) -> Consent {
        match code {
            0 => Consent::Verified,
            1 => Consent::DeviceNotPresent,
            2 => Consent::NotConfiguredForUser,
            3 => Consent::DisabledByPolicy,
            4 => Consent::DeviceBusy,
            5 => Consent::RetriesExhausted,
            6 => Consent::Canceled,
            other => Consent::Unknown(other),
        }
    }

    /// The name the record gives it.
    pub(crate) fn name(self) -> String {
        match self {
            Consent::Verified => "Verified".into(),
            Consent::DeviceNotPresent => "DeviceNotPresent".into(),
            Consent::NotConfiguredForUser => "NotConfiguredForUser".into(),
            Consent::DisabledByPolicy => "DisabledByPolicy".into(),
            Consent::DeviceBusy => "DeviceBusy".into(),
            Consent::RetriesExhausted => "RetriesExhausted".into(),
            Consent::Canceled => "Canceled".into(),
            Consent::Unknown(code) => format!("Unknown({})", code),
        }
    }
}

/// How a request for the consent of Windows Hello ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The system answered; the code is a `UserConsentVerificationResult`.
    Done(i32),
    /// The operation ended as cancelled, which nothing asked for here.
    Cancelled,
    /// The operation failed, with this HRESULT.
    Failed(u32),
    /// No answer came within the time allowed, and the request was cancelled.
    TimedOut,
    /// The process that started the guard exited while the request was open, and the request was cancelled.
    OwnerGone,
    /// The request was asked to cancel and did not end within the time given for it.
    CancelIgnored,
}

/// Windows Hello, as the decision uses it. The real one reaches the Windows Runtime; the unit tests give a scripted one.
pub(crate) trait Consenter {
    /// Whether Windows Hello can verify the user. It shows nothing. An error is the reason that it could not be found out
    /// (a library is missing, the call failed or took longer than `SYSTEM_TIMEOUT_MS`).
    fn availability(&self, owners: &Owners) -> Result<Availability, String>;

    /// Whether the guard runs as the user who is signed in to its session, which is the user whom Windows Hello verifies, and on
    /// the desktop that receives the input of that user, where the request can be read. An error is the reason that it cannot be
    /// said so.
    fn session_user(&self, owners: &Owners) -> Result<(), String>;

    /// Ask for the consent, showing `message`. It does not return before the request has ended or been cancelled, and watches
    /// `owners` while it waits.
    fn request(&self, message: &str, owners: &Owners) -> Outcome;
}

/// Decide on the exception of a run by the desktop method: the consent of Windows Hello where it is available, and otherwise the
/// credential dialog. In this order, which must not change: (1) the starter is still there; (2) somebody is present and no other
/// prompt of the guard is open, one right for the rest of the run; (3) Windows Hello is asked whether it is available, which shows
/// nothing; (4) if it is, the guard must run as the user signed in to the session, and then one request is made and its answer
/// decides; (5) in any other case, or when the question was not answered, the credential dialog asks for the password with its
/// three attempts, and every reason of the record ends with why Windows Hello was not used. A consent that was asked for and not
/// given never falls back to the dialog: that would let a refusal be answered by another door.
pub(crate) fn authorize_desktop(settings: &Settings, subject: &Subject, consenter: &dyn Consenter, prompt: &dyn Prompt, verifier: &dyn Verifier, owners: &Owners, log: &Log) -> Result<(), GuardError> {
    let Some(exception) = settings.exception.as_ref() else { return Ok(()) };
    if owners.gone() {
        return Err(refuse(settings, log, subject, 0, OWNER_GONE));
    }
    let text = prompt_text(subject, exception, settings.max_seconds);
    // Held for the rest of the run, whichever of the two asks, so that the lock is taken once.
    let _claim = match prompt.seat(&text) {
        Ok(claim) => claim,
        Err(reason) => return Err(refuse(settings, log, subject, 0, &reason)),
    };
    let availability = consenter.availability(owners);
    if owners.gone() {
        return Err(refuse(settings, log, subject, 0, OWNER_GONE));
    }
    match availability {
        Ok(Availability::Available) => hello(settings, subject, exception, consenter, owners, log, HELLO_MESSAGE_LIMIT),
        other => {
            let missing = match other {
                Ok(availability) => availability.name(),
                Err(reason) => reason,
            };
            let why = format!(" (Windows Hello was not available: {})", missing);
            if let Err(reason) = prompt.ready(&text) {
                return Err(refuse(settings, log, subject, 0, &format!("{}{}", reason, why)));
            }
            attempts(settings, subject, prompt, verifier, owners, log, &text, &why)
        }
    }
}

/// The consent of Windows Hello for a run: the check of the session user and the desktop, one more look at the starter, one
/// request, and what the answer decides. Only `Verified` approves. A refusal after the request was shown keeps the right to a
/// prompt for `COOL_DOWN_MS` more, as after any prompt; a request that had to be cancelled (no answer in time, the starter gone,
/// a cancel that was not taken) ends the process at once. A message of more than `limit` UTF-16 units is not sent.
pub(super) fn hello(settings: &Settings, subject: &Subject, exception: &Exception, consenter: &dyn Consenter, owners: &Owners, log: &Log, limit: usize) -> Result<(), GuardError> {
    let stop = |attempt: u32, reason: &str| refuse_by(settings, log, subject, HELLO_METHOD, attempt, reason);
    let refuse_shown = |reason: &str| {
        let error = stop(1, reason);
        pause(owners, cool_down_ms());
        error
    };
    if let Err(reason) = consenter.session_user(owners) {
        return Err(stop(0, &reason));
    }
    let (message, message_units) = hello_text_counted(subject, exception, settings.max_seconds);
    if message_units > limit {
        return Err(stop(0, "the message of the request for Windows Hello is longer than the system takes"));
    }
    // The starter may have left while the user and the message were looked at: no request is made for a run that nobody started.
    if owners.gone() {
        return Err(stop(0, OWNER_GONE));
    }
    match consenter.request(&message, owners) {
        Outcome::Done(code) => match Consent::from_code(code) {
            Consent::Verified => {
                let reason = format!("a cap of {} was approved: Windows Hello verified the signed-in user", hours_phrase(settings.max_seconds));
                match record_approval(settings, subject, HELLO_METHOD, 1, &reason, owners, log) {
                    Ok(()) => Ok(()),
                    Err(NotRecorded::OwnerGone) => Err(stop(1, OWNER_GONE)),
                    Err(unrecorded) => Err(refuse_shown(unrecorded.reason())),
                }
            }
            other => Err(refuse_shown(&format!("Windows Hello did not verify the user: {}", other.name()))),
        },
        Outcome::Cancelled => Err(refuse_shown("Windows Hello did not verify the user: the request was cancelled")),
        Outcome::Failed(code) => Err(refuse_shown(&format!("Windows Hello failed: 0x{:08x}", code))),
        Outcome::TimedOut => Err(stop(1, &format!("no answer within {} seconds, the request was cancelled", PROMPT_TIMEOUT_MS / 1000))),
        Outcome::OwnerGone => Err(stop(1, OWNER_GONE)),
        Outcome::CancelIgnored => Err(stop(1, "the request for Windows Hello was cancelled and did not end")),
    }
}

/// Whether the guard runs as the user who is signed in to its session. Windows Hello verifies the second, so the consent says
/// something about the first only when they are the same user: a guard started as another account (`runas`, a service account)
/// would otherwise borrow the Windows Hello of the user at the screen. `session` is the session of the guard, `signed_in` the
/// domain and the user name that the system gives for that session, `own` the name of the account the guard runs as
/// (`DOMAIN\user`), and `lookup` compares a name with the account of the guard when the two are not written alike (the same
/// look up as `account_check`: the security identifiers are compared). Nothing is approved when any of them is not known.
pub(super) fn session_user_matches(session: Option<u32>, signed_in: Option<(String, String)>, own: Option<String>, lookup: impl FnOnce(&str) -> Option<Verified>) -> Result<(), String> {
    match session {
        None => return Err("the session of the guard cannot be read, so the user signed in to it is not known".into()),
        Some(0) => return Err("the guard runs in session 0, in which no user is signed in".into()),
        Some(_) => {}
    }
    let Some((domain, user)) = signed_in.filter(|(domain, user)| !domain.is_empty() && !user.is_empty()) else {
        return Err("the user signed in to this session cannot be read".into());
    };
    let Some(own) = own.filter(|own| !own.is_empty()) else {
        return Err("the account the guard runs as cannot be read".into());
    };
    let signed_in = format!("{}\\{}", domain, user);
    if own.eq_ignore_ascii_case(&signed_in) {
        return Ok(());
    }
    // The names are the system's, but they reach the reason of a refusal, so they are shown as the prompt shows a name: written out
    // where a character is not plain, and cut.
    let (own_shown, signed_in_shown) = (shown(&own, ACCOUNT_LIMIT, Keep::End), shown(&signed_in, ACCOUNT_LIMIT, Keep::End));
    match lookup(&signed_in) {
        None => Ok(()),
        Some(Verified::OtherUser) => Err(format!("the guard runs as {}, which is not the user signed in to this session ({}), and Windows Hello verifies only the second; \
            start the run as the signed-in user", own_shown, signed_in_shown)),
        Some(_) => Err(format!("the user signed in to this session ({}) cannot be compared with the account the guard runs as ({})", signed_in_shown, own_shown)),
    }
}

/// The two looks that stand before a request for Windows Hello, in this order: the guard runs as the user signed in to its session,
/// and on the desktop that receives the input of that user; the second is not made when the first refuses. What is read from the
/// system is given in (`signed_in` for the names of the session, `lookup` for the comparison of two accounts by their security
/// identifiers, `desktops` for the names of the two desktops), so that a unit test can give a user that is not the guard's and see that
/// it is refused. `HelloConsenter::session_user` is the only caller, with the real ones (G102 scans for it).
pub(super) fn session_user_and_desktop(session: Option<u32>, signed_in: impl FnOnce(u32) -> Option<(String, String)>, own: Option<String>,
    lookup: impl FnOnce(&str) -> Option<Verified>, desktops: impl FnOnce() -> (Option<String>, Option<String>)) -> Result<(), String> {
    session_user_matches(session, session.and_then(signed_in), own, lookup)?;
    let (ours, input) = desktops();
    on_input_desktop(ours, input)
}

/// The domain and the user name of the user signed in to `session`, when the system gives them.
pub(super) fn signed_in_names(session: u32) -> Option<(String, String)> {
    let api = wts_api()?;
    let read = |class: u32| -> Option<String> {
        let (mut buffer, mut bytes): (*mut u16, u32) = (std::ptr::null_mut(), 0);
        // A null server is this computer.
        if unsafe { (api.query)(std::ptr::null_mut(), session, class, &mut buffer, &mut bytes) } == 0 || buffer.is_null() {
            return None;
        }
        // SAFETY: the system allocated `bytes` bytes at `buffer`, and the memory is freed once it has been read.
        let units = unsafe { std::slice::from_raw_parts(buffer, bytes as usize / std::mem::size_of::<u16>()) };
        let end = units.iter().position(|unit| *unit == 0).unwrap_or(units.len());
        let name = String::from_utf16_lossy(&units[..end]);
        unsafe { (api.free)(buffer as *mut c_void) };
        Some(name)
    };
    Some((read(WTS_DOMAIN_NAME)?, read(WTS_USER_NAME)?))
}

/// The session of this process, when the system gives it.
pub(super) fn own_session() -> Option<u32> {
    let mut session = 0u32;
    (unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) } != 0).then_some(session)
}

/// Whether the desktop that the guard runs on is the one that receives the input of the user, the one on the screen. The prompt of
/// Windows Hello is placed by the system, and a face can be matched with no act by the user, so a guard that a program of the
/// user started on a desktop that nobody is looking at (a desktop that the program made), or while the screen is locked or the
/// screen saver runs, would have the user verified without being shown what is asked (Common Weakness Enumeration, CWE-451, user
/// interface misrepresentation of critical information; National Institute of Standards and Technology (NIST) Special
/// Publication (SP) 800-63B-4, authentication intent). `ours` and `input` are the names of the two desktops, as the system gives
/// them; nothing is approved when one is not known.
pub(super) fn on_input_desktop(ours: Option<String>, input: Option<String>) -> Result<(), String> {
    match (ours, input) {
        (Some(ours), Some(input)) if !ours.is_empty() && ours.eq_ignore_ascii_case(&input) => Ok(()),
        (Some(ours), Some(input)) => Err(format!("the guard runs on the desktop {}, and the desktop that receives the input of the user is {}; \
            start the run on the desktop that the user is looking at", shown(&ours, ACCOUNT_LIMIT, Keep::Start), shown(&input, ACCOUNT_LIMIT, Keep::Start))),
        (None, _) => Err("the desktop of the guard cannot be read, so nobody can be shown the request".into()),
        (_, None) => Err("the desktop that receives the input of the user cannot be opened (the screen may be locked), so nobody can be shown the request".into()),
    }
}

/// The name of `desktop` (`UOI_NAME`), when the system gives it.
pub(super) fn desktop_name(api: &WindowStationApi, desktop: Handle) -> Option<String> {
    let mut name = [std::mem::MaybeUninit::<u16>::uninit(); 256];
    let mut needed = 0u32;
    let read = unsafe { (api.information)(desktop, UOI_NAME, name.as_mut_ptr() as *mut c_void, (name.len() * std::mem::size_of::<u16>()) as u32, &mut needed) };
    if read == 0 {
        return None;
    }
    // The call reports in `needed` the bytes it wrote, the end of the
    // name among them; the read is confined to exactly those units,
    // and a report that does not fit the buffer names nothing.
    let units = needed as usize / std::mem::size_of::<u16>();
    if units == 0 || units > name.len() {
        return None;
    }
    let name = unsafe { std::slice::from_raw_parts(name.as_ptr() as *const u16, units) };
    let end = name.iter().position(|unit| *unit == 0)?;
    Some(String::from_utf16_lossy(&name[..end]))
}

/// The names of the desktop of the calling thread and of the desktop that receives the input of the user, when the system gives them.
pub(super) fn desktop_names() -> (Option<String>, Option<String>) {
    let Some(api) = window_station_api() else { return (None, None) };
    // The desktop of the thread belongs to the process and is not closed; the input desktop is opened for the read of its name and is.
    let ours = unsafe { (api.thread_desktop)(GetCurrentThreadId()) };
    let ours = if ours.is_null() { None } else { desktop_name(api, ours) };
    let input = unsafe { (api.input_desktop)(0, 0, DESKTOP_READOBJECTS) };
    let input = if input.is_null() {
        None
    } else {
        let name = desktop_name(api, input);
        unsafe { (api.close_desktop)(input) };
        name
    };
    (ours, input)
}

/// The real consent of Windows Hello, through the Windows Runtime. Nothing here is linked: the libraries are loaded from the
/// system folder when it is first wanted, and where they are missing it is not available.
pub(crate) struct HelloConsenter;

impl Consenter for HelloConsenter {
    fn availability(&self, owners: &Owners) -> Result<Availability, String> {
        match bounded(Duration::from_millis(SYSTEM_TIMEOUT_MS), owners, read_availability) {
            Ok(answer) => answer,
            Err(Unanswered::TimedOut) => Err(format!("no answer within {} seconds", SYSTEM_TIMEOUT_MS / 1000)),
            Err(Unanswered::OwnerGone) => Err(OWNER_GONE.into()),
            Err(_) => Err("the question could not be asked".into()),
        }
    }

    fn session_user(&self, owners: &Owners) -> Result<(), String> {
        let compared = bounded(Duration::from_millis(SYSTEM_TIMEOUT_MS), owners, || {
            session_user_and_desktop(own_session(), signed_in_names, own_account_name(), account_check, desktop_names)
        });
        match compared {
            Ok(answer) => answer,
            Err(Unanswered::TimedOut) => Err(format!("the user signed in to this session was not found within {} seconds", SYSTEM_TIMEOUT_MS / 1000)),
            Err(Unanswered::OwnerGone) => Err(OWNER_GONE.into()),
            Err(_) => Err("the user signed in to this session could not be looked up".into()),
        }
    }

    fn request(&self, message: &str, owners: &Owners) -> Outcome {
        let message = message.to_string();
        request_with(owners, Duration::from_millis(PROMPT_TIMEOUT_MS), Duration::from_millis(CANCEL_GRACE_MS), move |stop| consent_request(&message, stop))
    }
}

/// Whether a message that was taken from the queue of the thread that serves the owner window is dispatched. A timer message is
/// not: the thread sets no timer, so every one is from another program, and `DispatchMessageW` is documented to call the function
/// that the `lParam` of a timer message names, in this process, for a message to a window and for one to the thread (no window).
/// Any program of the same user can post either kind to another process (a "shatter" attack; Common Weakness Enumeration, CWE-422,
/// unprotected Windows messaging channel). The system itself did not call it for a timer that the thread did not set on Windows 11
/// (build 10.0.26200; the unit test of the owner window records it), so this is defence in depth. Every other message goes to
/// `DefWindowProcW`, or is ignored when it has no window.
pub(super) fn dispatchable(message: &Msg) -> bool {
    message.message != WM_TIMER && message.message != WM_SYSTIMER
}

/// Ask Windows Hello whether it is available, on the worker, with its own limit: the operation is cancelled if it does not
/// answer in `SYSTEM_TIMEOUT_MS`, so that a worker that the caller has given up on ends. No window is made and nothing is shown.
pub(super) fn read_availability() -> Result<Availability, String> {
    let failed = |code: u32| format!("the question failed: 0x{:08x}", code);
    let api = hello_api().ok_or_else(|| HELLO_MISSING.to_string())?;
    let _apartment = Apartment::enter(api).map_err(failed)?;
    let class = HString::new(api, CONSENT_CLASS).map_err(failed)?;
    let statics = factory(api, &class, &IID_CONSENT_STATICS).map_err(failed)?;
    let mut raw: *mut c_void = std::ptr::null_mut();
    // SAFETY: slot 6 of IUserConsentVerifierStatics is CheckAvailabilityAsync.
    let code = unsafe { slot::<CheckAvailabilityAsyncFn>(statics.0, 6)(statics.0, &mut raw) };
    if code < 0 || raw.is_null() {
        return Err(failed(failure_of(code)));
    }
    // The operation as the interface whose result is read: asking for it also checks that the answer is of the kind expected.
    let typed = Com(raw).query(&IID_AVAILABILITY_OPERATION).map_err(failed)?;
    let operation = WinOperation::new(typed).map_err(failed)?;
    let until = Instant::now() + Duration::from_millis(SYSTEM_TIMEOUT_MS);
    let wait = || unsafe { Sleep(POLL_MS) };
    match drive(&operation, &AtomicBool::new(false), Some(until), Duration::from_millis(CANCEL_GRACE_MS), &wait) {
        Outcome::Done(code) => Ok(Availability::from_code(code)),
        Outcome::Failed(code) => Err(failed(code)),
        _ => Err(format!("no answer within {} seconds", SYSTEM_TIMEOUT_MS / 1000)),
    }
}

/// Ask for the consent, on the worker, and say how the request ended; a failure to make it is `Failed` with its error code.
pub(super) fn consent_request(message: &str, stop: &AtomicBool) -> Outcome {
    match consent_inner(message, stop) {
        Ok(outcome) => outcome,
        Err(code) => Outcome::Failed(code),
    }
}

/// The only place a request for Windows Hello is made (G102 scans for it). The window that owns the request is made first and
/// served while the request is open; the operation is cancelled when `stop` is set. Everything that was made is released on
/// every path, in the opposite order.
pub(super) fn consent_inner(message: &str, stop: &AtomicBool) -> Result<Outcome, u32> {
    let (Some(api), Some(windows)) = (hello_api(), window_api()) else { return Err(E_NOTIMPL) };
    let _apartment = Apartment::enter(api)?;
    let owner = OwnerWindow::create(windows)?;
    let text = HString::new(api, message)?;
    let class = HString::new(api, CONSENT_CLASS)?;
    let interop = factory(api, &class, &IID_CONSENT_INTEROP)?;
    let mut raw: *mut c_void = std::ptr::null_mut();
    // SAFETY: slot 6 of IUserConsentVerifierInterop is RequestVerificationForWindowAsync.
    let code = unsafe { slot::<RequestVerificationForWindowAsyncFn>(interop.0, 6)(interop.0, owner.handle, text.raw, &IID_RESULT_OPERATION, &mut raw) };
    if code < 0 || raw.is_null() {
        return Err(failure_of(code));
    }
    let operation = WinOperation::new(Com(raw))?;
    Ok(drive(&operation, stop, None, Duration::from_millis(CANCEL_GRACE_MS), &|| owner.serve()))
}
