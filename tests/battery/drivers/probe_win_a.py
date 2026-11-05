# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Windows probe for battery case (a). Runs on win32 only; on any
other platform it prints nothing and exits 3.

For each charged tree size both proofs race the SAME job: one
job is filled, and — for the dying sizes — terminated once,
with both proofs armed on that one job from the common start
under the single 5,000 ms budget. One job can carry both waits
because the job handle's signalled-when-empty state is
non-consuming (WaitForSingleObject observes it without draining
anything) and the poll is a read (QueryInformationJobObject
active count every 10 ms — the tree's current Sleep(10) shape);
neither proof can take the emptiness event away from the other,
so no paired-job or termination-order artefact enters the
measurement, and each trial record says so (shared_job). Each
proof records the tree contract's return shape: (empty,
remainder), the remainder being the unspent part of the one
budget at the instant that proof returned, computed from the
measured elapsed time. The never-dies trial does NOT terminate
the member before the proofs run: both proofs consume the
budget against the live member and return (empty=false,
remainder) at it; only after the measurement closes is the job
terminated and the member reaped. The race repetitions spawn a
member that dies at a randomised delay, check it alive, then
start the wait — a lost wakeup is a wait that times out although
the job is empty. Observations are printed as JSON on stdout.

Race bound and deadline arithmetic. The race is bounded by the
question it asks, not by the teardown budget: the member's
randomised death delay is 0–5 ms, so the per-repetition wait
timeout is a short bound (RACE_TIMEOUT_MS, 250 ms); a repetition
whose wait times out while active_processes(job) == 0 is a
counted lost wakeup, and the record reports the repetitions
actually completed. The work is ordered so the equivalence
trials run first and the race runs last, under its own capped
sub-budget, so the race can never eat the trials' evidence: the
probe returns its trial observations (empty=False wait verdicts
against the poll's empty=True are a refutation the decider can
already express) even when the wait proof never signals.
Worst case, from the constants below: the equivalence trials
are 6 trials (sizes 0/1/2/64/65 dying, plus one never-dies)
each bounded by the single shared BUDGET_MS, run with both
proofs concurrent on threads, so 6 × 5,000 ms = 30,000 ms; the
race is 1,000 repetitions each bounded by RACE_TIMEOUT_MS, so
1,000 × 250 ms = 250,000 ms. Total probe worst case is
30,000 + 250,000 = 280,000 ms = 280 s, provably below the
1,800 s probe timeout (BUILD_TIMEOUT_S, applied by case_a.py).
"""

import ctypes
import json
import random
import subprocess
import sys
import threading
import time

BUDGET_MS = 5000
# The race's per-repetition wait bound and repetition count:
# see the deadline arithmetic in the module docstring —
# 1,000 × 250 ms = 250 s of race worst case, inside the
# probe's 1,800 s timeout alongside the trials' 30 s.
RACE_TIMEOUT_MS = 250
RACE_REPETITIONS = 1000

if sys.platform != "win32":
    sys.exit(3)

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000
JobObjectExtendedLimitInformation = 9
JobObjectBasicAccountingInformation = 1
WAIT_OBJECT_0 = 0


class BasicLimitInformation(ctypes.Structure):
    _fields_ = [("PerProcessUserTimeLimit", ctypes.c_int64),
                ("PerJobUserTimeLimit", ctypes.c_int64),
                ("LimitFlags", ctypes.c_uint32),
                ("MinimumWorkingSetSize", ctypes.c_size_t),
                ("MaximumWorkingSetSize", ctypes.c_size_t),
                ("ActiveProcessLimit", ctypes.c_uint32),
                ("Affinity", ctypes.c_size_t),
                ("PriorityClass", ctypes.c_uint32),
                ("SchedulingClass", ctypes.c_uint32)]


class IoCounters(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in (
        "ReadOperationCount", "WriteOperationCount",
        "OtherOperationCount", "ReadTransferCount",
        "WriteTransferCount", "OtherTransferCount")]


class ExtendedLimitInformation(ctypes.Structure):
    _fields_ = [("BasicLimitInformation", BasicLimitInformation),
                ("IoInfo", IoCounters),
                ("ProcessMemoryLimit", ctypes.c_size_t),
                ("JobMemoryLimit", ctypes.c_size_t),
                ("PeakProcessMemoryUsed", ctypes.c_size_t),
                ("PeakJobMemoryUsed", ctypes.c_size_t)]


class BasicAccountingInformation(ctypes.Structure):
    _fields_ = [("TotalUserTime", ctypes.c_int64),
                ("TotalKernelTime", ctypes.c_int64),
                ("ThisPeriodTotalUserTime", ctypes.c_int64),
                ("ThisPeriodTotalKernelTime", ctypes.c_int64),
                ("TotalPageFaultCount", ctypes.c_uint32),
                ("TotalProcesses", ctypes.c_uint32),
                ("ActiveProcesses", ctypes.c_uint32),
                ("TotalTerminatedProcesses", ctypes.c_uint32)]


def make_job():
    job = kernel32.CreateJobObjectW(None, None)
    info = ExtendedLimitInformation()
    info.BasicLimitInformation.LimitFlags = (
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)
    kernel32.SetInformationJobObject(
        job, JobObjectExtendedLimitInformation, ctypes.byref(info),
        ctypes.sizeof(info))
    return job


def active_processes(job):
    info = BasicAccountingInformation()
    ok = kernel32.QueryInformationJobObject(
        job, JobObjectBasicAccountingInformation, ctypes.byref(info),
        ctypes.sizeof(info), None)
    return info.ActiveProcesses if ok else None


def spawn_sleeper(seconds):
    return subprocess.Popen(
        [sys.executable, "-c", f"import time; time.sleep({seconds})"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def poll_proof(job, start, out):
    empty = active_processes(job) == 0
    while not empty \
            and (time.perf_counter() - start) * 1000 < BUDGET_MS:
        time.sleep(0.010)
        empty = active_processes(job) == 0
    elapsed = (time.perf_counter() - start) * 1000
    out["poll"] = (empty, elapsed, max(0.0, BUDGET_MS - elapsed))


def wait_proof(job, start, out):
    remaining = BUDGET_MS - (time.perf_counter() - start) * 1000
    signalled = kernel32.WaitForSingleObject(
        job, max(0, int(remaining))) == WAIT_OBJECT_0
    empty = signalled and active_processes(job) == 0
    elapsed = (time.perf_counter() - start) * 1000
    out["wait"] = (empty, elapsed, max(0.0, BUDGET_MS - elapsed))


def run_trial(size, never_dies):
    # One job, filled once and (for the dying sizes) terminated
    # once; both proofs are armed on that same job.
    job = make_job()
    members = [spawn_sleeper(30 if never_dies else 0.3)
               for _ in range(size)]
    for member in members:
        kernel32.AssignProcessToJobObject(job, int(member._handle))
    if not never_dies:
        # The single termination is the one emptiness event both
        # proofs race. The never-dies member is deliberately NOT
        # terminated here — its proofs must consume the budget
        # against a live member.
        kernel32.TerminateJobObject(job, 1)
    start = time.perf_counter()
    out = {}
    threads = [threading.Thread(target=poll_proof,
                                args=(job, start, out)),
               threading.Thread(target=wait_proof,
                                args=(job, start, out))]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()
    if never_dies:
        # The measurement has closed; only now may the member die.
        kernel32.TerminateJobObject(job, 1)
    kernel32.CloseHandle(job)
    for member in members:
        try:
            member.kill()
        except OSError:
            pass
        member.wait()
    poll_empty, poll_latency, poll_remainder = out["poll"]
    wait_empty, wait_latency, wait_remainder = out["wait"]
    return {"size": size, "never_dies": never_dies,
            "event": f"single-job-{size}-"
                     f"{'never' if never_dies else 'dies'}",
            "shared_job": True,
            "poll_empty": poll_empty, "wait_empty": wait_empty,
            "poll_latency_ms": poll_latency,
            "wait_latency_ms": wait_latency,
            "poll_remainder_ms": poll_remainder,
            "wait_remainder_ms": wait_remainder}


def run_race(repetitions=RACE_REPETITIONS):
    lost = 0
    completed = 0
    rng = random.Random(20261029)
    for _ in range(repetitions):
        job = make_job()
        member = spawn_sleeper(rng.uniform(0.0, 0.005))
        kernel32.AssignProcessToJobObject(job, int(member._handle))
        _ = active_processes(job)  # the non-empty check
        signalled = kernel32.WaitForSingleObject(
            job, RACE_TIMEOUT_MS)
        if signalled != WAIT_OBJECT_0 and active_processes(job) == 0:
            lost += 1
        kernel32.CloseHandle(job)
        member.wait()
        completed += 1
    return {"repetitions": completed, "lost_wakeups": lost}


def main():
    # The equivalence trials run first; the race runs last,
    # under its own capped sub-budget, so it cannot eat the
    # trials' evidence even when the wait proof never signals.
    trials = [run_trial(size, False) for size in (0, 1, 2, 64, 65)]
    trials.append(run_trial(1, True))
    race = run_race()
    print(json.dumps({"trials": trials, "race": race}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
