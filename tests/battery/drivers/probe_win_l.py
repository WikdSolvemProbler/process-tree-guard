# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Windows probe for battery case (l). Runs on win32 only; on any
other platform it prints nothing and exits 3 (the --child mode is
the sacrificial per-fixture worker and has the same gate).

For each fixture from battery/env_fixtures.py the differential
runs in two forms: as-is, and with a designated set entry — the
channel name — whose existing case-variants must all be skipped
and replaced. Both forms PERFORM that set operation on both
sides; neither side merely re-serialises a list it was handed.

The route that executes tree code (recorded on every fixture as
builder_route): a small Rust helper is compiled from source
that includes the tree's own src/engine/environment.rs, so the
helper's environment_block IS the tree's builder, not a copy of
its rule. For each representable fixture the helper is created
through a CreateProcessW environment block built directly from
the fixture's entries — the mechanism that preserves what a
process environment can hold — and, inside that one process,
the helper emits three things: (i) the entry list vars_os
yields, (ii) the entry list its own direct
GetEnvironmentStringsW walk yields, UNFILTERED — every entry,
not only names the fixture names — and (iii) the blocks the
real environment_block builds: with the channel entry set, and
with a sentinel entry set (the as-is form: the probe removes
the sentinel entry from that block, and the remaining block is
exactly what the builder builds from the fixture alone, the
sort being stable and the sentinel colliding with no fixture
name under the ASCII-exact matcher). The model side serialises
the helper's unfiltered GetEnvironmentStringsW entries under
the builder rule as the reference model states it.

One case matcher everywhere a name is matched: the tree's
eq_ignore_ascii_case semantics — ASCII A-Z fold to a-z, every
other character compares as itself — implemented identically
in ascii_eq below, in the reference model, and (in the driver)
in the decider. Unicode upper()/casefold() approximations are
not used for matching; the sort key remains the builder's
upper-case key, as the builder computes it (a name already its
own upper case in ASCII takes itself as key; every other name
takes its upper case — the model's key, the name's upper case,
is the same text in both cases).

A fixture a real Windows environment cannot represent — the
duplicate-case fixture, whose two case-folded names a process
environment holds as one — is never installed; it is judged at
the list level, both sides computed over the fixture's entry
list by the builder rule, and the record says so
(representable: false, route: list-level).
"""

import ctypes
import json
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.dirname(HERE))

if sys.platform != "win32":
    sys.exit(3)

import msvcrt  # noqa: E402

import env_fixtures  # noqa: E402

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
CREATE_UNICODE_ENVIRONMENT = 0x00000400


class StartupInfo(ctypes.Structure):
    _fields_ = [("cb", ctypes.c_uint32),
                ("lpReserved", ctypes.c_wchar_p),
                ("lpDesktop", ctypes.c_wchar_p),
                ("lpTitle", ctypes.c_wchar_p),
                ("dwX", ctypes.c_uint32), ("dwY", ctypes.c_uint32),
                ("dwXSize", ctypes.c_uint32),
                ("dwYSize", ctypes.c_uint32),
                ("dwXCountChars", ctypes.c_uint32),
                ("dwYCountChars", ctypes.c_uint32),
                ("dwFillAttribute", ctypes.c_uint32),
                ("dwFlags", ctypes.c_uint32),
                ("wShowWindow", ctypes.c_uint16),
                ("cbReserved2", ctypes.c_uint16),
                ("lpReserved2", ctypes.c_void_p),
                ("hStdInput", ctypes.c_void_p),
                ("hStdOutput", ctypes.c_void_p),
                ("hStdError", ctypes.c_void_p)]


class ProcessInformation(ctypes.Structure):
    _fields_ = [("hProcess", ctypes.c_void_p),
                ("hThread", ctypes.c_void_p),
                ("dwProcessId", ctypes.c_uint32),
                ("dwThreadId", ctypes.c_uint32)]


CHANNEL = "PROCESS_TREE_GUARD_CHANNEL"
SET_VALUE = "\\\\.\\pipe\\set-by-case-l"
SENTINEL_VALUE = "ptg-case-l-sentinel"
BUILDER_ROUTE = ("rust-helper: the tree's "
                 "src/engine/environment.rs compiled in; "
                 "environment_block executed in a process whose "
                 "environment is the fixture")


def ascii_fold(text):
    """The tree's eq_ignore_ascii_case fold
    (src/engine/environment.rs): ASCII A-Z fold to a-z; every
    other character, non-ASCII included, compares as itself."""
    return "".join(chr(ord(c) + 32) if "A" <= c <= "Z" else c
                   for c in text)


def ascii_eq(a, b):
    return ascii_fold(a) == ascii_fold(b)


def build_block(entries):
    """The raw environment block for CreateProcessW, in the
    fixture's own order: name=value, NUL-ended, one final NUL."""
    text = "".join(f"{name}={value}\x00" for name, value in entries)
    return (text + "\x00").encode("utf-16-le")


