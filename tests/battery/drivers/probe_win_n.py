# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Windows probe for battery case (n); other platforms exit 3.

The charged console / pipe / file / closed matrix remains the adoption
contract. This fixture can run the pipe and file shapes, but DEVNULL cannot
stand in for a console or a closed standard handle. Those cells are recorded
as blocked before launching a role or API child, so the native matrix remains
BLOCKED-ERROR. The 0.4-second delay is not a finish acknowledgement.

Pipe shapes use an explicitly requested 64 KiB anonymous-pipe buffer and
require a 64 KiB receiving quota readback. A separate native regression
requires the unchanged 8,192-byte producer to exit before its parent drains.
Neither check establishes the real Console::finish timing or completes the
charged matrix. The driver keeps role output, measured pattern bytes, and
API verdict records for shapes it actually executes.
"""

import base64
import ctypes
import json
import os
import subprocess
import sys
import tempfile
from ctypes import wintypes

if sys.platform != "win32":
    sys.exit(3)

import msvcrt  # noqa: E402 - Windows-only module after the platform gate

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
# Explicit pointer-width HANDLE signatures: ctypes' default c_int truncates
# GetStdHandle results on 64-bit Windows.
kernel32.GetStdHandle.argtypes = (wintypes.DWORD,)
kernel32.GetStdHandle.restype = wintypes.HANDLE
kernel32.GetConsoleMode.argtypes = (wintypes.HANDLE,
                                    ctypes.POINTER(wintypes.DWORD))
kernel32.GetConsoleMode.restype = wintypes.BOOL
kernel32.FreeConsole.argtypes = ()
kernel32.FreeConsole.restype = wintypes.BOOL
kernel32.AttachConsole.argtypes = (wintypes.DWORD,)
kernel32.AttachConsole.restype = wintypes.BOOL
kernel32.CreatePipe.argtypes = (ctypes.POINTER(wintypes.HANDLE),
                                ctypes.POINTER(wintypes.HANDLE),
                                ctypes.c_void_p, wintypes.DWORD)
kernel32.CreatePipe.restype = wintypes.BOOL
kernel32.GetNamedPipeInfo.argtypes = (wintypes.HANDLE,
                                      ctypes.POINTER(wintypes.DWORD),
                                      ctypes.POINTER(wintypes.DWORD),
                                      ctypes.POINTER(wintypes.DWORD),
                                      ctypes.POINTER(wintypes.DWORD))
kernel32.GetNamedPipeInfo.restype = wintypes.BOOL
kernel32.CloseHandle.argtypes = (wintypes.HANDLE,)
kernel32.CloseHandle.restype = wintypes.BOOL
STD_INPUT_HANDLE = -10
STD_OUTPUT_HANDLE = -11
STD_ERROR_HANDLE = -12
HANDLES = (STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE)
PENDING_BYTES = 8192
PATTERN_BLOCK = b"PTG-PENDING-0001"  # 16 bytes
PATTERN_REPS = PENDING_BYTES // len(PATTERN_BLOCK)
ROLES = ("full", "deadline", "gate", "contain")
ROLE_EXES = {"full": "process-tree-guard.exe",
             "deadline": "process-tree-guard-deadline.exe",
             "gate": "process-tree-guard-gate.exe",
             "contain": "process-tree-guard-contain.exe"}


PIPE_REQUESTED_BYTES = 65536
MIN_PIPE_QUOTA_BYTES = PIPE_REQUESTED_BYTES
UNSUPPORTED_SHAPES = {
    "console": "DEVNULL is not a console standard handle",
    "closed": "DEVNULL is open, not a closed standard handle",
}


def buffered_pipe():
    """Return non-inheritable binary CRT fds and the measured read quota.

    Native handles belong to open_osfhandle after a successful conversion;
    never CloseHandle them as well. A failed CreatePipe has indeterminate
    output values, so there is nothing to close on that path.
    """
    read_handle = wintypes.HANDLE()
    write_handle = wintypes.HANDLE()
    if not kernel32.CreatePipe(ctypes.byref(read_handle),
                               ctypes.byref(write_handle), None,
                               PIPE_REQUESTED_BYTES):
        raise ctypes.WinError(ctypes.get_last_error())
    read_native, write_native = read_handle.value, write_handle.value
    read_fd = write_fd = None
    try:
        flags = wintypes.DWORD()
        outgoing = wintypes.DWORD()
        incoming = wintypes.DWORD()
        if not kernel32.GetNamedPipeInfo(read_native,
                                         ctypes.byref(flags),
                                         ctypes.byref(outgoing),
                                         ctypes.byref(incoming), None):
            raise ctypes.WinError(ctypes.get_last_error())
        # A zero reported buffer means allocated as needed, not proof that
        # this 8,192-byte workload can finish before its parent drains.
        if incoming.value < MIN_PIPE_QUOTA_BYTES:
            raise RuntimeError("pipe incoming quota is below the required "
                               f"headroom: {incoming.value} < "
                               f"{MIN_PIPE_QUOTA_BYTES}")
        read_fd = msvcrt.open_osfhandle(
            read_native, os.O_RDONLY | os.O_BINARY | os.O_NOINHERIT)
        read_native = None
        write_fd = msvcrt.open_osfhandle(
            write_native, os.O_WRONLY | os.O_BINARY | os.O_NOINHERIT)
        write_native = None
        result = (read_fd, write_fd, incoming.value)
        read_fd = write_fd = None
        return result
    finally:
        if read_fd is not None:
            os.close(read_fd)
        if write_fd is not None:
            os.close(write_fd)
        if read_native is not None:
            kernel32.CloseHandle(read_native)
        if write_native is not None:
            kernel32.CloseHandle(write_native)


def verdicts():
    out = []
    for which in HANDLES:
        handle = kernel32.GetStdHandle(which)
        mode = ctypes.c_uint32()
        out.append(bool(handle) and
                   kernel32.GetConsoleMode(handle, ctypes.byref(mode)) != 0)
    return out


def finish_cycle():
    kernel32.FreeConsole()
    kernel32.AttachConsole(ctypes.c_uint32(0xFFFFFFFF))  # parent


def produce_pending():
    """Write the API child's pattern before its simulated console cycle;
    return the number of bytes actually written."""
    produced = 0
    for fd in (1, 2):
        try:
            produced += os.write(fd, b"p" * (PENDING_BYTES // 2))
        except OSError:
            pass
    return produced


def write_pattern(fd):
    """Write the pending pattern to one descriptor; returns the
    pattern bytes actually written."""
    written = 0
    block = PATTERN_BLOCK * PATTERN_REPS
    try:
        while written < len(block):
            written += os.write(fd, block[written:])
    except OSError:
        pass
    return written


def count_pattern(data):
    if not data:
        return 0
    return data.count(PATTERN_BLOCK) * len(PATTERN_BLOCK)


def child_api_main(result_path, pending, pinned):
    eager = verdicts()
    produced = produce_pending() if pending else 0
    finish_cycle()
    record = {"eager": eager, "lazy": verdicts(),
              "produced_bytes": produced,
              "pinned_handles": (["output", "error"] if pinned
                                 and pending else [])}
    with open(result_path, "w", encoding="utf-8") as handle:
        json.dump(record, handle)
    return 0


def workload_main(marker_path, pending):
    """The workload the run roles execute. It first measures the
    console verdicts of the standard handles it was given, then
    — in a pending cell — writes the pending pattern after a delay.
    The delay does not establish that the real guard finished;
    its marker records verdicts and bytes actually written."""
    measured = verdicts()
    produced = 0
    if pending:
        import time
        time.sleep(0.4)  # simulated delay, not a finish ACK
        produced = write_pattern(2)
    with open(marker_path, "w", encoding="utf-8") as handle:
        json.dump({"produced_bytes": produced,
                   "verdicts": measured}, handle)
    return 0


def producer_main(marker_path):
    """The pending producer for gate cells: shares the gate's
    error handle (inherited at spawn), writes the pattern after
    a delay, and records bytes written; no real finish ACK exists."""
    import time
    time.sleep(0.4)
    produced = write_pattern(2)
    with open(marker_path, "w", encoding="utf-8") as handle:
        json.dump({"produced_bytes": produced}, handle)
    return 0


def contain_launcher_main(result_path, contain_exe, marker_path,
                          pending):
    """Runs inside the enclosing extendable run: executes the
    contain program on the workload (inheriting this process's
    standard handles, which the enclosing run arranged) and
    records the contain program's own exit code."""
    workload = [sys.executable, os.path.abspath(__file__),
                "--workload", marker_path, "1" if pending else "0"]
    proc = subprocess.run([contain_exe, "--timeout", "30", "--"]
                          + workload)
    with open(result_path, "w", encoding="utf-8") as handle:
        json.dump({"exit_code": proc.returncode}, handle)
    return 0


