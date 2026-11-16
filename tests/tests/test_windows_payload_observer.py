# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Windows marker-reader regression; run under a finite whole-tree test deadline.

Controls are preserved under the repository target directory. No product or
payload process is launched; a sentinel stops before the payload process open.
"""
from pathlib import Path
import importlib.util
import sys
import time
import unittest
from unittest import mock
import uuid
from types import SimpleNamespace

if sys.platform == "win32":
    import ctypes as C
    from ctypes import wintypes as W
    module_path = Path(__file__).resolve().parents[1] / "campaign" / "windows_payload_observer.py"
    spec = importlib.util.spec_from_file_location("campaign_windows_payload_observer_regression", module_path)
    observer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(observer)


class BeforePayloadOpen(Exception):
    pass


@unittest.skipUnless(sys.platform == "win32", "requires Windows file handles")
class NativeMarkerReaderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        tree = Path(__file__).resolve().parents[2]
        target = tree / "target"
        target.mkdir(exist_ok=True)
        if (target.is_symlink() or bool(target.lstat().st_file_attributes & 0x400) or
                target.resolve(strict=True).parent != tree.resolve(strict=True)):
            raise AssertionError("target escaped repository")
        owned_root = target / "windows-payload-observer-regression"
        owned_root.mkdir(exist_ok=True)
        if (owned_root.is_symlink() or bool(owned_root.lstat().st_file_attributes & 0x400) or
                owned_root.resolve(strict=True).parent != target.resolve(strict=True)):
            raise AssertionError("control root escaped target")
        cls.control_dir = owned_root / uuid.uuid4().hex
        if cls.control_dir.parent.resolve(strict=True) != owned_root.resolve(strict=True):
            raise AssertionError("control directory escaped owned target")
        cls.control_dir.mkdir(exist_ok=False)
        cls.kernel = C.WinDLL("kernel32", use_last_error=True)
        cls.create = cls.kernel.CreateFileW
        cls.create.argtypes = [W.LPCWSTR, W.DWORD, W.DWORD, C.c_void_p,
                               W.DWORD, W.DWORD, W.HANDLE]
        cls.create.restype = W.HANDLE
        cls.close = cls.kernel.CloseHandle
        cls.close.argtypes = [W.HANDLE]
        cls.close.restype = W.BOOL

    def observe(self, marker):
        row = {"armed_deadline_ns": time.perf_counter_ns() + 2_000_000_000}
        callback, _ = observer.make_observers(
            object(), SimpleNamespace(SYNC_QUERY=0), marker, marker, lambda *args: None)
        sentinel = lambda *args: (_ for _ in ()).throw(BeforePayloadOpen())
        with mock.patch.object(observer, "_winapi", SimpleNamespace(OpenProcess=sentinel)):
            try:
                callback(1, 1, time.monotonic() + 2, {}, row)
            except BaseException as error:
                return row, error
        self.fail("observer passed payload-open sentinel")

    def test_delete_holder_three_markers(self):
        for index in range(3):
            with self.subTest(index=index):
                marker = self.control_dir / ("delete-" + str(index) + ".pid")
                with marker.open("xb") as stream:
                    stream.write(b"12345")
                holder = self.create(str(marker), 0x00010000, 7, None, 3, 0x80, None)
                self.assertNotIn(holder, (None, observer.MARKER_INVALID_HANDLE))
                reader_handles, closes = [], []
                real_create, real_close = observer._marker_create, observer._marker_close

                def counted_create(*args):
                    handle = real_create(*args)
                    reader_handles.append(int(handle))
                    return handle

                def counted_close(handle):
                    self.assertEqual([int(handle)], reader_handles)
                    self.assertNotEqual(int(handle), int(holder))
                    closes.append(int(handle))
                    return real_close(handle)

                try:
                    with mock.patch.object(observer, "_marker_create", counted_create), \
                         mock.patch.object(observer, "_marker_close", counted_close):
                        row, error = self.observe(marker)
                    self.assertIsInstance(error, BeforePayloadOpen)
                    self.assertEqual(row["marker_hex"], b"12345".hex())
                    self.assertEqual(closes, reader_handles)
                finally:
                    self.assertTrue(self.close(holder))
                self.assertEqual(marker.read_bytes(), b"12345")

    def test_injected_native_failures_and_nonzero_success(self):
        marker = self.control_dir / "fault.pid"
        with marker.open("xb") as stream:
            stream.write(b"12345")

        def check_case(name, read_mode, close_mode, outer=False):
            calls = {"open": 0, "read": 0, "close": 0}
            primary = RuntimeError("internal read failure")

            def fake_create(*args):
                calls["open"] += 1
                return 0x1234

            def fake_read(handle, buffer, size, count_pointer, overlapped):
                calls["read"] += 1
                if read_mode == "raise":
                    raise primary
                if read_mode == "fail":
                    C.set_last_error(5)
                    return 0
                C.memmove(buffer, b"12345", 5)
                C.cast(count_pointer, C.POINTER(W.DWORD))[0] = 5
                return 2 if read_mode == "nonzero" else 1

            def fake_close(handle):
                calls["close"] += 1
                self.assertEqual(int(handle), 0x1234)
                if close_mode == "fail":
                    C.set_last_error(6)
                    return 0
                return 2 if close_mode == "nonzero" else 1

            with mock.patch.object(observer, "_marker_create", fake_create), \
                 mock.patch.object(observer, "_marker_read", fake_read), \
                 mock.patch.object(observer, "_marker_close", fake_close):
                if outer:
                    try:
                        raise RuntimeError("outer active exception")
                    except RuntimeError:
                        row, error = self.observe(marker)
                else:
                    row, error = self.observe(marker)
            self.assertEqual(calls, {"open": 1, "read": 1, "close": 1}, name)
            if read_mode == "fail":
                self.assertEqual(getattr(error, "winerror", None), 5)
                self.assertEqual(row["marker_access_failure"]["stage"], "marker.native_read")
                if close_mode == "fail":
                    self.assertEqual(row["marker_close_failure"]["native_winerror"], 6)
            elif read_mode == "raise":
                self.assertIs(error, primary)
            elif close_mode == "fail":
                self.assertEqual(getattr(error, "winerror", None), 6)
                self.assertEqual(row["marker_access_failure"]["stage"], "marker.native_close")
            else:
                self.assertIsInstance(error, BeforePayloadOpen)
                self.assertEqual(row["marker_hex"], b"12345".hex())

        check_case("read+close", "fail", "fail")
        check_case("outer exception + close", "ok", "fail", outer=True)
        check_case("internal read", "raise", "ok")
        check_case("nonzero BOOL", "nonzero", "nonzero")


if __name__ == "__main__":
    unittest.main()
