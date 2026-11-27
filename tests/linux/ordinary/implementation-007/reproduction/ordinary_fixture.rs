//! Ordinary application A for the private full-system deadline fixture.
use super::caller_adapter::{run_ordinary, OrdinaryOutcome};
use super::OwnerMode;
use std::ffi::{c_int, c_long, OsStr, OsString};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Write as _};
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;

const PROGRAM: &str = "/var/lib/ptg220-ordinary-witness/payload";
const PR_SET_CHILD_SUBREAPER: c_int = 36;
const PR_GET_CHILD_SUBREAPER: c_int = 37;
const MAX_RECEIPT_BYTES: u64 = 64 * 1024;

unsafe extern "C" {
    fn prctl(option: c_int, ...) -> c_int;
}

fn own_children() -> io::Result<()> {
    // A is the actual caller. Its fixed fresh guest process has no competing wait owner.
    let mut observed: c_int = 0;
    if unsafe { prctl(PR_SET_CHILD_SUBREAPER, 1 as c_long, 0 as c_long, 0 as c_long, 0 as c_long) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe {
        prctl(PR_GET_CHILD_SUBREAPER, &mut observed as *mut c_int, 0 as c_long,
              0 as c_long, 0 as c_long)
    } < 0 {
        return Err(io::Error::last_os_error());
    }
    if observed != 1 {
        return Err(io::Error::other("subreaper readback differed"));
    }
    Ok(())
}

fn receipt(fd: OwnedFd) -> io::Result<String> {
    let file = File::from(fd);
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_RECEIPT_BYTES {
        return Err(io::Error::other("unexpected original receipt length or type"));
    }
    let mut raw = vec![0; metadata.len() as usize];
    file.read_exact_at(&mut raw, 0)?;
    let mut hex = String::with_capacity(raw.len() * 2);
    for byte in raw {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(hex)
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    if std::env::args_os().nth(1).is_some() {
        return Err(io::Error::other("ordinary fixture accepts no arguments").into());
    }
    own_children()?;
    let args = [OsString::from(PROGRAM)];
    let run = run_ordinary(OsStr::new(PROGRAM), &args, 12.0, OwnerMode::Manual)
        .map_err(io::Error::other)?;
    let (kind, value, exit) = match run.outcome {
        OrdinaryOutcome::Exited(status) => ("exited", status, status),
        OrdinaryOutcome::Signaled(signal) => ("signaled", signal, 128 + signal),
        OrdinaryOutcome::TimedOut => ("deadline", 0, 124),
        OrdinaryOutcome::OwnerEnded => ("owner_ended", 0, 125),
        OrdinaryOutcome::ExecRefused(code) => ("exec_refused", code, 125),
        OrdinaryOutcome::Unconfirmed(code) => ("unconfirmed", code, 125),
    };
    let raw = run.evidence.map(receipt).transpose()?;
    let mut out = io::stdout().lock();
    let hex_json = raw.as_ref().map_or_else(|| "null".to_owned(), |hex| format!("\"{hex}\""));
    writeln!(out, "PTG220_ORIGINAL_NATIVE={{\"outcome\":\"{kind}\",\"value\":{value},\"evidence_hex\":{hex_json}}}")?;
    out.flush()?;
    if kind == "deadline" && raw.is_none() {
        return Err(io::Error::other("deadline outcome lacked its original native receipt").into());
    }
    Ok(exit)
}

pub(crate) fn entry() -> i32 {
    run().unwrap_or_else(|error| {
        eprintln!("ordinary fixture refused: {error}");
        125
    })
}
