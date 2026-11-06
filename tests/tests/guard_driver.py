# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Driver for the guard's CLI surfaces, shared by the test families.

Binaries are located, in order: the PTG_SUITE_BUILD environment
variable when set, then the suite's build folder (build/ next to
the suite root, as build.ps1 stages it). There is deliberately no
fallback to the guard tree's cargo target folder: a binary found
there was built by some other route, at some other time, from a
tree state nothing in this package records, and families passing
against it would certify a guard the campaign never staged —
an absent staged binary must skip, never silently substitute.
A missing binary raises BinaryMissing; families that require the
guard are skipped on that signal, and the negative-control family
asserts the signal itself, so an absent binary fails for the right
reason and in the right place — never as a cascade of spawn errors.

Every invocation runs under a finite deadline.

Environment overrides (`env_extra`) are applied with the suite's
shared case-insensitive setting rule (envcase.set_env_var): the
child environment is a copy of this process's environment, which
on Windows conventionally spells the search-path variable `Path`,
and a plain mapping update would leave that inherited entry in
the child block beside the override — two entries for one
variable, with the reader's case-insensitive lookup deciding
between them by block order instead of by the test's intent.
"""

import os
import subprocess
import sys

SUITE_ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, SUITE_ROOT)

from envcase import set_env_var  # noqa: E402
ROLES = {
    "full": "process-tree-guard",
    "deadline": "process-tree-guard-deadline",
    "gate": "process-tree-guard-gate",
    "contain": "process-tree-guard-contain",
    "testkit": "testkit",
}
RUN_DEADLINE_S = 60.0


class BinaryMissing(RuntimeError):
    pass


def _exe_name(base):
    return base + (".exe" if sys.platform == "win32" else "")


def spelling(path):
    """The executable spelling the guard resolves for a path."""
    text = path.replace("\\", "/")
    if text.startswith("//?/"):
        text = text[4:]
    return text


def build_dirs():
    dirs = []
    env = os.environ.get("PTG_SUITE_BUILD")
    if env:
        dirs.append(env)
    dirs.append(os.path.join(SUITE_ROOT, "build"))
    return [os.path.normpath(d) for d in dirs]


def locate(role):
    base = ROLES[role]
    for folder in build_dirs():
        candidate = os.path.join(folder, _exe_name(base))
        if os.path.isfile(candidate):
            return candidate
    raise BinaryMissing(
        f"{base} not found in any build folder {build_dirs()}; "
        "build the package first (build.ps1 on Windows)")


def guard_present():
    try:
        locate("full")
        locate("testkit")
        return True
    except BinaryMissing:
        return False


def run(argv, stdin=b"", deadline=RUN_DEADLINE_S, env_extra=None):
    """Run argv under a finite deadline; return (exit, stdout, stderr, seconds)."""
    import time
    env = None
    if env_extra:
        env = dict(os.environ)
        for name, value in env_extra.items():
            set_env_var(env, name, value)
    start = time.perf_counter()
    try:
        done = subprocess.run(argv, input=stdin, capture_output=True,
                              timeout=deadline, env=env)
        return done.returncode, done.stdout, done.stderr, time.perf_counter() - start
    except subprocess.TimeoutExpired:
        raise AssertionError(f"invocation exceeded its {deadline:g} s deadline: {argv!r}")


def guard(args, **kwargs):
    return run([locate("full")] + args, **kwargs)


def role(name, args, **kwargs):
    return run([locate(name)] + args, **kwargs)


def testkit(args):
    return [locate("testkit")] + args


def pid_alive(pid):
    """Windows-only survivor probe used by the kill-path family.
    Mirrors campaign/probe.py's semantics exactly: an access-denied
    open means the process exists, and a failed exit-code query is
    a check that cannot answer (raised), never a "dead" verdict —
    a survivor check that reports dead when it could not look can
    pass a kill-path test over a live descendant."""
    if sys.platform != "win32":
        raise RuntimeError("pid_alive is a Windows probe")
    import ctypes
    kernel32 = ctypes.windll.kernel32
    handle = kernel32.OpenProcess(0x1000, False, pid)  # QUERY_LIMITED
    if not handle:
        return ctypes.get_last_error() == 5  # ERROR_ACCESS_DENIED
    code = ctypes.c_ulong()
    ok = kernel32.GetExitCodeProcess(handle, ctypes.byref(code))
    kernel32.CloseHandle(handle)
    if not ok:
        raise RuntimeError("GetExitCodeProcess failed; "
                           "survival cannot be answered")
    return code.value == 259  # STILL_ACTIVE
