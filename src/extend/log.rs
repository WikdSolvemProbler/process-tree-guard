// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The audit log of an extendable run: its sinks, their threads, and the file that is turned over at its limit.

use super::*;


use crate::sys::windows::{

    CloseHandle, CreateEventW, FILE_TYPE_DISK, FILE_TYPE_PIPE, GetFileType, Handle, INFINITE,

    INVALID_HANDLE, SetEvent, WaitForSingleObject,

};
mod audit;
mod format;

pub(super) use audit::AuditFile;
pub(crate) use format::*;

/// How far a sink's thread has got: the lines it has finished
/// writing, and how many of those writes failed. The failure count
/// is what a waited record is decided by (see `Sink::wait_recorded`):
/// it moves only when a write fails, so a failure of the waited
/// line is still visible after a later line has been written,
/// where a flag holding only the last write's outcome is not.
pub(super) struct Progress {
    pub(super) written: usize,
    pub(super) failed: usize,
}

/// What a sink's thread runs when it is started: the receiving end of the
/// queue, the writing closure and its idle companion, the signal that the
/// thread has ended, and the thread's name. A sink is made with these held,
/// not with a thread: the thread starts on the first line queued, so a run
/// that records nothing starts no log thread at all.
pub(super) struct Pending {
    pub(super) lines: Receiver<Arc<str>>,
    /// Write one line; true when the line was written. A line's own
    /// outcome is what a waited record reads (see `Sink::wait_recorded`):
    /// the outcome of a later line must never stand in for it.
    pub(super) write: Box<dyn FnMut(&str) -> bool + Send>,
    pub(super) idle: Option<Idle>,
    pub(super) done: std::sync::mpsc::Sender<()>,
    pub(super) name: String,
}

/// One copy of the log: a bounded queue of lines and the thread that writes them. A
/// destination that stops taking lines holds up its own thread and nothing else; the
/// lines wait in the queue, and when it is full they are dropped and counted.
pub(super) struct Sink {
    pub(super) queue: Mutex<Option<SyncSender<Arc<str>>>>,
    /// The lines queued so far, and (with a signal for whoever waits) the lines the thread has finished writing.
    pub(super) queued: AtomicUsize,
    pub(super) progress: Arc<(Mutex<Progress>, Condvar)>,
    pub(super) dropped: Arc<AtomicUsize>,
    pub(super) finished: Mutex<Receiver<()>>,
    pub(super) thread: Mutex<Option<JoinHandle<()>>>,
    /// The thread's starting materials, until the first line queued starts it.
    pub(super) pending: Mutex<Option<Pending>>,
    /// Whether the starting materials have been taken: set under the
    /// pending lock at the moment they are taken, and read first by
    /// every later `ensure_started`, so a sink that is already
    /// started (or already failed to start) pays one atomic load per
    /// queued line instead of a lock of the pending slot whose take
    /// could only report that the materials are gone.
    pub(super) started: AtomicBool,
}

/// Write the notice for the lines dropped since the last one, if any.
pub(super) fn report_dropped(dropped: &AtomicUsize, write: &mut dyn FnMut(&str)) {
    let count = dropped.swap(0, Ordering::SeqCst);
    if count > 0 {
        write(&dropped_line(unix_ms_now(), count));
    }
}

/// What a destination does besides taking a line: called with `false` when no line has come
/// for `RETRY_MS` (so a destination that failed is tried again without waiting for the next
/// line), and once with `true` when the sink has taken its last line.
pub(super) type Idle = Box<dyn FnMut(bool) + Send>;

impl Sink {
    pub(super) fn start(name: &str, write: Box<dyn FnMut(&str) -> bool + Send>, capacity: usize) -> Result<Sink, GuardError> {
        Sink::start_with(name, write, None, capacity)
    }

