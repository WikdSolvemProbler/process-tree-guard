# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Native observation and independent telemetry replay for the laptop run."""

from contextlib import ExitStack
import ctypes as C
from ctypes import wintypes as W
from dataclasses import asdict, replace
from functools import lru_cache
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
from statistics import median

from laptop_barrier import (
    BARRIER_NS, EMPTY_JOB, GROUP_NS, HEX, NS, RESERVE_NS, BarrierError, BarrierServer, append_row,
    PRICING_COOLDOWN_NS, PRICING_EVIDENCE_RESERVE_NS, barrier_occupation_ns, checked,
    no_duplicates, pricing_witness_outcome, process_in_job, require, require_admission, wait_ms,
)


# DETACHED_PROCESS keeps the long-lived instrumentation worker console-free.
INSTRUMENTATION_WORKER_FLAGS = 0x00000008
PERIOD = 5 * NS
OUTSIDE_WINDOWS = 12
TRANSITION_NS = 50_000_000
CPU_RECONCILIATION_NS = 50_000_000
GIB = 1024 ** 3
ENVIRONMENT_LABELS = {"physical_memory", "power_state"}


class MemoryStatus(C.Structure):
    _fields_ = [("length", W.DWORD), ("load", W.DWORD)] + [
        (name, C.c_uint64) for name in (
            "physical_total", "physical_available", "page_total", "page_available",
            "virtual_total", "virtual_available", "extended_available",
        )
    ]


class PowerStatus(C.Structure):
    _fields_ = [(name, W.BYTE) for name in (
        "ac_line", "battery_flag", "battery_percent", "saver",
    )] + [(name, W.DWORD) for name in ("battery_life", "battery_full_life")]


def filetime(value):
    return value.dwHighDateTime << 32 | value.dwLowDateTime


@lru_cache(maxsize=1)
def _api():
    from windows_slot_job import _bind

    p = C.POINTER
    specifications = {
        "GlobalMemoryStatusEx": ([p(MemoryStatus)], W.BOOL),
        "GetSystemPowerStatus": ([p(PowerStatus)], W.BOOL),
        "GetDiskFreeSpaceExW": ([W.LPCWSTR, p(C.c_uint64), p(C.c_uint64), p(C.c_uint64)], W.BOOL),
        "GetSystemTimes": ([p(W.FILETIME)] * 3, W.BOOL),
        "GetActiveProcessorGroupCount": ([], W.WORD),
        "GetActiveProcessorCount": ([W.WORD], W.DWORD),
        "OpenThread": ([W.DWORD, W.BOOL, W.DWORD], W.HANDLE),
        "GetThreadTimes": ([W.HANDLE, *([p(W.FILETIME)] * 4)], W.BOOL),
        "CloseHandle": ([W.HANDLE], W.BOOL),
    }
    calls = {name: _bind(name, args, result)
             for name, (args, result) in specifications.items()}
    realtime = C.WinDLL("api-ms-win-core-realtime-l1-1-1")
    for name in ("QueryUnbiasedInterruptTimePrecise", "QueryInterruptTimePrecise"):
        clock = getattr(realtime, name)
        clock.argtypes = [p(C.c_uint64)]
        clock.restype = None
        calls[name] = clock
    return calls


def resources(output):
    api = _api()
    memory = MemoryStatus()
    memory.length = C.sizeof(memory)
    power = PowerStatus()
    disk = C.c_uint64()
    checked(api["GlobalMemoryStatusEx"](C.byref(memory)))
    checked(api["GetSystemPowerStatus"](C.byref(power)))
    checked(api["GetDiskFreeSpaceExW"](str(output), C.byref(disk), None, None))
    return {
        **{name: getattr(memory, name) for name, _ in memory._fields_},
        **{name: getattr(power, name) for name, _ in power._fields_},
        "disk_available": disk.value,
    }


def resource_stop(raw):
    require(type(raw) is dict and set(raw) == {"disk_available"} | {
        name for name, _ in MemoryStatus._fields_ + PowerStatus._fields_
    } and all(type(value) is int and 0 <= value < 2 ** 64 for value in raw.values()),
        "invalid native resource shape")
    ac, flag, percent, saver = (raw[name] for name in (
        "ac_line", "battery_flag", "battery_percent", "saver",
    ))
    require(ac in (0, 1, 255) and saver in (0, 1)
            and (percent <= 100 or percent == 255)
            and (flag == 255 or not flag & ~0x8F)
            and raw["battery_life"] < 2 ** 32 and raw["battery_full_life"] < 2 ** 32,
            "invalid native power encoding")
    require(raw["length"] == 64
            and raw["load"] <= 100
            and raw["physical_available"] <= raw["physical_total"]
            and raw["page_available"] <= raw["page_total"]
            and raw["virtual_available"] <= raw["virtual_total"],
            "invalid native memory readings")
    failures = []
    if raw["physical_available"] < GIB:
        failures.append("emergency_physical_memory")
    if raw["page_available"] < GIB:
        failures.append("emergency_commit")
    if flag != 255 and flag & 4:
        failures.append("critical_battery")
    if ac == 0 and percent <= 10:
        failures.append("battery_at_or_below_10_percent")
    if raw["disk_available"] < 2 * GIB:
        failures.append("emergency_output_volume_reserve")
    return failures


def captured_resource_errors(raw):
    try:
        return resource_stop(raw)
    except BarrierError as error:
        return [str(error)]


class InvasiveThreadClock:
    def __init__(self, native_id):
        self.api = _api()
        self.handle = checked(self.api["OpenThread"](0x0800, False, native_id))

    def read(self):
        times = [W.FILETIME() for _ in range(4)]
        checked(self.api["GetThreadTimes"](self.handle, *(C.byref(value) for value in times)))
        return 100 * (filetime(times[2]) + filetime(times[3]))

    def close(self):
        checked(self.api["CloseHandle"](self.handle))


