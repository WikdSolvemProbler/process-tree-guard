# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Slot execution: one slot is one timed outer lifetime.

The endpoint is the whole outer lifetime of the program under test
plus its included controller work and cleanup: the clock starts
before the arm process is created and stops only after the arm has
exited AND the slot's cleanup check has run (on the kill path, the
check that no descendant of the child survives). A slot whose cleanup
check finds a survivor is classified by that fact, not by its time.

Kill-path shape: the kill child is a tree. The child spawns a
grandchild that records its PID and sleeps (on the Windows machine,
testkit "spawn" running testkit "mark", the shape the guard's own
kill-path family uses; in logic exercises, the stub tree, which also
records the child's own PID). Each kill slot gets its own marker
file: the configuration names it with the {marker} placeholder and
{self} for the child program's own path. The observation record
carries the tree's identity — marker path, child PID where the
workload records it, grandchild PID, and the arm's PID — and the
survivor check is the shipped probe (probe.py) invoked with the
recorded tree PIDs (the child PID where the workload
records it, and the grandchild PID), per slot, before the slot's
elapsed time closes. A kill slot whose probe is not configured, cannot run, or
finds no recorded identity is infrastructure failure: survival was
not verified, and the slot is never "ok" with survivor null.

Exit codes are checked against the per-arm expected-exit table
(arms.expected_exit): fast slots must propagate the child's code;
kill slots must produce the arm's documented timeout code. A
mismatch is classified "exit-mismatch", distinctly from
infrastructure failure, and is excluded from every median.

Output observation (profile dimension (1)'s output component):
the fast child is an output workload — it emits a known stdout
line and a known stderr line (the shipped child is testkit
"relay": the runner feeds it the configured stdin text on a
regular file, and relay copies that text plus one newline to
stdout and writes its fixed "error-stream" line to stderr; the
stub child of the same name has the identical shape). Every
fast-path slot — product arms and both floor arms, every kind —
runs with its stdout and stderr redirected to per-slot regular
files, never to the null device and never to pipes (a pipe the
harness drains late could block a writing child and distort the
slot's lifetime; a file cannot), and with stdin taken from a
regular file holding the child spec's configured stdin text
(the null device when the spec names none). After the arm
exits, still inside the slot's elapsed time, the harness reads
the two files back and records, per stream, the emitted bytes'
SHA-256 and length and — when the child spec in the
configuration names the expected bytes for that stream
("expected_stdout" / "expected_stderr", configuration data
identical for every arm) — the match verdict. A fast slot whose
exit code matches but whose output does not is classified
"output-mismatch", distinctly from exit-mismatch and from
infrastructure failure, and is excluded from every median.
Kill-path slots carry no output observation (their output
fields stay null and their streams stay on the null device):
the kill child's observable product is the tree it leaves
behind — the PID marker the survivor probe judges — not a byte
stream, and no arm is compared on bytes it wrote while being
killed; dimension (1)'s kill component is kill-path
correctness, and its output component is observed on the fast
path, where output passthrough is the arm's ordinary work.

Every slot runs under a finite deadline. A slot that exceeds its
deadline is infrastructure failure, recorded as such, never silently
discarded, and its processes are cleaned up by the harness's own
bounded kill of the arm process (the harness makes no tree claim for
itself; survivor state is exactly what the cleanup check records).
A deadline-killed slot stays infrastructure failure whatever the
cleanup check answers: survival may be recorded in the record and
its detail, but it must not reclassify the slot — the arm never
returned, so its slot is not descendant-survived evidence about
anybody's kill path. The survivor-before-exit-code precedence
applies only to arms that returned inside the deadline: a
returned kill arm that left a survivor is classified
"descendant-survived" even when its exit code also mismatches
the expected code (one label carries the graver fault; the
report lists the exit fault separately, under its own name,
and does not call the record inconsistent for carrying the
survival label).

Console contexts: NO_WINDOW maps to CREATE_NO_WINDOW and DETACHED to
DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP on Windows. On any other
platform the context is recorded but not applied, and the run is a
logic exercise, not a measurement (the report says so).

Malformed-input slots (profile dimension (6), round-26 F1): a
malformed slot presents one named shape (plan.MALFORMED_SHAPES) to
the arm through the arm's own fast-path template — the timeout
spelled as raw text where the shape is a timeout, the child
program or its arguments replaced where the shape is a command
shape (see malformed_argv). The arm's exit code IS the
observation, with one designated-only exception (round-31 F2):
for the designated arm, the shapes its own contract pins carry
a pinned expected code (plan.malformed_expected_exit — the
documented refusal, 125), and a designated slot answering a
pinned shape with any other code is classified "exit-mismatch"
by the ordinary expected-exit rule below, exactly as on the
fast and kill paths. The shape read here is the SLOT's own
variant — the plan identity the slot was built from — and
the rule binds warmup and measured slots alike; the report
checker applies the same expectation through the one shared
formulation (plan.malformed_pin_violation, round-32 F2) to
records of both kinds, reading the shape from the record's
slot identity, never from a record's carried variant label. Every other malformed slot — every
competitor slot, and the designated arm's unpinned shape —
carries no expected code, and a slot whose arm spawned and
returned inside the deadline is classified "ok" — presented,
and answered with the recorded code. A shape
the harness itself cannot present on this platform (a lone
surrogate argument the platform's exec encoding refuses) is an
infrastructure failure naming that limit, never a pass and never
an arm verdict; on Windows, whose process creation is wide-text,
the shape presents as written.

Scaling slots (profile dimension (4), round-26 F4): a scaling
slot is a kill slot whose child builds a tree of the slot's named
width or depth (plan.SCALE_VARIANTS) through the configured scale
children (the testkit "wide" / "deep" shapes on Windows; the stub
equivalents at logic tier). Its marker is a directory the child
publishes its tree's PIDs into; the survivor probe judges every
published PID exactly as on the kill path, and the classification
rules are the kill path's. The published count is held to the
variant (round-27 F1): a wide-N record must publish exactly N
descendant PIDs and a deep-D record exactly D (the chain starts
at level 1, the in-tree convention of plan.py's derivation, so
deep-D has D levels each publishing its PID); a slot whose tree
published fewer — the arm's fixed timeout truncating a
sequential build is the standing case — is an infrastructure
failure, never "ok", at every count below the variant's, not
only at zero.

Resource instrumentation (profile dimension (3), round-26 F3):
every record carries the arm process's own resource counters —
"cpu_ms" (the arm's user + system CPU time) and
"peak_working_set_bytes" (its peak resident/working set) — read
from the platform's per-process accounting at slot close: on
POSIX, the rusage of the wait4 reap that ends the slot (exact for
that child; ru_maxrss is KiB on Linux, bytes on macOS); on
Windows, GetProcessTimes and GetProcessMemoryInfo on the arm's
process handle after its wait. A field the platform cannot
honestly supply for a slot is null and "resource_note" names why
— absent-with-reason, never silently zero. The floor arms are
processes too and carry the same fields.
"""

import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import time

from arms import arm_executable, expected_exit, expand_template, \
    expand_template_text, \
    materialized_candidate_pending, \
    materialized_child_args_proven, \
    materialized_list, resolve_template

SLOT_DEADLINE_S = 30.0          # harness deadline for any one slot
FAST_TIMEOUT_S = 10.0           # arm timeout on the fast path: never fires
KILL_TIMEOUT_S = 2.0            # arm timeout on the kill path: the deadline
KILL_CHILD_SLEEP_S = 30.0       # child outlives its deadline if not killed
FAST_CHILD_EXIT = 0
PROBE_DEADLINE_S = 10.0         # finite deadline for one probe call

# Child workloads are supplied by configuration: on the Windows
# machine they are the guard tree's own test helper (testkit) or an
# equivalent supplied sleeper/exiter. The harness ships Python stubs
# (stubs.py) with the same shapes for logic exercises anywhere.


def _materialized_args(spec):
    """The child spec's args as a fresh list, materialized
    ONCE (round-41 F4): a value stated as a one-shot iterator
    (a generator above all) is read to a list here, so every
    later read of the shared configuration object — this
    module's other constructions and the checker's budget
    derivation alike (cmd_run shares one configuration
    object between the runner and the checker) — sees the
    same full vector, never an exhausted iterator.
    Round-42 F3/F4: the materialization never requires a
    mutable mapping and is confined to the values that
    need it — a one-shot iterator's list is carried by
    arms' identity cache (and written back into the spec
    only where the spec accepts the assignment; a refusal
    — a MappingProxyType spec, a frozen dict — is
    absorbed, never raised, and the cache serves every
    later read identically, exactly as the as-found
    construction read such a spec without raising), while
    a re-iterable non-list value (a string, a dict, a
    tuple) is read through a local list() and never
    rewritten in the caller's spec. A value already stated
    as a list is copied, as before, with no write-back.
    Round-43 F3/F4: the read goes through arms'
    materialized_child_args — the one shared read every
    consumer of the spec uses (this construction, the
    checker's derivation, the pricing probes) — and the
    confinement's test is one-shot-ness of the value
    (is_one_shot_value), not iterator identity: a one-shot
    iterable, or a list subclass carrying a one-shot
    payload, is materialized once on the same terms as a
    generator, while a provably re-iterable value (a
    plain list included) is copied locally and never
    rewritten in the caller's spec.
    Round-44 F4: the shared read and the write-back are
    confined to a value PROVEN one-shot
    (is_proven_one_shot_value — an iterator in fact, or
    successive iter() calls handing back the same
    iterator). A merely unprovable value (the
    conservative third disjunct of is_one_shot_value) is
    copied through a local list() exactly as a provably
    re-iterable value is: it may be genuinely re-iterable
    over a live backing, and a local read sees that
    backing as its owner states it at this moment,
    neither freezing it by write-back nor serving it a
    cached snapshot.
    Round-45 F5: that local-read split is abolished —
    it made this construction read a different value
    than the derivation's cached read of the same
    object (an owner's append visible here and frozen
    there; a per-call payload alternating here against
    a fixed budget there), so the budget was no longer
    a function of the construction. Now EVERY value is
    read through the one shared read
    (arms.materialized_child_args_proven): a provably
    re-iterable value is read live by that read itself,
    on every consumer alike, and any other value —
    proven or merely unprovable — is served the first
    materialization the identity cache holds, the same
    tokens the derivation prices and the pricing probes
    read, in any reader order. Round-45 F3/F4: the
    write-back below fires only on the proven
    classification the shared read derives from its
    own materialization (the repaired breadth: a
    traversal that cannot restart by construction) —
    a merely unprovable value, and a restarting
    iterator, are served the shared read and never
    rewritten in the caller's spec. Round-49 F4: the
    write-back is gated ALSO on no candidate pending
    on the slot (arms.materialized_candidate_pending):
    this construction may itself BE the ambiguous
    read, and while a deferred candidate pends the
    interim value is unproven for write-back purposes
    — the interim candidate is never planted into the
    caller's spec, so the consumer-first sequence and
    the materialized-only sequence over the same spec
    reach the same settlement, and no write destroys
    the traversal-persistence evidence a following
    settlement reads. Round-50 F2: the gate is judged
    on the state the consumer's read found, not the
    state it left — the no-pending consultation is
    taken BEFORE this construction's own read as well
    as after it, so a construction whose read SETTLES
    a candidate (its read found the candidate pending)
    writes nothing back in its own call, and one whose
    read creates a candidate writes nothing either: a
    settling read plants nothing, and the
    consumer-first and materialized-only sequences
    over the same factory reach the same settlement
    and serve the owner's replacement identically."""
    candidate_at_read_start = \
        materialized_candidate_pending(spec)
    materialized, proven = materialized_child_args_proven(spec)
    if proven and not candidate_at_read_start \
            and not materialized_candidate_pending(spec):
        try:
            spec["args"] = list(materialized)
        except Exception:
            pass
    return list(materialized)


def child_for(slot, config, marker=None):
    """(child_program, child_args) for one slot.

    Placeholders in the configured arguments: {sleep_s} is the kill
    child's sleep, {marker} the slot's own marker file, {self} the
    child program's own path (so a tree child can name itself as the
    program it spawns)."""
    children = config["children"]
    if slot.path == "fast":
        spec = children["fast"]
        return spec["program"], _materialized_args(spec)
    spec = children["kill"]

    def fill(text):
        out = text.replace("{sleep_s}", "%g" % KILL_CHILD_SLEEP_S)
        out = out.replace("{self}", spec["program"])
        if marker is not None:
            out = out.replace("{marker}", marker)
        return out

    return spec["program"], [fill(a) for a in _materialized_args(spec)]


def output_spec_for(slot, config):
    """The child spec whose output expectation governs one fast
    slot: the floor-spawn arm runs its own minimal program, every
    other fast slot runs the fast child. The spec may name
    "stdin" (the text fed to the child on a regular file),
    "expected_stdout" and "expected_stderr" (the exact byte
    strings, as text, the child must emit on each stream)."""
    children = config.get("children", {})
    if slot.arm == "floor-spawn":
        return children.get("floor_spawn") or children.get("fast") or {}
    return children.get("fast") or {}


def _observe_output(record, spec, out_path, err_path):
    """Read one fast slot's captured streams back into its record:
    per stream, the emitted bytes' SHA-256 and length, and the
    match verdict when the child spec names the expected bytes
    for that stream (no expectation named: the digest is recorded
    and the verdict stays null — observed bytes, no judgement)."""
    for stream, path, key in (
            ("stdout", out_path, "expected_stdout"),
            ("stderr", err_path, "expected_stderr")):
        try:
            with open(path, "rb") as handle:
                data = handle.read()
        except OSError:
            continue
        record[f"output_{stream}_sha256"] = hashlib.sha256(
            data).hexdigest()
        record[f"output_{stream}_len"] = len(data)
        expected = spec.get(key)
        if isinstance(expected, str):
            record[f"output_{stream}_match"] = (
                data == expected.encode("utf-8"))


NONEXISTENT_CHILD = "ptg-no-such-program-9f3b.exe"
OVER_LONG_ARGUMENT = "A" * 4096
NON_UNICODE_ARGUMENT = "\ud800ptg-non-unicode"


def malformed_argv(slot, config):
    """The argv presenting one malformed shape to one arm, built
    through the arm's own fast-path template (the reviewer's
    executed proof: the shapes go through the template fields —
    timeout text, child, child_args — on every arm)."""
    exe = arm_executable(slot.arm, config)
    spec = config["children"]["fast"]
    program = spec["program"]
    args = _materialized_args(spec)
    timeout_s_text = "%g" % FAST_TIMEOUT_S
    timeout_ms_text = str(int(round(FAST_TIMEOUT_S * 1000)))
    child, child_args = program, args
    shape = slot.variant
    if shape == "nonexistent-program":
        child, child_args = NONEXISTENT_CHILD, []
    elif shape == "malformed-timeout":
        timeout_s_text = timeout_ms_text = "not-a-timeout"
    elif shape == "out-of-range-timeout":
        timeout_s_text = "999999999"
        timeout_ms_text = "999999999000"
    elif shape == "empty-command":
        child, child_args = "", []
    elif shape == "over-long-argument":
        child_args = args + [OVER_LONG_ARGUMENT]
    elif shape == "non-unicode-argument":
        child_args = args + [NON_UNICODE_ARGUMENT]
    else:
        raise ValueError(f"unknown malformed shape {shape!r}")
    template = resolve_template(slot.arm, "fast", config)
    return [exe] + expand_template_text(
        template, timeout_s_text, timeout_ms_text, child, child_args)


def scale_child_for(slot, config, marker_dir):
    """(child_program, child_args) building the slot's named tree:
    the configured scale_wide / scale_deep child spec with its
    placeholders filled ({width} / {depth}, {ids} the wide child's
    PID file inside the marker directory, {dir} the marker
    directory itself, {sleep_s} and {self} as on the kill path).
    Returns None when the configuration names no scale child for
    the variant — the slot then records an infrastructure failure
    (the shape was not presented), never a crash."""
    import plan as plan_mod
    variants = {name: (w, d) for name, w, d in plan_mod.SCALE_VARIANTS}
    width, depth = variants[slot.variant]
    children = config.get("children", {})
    spec = children.get("scale_wide" if width is not None
                        else "scale_deep")
    if not isinstance(spec, dict) or "program" not in spec:
        return None

    def fill(text):
        out = text.replace("{sleep_s}", "%g" % KILL_CHILD_SLEEP_S)
        out = out.replace("{self}", spec["program"])
        out = out.replace("{dir}", marker_dir)
        out = out.replace("{ids}", os.path.join(marker_dir, "ids"))
        if width is not None:
            out = out.replace("{width}", str(width))
        if depth is not None:
            out = out.replace("{depth}", str(depth))
        return out

    return spec["program"], [fill(a) for a in _materialized_args(spec)]


def _rusage_fields(rusage):
    """(cpu_ms, peak_bytes) from one child's wait4 rusage."""
    if rusage is None:
        return None, None
    cpu_ms = (rusage.ru_utime + rusage.ru_stime) * 1000.0
    peak = rusage.ru_maxrss
    if sys.platform.startswith("linux"):
        peak = peak * 1024  # ru_maxrss is KiB on Linux
    return cpu_ms, int(peak)


def _wait4_reap(proc, deadline_s):
    """Reap one POSIX child by wait4, polling to a finite deadline:
    (exit_code, rusage) on reap, (None, None) when the deadline
    passes with the child still running. Sets proc.returncode so
    the Popen object agrees with the reap it did not perform."""
    end = time.perf_counter() + deadline_s
    while True:
        pid, status, rusage = os.wait4(proc.pid, os.WNOHANG)
        if pid == proc.pid:
            proc.returncode = os.waitstatus_to_exitcode(status)
            return proc.returncode, rusage
        if time.perf_counter() >= end:
            return None, None
        time.sleep(0.0005)


def _windows_resource_fields(proc):
    """(cpu_ms, peak_bytes, note) from a Windows arm handle after
    its wait: GetProcessTimes for CPU, GetProcessMemoryInfo for
    the peak working set. Any failure yields nulls with the
    reason named — absent-with-reason, never a fabricated zero."""
    try:
        import ctypes
        from ctypes import wintypes

        class FILETIME(ctypes.Structure):
            _fields_ = [("lo", wintypes.DWORD), ("hi", wintypes.DWORD)]

        class PMC(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD),
                        ("page_fault_count", wintypes.DWORD),
                        ("peak_working_set", ctypes.c_size_t),
                        ("working_set", ctypes.c_size_t),
                        ("quota_peak_paged", ctypes.c_size_t),
                        ("quota_paged", ctypes.c_size_t),
                        ("quota_peak_nonpaged", ctypes.c_size_t),
                        ("quota_nonpaged", ctypes.c_size_t),
                        ("pagefile", ctypes.c_size_t),
                        ("peak_pagefile", ctypes.c_size_t)]

        handle = proc._handle
        created = FILETIME()
        exited = FILETIME()
        kernel = FILETIME()
        user = FILETIME()
        ok = ctypes.windll.kernel32.GetProcessTimes(
            handle, ctypes.byref(created), ctypes.byref(exited),
            ctypes.byref(kernel), ctypes.byref(user))
        if not ok:
            return None, None, "GetProcessTimes failed on this arm"
        cpu100ns = ((kernel.hi << 32) | kernel.lo) \
            + ((user.hi << 32) | user.lo)
        counters = PMC()
        counters.cb = ctypes.sizeof(PMC)
        ok = ctypes.windll.psapi.GetProcessMemoryInfo(
            handle, ctypes.byref(counters), counters.cb)
        if not ok:
            return cpu100ns / 10000.0, None, \
                "GetProcessMemoryInfo failed on this arm"
        return cpu100ns / 10000.0, int(counters.peak_working_set), None
    except Exception as error:  # ctypes surface absent or refused
        return None, None, f"resource counters unavailable: {error}"


def floor_spawn_for(config):
    """The second floor arm's minimal program: a distinct
    exit-at-once program with the same outer-lifetime shape as the
    fast child's. Falls back to the fast child itself — still a real
    process spawn, never an empty in-process measurement."""
    spec = config["children"].get("floor_spawn") or config["children"]["fast"]
    return spec["program"], _materialized_args(spec)


def creation_flags(context):
    if sys.platform != "win32":
        return 0
    flags = 0
    if context == "NO_WINDOW":
        flags |= 0x08000000  # CREATE_NO_WINDOW
    elif context == "DETACHED":
        flags |= 0x00000008 | 0x00000200  # DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    return flags


def _read_pid(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return int(handle.read().strip())
    except (OSError, ValueError):
        return None


def _checker_configured(config):
    """True when the configuration states a survivor
    checker with at least one token, judged on the
    materialized tokens (round-44 F2): a payload-carrying
    list subclass's empty list storage is not "no
    checker", here or at the probe itself."""
    checker = config.get("survivor_checker")
    if isinstance(checker, (list, tuple)):
        return bool(materialized_list(checker))
    return bool(checker)


def descendants_alive(config, descendant_pids):
    """Cleanup check for one kill slot: ask the configured probe.

    The probe is a command that receives the slot's recorded
    descendant PIDs as its final arguments and exits 0 when none of
    them survives, 1 when at least one does, and any other code when
    it cannot answer. Returns True (a descendant is alive), False
    (none is), or None (the check did not run: no probe configured,
    no identity recorded, or the probe itself failed)."""
    checker = config.get("survivor_checker")
    if not descendant_pids:
        return None
    if not isinstance(checker, (list, tuple)):
        # A checker of another shape cannot be invoked: the
        # check did not run (None, an infrastructure failure by
        # design), never a crash out of the probe invocation.
        return None
    # Round-44 F2: the checker's tokens are read ONCE,
    # through the shared materialization, and the
    # emptiness judgement is taken on the materialized
    # tokens — a raw any() validation pass over a
    # payload-carrying list subclass drained it (and its
    # empty list storage read as unconfigured), and the
    # command was then built from the empty remainder.
    tokens = materialized_list(checker)
    if not tokens:
        return None
    if any(not isinstance(token, str) for token in tokens):
        return None
    command = [sys.executable if token in ("python", "python3") else token
               for token in tokens]
    command = command + [str(pid) for pid in descendant_pids]
    try:
        done = subprocess.run(command, timeout=PROBE_DEADLINE_S,
                              capture_output=True)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if done.returncode == 0:
        return False
    if done.returncode == 1:
        return True
    return None


def run_slot(slot, config):
    """Execute one slot; return its observation record."""
    start = time.perf_counter_ns()
    record = {
        "kind": slot.kind, "arm": slot.arm, "path": slot.path,
        "context": slot.context, "rep": slot.rep,
        "variant": getattr(slot, "variant", None),
        "outcome": None, "elapsed_ms": None, "exit_code": None,
        "survivor": None, "argv": None,
        "arm_pid": None, "child_pid": None, "grandchild_pid": None,
        "descendant_pids": None,
        "tree_marker": None,
        "cpu_ms": None, "peak_working_set_bytes": None,
        "resource_note": "arm not spawned yet; no process accounted",
        "output_stdout_sha256": None, "output_stderr_sha256": None,
        "output_stdout_len": None, "output_stderr_len": None,
        "output_stdout_match": None, "output_stderr_match": None,
    }
    marker = None
    marker_dir = None
    out_path = err_path = in_path = None
    out_handle = err_handle = in_handle = None
    if slot.path == "kill":
        handle, marker = tempfile.mkstemp(prefix="ptg-kill-", suffix=".marker")
        os.close(handle)
        os.unlink(marker)  # the workload creates it by publishing its PID
        record["tree_marker"] = marker
    if slot.path == "scale":
        marker_dir = tempfile.mkdtemp(prefix="ptg-scale-")
        record["tree_marker"] = marker_dir
    try:
        if slot.arm in ("floor-direct", "floor-spawn") \
                and slot.path == "malformed":
            # Floor arms run no malformed cells in the plan; a
            # hand-built one is an infrastructure failure, not a
            # presentation.
            record["outcome"] = "infra-failure"
            record["detail"] = "floor arms present no malformed shapes"
            record["elapsed_ms"] = (time.perf_counter_ns() - start) / 1e6
            return record
        if slot.arm == "floor-direct":
            child, child_args = child_for(slot, config, marker)
            argv = [child] + child_args
        elif slot.arm == "floor-spawn":
            program, args = floor_spawn_for(config)
            argv = [program] + args
        elif slot.path == "malformed":
            argv = malformed_argv(slot, config)
        elif slot.path == "scale":
            exe = arm_executable(slot.arm, config)
            built = scale_child_for(slot, config, marker_dir)
            if built is None:
                record["outcome"] = "infra-failure"
                record["detail"] = (
                    f"no scale child configured for variant "
                    f"{slot.variant!r}; the tree was not presented")
                record["elapsed_ms"] = \
                    (time.perf_counter_ns() - start) / 1e6
                return record
            child, child_args = built
            argv = [exe] + expand_template(
                resolve_template(slot.arm, "kill", config),
                KILL_TIMEOUT_S, child, child_args)
        else:
            exe = arm_executable(slot.arm, config)
            child, child_args = child_for(slot, config, marker)
            timeout_s = FAST_TIMEOUT_S if slot.path == "fast" else KILL_TIMEOUT_S
            argv = [exe] + expand_template(
                resolve_template(slot.arm, slot.path, config),
                timeout_s, child, child_args)
        record["argv"] = argv
        if slot.path == "fast":
            # Output observation: per-slot regular files for both
            # streams, and for stdin when the child spec names its
            # stdin text (a regular file, so the child's read sees
            # end-of-file at the text's end however long any
            # process keeps the handle open).
            spec = output_spec_for(slot, config)
            fd, out_path = tempfile.mkstemp(prefix="ptg-out-",
                                            suffix=".stdout")
            out_handle = os.fdopen(fd, "wb")
            fd, err_path = tempfile.mkstemp(prefix="ptg-out-",
                                            suffix=".stderr")
            err_handle = os.fdopen(fd, "wb")
            stdin_text = spec.get("stdin")
            if isinstance(stdin_text, str):
                fd, in_path = tempfile.mkstemp(prefix="ptg-out-",
                                               suffix=".stdin")
                with os.fdopen(fd, "wb") as handle:
                    handle.write(stdin_text.encode("utf-8"))
                in_handle = open(in_path, "rb")
            popen_streams = {
                "stdout": out_handle, "stderr": err_handle,
                "stdin": in_handle if in_handle is not None
                else subprocess.DEVNULL,
            }
        else:
            popen_streams = {
                "stdout": subprocess.DEVNULL,
                "stderr": subprocess.DEVNULL,
                "stdin": subprocess.DEVNULL,
            }
        try:
            proc = subprocess.Popen(
                argv, creationflags=creation_flags(slot.context),
                **popen_streams,
            )
        except (OSError, UnicodeEncodeError) as error:
            # UnicodeEncodeError is the presentation limit of a
            # non-Unicode shape on a platform whose exec encoding
            # refuses a lone surrogate: the harness could not
            # present the shape here (on Windows it presents) —
            # an infrastructure failure naming that limit, never
            # an arm verdict.
            record["outcome"] = "infra-failure"
            record["detail"] = f"spawn failed: {error}"
            record["resource_note"] = \
                "arm never spawned; no process to account"
            record["elapsed_ms"] = (time.perf_counter_ns() - start) / 1e6
            return record
        record["arm_pid"] = proc.pid
        record["resource_note"] = None
        timed_out = False
        if sys.platform != "win32" and hasattr(os, "wait4"):
            # POSIX resource accounting: the wait4 reap that ends
            # the slot carries that child's own rusage.
            code, rusage = _wait4_reap(proc, SLOT_DEADLINE_S)
            if code is None:
                timed_out = True
                record["outcome"] = "infra-failure"
                record["detail"] = (
                    f"slot exceeded its {SLOT_DEADLINE_S:g} s "
                    "harness deadline")
                try:
                    proc.kill()
                except OSError:
                    pass
                code, rusage = _wait4_reap(proc, 5.0)
                if code is None:
                    record["detail"] += \
                        "; harness cleanup kill did not reap the arm"
            record["exit_code"] = code
            cpu_ms, peak_bytes = _rusage_fields(rusage)
            record["cpu_ms"] = cpu_ms
            record["peak_working_set_bytes"] = peak_bytes
            if rusage is None:
                record["resource_note"] = \
                    "arm not reaped; resource counters unavailable"
        else:
            try:
                record["exit_code"] = proc.wait(timeout=SLOT_DEADLINE_S)
            except subprocess.TimeoutExpired:
                timed_out = True
                record["outcome"] = "infra-failure"
                record["detail"] = (
                    f"slot exceeded its {SLOT_DEADLINE_S:g} s harness deadline")
                try:
                    proc.kill()
                    proc.wait(timeout=5.0)
                except (OSError, subprocess.TimeoutExpired):
                    record["detail"] += "; harness cleanup kill did not reap the arm"
            if sys.platform == "win32":
                cpu_ms, peak_bytes, note = _windows_resource_fields(proc)
                record["cpu_ms"] = cpu_ms
                record["peak_working_set_bytes"] = peak_bytes
                record["resource_note"] = note
            else:
                record["resource_note"] = (
                    "platform exposes no per-child resource "
                    "accounting the runner can read (no wait4)")
        if out_path is not None:
            # The arm has exited (or been killed): close the
            # capture handles and read the streams back into the
            # record, inside the slot's elapsed time.
            for handle in (out_handle, err_handle, in_handle):
                if handle is not None:
                    try:
                        handle.close()
                    except OSError:
                        pass
            out_handle = err_handle = in_handle = None
            _observe_output(record, output_spec_for(slot, config),
                            out_path, err_path)
        if slot.path == "kill":
            # Identity first, then the probe — both inside the slot's
            # elapsed time, which closes only after this check.
            record["child_pid"] = _read_pid(marker + ".child") if marker else None
            record["grandchild_pid"] = _read_pid(marker) if marker else None
            targets = [pid for pid in (record["child_pid"],
                                       record["grandchild_pid"])
                       if pid is not None]
            record["survivor"] = descendants_alive(config, targets)
            if record["survivor"] and not timed_out:
                record["outcome"] = "descendant-survived"
                record["detail"] = (
                    f"descendant pid {record['grandchild_pid']} survived "
                    "the kill")
            elif record["survivor"] and timed_out:
                # The arm exceeded its harness deadline and was
                # killed by the harness: survival is recorded in the
                # record and named here, but the slot remains the
                # infrastructure failure it already is — it must not
                # be reclassified as descendant-survived (which for a
                # competitor would read as competitor-loss evidence
                # for a kill the arm never performed).
                record["detail"] = (
                    f"{record.get('detail', '')}; descendant pid "
                    f"{record['grandchild_pid']} survived (recorded; "
                    "the slot remains an infrastructure failure "
                    "because the arm exceeded its harness deadline)")
            elif record["survivor"] is None and not timed_out:
                record["outcome"] = "infra-failure"
                if not targets:
                    record["detail"] = (
                        "kill-path identity not recorded (no marker PID); "
                        "survival unverified")
                elif not _checker_configured(config):
                    record["detail"] = (
                        "survivor probe not configured; kill-path "
                        "survival unverified")
                else:
                    record["detail"] = (
                        "survivor probe did not answer; kill-path "
                        "survival unverified")
        if slot.path == "scale":
            # The scale child's tree identity: every PID the child
            # published into the marker directory — the wide
            # child's ids file (its N children) plus its own
            # child.pid, the deep child's per-level files (levels
            # 1 through D, the in-tree convention: every level's
            # PID is a published tree PID, recorded in
            # descendant_pids, D in all). The probe judges them
            # all, exactly as on the kill path.
            child_pid = None
            descendants = []
            if marker_dir and os.path.isdir(marker_dir):
                for name in sorted(os.listdir(marker_dir)):
                    path_in_dir = os.path.join(marker_dir, name)
                    if name == "ids":
                        try:
                            with open(path_in_dir,
                                      encoding="utf-8") as handle:
                                descendants.extend(
                                    int(token) for token in
                                    handle.read().split()
                                    if token.strip())
                        except (OSError, ValueError):
                            pass
                    elif name == "child.pid" or name == "0.pid":
                        child_pid = _read_pid(path_in_dir)
                    elif name.endswith(".pid"):
                        pid = _read_pid(path_in_dir)
                        if pid is not None:
                            descendants.append(pid)
            record["child_pid"] = child_pid
            record["descendant_pids"] = sorted(set(descendants))
            record["grandchild_pid"] = (
                record["descendant_pids"][0]
                if record["descendant_pids"] else None)
            targets = [pid for pid in
                       [child_pid] + record["descendant_pids"]
                       if pid is not None]
            record["survivor"] = descendants_alive(config, targets)
            if record["survivor"] and not timed_out:
                record["outcome"] = "descendant-survived"
                record["detail"] = (
                    f"descendant pid {record['grandchild_pid']} "
                    f"of the {slot.variant} tree survived the kill")
            elif record["survivor"] and timed_out:
                record["detail"] = (
                    f"{record.get('detail', '')}; descendant pid "
                    f"{record['grandchild_pid']} survived (recorded; "
                    "the slot remains an infrastructure failure "
                    "because the arm exceeded its harness deadline)")
            elif record["survivor"] is None and not timed_out:
                record["outcome"] = "infra-failure"
                if not targets:
                    record["detail"] = (
                        "scale-path identity not recorded (no "
                        "published PID); survival unverified")
                elif not _checker_configured(config):
                    record["detail"] = (
                        "survivor probe not configured; scale-path "
                        "survival unverified")
                else:
                    record["detail"] = (
                        "survivor probe did not answer; scale-path "
                        "survival unverified")
            if record["survivor"] is False \
                    and record["outcome"] is None:
                # The published count is held to the variant
                # (round-27 F1): the arm killed the scale child
                # before it published the whole tree it was
                # building — at zero published PIDs or at any
                # count below the variant's, the named tree was
                # never presented in full, so its teardown is
                # unverified — the kill path's rule (no recorded
                # identity: infrastructure failure, never "ok")
                # applied to the scale path at every short count.
                import plan as plan_mod
                expected_count = plan_mod.expected_descendants(
                    slot.variant)
                # THE scale count rule (round-27 F1; round-28 F2;
                # round-29 F4; round-30 F1: ONE PREDICATE IN BOTH
                # PLACES) — plan.scale_count_matches, the same
                # function the report checker's count hold calls,
                # so this classification and the checker agree by
                # construction, by the same numbers: the published
                # list's LENGTH equals the variant's raw published
                # count AND its distinct-after-exclusion
                # cardinality equals the variant count. Per
                # variant: wide's ids file carries exactly the N
                # descendants (raw length N), the roots arm_pid
                # and child_pid (recorded separately above from
                # the wide child's child.pid file) excluded from
                # the distinct count; deep's per-level files carry
                # exactly the D level PIDs, level 1 included (raw
                # length D), arm_pid alone excluded and a root
                # also named in child_pid neither adding nor
                # subtracting. A PID repeated to the length, a
                # root offered as a descendant, or an excluded
                # root published BESIDE the full tree (length
                # owed+1) presented a tree other than the named
                # tree — never "ok" here, and named by the
                # checker on the record written here.
                if expected_count is not None \
                        and not plan_mod.scale_count_matches(
                            slot.variant,
                            record["descendant_pids"],
                            record.get("arm_pid"), child_pid):
                    published = plan_mod.scale_distinct_count(
                        slot.variant, record["descendant_pids"],
                        record.get("arm_pid"), child_pid)
                    record["outcome"] = "infra-failure"
                    record["detail"] = (
                        f"scale-path tree not presented in full: "
                        f"variant {slot.variant} publishes "
                        f"{expected_count} PID(s), "
                        f"{expected_count} of them distinct "
                        f"descendant PID(s) under the variant-aware "
                        f"exclusion, the record carries "
                        f"{len(record['descendant_pids'])} "
                        f"published PID(s), {published} of them "
                        f"distinct — survival unverified for the "
                        f"unpublished remainder")
        if record["outcome"] is None:
            expected = expected_exit(
                slot.arm, slot.path, config,
                variant=getattr(slot, "variant", None))
            if expected is not None and record["exit_code"] != expected:
                record["outcome"] = "exit-mismatch"
                record["detail"] = (
                    f"exit code {record['exit_code']} != expected "
                    f"{expected} for {slot.arm}/{slot.path}")
            elif slot.path == "fast" and (
                    record["output_stdout_match"] is False
                    or record["output_stderr_match"] is False):
                record["outcome"] = "output-mismatch"
                streams = [name for name, key in (
                    ("stdout", "output_stdout_match"),
                    ("stderr", "output_stderr_match"))
                    if record[key] is False]
                record["detail"] = (
                    f"{' and '.join(streams)} output did not match "
                    f"the expected bytes for {slot.arm}/fast "
                    f"(emitted stdout sha256 "
                    f"{record['output_stdout_sha256']}, "
                    f"{record['output_stdout_len']} byte(s); "
                    f"emitted stderr sha256 "
                    f"{record['output_stderr_sha256']}, "
                    f"{record['output_stderr_len']} byte(s))")
            else:
                record["outcome"] = "ok"
        record["elapsed_ms"] = (time.perf_counter_ns() - start) / 1e6
        return record
    finally:
        for handle in (out_handle, err_handle, in_handle):
            if handle is not None:
                try:
                    handle.close()
                except OSError:
                    pass
        for path in (out_path, err_path, in_path):
            if path:
                try:
                    os.unlink(path)
                except OSError:
                    pass
        if marker:
            for path in (marker, marker + ".child", marker + ".tmp",
                         marker + ".child.tmp"):
                try:
                    os.unlink(path)
                except OSError:
                    pass
        if marker_dir:
            shutil.rmtree(marker_dir, ignore_errors=True)