class Arrangement:
    """One role's standard handles and any readable pending output.

    Pipe and file shapes retain drainable stdout/stderr. A DEVNULL handle
    cannot establish either the console or closed shape; those charged
    cells are recorded as blocked before any role or API child launches.
    """

    def __init__(self, handles_kind):
        self.kind = handles_kind
        self.kwargs = {}
        self._out_r = self._err_r = None
        self._out_path = self._err_path = None
        self._files = []
        self.pipe_incoming_quota = None
        if handles_kind == "pipe":
            self._out_r, out_w, out_quota = buffered_pipe()
            try:
                self._err_r, err_w, err_quota = buffered_pipe()
            except BaseException:
                os.close(self._out_r)
                os.close(out_w)
                raise
            self.pipe_incoming_quota = (out_quota, err_quota)
            self.kwargs = {"stdin": subprocess.PIPE,
                           "stdout": out_w, "stderr": err_w}
            self._write_ends = (out_w, err_w)
        elif handles_kind == "file":
            self._out_path = self._temp_path()
            self._err_path = self._temp_path()
            out_f = open(self._out_path, "wb")
            err_f = open(self._err_path, "wb")
            self._files = [out_f, err_f]
            self.kwargs = {"stdin": subprocess.PIPE,
                           "stdout": out_f, "stderr": err_f}
        else:
            raise ValueError("unsupported standard-handle shape: "
                             f"{handles_kind}")

    @staticmethod
    def _temp_path():
        fd, path = tempfile.mkstemp()
        os.close(fd)
        return path

    def close_write_ends(self):
        for fd in getattr(self, "_write_ends", ()):
            try:
                os.close(fd)
            except OSError:
                pass
        self._write_ends = ()
        for handle in self._files:
            handle.close()
        self._files = []

    @staticmethod
    def _drain(fd):
        data = b""
        try:
            while True:
                chunk = os.read(fd, 65536)
                if not chunk:
                    break
                data += chunk
        except BaseException:
            try:
                os.close(fd)
            except OSError:
                pass  # keep the read failure primary
            raise
        os.close(fd)
        return data

    def drain_stdout(self):
        if self.kind == "pipe":
            return self._drain(self._out_r)
        if self.kind == "file":
            with open(self._out_path, "rb") as handle:
                return handle.read()
        return None

    def drain_stderr(self):
        if self.kind == "pipe":
            return self._drain(self._err_r)
        if self.kind == "file":
            with open(self._err_path, "rb") as handle:
                return handle.read()
        return None

    def producer_kwargs(self):
        """Spawn kwargs giving a pending producer this
        arrangement's error handle."""
        if self.kind == "pipe":
            return {"stderr": self._write_ends[1],
                    "stdout": subprocess.DEVNULL,
                    "stdin": subprocess.DEVNULL}
        if self.kind == "file":
            handle = open(self._err_path, "ab")
            self._files.append(handle)
            return {"stderr": handle, "stdout": subprocess.DEVNULL,
                    "stdin": subprocess.DEVNULL}
        raise ValueError("unsupported producer handle shape")


