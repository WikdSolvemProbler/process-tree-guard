# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Windows worker checkpoints for the observed-load campaign."""

import ctypes as C
from ctypes import wintypes as W
from contextlib import ExitStack
from functools import lru_cache
import hashlib
import json
import os
from queue import Queue
import re
from threading import Event, Thread
import time


NS = 1_000_000_000
BARRIER_NS = NS
GROUP_NS = 220 * NS
RESERVE_NS = 5 * NS
FRAME_BYTES = 8192
EMPTY_JOB = {"active_before": 0, "terminated": False, "active_after": 0}
PIPE_NAME = re.compile(r"\\\\\.\\pipe\\[A-Za-z0-9_.-]+", re.ASCII)
HEX = re.compile(r"[a-f0-9]{64}")


class BarrierError(RuntimeError):
    pass


def require(ok, message):
    if not ok:
        raise BarrierError(message)


def require_admission(prior, requested, admitted, during_deadline):
    require(all(type(value) is int for value in (prior, requested, admitted, during_deadline))
            and 0 <= prior <= requested <= admitted <= requested + BARRIER_NS
            and requested <= prior + GROUP_NS, "invalid barrier request or group deadline")
    require(admitted + GROUP_NS + RESERVE_NS <= during_deadline,
            "insufficient during budget for group and cleanup reserve")


def barrier_occupation_ns(authorizations, ack_settled_ns):
    require(type(ack_settled_ns) is list and len(authorizations) == len(ack_settled_ns),
            "missing native acknowledgment completion")
    occupied = 0
    for row, settled in zip(authorizations, ack_settled_ns):
        requested, authorized = row["requested_ns"], row["authorized_ns"]
        require(type(settled) is int and requested <= authorized <= settled <= requested + BARRIER_NS,
                "invalid native acknowledgment completion")
        occupied += settled - requested
    return occupied