def job_cpu(job):
    from quiet_gate import Job
    from windows_slot_job import (
        BasicAccountingInformation, JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION,
        QueryInformationJobObject,
    )

    value = BasicAccountingInformation()
    checked(QueryInformationJobObject(
        job._handle, JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION, C.byref(value),
        C.sizeof(value), None,
    ))
    return Job(value.total_kernel_time, value.total_user_time,
               value.total_processes, value.active_processes)


def sample_native(job, raw_resources):
    """Preserve the 017 inner native-read bracket and the underlying API order."""
    from quiet_gate import Cpu, ReadBounds, Sample

    api = _api()
    groups = api["GetActiveProcessorGroupCount"]()
    processors = api["GetActiveProcessorCount"](0)
    awake, interrupt, inner_left, inner_right = (C.c_uint64() for _ in range(4))
    idle, kernel, user = (W.FILETIME() for _ in range(3))
    errors, bounds = [], []

    def measured(name, operation, *args):
        begin = time.perf_counter_ns()
        try:
            return operation(*args)
        except OSError as error:
            errors.append(f"{name}:{type(error).__name__}:{error}")
            return None
        finally:
            bounds.extend((begin, time.perf_counter_ns()))

    awake_query = api["QueryUnbiasedInterruptTimePrecise"]
    begin = time.perf_counter_ns()
    awake_query(C.byref(awake))
    api["QueryInterruptTimePrecise"](C.byref(interrupt))
    before = measured("job_before", job_cpu, job) if job is not None else None
    if job is None:
        bounds.extend((0, 0))
    measured("awake_before", awake_query, C.byref(inner_left))
    cpu = measured("system", lambda: checked(api["GetSystemTimes"](
        C.byref(idle), C.byref(kernel), C.byref(user),
    )))
    measured("awake_after", awake_query, C.byref(inner_right))
    after = measured("job_after", job_cpu, job) if job is not None else None
    if job is None:
        bounds.extend((0, 0))
    end = time.perf_counter_ns()
    times = Cpu(filetime(idle), filetime(kernel), filetime(user)) if cpu else Cpu(0, 0, 0)
    return Sample(begin, awake.value, interrupt.value, time.process_time_ns(), end - begin,
                  groups, processors, raw_resources["physical_available"],
                  raw_resources["ac_line"], raw_resources["saver"], 2,
                  times, inner_left.value, inner_right.value, before, after,
                  ReadBounds(*bounds), tuple(errors))


def decode_sample(raw):
    from quiet_gate import Cpu, Job, ReadBounds, Sample

    fields = dict(raw)
    fields["system"] = Cpu(**fields["system"])
    fields["calls"] = ReadBounds(**fields["calls"])
    for key in ("job_before", "job_after"):
        if fields[key] is not None:
            fields[key] = Job(**fields[key])
    require(type(fields["native_errors"]) in (list, tuple), "invalid native error list")
    fields["native_errors"] = tuple(fields["native_errors"])
    return Sample(**fields)


def score_window(first, last, *, during, mode="complete"):
    """Re-evaluate the old counter arithmetic with only the approved gate split."""
    from quiet_gate import DURING_MAX_CPU, DURING_PAIRED_CPU, MEDIAN_CPU, OUTSIDE_MAX_CPU, window

    left, right = (decode_sample(item["raw"]) for item in (first, last))
    threads = [
        last[key] - first[key] for key in ("sampler_cpu_ns", "invasive_cpu_ns")
    ]
    process_cpu = right.observer_ns - left.observer_ns
    score = window(replace(left, observer_ns=first["sampler_cpu_ns"]),
                   replace(right, observer_ns=last["sampler_cpu_ns"]), during=during)
    errors = score["errors"]
    labels = [entry for entry in errors if entry in ENVIRONMENT_LABELS]
    if mode == "partial":
        labels_for_mode = {"sample_gap"}
    elif mode == "short_tail":
        labels_for_mode = {"sample_gap", "system_counter_capacity"}
    else:
        labels_for_mode = set()
    score["errors"] = [entry for entry in errors
                       if entry not in ENVIRONMENT_LABELS | labels_for_mode]
    if min(process_cpu, *threads) < 0:
        score["errors"].append("observer_counter_order")
    reconcile = process_cpu - sum(threads)
    if mode == "complete" and abs(reconcile) > CPU_RECONCILIATION_NS:
        score["errors"].append("observer_thread_accounting")
    score["observer_process_ns"] = process_cpu
    score["invasive_cpu_ns"] = threads[1]
    score["observer_reconciliation_ns"] = reconcile
    value = score["residual_cpu_upper_pct"]
    if value is not None:
        if value > MEDIAN_CPU:
            labels.append("above_017_median_threshold")
        if value > (DURING_MAX_CPU if during else OUTSIDE_MAX_CPU):
            labels.append("above_017_maximum_threshold")
        if during and value > DURING_PAIRED_CPU:
            labels.append("above_017_adjacent_window_threshold")
    score["condition_labels"] = labels
    return score


def window_rows(row, previous, last_start):
    if previous is None:
        return []
    during = row["phase"] == "during"
    elapsed = row["raw"]["monotonic_ns"] - previous["raw"]["monotonic_ns"]
    if not during or not row["endpoint"]:
        return [{"mode": "complete", **score_window(previous, row, during=during)}]
    if NS <= elapsed < 4.5 * NS:
        return [{"mode": "partial", **score_window(previous, row, during=True, mode="partial")}]
    if elapsed < NS and last_start is not None:
        return [
            {"mode": "merged", **score_window(last_start, row, during=True, mode="merged")},
            {"mode": "short_tail", **score_window(previous, row, during=True, mode="short_tail")},
        ]
    return [{"mode": "terminal", **score_window(previous, row, during=True, mode="terminal")}]


