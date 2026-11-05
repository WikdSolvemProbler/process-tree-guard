# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Windows probe for battery case (o). Runs on win32 only; on any
other platform it prints nothing and exits 3 (the --child mode,
which tests whether a handle value is usable in a descendant,
and the --forge mode, which posts forged packets, have the same
gate).

Executes the charged burden at the API level: a job associated
with a completion port (SetInformationJobObject with the
association class, as the tree's extendable path does), members
spawned and terminated, packets dequeued with
GetQueuedCompletionStatus, and every verdict taken from
QueryInformationJobObject at the decision instant — never from a
packet. Nothing decided is a constant of this program:

- Zero-packet delivery counts the active-process-zero packets
  (message type 4) actually dequeued per zero-trial; exactly-
  once is that count, compared to 1 by the driver.
- The packet-stream composition counts, per tree size, the exit
  packets and zero packets actually dequeued.
- Port-close accounting performs the real teardown sequence —
  create the port, associate, disassociate (a zeroed
  association), drain-check, close — checking every call's
  return value: each operation is recorded with its own
  confirmation (the disassociation is confirmed both by its
  call's success and by a subsequent association-state query
  showing no port), and the count the driver decides from is
  the number of confirmed operations in the observed sequence,
  per cycle, across 10,000 cycles leak-checked by process
  handle count; consumers after close are counted as packets
  still delivered after disassociation.
- The fallback trial injects association failure at each point
  the implementation exposes (a null port handle, an invalid
  job handle, an already-closed port) and records two
  independent observations at each point: the fallback verdict
  (the poll proof run on the job the association failed for)
  and a poll verdict from a separate poll run over a separately
  created identical job state. Each observation carries its
  own observation id (its job handle and member pid).
- The forged-packet steps run in a SEPARATE child process: the
  child inherits the port handle and posts packets whose
  payload fields (bytes transferred, completion key, overlapped
  value) vary over the shapes the port accepts, including the
  active-process-zero message type claiming an empty job while
  a member lives and an exit message type claiming the opposite
  once the job is empty. The verdict function under test
  receives each dequeued packet's fields — a code path that
  could in principle read them — and returns the job query's
  answer. Running this probe with --sensitive swaps in a
  verdict function that DOES read the packet's claim; the same
  driver must refute that run (the negative control that proves
  the test can see packet influence).

Non-inheritance spawns the checking child WITH handle
inheritance enabled and the port handle explicitly offered
through the inheritance path (a STARTUPINFOEX handle list,
mirroring the forged trial's inheritance handling), while the
port handle itself is created non-inheritable and its inherit
flag is cleared and read back before the spawn — so the trial
passes only if the handle truly does not cross. The record
states the creation attributes used. A positive control proves
the machinery can see inheritance: a deliberately inheritable
event handle, offered through the same spawn, must be reported
inherited by the same child check.
"""

import ctypes
import json
import os
import subprocess
import sys
import time

if sys.platform != "win32":
    sys.exit(3)

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
JobObjectAssociateCompletionPortInformation = 7
JobObjectBasicAccountingInformation = 1
JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO = 4
JOB_OBJECT_MSG_EXIT_PROCESS = 7
JOB_OBJECT_MSG_ABNORMAL_EXIT_PROCESS = 8
HANDLE_FLAG_INHERIT = 1
WAIT_TIMEOUT = 0x102
BUDGET_MS = 5000


class AssociateCompletionPort(ctypes.Structure):
    _fields_ = [("CompletionKey", ctypes.c_void_p),
                ("CompletionPort", ctypes.c_void_p)]


class BasicAccountingInformation(ctypes.Structure):
    _fields_ = [("TotalUserTime", ctypes.c_int64),
                ("TotalKernelTime", ctypes.c_int64),
                ("ThisPeriodTotalUserTime", ctypes.c_int64),
                ("ThisPeriodTotalKernelTime", ctypes.c_int64),
                ("TotalPageFaultCount", ctypes.c_uint32),
                ("TotalProcesses", ctypes.c_uint32),
                ("ActiveProcesses", ctypes.c_uint32),
                ("TotalTerminatedProcesses", ctypes.c_uint32)]


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


def make_associated_job():
    job = kernel32.CreateJobObjectW(None, None)
    port = kernel32.CreateIoCompletionPort(
        ctypes.c_void_p(-1), None, 0, 1)
    assoc = AssociateCompletionPort(0, port)
    ok = kernel32.SetInformationJobObject(
        job, JobObjectAssociateCompletionPortInformation,
        ctypes.byref(assoc), ctypes.sizeof(assoc))
    return job, port, bool(ok)


def associate(job, port):
    assoc = AssociateCompletionPort(0, port)
    return bool(kernel32.SetInformationJobObject(
        job, JobObjectAssociateCompletionPortInformation,
        ctypes.byref(assoc), ctypes.sizeof(assoc)))


def dequeue(port, timeout_ms):
    """One dequeue; returns the packet's fields as a dict, or
    None on timeout."""
    bytes_out = ctypes.c_uint32()
    key = ctypes.c_size_t()
    overlapped = ctypes.c_void_p()
    ok = kernel32.GetQueuedCompletionStatus(
        port, ctypes.byref(bytes_out), ctypes.byref(key),
        ctypes.byref(overlapped), timeout_ms)
    if not ok and not bytes_out.value and not key.value \
            and not overlapped.value:
        return None
    return {"bytes": bytes_out.value, "key": key.value,
            "overlapped": overlapped.value or 0}


def prove_empty(port, job, budget_ms=BUDGET_MS):
    """Wake-driven loop; the verdict is the job query's alone.
    Returns (empty, zero_packets_dequeued, total_dequeued)."""
    deadline = time.perf_counter() + budget_ms / 1000.0
    zero_packets = 0
    total = 0
    while True:
        if active_processes(job) == 0 and zero_packets >= 1:
            return True, zero_packets, total
        remaining = deadline - time.perf_counter()
        if remaining <= 0:
            return active_processes(job) == 0, zero_packets, total
        packet = dequeue(port, max(1, int(remaining * 1000)))
        if packet is not None:
            total += 1
            if packet["bytes"] == JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO:
                zero_packets += 1


def zero_packet_trials():
    trials = []
    # Empty before association: job created, member lived and
    # died, then association is made.
    job = kernel32.CreateJobObjectW(None, None)
    member = spawn_sleeper(0.1)
    kernel32.AssignProcessToJobObject(job, int(member._handle))
    member.wait()
    port = kernel32.CreateIoCompletionPort(ctypes.c_void_p(-1),
                                           None, 0, 1)
    associate(job, port)
    empty, zero, total = prove_empty(port, job)
    trials.append({"phase": "before", "zero_packets_dequeued": zero,
                   "total_dequeued": total, "empty": empty})
    kernel32.CloseHandle(port)
    kernel32.CloseHandle(job)
    # Empty at association and after association.
    for phase, die_after in (("at", 0.0), ("after", 0.3)):
        job, port, _ok = make_associated_job()
        member = spawn_sleeper(die_after)
        kernel32.AssignProcessToJobObject(job, int(member._handle))
        empty, zero, total = prove_empty(port, job)
        trials.append({"phase": phase,
                       "zero_packets_dequeued": zero,
                       "total_dequeued": total, "empty": empty})
        kernel32.CloseHandle(port)
        kernel32.CloseHandle(job)
    return trials


def composition_counts():
    counts = {}
    for size in (1, 64, 65, 300):
        job, port, _ok = make_associated_job()
        members = [spawn_sleeper(0.2) for _ in range(size)]
        for member in members:
            kernel32.AssignProcessToJobObject(job,
                                              int(member._handle))
        kernel32.TerminateJobObject(job, 1)
        # Count the packet stream itself until the zero packet
        # has been dequeued and the job queries empty.
        exit_packets = 0
        zero_packets = 0
        other_packets = 0
        total = 0
        deadline = time.perf_counter() + BUDGET_MS / 1000.0
        while time.perf_counter() < deadline:
            packet = dequeue(port, 200)
            if packet is None:
                if active_processes(job) == 0 and zero_packets >= 1:
                    break
                continue
            total += 1
            if packet["bytes"] == JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO:
                zero_packets += 1
            elif packet["bytes"] in (JOB_OBJECT_MSG_EXIT_PROCESS,
                                     JOB_OBJECT_MSG_ABNORMAL_EXIT_PROCESS):
                exit_packets += 1
            else:
                other_packets += 1
            if active_processes(job) == 0 and zero_packets >= 1:
                break
        counts[str(size)] = {"exit_packets": exit_packets,
                             "zero_packets": zero_packets,
                             "other_packets": other_packets,
                             "total_dequeued": total}
        kernel32.CloseHandle(port)
        kernel32.CloseHandle(job)
        for member in members:
            member.wait()
    return counts


def never_dies_trial():
    job, port, _ok = make_associated_job()
    member = spawn_sleeper(30)
    kernel32.AssignProcessToJobObject(job, int(member._handle))
    start = time.perf_counter()
    empty, _zero, _total = prove_empty(port, job)
    elapsed_ms = (time.perf_counter() - start) * 1000
    kernel32.TerminateJobObject(job, 1)
    kernel32.CloseHandle(port)
    kernel32.CloseHandle(job)
    member.wait()
    return {"empty": empty, "elapsed_ms": elapsed_ms,
            "remainder_ms": max(0.0, BUDGET_MS - elapsed_ms)}


def _poll_verdict(job):
    deadline = time.perf_counter() + BUDGET_MS / 1000.0
    while active_processes(job) != 0 \
            and time.perf_counter() < deadline:
        time.sleep(0.010)
    return active_processes(job) == 0


def _fallback_point(point, attempt):
    """One fallback point, decided from two independent
    observations: (1) the fallback verdict — association is
    attempted on a fresh job and fails, the member is
    terminated, and the poll proof runs on THAT job (the
    behaviour with the port absent); (2) the poll verdict — a
    separate poll run over a separately created identical job
    state (a second fresh job, a second member of the same
    lifetime, terminated the same way). Each observation is
    identified by its own job handle and member pid."""
    job, member, failed = attempt()
    kernel32.TerminateJobObject(job, 1)
    fallback_verdict = _poll_verdict(job)
    fallback_obs = f"job@{job}/member-pid-{member.pid}"
    kernel32.CloseHandle(job)
    member.wait()
    job2 = kernel32.CreateJobObjectW(None, None)
    member2 = spawn_sleeper(0.2)
    kernel32.AssignProcessToJobObject(job2, int(member2._handle))
    kernel32.TerminateJobObject(job2, 1)
    poll_verdict = _poll_verdict(job2)
    poll_obs = f"job@{job2}/member-pid-{member2.pid}"
    kernel32.CloseHandle(job2)
    member2.wait()
    return {"point": point,
            "association_failed": bool(failed),
            "fallback_verdict": fallback_verdict,
            "fallback_obs_id": fallback_obs,
            "poll_verdict": poll_verdict,
            "poll_obs_id": poll_obs}


def fallback_points():
    def attempt_null_port():
        job = kernel32.CreateJobObjectW(None, None)
        member = spawn_sleeper(0.2)
        kernel32.AssignProcessToJobObject(job, int(member._handle))
        return job, member, not associate(job, None)

    def attempt_invalid_job():
        port = kernel32.CreateIoCompletionPort(
            ctypes.c_void_p(-1), None, 0, 1)
        assoc = AssociateCompletionPort(0, port)
        failed = not kernel32.SetInformationJobObject(
            ctypes.c_void_p(0x7FFFFFFF),
            JobObjectAssociateCompletionPortInformation,
            ctypes.byref(assoc), ctypes.sizeof(assoc))
        kernel32.CloseHandle(port)
        job = kernel32.CreateJobObjectW(None, None)
        member = spawn_sleeper(0.2)
        kernel32.AssignProcessToJobObject(job, int(member._handle))
        return job, member, failed

    def attempt_closed_port():
        job = kernel32.CreateJobObjectW(None, None)
        port = kernel32.CreateIoCompletionPort(
            ctypes.c_void_p(-1), None, 0, 1)
        kernel32.CloseHandle(port)
        member = spawn_sleeper(0.2)
        kernel32.AssignProcessToJobObject(job, int(member._handle))
        return job, member, not associate(job, port)

    return [_fallback_point("null-port", attempt_null_port),
            _fallback_point("invalid-job", attempt_invalid_job),
            _fallback_point("closed-port", attempt_closed_port)]


def teardown_cycle(port_holder):
    """One full teardown of an associated port; returns the
    operations actually performed, in order, each with the
    confirmation its own call's return value (or a confirming
    query) produced. An operation whose call fails is recorded
    unconfirmed — it is performed, but it does not count."""
    ops = []
    job = kernel32.CreateJobObjectW(None, None)
    port = kernel32.CreateIoCompletionPort(ctypes.c_void_p(-1),
                                           None, 0, 1)
    ops.append({"op": "create",
                "confirmed": bool(job) and bool(port)})
    ops.append({"op": "associate",
                "confirmed": associate(job, port)})
    disassoc = AssociateCompletionPort(0, None)
    disassoc_ok = bool(kernel32.SetInformationJobObject(
        job, JobObjectAssociateCompletionPortInformation,
        ctypes.byref(disassoc), ctypes.sizeof(disassoc)))
    # Confirmation by query: the job's current association must
    # now report no port.
    current = AssociateCompletionPort(0, None)
    queried = bool(kernel32.QueryInformationJobObject(
        job, JobObjectAssociateCompletionPortInformation,
        ctypes.byref(current), ctypes.sizeof(current), None))
    ops.append({"op": "disassociate",
                "confirmed": bool(disassoc_ok and queried
                                  and not current.CompletionPort)})
    # Drain check: confirmed when no packet remains queued.
    ops.append({"op": "drain-check",
                "confirmed": dequeue(port, 0) is None})
    ops.append({"op": "close",
                "confirmed": bool(kernel32.CloseHandle(port))})
    port_holder.append((job, port))
    return ops


def port_close_accounting():
    count = ctypes.c_uint32()
    kernel32.GetProcessHandleCount(
        kernel32.GetCurrentProcess(), ctypes.byref(count))
    before = count.value
    first_ops = None
    holder = []
    last_job = None
    for i in range(10000):
        holder.clear()
        ops = teardown_cycle(holder)
        if i == 0:
            first_ops = ops
        last_job = holder[0][0]
        kernel32.CloseHandle(last_job)
    # Consumers after disassociation: a member terminated in the
    # last (already disassociated) job must deliver no packet.
    member = spawn_sleeper(0.2)
    job, port, _ok = make_associated_job()
    kernel32.AssignProcessToJobObject(job, int(member._handle))
    disassoc = AssociateCompletionPort(0, None)
    kernel32.SetInformationJobObject(
        job, JobObjectAssociateCompletionPortInformation,
        ctypes.byref(disassoc), ctypes.sizeof(disassoc))
    kernel32.TerminateJobObject(job, 1)
    member.wait()
    consumers = 0
    deadline = time.perf_counter() + 0.5
    while time.perf_counter() < deadline:
        if dequeue(port, 50) is not None:
            consumers += 1
    kernel32.CloseHandle(port)
    kernel32.CloseHandle(job)
    kernel32.GetProcessHandleCount(
        kernel32.GetCurrentProcess(), ctypes.byref(count))
    leaks = count.value - before
    confirmed_first = sum(1 for op in first_ops if op["confirmed"])
    return {"ops": first_ops, "ops_per_cycle": confirmed_first,
            "cycles": 10000, "leaks": leaks,
            "consumers_after_close": consumers}


class SecurityAttributes(ctypes.Structure):
    _fields_ = [("nLength", ctypes.c_uint32),
                ("lpSecurityDescriptor", ctypes.c_void_p),
                ("bInheritHandle", ctypes.c_int)]


def _spawn_handle_checker(handle_value):
    """Spawn the checking child with handle inheritance enabled
    and the given handle explicitly offered through a
    STARTUPINFOEX handle list — the inheritance path a guarded
    child is created through. Returns the child's report:
    "usable" when the handle names a usable handle in the
    child."""
    startupinfo = subprocess.STARTUPINFO()
    try:
        startupinfo.lpAttributeList = {
            "handle_list": [int(handle_value)]}
    except (AttributeError, TypeError):
        pass
    proc = subprocess.run(
        [sys.executable, os.path.abspath(__file__), "--child",
         str(handle_value)], capture_output=True, text=True,
        timeout=120, close_fds=False, startupinfo=startupinfo)
    return proc.stdout.strip()


def non_inheritance_trial():
    _job, port, _ok = make_associated_job()
    # The port is created non-inheritable; make that state
    # explicit and read it back before the spawn, so the record
    # states the creation attributes the trial used.
    kernel32.SetHandleInformation(ctypes.c_void_p(port),
                                  HANDLE_FLAG_INHERIT, 0)
    flags = ctypes.c_uint32()
    read = kernel32.GetHandleInformation(
        ctypes.c_void_p(port), ctypes.byref(flags))
    inherit_flag = bool(read) and bool(flags.value & HANDLE_FLAG_INHERIT)
    # Positive control: a deliberately inheritable event handle,
    # offered through the very same spawn, must be reported
    # inherited — otherwise the trial cannot see inheritance.
    attrs = SecurityAttributes(ctypes.sizeof(SecurityAttributes),
                               None, 1)
    control = kernel32.CreateEventW(ctypes.byref(attrs), True,
                                    False, None)
    control_detected = _spawn_handle_checker(control) == "usable"
    kernel32.CloseHandle(control)
    inherited = _spawn_handle_checker(port) == "usable"
    kernel32.CloseHandle(port)
    kernel32.CloseHandle(_job)
    return {"port_inherited": inherited,
            "inheritance_enabled": True,
            "port_inherit_flag": inherit_flag,
            "control_inheritable_detected": bool(control_detected),
            "spawn_mechanism": "handle inheritance enabled "
                               "(close_fds=False), port handle "
                               "offered through a STARTUPINFOEX "
                               "handle list; port created "
                               "non-inheritable, inherit flag "
                               "cleared and read back"}


def verdict_from_query(packet, job):
    """The verdict function under test. The packet's fields are
    in scope — this code path could read them — and the verdict
    is the authoritative job query's at the decision instant."""
    _ = packet
    return active_processes(job) == 0


def verdict_from_packet(packet, job):
    """Negative-control variant (--sensitive): the verdict reads
    the packet's claim. The driver must refute a run made with
    this function."""
    _ = job
    return packet["bytes"] == JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO


# Packet shapes the port accepts: (bytes transferred, completion
# key, overlapped value). The bytes field carries the job message
# types, including the zero message claiming emptiness.
FORGE_SHAPES = [
    (JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO, 0, 0),
    (0, 7, 4096),
    (JOB_OBJECT_MSG_EXIT_PROCESS, 0, 0),
    (1, 0xDEAD, 8192),
    (0xFFFFFFFF, 0xFFFF, 0),
    (JOB_OBJECT_MSG_ABNORMAL_EXIT_PROCESS, 3, 16384),
]


def forge_child_main(port_value):
    port = int(port_value)
    for nbytes, key, overlapped in FORGE_SHAPES:
        kernel32.PostQueuedCompletionStatus(
            ctypes.c_void_p(port), nbytes, key,
            ctypes.c_void_p(overlapped) if overlapped else None)
    return 0


def forged_trial(sensitive=False):
    verdict_fn = verdict_from_packet if sensitive \
        else verdict_from_query
    job, port, _ok = make_associated_job()
    member = spawn_sleeper(30)
    kernel32.AssignProcessToJobObject(job, int(member._handle))
    # The forging process is a separate child that inherits the
    # port handle and posts the shapes; the deciding process
    # never posts to its own port.
    kernel32.SetHandleInformation(ctypes.c_void_p(port),
                                  HANDLE_FLAG_INHERIT,
                                  HANDLE_FLAG_INHERIT)
    trials = []
    for phase in ("alive", "empty"):
        if phase == "empty":
            kernel32.TerminateJobObject(job, 1)
            deadline = time.perf_counter() + BUDGET_MS / 1000.0
            while active_processes(job) != 0 \
                    and time.perf_counter() < deadline:
                time.sleep(0.010)
        # Drain whatever the job itself queued (new-process and
        # exit packets), so every packet dequeued below is one
        # the forging child posted.
        while dequeue(port, 100) is not None:
            pass
        child = subprocess.Popen(
            [sys.executable, os.path.abspath(__file__), "--forge",
             str(port)], stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL, close_fds=False)
        child.wait()
        for _ in FORGE_SHAPES:
            packet = dequeue(port, 1000)
            if packet is None:
                continue
            trials.append({
                "poster_pid": child.pid,
                "packet_bytes": packet["bytes"],
                "packet_key": packet["key"],
                "packet_overlapped": packet["overlapped"],
                "claim_empty": packet["bytes"]
                == JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO,
                "query_active": active_processes(job),
                "verdict_empty": verdict_fn(packet, job)})
    kernel32.CloseHandle(port)
    kernel32.CloseHandle(job)
    member.wait()
    return {"probe_pid": os.getpid(), "trials": trials}


def child_main(handle_text):
    handle = int(handle_text)
    flags = ctypes.c_uint32()
    usable = kernel32.GetHandleInformation(
        ctypes.c_void_p(handle), ctypes.byref(flags)) != 0
    print("usable" if usable else "unusable")
    return 0


def main():
    if len(sys.argv) > 2 and sys.argv[1] == "--child":
        return child_main(sys.argv[2])
    if len(sys.argv) > 2 and sys.argv[1] == "--forge":
        return forge_child_main(sys.argv[2])
    sensitive = "--sensitive" in sys.argv
    print(json.dumps({
        "zero_packet": {"trials": zero_packet_trials()},
        "composition": {"dequeue_counts": composition_counts()},
        "never_dies": never_dies_trial(),
        "fallback": {"points": fallback_points()},
        "port_close": port_close_accounting(),
        "non_inheritance": non_inheritance_trial(),
        "forged": forged_trial(sensitive),
    }))
    return 0


if __name__ == "__main__":
    sys.exit(main())