    pub(super) fn start_with(name: &str, write: Box<dyn FnMut(&str) -> bool + Send>, idle: Option<Idle>, capacity: usize) -> Result<Sink, GuardError> {
        let (sender, lines) = sync_channel::<Arc<str>>(capacity);
        let (done, finished) = std::sync::mpsc::channel();
        let dropped = Arc::new(AtomicUsize::new(0));
        let progress = Arc::new((Mutex::new(Progress { written: 0, failed: 0 }), Condvar::new()));
        let pending = Pending { lines, write, idle, done, name: name.into() };
        Ok(Sink { queue: Mutex::new(Some(sender)), queued: AtomicUsize::new(0), progress, dropped, finished: Mutex::new(finished), thread: Mutex::new(None), pending: Mutex::new(Some(pending)), started: AtomicBool::new(false) })
    }

    /// Start the sink's thread, the first time a line is queued for it. The
    /// thread waits behind a gate until it has been hardened through its
    /// creation handle, as every other worker of a process-owning
    /// invocation is hardened before it is given any payload: lines a
    /// racing caller queues in the meantime wait in the queue, and no
    /// line is written before the gate is released. A thread that cannot
    /// be started, or cannot be hardened, ends the sink instead: it is
    /// released dead and writes nothing, and the queue is closed, so the
    /// line that came and every later one are refused as the lines of a
    /// closed queue are — the failure the start of the run once reported
    /// before it began, now met by the first record, whose own refusal
    /// paths already carry it.
    fn ensure_started(&self) {
        if self.started.load(Ordering::SeqCst) {
            return;
        }
        let Some(mut pending) = relock(&self.pending).take() else { return };
        // The materials are taken: no later call can start the sink
        // again, whatever the spawn below does, so the flag is set
        // here, before it. A caller that raced this one already
        // observed the same outcome the pending take gave it (the
        // materials gone), exactly as it did before the flag.
        self.started.store(true, Ordering::SeqCst);
        let counted = self.dropped.clone();
        let written = self.progress.clone();
        // The thread's name moves out of the materials instead of
        // being cloned for the builder: the closure below drops the
        // name field unread (its destructure takes the other fields
        // by pattern and the rest by `..`), so the one string the
        // start built is the one the builder takes, and the emptied
        // field the closure drops allocates nothing.
        let name = std::mem::take(&mut pending.name);
        // The gate the thread waits on before it touches a line: made
        // before the thread is, so a thread that exists always has a
        // gate that will be set. A gate that cannot be made ends the
        // sink here, with no thread started.
        let go = unsafe { CreateEventW(std::ptr::null_mut(), 1, 0, std::ptr::null()) };
        if go.is_null() {
            relock(&self.queue).take();
            return;
        }
        let live = Arc::new(AtomicBool::new(false));
        let (thread_live, gate) = (live.clone(), go as usize);
        let spawned = std::thread::Builder::new().name(name).spawn(move || {
            let go = gate as Handle;
            unsafe {
                WaitForSingleObject(go, INFINITE);
                CloseHandle(go);
            }
            if !thread_live.load(Ordering::SeqCst) {
                // Released dead: the materials go with the thread and
                // nothing is written, as a gated worker that could not
                // be hardened is never given its payload.
                return;
            }
            let Pending { lines, mut write, mut idle, done, .. } = pending;
            loop {
                let line = match idle.as_mut() {
                    None => match lines.recv() {
                        Ok(line) => line,
                        Err(_) => break,
                    },
                    Some(idle) => match lines.recv_timeout(Duration::from_millis(RETRY_MS)) {
                        Ok(line) => line,
                        Err(RecvTimeoutError::Timeout) => {
                            idle(false);
                            continue;
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    },
                };
                let written_ok = write(&line);
                {
                    let mut progress = relock(&written.0);
                    progress.written += 1;
                    if !written_ok {
                        progress.failed += 1;
                    }
                }
                written.1.notify_all();
                // The dropped-lines notice is no queued line's
                // record: its own outcome is not counted here (the
                // standard-error copy's outcome still reaches the
                // shared flag its closure stores, as before).
                report_dropped(&counted, &mut |line: &str| {
                    write(line);
                });
            }
            report_dropped(&counted, &mut |line: &str| {
                write(line);
            });
            if let Some(idle) = idle.as_mut() {
                idle(true);
            }
            let _ = done.send(());
        });
        match spawned {
            Ok(thread) => {
                // Real CLI only: a payload must not suspend a worker holding audit state.
                let hardened = crate::cli_process::harden_worker(&thread).is_ok();
                *relock(&self.thread) = Some(thread);
                // The gate is released only now, live when the thread
                // was hardened and dead when it was not: the thread
                // takes its first line after this, never before.
                live.store(hardened, Ordering::SeqCst);
                unsafe { SetEvent(go) };
                if !hardened {
                    relock(&self.queue).take();
                }
            }
            Err(_) => {
                unsafe { CloseHandle(go) };
                relock(&self.queue).take();
            }
        }
    }

    /// Queue a line without waiting: a full queue drops it and counts it. The answer is the
    /// number of the line among those queued (the queue lock is held, so numbers are in queue
    /// order), or none when the line was dropped.
    pub(super) fn enqueue(&self, line: &str) -> Option<usize> {
        self.enqueue_shared(&Arc::from(line))
    }

    /// `enqueue` with the line already in shared storage: a line the
    /// log queues for several sinks is built once, and each further
    /// sink queues a reference to the same text — an atomic count,
    /// not another allocation and copy of the whole line per sink.
    pub(super) fn enqueue_shared(&self, line: &Arc<str>) -> Option<usize> {
        self.ensure_started();
        let queue = relock(&self.queue);
        let sender = queue.as_ref()?;
        if sender.try_send(line.clone()).is_err() {
            self.dropped.fetch_add(1, Ordering::SeqCst);
            return None;
        }
        Some(self.queued.fetch_add(1, Ordering::SeqCst) + 1)
    }

    pub(super) fn push(&self, line: &str) {
        let _ = self.enqueue(line);
    }

    /// Whether this sink can never take another line: its thread was
    /// started (the starting materials were taken) and its queue has
    /// since been closed — when the thread could not be started or
    /// could not be hardened, and when the sink is finished. A sink
    /// that was never started is not closed: its first queued line
    /// is the one that starts it.
    pub(super) fn closed(&self) -> bool {
        self.started.load(Ordering::SeqCst) && relock(&self.queue).is_none()
    }

    /// Wait, at most `wait_ms`, until the thread has finished writing the line with this number.
    pub(super) fn wait_written(&self, number: usize, wait_ms: u64) -> bool {
        let end = Instant::now() + Duration::from_millis(wait_ms);
        let mut done = relock(&self.progress.0);
        while done.written < number {
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            done = self.progress.1.wait_timeout(done, left).unwrap_or_else(|poisoned| poisoned.into_inner()).0;
        }
        true
    }

    /// How many of this sink's writes have failed so far: the mark
    /// a waited record snapshots before it queues its line.
    pub(super) fn failed_writes(&self) -> usize {
        relock(&self.progress.0).failed
    }

    /// Wait, at most `wait_ms`, for the line with this number, and
    /// say whether it stands as a record: its write finished within
    /// the wait and no write failed between the caller's
    /// `failed_writes` snapshot (taken before the line was queued)
    /// and the wait's end. A failure of this line always moves the
    /// failure count, so its outcome cannot be masked by a later
    /// line that was written; a failure of a neighbouring line in
    /// the same window also refuses the record — while writes
    /// around it are failing, this line's record is not relied on.
    pub(super) fn wait_recorded(&self, number: usize, failed_before: usize, wait_ms: u64) -> bool {
        if !self.wait_written(number, wait_ms) {
            return false;
        }
        relock(&self.progress.0).failed == failed_before
    }

    /// Stop taking lines and wait, at most `wait_ms`, for the ones queued to be written.
    /// A thread still stuck in a write is left to end with the process.
    pub(super) fn finish(&self, wait_ms: u64) {
        if relock(&self.pending).take().is_some() {
            // The thread was never started, so no line was ever queued for
            // it: there is nothing to wait for.
            relock(&self.queue).take();
            return;
        }
        if relock(&self.queue).take().is_none() {
            return;
        }
        if relock(&self.finished).recv_timeout(Duration::from_millis(wait_ms)).is_ok() {
            if let Some(thread) = relock(&self.thread).take() {
                let _ = thread.join();
            }
        }
    }
}

/// True when standard error can hold a record: it is open and is a console, a file or a
/// pipe. A character device that is not a console (the null device) takes every write and
/// keeps none of it, so a log written there is a log nowhere.
pub(super) fn standard_error_records() -> bool {
    // The handle and its console verdict come from standard error's
    // own probe slot (see `cli_process::standard_error_target`): this
    // ask is made at the extension's start, before the run's handle
    // loop reads the same slot, and probing the handle live here paid
    // a second `GetStdHandle` and `GetConsoleMode` for the one handle
    // on every extendable run that has no audit file.
    let (handle, console) = crate::cli_process::standard_error_target();
    if handle.is_null() || handle == INVALID_HANDLE {
        return false;
    }
    console || matches!(unsafe { GetFileType(handle) }, FILE_TYPE_DISK | FILE_TYPE_PIPE)
}

/// Open the file once for appending, so a bad path is an error before the command starts.
/// The path must name a file on a disk: a device (the null device, a console) or a pipe
/// takes every write and keeps none of it, which is a log nowhere (Common Weakness
/// Enumeration, CWE-778). The opened, typed file is returned, so that the one handle
/// is handed on as the audit file's held handle instead of the path being opened again.
pub(super) fn check_log_path(path: Option<&Path>) -> Result<Option<std::fs::File>, GuardError> {
    if let Some(path) = path {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)
            .map_err(|error| GuardError(format!("cannot open the extension log {}: {}", path.display(), error)))?;
        require_disk_file(&file, path)?;
        return Ok(Some(file));
    }
    Ok(None)
}

