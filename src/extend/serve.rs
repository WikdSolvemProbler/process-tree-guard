// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Serving the channel: the pipe threads, their overlapped I/O, and the monitor that reports a job exit.

use super::*;


use crate::sys::windows::{

    CancelIoEx, CloseHandle, ConnectNamedPipe, CreateEventW, DisconnectNamedPipe,

    ERROR_IO_PENDING, ERROR_MORE_DATA, ERROR_PIPE_CONNECTED, GetLastError,

    GetNamedPipeClientProcessId, GetOverlappedResult, GetQueuedCompletionStatus, Handle, INFINITE,

    JOB_OBJECT_MSG_ABNORMAL_EXIT_PROCESS, JOB_OBJECT_MSG_EXIT_PROCESS, Overlapped, ReadFile,

    ResetEvent, SetEvent, Sleep, WAIT_OBJECT_0, WAIT_TIMEOUT, WaitForMultipleObjects,

    WaitForSingleObject, WriteFile, os_error,

};
mod decide;

pub(super) use decide::*;

// ---------------------------------------------------------------------- the threads

/// Tell the supervisor when a process in the job exits. The wait's timeout is
/// only a backstop for a lost packet: shutdown sets the stopping flag and posts
/// a packet to the port, which wakes this wait at once, so the long timeout
/// costs no shutdown latency beyond the bounded wait itself.
pub(super) fn watch_job(shared: &Shared, port: Handle) {
    while !shared.stopping.load(Ordering::SeqCst) {
        let (mut message, mut key, mut overlapped) = (0u32, 0usize, std::ptr::null_mut());
        let got = unsafe { GetQueuedCompletionStatus(port, &mut message, &mut key, &mut overlapped, 10_000) };
        if got != 0 && (message == JOB_OBJECT_MSG_EXIT_PROCESS || message == JOB_OBJECT_MSG_ABNORMAL_EXIT_PROCESS) {
            unsafe { SetEvent(shared.wake as Handle) };
        }
    }
}

/// The bytes of one client message, held inline: a message is at
/// most the 64 bytes `Io::read` reads, so it moves from the read
/// buffer into the decision by value, with no heap copy per request.
pub(super) struct MessageBytes {
    buffer: [u8; 64],
    length: u8,
}

impl MessageBytes {
    /// The form the decision's unit tests construct a message with;
    /// production messages move out of `Io::read`'s buffer directly.
    #[cfg(test)]
    pub(super) fn from_slice(bytes: &[u8]) -> Self {
        let mut buffer = [0u8; 64];
        buffer[..bytes.len()].copy_from_slice(bytes);
        MessageBytes { buffer, length: bytes.len() as u8 }
    }

    pub(super) fn as_slice(&self) -> &[u8] {
        &self.buffer[..self.length as usize]
    }
}

pub(super) enum Message {
    Bytes(MessageBytes),
    TooLong,
    Silent,
}

/// The pipe, the event its operations signal, and the event that ends the service.
pub(super) struct Io {
    pub(super) pipe: Handle,
    pub(super) event: Handle,
    pub(super) stop: Handle,
}

/// Finish an overlapped operation on `pipe`: wait for it at most `timeout_ms`, the
/// first handle of `wait` being the event it signals and any other a reason to stop
/// waiting, and cancel it and wait for the cancellation otherwise. The bytes moved, or
/// the Windows error (`WAIT_TIMEOUT` when it was cancelled). `started` is true when
/// the call completed at once, and the call is the last one made before this.
/// `observed` is the count that call itself wrote into its caller's out-variable:
/// when `started` is true the operation is already complete and counted, so that
/// count is the answer and no reap is paid to learn it again.
pub(super) fn finish_io(pipe: Handle, overlapped: &mut Overlapped, started: bool, observed: u32, wait: &[Handle], timeout_ms: u32) -> Result<u32, u32> {
    let mut transferred = 0u32;
    if !started {
        let error = unsafe { GetLastError() };
        if error != ERROR_IO_PENDING {
            return Err(error);
        }
        if unsafe { WaitForMultipleObjects(wait.len() as u32, wait.as_ptr(), 0, timeout_ms) } != WAIT_OBJECT_0 {
            // The operation may have completed in the instant before it was cancelled, and then it is done.
            let completed = unsafe {
                CancelIoEx(pipe, overlapped);
                GetOverlappedResult(pipe, overlapped, &mut transferred, 1)
            };
            return if completed != 0 { Ok(transferred) } else { Err(WAIT_TIMEOUT) };
        }
    }
    if started {
        return Ok(observed);
    }
    if unsafe { GetOverlappedResult(pipe, overlapped, &mut transferred, 0) } == 0 {
        return Err(unsafe { GetLastError() });
    }
    Ok(transferred)
}