class Observer:
    def __init__(self, output, records, inventories, ancestry, image_policy):
        self.output = output
        self.records, self.inventories = records, inventories
        self.ancestry = ancestry
        self.guard, self.image_policy = ancestry.primary_guard(), image_policy
        self.last_trace = None
        self.sequence = 0

    def emit(self, kind, **fields):
        return append_row(self.records, {"kind": kind, **fields})

    def sample(self, phase, job, clocks, *, previous=None, last_start=None, endpoint=False):
        from quiet_gate import sample_errors

        outer = time.perf_counter_ns()
        raw_resources = None
        try:
            raw_resources = resources(self.output)
            reading = sample_native(job, raw_resources)
        except OSError as error:
            self.emit("sample_error", phase=phase, outer_begin_ns=outer,
                      resources=raw_resources, error=str(error))
            raise
        row = {
            "index": self.sequence, "phase": phase, "endpoint": endpoint,
            "outer_begin_ns": outer, "utc_ns": time.time_ns(),
            "previous_outer_trace": self.last_trace, "raw": asdict(reading),
            "sampler_cpu_ns": time.thread_time_ns(), "invasive_cpu_ns": clocks.read(),
            "resources": raw_resources,
        }
        row["window_rows"] = window_rows(row, previous, last_start)
        row["errors"] = (sample_errors(reading, during=job is not None)
                         + captured_resource_errors(raw_resources))
        self.emit("sample", **row)
        self.last_trace = {"index": self.sequence, "outer_end_ns": time.perf_counter_ns()}
        self.sequence += 1
        require(not row["errors"], f"{phase} sample rejected: {row['errors']}")
        for score in row["window_rows"]:
            require(not score["errors"] and score["residual_cpu_upper_pct"] is not None,
                    f"{phase} window rejected: {score['errors']}")
        return row

    def census(self, phase, index, job, requested_ns, identity=None, label=None):
        from image_policy import ALLOWED
        from inventory_gate import inventory
        from laptop_ancestry import census_admissions, census_allowed

        begin = time.perf_counter_ns()
        bad = captured_resource_errors(raw_resources := resources(self.output))
        rows, before, after, admissions = [], [], [], []
        if not bad:
            try:
                before = self.ancestry.verify()
                rows = inventory(self.guard, None if job is None else job._handle, self.image_policy)
                after = self.ancestry.verify()
                admissions = census_admissions(rows, self.ancestry.proof, before, after)
            except (OSError, ValueError) as error:
                bad.append(f"ancestry or census proof: {type(error).__name__}: {error}")
        finished = time.perf_counter_ns()
        row = {"phase": phase, "index": index, "requested_ns": requested_ns,
               "begin_ns": begin, "finished_ns": finished, "utc_ns": time.time_ns(),
               "resources": raw_resources, "guard_pid": self.guard[0],
               "guard_created_ticks": self.guard[2], "identity": identity, "label": label,
               "rows": rows, "above_017_span_threshold": finished - begin > TRANSITION_NS,
               "held_guards_before": before, "held_guards_after": after,
               "ancestry_admissions": admissions, "errors": bad}
        digest = append_row(self.inventories, row)
        end = time.perf_counter_ns()
        require(not bad and census_allowed(rows, admissions, ALLOWED),
                f"{phase} census rejected: {bad or 'unverified or competing workload'}")
        require(requested_ns <= begin <= finished <= end <= requested_ns + BARRIER_NS,
                f"{phase} full census exceeded one-second bound")
        return {"begin_ns": begin, "end_ns": end, "sha256": digest}

    def trace_end(self):
        if self.last_trace is not None:
            self.emit("outer_trace_end", **self.last_trace)


def at(target):
    remaining = target - time.perf_counter_ns()
    if remaining > 0:
        time.sleep(remaining / NS)


def counter_time(row):
    return row["raw"]["monotonic_ns"]


def within_transition(left, right):
    require(0 <= right - left <= TRANSITION_NS, "non-census transition exceeded 50 ms")


def outside(observer, server, clocks, phase, after=None):
    def census(index, deadline):
        requested = time.perf_counter_ns()
        receipt = []
        done = server.submit(lambda: receipt.append(
            observer.census(phase, index, None, requested),
        ))
        elapsed = max(0, deadline - time.perf_counter_ns())
        require(done.wait(elapsed / NS) and time.perf_counter_ns() <= deadline,
                f"{phase} census did not settle on time")
        server.check()
        observer.emit("outside_census", phase=phase, index=index, requested_ns=requested,
                      **receipt[0])

    first = previous = observer.sample(phase, None, clocks)
    if after is not None:
        within_transition(counter_time(after), counter_time(first))
    census(0, time.perf_counter_ns() + BARRIER_NS)
    for index in range(1, OUTSIDE_WINDOWS + 1):
        target = counter_time(first) + PERIOD * index
        if phase == "pre" and index == OUTSIDE_WINDOWS:
            at(target - BARRIER_NS)
            census(index, target)
        at(target)
        previous = observer.sample(phase, None, clocks, previous=previous)
        if phase == "post" or index < OUTSIDE_WINDOWS:
            census(index, time.perf_counter_ns() + BARRIER_NS)
    observer.emit("phase_decision", phase=phase, complete_windows=OUTSIDE_WINDOWS)
    return previous


def wait_for_worker(server, worker, target):
    win = server.win
    status = win.WaitForMultipleObjects([worker._handle, server.failed_handle], False, wait_ms(target))
    if status == win.WAIT_OBJECT_0:
        return win.GetExitCodeProcess(worker._handle)
    if status == win.WAIT_OBJECT_0 + 1:
        server.check()
        raise RuntimeError("invasive failure event had no cause")
    require(status == win.WAIT_TIMEOUT, f"unexpected worker wait status {status}")
    return None