def builder_rule_block(entries, set_name, set_value):
    """The builder rule as the reference model states it, over
    the given entry list: skip every entry whose name matches
    the entry being set under the ASCII-exact matcher, append
    that entry, stable-sort by upper-case key, serialise. Used
    for the model side (over the unfiltered
    GetEnvironmentStringsW entries) and for the list-level
    judgement of the fixture a process environment cannot
    represent — never for a representable fixture's tree side,
    which the compiled helper produces by executing the tree's
    own environment_block."""
    kept = []
    for name, value in entries:
        if set_name is not None and ascii_eq(name, set_name):
            continue
        kept.append((name, value))
    if set_name is not None:
        kept.append((set_name, set_value))
    ordered = sorted(kept, key=lambda kv: kv[0].upper())
    text = "".join(f"{name}={value}\x00" for name, value in ordered)
    return (text + "\x00").encode("utf-16-le")


def parse_block(data):
    """Entries of a serialised UTF-16LE block, parsed back
    independently of either serialiser."""
    entries = []
    for chunk in data.decode("utf-16-le").split("\x00"):
        if not chunk:
            break
        cut = chunk.find("=", 1) if chunk.startswith("=") \
            else chunk.find("=")
        entries.append((chunk[:cut], chunk[cut + 1:]))
    return entries


def serialise_block(entries):
    text = "".join(f"{name}={value}\x00" for name, value in entries)
    return (text + "\x00").encode("utf-16-le")


HELPER_RS = r"""
// The battery case (l) enumeration helper: the tree's own
// environment builder, compiled in, plus the two enumerations.
// No dependencies; the Win32 walk is declared directly.
mod sys {
    pub mod windows {
        include!(__ENCODING_RS__);
    }
}

mod engine {
    #[path = __ENVIRONMENT_RS__]
    mod environment;
    // environment_block is pub(super) in the tree's module: it is
    // visible inside engine, and this wrapper carries it out.
    pub fn build(name: &str, value: &str) -> Vec<u16> {
        environment::environment_block(name, value)
    }
}

extern "system" {
    fn GetEnvironmentStringsW() -> *mut u16;
    fn FreeEnvironmentStringsW(ptr: *mut u16) -> i32;
}

fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out
}

fn entries_json(entries: &[(String, String)]) -> String {
    let parts: Vec<String> = entries
        .iter()
        .map(|(name, value)| {
            format!("[\"{}\",\"{}\"]", json_escape(name), json_escape(value))
        })
        .collect();
    format!("[{}]", parts.join(","))
}

fn gesw_entries() -> Vec<(String, String)> {
    let mut entries = Vec::new();
    unsafe {
        let ptr = GetEnvironmentStringsW();
        if ptr.is_null() {
            return entries;
        }
        let mut offset = 0usize;
        loop {
            let mut end = offset;
            while *ptr.add(end) != 0 {
                end += 1;
            }
            if end == offset {
                break;
            }
            let units = std::slice::from_raw_parts(ptr.add(offset), end - offset);
            let text = String::from_utf16_lossy(units);
            offset = end + 1;
            // A drive entry's name itself starts with '=' (=C:),
            // so its separator is the second '=' in the string.
            let cut = if text.starts_with('=') {
                text[1..].find('=').map(|i| i + 1)
            } else {
                text.find('=')
            };
            if let Some(cut) = cut {
                if cut > 0 {
                    entries.push((text[..cut].to_string(), text[cut + 1..].to_string()));
                }
            }
        }
        FreeEnvironmentStringsW(ptr);
    }
    entries
}

fn block_hex(units: &[u16]) -> String {
    let mut out = String::with_capacity(units.len() * 4);
    for unit in units {
        out.push_str(&format!("{:02x}{:02x}", unit & 0xFF, unit >> 8));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // args: SET_NAME SET_VALUE SENTINEL_NAME SENTINEL_VALUE
    let vars_os: Vec<(String, String)> = std::env::vars_os()
        .map(|(name, value)| {
            (name.to_string_lossy().into_owned(), value.to_string_lossy().into_owned())
        })
        .collect();
    let gesw = gesw_entries();
    let block_set = engine::build(&args[1], &args[2]);
    let block_sentinel = engine::build(&args[3], &args[4]);
    println!(
        "{{\"vars_os\": {}, \"gesw\": {}, \"block_set_hex\": \"{}\", \"block_sentinel_hex\": \"{}\"}}",
        entries_json(&vars_os),
        entries_json(&gesw),
        block_hex(&block_set),
        block_hex(&block_sentinel)
    );
}
"""