pub(super) fn require_disk_file(file: &std::fs::File, path: &Path) -> Result<(), GuardError> {
    if unsafe { GetFileType(crate::sys::windows::raw_handle(file)) } != FILE_TYPE_DISK {
        return Err(GuardError(format!("the extension log must be a regular file: {} is a device or a pipe; the run did not start", path.display())));
    }
    Ok(())
}

/// The audit record: standard error, and with `--extension-log` a file bounded at
/// one mebibyte (MiB) with one rotation, `PATH.1` (see `AuditFile` for a file that
/// cannot be turned over or written). Each copy has its queue and a thread that
/// starts on its first queued line. It records;
/// it controls nothing, and nothing waits for it.
pub(crate) struct Log {
    pub(super) sinks: Vec<Arc<Sink>>,
    /// The sink that writes the audit file, when there is one. The
    /// audit file's own state (see `blocked` below) reports only the
    /// failures of writes it was given; a file sink whose queue has
    /// closed is given no line at all, so its failure is invisible
    /// there and `blocked` reads this sink's state beside it.
    pub(super) file_sink: Option<Arc<Sink>>,
    /// The file cannot be relied on to take the lines of the grants (see `AuditFile`).
    pub(super) blocked: Arc<AtomicBool>,
    /// Without an audit file, standard error is the only record, and a grant stands only once its
    /// line has been written there (see `record`).
    pub(super) only: Option<StandardErrorOnly>,
}