def held_worker_launch(job, worker, argv, python_ref):
    from inventory_gate import identity, image_digest
    from laptop_schedule import canonical_bytes

    original = process_in_job(worker._handle, job._handle)
    image, created = identity(worker._handle)
    expected = Path(python_ref["path"])
    require(original == {"pid": worker.pid, "creation_filetime": created}
            and argv[0] == str(expected) and image.samefile(expected),
            "original worker process does not match the frozen executable")
    digest = image_digest(image)
    require(digest == python_ref["sha256"], "original worker executable bytes differ")
    return {**original, "image_path": str(image), "image_sha256": digest, "job_member": True,
            "worker_recipe_sha256": hashlib.sha256(canonical_bytes(list(argv))).hexdigest()}


def during(observer, server, clocks, job, handles, worker_argv, manifest, previous, allowance):
    first = previous_during = observer.sample("during", job, clocks)
    within_transition(counter_time(previous), counter_time(first))
    for field in ("job_before", "job_after"):
        require(first["raw"][field] == {
            "kernel": 0, "user": 0, "total_processes": 0, "active_processes": 0,
        }, "campaign Job must begin empty")
    start = counter_time(first)
    deadline = start + allowance
    dispatch = time.perf_counter_ns()
    within_transition(start, dispatch)
    worker = job.launch(worker_argv, INSTRUMENTATION_WORKER_FLAGS,
                        stdin=subprocess.DEVNULL,
                        stdout=handles[0], stderr=handles[1], cwd=manifest["working_directory"])
    handles.append(worker)
    launched = time.perf_counter_ns()
    require(dispatch <= launched < start + PERIOD, "launch outside first counter window")
    receipt = held_worker_launch(job, worker, worker_argv, manifest["files"]["python"])
    observer.emit("worker_dispatched", **receipt, dispatch_ns=dispatch,
                  creation_flags=INSTRUMENTATION_WORKER_FLAGS,
                  launched_ns=launched, deadline_ns=deadline)
    served = server.bind_worker(job, worker, dispatch, deadline)
    next_sample = start + PERIOD
    n_complete = 0
    last_start = None
    while True:
        target = min(next_sample, deadline)
        code = wait_for_worker(server, worker, target)
        if code is None:
            at(target)
        require(time.perf_counter_ns() < deadline, "during deadline exceeded")
        last = observer.sample("during", job, clocks, previous=previous_during,
                               last_start=last_start, endpoint=code is not None)
        require(time.perf_counter_ns() < deadline, "during deadline exceeded while sampling")
        if code is None:
            n_complete += 1
            last_start, previous_during = previous_during, last
            next_sample += PERIOD
            continue
        require(served.wait(wait_ms(deadline) / 1000), "invasive checkpoints not settled")
        server.check()
        require(len(server.authorizations) == len(server.groups), "incomplete during census population")
        require(code == 0 and n_complete >= OUTSIDE_WINDOWS,
                f"incomplete during phase: worker={code}, complete_windows={n_complete}")
        require(all(last["raw"][key]["active_processes"] == 0 for key in ("job_before", "job_after")),
                "worker exit left live campaign Job members")
        drain = job.drain()
        observer.emit("drain", **drain)
        require(drain == EMPTY_JOB, "campaign Job did not drain naturally")
        require(time.perf_counter_ns() < deadline, "during deadline exceeded while draining")
        price_occupied = barrier_occupation_ns(server.pricing_authorizations, server.pricing_ack_settled_ns)
        occupied = barrier_occupation_ns(server.authorizations, server.ack_settled_ns) + price_occupied
        require(10 * occupied <= counter_time(last) - start, "invasive barrier occupation >10%")
        observer.emit("phase_decision", phase="during", complete_windows=n_complete,
                      code=code, occupied_ns=occupied, ack_settled_ns=server.ack_settled_ns,
                      pricing_ack_settled_ns=server.pricing_ack_settled_ns)
        return last


def capture(manifest, capture_dir, worker_argv, *, pipe_name, nonce, groups,
            manifest_sha, guard_ancestry, max_during_seconds=6600):
    """Capture one pre-approved worker invocation. It has no retry path."""
    from windows_slot_job import SlotJob

    require(type(max_during_seconds) is int and 0 < max_during_seconds <= 6600,
            "invalid during cap")
    output = Path(capture_dir)
    identity_policy = manifest["image_policy"]
    result = {"accepted": False, "manifest_sha256": manifest_sha, "problems": []}
    with ExitStack() as stack:
        records = stack.enter_context((output / "telemetry.jsonl").open("xb"))
        inventory_file = stack.enter_context((output / "inventory.jsonl").open("xb"))
        stdout = stack.enter_context((output / "stdout.log").open("xb"))
        stderr = stack.enter_context((output / "stderr.log").open("xb"))
        handles = [stdout, stderr]
        observer = job = None
        try:
            guard = guard_ancestry.primary_guard()
            observer = Observer(output, records, inventory_file, guard_ancestry, identity_policy)
            observer.emit("started", manifest_sha256=manifest_sha, observer_pid=os.getpid(),
                          guard_ancestry=guard_ancestry.proof, worker_argv=list(worker_argv),
                          max_during_seconds=max_during_seconds, barrier_nonce=nonce, guard_pid=guard[0],
                          guard_created_ticks=guard[2])
            with BarrierServer(
                pipe_name, nonce, manifest_sha, groups, output / "barrier.jsonl",
                lambda campaign, identity, ordinal, label, asked: observer.census(
                    "during", ordinal, campaign, asked, identity=identity, label=label,
                ),
            ) as server:
                clocks = InvasiveThreadClock(server.thread.native_id)
                try:
                    # Warm the Job query before the first recorded clock anchor.
                    job = SlotJob()
                    job_cpu(job)
                    previous = outside(observer, server, clocks, "pre")
                    endpoint = during(observer, server, clocks, job, handles, worker_argv,
                                      manifest, previous, max_during_seconds * NS)
                    outside(observer, server, clocks, "post", after=endpoint)
                    server.check()
                finally:
                    clocks.close()
            result["accepted"] = True
        except Exception as error:
            result["problems"].append(f"{type(error).__name__}: {error}")
            if job is not None:
                try:
                    append_row(records, {"kind": "abort_drain", **job.drain()})
                except Exception as cleanup:
                    result["problems"].append(f"abort drain: {cleanup}")
        finally:
            for owned in [*handles[2:], job]:
                if owned is not None:
                    try:
                        owned.close()
                    except Exception as cleanup:
                        result["accepted"] = False
                        result["problems"].append(f"handle close: {cleanup}")
            if observer is not None:
                observer.trace_end()
            append_row(records, {"kind": "terminal", **result})
    return result