def read_json(path):
    if path and os.path.isfile(path):
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    return None


def temp_json_path():
    return os.path.join(tempfile.mkdtemp(), "record.json")


def held_for(arrangement, stderr_bytes, produced_bytes):
    """(held_output_bytes, held_pattern_bytes, how measured):
    both counts are the measured pattern bytes — drained from
    the stream where a drain exists, otherwise the producer's
    write record."""
    if stderr_bytes is not None:
        counted = count_pattern(stderr_bytes)
        return counted, counted, "pattern bytes counted in the drained stream"
    return produced_bytes, produced_bytes, "producer write record"


def spawn_role_cell(build_dir, role, handles_kind, pending):
    exe = os.path.join(build_dir, ROLE_EXES[role])
    if not os.path.isfile(exe):
        return {"exit_code": None, "role_present": False,
                "held_output_bytes": 0}
    if handles_kind in UNSUPPORTED_SHAPES:
        return {"exit_code": None, "role_present": True,
                "held_output_bytes": 0,
                "blocked_error": UNSUPPORTED_SHAPES[handles_kind]}
    arrangement = Arrangement(handles_kind)
    marker = temp_json_path()
    workload = [sys.executable, os.path.abspath(__file__),
                "--workload", marker, "1" if pending else "0"]
    producer_marker = temp_json_path()
    producer = None
    blocked_error = False
    if role == "gate":
        full_exe = os.path.join(build_dir, ROLE_EXES["full"])
        command = (f"{full_exe} --timeout 60 --shell powershell "
                   f"--command-b64 "
                   + base64.b64encode(b"echo OK").decode())
        payload = json.dumps(
            {"tool_name": "Bash",
             "tool_input": {"command": command}}).encode()
        proc = subprocess.Popen([exe, "pretooluse"],
                                stdin=subprocess.PIPE,
                                stdout=arrangement.kwargs.get("stdout"),
                                stderr=arrangement.kwargs.get("stderr"))
        if pending:
            producer = subprocess.Popen(
                [sys.executable, os.path.abspath(__file__),
                 "--pending-producer", producer_marker],
                **arrangement.producer_kwargs())
        proc.communicate(input=payload)
        exit_code = proc.returncode
        if producer is not None:
            producer.wait()
        arrangement.close_write_ends()
        decision_bytes = arrangement.drain_stdout()
        stderr_bytes = arrangement.drain_stderr()
        produced_record = read_json(producer_marker) or {}
        produced = int(produced_record.get("produced_bytes", 0))
        held, pattern, how = held_for(arrangement, stderr_bytes,
                                      produced)
        decision_text = decision_bytes.decode("utf-8",
                                              errors="replace")
        # The gate's decision shapes: allow is exit 0 with {}
        # (or an allow permission) on stdout; deny is exit 2
        # with a deny permission. Only drainable pipe/file shapes
        # reach this branch.
        if '"deny"' in decision_text or exit_code == 2:
            decision_verdict = "deny"
        elif decision_text.strip() == "{}" \
                or '"allow"' in decision_text or exit_code == 0:
            decision_verdict = "allow"
        else:
            decision_verdict = f"exit-{exit_code}"
        return {"exit_code": exit_code, "role_present": True,
                "held_output_bytes": held,
                "held_pattern_bytes": pattern,
                "held_measured": how,
                "decision_output_hex": decision_bytes.hex(),
                "decision_verdict": decision_verdict}
    if role in ("full", "deadline"):
        argv = [exe, "--timeout", "30", "--"] + workload
        proc = subprocess.Popen(argv, **arrangement.kwargs)
        proc.wait()
        exit_code = proc.returncode
    else:  # contain, inside a real enclosing extendable run
        result = temp_json_path()
        launcher = [sys.executable, os.path.abspath(__file__),
                    "--contain-launcher", result, exe, marker,
                    "1" if pending else "0"]
        full_exe = os.path.join(build_dir, ROLE_EXES["full"])
        proc = subprocess.Popen(
            [full_exe, "--extendable", "--timeout", "120", "--"]
            + launcher, **arrangement.kwargs)
        proc.wait()
        contain_record = read_json(result)
        if contain_record is None:
            blocked_error = True
            exit_code = proc.returncode
        else:
            exit_code = contain_record["exit_code"]
            if exit_code == 125:
                blocked_error = True
    arrangement.close_write_ends()
    decision_bytes = arrangement.drain_stdout()
    stderr_bytes = arrangement.drain_stderr()
    record = read_json(marker) or {}
    produced = int(record.get("produced_bytes", 0))
    held, pattern, how = held_for(arrangement, stderr_bytes,
                                  produced)
    cell = {"exit_code": exit_code, "role_present": True,
            "held_output_bytes": held,
            "held_pattern_bytes": pattern,
            "held_measured": how,
            "decision_output_hex": (decision_bytes or b"").hex(),
            "decision_verdict": record.get("verdicts")}
    if blocked_error:
        cell["blocked_error"] = True
    return cell


