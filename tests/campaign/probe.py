# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Survivor probe: is any named process still alive?

Usage: probe.py PID [PID ...]

Exit codes: 0 when no named process is alive, 1 when at least one
is alive, 2 on any usage or inspection error (no PIDs named, a PID
that is not a positive integer, a Windows PID outside the DWORD
range, or a liveness check the platform refused to answer). The campaign runner passes the descendant PIDs
recorded for one kill slot; exit 2 is recorded by the runner as a
check that did not run, never as "no survivor".

On Windows the check opens a process for synchronization and tests
whether its handle is signaled. An access-denied open means the
process exists. Elsewhere it is a zero-signal probe.
The probe inspects only the PIDs it is given: it attributes survival
to exactly one slot, the slot whose recorded identity it was handed.
"""

import os
import sys

ERROR_ACCESS_DENIED = 5
ERROR_INVALID_PARAMETER = 87
MAX_WINDOWS_PID = 0xFFFFFFFF


def pid_alive_windows(pid):
    import _winapi

    if not 0 < pid <= MAX_WINDOWS_PID:
        raise OSError("process ID is outside the Windows DWORD range")
    try:
        handle = _winapi.OpenProcess(_winapi.SYNCHRONIZE, False, pid)
    except OSError as error:
        if error.winerror == ERROR_ACCESS_DENIED:
            return True
        if error.winerror == ERROR_INVALID_PARAMETER:
            return False
        raise
    try:
        state = _winapi.WaitForSingleObject(handle, 0)
    finally:
        _winapi.CloseHandle(handle)
    if state == _winapi.WAIT_OBJECT_0:
        return False
    if state == _winapi.WAIT_TIMEOUT:
        return True
    raise OSError(f"unexpected process wait result: {state}")


def pid_alive_posix(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def main(argv):
    if not argv:
        return 2
    pids = []
    for text in argv:
        try:
            pid = int(text)
        except ValueError:
            return 2
        if pid <= 0 or (sys.platform == "win32" and pid > MAX_WINDOWS_PID):
            return 2
        pids.append(pid)
    check = pid_alive_windows if sys.platform == "win32" else pid_alive_posix
    try:
        for pid in pids:
            if check(pid):
                return 1
    except OSError:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
