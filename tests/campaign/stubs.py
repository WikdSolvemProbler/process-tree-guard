# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Stub programs with the shapes the harness drives, for logic
exercises on any platform. A stub is not a competitor and is never
measured as one: stub runs exercise slot execution, classification
and report generation, and their timings prove nothing.

Usage:
  stubs.py exit0                 exit at once with code 0 (fast child
                                 and first floor program)
  stubs.py exit0b                exit at once with code 0 (second
                                 floor program: a distinct entry
                                 point with the same trivial shape)
  stubs.py relay CODE            the output child, with the same
                                 shape as the guard tree's testkit
                                 "relay": copy standard input to
                                 standard output, add one newline,
                                 write "error-stream" to standard
                                 error, and exit with CODE
  stubs.py sleep SECONDS         sleep, then exit 0
  stubs.py tree MARKER SECONDS   tree-shaped kill child: records its
                                 own PID at MARKER.child, spawns a
                                 grandchild that records its PID at
                                 MARKER, then sleeps SECONDS
  stubs.py wide COUNT IDS CHILD_SECONDS HOLD_SECONDS
                                 width-shaped scaling child (the
                                 testkit "wide" shape): records its
                                 own PID at IDS's sibling child.pid,
                                 spawns COUNT sleepers, writes their
                                 PIDs one per line to IDS, then
                                 holds HOLD_SECONDS
  stubs.py deep LEVEL MAX DIR SECONDS
                                 depth-shaped scaling child (the
                                 testkit "deep" shape): records its
                                 own PID at DIR/LEVEL.pid, spawns
                                 the next level, then sleeps SECONDS
  stubs.py tree-leaf MARKER SECONDS
                                 the grandchild: records its own PID
                                 at MARKER, then sleeps SECONDS
  stubs.py arm-fast TIMEOUT -- CHILD ARGS...   fast arm: runs the
                                 child and propagates its exit code
  stubs.py arm-fast-mute TIMEOUT -- CHILD ARGS...
                                 fast arm that runs the child with
                                 its standard streams sent to the
                                 null device and propagates its exit
                                 code: the output negative control
                                 (the expected exit code with the
                                 child's output suppressed)
  stubs.py arm-fast-garble TIMEOUT -- CHILD ARGS...
                                 fast arm that runs the child,
                                 replaces its standard output with
                                 an altered line, passes its
                                 standard error through, and
                                 propagates its exit code: the
                                 output-alteration negative control
  stubs.py arm-kill TIMEOUT -- CHILD ARGS...   kill arm that kills
                                 only the direct child at TIMEOUT
                                 and exits 124; a grandchild of a
                                 tree-shaped child survives it
  stubs.py arm-kill-tree TIMEOUT -- CHILD ARGS...
                                 kill arm that kills the child's
                                 whole process group at TIMEOUT and
                                 exits 124; no descendant survives it
  stubs.py survivor-check [PID ...]
                                 exits 1 when any named PID is alive,
                                 0 when none is (with no PIDs, 0)
  stubs.py launch PROGRAM ARGS...
                                 intermediate launcher: runs the named
                                 program, waits for it, propagates its
                                 exit code (the pricing probes' owner
                                 shape at stub tier)
"""

import os
import signal
import subprocess
import sys
import time


def _publish(path, text):
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as handle:
        handle.write(text)
    os.replace(tmp, path)


def _pid_alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def main(argv):
    if not argv:
        return 97
    mode, rest = argv[0], argv[1:]
    if mode in ("exit0", "exit0b"):
        return 0
    if mode == "relay":
        data = sys.stdin.buffer.read()
        sys.stdout.buffer.write(data + b"\n")
        sys.stdout.buffer.flush()
        sys.stderr.write("error-stream\n")
        sys.stderr.flush()
        return int(rest[0])
    if mode == "sleep":
        time.sleep(float(rest[0]))
        return 0
    if mode == "tree":
        marker, seconds = rest[0], float(rest[1])
        _publish(marker + ".child", str(os.getpid()))
        subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), "tree-leaf",
             marker, rest[1]])
        time.sleep(seconds)
        return 0
    if mode == "tree-leaf":
        marker, seconds = rest[0], float(rest[1])
        _publish(marker, str(os.getpid()))
        time.sleep(seconds)
        return 0
    if mode == "wide":
        count, ids_path = int(rest[0]), rest[1]
        child_seconds, hold = rest[2], float(rest[3])
        _publish(os.path.join(os.path.dirname(ids_path), "child.pid"),
                 str(os.getpid()))
        children = [subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), "sleep",
             child_seconds]) for _ in range(count)]
        _publish(ids_path, "".join(f"{c.pid}\n" for c in children))
        time.sleep(hold)
        return 0
    if mode == "deep":
        level, maximum, directory = int(rest[0]), int(rest[1]), rest[2]
        seconds = float(rest[3])
        _publish(os.path.join(directory, f"{level}.pid"),
                 str(os.getpid()))
        if level < maximum:
            subprocess.Popen(
                [sys.executable, os.path.abspath(__file__), "deep",
                 str(level + 1), str(maximum), directory, rest[3]])
        time.sleep(seconds)
        return 0
    if mode == "launch":
        if not rest:
            return 97
        return subprocess.run(rest).returncode
    if mode == "survivor-check":
        for text in rest:
            if _pid_alive(int(text)):
                return 1
        return 0
    if mode == "arm-fast-mute":
        child = rest[2:]
        proc = subprocess.Popen(child, stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL)
        return proc.wait()
    if mode == "arm-fast-garble":
        child = rest[2:]
        done = subprocess.run(child, capture_output=True)
        sys.stdout.write("ptg-output-garbled\n")
        sys.stdout.flush()
        sys.stderr.buffer.write(done.stderr)
        sys.stderr.buffer.flush()
        return done.returncode
    if mode in ("arm-fast", "arm-kill", "arm-kill-tree"):
        timeout = float(rest[0])
        child = rest[2:]
        if mode == "arm-kill-tree":
            proc = subprocess.Popen(child, start_new_session=True)
        else:
            proc = subprocess.Popen(child)
        if mode == "arm-fast":
            return proc.wait()
        try:
            return proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            if mode == "arm-kill-tree":
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except (ProcessLookupError, PermissionError):
                    proc.kill()
            else:
                proc.kill()
            proc.wait()
            return 124
    return 97


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