impl Io {
    /// Finish an overlapped operation on the pipe: at most `timeout_ms`, or until
    /// the service is stopped.
    pub(super) fn finish(&self, overlapped: &mut Overlapped, started: bool, observed: u32, timeout_ms: u32) -> Result<u32, u32> {
        finish_io(self.pipe, overlapped, started, observed, &[self.event, self.stop], timeout_ms)
    }

    /// The client's message, waiting for it at most `timeout_ms`.
    pub(super) fn read(&self, timeout_ms: u32) -> Message {
        let mut buffer = [0u8; 64];
        let mut overlapped = Overlapped::with_event(self.event);
        unsafe { ResetEvent(self.event) };
        let mut read = 0u32;
        let started = unsafe { ReadFile(self.pipe, buffer.as_mut_ptr() as *mut c_void, buffer.len() as u32, &mut read, &mut overlapped) } != 0;
        match self.finish(&mut overlapped, started, read, timeout_ms) {
            Ok(n) => Message::Bytes(MessageBytes { buffer, length: n as u8 }),
            Err(ERROR_MORE_DATA) => Message::TooLong,
            Err(_) => Message::Silent,
        }
    }

    /// Send the reply and wait, at most `timeout_ms`, for the client to take it. The pipe has no
    /// outgoing buffer, so the write completes only when the client has read the message; one
    /// that is not read in time is cancelled, and nothing of it is left for the client to read.
    /// True when the client took the reply.
    pub(super) fn write(&self, reply: &Reply, timeout_ms: u32) -> bool {
        let bytes = encode_reply(reply);
        let mut overlapped = Overlapped::with_event(self.event);
        unsafe { ResetEvent(self.event) };
        let mut written = 0u32;
        let started = unsafe { WriteFile(self.pipe, bytes.as_ptr() as *const c_void, bytes.len() as u32, &mut written, &mut overlapped) } != 0;
        self.finish(&mut overlapped, started, written, timeout_ms) == Ok(bytes.len() as u32)
    }
}

/// One instance of the pipe can no longer be served. When it was the last, the channel is
/// gone: nothing more is granted, and the log says so. The holds already granted stand, and
/// the run ends at its deadline as it would have.
pub(super) fn stopped_serving(shared: &Shared) {
    if shared.serving.fetch_sub(1, Ordering::SeqCst) != 1 {
        return;
    }
    shared.lock().closed = true;
    let line = log_line(unix_ms_now(), &Entry {
        pid: std::process::id(), image: &image_of(std::process::id()), requested: None, granted: None, reason: "the channel stopped serving; no further request is granted",
    });
    // A decision line, not a notice: the decision entry point, with
    // its waited standard-error fallback when the audit file's sink
    // refused it.
    shared.log.push_decision_waited(&line, RECORD_MS);
}