pub(super) struct StandardErrorOnly {
    pub(super) sink: Arc<Sink>,
    /// Whether the last line written to standard error was written.
    pub(super) last_written: Arc<AtomicBool>,
}

impl Log {
    /// `keep` is the audit file already opened for `path` — on the start of a
    /// run the one open `check_log_path` made and typed — and becomes the
    /// audit file's held handle; it is not opened again here. None when no
    /// file was opened for the path (a file that a reader keeps open, whose
    /// lines are then held, see `AuditFile`).
    pub(crate) fn start(path: Option<Arc<Path>>, limit: u64, keep: Option<std::fs::File>) -> Result<Log, GuardError> {
        let last_written = Arc::new(AtomicBool::new(true));
        let seen = last_written.clone();
        // Select this run's stderr on its invoking thread, before the lazy
        // worker exists. Console cleanup can later change the live table.
        // This borrows the target; a detached or closed target can fail normally.
        let (stderr_handle, stderr_console) = crate::cli_process::standard_error_target();
        let stderr_handle = stderr_handle as usize;
        let stderr = Arc::new(Sink::start("extension-log-stderr", Box::new(move |line: &str| {
            let written = crate::cli_process::try_write_line_counted_to(stderr_handle as Handle, stderr_console, line);
            seen.store(written, Ordering::SeqCst);
            written
        }), LOG_QUEUE)?);
        let mut sinks = vec![stderr.clone()];
        let mut blocked = Arc::new(AtomicBool::new(false));
        let mut file_sink = None;
        let mut only = Some(StandardErrorOnly { sink: stderr.clone(), last_written });
        if let Some(path) = path {
            only = None;
            // The file's notices go to standard error through its queue, never by a write that could wait.
            let mut file = AuditFile::new(path, limit, keep).alerting(Box::new(move |text: &str| stderr.push(text)));
            if !file.settle() {
                return Err(GuardError(format!("the extension log {} is past its limit and cannot be turned over (is {}.1 held open?); the run did not start",
                    file.path.display(), file.path.display())));
            }
            blocked = file.blocked.clone();
            let written = Arc::new(Mutex::new(file));
            let on_idle = written.clone();
            let idle: Idle = Box::new(move |last: bool| {
                let mut file = relock(&on_idle);
                if last { file.close() } else { file.flush() }
            });
            let sink = Arc::new(Sink::start_with("extension-log-file", Box::new(move |line: &str| {
                relock(&written).append(line);
                true
            }), Some(idle), LOG_QUEUE)?);
            sinks.push(sink.clone());
            file_sink = Some(sink);
        }
        Ok(Log { sinks, file_sink, blocked, only })
    }

