// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! The audit file: lines are appended, and the file is turned over to `PATH.1` when a line would take it past its limit.

use super::*;

/// The audit file: lines are appended, and the file is turned over to `PATH.1` when a line
/// would take it past `limit`. A turn over can fail, because a program that follows the
/// file may hold it open without sharing the right to delete it, which stops a rename.
/// The earlier lines are never discarded for that: the file is copied to `PATH.1` and
/// emptied instead, and when that fails too it grows past `limit` as far as twice `limit`,
/// with a notice in it, and then takes no more lines until a turn over works (standard
/// error still has them). `PATH.1` is not held open by the guard: a program that reads it
/// without sharing the right to delete it (and any program in the job can) stops both ways
/// of turning the file over. So the file cannot be relied on to take lines for ever, and a
/// run whose file is about to stop taking them stops granting (see `blocked`).
///
/// A write can fail too: a program that reads the file without sharing the right to write
/// it (the default of many file readers) refuses the guard's append. The file is held open
/// for the run, which keeps such a reader from opening it at all; when a write fails
/// nonetheless (the reader was there first, the folder went away), the line is held, up to
/// `HELD_LINES`, and written in order as soon as a write works again. Standard error is told
/// of the failure at once, and the file gets a notice with the counts once it takes lines.
///
/// A grant that the file would not record is a deadline moved by a request that is not in
/// the audit record, so while lines cannot be placed (writes failing, or a file that cannot
/// be turned over and has less than `reserve` bytes left before twice its limit) the file is
/// `blocked` and the run grants nothing, with a reason, until it can place lines again.
pub(in crate::extend) struct AuditFile {
    pub(in crate::extend) path: Arc<Path>,
    pub(in crate::extend) limit: u64,
    /// Bytes below twice `limit` that a file which cannot be turned over keeps for the lines
    /// that are already on their way when it becomes blocked.
    pub(in crate::extend) reserve: u64,
    /// Set while the file cannot be relied on to take the lines of the grants: the run reads it.
    pub(in crate::extend) blocked: Arc<AtomicBool>,
    /// The notice that a turn over failed is in the file.
    pub(in crate::extend) stuck: bool,
    /// The notice that the file takes no more lines is in the file.
    pub(in crate::extend) full: bool,
    /// When a failed turn over was last tried, so a file that cannot be turned over is not
    /// copied for every line.
    pub(in crate::extend) tried: Option<std::time::Instant>,
    /// The file, open for appending for the whole run, so that a program that reads it
    /// without sharing the right to write cannot open it. Written through: every line
    /// placed goes through this held handle, opened once, not through a fresh open
    /// of the path for each line.
    pub(in crate::extend) _keep: Option<std::fs::File>,
    /// The file's size as tracked here, in bytes: seeded from one metadata call at
    /// `settle`, incremented by the bytes of each line and notice actually written
    /// through the held handle, and re-seeded only at a turn over (to the empty
    /// file a turn over leaves). The trade: the turn-over decision no longer sees
    /// bytes appended by foreign writers (none are sanctioned; the audit file is
    /// the guard's own record). While only the guard writes, the tracked size is
    /// the file's real size at every decision.
    pub(in crate::extend) tracked_size: u64,
    /// Lines that could not be written, oldest first.
    pub(in crate::extend) held: VecDeque<String>,
    /// Lines left out because `HELD_LINES` were already held.
    pub(in crate::extend) lost: usize,
    /// Lines written late (held, and written when the file took lines again) in this failure.
    pub(in crate::extend) late: usize,
    /// The file cannot be written, and standard error has been told.
    pub(in crate::extend) failing: bool,
    /// Where a notice goes at once: standard error.
    pub(in crate::extend) alert: Box<dyn Fn(&str) + Send>,
    /// The line being written, with its newline: one buffer kept for
    /// the run and refilled per line, where forming the line's text
    /// fresh per write allocated and copied it once more on top of
    /// the copy the queue already made.
    pub(in crate::extend) scratch: String,
}

