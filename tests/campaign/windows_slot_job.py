# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Private containment for one Windows campaign slot."""

import _winapi
import ctypes
import msvcrt
import os
import subprocess
import time
from contextlib import ExitStack
from ctypes import wintypes


JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION = 1
JOB_OBJECT_EXTENDED_LIMIT_INFORMATION = 9
JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000
PROC_THREAD_ATTRIBUTE_HANDLE_LIST = 0x00020002
PROC_THREAD_ATTRIBUTE_JOB_LIST = 0x0002000D
EXTENDED_STARTUPINFO_PRESENT = 0x00080000
CREATE_UNICODE_ENVIRONMENT = 0x00000400
STARTF_USESTDHANDLES = 0x00000100
CLEANUP_EXIT_CODE = 0xE0000071
CLEANUP_TIMEOUT_S = 5.0


class StartupInfo(ctypes.Structure):
    _fields_ = [
        ("size", wintypes.DWORD),
        ("reserved", wintypes.LPWSTR),
        ("desktop", wintypes.LPWSTR),
        ("title", wintypes.LPWSTR),
        ("x", wintypes.DWORD),
        ("y", wintypes.DWORD),
        ("width", wintypes.DWORD),
        ("height", wintypes.DWORD),
        ("columns", wintypes.DWORD),
        ("rows", wintypes.DWORD),
        ("fill", wintypes.DWORD),
        ("flags", wintypes.DWORD),
        ("show", wintypes.WORD),
        ("reserved2_size", wintypes.WORD),
        ("reserved2", ctypes.c_void_p),
        ("stdin", wintypes.HANDLE),
        ("stdout", wintypes.HANDLE),
        ("stderr", wintypes.HANDLE),
    ]


class StartupInfoEx(ctypes.Structure):
    _fields_ = [("info", StartupInfo), ("attributes", ctypes.c_void_p)]


class ProcessInformation(ctypes.Structure):
    _fields_ = [
        ("process", wintypes.HANDLE),
        ("thread", wintypes.HANDLE),
        ("pid", wintypes.DWORD),
        ("tid", wintypes.DWORD),
    ]


class BasicLimitInformation(ctypes.Structure):
    _fields_ = [
        ("process_user_time", ctypes.c_int64),
        ("job_user_time", ctypes.c_int64),
        ("flags", wintypes.DWORD),
        ("min_working_set", ctypes.c_size_t),
        ("max_working_set", ctypes.c_size_t),
        ("active_process_limit", wintypes.DWORD),
        ("affinity", ctypes.c_size_t),
        ("priority_class", wintypes.DWORD),
        ("scheduling_class", wintypes.DWORD),
    ]


class ExtendedLimitInformation(ctypes.Structure):
    _fields_ = [
        ("basic", BasicLimitInformation),
        ("io_counters", ctypes.c_uint64 * 6),
        ("process_memory_limit", ctypes.c_size_t),
        ("job_memory_limit", ctypes.c_size_t),
        ("peak_process_memory", ctypes.c_size_t),
        ("peak_job_memory", ctypes.c_size_t),
    ]


class BasicAccountingInformation(ctypes.Structure):
    _fields_ = [
        ("total_user_time", ctypes.c_int64),
        ("total_kernel_time", ctypes.c_int64),
        ("period_user_time", ctypes.c_int64),
        ("period_kernel_time", ctypes.c_int64),
        ("total_page_faults", wintypes.DWORD),
        ("total_processes", wintypes.DWORD),
        ("active_processes", wintypes.DWORD),
        ("total_terminated_processes", wintypes.DWORD),
    ]


_kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)


def _bind(name, args, result):
    function = getattr(_kernel32, name)
    function.argtypes = args
    function.restype = result
    return function


CreateJobObjectW = _bind(
    "CreateJobObjectW", [ctypes.c_void_p, wintypes.LPCWSTR], wintypes.HANDLE)
SetInformationJobObject = _bind(
    "SetInformationJobObject",
    [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD], wintypes.BOOL)
