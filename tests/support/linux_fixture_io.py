"""Linux-only I/O for a separately authorized, finite fixture controller.

Publish and authorize the generated bootstrap bytes independently of the
ordinary stage. The bootstrap imports no package from that stage. All callers
still need a native whole-process deadline: even regular-file I/O can block.
"""

import hashlib
import inspect
import os
import stat
import sys
import time


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _check_deadline(deadline_ns):
    if deadline_ns is not None and time.monotonic_ns() >= deadline_ns:
        raise TimeoutError("fixture I/O deadline expired")


def _fingerprint(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_gid, info.st_mode,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _read_regular(directory_fd, name, uid, gid, mode, max_bytes, deadline_ns, budget):
    _require(type(name) is str and name not in ("", ".", "..")
             and "/" not in name and "\0" not in name, "expected one file component")
    _require(type(max_bytes) is int and 0 <= max_bytes <= 2_300_000,
             "invalid file byte bound")
    _check_deadline(deadline_ns)
    directory = os.fstat(directory_fd)
    _require(stat.S_ISDIR(directory.st_mode), "expected held directory")
    held = os.open(name, os.O_PATH | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=directory_fd)
    try:
        info = os.fstat(held)
        _require(stat.S_ISREG(info.st_mode) and info.st_dev == directory.st_dev
                 and (info.st_uid, info.st_gid, stat.S_IMODE(info.st_mode), info.st_nlink)
                 == (uid, gid, mode, 1), "file type, device, ownership, mode or link count differs")
        size = info.st_size
        _require(0 <= size <= max_bytes, "file byte bound exceeded")
        if budget is not None:
            _require(size <= budget[0], "aggregate evidence byte bound exceeded")
            # A file that later fails still consumes its reserved read budget.
            budget[0] -= size
        identity = _fingerprint(info)
        _check_deadline(deadline_ns)
        # The only read-open is through the kernel reference to the held inode.
        readable = os.open(f"/proc/self/fd/{held}", os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC)
        try:
            _require(_fingerprint(os.fstat(readable)) == identity,
                     "held inode changed before read")
            data = bytearray()
            while len(data) < size:
                _check_deadline(deadline_ns)
                want = min(65_536, size - len(data))
                piece = os.read(readable, want)
                _require(0 < len(piece) <= want, "held file ended early or exceeded read bound")
                data.extend(piece)
            _check_deadline(deadline_ns)
            _require(_fingerprint(os.fstat(readable)) == identity
                     and _fingerprint(os.fstat(held)) == identity,
                     "held inode changed during read")
            _require(_fingerprint(os.stat(name, dir_fd=directory_fd, follow_symlinks=False))
                     == identity, "named inode changed during read")
            _check_deadline(deadline_ns)
            return bytes(data)
        finally:
            os.close(readable)
    except FileNotFoundError as error:
        raise ValueError("held file disappeared or could not be reopened") from error
    finally:
        os.close(held)


def read_regular(directory_fd, name, *, uid, gid, mode, max_bytes, deadline_ns=None):
    """Read bounded bytes from one regular file under a caller-held directory."""
    return _read_regular(directory_fd, name, uid, gid, mode, max_bytes, deadline_ns, None)


def collect_ordinary_evidence(directory_fd, *, uid, gid, groups, deadline_ns):
    """Collect the three ordinary records only after the caller has dropped privileges.

    This function does not drop credentials or provide a process timeout. Run
    it inside the existing finite descendant guard and bound its output pipe.
    The launcher must also clear elevated capabilities and inherited authority.
    Matching metadata is not an atomic snapshot of mutable file contents:
    returned ordinary records remain untrusted evidence for the caller.
    """
    _require(type(uid) is int and uid > 0 and type(gid) is int and gid > 0
             and os.getresuid() == (uid,) * 3 and os.getresgid() == (gid,) * 3
             and set(os.getgroups()) == set(groups), "ordinary collector credentials differ")
    _require(type(deadline_ns) is int and deadline_ns > 0, "collector requires a finite deadline")
    directory = os.fstat(directory_fd)
    _require(stat.S_ISDIR(directory.st_mode) and directory.st_uid == uid
             and directory.st_gid == gid and stat.S_IMODE(directory.st_mode) == 0o700,
             "ordinary evidence directory identity differs")
    budget = [1_500_000]
    rows = []
    for name, limit in (("LIVE.json", 16_384), ("DONE.json", 16_384), ("result.json", 1_500_000)):
        record = {"name": name}
        try:
            data = _read_regular(directory_fd, name, uid, gid, 0o600, limit, deadline_ns, budget)
            record.update(status="complete", data=data, sha256=hashlib.sha256(data).hexdigest())
        except FileNotFoundError:
            record.update(status="missing")
        except TimeoutError:
            record.update(status="refused", reason="deadline")
        except OSError as error:
            record.update(status="refused", reason="native I/O error", errno=error.errno)
        except ValueError as error:
            record.update(status="refused", reason=str(error))
        rows.append(record)
    return rows


def _trusted_directory(info):
    return (stat.S_ISDIR(info.st_mode) and info.st_uid == info.st_gid == 0
            and not stat.S_IMODE(info.st_mode) & 0o022)


def _open_stage(stage, uid, gid):
    _require(type(stage) is str and stage.startswith("/") and len(stage) <= 4096,
             "expected exact absolute stage")
    parts = stage[1:].split("/")
    _require(len(parts) >= 3 and all(part not in ("", ".", "..") and "\0" not in part
                                     for part in parts), "invalid stage components")
    flags = os.O_PATH | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    fd = os.open("/", flags)
    try:
        info = os.fstat(fd)
        _require(_trusted_directory(info), "root stage ancestor differs")
        for index, part in enumerate(parts):
            child = os.open(part, flags, dir_fd=fd)
            try:
                next_info = os.fstat(child)
                if index < len(parts) - 2:
                    _require(_trusted_directory(next_info), "root stage ancestor differs")
                else:
                    _require(stat.S_ISDIR(next_info.st_mode)
                             and (next_info.st_uid, next_info.st_gid) == (uid, gid)
                             and not stat.S_IMODE(next_info.st_mode) & 0o022,
                             "ordinary stage parent or directory differs")
                if index == len(parts) - 1:
                    _require(stat.S_IMODE(next_info.st_mode) == 0o700
                             and next_info.st_dev == info.st_dev, "ordinary stage is not confined")
            except BaseException:
                os.close(child)
                raise
            os.close(fd)
            fd, info = child, next_info
        return fd
    except BaseException:
        os.close(fd)
        raise


def _run_authenticated(directory_fd, diagnostic_path, expected_sha256, uid, gid):
    _require(type(expected_sha256) is str and len(expected_sha256) == 64
             and all(ch in "0123456789abcdef" for ch in expected_sha256), "invalid pinned SHA-256")
    source = read_regular(directory_fd, "fixture.py", uid=uid, gid=gid, mode=0o600,
                          max_bytes=65_536)
    _require(hashlib.sha256(source).hexdigest() == expected_sha256, "fixture SHA-256 differs")
    filename = "<authenticated Linux fixture>"
    code = compile(source, filename, "exec", dont_inherit=True)
    scope = {"__name__": "__main__", "__file__": filename,
             "AUTHENTICATED_FIXTURE_BYTES": source, "AUTHENTICATED_FIXTURE_SHA256": expected_sha256}
    original_argv = sys.argv
    sys.argv = [diagnostic_path, "--root", expected_sha256]
    try:
        exec(code, scope)
    finally:
        sys.argv = original_argv


def _role_number(text):
    _require(0 < len(text) <= 10 and text.isascii() and text.isdecimal() and text[0] != "0",
             "invalid ordinary credential number")
    value = int(text)
    _require(value < 2 ** 32 - 1, "ordinary credential number out of range")
    return value


def _root_entry():
    _require(len(sys.argv) == 5 and os.getresuid() == (0, 0, 0)
             and os.getresgid() == (0, 0, 0), "exact root bootstrap role differs")
    stage, expected_sha256, raw_uid, raw_gid = sys.argv[1:]
    uid, gid = _role_number(raw_uid), _role_number(raw_gid)
    fd = _open_stage(stage, uid, gid)
    try:
        _run_authenticated(fd, stage + "/fixture.py", expected_sha256, uid, gid)
    finally:
        os.close(fd)


def root_bootstrap_source():
    """Return self-contained code for independently pinned administrator argv.

    Arguments after the code are the exact absolute stage, fixture digest, UID,
    and GID. The consuming fixture must use the authenticated bytes global in
    its root entry, not reopen the diagnostic pathname as executable source.
    """
    functions = (_require, _check_deadline, _fingerprint, _read_regular, read_regular,
                 _trusted_directory, _open_stage, _run_authenticated, _role_number, _root_entry)
    return "\n\n".join(["import hashlib, os, stat, sys, time",
                        *(inspect.getsource(fn).strip() for fn in functions), "_root_entry()\n"])