def build_helper(tree):
    """Compile the enumeration helper with the tree's own
    environment module included. Returns the helper's path, or
    None when no rustc is available or compilation fails (the
    caller records the fixture unrun, which the driver treats
    as BLOCKED-ERROR, never evidenced)."""
    rustc = shutil.which("rustc")
    if rustc is None:
        return None
    env_rs = os.path.join(tree, "src", "engine",
                          "environment.rs").replace("\\", "/")
    work = tempfile.mkdtemp(prefix="case-l-helper-")
    encoding_rs = os.path.join(tree, "src", "sys", "windows",
                               "encoding.rs").replace("\\", "/")
    source = HELPER_RS.replace("__ENVIRONMENT_RS__", json.dumps(env_rs))
    source = source.replace("__ENCODING_RS__", json.dumps(encoding_rs))
    source_path = os.path.join(work, "env_helper.rs")
    with open(source_path, "w", encoding="utf-8") as handle:
        handle.write(source)
    exe = os.path.join(work, "env_helper.exe")
    proc = subprocess.run([rustc, "--edition", "2021", "-O",
                           source_path, "-o", exe],
                          capture_output=True, text=True,
                          timeout=600)
    if proc.returncode != 0 or not os.path.isfile(exe):
        return None
    return exe


def spawn_helper_with_block(helper, entries, set_name, set_value,
                            sentinel_name, sentinel_value):
    """Create the helper with an explicit environment block via
    CreateProcessW; returns its parsed report, or None when
    creation failed."""
    block = build_block(entries)
    block_buf = ctypes.create_string_buffer(block, len(block))
    startup = StartupInfo()
    startup.cb = ctypes.sizeof(startup)
    info = ProcessInformation()
    read_r, read_w = os.pipe()
    write_r, write_w = os.pipe()
    try:
        # os.pipe() yields CRT file descriptors, not Win32
        # HANDLEs: every pipe end used as a standard handle is
        # converted with msvcrt.get_osfhandle, the child-side
        # ends (the stdin read end and the output write end)
        # are marked inheritable, and hStdError gets a real
        # handle too — the write end of the output pipe.
        os.set_inheritable(read_r, True)
        os.set_inheritable(write_w, True)
        startup.hStdOutput = ctypes.c_void_p(
            msvcrt.get_osfhandle(write_w))
        startup.hStdInput = ctypes.c_void_p(
            msvcrt.get_osfhandle(read_r))
        startup.hStdError = ctypes.c_void_p(
            msvcrt.get_osfhandle(write_w))
        startup.dwFlags = 0x00000100  # STARTF_USESTDHANDLES
        command = ctypes.create_unicode_buffer(
            f'"{helper}" "{set_name}" "{set_value}" '
            f'"{sentinel_name}" "{sentinel_value}"')
        created = kernel32.CreateProcessW(
            None, command, None, None, True,
            CREATE_UNICODE_ENVIRONMENT, block_buf, None,
            ctypes.byref(startup), ctypes.byref(info))
        if not created:
            return None
        # Close the parent-side copies of the child's ends:
        # the child holds its own duplicates now, and holding
        # the output write end open here would keep the read
        # below from ever seeing end-of-stream. The parent's
        # read end of the output pipe is write_r.
        os.close(write_w)
        write_w = -1
        os.close(read_r)
        read_r = -1
        out = b""
        while True:
            chunk = os.read(write_r, 65536)
            if not chunk:
                break
            out += chunk
        kernel32.WaitForSingleObject(info.hProcess, 120000)
        kernel32.CloseHandle(info.hProcess)
        kernel32.CloseHandle(info.hThread)
        return json.loads(out.decode("utf-8"))
    finally:
        for fd in (read_r, read_w, write_r, write_w):
            if fd >= 0:
                try:
                    os.close(fd)
                except OSError:
                    pass