    /// A log that keeps its lines in `lines`, for the tests of other modules. With `fail`, it
    /// behaves as a log whose only record is a standard error: a line is a record only when `fail`
    /// is clear as it is written; without it, the log has a file's rule and nothing is waited for.
    #[cfg(test)]
    pub(crate) fn capture(lines: Arc<Mutex<Vec<String>>>, fail: Option<Arc<AtomicBool>>) -> Log {
        let last_written = Arc::new(AtomicBool::new(true));
        let seen = last_written.clone();
        let failing = fail.clone();
        let write = move |line: &str| {
            relock(&lines).push(line.to_string());
            let written = !failing.as_ref().map_or(false, |fail| fail.load(Ordering::SeqCst));
            seen.store(written, Ordering::SeqCst);
            written
        };
        let sink = Arc::new(Sink::start("test-log", Box::new(write), 16).expect("start the sink"));
        let only = fail.map(|_| StandardErrorOnly { sink: sink.clone(), last_written });
        Log { sinks: vec![sink], file_sink: None, blocked: Arc::new(AtomicBool::new(false)), only }
    }

    /// A log whose audit file's sink has a closed queue (its thread could not
    /// be started or hardened), for the tests of other modules: the file sink
    /// is first in `sinks` and the copy that keeps its lines in `lines` is
    /// last, so a record decided by whichever sink is last is decided by the
    /// copy, not the file.
    #[cfg(test)]
    pub(crate) fn capture_with_closed_file(lines: Arc<Mutex<Vec<String>>>) -> Log {
        let file = Arc::new(Sink::start("test-file-closed", Box::new(|_: &str| true), 8).expect("start the sink"));
        file.started.store(true, Ordering::SeqCst);
        relock(&file.queue).take();
        let write = move |line: &str| {
            relock(&lines).push(line.to_string());
            true
        };
        let copy = Arc::new(Sink::start("test-copy", Box::new(write), 16).expect("start the sink"));
        Log { sinks: vec![file.clone(), copy], file_sink: Some(file), blocked: Arc::new(AtomicBool::new(false)), only: None }
    }

    /// True while the audit record cannot be relied on to take lines: nothing is granted then.
    /// That is the audit file when there is one, and otherwise standard error, once a line
    /// written to it has failed (a write that works lifts it).
    pub(crate) fn blocked(&self) -> bool {
        self.blocked.load(Ordering::SeqCst)
            || self.file_sink.as_ref().is_some_and(|sink| sink.closed())
            || self.only.as_ref().is_some_and(|only| !only.last_written.load(Ordering::SeqCst))
    }