def load_rows(path):
    raw = path.read_bytes()
    require(raw and raw.endswith(b"\n"), f"incomplete receipt file: {path.name}")
    rows = []
    for line in raw.splitlines(keepends=True):
        parsed = json.loads(line, object_pairs_hook=no_duplicates)
        require(type(parsed) is dict, f"invalid receipt in {path.name}")
        rows.append((parsed, hashlib.sha256(line).hexdigest()))
    return rows


def check_pricing_membership(terminal, attempts, worker_identity, manifest_sha, nonce):
    context = {"nonce": nonce, "manifest_sha256": manifest_sha, "group": terminal["group"],
               "index": terminal["index"], "requested_ns": terminal["requested_ns"]}
    require(set(terminal) == set(context) | {
        "kind", "admitted_ns", "deadline_ns", "decision_ns", "authorized_ns",
        "outcome", "witness", "attempt_sha256s",
    } and all(terminal[key] == value for key, value in context.items()),
            "pricing terminal receipt does not bind its exact request")
    asked, admitted, deadline, decision = (terminal[key] for key in (
        "requested_ns", "admitted_ns", "deadline_ns", "decision_ns",
    ))
    require(all(type(value) is int and value >= 0 for value in (
        context["group"], context["index"], asked, admitted, deadline, decision,
    )) and asked <= admitted <= deadline <= asked + BARRIER_NS and decision >= admitted,
            "invalid pricing request or absolute deadline")
    require(terminal["attempt_sha256s"] == [digest for _, digest in attempts],
            "pricing attempt digest history differs")
    cutoff = deadline - PRICING_EVIDENCE_RESERVE_NS
    earlier_end = admitted
    last = None
    previous_result = None
    for ordinal, (attempt, _) in enumerate(attempts):
        require(set(attempt) == set(context) | {
            "kind", "ordinal", "query_start_ns", "query_end_ns", "outcome", "witness",
        } and all(attempt[key] == value and type(attempt[key]) is type(value)
                  for key, value in context.items())
                and type(attempt["ordinal"]) is int and attempt["ordinal"] == ordinal,
                "pricing attempt does not bind its request or sequence")
        start, end = attempt["query_start_ns"], attempt["query_end_ns"]
        require(type(start) is int and type(end) is int
                and earlier_end + (PRICING_COOLDOWN_NS if ordinal else 0) <= start <= cutoff
                and start <= end <= decision, "pricing attempt violated its cooldown or deadline")
        require(ordinal == 0 or previous_result == "extra-members",
                "pricing retried after an authoritative terminal observation")
        last = attempt["witness"]
        previous_result = pricing_witness_outcome(last, worker_identity)
        require(attempt["outcome"] == previous_result, "pricing attempt outcome differs from native witness")
        earlier_end = end
    require(terminal["witness"] == last, "pricing final witness differs from the last raw query")
    status = terminal["outcome"]
    if status == "sole-worker":
        require(previous_result == status and type(terminal["authorized_ns"]) is int
                and terminal["authorized_ns"] == decision <= deadline,
                "pricing ACK lacks a deadline-bounded exact sole-worker witness")
    else:
        require(terminal["authorized_ns"] is None, "refused pricing witness carries an authorization")
        if status in ("native-error", "invalid-native-list"):
            require(previous_result == status, "pricing terminal error differs from the last native result")
        elif status == "persistent-extra":
            require(previous_result == "extra-members" and decision <= deadline
                    and (earlier_end + PRICING_COOLDOWN_NS > cutoff or decision > cutoff),
                    "pricing refused extra members before exhausting the original query bound")
        elif status == "deadline":
            require(previous_result in ("sole-worker", "extra-members") and decision > deadline,
                    "pricing deadline refusal lacks the native deadline crossing")
        elif status == "stopped":
            require(previous_result in (None, "extra-members", "sole-worker"),
                    "pricing stop differs from its last native result")
        else:
            require(status == "insufficient-budget" and previous_result is None and decision > cutoff,
                    "unknown pricing refusal or unexhausted first-query budget")


def check_pricing_receiver(member, settlement):
    started, settled = settlement["timing_end_ns"], settlement["settled_ns"]
    received = settlement.get("pricing_ack_received_ns")
    deadline = settlement.get("pricing_ack_deadline_ns")
    require(type(received) is int and type(deadline) is int and deadline == member["deadline_ns"]
            and started <= member["requested_ns"] <= member["authorized_ns"] <= received
            <= min(deadline, settled) and settled <= member["requested_ns"] + BARRIER_NS,
            "pricing receiver ACK crossed the exact effective deadline or stock return")


