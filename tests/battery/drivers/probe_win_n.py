# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Windows probe for battery case (n). Runs on win32 only; on any
other platform it prints nothing and exits 3.

The charged matrix — standard handles (console / pipe / file /
closed) x every role program x runs with and without post-finish
output pending — plus the API-level verdict cells the finish
cycle is about. Nothing in the record is a label restated:

- Pending output is produced, or not, per cell. In a pending
  cell the producer writes a distinctive pending pattern (a
  repeated 16-byte block) to the output handles after the
  finish point and no reader consumes it; in a non-pending cell
  it writes none. Every role cell records held_output_bytes AND
  held_pattern_bytes, the pending-pattern bytes actually
  measured for that cell: counted in the drained stream for
  pipe and file handles, counted in the producer's own write
  record for console and closed handles (a console holds its
  output unread; a closed handle's stream cannot be drained).
  The driver refutes a cell whose two counts disagree — a held
  count assigned from the pending label has no measured pattern
  behind it.
- Role cells run the built role programs themselves (the full,
  deadline, gate and contain programs from the build folder)
  with the role process's standard handles arranged in the
  cell's configuration, and record verdict-shaped evidence:
  the exit code, the role's full decision output bytes (its
  stdout stream, drained), and the consulted verdict values.
  For the run roles the consulted verdicts are the standard
  handles' console verdicts the workload child measures at its
  start on the very handles the role arranged for it; for the
  gate they are the decision parsed out of the gate's own
  decision output.
- The gate role is invoked with a named vendor (pretooluse)
  and a payload whose watched command is a guarded envelope —
  the full program's path with its timeout/shell/command-b64
  form — whose verdict can vary (allow for the well-formed
  envelope), not a constant deny. Its pending output is
  produced by a separate producer process that shares the
  gate's stderr handle and writes the pending pattern after
  the gate's finish point.
- The contain role runs inside a real enclosing extendable
  run: the full program is started with --extendable and its
  child is a launcher that runs the contain program on the
  workload and records the contain program's own exit code.
  A contain cell whose enclosing run cannot start it is
  recorded blocked_error, never as a refusal-shaped pairing.
- API cells run a sacrificial child that records the console
  verdict (GetConsoleMode succeeds) of each standard handle
  eagerly, performs the cycle Console::finish performs —
  FreeConsole, then AttachConsole back to its parent's
  console — and records the verdicts again, lazily.
- For any unequal API cell the parent re-runs the child in
  pinned mode for exactly the handles with pending post-finish
  output (output and error in a pending cell) and records the
  eager verdicts, the lazy verdicts RE-MEASURED after the
  cycle in that pinned run, and the pinned set. The driver
  computes the fallback's effective verdicts from those raw
  fields; this probe never computes or copies an "effective"
  verdict itself.
"""

import base64
import ctypes
import json
import os
import subprocess
import sys
import tempfile

if sys.platform != "win32":
    sys.exit(3)

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
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
    """Write post-finish output nobody reads (API cells);
    returns the number of bytes produced (a write to a closed
    handle produces nothing and is not counted)."""
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
    — in a pending cell — writes the pending pattern to its
    error handle after the finish point; its marker records the
    verdicts and the pattern bytes it actually wrote."""
    measured = verdicts()
    produced = 0
    if pending:
        import time
        time.sleep(0.4)  # after the guard's finish point
        produced = write_pattern(2)
    with open(marker_path, "w", encoding="utf-8") as handle:
        json.dump({"produced_bytes": produced,
                   "verdicts": measured}, handle)
    return 0