QueryInformationJobObject = _bind(
    "QueryInformationJobObject",
    [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD,
     ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL)
TerminateJobObject = _bind(
    "TerminateJobObject", [wintypes.HANDLE, wintypes.UINT], wintypes.BOOL)
InitializeProcThreadAttributeList = _bind(
    "InitializeProcThreadAttributeList",
    [ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD,
     ctypes.POINTER(ctypes.c_size_t)], wintypes.BOOL)
UpdateProcThreadAttribute = _bind(
    "UpdateProcThreadAttribute",
    [ctypes.c_void_p, wintypes.DWORD, ctypes.c_size_t, ctypes.c_void_p,
     ctypes.c_size_t, ctypes.c_void_p, ctypes.c_void_p], wintypes.BOOL)
DeleteProcThreadAttributeList = _bind(
    "DeleteProcThreadAttributeList", [ctypes.c_void_p], None)
CreateProcessW = _bind(
    "CreateProcessW",
    [wintypes.LPCWSTR, wintypes.LPWSTR, ctypes.c_void_p, ctypes.c_void_p,
     wintypes.BOOL, wintypes.DWORD, ctypes.c_void_p, wintypes.LPCWSTR,
     ctypes.POINTER(StartupInfo), ctypes.POINTER(ProcessInformation)], wintypes.BOOL)


def _checked(value, operation):
    if not value:
        code = ctypes.get_last_error()
        raise ctypes.WinError(code, f"{operation}: {ctypes.FormatError(code)}")
    return value


class _Process:
    def __init__(self, argv, handle, pid):
        self._argv = argv
        self._handle = handle
        self.pid = pid

    def wait(self, timeout):
        status = _winapi.WaitForSingleObject(self._handle, max(0, int(timeout * 1000)))
        if status == _winapi.WAIT_TIMEOUT:
            raise subprocess.TimeoutExpired(self._argv, timeout)
        if status != _winapi.WAIT_OBJECT_0:
            raise OSError(f"unexpected process wait status {status:#x}")
        return _winapi.GetExitCodeProcess(self._handle)

    def close(self):
        handle, self._handle = self._handle, None
        if handle is not None:
            _winapi.CloseHandle(handle)


class SlotJob:
    def __init__(self):
        self._handle = _checked(CreateJobObjectW(None, None), "CreateJobObjectW")
        try:
            limits = ExtendedLimitInformation()
            limits.basic.flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            _checked(SetInformationJobObject(
                self._handle, JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                ctypes.byref(limits), ctypes.sizeof(limits)),
                "SetInformationJobObject(KILL_ON_JOB_CLOSE)")
        except BaseException:
            self.close()
            raise

    def launch(self, argv, creationflags, *, stdin, stdout, stderr):
        """Create the original process already in this Job, with the same stdio."""
        if self._handle is None:
            raise OSError("slot Job is closed")
        command_text = subprocess.list2cmdline(argv)
        if "\0" in command_text:
            raise ValueError("embedded null character")
        command = ctypes.create_unicode_buffer(command_text)
        process = ProcessInformation()
        try:
            with ExitStack() as resources:
                parent = _winapi.GetCurrentProcess()
                null_fd = None
                stdio = (wintypes.HANDLE * 3)()
                for index, stream in enumerate((stdin, stdout, stderr)):
                    if stream == subprocess.DEVNULL:
                        if null_fd is None:
                            null_fd = os.open(os.devnull, os.O_RDWR)
                            resources.callback(os.close, null_fd)
                        fd = null_fd
                    else:
                        fd = stream.fileno()
                    handle = _winapi.DuplicateHandle(
                        parent, msvcrt.get_osfhandle(fd), parent, 0, True,
                        _winapi.DUPLICATE_SAME_ACCESS)
                    resources.callback(_winapi.CloseHandle, handle)
                    stdio[index] = handle

                size = ctypes.c_size_t()
                InitializeProcThreadAttributeList(None, 2, 0, ctypes.byref(size))
                _checked(size.value, "InitializeProcThreadAttributeList(size)")
                attributes = ctypes.create_string_buffer(size.value)
                _checked(InitializeProcThreadAttributeList(
                    attributes, 2, 0, ctypes.byref(size)),
                    "InitializeProcThreadAttributeList")
                resources.callback(DeleteProcThreadAttributeList, attributes)
                jobs = (wintypes.HANDLE * 1)(self._handle)
                for attribute, values in ((PROC_THREAD_ATTRIBUTE_HANDLE_LIST, stdio),
                                          (PROC_THREAD_ATTRIBUTE_JOB_LIST, jobs)):
                    _checked(UpdateProcThreadAttribute(
                        attributes, 0, attribute, values, ctypes.sizeof(values), None, None),
                        f"UpdateProcThreadAttribute({attribute:#x})")

                startup = StartupInfoEx()
                startup.info.size = ctypes.sizeof(startup)
                startup.info.flags = STARTF_USESTDHANDLES
                startup.info.stdin, startup.info.stdout, startup.info.stderr = stdio
                startup.attributes = ctypes.cast(attributes, ctypes.c_void_p)
                flags = creationflags | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT
                try:
                    _checked(CreateProcessW(
                        None, command, None, None, True, flags, None, None,
                        ctypes.byref(startup.info), ctypes.byref(process)), "CreateProcessW")
                finally:
                    if process.thread:
                        _winapi.CloseHandle(process.thread)
            return _Process(argv, process.process, process.pid)
        except BaseException:
            if process.process:
                _winapi.CloseHandle(process.process)
            raise

    def _active_processes(self):
        if self._handle is None:
            raise OSError("slot Job is closed")
        accounting = BasicAccountingInformation()
        _checked(QueryInformationJobObject(
            self._handle, JOB_OBJECT_BASIC_ACCOUNTING_INFORMATION,
            ctypes.byref(accounting), ctypes.sizeof(accounting), None),
            "QueryInformationJobObject(BasicAccountingInformation)")
        return accounting.active_processes

    def drain(self):
        """Terminate remaining members and prove the Job empty before returning."""
        deadline = time.monotonic() + CLEANUP_TIMEOUT_S
        before = self._active_processes()
        after = before
        if before:
            _checked(TerminateJobObject(self._handle, CLEANUP_EXIT_CODE), "TerminateJobObject")
            after = self._active_processes()
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"slot Job did not drain within {CLEANUP_TIMEOUT_S:g} seconds")
            if after == 0:
                return {"active_before": before, "terminated": bool(before), "active_after": after}
            time.sleep(min(remaining, 0.005))
            after = self._active_processes()

    def close(self):
        handle, self._handle = self._handle, None
        if handle is not None:
            _winapi.CloseHandle(handle)