def read_pricing_membership(path, worker_identity, manifest_sha, nonce, *, accepted=True):
    """Replay the complete, durable history behind each raw settlement digest."""
    require(type(worker_identity) is dict and set(worker_identity) == {"pid", "creation_filetime"}
            and all(type(value) is int and value > 0 for value in worker_identity.values()),
            "invalid exact worker identity for pricing replay")
    groups = []
    attempts = []
    for row, digest in load_rows(path):
        if row.get("kind") == "pricing-membership-attempt":
            attempts.append((row, digest))
            continue
        require(row.get("kind") == "pricing-settlement", "unbound row in pricing membership evidence")
        check_pricing_membership(row, attempts, worker_identity, manifest_sha, nonce)
        require(not accepted or row["outcome"] == "sole-worker", "pricing membership did not settle")
        groups.append((row, digest, [attempt for attempt, _ in attempts]))
        attempts = []
    require(groups and not attempts, "pricing membership contains an unterminated proof history")
    return groups


def validate_sample(row, previous, last_start):
    from quiet_gate import sample_errors

    sample = decode_sample(row["raw"])
    raw = row["resources"]
    require(row["errors"] == sample_errors(sample, during=row["phase"] == "during")
            + resource_stop(raw) == [], "native sample failure or mismatched classification")
    require((sample.available_bytes, sample.ac_line, sample.saver)
            == (raw["physical_available"], raw["ac_line"], raw["saver"]),
            "native sample differs from auxiliary resource readings")
    require(row["window_rows"] == window_rows(row, previous, last_start),
            "window arithmetic differs from raw readings")
    require(all(not score["errors"] and score["residual_cpu_upper_pct"] is not None
                for score in row["window_rows"]), "hard window failure")
    require(type(row["sampler_cpu_ns"]) is int and row["sampler_cpu_ns"] >= 0
            and type(row["invasive_cpu_ns"]) is int and row["invasive_cpu_ns"] >= 0
            and row["outer_begin_ns"] <= sample.monotonic_ns, "invalid observer sample clocks")


def telemetry_rows(capture_dir, manifest_sha):
    rows = [row for row, _ in load_rows(capture_dir / "telemetry.jsonl")]
    require(rows[0]["kind"] == "started" and rows[0]["manifest_sha256"] == manifest_sha
            and rows[-1]["kind"] == "terminal" and rows[-1]["manifest_sha256"] == manifest_sha
            and rows[-1]["accepted"] is True and rows[-1]["problems"] == [],
            "telemetry receipt does not prove a complete bound attempt")
    phases = {name: [] for name in ("pre", "during", "post")}
    decisions, dispatches, drains, outside_censuses = {}, [], [], {}
    spans, last = [], None
    active_phase = "pre"
    trace_closed = False
    for row in rows[1:-1]:
        kind = row["kind"]
        require(not trace_closed, "telemetry after the final observer trace")
        if kind == "sample":
            name = row["phase"]
            require(name == active_phase, "sample phase is out of order")
            if last is None:
                require(row["index"] == 0 and row["previous_outer_trace"] is None,
                        "invalid first trace anchor")
            else:
                trace = row["previous_outer_trace"]
                require(row["index"] == last["index"] + 1 and trace["index"] == last["index"]
                        and counter_time(last) + last["raw"]["read_ns"] <= trace["outer_end_ns"]
                        <= row["outer_begin_ns"] <= counter_time(row), "invalid observer span")
                spans.append((last["outer_begin_ns"], trace["outer_end_ns"]))
            sample_list = phases[name]
            require(not sample_list or not sample_list[-1]["endpoint"], "sample after endpoint")
            validate_sample(row, sample_list[-1] if sample_list else None,
                            sample_list[-2] if len(sample_list) >= 2 else None)
            sample_list.append(row)
            last = row
        elif kind == "outside_census":
            phase, index = row["phase"], row["index"]
            require(phase in ("pre", "post") and phase == active_phase
                    and (phase, index) not in outside_censuses
                    and row["requested_ns"] <= row["begin_ns"] <= row["end_ns"]
                    <= row["requested_ns"] + BARRIER_NS, "invalid outside census emission")
            outside_censuses[phase, index] = row
        elif kind == "phase_decision":
            name = row["phase"]
            require(name == active_phase and name not in decisions, "phase decision out of order")
            decisions[name] = row
            active_phase = {"pre": "during", "during": "post", "post": "done"}[name]
        elif kind == "worker_dispatched":
            require(active_phase == "during" and len(phases["during"]) == 1 and not dispatches,
                    "worker dispatch not bound to the during baseline")
            require(type(row["pid"]) is int and row["pid"] > 0
                    and type(row["creation_filetime"]) is int and row["creation_filetime"] > 0
                    and row["creation_flags"] == INSTRUMENTATION_WORKER_FLAGS
                    and row["job_member"] is True and isinstance(row["image_path"], str) and row["image_path"]
                    and all(isinstance(row[key], str) and HEX.fullmatch(row[key])
                            for key in ("image_sha256", "worker_recipe_sha256")),
                    "invalid held worker launch receipt")
            dispatches.append(row)
        elif kind == "drain":
            require(active_phase == "during" and phases["during"][-1]["endpoint"] and not drains
                    and {key: value for key, value in row.items() if key != "kind"} == EMPTY_JOB,
                    "invalid natural campaign Job drain")
            drains.append(row)
        elif kind == "outer_trace_end":
            require(active_phase == "done" and row["index"] == last["index"]
                    and counter_time(last) + last["raw"]["read_ns"] <= row["outer_end_ns"],
                    "invalid final observer span")
            spans.append((last["outer_begin_ns"], row["outer_end_ns"]))
            trace_closed = True
        else:
            raise ValueError(f"unexpected complete telemetry receipt {kind}")
    require(trace_closed and len(dispatches) == len(drains) == 1
            and list(decisions) == list(phases), "incomplete telemetry evidence")
    for phase in ("pre", "post"):
        require(len(phases[phase]) == OUTSIDE_WINDOWS + 1
                and decisions[phase]["complete_windows"] == OUTSIDE_WINDOWS
                and not any(row["endpoint"] for row in phases[phase]), "outside sampling coverage")
    during_rows = phases["during"]
    require(len(during_rows) >= OUTSIDE_WINDOWS + 2 and during_rows[-1]["endpoint"]
            and decisions["during"]["complete_windows"] == len(during_rows) - 2
            and decisions["during"]["code"] == 0, "during sampling coverage")
    for field in ("job_before", "job_after"):
        require(during_rows[0]["raw"][field] == {
            "kernel": 0, "user": 0, "total_processes": 0, "active_processes": 0,
        } and during_rows[-1]["raw"][field]["active_processes"] == 0,
                "campaign Job endpoints lack empty-Job evidence")
    during_start, during_end = map(counter_time, (during_rows[0], during_rows[-1]))
    within_transition(counter_time(phases["pre"][-1]), during_start)
    within_transition(during_end, counter_time(phases["post"][0]))
    dispatch = dispatches[0]
    within_transition(during_start, dispatch["dispatch_ns"])
    require(dispatch["dispatch_ns"] <= dispatch["launched_ns"] < during_start + PERIOD
            and during_end < dispatch["deadline_ns"]
            == during_start + rows[0]["max_during_seconds"] * NS,
            "worker dispatch/deadline differs from raw receipt")
    return phases, spans, rows[0], decisions["during"], dispatch, outside_censuses


