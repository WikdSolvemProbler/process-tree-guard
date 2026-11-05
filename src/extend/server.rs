// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The extension server: a run's settings, and the running channel with its pipes, threads and deadline.

use super::*;


use crate::sys::windows::{

    ASSOCIATE_COMPLETION_PORT_CLASS, AssociateCompletionPort, CancelIoEx, CloseHandle,

    CreateEventW, CreateIoCompletionPort, Handle, INVALID_HANDLE, PostQueuedCompletionStatus,

    SetEvent, SetInformationJobObject, checked, os_error, secure_random_16,

};
/// What the run asks of its channel: the cap and the audit file. `exception` is set when the
/// cap is over the default, and the run then needs the approval of the signed-in user to start.
pub struct Settings {
    pub max_seconds: f64,
    pub log_path: Option<std::sync::Arc<Path>>,
    pub exception: Option<Exception>,
}

/// A running channel: the instances of the pipe, the threads that serve them and watch the job, and the deadline.
pub struct Extension {
    pub(super) shared: Arc<Shared>,
    pub(super) pipes: [Handle; INSTANCES as usize],
    pub(super) stop: Handle,
    pub(super) port: Handle,
    pub(super) wake: Handle,
    pub(super) servers: Vec<JoinHandle<()>>,
    pub(super) monitor: Option<JoinHandle<()>>,
    pub(super) started: u64,
    /// The name of the channel, for `PROCESS_TREE_GUARD_CHANNEL`.
    pub channel: String,
    /// Whether `shutdown` has already run: it is called explicitly
    /// at the end of a run and again by `Drop`, and the second call
    /// must not repeat the first's work (its clock read for the
    /// final notices, its lock passes and its log finish).
    pub(super) shutdown_done: bool,
}

/// The channel name of a run: the pipe prefix and 32 hexadecimal
/// characters from the system generator, in one string. Building
/// the hex text as its own string first and formatting the name
/// from it made a second allocation and a second copy of the 32
/// characters for a name this one buffer holds as well in place.
/// One string, built from the hex table: formatting each byte into
/// its own string first made sixteen allocations and sixteen
/// copies for a 32-character name.
pub(super) fn channel_name() -> Result<String, GuardError> {
    let mut bytes = [0u8; 16];
    secure_random_16(&mut bytes)?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(PIPE_PREFIX.len() + 32);
    out.push_str(PIPE_PREFIX);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(out)
}