def spawn_api_cell(handles_kind, pending, pinned=False):
    if handles_kind in UNSUPPORTED_SHAPES:
        return {"blocked_error": UNSUPPORTED_SHAPES[handles_kind],
                "held_output_bytes": 0, "eager": None,
                "lazy": None, "pinned_handles": []}
    result = temp_json_path()
    arrangement = Arrangement(handles_kind)
    kwargs = dict(arrangement.kwargs)
    kwargs.pop("stdin", None)
    args = [sys.executable, os.path.abspath(__file__), "--child-api",
            result, "1" if pending else "0",
            "1" if pinned else "0"]
    proc = subprocess.Popen(args, **kwargs)
    proc.wait()
    arrangement.close_write_ends()
    record = read_json(result)
    if record is None:
        return None
    if handles_kind == "pipe":
        held = len(arrangement.drain_stdout() or b"") \
            + len(arrangement.drain_stderr() or b"")
    elif handles_kind == "file":
        held = len(arrangement.drain_stdout() or b"") \
            + len(arrangement.drain_stderr() or b"")
    else:
        held = int(record.get("produced_bytes", 0))
    return {"eager": record["eager"], "lazy": record["lazy"],
            "held_output_bytes": held,
            "pinned_handles": record["pinned_handles"]}


def pipe_producer_main():
    """Write the unchanged 8,192-byte pattern to inherited stderr."""
    block = PATTERN_BLOCK * PATTERN_REPS
    written = 0
    while written < len(block):
        count = os.write(2, block[written:])
        if count <= 0:
            return 2
        written += count
    return 0