def inventory_rows(capture_dir, image_policy, origin, phases, groups, outside_censuses):
    from image_policy import ALLOWED, classify
    from inventory_gate import COMPILERS
    from laptop_ancestry import census_admissions, census_allowed, validate_ancestry

    ancestry = origin["guard_ancestry"]
    validate_ancestry(ancestry, image_policy, origin["observer_pid"])
    primary = ancestry["roles"][0]
    require((primary["pid"], primary["creation_filetime"])
            == (origin["guard_pid"], origin["guard_created_ticks"]),
            "census ancestry names a different primary guard")
    expected = [
        *(("pre", index) for index in range(OUTSIDE_WINDOWS + 1)),
        *(("during", group[0]) for group in groups),
        *(("post", index) for index in range(OUTSIDE_WINDOWS + 1)),
    ]
    actual = load_rows(capture_dir / "inventory.jsonl")
    require([(row["phase"], row["index"]) for row, _ in actual] == expected,
            "census population differs from the plan")
    during_census = []
    for (phase, index), (row, digest) in zip(expected, actual):
        require(row["guard_pid"] == origin["guard_pid"]
                and row["guard_created_ticks"] == origin["guard_created_ticks"]
                and row["errors"] == resource_stop(row["resources"]) == []
                and row["requested_ns"] <= row["begin_ns"] <= row["finished_ns"]
                <= row["requested_ns"] + BARRIER_NS
                and row["above_017_span_threshold"] is (
                    row["finished_ns"] - row["begin_ns"] > TRANSITION_NS),
                "invalid census native or timing fields")
        admissions = census_admissions(row["rows"], ancestry,
                                       row["held_guards_before"], row["held_guards_after"])
        require(admissions == row["ancestry_admissions"]
                and census_allowed(row["rows"], admissions, ALLOWED),
                "census advertised an unverified ancestry allowance or competing workload")
        for entry in row["rows"]:
            state = entry["state"]
            require(type(entry["pid"]) is int and entry["pid"] > 0
                    and type(entry["created_ticks"]) is int
                    and type(entry["snapshot_upper_ticks"]) is int
                    and 0 < entry["created_ticks"] <= entry["snapshot_upper_ticks"],
                    "invalid sampled identity")
            if state != "pinned_enclosing_guard":
                require(state == classify(entry["name"], entry["image_sha256"], enclosing=False,
                                          member=state == "campaign_member",
                                          compiler_names=COMPILERS,
                                          veto_sha256=set(image_policy["veto_sha256"])),
                        "sampled classification changed")
        if phase == "during":
            during_census.append((row, digest))
        else:
            require(row["identity"] is None and row["label"] is None
                    and not any(entry["state"] == "campaign_member" for entry in row["rows"]),
                    "outside census asserted campaign membership")
            anchor = counter_time(phases[phase][index])
            emission = outside_censuses.pop((phase, index))
            require(emission["sha256"] == digest
                    and row["requested_ns"] == emission["requested_ns"]
                    and row["begin_ns"] == emission["begin_ns"]
                    and row["finished_ns"] <= emission["end_ns"]
                    <= row["requested_ns"] + BARRIER_NS, "outside census digest or span differs")
            if phase == "pre" and index == OUTSIDE_WINDOWS:
                require(emission["end_ns"] <= anchor, "final pre-census crossed transition")
            else:
                require(anchor <= row["begin_ns"], "census crossed its sample boundary")
    require(not outside_censuses, "unbound outside census emission")
    return during_census


