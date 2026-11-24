"""Offline fixture I/O tests. The descriptor model runs on non-Linux hosts too."""

import builtins
import hashlib
import importlib.util
import os
from pathlib import Path
import stat
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "linux_fixture_io", Path(__file__).with_name("linux_fixture_io.py"))
fixture_io = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture_io)


class DescriptorModel:
    """Keep real temp-file bytes behind modeled Linux descriptor operations."""

    O_RDONLY, O_PATH, O_NOFOLLOW, O_CLOEXEC, O_NONBLOCK, O_DIRECTORY = 0, 1, 2, 4, 8, 16
    stage = "/home/ordinary/stage"

    def __init__(self, local):
        self.local = local
        self.next_inode = 0
        self.next_fd = 10
        self.nodes = {}
        self.handles = {}
        self.calls = []
        self.before_read = None
        self.uid, self.gid, self.groups = (501,) * 3, (1000,) * 3, [999, 1000]
        for name in ("/", "/home", "/home/ordinary", self.stage):
            ordinary = name in ("/home/ordinary", self.stage)
            uid, gid = (501, 1000) if ordinary else (0, 0)
            self.nodes[name] = self.node(stat.S_IFDIR, b"", 0o700 if ordinary else 0o755, uid, gid)
        self.directory_fd = self.handle(self.stage, self.nodes[self.stage], False)

    def node(self, kind, data, mode, uid=501, gid=1000):
        self.next_inode += 1
        return SimpleNamespace(st_dev=17, st_ino=self.next_inode, st_uid=uid, st_gid=gid,
                               st_mode=kind | mode, st_nlink=1, st_size=len(data),
                               st_mtime_ns=1, st_ctime_ns=1, data=data)

    def file(self, name, data, *, kind=stat.S_IFREG, mode=0o600):
        path = self.local / name
        path.write_bytes(data)
        node = self.node(kind, path.read_bytes(), mode)
        self.nodes[f"{self.stage}/{name}"] = node
        return node

    def handle(self, path, node, readable):
        self.next_fd += 1
        self.handles[self.next_fd] = SimpleNamespace(path=path, node=node, readable=readable, offset=0)
        return self.next_fd

    def open(self, path, flags, *, dir_fd=None):
        self.calls.append(("open", path, flags, dir_fd))
        if path.startswith("/proc/self/fd/") and dir_fd is None:
            if flags != self.O_NONBLOCK | self.O_CLOEXEC:
                raise AssertionError("unexpected procfd read-open flags")
            held = self.handles[int(path.rsplit("/", 1)[1])]
            return self.handle(held.path, held.node, True)
        if not flags & self.O_PATH or not flags & self.O_NOFOLLOW:
            raise AssertionError("unexpected read-open of a pathname")
        full = path if dir_fd is None else self.handles[dir_fd].path.rstrip("/") + "/" + path
        if full not in self.nodes:
            raise FileNotFoundError(full)
        return self.handle(full, self.nodes[full], False)

    def stat(self, name, *, dir_fd, follow_symlinks):
        if follow_symlinks:
            raise AssertionError("unexpected symlink-following stat")
        path = self.handles[dir_fd].path + "/" + name
        if path not in self.nodes:
            raise FileNotFoundError(path)
        return SimpleNamespace(**vars(self.nodes[path]))

    def fstat(self, fd):
        return SimpleNamespace(**vars(self.handles[fd].node))

    def read(self, fd, count):
        stream = self.handles[fd]
        if not stream.readable:
            raise AssertionError("unexpected read on a metadata descriptor")
        self.calls.append(("read", stream.path, count, fd))
        if self.before_read:
            callback, self.before_read = self.before_read, None
            callback(stream)
        piece = stream.node.data[stream.offset:stream.offset + count]
        stream.offset += len(piece)
        return piece

    def close(self, fd):
        del self.handles[fd]

    def getresuid(self):
        return self.uid

    def getresgid(self):
        return self.gid

    def getgroups(self):
        return self.groups


class FixtureIOTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.local = Path(self.directory.name)
        self.model = DescriptorModel(self.local)
        model = patch.object(fixture_io, "os", self.model)
        model.start()
        self.addCleanup(model.stop)

    def read(self, name="fixture.py", limit=65_536):
        return fixture_io.read_regular(self.model.directory_fd, name, uid=501, gid=1000,
                                       mode=0o600, max_bytes=limit)

    def collect(self, deadline=None):
        if deadline is None:
            deadline = time.monotonic_ns() + 2_000_000_000
        return fixture_io.collect_ordinary_evidence(self.model.directory_fd, uid=501, gid=1000,
                                                   groups=(999, 1000), deadline_ns=deadline)

    def test_authenticates_before_compiling_or_executing_altered_code(self):
        sentinel = self.local / "never-created"
        altered = f"from pathlib import Path\nPath({str(sentinel)!r}).touch()\n".encode()
        for data in (altered, b"parse error ("):
            with self.subTest(data=data):
                self.model.file("fixture.py", data)
                good = hashlib.sha256(b"pass\n").hexdigest()
                with patch.object(builtins, "compile", side_effect=AssertionError("compiled rejected input")):
                    with self.assertRaisesRegex(ValueError, "fixture SHA-256 differs"):
                        fixture_io._run_authenticated(self.model.directory_fd, "mutable.py", good, 501, 1000)
                self.assertFalse(sentinel.exists())

    def test_standalone_bootstrap_executes_only_verified_snapshot(self):
        sentinel = self.local / "accepted"
        source = ("import sys\nfrom pathlib import Path\n"
                  "assert sys.argv[1] == '--root' and __file__ != sys.argv[0]\n"
                  "assert AUTHENTICATED_FIXTURE_SHA256 == sys.argv[2]\n"
                  "assert type(AUTHENTICATED_FIXTURE_BYTES) is bytes\n"
                  f"Path({str(sentinel)!r}).write_bytes(b'accepted')\n").encode()
        self.model.file("fixture.py", source)
        digest = hashlib.sha256(source).hexdigest()
        standalone = fixture_io.root_bootstrap_source()
        self.assertEqual(standalone, fixture_io.root_bootstrap_source())
        self.assertNotIn("import fixture_io", standalone)
        self.model.uid, self.model.gid = (0,) * 3, (0,) * 3
        fake_sys = SimpleNamespace(argv=["-c", self.model.stage, digest, "501", "1000"])
        real_import, real_compile = builtins.__import__, builtins.compile

        def modeled_import(name, *args, **kwargs):
            if name == "os":
                return self.model
            if name == "sys":
                return fake_sys
            return real_import(name, *args, **kwargs)

        def compile_after_substitution(value, *args, **kwargs):
            self.assertEqual(value, source)
            self.model.file("fixture.py", b"raise RuntimeError('reopened path')")
            return real_compile(value, *args, **kwargs)

        namespace = {"__builtins__": dict(vars(builtins), __import__=modeled_import,
                                          compile=compile_after_substitution)}
        with patch.object(builtins, "__import__", side_effect=modeled_import):
            exec(real_compile(standalone, "<bootstrap test>", "exec"), namespace)
        self.assertEqual(sentinel.read_bytes(), b"accepted")
        file_opens = [row for row in self.model.calls if row[0] == "open" and row[1] == "fixture.py"]
        self.assertEqual(len(file_opens), 1)
        self.assertTrue(file_opens[0][2] & self.model.O_PATH)
        self.assertEqual(list(self.model.handles), [self.model.directory_fd])

    def test_rejects_untrusted_ancestor_or_different_stage_device(self):
        self.model.nodes["/home"].st_mode = stat.S_IFDIR | 0o777
        with self.assertRaisesRegex(ValueError, "root stage ancestor"):
            fixture_io._open_stage(self.model.stage, 501, 1000)
        self.model.nodes["/home"].st_mode = stat.S_IFDIR | 0o755
        self.model.nodes[self.model.stage].st_dev = 18
        with self.assertRaisesRegex(ValueError, "stage is not confined"):
            fixture_io._open_stage(self.model.stage, 501, 1000)
        self.assertEqual(list(self.model.handles), [self.model.directory_fd])

    def test_special_metadata_never_reaches_read_open(self):
        for kind in (stat.S_IFLNK, stat.S_IFIFO, stat.S_IFCHR):
            with self.subTest(kind=kind):
                self.model.file("fixture.py", b"", kind=kind)
                self.model.calls.clear()
                with self.assertRaisesRegex(ValueError, "file type"):
                    self.read()
                self.assertEqual([row[1] for row in self.model.calls], ["fixture.py"])

    def test_rejects_wrong_owner_group_mode_link_and_device(self):
        for field, value in (("st_uid", 502), ("st_gid", 1001), ("st_mode", stat.S_IFREG | 0o644),
                             ("st_nlink", 2), ("st_dev", 18)):
            with self.subTest(field=field):
                node = self.model.file("fixture.py", b"pass")
                setattr(node, field, value)
                self.model.calls.clear()
                with self.assertRaisesRegex(ValueError, "file type"):
                    self.read()
                self.assertEqual(len(self.model.calls), 1)

    def test_rejects_oversize_before_any_data_read(self):
        self.model.file("fixture.py", b"abcdef")
        with self.assertRaisesRegex(ValueError, "file byte bound"):
            self.read(limit=5)
        self.assertEqual(len(self.model.calls), 1)

    def test_rejects_early_or_overlong_read(self):
        self.model.file("fixture.py", b"abcd")
        for piece in (b"", b"abcde"):
            with self.subTest(piece=piece), patch.object(self.model, "read", return_value=piece):
                with self.assertRaisesRegex(ValueError, "ended early or exceeded"):
                    self.read()

    def test_rejects_append_and_does_not_read_the_appended_byte(self):
        self.model.file("fixture.py", b"pass")

        def append(stream):
            stream.node.data += b"x"
            stream.node.st_size += 1
            stream.node.st_ctime_ns += 1

        self.model.before_read = append
        with self.assertRaisesRegex(ValueError, "changed during read"):
            self.read()
        self.assertEqual([row[2] for row in self.model.calls if row[0] == "read"], [4])

    def test_rejects_name_substitution_and_disappearance(self):
        for remove in (False, True):
            with self.subTest(remove=remove):
                self.model.file("fixture.py", b"pass")

                def replace(stream):
                    if remove:
                        del self.model.nodes[stream.path]
                    else:
                        self.model.file("fixture.py", b"pass")

                self.model.before_read = replace
                with self.assertRaisesRegex(ValueError, "named inode changed|held file disappeared"):
                    self.read()

    def test_collector_checks_credentials_and_held_directory(self):
        self.model.uid = (501, 0, 501)
        with self.assertRaisesRegex(ValueError, "credentials"):
            self.collect()
        self.model.uid = (501,) * 3
        self.model.groups = [1000]
        with self.assertRaisesRegex(ValueError, "credentials"):
            self.collect()
        self.model.groups = [999, 1000]
        self.model.nodes[self.model.stage].st_mode = stat.S_IFDIR | 0o770
        with self.assertRaisesRegex(ValueError, "directory identity"):
            self.collect()
        self.assertFalse(self.model.calls)

    def test_collector_reports_missing_and_refused_only_for_designed_names(self):
        self.model.file("DONE.json", b"{}", kind=stat.S_IFIFO)
        self.model.file("result.json", b"{}")
        self.model.file("unrelated.bin", b"ignored")
        rows = self.collect()
        self.assertEqual([(row["name"], row["status"]) for row in rows],
                         [("LIVE.json", "missing"), ("DONE.json", "refused"), ("result.json", "complete")])
        self.assertEqual(rows[2]["data"], b"{}")
        self.assertNotIn("unrelated.bin", [row[1] for row in self.model.calls])

    def test_collector_charges_failed_read_and_rejects_aggregate_overflow(self):
        self.model.file("LIVE.json", b"l" * 16_384)
        self.model.file("DONE.json", b"d" * 16_384)
        self.model.file("result.json", b"r" * (1_500_000 - 32_768 + 1))
        self.model.before_read = lambda stream: setattr(stream.node, "st_ctime_ns", 2)
        rows = self.collect()
        self.assertEqual([row["status"] for row in rows], ["refused", "complete", "refused"])
        self.assertEqual(rows[2]["reason"], "aggregate evidence byte bound exceeded")
        self.assertEqual([row[2] for row in self.model.calls if row[0] == "read"], [16_384, 16_384])

    def test_expired_collector_does_not_open_any_evidence(self):
        rows = self.collect(deadline=time.monotonic_ns() - 1)
        self.assertEqual([row["reason"] for row in rows], ["deadline"] * 3)
        self.assertFalse(self.model.calls)


@unittest.skipUnless(sys.platform == "linux" and hasattr(os, "O_PATH")
                     and os.getuid() != 0, "requires unprivileged Linux O_PATH and procfd")
class LinuxDescriptorTests(unittest.TestCase):
    def test_real_regular_file_symlink_and_fifo(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            regular = directory / "regular"
            regular.write_bytes(b"sample")
            regular.chmod(0o600)
            (directory / "link").symlink_to(regular)
            os.mkfifo(directory / "pipe", 0o600)
            fd = os.open(directory, os.O_PATH | os.O_DIRECTORY | os.O_CLOEXEC)
            try:
                limits = dict(uid=os.getuid(), gid=os.getgid(), mode=0o600, max_bytes=10)
                self.assertEqual(fixture_io.read_regular(fd, "regular", **limits), b"sample")
                for name in ("link", "pipe"):
                    with self.assertRaisesRegex(ValueError, "file type"):
                        fixture_io.read_regular(fd, name, **limits)
            finally:
                os.close(fd)


if __name__ == "__main__":
    unittest.main()