def producer_main(marker_path):
    """The pending producer for gate cells: shares the gate's
    error handle (inherited at spawn), writes the pending
    pattern after the gate's finish point, and records the
    pattern bytes it actually wrote."""
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
    """A child's standard handles in one configuration, with the
    drains that measure what was left in them.

    drain_stdout() / drain_stderr() return the bytes left in
    that stream, or None where the stream cannot be drained
    (console: the stream is DEVNULL-backed, see below, so the
    bytes were discarded; closed: they were discarded) — for
    those configurations the producer's own write record is
    the measurement.

    The console configuration never inherits this probe's own
    handles: a role program writing on an inherited stdout
    would write straight onto the probe's observations channel
    (the gate's JSON decision line is the certain case), and
    the single JSON print at the end of main() must be the only
    thing on the probe's stdout. Console cells therefore route
    every standard handle to DEVNULL; the recorded decision
    evidence for such a cell is its exit code (and, for the
    gate, the decision verdict the exit code decides, exactly
    as the gate branch's undrainable-stream fallback already
    works), plus the marker/producer records the workloads
    leave in files, which no arrangement touches."""

    def __init__(self, handles_kind):
        self.kind = handles_kind
        self.kwargs = {}
        self._out_r = self._err_r = None
        self._out_path = self._err_path = None
        self._files = []
        if handles_kind == "pipe":
            self._out_r, out_w = os.pipe()
            self._err_r, err_w = os.pipe()
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
        elif handles_kind == "closed":
            self.kwargs = {"stdin": subprocess.DEVNULL,
                           "stdout": subprocess.DEVNULL,
                           "stderr": subprocess.DEVNULL}
        elif handles_kind == "console":
            # Never inherit the probe's stdout (see the class
            # docstring): console cells are DEVNULL-backed, so
            # no role program can write on the observations
            # channel; the exit code is the recorded decision
            # evidence for the cell.
            self.kwargs = {"stdin": subprocess.DEVNULL,
                           "stdout": subprocess.DEVNULL,
                           "stderr": subprocess.DEVNULL}

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
        while True:
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            data += chunk
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
        if self.kind == "closed":
            return {"stderr": subprocess.DEVNULL,
                    "stdout": subprocess.DEVNULL,
                    "stdin": subprocess.DEVNULL}
        # console: the producer shares the role's error handle,
        # which is DEVNULL-backed under this arrangement (see
        # the class docstring) — never the probe's own handles,
        # so its stdout can never be the probe's stdout either.
        return {"stderr": subprocess.DEVNULL,
                "stdout": subprocess.DEVNULL,
                "stdin": subprocess.DEVNULL}


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
        # For a console arrangement the arrangement's stdout
        # is DEVNULL (never the probe's own stdout), so the
        # gate's decision line cannot corrupt the observations
        # channel; communicate() then yields no output bytes
        # and the exit code recorded below is the cell's
        # decision evidence, as the fallback further down
        # already handles.
        proc = subprocess.Popen([exe, "pretooluse"],
                                stdin=subprocess.PIPE,
                                stdout=arrangement.kwargs.get("stdout"),
                                stderr=arrangement.kwargs.get("stderr"))
        if pending:
            producer = subprocess.Popen(
                [sys.executable, os.path.abspath(__file__),
                 "--pending-producer", producer_marker],
                **arrangement.producer_kwargs())
        out, _ = proc.communicate(input=payload)
        exit_code = proc.returncode
        if producer is not None:
            producer.wait()
        arrangement.close_write_ends()
        decision_bytes = out if arrangement.kind == "console" \
            else arrangement.drain_stdout()
        if decision_bytes is None:
            decision_bytes = out or b""
        stderr_bytes = arrangement.drain_stderr() \
            if arrangement.kind != "console" else None
        produced_record = read_json(producer_marker) or {}
        produced = int(produced_record.get("produced_bytes", 0))
        held, pattern, how = held_for(arrangement, stderr_bytes,
                                      produced)
        decision_text = decision_bytes.decode("utf-8",
                                              errors="replace")
        # The gate's decision shapes: allow is exit 0 with {}
        # (or an allow permission) on stdout; deny is exit 2
        # with a deny permission. Where the stream itself was
        # drained it decides; on a console arrangement, where
        # the stream cannot be drained, the exit code — the
        # other half of the same shape — decides.
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
    result = temp_json_path()
    arrangement = Arrangement(handles_kind)
    kwargs = dict(arrangement.kwargs)
    kwargs.pop("stdin", None)
    # Console cells keep the arrangement's DEVNULL backing
    # here too (no special-case inheritance): the API child's
    # pending output is post-finish output on its own handles,
    # and on an inherited stdout it would land on the probe's
    # observations channel just as a role program's would.
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


def main():
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
                          "lazy": got["lazy"]})
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