def wait_ms(deadline):
    remaining = deadline - time.perf_counter_ns()
    return min(0xFFFFFFFE, max(0, (remaining + 999_999) // 1_000_000))


def packed(row):
    return json.dumps(row, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def append_row(stream, row):
    data = packed(row)
    stream.write(data + b"\n")
    stream.flush()
    os.fsync(stream.fileno())
    return hashlib.sha256(data + b"\n").hexdigest()


class SecurityAttributes(C.Structure):
    _fields_ = [
        ("length", W.DWORD), ("descriptor", C.c_void_p), ("inherit", W.BOOL),
    ]


@lru_cache(maxsize=1)
def native():
    import _winapi

    kernel = C.WinDLL("kernel32", use_last_error=True)
    security = C.WinDLL("advapi32", use_last_error=True)
    pointer = C.c_void_p
    declarations = (
        (kernel, "CreateNamedPipeW", (W.LPCWSTR, W.DWORD, W.DWORD, W.DWORD, W.DWORD,
                                     W.DWORD, W.DWORD, C.POINTER(SecurityAttributes)), W.HANDLE),
        (kernel, "GetNamedPipeClientProcessId", (W.HANDLE, C.POINTER(W.ULONG)), W.BOOL),
        (kernel, "GetNamedPipeServerProcessId", (W.HANDLE, C.POINTER(W.ULONG)), W.BOOL),
        (kernel, "GetProcessId", (W.HANDLE,), W.DWORD),
        (kernel, "GetProcessTimes", (W.HANDLE, *([C.POINTER(W.FILETIME)] * 4)), W.BOOL),
        (kernel, "IsProcessInJob", (W.HANDLE, W.HANDLE, C.POINTER(W.BOOL)), W.BOOL),
        (kernel, "CreateEventW", (pointer, W.BOOL, W.BOOL, W.LPCWSTR), W.HANDLE),
        (kernel, "SetEvent", (W.HANDLE,), W.BOOL),
        (kernel, "LocalFree", (pointer,), pointer),
        (security, "OpenProcessToken", (W.HANDLE, W.DWORD, C.POINTER(W.HANDLE)), W.BOOL),
        (security, "GetTokenInformation", (W.HANDLE, C.c_int, pointer, W.DWORD,
                                           C.POINTER(W.DWORD)), W.BOOL),
        (security, "ConvertSidToStringSidW", (pointer, C.POINTER(W.LPWSTR)), W.BOOL),
        (security, "ConvertStringSecurityDescriptorToSecurityDescriptorW",
         (W.LPCWSTR, W.DWORD, C.POINTER(pointer), C.POINTER(W.ULONG)), W.BOOL),
    )
    for library, name, args, result in declarations:
        call = getattr(library, name)
        call.argtypes, call.restype = args, result
    return _winapi, kernel, security


def checked(value):
    if not value:
        raise C.WinError(C.get_last_error())
    return value


def current_user_pipe(name):
    win, kernel, security = native()
    token = W.HANDLE()
    checked(security.OpenProcessToken(win.GetCurrentProcess(), 0x0008, C.byref(token)))
    try:
        length = W.DWORD()
        security.GetTokenInformation(token, 1, None, 0, C.byref(length))
        checked(length.value)
        user = C.create_string_buffer(length.value)
        checked(security.GetTokenInformation(token, 1, user, length, C.byref(length)))
        sid = W.LPWSTR()
        checked(security.ConvertSidToStringSidW(C.c_void_p.from_buffer(user), C.byref(sid)))
        try:
            # One protected DACL entry, for the actual controller's token SID.
            descriptor = C.c_void_p()
            checked(security.ConvertStringSecurityDescriptorToSecurityDescriptorW(
                f"D:P(A;;GA;;;{sid.value})", 1, C.byref(descriptor), None,
            ))
            try:
                attrs = SecurityAttributes(C.sizeof(SecurityAttributes), descriptor, False)
                access = 0x00000003 | win.FILE_FLAG_OVERLAPPED | 0x00080000
                mode = win.PIPE_TYPE_MESSAGE | win.PIPE_READMODE_MESSAGE | 0x00000008
                handle = kernel.CreateNamedPipeW(
                    name, access, mode, 1, FRAME_BYTES, FRAME_BYTES, 0, C.byref(attrs),
                )
                return checked(handle if handle != C.c_void_p(-1).value else None)
            finally:
                kernel.LocalFree(descriptor)
        finally:
            kernel.LocalFree(C.cast(sid, C.c_void_p))
    finally:
        win.CloseHandle(token.value)


def validate_context(name, nonce, manifest_sha):
    require(isinstance(name, str) and PIPE_NAME.fullmatch(name), "not a local pipe name")
    require(isinstance(nonce, str) and HEX.fullmatch(nonce), "invalid barrier nonce")
    require(isinstance(manifest_sha, str) and HEX.fullmatch(manifest_sha),
            "invalid barrier manifest digest")


def process_in_job(handle, job_handle):
    win, kernel, _ = native()
    require(win.WaitForSingleObject(handle, 0) == win.WAIT_TIMEOUT,
            "the held worker process is not live")
    member = W.BOOL()
    checked(kernel.IsProcessInJob(handle, job_handle, C.byref(member)))
    require(member.value, "the held worker process is outside the campaign Job")
    times = [W.FILETIME() for _ in range(4)]
    checked(kernel.GetProcessTimes(handle, *(C.byref(item) for item in times)))
    created = times[0].dwHighDateTime << 32 | times[0].dwLowDateTime
    require(created > 0, "the held worker has no creation time")
    return {"pid": int(checked(kernel.GetProcessId(handle))), "creation_filetime": created}


class ProcessIdList(C.Structure):
    _fields_ = [
        ("assigned", W.DWORD), ("listed", W.DWORD), ("pids", C.c_size_t * 2),
    ]


def describe_pricing_membership(job, worker, identity):
    """Witness the stock pricing launch's phase Job after its timed return."""
    from windows_slot_job import QueryInformationJobObject

    before = process_in_job(worker._handle, job._handle)
    data = ProcessIdList()
    checked(QueryInformationJobObject(job._handle, 3, C.byref(data), C.sizeof(data), None))
    after = process_in_job(worker._handle, job._handle)
    pids = list(data.pids[:data.listed])
    return {
        "before": before, "after": after,
        "assigned": int(data.assigned), "listed": int(data.listed), "pids": pids,
        "accepted": before == after == identity and data.assigned == data.listed == 1
                    and pids == [identity["pid"]],
    }


class Pipe:
    def __init__(self, handle, stop=None):
        self.win, self.kernel, _ = native()
        self.handle, self.stop = handle, stop
        self.unsettled = []

    def _finish(self, operation, deadline, *, pending=True):
        handles = [operation.event]
        if self.stop is not None:
            handles.append(self.stop)
        try:
            if pending:
                outcome = self.win.WaitForMultipleObjects(handles, False, wait_ms(deadline))
                require(outcome == self.win.WAIT_OBJECT_0,
                        f"pipe operation did not settle before deadline ({outcome})")
            count, status = operation.GetOverlappedResult(False)
            require(status == 0, f"pipe I/O completion error {status}")
            settled = time.perf_counter_ns()
            require(settled <= deadline, "pipe I/O exceeded deadline")
            return count, settled
        except BaseException:
            # Keep the native OVERLAPPED and buffer alive through cancellation.
            self.unsettled.append(operation)
            try:
                operation.cancel()
            except OSError:
                # Keep the original deadline or communication failure.
                pass
            raise

    def connect(self, deadline):
        self._finish(self.win.ConnectNamedPipe(self.handle, overlapped=True), deadline)

    def read(self, deadline):
        operation, status = self.win.ReadFile(self.handle, FRAME_BYTES, overlapped=True)
        count, _ = self._finish(operation, deadline, pending=status == self.win.ERROR_IO_PENDING)
        require(count > 0, "empty pipe frame")
        try:
            row = json.loads(operation.getbuffer(), object_pairs_hook=no_duplicates)
        except (TypeError, ValueError) as error:
            raise BarrierError("invalid pipe frame") from error
        require(type(row) is dict, "pipe frame must be an object")
        return row

    def write(self, frame, deadline):
        blob = packed(frame)
        require(0 < len(blob) <= FRAME_BYTES, "oversize pipe frame")
        operation, status = self.win.WriteFile(self.handle, blob, overlapped=True)
        count, settled = self._finish(operation, deadline, pending=status == self.win.ERROR_IO_PENDING)
        require(count == len(blob), "short pipe write")
        return settled

    def close(self):
        if self.handle is not None:
            self.win.CloseHandle(self.handle)
            self.handle = None


def no_duplicates(fields):
    result = {}
    for key, value in fields:
        require(key not in result, f"duplicate pipe field {key}")
        result[key] = value
    return result


class BarrierClient:
    def __init__(self, pipe_name, nonce, manifest_sha):
        validate_context(pipe_name, nonce, manifest_sha)
        self.name = pipe_name
        self.context = {"nonce": nonce, "manifest_sha256": manifest_sha}
        self.pipe = None
        self.last_receipt_sha256 = None

    def __enter__(self):
        win, kernel, _ = native()
        handle = win.CreateFile(self.name, win.GENERIC_READ | win.GENERIC_WRITE,
                                0, None, win.OPEN_EXISTING, win.FILE_FLAG_OVERLAPPED, None)
        self.pipe = Pipe(handle)
        try:
            win.SetNamedPipeHandleState(handle, win.PIPE_READMODE_MESSAGE, None, None)
            pid = W.ULONG()
            checked(kernel.GetNamedPipeServerProcessId(handle, C.byref(pid)))
            require(pid.value == os.getppid(), "barrier server is not the worker parent")
        except BaseException:
            self.pipe.close()
            raise
        return self

    def checkpoint(self, group, label, settled_through):
        asked = time.perf_counter_ns()
        deadline = asked + BARRIER_NS
        request = {**self.context, "group": group, "label": label,
                   "settled_through": settled_through, "requested_ns": asked}
        self.pipe.write(request, deadline)
        ack = self.pipe.read(deadline)
        released = time.perf_counter_ns()
        require(set(ack) == set(request) | {"authorized_ns", "receipt_sha256"}
                and all(ack[key] == value for key, value in request.items())
                and type(ack["authorized_ns"]) is int
                and asked <= ack["authorized_ns"] <= released <= deadline
                and isinstance(ack["receipt_sha256"], str) and HEX.fullmatch(ack["receipt_sha256"]),
                "barrier authorization does not bind the request or deadline")
        self.last_receipt_sha256 = ack['receipt_sha256']
        return released

    def pricing_settlement(self, group, index):
        asked = time.perf_counter_ns()
        deadline = asked + BARRIER_NS
        request = {**self.context, "kind": "pricing-settlement", "group": group,
                   "index": index, "requested_ns": asked}
        self.pipe.write(request, deadline)
        ack = self.pipe.read(deadline)
        received = time.perf_counter_ns()
        require(set(ack) == set(request) | {"authorized_ns", "receipt_sha256"}
                and all(ack[key] == value for key, value in request.items())
                and type(ack["authorized_ns"]) is int
                and asked <= ack["authorized_ns"] <= received <= deadline
                and isinstance(ack["receipt_sha256"], str) and HEX.fullmatch(ack["receipt_sha256"]),
                "pricing settlement does not bind its native witness")
        return ack["receipt_sha256"]

    def __exit__(self, *_):
        self.pipe.close()


class BarrierServer:
    """Own one invasive thread for census requests before, during and after the worker."""

    def __init__(self, pipe_name, nonce, manifest_sha, groups, capture_path, on_group):
        validate_context(pipe_name, nonce, manifest_sha)
        self.name = pipe_name
        self.context = {"nonce": nonce, "manifest_sha256": manifest_sha}
        self.groups = tuple(groups)
        require(bool(self.groups) and self.groups[-1][1] == "tail", "missing tail barrier")
        self.path, self.on_group = capture_path, on_group
        self.tasks = Queue()
        self.ready = Event()
        self.error = None
        self.authorizations = []
        self.ack_settled_ns = []
        self.pricing_authorizations = []
        self.pricing_ack_settled_ns = []
        self.thread = Thread(target=self._run, name="laptop-invasive-observer")
        self.win, self.kernel, _ = native()
        self.stop_handle = checked(self.kernel.CreateEventW(None, True, False, None))
        try:
            self.failed_handle = checked(self.kernel.CreateEventW(None, True, False, None))
            try:
                self.pipe = Pipe(current_user_pipe(pipe_name), self.stop_handle)
            except BaseException:
                self.win.CloseHandle(self.failed_handle)
                raise
        except BaseException:
            self.win.CloseHandle(self.stop_handle)
            raise

    def __enter__(self):
        self.thread.start()
        if not self.ready.wait(BARRIER_NS / NS):
            self.__exit__()
            raise BarrierError("invasive observer did not start")
        return self

    def _run(self):
        self.ready.set()
        while True:
            task = self.tasks.get()
            if task is None:
                return
            action, done = task
            try:
                action()
            except BaseException as error:
                self.error = error
                checked(self.kernel.SetEvent(self.failed_handle))
                return
            finally:
                done.set()

    def submit(self, action):
        self.check()
        done = Event()
        self.tasks.put((action, done))
        return done

    def check(self):
        if self.error is not None:
            raise BarrierError(f"invasive observer failed: {self.error}") from self.error

    def bind_worker(self, job, worker, dispatch_ns, during_deadline):
        identity = process_in_job(worker._handle, job._handle)
        require(identity["pid"] == worker.pid, "worker handle differs from launch PID")
        return self.submit(lambda: self._serve(job, worker, identity, dispatch_ns, during_deadline))

    def _serve(self, job, worker, identity, dispatch_ns, during_deadline):
        prior = dispatch_ns
        self.pipe.connect(min(prior + GROUP_NS, during_deadline - RESERVE_NS))
        pid = W.ULONG()
        checked(self.kernel.GetNamedPipeClientProcessId(self.pipe.handle, C.byref(pid)))
        require(pid.value == identity["pid"], "pipe client differs from the held worker")
        prior_scan = None
        with ExitStack() as files:
            file = files.enter_context(self.path.open("xb"))
            pricing_file = None
            if any(label.startswith("pricing:") for _, label, _ in self.groups):
                pricing_file = files.enter_context(self.path.with_name("pricing-membership.jsonl").open("xb"))
            for ordinal, label, settled_through in self.groups:
                request = self.pipe.read(min(prior + GROUP_NS, during_deadline - RESERVE_NS))
                asked = request.get("requested_ns")
                now = time.perf_counter_ns()
                expected = {**self.context, "group": ordinal, "label": label,
                            "settled_through": settled_through, "requested_ns": asked}
                require(request == expected and type(request["group"]) is int
                        and type(request["settled_through"]) is int, "invalid barrier request fields")
                require_admission(prior, asked, now, during_deadline)
                require(process_in_job(worker._handle, job._handle) == identity,
                        "worker changed while requesting a barrier")
                census = self.on_group(job, identity, ordinal, label, asked)
                scan_start = census["begin_ns"]
                require(prior_scan is None or 0 < scan_start - prior_scan <= GROUP_NS,
                        "during census gap exceeded 220 seconds")
                authorized = time.perf_counter_ns()
                receipt = {**request, "admitted_ns": now, "authorized_ns": authorized,
                           "identity": identity, "census": census, "client_pid": pid.value}
                digest = append_row(file, receipt)
                settled = self.pipe.write({**request, "authorized_ns": authorized,
                                           "receipt_sha256": digest}, asked + BARRIER_NS)
                self.authorizations.append(receipt)
                # The existing phase decision persists these after all groups. No
                # receipt write follows an acknowledgment into a timed slot.
                self.ack_settled_ns.append(settled)
                prior, prior_scan = asked, scan_start
                if label.startswith("pricing:"):
                    preceding_authorization = authorized
                    for index in (settled_through + 1, settled_through + 2):
                        preceding_authorization = self._pricing_settlement(
                            pricing_file, job, worker, identity, ordinal, index, preceding_authorization,
                            min(prior + GROUP_NS, during_deadline - RESERVE_NS))

    def _pricing_settlement(self, stream, job, worker, identity, group, index, current, end):
        request = self.pipe.read(end)
        asked = request.get("requested_ns")
        admitted = time.perf_counter_ns()
        expected = {**self.context, "kind": "pricing-settlement", "group": group,
                    "index": index, "requested_ns": asked}
        require(request == expected and all(type(request[key]) is int for key in ("group", "index"))
                and type(asked) is int and current <= asked <= admitted <= min(asked + BARRIER_NS, end),
                "invalid pricing settlement request")
        try:
            witness = describe_pricing_membership(job, worker, identity)
        except (OSError, BarrierError) as error:
            witness = {"accepted": False, "error": f"{type(error).__name__}: {error}"}
        authorized = time.perf_counter_ns()
        receipt = {**request, "authorized_ns": authorized, "witness": witness}
        digest = append_row(stream, receipt)
        require(witness["accepted"] is True, "pricing phase Job does not contain only the exact worker")
        ack_settled = self.pipe.write({**request, "authorized_ns": authorized, "receipt_sha256": digest},
                                      min(asked + BARRIER_NS, end))
        self.pricing_authorizations.append(receipt)
        self.pricing_ack_settled_ns.append(ack_settled)
        return authorized

    def __exit__(self, *_):
        checked(self.kernel.SetEvent(self.stop_handle))
        self.tasks.put(None)
        self.thread.join(RESERVE_NS / NS)
        require(not self.thread.is_alive(), "invasive observer did not join")
        self.pipe.close()
        self.win.CloseHandle(self.failed_handle)
        self.win.CloseHandle(self.stop_handle)

