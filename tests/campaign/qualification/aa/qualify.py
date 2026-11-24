# SPDX-License-Identifier: Apache-2.0
"""Bounded native qualification of the fixed laptop A/A recipe."""

import argparse
from contextlib import ExitStack
import copy
import hashlib
import mmap
import os
from pathlib import Path
import secrets
import struct
import subprocess
import sys
import time

NS = 1_000_000_000
CAP_SECONDS = 600
DISPATCH_SECONDS = 145
OFFSET_NS = 1_000_000
RECORD = struct.Struct("<QQQ")
ORDERS = (("off", "on"), ("on", "off"))
EMPTY = {"active_before": 0, "terminated": False, "active_after": 0}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def pairs():
    from laptop_schedule import ScheduleRandom, SEED_BYTES

    rows = [{"context": context, "recipe": recipe, "order": list(order), "rep": rep}
            for context in ("DETACHED", "NO_WINDOW")
            for recipe, count in (("long", 3), ("relay", 15))
            for order in ORDERS for rep in range(count)]
    ScheduleRandom(b"ptg/laptop-182-001/a-a", SEED_BYTES).shuffle(rows)
    return [{"pair": index, **row} for index, row in enumerate(rows)]


def remaining_ms(deadline, local_cap=10):
    ns = deadline - time.perf_counter_ns()
    require(ns > 0, "qualification deadline exceeded")
    return min(local_cap * 1000, (ns + 999_999) // 1_000_000)


def wait(handles, deadline):
    import _winapi as win

    status = win.WaitForMultipleObjects(handles, False, remaining_ms(deadline))
    require(status == win.WAIT_OBJECT_0, f"dispatch rendezvous failed, wait status {status}")
    require(time.perf_counter_ns() < deadline, "qualification deadline exceeded")


def mailbox(stack, output, nonce, owner):
    import _winapi as win
    import ctypes as C
    from ctypes import wintypes as W

    kernel = C.WinDLL("kernel32", use_last_error=True)
    kernel.CreateEventW.argtypes = [W.LPVOID, W.BOOL, W.BOOL, W.LPCWSTR]
    kernel.CreateEventW.restype = W.HANDLE
    kernel.OpenEventW.argtypes = [W.DWORD, W.BOOL, W.LPCWSTR]
    kernel.OpenEventW.restype = W.HANDLE
    kernel.SetEvent.argtypes = [W.HANDLE]
    kernel.SetEvent.restype = W.BOOL
    handles = []
    for tag in ("dispatch", "settle"):
        name = f"Local\\ptg-aa-{nonce}-{tag}"
        if owner:
            C.set_last_error(0)
            handle = kernel.CreateEventW(None, False, False, name)
            if handle and C.get_last_error() == 183:
                win.CloseHandle(handle)
                raise ValueError("fresh dispatch rendezvous event already exists")
        else:
            handle = kernel.OpenEventW(0x00100002, False, name)
        require(handle, f"could not {'create' if owner else 'open'} {tag} event")
        stack.callback(win.CloseHandle, handle)
        handles.append(handle)

    # One writer publishes a complete row before signaling the dispatch event.
    # The reader waits on that event before checking both sequence copies and
    # rereading the row. The writer cannot publish again before the settle event.
    file = stack.enter_context((output / "dispatch.bin").open("x+b" if owner else "r+b"))
    if owner:
        file.write(bytes(RECORD.size))
        file.flush()
    shared = stack.enter_context(mmap.mmap(file.fileno(), RECORD.size))

    def send(handle):
        if not kernel.SetEvent(handle):
            raise C.WinError(C.get_last_error())

    return shared, *handles, send


def stock_slot_drained(receipt):
    from harness import _slot_job_drained

    return _slot_job_drained(receipt) and receipt.keys() == EMPTY.keys()


def slot_oracle(row, path, executable, fixture):
    require(row["outcome"] == "ok", f"{path} stock oracle: {row['outcome']}: {row.get('detail')}")
    require(Path(row["argv"][0]).samefile(executable), "wrong executable in stock slot")
    require(any(arg and os.path.isfile(arg) and Path(arg).samefile(fixture) for arg in row["argv"][1:]),
            "frozen fixture missing in observed argv")
    if path == "fast":
        require(row["exit_code"] == 0 and row["output_stdout_match"] is True
                and row["output_stderr_match"] is True and row["output_stdout_len"] == 18
                and row["output_stderr_len"] == 13 and "laptop_stream_error" not in row,
                "stock relay streams or exit did not match")
    else:
        from report import _is_pid_form

        arm, marked = row.get("arm_pid"), row.get("grandchild_pid")
        child, marker = row.get("child_pid"), row.get("tree_marker")
        require(row["exit_code"] == 124 and row.get("survivor") is False
                and isinstance(marker, str) and marker
                and _is_pid_form(arm) and _is_pid_form(marked) and arm != marked
                and (child is None or (_is_pid_form(child) and child not in (arm, marked))),
                "stock deadline did not witness the marked payload gone before cleanup")
    require(stock_slot_drained(row.get("slot_job")), "stock slot Job did not prove cleanup")


def relay_retention(output, lane):
    def retain(row, paths):
        copies = {}
        for stream, path in paths.items():
            with Path(path).open("rb") as source:
                raw = source.read(1_048_577)
            require(len(raw) <= 1_048_576, "qualification stream exceeds retention cap")
            digest = hashlib.sha256(raw).hexdigest()
            require(digest == row[f"output_{stream}_sha256"]
                    and len(raw) == row[f"output_{stream}_len"],
                    "retained bytes differ from stock timed observation")
            name = f"lane-{lane:03d}.{stream}"
            with (output / name).open("xb") as target:
                target.write(raw)
                target.flush()
                os.fsync(target.fileno())
            copies[stream] = {"path": name, "size": len(raw), "sha256": digest}
        row["aa_streams"] = copies

    return retain


def backup_controls(manifest, config, emit):
    import runner
    from laptop_barrier import process_in_job
    from laptop_telemetry import INSTRUMENTATION_WORKER_FLAGS
    from plan import Slot
    from windows_slot_job import SlotJob

    candidate = manifest["files"]["guard"]["path"]
    fixture = manifest["files"]["fixture"]["path"]
    malformed = runner.run_slot(
        Slot("control", "guard", "malformed", "NO_WINDOW", 0, "malformed-timeout"), config)
    emit("candidate_malformed", observation=malformed)
    require(malformed["outcome"] == "ok" and malformed["exit_code"] == 125
            and Path(malformed["argv"][0]).samefile(candidate),
            "candidate malformed-timeout refusal did not match")
    require(stock_slot_drained(malformed.get("slot_job")), "malformed slot Job did not prove cleanup")
    current_timeout = runner.run_slot(Slot("control", "guard", "kill", "NO_WINDOW", 0), config)
    emit("candidate_timeout", observation=current_timeout)
    slot_oracle(current_timeout, "kill", candidate, fixture)
    backup = manifest["files"]["primary_guard"]["path"]
    require(not Path(backup).samefile(candidate), "candidate replaced its independent backup")
    independent = copy.deepcopy(config)
    independent["arms"]["guard"]["executable"] = backup
    observation = runner.run_slot(
        Slot("control", "guard", "kill", "NO_WINDOW", 0), independent)
    emit("backup_timeout", observation=observation)
    slot_oracle(observation, "kill", backup, fixture)

    job = SlotJob()
    try:
        process = job.launch([fixture, "sleep", "30"], INSTRUMENTATION_WORKER_FLAGS,
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL)
        try:
            before = process_in_job(process._handle, job._handle)
            try:
                process.wait(0)
            except subprocess.TimeoutExpired:
                emit("owned_survivor", identity=before,
                     creation_flags=INSTRUMENTATION_WORKER_FLAGS)
            else:
                raise ValueError("deliberate ordinary fixture was not alive before independent drain")
            receipt = job.drain()
            code = process.wait(1)
            emit("owned_survivor_settled", identity=before, job=receipt, exit_code=code)
            require(receipt == {"active_before": 1, "terminated": True, "active_after": 0}
                    and code != 0, "independent forced drain was not witnessed")
        finally:
            process.close()
    finally:
        job.close()


def clock_anchor(output, job, clocks):
    from dataclasses import asdict
    from laptop_telemetry import resources, sample_native
    from quiet_gate import sample_errors

    outer = time.perf_counter_ns()
    choices = resources(output)
    sample = sample_native(job, choices)
    sampler = time.thread_time_ns()
    invasive = clocks.read()
    from laptop_telemetry import captured_resource_errors

    return {"outer_begin_ns": outer, "outer_end_ns": time.perf_counter_ns(),
            "raw": asdict(sample), "resources": choices, "sampler_cpu_ns": sampler,
            "invasive_cpu_ns": invasive, "errors": sample_errors(sample, during=True)
            + captured_resource_errors(choices)}


def worker(args, manifest):
    from envcase import suppress_windows_error_dialogs
    from harness import load_config
    from laptop_barrier import BarrierClient, append_row
    from plan import Slot
    from laptop_telemetry import at
    import runner

    suppress_windows_error_dialogs()
    config = load_config(manifest["inputs"]["config"])
    config["_windows_slot_jobs"] = True
    candidate = manifest["files"]["guard"]["path"]
    fixture = manifest["files"]["fixture"]["path"]
    require(Path(config["arms"]["guard"]["executable"]).samefile(candidate)
            and all(Path(config["children"][kind]["program"]).samefile(fixture)
                    for kind in ("kill", "fast")), "stock worker config differs from frozen images")
    with ExitStack() as stack:
        shared, dispatch, settle, send = mailbox(stack, args.output, args.nonce, False)
        stream = stack.enter_context((args.output / "worker.jsonl").open("xb"))
        gate = stack.enter_context(BarrierClient(args.pipe, args.nonce, args.manifest_sha))

        def emit(kind, **fields):
            return append_row(stream, {"kind": kind, **fields})

        try:
            backup_controls(manifest, config, emit)
            trusted = manifest["files"]["primary_guard"]["path"]
            long_config = copy.deepcopy(config)
            long_config["arms"]["guard"]["executable"] = trusted
            for pair in pairs():
                number = pair["pair"]
                release = gate.checkpoint(number, "aa-pair", 2 * number - 1)
                emit("pair_release", pair=number, release_ns=release,
                     barrier_sha256=gate.last_receipt_sha256)
                path = "fast" if pair["recipe"] == "relay" else "kill"
                for position, mode in enumerate(pair["order"]):
                    lane = 2 * number + position
                    slot = Slot("control", "guard", path, pair["context"], pair["rep"])
                    retain = relay_retention(args.output, lane)
                    require(time.perf_counter_ns() < args.deadline, "worker budget ended before dispatch")
                    stamped = time.perf_counter_ns()
                    RECORD.pack_into(shared, 0, lane + 1, stamped, lane + 1)
                    send(dispatch)
                    selected = config if path == "fast" else long_config
                    observation = runner.run_slot(slot, selected, retention=retain)
                    settled_ns = time.perf_counter_ns()
                    emit("lane", lane=lane, mode=mode, dispatch_ns=stamped,
                         settled_ns=settled_ns, observation=observation)
                    slot_oracle(observation, path, candidate if path == "fast" else trusted, fixture)
                    wait([settle], args.deadline)
            count = len(pairs())
            released = gate.checkpoint(count, "tail", 2 * count - 1)
            emit("tail_release", released_ns=released, barrier_sha256=gate.last_receipt_sha256)
            padding_start = time.perf_counter_ns()
            at(args.tail)
            emit("fixed_tail", requested_ns=args.tail, idle_start_ns=padding_start,
                 finished_ns=time.perf_counter_ns())
            emit("worker_terminal", passed=True)
        except Exception as error:
            emit("worker_terminal", passed=False, error=f"{type(error).__name__}: {error}")
            raise
    return 0


def supervise(args, manifest, launch):
    import _winapi as win
    from envcase import suppress_windows_error_dialogs
    from laptop_barrier import BarrierServer, append_row
    from laptop_schedule import canonical_bytes
    from laptop_telemetry import (
        at, counter_time, held_worker_launch, INSTRUMENTATION_WORKER_FLAGS,
        InvasiveThreadClock, job_cpu, Observer,
        outside, OUTSIDE_WINDOWS, PERIOD, wait_for_worker, within_transition,
    )
    from windows_slot_job import SlotJob

    suppress_windows_error_dialogs()
    output = args.output
    output.mkdir()
    nonce = secrets.token_hex(32)
    pipe = launch.worker_pipe_name(nonce)
    plan = pairs()
    groups = [(pair["pair"], "aa-pair", 2 * pair["pair"] - 1) for pair in plan]
    groups.append((len(plan), "tail", 2 * len(plan) - 1))
    deadline = time.perf_counter_ns() + CAP_SECONDS * NS
    source = Path(__file__).resolve()
    driver_hash = args.driver_sha
    replay_source = source.with_name("replay_aa.py")
    stage = Path(args.manifest).resolve()
    argv = [manifest["files"]["python"]["path"], "-I", "-B", "-X", "utf8", str(source),
            "--stage", str(args.stage), "--manifest", str(stage), "--manifest-sha", args.manifest_sha,
            "--output", str(output), "--pipe", pipe, "--nonce", nonce, "--deadline", str(deadline),
            "--addendum", str(args.addendum), "--addendum-sha", args.addendum_sha,
            "--driver-sha", driver_hash, "--replay-sha", args.replay_sha]

    with ExitStack() as stack:
        refs = [*launch.iter_held_refs(manifest), launch.file_identity(stage, args.manifest_sha),
                launch.file_identity(source, driver_hash),
                launch.file_identity(replay_source, args.replay_sha),
                launch.file_identity(args.addendum, args.addendum_sha)]
        held_refs = [launch.windows_read_hold(stack, ref) for ref in refs]
        ancestry = launch.hold_guard_ancestry(stack, manifest)
        stdout = stack.enter_context((output / "worker.stdout").open("xb"))
        stderr = stack.enter_context((output / "worker.stderr").open("xb"))
        journal = stack.enter_context((output / "supervisor.jsonl").open("xb"))
        inventories = stack.enter_context((output / "inventory.jsonl").open("xb"))
        guard = ancestry.primary_guard()
        shared, dispatch, settle, send = mailbox(stack, output, nonce, True)
        launch.write_exclusive(output / "plan.json", plan)
        observer_metadata = {"guard_pid": guard[0], "guard_created_ticks": guard[2],
                             "manifest_sha256": args.manifest_sha, "addendum_sha256": args.addendum_sha,
                             "driver_sha256": driver_hash, "observer_pid": os.getpid(),
                             "guard_ancestry": ancestry.proof}
        append_row(journal, {"kind": "started", **observer_metadata})
        def recipe(argv, tail_ns):
            launch.write_exclusive(output / "recipe.json", {
            "manifest_sha256": args.manifest_sha, "driver_sha256": driver_hash,
            "replay_sha256": args.replay_sha,
            "plan_sha256": hashlib.sha256(canonical_bytes(plan)).hexdigest(),
            "worker_argv": argv, "worker_creation_flags": INSTRUMENTATION_WORKER_FLAGS,
            "deadline_ns": deadline,
            "guard_ancestry": ancestry.proof, "supervisor_pid": os.getpid(),
            "held": [{"path": str(file), "device": stat.st_dev, "file": stat.st_ino}
                     for file, stat in held_refs],
            "caps": {"qualification_s": CAP_SECONDS, "dispatch_s": DISPATCH_SECONDS,
                     "receipt_rendezvous_s": 10},
            "offset_ns": OFFSET_NS, "minimum_during_ns": 65 * NS, "tail_ns": tail_ns,
            "addendum_sha256": args.addendum_sha,
            })
        observer = Observer(output, journal, inventories, ancestry, manifest["image_policy"])
        server = stack.enter_context(BarrierServer(
            pipe, nonce, args.manifest_sha, groups, output / "barrier.jsonl",
            lambda job, identity, group, label, requested: observer.census(
                "during", group, job, requested, identity=identity, label=label)))
        clocks = InvasiveThreadClock(server.thread.native_id)
        stack.callback(clocks.close)
        job = SlotJob()
        stack.callback(job.close)
        worker_proc = None
        try:
            info = time.get_clock_info("perf_counter")
            observer.emit("qpc", source=info.implementation, resolution_s=info.resolution,
                          monotonic=info.monotonic, adjustable=info.adjustable)
            require(info.monotonic and not info.adjustable
                    and "QueryPerformanceCounter" in info.implementation
                    and 0 < info.resolution <= 0.000001, "native QPC contract not met")
            job_cpu(job)
            last_pre = outside(observer, server, clocks, "pre")
            first = previous = observer.sample("during", job, clocks)
            within_transition(counter_time(last_pre), counter_time(first))
            for field in ("job_before", "job_after"):
                require(first["raw"][field] == {"kernel": 0, "user": 0, "total_processes": 0,
                                                "active_processes": 0},
                        "qualification Job was not initially empty")
            start = counter_time(first)
            tail_ns = start + 65 * NS
            exact_argv = argv + ["--tail", str(tail_ns)]
            recipe(exact_argv, tail_ns)
            created = time.perf_counter_ns()
            within_transition(start, created)
            worker_proc = job.launch(exact_argv, INSTRUMENTATION_WORKER_FLAGS,
                                     stdin=subprocess.DEVNULL,
                                     stdout=stdout, stderr=stderr, cwd=manifest["working_directory"])
            stack.callback(worker_proc.close)
            identity = held_worker_launch(job, worker_proc, exact_argv, manifest["files"]["python"])
            observer.emit("worker_dispatched", dispatch_ns=created,
                          creation_flags=INSTRUMENTATION_WORKER_FLAGS,
                          launched_ns=time.perf_counter_ns(), deadline_ns=deadline, **identity)
            served = server.bind_worker(job, worker_proc, created, deadline)
            next_sample = start + PERIOD
            last_start = None
            complete = 0

            def periodic():
                nonlocal previous, last_start, next_sample, complete
                at(next_sample)
                row = observer.sample("during", job, clocks, previous=previous, last_start=last_start)
                last_start, previous = previous, row
                next_sample += PERIOD
                complete += 1
                require(time.perf_counter_ns() < deadline, "deadline elapsed while sampling")

            dispatch_wait = [dispatch, server.failed_handle, worker_proc._handle]

            def next_dispatch():
                expiry = min(deadline, time.perf_counter_ns() + DISPATCH_SECONDS * NS)
                while True:
                    if next_sample >= expiry:
                        # Same finite watchdog over the whole dispatch, including anchor work.
                        wait(dispatch_wait, expiry)
                        return
                    interval = max(0, next_sample - time.perf_counter_ns())
                    status = win.WaitForMultipleObjects(
                        dispatch_wait, False, (interval + 999_999) // 1_000_000,
                    )
                    if status == win.WAIT_OBJECT_0:
                        return
                    if status != win.WAIT_TIMEOUT:
                        server.check()
                        raise ValueError(f"worker exited or dispatch wait failed: {status}")
                    ready = win.WaitForMultipleObjects(dispatch_wait, False, 0)
                    if ready == win.WAIT_OBJECT_0:
                        return
                    if ready != win.WAIT_TIMEOUT:
                        server.check()
                        raise ValueError(f"worker exited or dispatch wait failed: {ready}")
                    periodic()

            previous_trace = None
            for lane, mode in enumerate(mode for pair in plan for mode in pair["order"]):
                next_dispatch()
                outer_start = time.perf_counter_ns()
                first_raw = RECORD.unpack_from(shared)
                sequence, dispatched, repeated_sequence = RECORD.unpack_from(shared)
                received = time.perf_counter_ns()
                require(first_raw == (sequence, dispatched, repeated_sequence)
                        and sequence == repeated_sequence == lane + 1
                        and created <= dispatched <= outer_start <= received,
                        "stale, torn or out-of-order exact dispatch marker")
                requested = dispatched + OFFSET_NS
                while next_sample <= requested:
                    periodic()
                at(requested)
                wake = time.perf_counter_ns()
                reading = clock_anchor(output, job, clocks) if mode == "on" else None
                observer.emit("wake", lane=lane, mode=mode, dispatch_ns=dispatched,
                              outer_begin_ns=outer_start, received_ns=received,
                              requested_ns=requested, wake_ns=wake,
                              reading=reading, previous_trace=previous_trace)
                require(wake >= requested and time.perf_counter_ns() < deadline,
                        "invalid scheduled 1ms wake")
                require(reading is None or not reading["errors"],
                        f"ON native read errors: {None if reading is None else reading['errors']}")
                send(settle)
                previous_trace = {"lane": lane, "outer_end_ns": time.perf_counter_ns()}
            observer.emit("wake_trace_end", **previous_trace)
            while True:
                exit_code = wait_for_worker(server, worker_proc, min(deadline, next_sample))
                if exit_code is not None:
                    break
                require(time.perf_counter_ns() < deadline, "worker did not reach terminal deadline")
                periodic()
            endpoint = observer.sample("during", job, clocks, previous=previous,
                                       last_start=last_start, endpoint=True)
            require(served.wait(remaining_ms(deadline) / 1000), "tail barrier did not settle")
            server.check()
            drain = job.drain()
            observer.emit("natural_drain", code=exit_code, **drain)
            require(exit_code == 0 and drain == EMPTY and complete >= OUTSIDE_WINDOWS
                    and counter_time(endpoint) >= tail_ns
                    and len(server.authorizations) == len(groups),
                    "A/A worker, 5s accounting coverage or containment failed")
            observer.emit("phase_decision", phase="during", complete_windows=complete)
            outside(observer, server, clocks, "post", after=endpoint)
            observer.trace_end()
            observer.emit("barrier_settled", ack_settled_ns=server.ack_settled_ns)
            for ref, (file, original) in zip(refs, held_refs, strict=True):
                reopened, terminal = launch.windows_read_hold(stack, ref)
                require(file == reopened and os.path.samestat(original, terminal),
                        "terminal held qualification identity changed")
            observer.emit("terminal", scope="native A/A capture only", capture_finished=True)
        except Exception as error:
            observer.emit("failure", error=f"{type(error).__name__}: {error}")
            try:
                observer.emit("abort_drain", **job.drain())
            except Exception as cleanup:
                observer.emit("abort_drain_failure", error=f"{type(cleanup).__name__}: {cleanup}")
            observer.emit("terminal", scope="native A/A capture only", capture_finished=False)
            raise
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--manifest-sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--addendum", type=Path, required=True)
    parser.add_argument("--addendum-sha", required=True)
    parser.add_argument("--driver-sha", required=True)
    parser.add_argument("--replay-sha", required=True)
    parser.add_argument("--tail", type=int)
    parser.add_argument("--pipe")
    parser.add_argument("--nonce")
    parser.add_argument("--deadline", type=int)
    args = parser.parse_args()
    require(sys.platform == "win32" and sys.flags.isolated and sys.flags.dont_write_bytecode,
            "qualification requires native Windows and Python -I -B")
    args.stage = args.stage.resolve(strict=True)
    args.manifest = args.manifest.resolve(strict=True)
    args.output = args.output.resolve()
    args.addendum = args.addendum.resolve(strict=True)
    require(args.stage.is_dir() and args.manifest.parent == args.stage
            and not args.output.is_relative_to(args.stage),
            "qualification uses frozen stage plus fresh separate output")
    tests = args.stage / "tests"
    require(not any((directory / "__pycache__").exists()
                    for directory in (tests, tests / "campaign")),
            "staged Python bytecode cache prohibited")
    sys.path.insert(0, str(tests / "campaign"))
    import laptop_launch as launch

    source = Path(__file__).resolve()
    launch.file_identity(source, args.driver_sha)
    launch.file_identity(source.with_name("replay_aa.py"), args.replay_sha)
    manifest = launch.load_manifest(args.manifest, args.manifest_sha)
    launch.code_refs(manifest)
    require(launch.file_identity(args.addendum, args.addendum_sha)
            == manifest["protocol"]["ADDENDUM-A-A-001.md"],
            "qualification addendum differs from the sealed campaign protocol")
    current = launch.describe(Path(manifest["inputs"]["description"]["path"]),
                              Path(manifest["capture"]), manifest["run_id"])
    require(current == manifest, "current staged configuration differs from pending manifest")
    require(Path(sys.executable).samefile(manifest["files"]["python"]["path"]),
            "qualifier Python executable differs from freeze")
    launch.bind_observers(manifest)
    for key in list(os.environ):
        if key.upper() == "RUST_LOG":
            del os.environ[key]
    if args.pipe is None:
        require(args.nonce is None and args.deadline is None and args.tail is None,
                "invalid supervisor recipe")
        return supervise(args, manifest, launch)
    require(args.deadline is not None and 0 < args.deadline - time.perf_counter_ns() <= CAP_SECONDS * NS
            and args.tail is not None and time.perf_counter_ns() < args.tail < args.deadline
            and args.nonce is not None and args.pipe == launch.worker_pipe_name(args.nonce),
            "invalid native worker recipe")
    require(Path.cwd().samefile(manifest["working_directory"]), "unfrozen worker working directory")
    return worker(args, manifest)


if __name__ == "__main__":
    raise SystemExit(main())

