// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The Windows Runtime operation of a request for Windows Hello: the thread it is waited on, the loop that drives it, and the window that owns it.

use super::*;


use crate::sys::windows::{


    ASYNC_CANCELED, ASYNC_COMPLETED, ASYNC_ERROR, ASYNC_STARTED, CallFn, Com,


    ERROR_CLASS_ALREADY_EXISTS, E_FAIL, E_OUTOFMEMORY, E_UNEXPECTED, GetLastError,


    GetModuleHandleW, GetValueFn, Guid, HString, Handle, HelloApi, IID_ASYNC_INFO,


    MWMO_INPUTAVAILABLE, Msg, PM_REMOVE, QS_ALLINPUT, RO_INIT_MULTITHREADED, Sleep, WAIT_FAILED,


    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WindowApi, WndClassExW, failure_of, slot, wide,


};
/// Run `work` on a thread of its own and wait for its outcome for at most `timeout`, looking at `owners` every
/// `OWNER_SLICE_MS`. When the time is up or a starter has exited, `work` is told to stop through the flag it is given (it is to
/// cancel the request) and is waited for `grace` more. The thread is left if it does not report, and ends with the process,
/// which the refusal that follows ends at once.
pub(in crate::auth) fn request_with(owners: &Owners, timeout: Duration, grace: Duration, work: impl FnOnce(&AtomicBool) -> Outcome + Send + 'static) -> Outcome {
    let stop = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = std::sync::mpsc::channel();
    let flag = stop.clone();
    // The request is handed to the thread only after the thread
    // has been hardened through its creation handle, as every
    // other worker of a process-owning invocation is hardened:
    // the thread waits for its one job, and a thread that cannot
    // be hardened is never given the consent request (the job
    // channel closes with the return and the thread ends).
    let (job_sender, job_receiver) = std::sync::mpsc::channel::<Box<dyn FnOnce(&AtomicBool) -> Outcome + Send>>();
    let started = std::thread::Builder::new().name("consent".into()).spawn(move || {
        if let Ok(work) = job_receiver.recv() {
            let _ = sender.send(work(&flag));
        }
    });
    let thread = match started {
        Ok(thread) => thread,
        Err(_) => return Outcome::Failed(E_OUTOFMEMORY),
    };
    if crate::cli_process::harden_worker(&thread).is_err() {
        return Outcome::Failed(E_OUTOFMEMORY);
    }
    if job_sender.send(Box::new(work)).is_err() {
        return Outcome::Failed(E_FAIL);
    }
    match wait_for(&receiver, timeout, owners) {
        Ok(outcome) => outcome,
        Err(Unanswered::Ended) | Err(Unanswered::NotStarted) => Outcome::Failed(E_FAIL),
        Err(cause) => {
            stop.store(true, Ordering::SeqCst);
            let late = receiver.recv_timeout(grace);
            match (cause, late) {
                (Unanswered::OwnerGone, _) => Outcome::OwnerGone,
                (_, Ok(Outcome::CancelIgnored)) | (_, Err(_)) => Outcome::CancelIgnored,
                _ => Outcome::TimedOut,
            }
        }
    }
}

/// An asynchronous operation of the Windows Runtime, as the loop that waits for it uses it. The real one reads the v-table of
/// `IAsyncInfo`; the unit tests give a scripted one.
pub(in crate::auth) trait Operation {
    /// The state of the operation (`AsyncStatus`); an error is the HRESULT of the call.
    fn status(&self) -> Result<i32, u32>;
    /// The HRESULT an operation in the state `ASYNC_ERROR` failed with.
    fn error(&self) -> u32;
    /// The result of an operation in the state `ASYNC_COMPLETED`; an error is the HRESULT of the call.
    fn result(&self) -> Result<i32, u32>;
    /// Ask the operation to cancel.
    fn cancel(&self);
}

