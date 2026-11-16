# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Windows payload witness for a finite, externally guarded benchmark slot.

make_observers(N, S, helper_path, marker, write_record) returns observe and
after_wait callbacks. The caller holds original guard and Job handles, supplies an
armed slot deadline, calls observe before the first blocking root wait, and calls
after_wait immediately after that root endpoint while the original payload
handle is retained. The caller closes retained process handles in its existing
cleanup path. This module launches no process and supplies no timeout guard.

The marker reader opens once with GENERIC_READ and share READ|WRITE|DELETE,
reads at most 11 bytes, and closes exactly once. Access failures propagate
without retry; a close-only failure also propagates. The PID and original
payload identity, image, Job, liveness, and deadline checks remain mandatory.
"""
import ctypes as C
from ctypes import wintypes as W
import os
from pathlib import Path
import time
import traceback
import _winapi

WAIT_LIVE = 258
MARKER_GENERIC_READ = 0x80000000
MARKER_SHARE_ALL = 0x00000007
MARKER_OPEN_EXISTING = 3
MARKER_ATTRIBUTE_NORMAL = 0x00000080
MARKER_INVALID_HANDLE = C.c_void_p(-1).value
_marker_kernel = C.WinDLL("kernel32", use_last_error=True)
_marker_create = _marker_kernel.CreateFileW
_marker_create.argtypes = [W.LPCWSTR, W.DWORD, W.DWORD, C.c_void_p, W.DWORD, W.DWORD, W.HANDLE]
_marker_create.restype = W.HANDLE
_marker_read = _marker_kernel.ReadFile
_marker_read.argtypes = [W.HANDLE, C.c_void_p, W.DWORD, C.POINTER(W.DWORD), C.c_void_p]
_marker_read.restype = W.BOOL
_marker_close = _marker_kernel.CloseHandle
_marker_close.argtypes = [W.HANDLE]
_marker_close.restype = W.BOOL

def require(ok, text):
    if not ok:
        raise RuntimeError(text)

def make_observers(N, S, helper_path, marker, write_record):
    def observe(root, job, deadline, extra_handles, row):
        def record_marker_error(stage, error):
            # Best effort only. This witness must never replace the original OSError.
            try:
                try:
                    trace = "".join(traceback.format_exception(type(error), error, error.__traceback__, limit=4))[-2048:]
                except BaseException:
                    trace = "TRACEBACK_UNAVAILABLE"
                row["marker_access_failure"] = dict(stage=stage, exception_type=type(error).__name__,
                    errno=getattr(error, "errno", None), winerror=getattr(error, "winerror", None),
                    filename=str(getattr(error, "filename", None))[:512],
                    filename2=str(getattr(error, "filename2", None))[:512], traceback=trace)
            except BaseException:
                pass
        cutoff_ns = row["armed_deadline_ns"]
        require(time.perf_counter_ns() < cutoff_ns, "original slot exhausted before acquisition")
        while True:
            try:
                marker_present = marker.is_file()
            except OSError as error:
                record_marker_error("marker.is_file", error)
                raise
            if marker_present:
                break
            probe = N.receipt("WaitForSingleObject", lambda: N.WaitForSingleObject(root, 0), process_handle=int(root), timeout_ms=0)
            row["root_while_waiting_marker"] = probe
            require(probe["native_return"] == WAIT_LIVE, "root ended before live payload acquisition")
            remaining = min((cutoff_ns-time.perf_counter_ns())/1e9, deadline-time.monotonic())
            require(remaining > 0, "marker not acquired inside original slot")
            time.sleep(min(0.005, remaining))
        C.set_last_error(0)
        marker_handle = _marker_create(str(marker), MARKER_GENERIC_READ, MARKER_SHARE_ALL,
                                       None, MARKER_OPEN_EXISTING, MARKER_ATTRIBUTE_NORMAL, None)
        if marker_handle in (None, MARKER_INVALID_HANDLE):
            native_error = C.get_last_error()
            error = C.WinError(native_error)
            record_marker_error("marker.native_open", error)
            raise error
        primary_error = None
        try:
            buffer = C.create_string_buffer(11)
            count = W.DWORD(0)
            C.set_last_error(0)
            read_ok = int(_marker_read(marker_handle, buffer, 11, C.byref(count), None))
            if not read_ok:
                native_error = C.get_last_error()
                error = C.WinError(native_error)
                record_marker_error("marker.native_read", error)
                raise error
            require(count.value <= 11, "marker native read exceeded fixed buffer")
            raw = buffer.raw[:count.value]
        except BaseException as error:
            primary_error = error
            raise
        finally:
            C.set_last_error(0)
            close_ok = int(_marker_close(marker_handle))
            if not close_ok:
                native_error = C.get_last_error()
                error = C.WinError(native_error)
                if primary_error is not None:
                    try:
                        row["marker_close_failure"] = dict(stage="marker.native_close",
                            native_winerror=native_error, exception_type=type(error).__name__)
                    except BaseException:
                        pass
                else:
                    record_marker_error("marker.native_close", error)
                    raise error
        row["marker_hex"] = raw.hex()
        require(1 <= len(raw) <= 10 and raw.isdigit() and raw[:1] != b"0" and int(raw) <= 0xffffffff, "marker PID not canonical")
        pid = int(raw)
        opened_ns = time.perf_counter_ns()
        child = _winapi.OpenProcess(S.SYNC_QUERY, False, pid)
        extra_handles["payload"] = child
        initial = S.snapshot(child, job)
        root_live = S.snapshot(root, job)
        size = W.DWORD(32768)
        image = C.create_unicode_buffer(size.value)
        S.native_ok(S.image_name(child, 0, image, C.byref(size)))
        acquisition = dict(snapshot=initial, image=image.value, opened_ns=opened_ns,
                           process_handle=int(child), verified_ns=time.perf_counter_ns())
        row["payload_acquisition"] = acquisition
        row["root_at_payload_acquisition"] = root_live
        original = row["suspended"]
        require(initial["pid"] == pid != original["pid"] and initial["creation_filetime"] >= original["creation_filetime"]
                and initial["member"] and initial["native_wait"] == WAIT_LIVE, "original live payload not bound")
        require(root_live["pid"] == original["pid"] and root_live["creation_filetime"] == original["creation_filetime"]
                and root_live["native_wait"] == WAIT_LIVE and root_live["member"], "original root not live at acquisition")
        require(os.path.normcase(os.path.normpath(image.value)) == os.path.normcase(os.path.normpath(str(helper_path))), "unexpected payload image")
        require(time.perf_counter_ns() < cutoff_ns and time.monotonic() < deadline, "live acquisition verification exceeded original slot")
        row["payload_identified_live"] = True

    def after_wait(root, job, extra_handles, row, root_wait, root_end_ns):
        child = extra_handles["payload"]
        first = dict(process_handle=int(child), timeout_ms=0, root_native_wait=root_wait,
                     root_measurement_end_ns=root_end_ns)
        row["first_payload_poll"] = first
        # The original root wait-end timestamp is already taken. This is the
        # next native process observation, before any snapshot or cleanup.
        first["native_return"] = int(N.WaitForSingleObject(child, 0))
        first["last_error"] = C.get_last_error() if first["native_return"] == 0xffffffff else None
        first["observed_ns"] = time.perf_counter_ns()
        first["unassisted_before_deadline"] = (not row.get("watchdog_fired_ns") and not row.get("trusted_cleanup_requested")
                                                and first["observed_ns"] < row["armed_deadline_ns"])
        row["first_physical_observation"] = ({0:"RAW_TERMINAL_AT_SAMPLE", WAIT_LIVE:"RAW_ORIGINAL_PAYLOAD_NONSIGNALED"}
                                              .get(first["native_return"], "INCONCLUSIVE_NATIVE_POLL"))
        write_record(marker.parent / "FIRST-RAW-PAYLOAD-POLL.json", first)
        require(first["native_return"] in (0, WAIT_LIVE), "original payload native first poll failed")
        later = S.snapshot(child, job)
        row["payload_before_cleanup"] = later
        before = row["payload_acquisition"]["snapshot"]
        require((later["pid"], later["creation_filetime"]) == (before["pid"], before["creation_filetime"]), "held original payload identity changed")
    return observe, after_wait

def classify_first_sample(row, trace_valid):
    """Raw LIVE stays raw unless the independent pre-cleanup facts validate it."""
    first = row.get("first_payload_poll", {})
    acq = row.get("payload_acquisition", {})
    original = row.get("suspended", {})
    root = row.get("root_before_cleanup", {})
    exit_query = row.get("exit", {})
    payload = row.get("payload_before_cleanup", {})
    before = acq.get("snapshot", {})
    valid = bool(row.get("payload_identified_live") is True
        and first.get("process_handle") == acq.get("process_handle")
        and first.get("root_native_wait") == 0 and row.get("native_wait") == 0
        and first.get("unassisted_before_deadline") is True
        and first.get("native_return") in (0, WAIT_LIVE)
        and row.get("measurement_end_ns") == first.get("root_measurement_end_ns")
        and root.get("native_wait") == 0 and root.get("exit_code_u32") == 0
        and root.get("pid") == original.get("pid")
        and root.get("creation_filetime") == original.get("creation_filetime")
        and exit_query.get("native_return") == 1 and exit_query.get("exit_code_u32") == 0
        and before.get("member") is True and before.get("native_wait") == WAIT_LIVE
        and payload.get("native_wait") in ((0,) if first.get("native_return") == 0 else (0, WAIT_LIVE))
        and first.get("observed_ns", -1) >= first.get("root_measurement_end_ns", 0)
        and payload.get("pid") == before.get("pid")
        and payload.get("creation_filetime") == before.get("creation_filetime")
        and trace_valid(row))
    row["first_sample_oracles_validated"] = valid
    row["first_sample_contract"] = ("FAIL_ORIGINAL_PAYLOAD_LIVE" if first.get("native_return") == WAIT_LIVE
                                    else "TERMINAL_AT_OBSERVED_SAMPLE_ONLY") if valid else "INCONCLUSIVE"
    return valid