/// Wait for a client to connect to `pipe` (or for `stop`). When one has, the moment this
/// thread learns of it, taken as soon as it does, in the units of a process's creation time:
/// the client connected at that moment or earlier, and how much earlier is the time this
/// thread took to run (the whole time since the connect when the client was there before the
/// call). None when the wait ended without a client.
pub(super) fn accept(pipe: Handle, event: Handle, stop: Handle) -> Option<u64> {
    let mut overlapped = Overlapped::with_event(event);
    unsafe { ResetEvent(event) };
    if unsafe { ConnectNamedPipe(pipe, &mut overlapped) } != 0 {
        return Some(filetime_now());
    }
    match unsafe { GetLastError() } {
        ERROR_PIPE_CONNECTED => Some(filetime_now()),
        ERROR_IO_PENDING => {
            let handles = [event, stop];
            let mut transferred = 0u32;
            if unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) } == WAIT_OBJECT_0 {
                let connected_at = filetime_now();
                (unsafe { GetOverlappedResult(pipe, &mut overlapped, &mut transferred, 0) } != 0).then_some(connected_at)
            } else {
                unsafe {
                    CancelIoEx(pipe, &mut overlapped);
                    GetOverlappedResult(pipe, &mut overlapped, &mut transferred, 1);
                }
                None
            }
        }
        _ => None,
    }
}

/// The instances that have no serve thread yet, as a fixed list in
/// stack storage: there are never more than `INSTANCES - 1` of them,
/// so the list a serve thread carries needs no heap vector — the
/// vector form allocated once when the extension started and once
/// more for the tail of the list at the first connection, for a
/// count the constant already bounds.
#[derive(Clone, Copy)]
pub(super) struct Rest {
    pub(super) slots: [usize; (INSTANCES - 1) as usize],
    pub(super) len: usize,
}

impl Rest {
    pub(super) fn empty() -> Rest {
        Rest { slots: [0; (INSTANCES - 1) as usize], len: 0 }
    }

    /// This list without its first instance: the next serve thread's
    /// own unserved instances.
    fn tail(&self) -> Rest {
        let mut rest = Rest { slots: [0; (INSTANCES - 1) as usize], len: self.len - 1 };
        rest.slots[..rest.len].copy_from_slice(&self.slots[1..self.len]);
        rest
    }
}

/// The gate of a thread started by `spawn_gated`: the thread waits
/// on the gate's event before it runs its body, and `release` is
/// what lets it — live, to run the body, or dead, to end without
/// running it.
pub(super) struct Gate {
    go: Handle,
    live: Arc<AtomicBool>,
}

impl Gate {
    pub(super) fn release(&self, live: bool) {
        self.live.store(live, Ordering::SeqCst);
        unsafe { SetEvent(self.go) };
    }
}

/// Start a thread behind a gate, in the shape of the gate
/// `spawn_next` builds for a successor serve thread: the thread
/// waits for its gate before it is given any work, so the starter
/// can harden it through its creation handle first — as every
/// other worker of a process-owning invocation is hardened before
/// it is given any payload — and a thread that cannot be hardened
/// is released dead and never runs its body. The gate event is
/// closed by the thread itself once it has waited on it.
pub(super) fn spawn_gated(name: &str, body: impl FnOnce() + Send + 'static) -> Result<(JoinHandle<()>, Gate), GuardError> {
    let go = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
    if go.is_null() {
        return Err(os_error());
    }
    let live = Arc::new(AtomicBool::new(false));
    let (thread_live, gate) = (live.clone(), go as usize);
    let spawned = std::thread::Builder::new().name(name.into()).spawn(move || {
        let go = gate as Handle;
        unsafe {
            WaitForSingleObject(go, INFINITE);
            CloseHandle(go);
        }
        if thread_live.load(Ordering::SeqCst) {
            body();
        }
    });
    match spawned {
        Ok(handle) => Ok((handle, Gate { go, live })),
        Err(error) => {
            unsafe { CloseHandle(go) };
            Err(GuardError(format!("cannot start the extension thread: {}", error)))
        }
    }
}