def worker_rows(capture_dir, groups, manifest_sha, nonce, census, spans, timing, dispatch, decision):
    barriers = load_rows(capture_dir / "barrier.jsonl")
    worker = iter(row for row, _ in load_rows(capture_dir / "laptop-worker.jsonl"))
    identity = {key: dispatch[key] for key in ("pid", "creation_filetime")}
    worker_wait_ns = 0
    previous_census = None
    previous_request = dispatch["dispatch_ns"]
    settled_at = dispatch["launched_ns"]
    overlays = []
    require(len(groups) == len(barriers) == len(census), "barrier receipt population differs")
    price_count = 2 * sum(label.startswith("pricing:") for _, label, _ in groups)
    prices = read_pricing_membership(
        capture_dir / "pricing-membership.jsonl", identity, manifest_sha, nonce,
    ) if price_count else []
    require(len(prices) == price_count, "pricing barrier envelope population differs")
    price_acks = decision.get("pricing_ack_settled_ns", [])
    price_occupied = barrier_occupation_ns([row for row, _, _ in prices], price_acks)
    price_index = 0
    occupied = barrier_occupation_ns([gate for gate, _ in barriers], decision["ack_settled_ns"]) + price_occupied
    require(occupied == decision["occupied_ns"], "recorded barrier occupation differs from native completions")
    for index, ((ordinal, label, through), (gate, gate_digest), (raw, digest)) in enumerate(
        zip(groups, barriers, census)
    ):
        release = next(worker)
        area = gate["census"]
        requested, admitted, authorized, released = (
            gate["requested_ns"], gate["admitted_ns"], gate["authorized_ns"], release["release_ns"],
        )
        require_admission(previous_request, requested, admitted, dispatch["deadline_ns"])
        previous_request = requested
        require(identity == gate["identity"] == raw["identity"]
                and identity["pid"] == gate["client_pid"],
                "worker native identity differs across receipts")
        require(gate["manifest_sha256"] == manifest_sha and gate["nonce"] == nonce
                and release["barrier_sha256"] == gate_digest
                and (gate["group"], gate["label"], gate["settled_through"]) == (ordinal, label, through)
                and (release["kind"], release["group"], release["label"], release["settled_through"])
                == ("group_release", ordinal, label, through) and raw["label"] == label
                and area["sha256"] == digest and raw["begin_ns"] == area["begin_ns"]
                and raw["requested_ns"] == requested
                and settled_at <= release["requested_ns"] <= requested <= admitted <= area["begin_ns"]
                <= raw["finished_ns"] <= area["end_ns"] <= authorized <= released
                <= min(requested + BARRIER_NS, timing[1]),
                "worker crossed a census boundary or lost receipt custody")
        if previous_census is not None:
            require(0 < area["begin_ns"] - previous_census <= GROUP_NS, "census gap over 220 seconds")
        previous_census = area["begin_ns"]
        worker_wait_ns += released - requested
        settled_at = released
        next_through = groups[index + 1][2] if index + 1 < len(groups) else through
        for item_index in range(through + 1, next_through + 1):
            begin, end = next(worker), next(worker)
            require((begin["kind"], end["kind"]) == ("item_begin", "item_settled")
                    and begin["group"] == end["group"] == ordinal
                    and begin["index"] == end["index"] == item_index
                    and begin["identity"] == end["identity"]
                    and settled_at <= begin["begun_ns"] <= end["settled_ns"] <= timing[1],
                    "worker overlapped a census or changed slot identity")
            lo, hi = end.get("timing_start_ns"), end.get("timing_end_ns")
            if lo is not None or hi is not None:
                require(type(lo) is int and type(hi) is int
                        and begin["begun_ns"] <= lo <= hi <= end["settled_ns"],
                        "timed slot exceeded its settled bracket")
                overlays.append({"index": item_index, "overlaps": [
                    stamp_index for stamp_index, (left, right) in enumerate(spans)
                    if lo < hi and left < right and left < hi and lo < right
                ]})
            if label.startswith("pricing:"):
                member, member_digest, _ = prices[price_index]
                price_deadline = min(member["requested_ns"] + BARRIER_NS,
                                     requested + GROUP_NS, dispatch["deadline_ns"] - RESERVE_NS)
                require((member["group"], member["index"]) == (ordinal, item_index)
                        and member["deadline_ns"] == price_deadline
                        and end.get("pricing_membership_sha256") == member_digest
                        and hi is not None
                        and member["authorized_ns"] <= price_acks[price_index] <= price_deadline,
                        "pricing sender ACK crossed its original absolute deadline")
                check_pricing_receiver(member, end)
                price_index += 1
            settled_at = end["settled_ns"]
    require(price_index == len(prices), "unbound pricing membership receipt")
    terminal = next(worker)
    require(terminal["kind"] == "worker_terminal" and terminal["success"] is True
            and terminal["settled_through"] == groups[-1][2] and next(worker, None) is None,
            "worker stream is incomplete or has extra rows")
    require(10 * occupied <= timing[1] - timing[0], "raw invasive barrier occupation exceeds 10%")
    costs = [row["census"]["end_ns"] - row["census"]["begin_ns"] for row, _ in barriers]
    starts = [row["census"]["begin_ns"] for row, _ in barriers]
    gaps = [right - left for left, right in zip(starts, starts[1:])]
    return {
        "cheap_observer_overlap": overlays, "occupied_ns": occupied, "worker_barrier_wait_ns": worker_wait_ns,
        "maximum_census_span_ns": max(costs), "median_census_span_ns": median(costs),
        "maximum_census_gap_ns": max(gaps), "median_census_gap_ns": median(gaps),
        "census_spans_above_017_threshold": sum(cost > TRANSITION_NS for cost in costs),
    }


def replay(capture_dir, *, groups, manifest_sha, image_policy):
    """Recalculate chronology, coverage and safety from the append-only capture."""
    output = Path(capture_dir)
    phases, spans, origin, decision, dispatch, outside_censuses = telemetry_rows(output, manifest_sha)
    census = inventory_rows(output, image_policy, origin, phases, groups, outside_censuses)
    timing = (counter_time(phases["during"][0]), counter_time(phases["during"][-1]))
    require(type(decision["occupied_ns"]) is int and decision["occupied_ns"] > 0
            and 10 * decision["occupied_ns"] <= timing[1] - timing[0],
            "supervisor barrier occupation is invalid")
    evidence = worker_rows(output, groups, manifest_sha, origin["barrier_nonce"], census,
                           spans, timing, dispatch, decision)
    evidence["observer_spans_ns"] = [list(span) for span in spans]
    return evidence