impl AuditFile {
    /// `keep` is the file already opened for this path (the one open of the
    /// start, typed by its opener), held for the whole run; None when that
    /// open could not be made, in which case a write tries the open afresh,
    /// as it always has.
    pub(in crate::extend) fn new(path: Arc<Path>, limit: u64, keep: Option<std::fs::File>) -> AuditFile {
        AuditFile {
            path, limit, reserve: (limit / 16).min(MAX_RESERVE), blocked: Arc::new(AtomicBool::new(false)), stuck: false, full: false, tried: None,
            _keep: keep, tracked_size: 0, held: VecDeque::new(), lost: 0, late: 0, failing: false, alert: Box::new(|_| {}), scratch: String::new(),
        }
    }

    /// Before the command starts: turn the file over if it is already past its limit, and say
    /// whether it can take lines. A file that cannot is a record the run could not keep.
    pub(in crate::extend) fn settle(&mut self) -> bool {
        // The one metadata call the tracked size is seeded from; every
        // later decision reads the tracked size, not the file again.
        self.tracked_size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        self.turn_over_if_due(0);
        self.refresh();
        !self.blocked.load(Ordering::SeqCst)
    }

    /// Work out whether the file can be relied on to take the lines of the grants, and tell the run.
    pub(in crate::extend) fn refresh(&self) {
        // `tried` is set from a failed turn over until one works.
        let near_full = self.tried.is_some() && self.size() + self.reserve > 2 * self.limit;
        self.blocked.store(self.failing || self.full || near_full, Ordering::SeqCst);
    }

    pub(in crate::extend) fn size(&self) -> u64 {
        self.tracked_size
    }

    /// Turn the file over when `extra` more bytes would take it past its limit, unless a turn
    /// over failed less than `TURN_OVER_RETRY_MS` ago (a file that cannot be turned over is
    /// not copied for every line).
    pub(in crate::extend) fn turn_over_if_due(&mut self, extra: u64) {
        let waiting = self.tried.is_some_and(|at| at.elapsed() < Duration::from_millis(TURN_OVER_RETRY_MS));
        let size = self.size();
        if size > 0 && size + extra > self.limit && !waiting {
            if self.turn_over() {
                self.stuck = false;
                self.full = false;
                self.tried = None;
                // Re-seeded at the turn over: the file it leaves is empty.
                self.tracked_size = 0;
                // A rename moved the file the held handle was on; hold the new one.
                self._keep = AuditFile::open(&self.path).ok();
            } else {
                self.tried = Some(std::time::Instant::now());
            }
        }
    }

    /// The same, with the place a notice goes at once.
    pub(in crate::extend) fn alerting(mut self, alert: Box<dyn Fn(&str) + Send>) -> AuditFile {
        self.alert = alert;
        self
    }

