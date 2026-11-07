# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Real Windows failed-close ownership regressions for the campaign Job helper."""

from pathlib import Path
import sys
import unittest

EVENT_SINK = None


def record(kind, **facts):
    if EVENT_SINK is not None:
        EVENT_SINK({"kind": kind, **facts})


if sys.platform == "win32":
    import ctypes
    from ctypes import wintypes
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "campaign"))
    import windows_slot_job as slot

    kernel = ctypes.WinDLL("kernel32.dll", use_last_error=True)
    get_flags = kernel.GetHandleInformation
    get_flags.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
    get_flags.restype = wintypes.BOOL
    set_flags = kernel.SetHandleInformation
    set_flags.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.DWORD]
    set_flags.restype = wintypes.BOOL
    get_system_directory = kernel.GetSystemDirectoryW
    get_system_directory.argtypes = [wintypes.LPWSTR, wintypes.UINT]
    get_system_directory.restype = wintypes.UINT

    def probe(handle):
        flags = wintypes.DWORD()
        ctypes.set_last_error(0)
        ok = bool(get_flags(handle, ctypes.byref(flags)))
        error = ctypes.get_last_error()
        record("GetHandleInformation", handle=str(handle), ok=ok,
               flags=flags.value, error=error)
        return ok, flags.value, error

    def protect(handle, enabled):
        ctypes.set_last_error(0)
        ok = bool(set_flags(handle, 2, 2 if enabled else 0))
        error = ctypes.get_last_error()
        record("SetHandleInformation", handle=str(handle), ok=ok,
               mask=2, flags=2 if enabled else 0, error=error)
        if not ok:
            raise ctypes.WinError(error)

    def system_cmd():
        buffer = ctypes.create_unicode_buffer(32768)
        count = get_system_directory(buffer, len(buffer))
        if not 0 < count < len(buffer):
            raise ctypes.WinError(ctypes.get_last_error())
        return str(Path(buffer.value) / "cmd.exe")


@unittest.skipUnless(sys.platform == "win32", "requires real Windows handles")
class NativeCloseOwnershipTests(unittest.TestCase):
    def assert_close_contract(self, owner):
        original = owner._handle
        closed = False
        record("owner_acquired", owner_type=type(owner).__name__,
               handle=str(original))
        try:
            self.assertTrue(probe(original)[0])
            protect(original, True)
            self.assertEqual(probe(original)[:2], (True, 2))
            with self.assertRaises(OSError) as refused:
                owner.close()
            record("close_refused", handle=str(original),
                   error=refused.exception.winerror,
                   retained_handle=str(owner._handle))
            self.assertEqual(refused.exception.winerror, 6)
            self.assertEqual(owner._handle, original, "NATIVE_CLOSE_OWNER_LOST")
            self.assertEqual(probe(original)[:2], (True, 2))
            protect(original, False)
            owner.close()
            closed = True
            record("close_succeeded", handle=str(original),
                   retained_handle=str(owner._handle))
            self.assertIsNone(owner._handle)
            owner.close()
            record("close_idempotent", handle=str(original))
            self.assertIsNone(owner._handle)
        finally:
            # Never probe/reclose after success: a numeric handle may be reused.
            if not closed:
                ok, _, _ = probe(original)
                if ok:
                    protect(original, False)
                    slot._winapi.CloseHandle(original)
                    record("exact_fixture_recovery", handle=str(original))
                    if owner._handle == original:
                        owner._handle = None

    def test_job_retains_owner_when_native_close_is_refused(self):
        job = slot.SlotJob()
        try:
            self.assertEqual(job.drain()["active_after"], 0)
            self.assert_close_contract(job)
        finally:
            if job._handle is not None:
                protect(job._handle, False)
                job.close()

    def test_terminal_process_retains_owner_when_native_close_is_refused(self):
        job = slot.SlotJob()
        process = None
        try:
            process = job.launch([system_cmd(), "/d", "/c", "exit", "0"],
                                 0x08000000, stdin=slot.subprocess.DEVNULL,
                                 stdout=slot.subprocess.DEVNULL,
                                 stderr=slot.subprocess.DEVNULL)
            record("terminal_process_created", pid=process.pid,
                   handle=str(process._handle))
            self.assertEqual(process.wait(2.0), 0)
            self.assertEqual(job.drain()["active_after"], 0)
            self.assert_close_contract(process)
        finally:
            try:
                self.assertEqual(job.drain()["active_after"], 0)
            finally:
                if process is not None and process._handle is not None:
                    protect(process._handle, False)
                    process.close()
                if job._handle is not None:
                    job.close()


if __name__ == "__main__":
    unittest.main()
