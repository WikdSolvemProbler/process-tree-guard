"""Draft sampled census of known competing workload images on Windows."""

import _winapi
import ctypes
import json
import os
import hashlib
from ctypes import wintypes
from dataclasses import asdict, dataclass
from image_policy import classify

PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
TH32CS_SNAPPROCESS = 2
ERROR_NO_MORE_FILES = 18
INVALID_HANDLE = ctypes.c_void_p(-1).value
COMPILERS = frozenset({
    "cargo.exe", "rustc.exe", "rustdoc.exe", "cl.exe", "link.exe", "clang.exe",
    "clang-cl.exe", "lld.exe", "lld-link.exe", "rust-lld.exe", "gcc.exe",
    "g++.exe", "cc1.exe", "cc1plus.exe", "msbuild.exe", "cmake.exe", "ninja.exe",
})
WORKLOADS = frozenset({
    "python.exe", "pythonw.exe", "python3.exe", "pytest.exe", "ptg.exe",
    "process-tree-guard.exe", "testkit.exe", "sysprims.exe", "processkit-cli.exe",
    "timeout.exe",
})
GUARD_IMAGE = "process-tree-guard.exe"


def image_digest(image):
    """Read on-disk content while the opened file's native identity is stable."""
    def signature(info):
        if not info.st_ino:
            raise ValueError("image file identity unavailable")
        return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)

    path_before = signature(image.stat())
    with image.open("rb") as stream:
        before = signature(os.fstat(stream.fileno()))
        if path_before[:4] != before[:4]:
            raise ValueError("image path and opened handle differ")
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        after = signature(os.fstat(stream.fileno()))
        path_after = signature(image.stat())
        if after != before or path_after != path_before or path_after[:4] != after[:4]:
            raise ValueError("image file changed during inspection")
        return digest


class ProcessEntry(ctypes.Structure):
    _fields_ = [
        ("size", wintypes.DWORD), ("usage", wintypes.DWORD), ("pid", wintypes.DWORD),
        ("heap", ctypes.c_size_t), ("module", wintypes.DWORD), ("threads", wintypes.DWORD),
        ("parent_pid", wintypes.DWORD), ("base_priority", wintypes.LONG),
        ("flags", wintypes.DWORD), ("name", wintypes.WCHAR * 260),
    ]


@dataclass(frozen=True)
class Entry:
    pid: int
    parent_pid: int
    name: str


def bind(name, args, result):
    call = getattr(kernel32, name)
    call.argtypes = args
    call.restype = result
    return call


kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
create_snapshot = bind("CreateToolhelp32Snapshot", [wintypes.DWORD, wintypes.DWORD], wintypes.HANDLE)
first_process = bind("Process32FirstW", [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)], wintypes.BOOL)
next_process = bind("Process32NextW", [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)], wintypes.BOOL)
is_member = bind("IsProcessInJob", [wintypes.HANDLE, wintypes.HANDLE, ctypes.POINTER(wintypes.BOOL)],
                 wintypes.BOOL)


precise_filetime = bind("GetSystemTimePreciseAsFileTime",
                        [ctypes.POINTER(wintypes.FILETIME)], None)


def entries(*, capture_upper=False):
    handle = create_snapshot(TH32CS_SNAPPROCESS, 0)
    upper = wintypes.FILETIME()
    precise_filetime(ctypes.byref(upper))
    if not handle or handle == INVALID_HANDLE:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        data = ProcessEntry()
        data.size = ctypes.sizeof(data)
        if not first_process(handle, ctypes.byref(data)):
            raise ctypes.WinError(ctypes.get_last_error())
        result = {}
        while True:
            if data.pid in result:
                raise RuntimeError("duplicate PID in process snapshot")
            result[data.pid] = Entry(data.pid, data.parent_pid, data.name.casefold())
            if not next_process(handle, ctypes.byref(data)):
                error = ctypes.get_last_error()
                if error != ERROR_NO_MORE_FILES:
                    raise ctypes.WinError(error)
                return (result, filetime(upper)) if capture_upper else result
    finally:
        _winapi.CloseHandle(handle)


from contextlib import ExitStack, contextmanager
from pathlib import Path
from time import time_ns

SYNCHRONIZE = 0x100000
FILETIME_UNIX_OFFSET = 116444736000000000
P = ctypes.POINTER
process_image = bind("QueryFullProcessImageNameW",
                     [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, P(wintypes.DWORD)],
                     wintypes.BOOL)
process_times = bind("GetProcessTimes", [wintypes.HANDLE] + [P(wintypes.FILETIME)] * 4,
                     wintypes.BOOL)