/// Read the state of `operation` every time `step` returns, until it ends. `stop`, or the time `until`, makes the loop cancel
/// the operation, once, and wait `grace` for the state to leave `ASYNC_STARTED`. The outcome is `TimedOut` when the operation
/// reached `ASYNC_CANCELED` after it was asked to, `Cancelled` when it did so by itself, and `CancelIgnored` when it was
/// still started once the grace was over. A state that cannot be read, or is not one of the four, ends the loop with a failure
/// after one attempt to cancel, so that a refusal does not leave a request open that nothing watches any more (CWE-459,
/// incomplete cleanup).
pub(in crate::auth) fn drive(operation: &dyn Operation, stop: &AtomicBool, until: Option<Instant>, grace: Duration, step: &dyn Fn()) -> Outcome {
    let mut cancelled: Option<Instant> = None;
    loop {
        match operation.status() {
            Err(code) => {
                if cancelled.is_none() {
                    operation.cancel();
                }
                return Outcome::Failed(code);
            }
            Ok(ASYNC_COMPLETED) => {
                return match operation.result() {
                    Ok(code) => Outcome::Done(code),
                    Err(code) => Outcome::Failed(code),
                };
            }
            Ok(ASYNC_ERROR) => return Outcome::Failed(operation.error()),
            Ok(ASYNC_CANCELED) => return if cancelled.is_some() { Outcome::TimedOut } else { Outcome::Cancelled },
            Ok(ASYNC_STARTED) => {}
            Ok(_) => {
                if cancelled.is_none() {
                    operation.cancel();
                }
                return Outcome::Failed(E_UNEXPECTED);
            }
        }
        let stopping = stop.load(Ordering::SeqCst) || until.is_some_and(|until| Instant::now() >= until);
        match cancelled {
            None if stopping => {
                operation.cancel();
                cancelled = Some(Instant::now());
            }
            Some(at) if at.elapsed() >= grace => return Outcome::CancelIgnored,
            _ => {}
        }
        step();
    }
}

/// An operation of the Windows Runtime that is read through `IAsyncInfo`. Dropped, it is closed and released.
pub(in crate::auth) struct WinOperation {
    pub(in crate::auth) operation: Com,
    pub(in crate::auth) info: Com,
}

impl WinOperation {
    pub(in crate::auth) fn new(operation: Com) -> Result<WinOperation, u32> {
        let info = operation.query(&IID_ASYNC_INFO)?;
        Ok(WinOperation { operation, info })
    }
}

impl Operation for WinOperation {
    fn status(&self) -> Result<i32, u32> {
        let mut status = ASYNC_STARTED;
        // SAFETY: slot 7 of IAsyncInfo is get_Status.
        let code = unsafe { slot::<GetValueFn>(self.info.0, 7)(self.info.0, &mut status) };
        if code < 0 { Err(code as u32) } else { Ok(status) }
    }

    fn error(&self) -> u32 {
        let mut code = E_FAIL as i32;
        // SAFETY: slot 8 of IAsyncInfo is get_ErrorCode.
        unsafe { slot::<GetValueFn>(self.info.0, 8)(self.info.0, &mut code) };
        // A code that is no failure (0 or a success code) names none, as for a call that gave no object.
        failure_of(code)
    }

    fn result(&self) -> Result<i32, u32> {
        let mut result = NO_RESULT;
        // SAFETY: slot 8 of IAsyncOperation is GetResults, read only once the status is Completed.
        let code = unsafe { slot::<GetValueFn>(self.operation.0, 8)(self.operation.0, &mut result) };
        if code < 0 { Err(code as u32) } else { Ok(result) }
    }

    fn cancel(&self) {
        // SAFETY: slot 9 of IAsyncInfo is Cancel.
        unsafe { slot::<CallFn>(self.info.0, 9)(self.info.0) };
    }
}

impl Drop for WinOperation {
    fn drop(&mut self) {
        // SAFETY: slot 10 of IAsyncInfo is Close. It fails for an operation that is still started, which is left to the process.
        unsafe { slot::<CallFn>(self.info.0, 10)(self.info.0) };
    }
}