    pub(in crate::extend) fn open(path: &Path) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new().create(true).append(true).open(path)
    }

    /// Write one line through the held append handle, and count its bytes
    /// (with the newline) in the tracked size once they are written. A
    /// handle that was never opened — a reader that denies writers was
    /// there first, at `new` — is tried again here, as the fresh open of
    /// the old per-line write was; a write that fails lets the handle go,
    /// so the next try opens the file afresh too.
    pub(in crate::extend) fn write(&mut self, line: &str) -> std::io::Result<()> {
        self.scratch.clear();
        self.scratch.push_str(line);
        self.scratch.push('\n');
        if self._keep.is_none() {
            self._keep = Some(AuditFile::open(&self.path)?);
        }
        let file = self._keep.as_mut().expect("the held handle was just opened");
        match file.write_all(self.scratch.as_bytes()) {
            Ok(()) => {
                self.tracked_size += self.scratch.len() as u64;
                Ok(())
            }
            Err(error) => {
                self._keep = None;
                Err(error)
            }
        }
    }

    /// Move the lines to `PATH.1`, replacing what is there, and leave the file empty.
    pub(in crate::extend) fn turn_over(&self) -> bool {
        let mut rotated = self.path.as_os_str().to_owned();
        rotated.push(".1");
        let rotated = PathBuf::from(rotated);
        std::fs::rename(&self.path, &rotated).is_ok()
            || (std::fs::copy(&self.path, &rotated).is_ok()
                && std::fs::OpenOptions::new().write(true).truncate(true).open(&self.path).is_ok())
    }

    /// Write one line, turning the file over first when it is due. An error is a write that
    /// failed; a line left out on purpose, because the file is at twice its limit, is not one.
    pub(in crate::extend) fn place(&mut self, line: &str) -> std::io::Result<()> {
        self.turn_over_if_due(line.len() as u64 + 1);
        let size = self.size();
        if size > 0 && size + line.len() as u64 + 1 > self.limit {
            // The turn over failed or was not tried: the lines stay, and the file says so.
            if !self.stuck {
                self.write(&notice_line(unix_ms_now(), "the log file could not be turned over; it grows past its limit, to twice the limit, and then takes no more lines (standard error has them)"))?;
                self.stuck = true;
            }
            if size + line.len() as u64 + 1 > 2 * self.limit {
                if !self.full {
                    self.write(&notice_line(unix_ms_now(), "the log file is at twice its limit and could not be turned over; it takes no more lines until it can be (standard error has them)"))?;
                    self.full = true;
                }
                return Ok(());
            }
        }
        self.write(line)
    }

    /// Take a line: it is written after the ones held before it, or held itself.
    pub(in crate::extend) fn append(&mut self, line: &str) {
        // With nothing held, the line goes straight to the file:
        // queueing it first copied it into a fresh string for the
        // queue, only for `drain` to pop that string and write the
        // line at once — one allocation and one full copy per line
        // on the path where writes succeed. A line that cannot be
        // placed falls through to the queue below, and `flush`
        // accounts for the failure exactly as it always has (its
        // retry of the placement is the failure path's second
        // attempt; the outcome — the line held, the alert sent
        // once, the run told — is the same). Nothing held also
        // means no failure is outstanding: `drain` sets `failing`
        // only with a line held, and clears it only as the last
        // held line is written, so no late-line accounting is owed
        // on this path, and `refresh` keeps the run's blocked flag
        // current, as `flush` ends by doing.
        if self.held.is_empty() && self.place(line).is_ok() {
            self.refresh();
            return;
        }
        if self.held.len() < HELD_LINES {
            self.held.push_back(line.to_string());
        } else {
            self.lost += 1;
        }
        self.flush();
    }

    /// Write the lines held, oldest first, until one cannot be written. The first failure
    /// of a run of them is told to standard error at once; when the last line held has been
    /// written, the file and standard error are told how many were written late and how many
    /// were not written at all. A file that could not be turned over is tried again here too,
    /// with no line to wake it, so that a run which stopped granting for want of room starts
    /// again once a program lets go of `PATH.1`; and the run is told whether the file can be
    /// relied on to take lines.
    pub(in crate::extend) fn flush(&mut self) {
        self.turn_over_if_due(0);
        self.drain();
        self.refresh();
    }

    pub(in crate::extend) fn drain(&mut self) {
        while let Some(line) = self.held.pop_front() {
            if let Err(error) = self.place(&line) {
                self.held.push_front(line);
                if !self.failing {
                    self.failing = true;
                    (self.alert)(&notice_line(unix_ms_now(), &format!(
                        "the log file {} cannot be written ({}); its lines are kept, up to {}, and written when it can be (standard error has them)",
                        self.path.display(), error, HELD_LINES)));
                }
                return;
            }
            if self.failing {
                self.late += 1;
                if self.held.is_empty() {
                    let text = format!("the log file could not be written for a time: {} lines were written late and {} were not written because too many were waiting (standard error has them)",
                        self.late, self.lost);
                    // The one line is built once, from one sample
                    // of the clock, and carried to both places it
                    // is owed: the alert reads it where it stands
                    // and the held queue takes it. Building it per
                    // destination paid the escape pass, the
                    // allocation and the clock sample twice for
                    // two copies of the one notice.
                    let line = notice_line(unix_ms_now(), &text);
                    (self.alert)(&line);
                    self.held.push_back(line);
                    self.failing = false;
                    self.late = 0;
                    self.lost = 0;
                }
            }
        }
    }

    /// The run is over: a last try, and a word to standard error about what the file never got.
    pub(in crate::extend) fn close(&mut self) {
        self.flush();
        if !self.held.is_empty() {
            (self.alert)(&notice_line(unix_ms_now(), &format!(
                "{} log lines were not written to the log file, which could not be written when the run ended (standard error has them)",
                self.held.len() + self.lost)));
        }
    }
}