def representable(entries):
    """Whether a real Windows process environment can hold this
    entry list: names are case-insensitive and unique per
    process, so duplicate case-folded names collapse."""
    folded = [ascii_fold(name) for name, _ in entries]
    return len(set(folded)) == len(folded)


def choose_sentinel(entries):
    for i in range(1000):
        candidate = f"PTG_CASE_L_SENTINEL_{i}"
        if not any(ascii_eq(name, candidate) for name, _ in entries):
            return candidate
    return None


def main():
    tree = os.path.normpath(os.path.join(HERE, "..", "..", ".."))
    if len(sys.argv) > 2 and sys.argv[1] == "--tree":
        tree = sys.argv[2]
    helper = build_helper(tree)
    results = []
    for fixture in env_fixtures.load_fixtures():
        entries = [tuple(e) for e in fixture["entries"]]
        can_install = representable(entries)
        if not can_install:
            # Judged at the list level: installing would destroy
            # the fixture before measurement.
            results.append({
                "name": fixture["name"],
                "representable": False,
                "route": "list-level",
                "builder_route": "list-level over the fixture's "
                                 "entry list (a process "
                                 "environment cannot represent "
                                 "duplicate case-folded names)",
                "vars_os_entries": [list(e) for e in entries],
                "gesw_entries": [list(e) for e in entries],
                "as_is": {
                    "tree_hex": builder_rule_block(
                        entries, None, None).hex(),
                    "model_hex": builder_rule_block(
                        entries, None, None).hex()},
                "with_set": {
                    "set_name": CHANNEL, "set_value": SET_VALUE,
                    "set_performed": True,
                    "tree_hex": builder_rule_block(
                        entries, CHANNEL, SET_VALUE).hex(),
                    "model_hex": builder_rule_block(
                        entries, CHANNEL, SET_VALUE).hex()},
            })
            continue
        report = None
        sentinel = choose_sentinel(entries)
        if helper is not None and sentinel is not None:
            report = spawn_helper_with_block(
                helper, entries, CHANNEL, SET_VALUE,
                sentinel, SENTINEL_VALUE)
        if report is None:
            results.append({"name": fixture["name"],
                            "representable": True})
            continue
        gesw_entries = [tuple(e) for e in report["gesw"]]
        # The as-is tree block: the real builder's block with
        # the sentinel entry removed — the builder's own output
        # for the fixture alone.
        sentinel_block = parse_block(
            bytes.fromhex(report["block_sentinel_hex"]))
        as_is_entries = [(n, v) for n, v in sentinel_block
                         if not (ascii_eq(n, sentinel)
                                 and v == SENTINEL_VALUE)]
        results.append({
            "name": fixture["name"],
            "representable": True,
            "route": "executed",
            "builder_route": BUILDER_ROUTE,
            "vars_os_entries": report["vars_os"],
            "gesw_entries": report["gesw"],
            "as_is": {
                "tree_hex": serialise_block(as_is_entries).hex(),
                "model_hex": builder_rule_block(
                    gesw_entries, None, None).hex()},
            "with_set": {
                "set_name": CHANNEL, "set_value": SET_VALUE,
                "set_performed": True,
                "tree_hex": report["block_set_hex"],
                "model_hex": builder_rule_block(
                    gesw_entries, CHANNEL, SET_VALUE).hex()},
        })
    print(json.dumps({"fixtures": results}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