/// Start the serve thread of the instance `pipe`, which takes `rest` as its
/// own unserved instances. The thread waits at a gate until it has been
/// hardened, as the threads started with the run are hardened before the run
/// proceeds: a thread that cannot be hardened accounts for its instance as
/// stopped serving and never serves. None when the thread cannot be started
/// at all: the instance then stays unserved, a client that reaches it waits
/// as a client of a busy channel waits, and the next connection taken by the
/// thread that tried tries the start again.
fn spawn_next(shared: &Arc<Shared>, pipe: usize, stop: usize, rest: Rest) -> Option<JoinHandle<()>> {
    let go = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
    if go.is_null() {
        return None;
    }
    let live = Arc::new(AtomicBool::new(false));
    let (thread_shared, thread_live, gate) = (shared.clone(), live.clone(), go as usize);
    let spawned = std::thread::Builder::new().name("extension-server".into()).spawn(move || {
        let go = gate as Handle;
        unsafe {
            WaitForSingleObject(go, INFINITE);
            CloseHandle(go);
        }
        if !thread_live.load(Ordering::SeqCst) {
            stopped_serving(&thread_shared);
            return;
        }
        serve(thread_shared, pipe as Handle, stop as Handle, rest);
    });
    match spawned {
        Ok(handle) => {
            shared.serving.fetch_add(1, Ordering::SeqCst);
            if crate::cli_process::harden_worker(&handle).is_ok() {
                live.store(true, Ordering::SeqCst);
            }
            unsafe { SetEvent(go) };
            Some(handle)
        }
        Err(_) => {
            unsafe { CloseHandle(go) };
            None
        }
    }
}

/// Serve one instance of the pipe. `rest` names the instances that have no
/// thread yet: the first connection this thread takes starts the thread of
/// the next instance, before the connection is handled, and that thread
/// starts the one after in its turn, so the fleet grows by one thread per
/// concurrent connection up to INSTANCES and a run nobody asks keeps only
/// this thread. The thread started here is joined before this one ends, so
/// shutdown's join of the first thread joins them all.
pub(super) fn serve(shared: Arc<Shared>, pipe: Handle, stop: Handle, mut rest: Rest) {
    let event = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
    if event.is_null() {
        stopped_serving(&shared);
        return;
    }
    let mut failures = 0;
    let mut gave_up = false;
    let mut child: Option<JoinHandle<()>> = None;
    while !shared.stopping.load(Ordering::SeqCst) {
        if let Some(connected_at) = accept(pipe, event, stop) {
            failures = 0;
            if child.is_none() && rest.len > 0 {
                if let Some(started) = spawn_next(&shared, rest.slots[0], stop as usize, rest.tail()) {
                    child = Some(started);
                    rest.len = 0;
                }
            }
            handle_client(&shared, &Io { pipe, event, stop }, connected_at);
        } else {
            // A wait that ended because the service is stopping has
            // nothing to retry and no client to disconnect: counting
            // it as a failure, sleeping on it and disconnecting an
            // instance no client reached only delayed the join that
            // shutdown is waiting for, on every extendable run's
            // teardown. Genuine failures keep the count, the pause
            // and the disconnect below.
            if shared.stopping.load(Ordering::SeqCst) {
                break;
            }
            failures += 1;
            if failures > 100 {
                gave_up = true;
                break;
            }
            unsafe { Sleep(10) };
        }
        unsafe { DisconnectNamedPipe(pipe) };
    }
    unsafe { CloseHandle(event) };
    if gave_up {
        stopped_serving(&shared);
    }
    if let Some(child) = child {
        let _ = child.join();
    }
}

