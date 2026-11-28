//! Private A for the enrolled Manual natural-exit case, through the public API.
use process_tree_guard::ordinary_linux::{run_manual, Outcome};
use std::ffi::{c_int, c_long, OsString};
use std::io::{self, Write};
use std::path::Path;

const PROGRAM: &str = "/var/lib/ptg220-ordinary-witness/payload";
const PR_SET_CHILD_SUBREAPER: c_int = 36;
const PR_GET_CHILD_SUBREAPER: c_int = 37;

unsafe extern "C" {
    fn prctl(option: c_int, ...) -> c_int;
}

fn own_children() -> io::Result<()> {
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

fn run() -> io::Result<()> {
    if std::env::args_os().nth(1).is_some() {
        return Err(io::Error::other("ordinary fixture accepts no arguments"));
    }
    own_children()?;
    let args = [OsString::from(PROGRAM)];
    // The enrolled guest starts A by direct exec with default SIGCHLD and no
    // existing children. This single-threaded caller owns all its child waits.
    let result = unsafe { run_manual(Path::new(PROGRAM), &args, 12.0) };
    match result {
        Ok(Outcome::Exited(124)) => {
            let mut output = io::stdout().lock();
            output.write_all(b"PTG220_PUBLIC_OUTCOME={\"outcome\":\"exited\",\"status\":124}\n")?;
            output.flush()
        }
        other => Err(io::Error::other(format!("ordinary public API outcome: {other:?}"))),
    }
}

fn main() {
    match run() {
        Ok(()) => std::process::exit(124),
        Err(error) => {
            eprintln!("ordinary public API fixture refused: {error}");
            std::process::exit(125);
        }
    }
}
