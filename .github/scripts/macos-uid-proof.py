# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller

"""Provision two disabled fixture identities inside the pinned hosted macOS VM."""

import grp
import hashlib
import os
from pathlib import Path
import pwd
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time


HOSTED = {
    "GITHUB_ACTIONS": "true",
    "RUNNER_ENVIRONMENT": "github-hosted",
    "RUNNER_OS": "macOS",
    "RUNNER_ARCH": "ARM64",
    "GITHUB_REF": "refs/heads/main",
}
ACCOUNTS = (("ptgproofworker", 62081), ("ptgproofpeer", 62082))


def command(*argv, timeout=5):
    process = subprocess.Popen(
        argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    try:
        output, _ = process.communicate(timeout=timeout)
    except BaseException as error:
        # The new group has this unreaped child as its leader.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        if isinstance(error, subprocess.TimeoutExpired) and error.output:
            sys.stdout.buffer.write(error.output)
            sys.stdout.flush()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            print("The launched process could not be reaped by its deadline", flush=True)
            os._exit(124)
        raise
    finally:
        process.stdout.close()
    if process.returncode:
        sys.stdout.buffer.write(output)
        sys.stdout.flush()
        raise RuntimeError(f"{argv[0]} exited {process.returncode}")
    return output.decode("utf-8", errors="replace").strip()


def hosted_environment():
    for key, value in HOSTED.items():
        if os.environ.get(key) != value:
            raise RuntimeError(f"Unexpected hosted identity for {key}")
    environment = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", **HOSTED}
    for key in ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT"):
        value = os.environ.get(key, "")
        if not re.fullmatch(r"[1-9][0-9]{0,19}", value):
            raise RuntimeError(f"Invalid hosted identifier for {key}")
        environment[key] = value
    build = command("/usr/sbin/sysctl", "-n", "kern.osversion")
    kernel = command("/usr/sbin/sysctl", "-n", "kern.osrelease")
    model = command("/usr/sbin/sysctl", "-n", "hw.model")
    if build != "26A434" or kernel != "27.0.0" or not model.startswith("VirtualMac"):
        raise RuntimeError(f"Unexpected hosted VM {build} {kernel} {model}")
    print(f"hosted VM {build} {kernel} {model}", flush=True)
    return environment


def directory(*args):
    return command("/usr/bin/dscl", ".", *args)


def require_absent(lookup, value):
    try:
        lookup(value)
    except KeyError:
        return
    raise RuntimeError(f"Fixture identity already exists for {value}")


def set_record(record, fields):
    directory("-create", record)
    for name, value in fields.items():
        directory("-create", record, name, value)
    for name, value in fields.items():
        result = directory("-read", record, name)
        if result.split() != [f"{name}:", value]:
            raise RuntimeError(f"Unexpected {name} field for {record}")


def deadline(_signum, _frame):
    raise TimeoutError("The privileged fixture exceeded its total deadline")


def provision(binary, expected_sha):
    if {os.getuid(), os.getgid(), os.geteuid(), os.getegid()} != {0}:
        raise RuntimeError("The privileged fixture requires root:wheel")
    signal.signal(signal.SIGALRM, deadline)
    expires = time.monotonic() + 180
    signal.alarm(180)
    environment = hosted_environment()
    parent = Path("/private/var/tmp")
    parent_stat = parent.lstat()
    if not stat.S_ISDIR(parent_stat.st_mode) or parent_stat.st_uid or not parent_stat.st_mode & stat.S_ISVTX:
        raise RuntimeError("The fixture directory parent must be root-owned and sticky")
    lease = parent / f"ptg-uid-proof-{environment['GITHUB_RUN_ID']}-{environment['GITHUB_RUN_ATTEMPT']}"
    os.umask(0o077)
    lease.mkdir(mode=0o700)
    if not binary.is_absolute() or binary.name != "uid-proof":
        raise RuntimeError("Unexpected fixture executable path")
    with os.fdopen(os.open(binary, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as source:
        info = os.fstat(source.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o022 or info.st_size > 1024 * 1024:
            raise RuntimeError("Unexpected fixture executable metadata")
        data = source.read(1024 * 1024 + 1)
    if len(data) > 1024 * 1024 or hashlib.sha256(data).hexdigest() != expected_sha:
        raise RuntimeError("The compiled fixture executable changed")
    executable = lease / "uid-proof"
    with executable.open("xb") as output:
        output.write(data)
    executable.chmod(0o700)
    print(f"root-owned executable sha256={expected_sha}", flush=True)

    for name, number in ACCOUNTS:
        require_absent(pwd.getpwnam, name)
        require_absent(pwd.getpwuid, number)
        require_absent(grp.getgrnam, name)
        require_absent(grp.getgrgid, number)
    for name, number in ACCOUNTS:
        set_record(f"/Groups/{name}", {"PrimaryGroupID": str(number)})
        set_record(
            f"/Users/{name}",
            {
                "AuthenticationAuthority": ";DisabledUser;",
                "Password": "*",
                "UniqueID": str(number),
                "PrimaryGroupID": str(number),
                "NFSHomeDirectory": "/var/empty",
                "UserShell": "/usr/bin/false",
            },
        )
    print("verified two disabled fixture accounts", flush=True)

    if expires - time.monotonic() < 135:
        raise TimeoutError("Insufficient deadline reserve for the native fixture")
    # The native fixture also independently bounds its session-escaping children.
    print(command(str(executable), timeout=130), flush=True)
    signal.alarm(0)


def main():
    if len(sys.argv) == 4 and sys.argv[1] == "--provision":
        provision(Path(sys.argv[2]), sys.argv[3])
        return
    fixtures = {"parked": "uid-proof.c", "fork-churn": "uid-fork-churn.c"}
    selection = sys.argv[1] if len(sys.argv) == 2 else "parked"
    if len(sys.argv) > 2 or selection not in fixtures or os.geteuid() == 0:
        raise RuntimeError("Select a fixture as the unprivileged hosted runner")
    environment = hosted_environment()
    source = Path(__file__).resolve().with_name(fixtures[selection])
    print(f"fixture={selection}", flush=True)
    print(f"source sha256={hashlib.sha256(source.read_bytes()).hexdigest()}", flush=True)
    with tempfile.TemporaryDirectory(prefix="ptg-macos-proof-", dir=os.environ["RUNNER_TEMP"]) as scratch:
        binary = Path(scratch) / "uid-proof"
        print(command("/usr/bin/xcrun", "--sdk", "macosx", "clang", "--version"), flush=True)
        command(
            "/usr/bin/xcrun", "--sdk", "macosx", "clang",
            "-std=c11", "-Wall", "-Wextra", "-Werror", "-O2", str(source),
            "-lproc", "-o", str(binary), timeout=30,
        )
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        output = command(
            "/usr/bin/sudo", "-n", "/usr/bin/env", "-i",
            *(f"{key}={value}" for key, value in environment.items()),
            "/usr/bin/python3", "-I", "-B", str(Path(__file__).resolve()),
            "--provision", str(binary), digest, timeout=190,
        )
        print(output, flush=True)


if __name__ == "__main__":
    main()
