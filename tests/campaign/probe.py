# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Survivor probe: is any named process still alive?

Usage: probe.py PID [PID ...]

Exit codes: 0 when no named process is alive, 1 when at least one
is alive, 2 on any usage or inspection error (no PIDs named, a PID
that is not a positive integer, or a liveness check the platform
refused to answer). The campaign runner passes the descendant PIDs
recorded for one kill slot; exit 2 is recorded by the runner as a
check that did not run, never as "no survivor".

On Windows the check is OpenProcess with limited query rights plus
GetExitCodeProcess against the still-active code; an access-denied
open means the process exists. Elsewhere it is a zero-signal probe.
The probe inspects only the PIDs it is given: it attributes survival
to exactly one slot, the slot whose recorded identity it was handed.
"""

import os
import sys

STILL_ACTIVE = 259
ERROR_ACCESS_DENIED = 5


def pid_alive_windows(pid):
    import ctypes

    kernel32 = ctypes.windll.kernel32
    handle = kernel32.OpenProcess(0x1000, False, pid)  # QUERY_LIMITED
    if not handle:
        return ctypes.get_last_error() == ERROR_ACCESS_DENIED
    code = ctypes.c_ulong()
    ok = kernel32.GetExitCodeProcess(handle, ctypes.byref(code))
    kernel32.CloseHandle(handle)
    if not ok:
        raise OSError("GetExitCodeProcess failed")
    return code.value == STILL_ACTIVE


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
        if pid <= 0:
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