@contextmanager
def open_exact(pid):
    handle = _winapi.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, False, pid)
    try:
        yield handle
    finally:
        _winapi.CloseHandle(handle)


def filetime(value):
    return value.dwHighDateTime << 32 | value.dwLowDateTime


def identity(handle):
    name = ctypes.create_unicode_buffer(32768)
    size = wintypes.DWORD(len(name))
    if not process_image(handle, 0, name, ctypes.byref(size)):
        raise ctypes.WinError(ctypes.get_last_error())
    created, exited, kernel, user = (wintypes.FILETIME() for _ in range(4))
    if not process_times(handle, ctypes.byref(created), ctypes.byref(exited),
                         ctypes.byref(kernel), ctypes.byref(user)):
        raise ctypes.WinError(ctypes.get_last_error())
    if not filetime(created):
        raise ValueError("process creation time unavailable")
    return Path(name.value), filetime(created)


def is_alive(handle):
    status = _winapi.WaitForSingleObject(handle, 0)
    if status == _winapi.WAIT_TIMEOUT:
        return True
    if status == _winapi.WAIT_OBJECT_0:
        return False
    raise OSError(f"unexpected process wait status {status}")


def snapshot_identity(entry, image, created, upper):
    if not 0 < created <= upper:
        raise ValueError("process creation exceeds precise snapshot upper bound")
    if image.name.casefold() != entry.name:
        raise ValueError("process image name differs from snapshot")


@contextmanager
def enclosing_guard(expected_path):
    """Hold the named ancestor selected by its image and ordered creation times."""
    snapshot = entries()
    current = os.getpid()
    seen = set()
    last_created = time_ns() // 100 + FILETIME_UNIX_OFFSET
    with ExitStack() as stack:
        while current in snapshot and current not in seen:
            seen.add(current)
            entry = snapshot[current]
            handle = stack.enter_context(open_exact(current))
            image, created = identity(handle)
            if not is_alive(handle) or not 0 < created <= last_created \
                    or image.name.casefold() != entry.name:
                raise ValueError("unverified enclosing process chain")
            last_created = created
            if entry.name == GUARD_IMAGE:
                if not image.samefile(expected_path):
                    raise ValueError("enclosing guard is not the pinned image")
                yield current, handle, created
                return
            current = entry.parent_pid
    raise ValueError("pinned enclosing guard was not found")


def inventory(guard, job_handle=None, image_policy=None):
    """Classify sampled image content while each exact process handle is held."""
    if image_policy is None:
        raise ValueError("sealed image policy required")
    veto_sha256 = set(image_policy["veto_sha256"])
    relevant = COMPILERS | WORKLOADS | set(image_policy["pinned_basenames"])
    current = os.getpid()
    snapshot, upper = entries(capture_upper=True)
    guard_pid, guard_handle, guard_created = guard
    if not is_alive(guard_handle) or guard_pid not in snapshot:
        raise ValueError("pinned enclosing guard was lost")
    observed = []
    for entry in snapshot.values():
        if entry.pid == current or (entry.pid != guard_pid and entry.name not in relevant):
            continue
        row = asdict(entry)
        row["snapshot_upper_ticks"] = upper
        if entry.pid == guard_pid:
            row.update(state="pinned_enclosing_guard", created_ticks=guard_created)
        else:
            try:
                with open_exact(entry.pid) as handle:
                    image, created = identity(handle)
                    row["created_ticks"] = created
                    row["image_path"] = str(image)
                    snapshot_identity(entry, image, created, upper)
                    member = False
                    if job_handle is not None:
                        found = wintypes.BOOL()
                        if not is_member(handle, job_handle, ctypes.byref(found)):
                            raise ctypes.WinError(ctypes.get_last_error())
                        member = bool(found.value)
                    row["image_sha256"] = (
                        None if member or entry.name in COMPILERS else image_digest(image)
                    )
                    # Re-query through the retained handle after any file read.
                    if identity(handle) != (image, created):
                        raise ValueError("process identity changed during image inspection")
                    row["alive_after"] = is_alive(handle)
                    row["state"] = classify(
                        entry.name, row["image_sha256"], enclosing=False, member=member,
                        compiler_names=COMPILERS, veto_sha256=veto_sha256,
                    )
            except (OSError, ValueError) as error:
                row.update(state="membership_unverified", error=type(error).__name__,
                           winerror=getattr(error, "winerror", None), reason=str(error))
        observed.append(row)
    return observed


if __name__ == "__main__":
    import sys
    with enclosing_guard(sys.argv[1]) as guard:
        print(json.dumps({"kind": "observation-only", "benchmark_slots": 0,
                          "rows": inventory(guard)}, indent=2))