impl Extension {
    /// Open the channel of a run whose own deadline is `timeout_seconds` from now.
    /// `job` is the run's job object, which stays open for as long as the extension. `log` is
    /// the log the run has already opened (it records the decision on an exception above the
    /// cap before the job exists); without one it is opened here. The log is finished on every
    /// path out of this function: by the extension when it ends, or here when it cannot start.
    pub fn start(job: Handle, settings: &Settings, timeout_seconds: f64, log: Option<Log>) -> Result<Extension, GuardError> {
        let log = match log {
            Some(log) => log,
            None => open_log(settings)?,
        };
        // A run that holds an approved exception says in the log when it then fails to start, so that the record does not
        // show a cap approved for a command that never ran.
        let announce = settings.exception.is_some();
        let channel = match channel_name() {
            Ok(channel) => channel,
            Err(error) => {
                if announce {
                    log.note(&not_started_text(&error));
                }
                log.finish(LOG_FLUSH_MS);
                return Err(error);
            }
        };
        let started = tick();
        let (wake, stop) = unsafe { (CreateEventW(std::ptr::null_mut(), 0, 0, std::ptr::null()), CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null())) };
        if wake.is_null() || stop.is_null() {
            let error = os_error();
            unsafe {
                for event in [wake, stop] {
                    if !event.is_null() {
                        CloseHandle(event);
                    }
                }
            }
            if announce {
                log.note(&not_started_text(&error));
            }
            log.finish(LOG_FLUSH_MS);
            return Err(error);
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State::new(started + millis(timeout_seconds), settings.max_seconds)),
            cap_seconds: settings.max_seconds,
            log,
            job: job as usize,
            wake: wake as usize,
            stopping: AtomicBool::new(false),
            serving: AtomicUsize::new(1),
        });
        // From here on a failure drops the extension, which closes whatever exists.
        let mut extension = Extension {
            shared: shared.clone(), pipes: [std::ptr::null_mut(); INSTANCES as usize], stop, port: std::ptr::null_mut(), wake, servers: Vec::new(), monitor: None, started, channel, shutdown_done: false,
        };
        let for_note = shared.clone();
        let made = (|| -> Result<(), GuardError> {
            extension.pipes = create_pipes(&extension.channel)?;
            unsafe {
                extension.port = CreateIoCompletionPort(INVALID_HANDLE, std::ptr::null_mut(), 0, 1);
                if extension.port.is_null() {
                    return Err(os_error());
                }
                let mut association = AssociateCompletionPort { key: 0, port: extension.port };
                checked(SetInformationJobObject(job, ASSOCIATE_COMPLETION_PORT_CLASS, &mut association as *mut _ as *mut c_void,
                    std::mem::size_of::<AssociateCompletionPort>() as u32))?;
            }
            let (port, stop_event) = (extension.port as usize, stop as usize);
            // One serve thread starts now, for the first instance; it starts
            // the thread of the next instance when a connection arrives, and
            // so on, so a run that is never asked for an extension keeps one
            // serve thread instead of one per instance.
            let first = extension.pipes[0];
            let mut rest = Rest::empty();
            for pipe in extension.pipes.iter().skip(1) {
                rest.slots[rest.len] = *pipe as usize;
                rest.len += 1;
            }
            // Both threads start behind their gates, as a successor
            // serve thread does in `spawn_next`: neither body runs —
            // the serve thread posts no accept and decides no grant —
            // until the starter has hardened the thread through its
            // creation handle, and a thread that cannot be hardened
            // is released dead and never serves.
            let (serving, first) = (shared.clone(), first as usize);
            let (server, server_gate) = spawn_gated("extension-server", move || serve(serving, first as Handle, stop_event as Handle, rest))?;
            extension.servers.push(server);
            let (monitor, monitor_gate) = match spawn_gated("extension-monitor", move || watch_job(&shared, port as Handle)) {
                Ok(pair) => pair,
                Err(error) => {
                    server_gate.release(false);
                    return Err(error);
                }
            };
            extension.monitor = Some(monitor);
            // Retain every thread in Extension before the release, so a
            // failing ACL operation leaves both gates to be released
            // dead below: the rollback that drops the extension then
            // signals stop and joins two threads that have already left.
            let hardened = extension.servers.iter().chain(extension.monitor.iter())
                .try_for_each(crate::cli_process::harden_worker);
            let live = hardened.is_ok();
            server_gate.release(live);
            monitor_gate.release(live);
            hardened?;
            Ok(())
        })();
        match made {
            Ok(()) => Ok(extension),
            Err(error) => {
                // The extension is dropped on return, which ends its threads and finishes the log: the line comes first.
                if announce {
                    for_note.log.note(&not_started_text(&error));
                }
                Err(error)
            }
        }
    }

    /// Put a line in the log saying what happened to the run, for the engine: a run with an approved exception whose command could
    /// not be started says so, before the log is finished.
    pub fn note(&self, text: &str) {
        self.shared.log.note(text);
    }

    /// The event the supervisor waits on besides the processes: set when a process
    /// in the job exits, so the deadline is recalculated without waiting for the backstop.
    pub fn wake(&self) -> Handle {
        self.wake
    }

    /// Milliseconds until the deadline of the run, recalculated now under the lock;
    /// None when it has passed, and the run then grants nothing more. A run that grants
    /// nothing more for another reason (its channel stopped serving) goes on to its deadline.
    pub fn remaining(&self) -> Option<u64> {
        let mut state = self.shared.lock();
        let now = tick();
        let passed = state.expire(now, Some(self.shared.job as Handle));
        for line in state.notices_lazy(now, false, &mut || unix_ms_now()) {
            self.shared.log.push(&line);
        }
        // The deadline of this pass is the one `expire` just computed
        // and kept: the hold list is not folded a second time here.
        if passed { None } else { Some(state.expire_deadline - now) }
    }

    /// A hold of `seconds` on the process `id`, as a request from it would make, for tests of the supervisor.
    #[cfg(test)]
    pub(crate) fn hold_for_test(&self, id: u32, seconds: f64) {
        let process = Process::open(id).expect("open the holder");
        let mut state = self.shared.lock();
        assert_eq!(state.grant(tick(), Some(self.shared.job as Handle), seconds, process, 0, None), Ok(None));
    }

    /// Stop granting: the command has finished, or the deadline has passed.
    pub fn close(&self) {
        self.shared.lock().closed = true;
    }

    /// Seconds since the run began.
    pub fn elapsed_seconds(&self) -> f64 {
        tick().saturating_sub(self.started) as f64 / 1000.0
    }

    /// Stop the threads, cancel what they wait for and close the handles.
    pub fn shutdown(&mut self) {
        if self.shutdown_done {
            return;
        }
        self.shutdown_done = true;
        self.close();
        self.shared.stopping.store(true, Ordering::SeqCst);
        unsafe {
            if !self.stop.is_null() {
                SetEvent(self.stop);
            }
            for pipe in &self.pipes {
                if !pipe.is_null() {
                    CancelIoEx(*pipe, std::ptr::null_mut());
                }
            }
            if !self.port.is_null() {
                PostQueuedCompletionStatus(self.port, 0, 0, std::ptr::null_mut());
            }
        }
        // These threads wait on the pipe and the port only, with the stop event to wake them,
        // and never on the log, so the join is bounded. A serve thread started by another is
        // joined by the one that started it, so the first thread's join covers them all.
        for thread in self.servers.drain(..).chain(self.monitor.take()) {
            let _ = thread.join();
        }
        {
            let mut state = self.shared.lock();
            for line in state.notices_lazy(tick(), true, &mut || unix_ms_now()) {
                self.shared.log.push(&line);
            }
            state.holds.clear();
        }
        // The last lines get a moment to be written. A destination that does not take
        // them (a standard error nobody reads) is left behind: the process is about to end.
        self.shared.log.finish(LOG_FLUSH_MS);
        unsafe {
            for pipe in &mut self.pipes {
                if !pipe.is_null() {
                    CloseHandle(*pipe);
                    *pipe = std::ptr::null_mut();
                }
            }
            for handle in [&mut self.stop, &mut self.port, &mut self.wake] {
                if !handle.is_null() {
                    CloseHandle(*handle);
                    *handle = std::ptr::null_mut();
                }
            }
        }
    }
}

impl Drop for Extension {
    fn drop(&mut self) {
        self.shutdown();
    }
}