/// Serve one connection. Who the client is, and what the run makes of it, is settled before
/// a byte of its message is waited for, and the time spent on a connection is bounded: a
/// client that stalls costs this instance at most a few hundred milliseconds, is charged
/// to its share of the rate limit like any request, and cannot keep the other instances
/// from serving the rest.
pub(super) fn handle_client(shared: &Shared, io: &Io, connected_at: u64) {
    let mut id = 0u32;
    if unsafe { GetNamedPipeClientProcessId(io.pipe, &mut id) } == 0 {
        id = 0;
    }
    let mut arrival = arrive(shared, id);
    let (read_ms, take_ms) = match (&arrival.shut, arrival.in_job) {
        (Some(_), false) => {
            // A client outside the job whose allowance is used up (or whose run has ended) is
            // disconnected at once: its message is not waited for and it is not answered.
            let (_, line, _) = decide(shared, arrival, connected_at, &Message::Silent);
            if let Some(line) = line {
                shared.log.push_decision_waited(&line, RECORD_MS);
            }
            return;
        }
        (Some(_), true) => (REFUSED_READ_MS, REFUSED_TAKE_MS),
        (None, _) => (READ_MS, TAKE_MS),
    };
    let message = io.read(read_ms);
    if shared.stopping.load(Ordering::SeqCst) {
        return;
    }
    if let (Message::Silent, None) = (&message, &arrival.shut) {
        // The client was let in and sent nothing: it has used a request of its share, and the
        // log says so (as a refusal, within the budget of refusal lines). There is nothing to answer.
        // The budget is asked first, as `decide` asks it for a refusal: a line the
        // budget withholds is never built — building it first (the reason text, the
        // escaped line, and a wall-clock sample for its time) was work paid for
        // every silent client in a flood only to be thrown away.
        // The budget's answer is taken into a value first, so the
        // state lock is released before the line is built and
        // queued: a lock guard formed in the `if` condition itself
        // would live for the whole body, holding the state lock
        // across the format, the wall-clock sample and the log
        // queue's own locks on every silent client.
        let allowed = shared.lock().budget.allow(tick());
        if allowed {
            let reason = format!("no request arrived within {} milliseconds", READ_MS);
            let line = log_line(unix_ms_now(), &Entry { pid: arrival.id, image: arrival.image.as_str(), requested: None, granted: None, reason: &reason });
            // A refusal line, not a notice: it travels by the decision
            // entry point, with its waited standard-error fallback when
            // the audit file's sink refused it, as every other refusal
            // line of this server does.
            shared.log.push_decision_waited(&line, RECORD_MS);
        }
        return;
    }
    // The client is still there, with its end open, or it is refused as not verified.
    // The peek is paid only for a message the decision can read the
    // answer of: a message that is not bytes (nothing arrived, or too
    // much of it) decodes to no request, and `evaluate` returns for
    // that — on the refusal its `shut` already carries, or as
    // malformed — before it ever reads `gone`, so asking the pipe
    // about those connections bought a call no verdict used.
    arrival.gone = matches!(message, Message::Bytes(_)) && client_gone(io.pipe);
    let (mut reply, line, mut back) = decide(shared, arrival, connected_at, &message);
    // The line is queued, never written here: nothing the log is written to can hold up
    // the answers, and so nothing it is written to can hold up the deadline. The one
    // exception is a grant in a run with no audit file: standard error is then the only
    // record, and the grant stands only once its line is written (see `Log::record`).
    if let Some(line) = line {
        let stands = if back.is_some() { shared.log.record(&line, RECORD_MS) } else { shared.log.push_decision_waited(&line, RECORD_MS) };
        if !stands {
            if let Some(back) = back.take() {
                // The withdraw's answer is taken into a value first,
                // as at the silent-client budget above: the wake and
                // the log queue run after the state lock is released,
                // not under a condition-formed guard.
                let withdrawn = shared.lock().finish_grant(&back, false);
                if withdrawn {
                    unsafe { SetEvent(shared.wake as Handle) };
                    shared.log.push_decision_waited(&unrecorded_line(unix_ms_now(), &back, RECORD_MS), RECORD_MS);
                }
                // The cap is read from the shared copy that is
                // fixed for the run's life (see `Shared`), the copy
                // `evaluate` already reads without the lock: taking
                // the state lock for this one field again, straight
                // after the withdraw's lock above, bought a second
                // serialisation with the supervisor for a value that
                // cannot change under the run.
                let cap = shared.cap_seconds;
                reply = Reply::Refused { refusal: Refusal::AuditBlocked, cap };
            }
        }
    }
    let delivered = io.write(&reply, take_ms);
    if let Some(back) = back {
        // Resolve the reservation on success and on failure, including shutdown.
        // Until delivery a shorter renewal leaves the old deadline in effect.
        let settled = shared.lock().finish_grant(&back, delivered);
        if settled && (!delivered || back.previous.is_some_and(|until| until > back.until)) {
            unsafe { SetEvent(shared.wake as Handle) };
            if !delivered && !shared.stopping.load(Ordering::SeqCst) {
                shared.log.push_decision_waited(&withdrawn_line(unix_ms_now(), &back, take_ms), RECORD_MS);
            }
        }
    }
}