/// A Windows Runtime apartment on this thread, left when it is dropped (`RoInitialize` succeeds with S_OK and with S_FALSE, and
/// both need the call back).
pub(in crate::auth) struct Apartment(&'static HelloApi);

impl Apartment {
    pub(in crate::auth) fn enter(api: &'static HelloApi) -> Result<Apartment, u32> {
        let code = unsafe { (api.initialize)(RO_INIT_MULTITHREADED) };
        if code < 0 { Err(code as u32) } else { Ok(Apartment(api)) }
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { (self.0.uninitialize)() };
    }
}

/// The activation factory of the class `class`, through the interface `interface`.
pub(in crate::auth) fn factory(api: &HelloApi, class: &HString, interface: &Guid) -> Result<Com, u32> {
    let mut found: *mut c_void = std::ptr::null_mut();
    let code = unsafe { (api.factory)(class.raw, interface, &mut found) };
    if code < 0 || found.is_null() { Err(failure_of(code)) } else { Ok(Com(found)) }
}

/// The HRESULT for a Windows error code.
fn hresult_of(code: u32) -> u32 {
    if code == 0 { E_FAIL } else { 0x8007_0000 | (code & 0xffff) }
}

/// The window that owns the request, which a program of the desktop needs: a tool window of zero size, with the procedure of
/// the system, that is never shown and never takes the focus. Whether Windows Hello puts its dialog in front of the windows
/// of the user with an owner that is not shown is recorded by step 12 of the manual check. Dropped, it is destroyed, and its
/// class is removed.
pub(in crate::auth) struct OwnerWindow {
    pub(in crate::auth) api: &'static WindowApi,
    pub(in crate::auth) handle: Handle,
    pub(in crate::auth) class: Vec<u16>,
    pub(in crate::auth) instance: Handle,
}

impl OwnerWindow {
    pub(in crate::auth) fn create(api: &'static WindowApi) -> Result<OwnerWindow, u32> {
        let class = wide(OWNER_CLASS);
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let description = WndClassExW {
            size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            procedure: api.default_procedure,
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: std::ptr::null_mut(),
            cursor: std::ptr::null_mut(),
            background: std::ptr::null_mut(),
            menu: std::ptr::null(),
            class_name: class.as_ptr(),
            icon_small: std::ptr::null_mut(),
        };
        let registered = unsafe { (api.register)(&description) } != 0;
        let code = unsafe { GetLastError() };
        if !registered && code != ERROR_CLASS_ALREADY_EXISTS {
            return Err(hresult_of(code));
        }
        let caption = wide(CAPTION);
        let handle = unsafe {
            (api.create)(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, class.as_ptr(), caption.as_ptr(), WS_POPUP, 0, 0, 0, 0,
                std::ptr::null_mut(), std::ptr::null_mut(), instance, std::ptr::null_mut())
        };
        if handle.is_null() {
            let code = unsafe { GetLastError() };
            unsafe { (api.unregister)(class.as_ptr(), instance) };
            return Err(hresult_of(code));
        }
        Ok(OwnerWindow { api, handle, class, instance })
    }

    /// Dispatch the messages that wait for the window, at most `PUMP_LIMIT` at a time: a program that floods the window with
    /// posted messages cannot keep the loop that waits for the request from looking at its stop flag and cancelling (CWE-834).
    /// A message that another process sends (not posts) is handled by the system when the queue is read and is not counted; a
    /// flood of those can delay the look at the request, and then the caller gives up on the cancel after its grace and refuses
    /// the run, so it costs availability and never approves. A message that is not `dispatchable` is taken from the queue and dropped.
    pub(in crate::auth) fn pump(&self) {
        let mut message = Msg { window: std::ptr::null_mut(), message: 0, w: 0, l: 0, time: 0, point: [0; 2] };
        for _ in 0..PUMP_LIMIT {
            if unsafe { (self.api.peek)(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } == 0 {
                break;
            }
            if dispatchable(&message) {
                unsafe { (self.api.dispatch)(&message) };
            }
        }
    }

    /// Serve the window for `POLL_MS`, or until a message comes, and dispatch at most `PUMP_LIMIT` messages: the wait returns
    /// at once when a message is already in the queue, so one pass after it is all that a look at the request allows. A wait that
    /// fails (the desktop of the thread is gone, say) also returns at once, so the time is slept instead: the loop that waits for
    /// the request must not spin for up to 300 seconds, calling into another process on every turn (CWE-834).
    pub(in crate::auth) fn serve(&self) {
        if unsafe { (self.api.wait)(0, std::ptr::null(), POLL_MS, QS_ALLINPUT, MWMO_INPUTAVAILABLE) } == WAIT_FAILED {
            unsafe { Sleep(POLL_MS) };
        }
        self.pump();
    }
}

impl Drop for OwnerWindow {
    fn drop(&mut self) {
        unsafe {
            (self.api.destroy)(self.handle);
            (self.api.unregister)(self.class.as_ptr(), self.instance);
        }
    }
}