    /// Queue the line of a grant and say whether the grant may stand. With an audit file the
    /// file is the record and `blocked` stops the grants when it fails, so the writing is not
    /// waited for — but the line must at least have been taken by the file sink's queue: a
    /// line that queue refuses (the queue is closed, because the sink's thread could not be
    /// started or hardened, or the queue is full behind a sink thread that cannot keep up)
    /// reaches no record at all, not even the audit file's held lines, so the grant does not
    /// stand. Without one, the line is waited for, for at most `wait_ms`: a grant whose line was
    /// not written (standard error closed, full, or unreadable) is a deadline moved by a
    /// request that no record has (Common Weakness Enumeration, CWE-778).
    pub(crate) fn record(&self, line: &str, wait_ms: u64) -> bool {
        let Some(only) = &self.only else {
            let Some(file) = &self.file_sink else {
                self.push(line);
                return true;
            };
            // The file sink is queued first and its answer is the
            // record's: every other sink is a copy and its answer is
            // not read, exactly as in `push`.
            let shared: Arc<str> = Arc::from(line);
            let filed = file.enqueue_shared(&shared).is_some();
            for sink in &self.sinks {
                if !Arc::ptr_eq(sink, file) {
                    let _ = sink.enqueue_shared(&shared);
                }
            }
            return filed;
        };
        // The failure count is snapshotted before the line is
        // queued: the record is this line's own outcome, decided
        // by `wait_recorded`, never by the outcome of the last
        // write any thread happened to make.
        let failed_before = only.sink.failed_writes();
        match only.sink.enqueue(line) {
            Some(number) => only.sink.wait_recorded(number, failed_before, wait_ms),
            None => false,
        }
    }

    /// Queue the line of the approval of an exception above the cap and say whether it stands.
    /// Unlike the line of a grant, which with an audit file is not waited for, the line of an
    /// approval is waited for, for at most `wait_ms`, in the record: the audit file when there is
    /// one (which must not be blocked once the line is placed: a file that cannot take it holds
    /// it, and `blocked` says so), and otherwise standard error. An approval whose line was not
    /// written is not an approval (Common Weakness Enumeration, CWE-778).
    pub(crate) fn record_decision(&self, line: &str, wait_ms: u64) -> bool {
        let Some(only) = &self.only else {
            // Standard error is a copy of the audit file: the line is queued for both, the file's copy is waited for, and the
            // file must not be unable to take it, which `blocked` says once its write has failed.
            // The file sink is named, as in `record`, and its own
            // enqueue number is the one waited for: which sink is
            // last in `sinks` decides nothing, so a copy added or
            // the two copies swapped cannot stand an approval on a
            // copy's write while the file's own enqueue result goes
            // unread. The line itself is built once and shared, as
            // in `push`: queueing it per sink copied it into a fresh
            // allocation for every sink.
            let Some(file) = &self.file_sink else {
                // No audit file and no only record either: the
                // capture shape of the tests, whose present
                // fall-through is kept — the last sink queued is
                // waited for, as before.
                let shared: Arc<str> = Arc::from(line);
                let mut last_sink = None;
                let mut last_number = None;
                for sink in &self.sinks {
                    last_number = sink.enqueue_shared(&shared);
                    last_sink = Some(sink);
                }
                return match (last_sink, last_number) {
                    (Some(sink), Some(number)) => sink.wait_written(number, wait_ms) && !self.blocked.load(Ordering::SeqCst),
                    (None, _) => !self.blocked.load(Ordering::SeqCst),
                    _ => false,
                };
            };
            let shared: Arc<str> = Arc::from(line);
            let number = file.enqueue_shared(&shared);
            for sink in &self.sinks {
                if !Arc::ptr_eq(sink, file) {
                    let _ = sink.enqueue_shared(&shared);
                }
            }
            return match number {
                Some(number) => file.wait_written(number, wait_ms) && !self.blocked.load(Ordering::SeqCst),
                None => false,
            };
        };
        let failed_before = only.sink.failed_writes();
        match only.sink.enqueue(line) {
            Some(number) => only.sink.wait_recorded(number, failed_before, wait_ms),
            None => false,
        }
    }