def pipe_regression_main():
    """Prove the producer exits before its parent drains one fresh pipe."""
    read_fd, write_fd, incoming = buffered_pipe()
    proc = None
    try:
        proc = subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), "--pipe-producer"],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=write_fd)
        os.close(write_fd)
        write_fd = None
        # Ordering is deliberate: the full write must finish without any
        # parent read before this wait returns.
        native_exit = proc.wait(timeout=10)
        drain_fd = read_fd
        read_fd = None
        data = Arrangement._drain(drain_fd)
        passed = (native_exit == 0 and
                  data == PATTERN_BLOCK * PATTERN_REPS)
        print(json.dumps({"passed": passed,
                          "producer_exited_before_parent_drain": True,
                          "native_exit": native_exit,
                          "incoming_quota_bytes": incoming,
                          "pattern_bytes": len(data)}))
        return 0 if passed else 2
    finally:
        if proc is not None and proc.poll() is None:
            proc.kill()
            proc.wait(timeout=5)
        if write_fd is not None:
            os.close(write_fd)
        if read_fd is not None:
            os.close(read_fd)


def main():
    if len(sys.argv) == 2 and sys.argv[1] == "--pipe-producer":
        return pipe_producer_main()
    if len(sys.argv) == 2 and sys.argv[1] == "--pipe-regression":
        return pipe_regression_main()
    if len(sys.argv) > 4 and sys.argv[1] == "--child-api":
        return child_api_main(sys.argv[2], sys.argv[3] == "1",
                              sys.argv[4] == "1")
    if len(sys.argv) > 3 and sys.argv[1] == "--workload":
        return workload_main(sys.argv[2], sys.argv[3] == "1")
    if len(sys.argv) > 2 and sys.argv[1] == "--pending-producer":
        return producer_main(sys.argv[2])
    if len(sys.argv) > 5 and sys.argv[1] == "--contain-launcher":
        return contain_launcher_main(sys.argv[2], sys.argv[3],
                                     sys.argv[4],
                                     sys.argv[5] == "1")
    build_dir = "."
    if len(sys.argv) > 2 and sys.argv[1] == "--build-dir":
        build_dir = sys.argv[2]
    cells = []
    for role in ROLES:
        for handles in ("console", "pipe", "file", "closed"):
            for pending in (False, True):
                got = spawn_role_cell(build_dir, role, handles,
                                      pending)
                cells.append({"kind": "role", "handles": handles,
                              "role": role,
                              "pending_output": pending, **got})
    unequal = []
    for handles in ("console", "pipe", "file", "closed"):
        for pending in (False, True):
            got = spawn_api_cell(handles, pending)
            if got is None:
                continue
            cells.append({"kind": "api", "handles": handles,
                          "pending_output": pending,
                          "held_output_bytes":
                              got["held_output_bytes"],
                          "eager": got["eager"],
                          "lazy": got["lazy"],
                          **({"blocked_error": got["blocked_error"]}
                             if "blocked_error" in got else {})})
            if got["eager"] != got["lazy"]:
                unequal.append((handles, pending))
    fallback_cells = []
    for handles, pending in unequal:
        got = spawn_api_cell(handles, pending, pinned=True)
        if got is not None:
            fallback_cells.append({
                "handles": handles, "pending_output": pending,
                "eager": got["eager"], "lazy": got["lazy"],
                "pinned_handles": got["pinned_handles"]})
    print(json.dumps({
        "cells": cells,
        "fallback": {"needed": bool(unequal),
                     "cells": fallback_cells}}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