    /// Queue the line of a decision that moves no deadline outward (a refusal, a
    /// release, the notices that a grant was withdrawn or unrecorded) and say
    /// whether the audit file's sink took it. With an audit file the file sink
    /// is queued first and its answer is the answer, exactly as in `record`:
    /// a line that queue refuses (closed, or full behind a sink thread that
    /// cannot keep up) reaches the record of the run nowhere on the file
    /// side, and the caller is told, where `push` discarded every answer.
    /// The copies are queued after, their answers unread. With no audit
    /// file this is `push` and the answer is true: standard error remains a
    /// best-effort copy for a line whose decision does not stand on it.
    pub(crate) fn push_decision(&self, line: &str) -> bool {
        let Some(file) = &self.file_sink else {
            self.push(line);
            return true;
        };
        let shared: Arc<str> = Arc::from(line);
        let filed = file.enqueue_shared(&shared).is_some();
        for sink in &self.sinks {
            if !Arc::ptr_eq(sink, file) {
                let _ = sink.enqueue_shared(&shared);
            }
        }
        filed
    }

    /// `push_decision` with the fallback of the server's decision lines:
    /// when the audit file's sink refused the line, the line is queued
    /// again to the copies that are not the file sink and the first of
    /// them is waited for, at most `wait_ms` — the waited form `record`
    /// gives a grant when standard error is the only record — so the
    /// line reaches that remaining record, or the answer is false and
    /// the copies' dropped-lines notice is the word that it could not.
    /// With no audit file, or when the file took the line, this is
    /// `push_decision` alone. The answer decides nothing: a refusal
    /// has refused and a release has ended its hold before either
    /// line is built.
    pub(crate) fn push_decision_waited(&self, line: &str, wait_ms: u64) -> bool {
        if self.push_decision(line) {
            return true;
        }
        let shared: Arc<str> = Arc::from(line);
        let mut waited = None;
        for sink in &self.sinks {
            if self.file_sink.as_ref().is_some_and(|file| Arc::ptr_eq(sink, file)) {
                continue;
            }
            if waited.is_none() {
                waited = Some((sink.clone(), sink.failed_writes(), sink.enqueue_shared(&shared)));
            } else {
                let _ = sink.enqueue_shared(&shared);
            }
        }
        match waited {
            Some((sink, failed_before, Some(number))) => sink.wait_recorded(number, failed_before, wait_ms),
            _ => false,
        }
    }

    pub(crate) fn push(&self, line: &str) {
        // One shared build of the line for every sink: with an audit
        // file the two sinks each queued their own copy of it before.
        let shared: Arc<str> = Arc::from(line);
        for sink in &self.sinks {
            let _ = sink.enqueue_shared(&shared);
        }
    }

    /// A line of the log that is a notice, with the time now.
    pub(crate) fn note(&self, text: &str) {
        self.push(&notice_line(unix_ms_now(), text));
    }

    /// Stop taking lines and give each copy a moment to write the ones it has. The file
    /// goes first, so that what it has to say as it ends still reaches standard error.
    pub(crate) fn finish(&self, wait_ms: u64) {
        for sink in self.sinks.iter().rev() {
            sink.finish(wait_ms);
        }
    }
}

/// Open the log of an extendable run: the audit file, when there is one, must be a file on a
/// disk, and without one standard error must be able to hold a record. The run does not start
/// when it cannot be opened. It is opened before the command is known to be wanted, so that
/// the decision on an exception above the cap has a place to be recorded.
pub(crate) fn open_log(settings: &Settings) -> Result<Log, GuardError> {
    let opened = check_log_path(settings.log_path.as_deref())?;
    if settings.log_path.is_none() && !standard_error_records() {
        return Err(GuardError("an extendable run needs a record of its grants: standard error is closed or is the null device, \
            and --extension-log was not given; the run did not start".into()));
    }
    Log::start(settings.log_path.clone(), LOG_LIMIT, opened)
}
