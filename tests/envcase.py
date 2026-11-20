# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Case-insensitive environment-variable setting, shared by every
suite site that overrides a named variable in a child environment.

A Python mapping is case-sensitive; the Windows environment
namespace is not, and a Windows parent environment conventionally
spells the search-path variable `Path`. A plain assignment on a
copy of `os.environ` — `env["PATH"] = controlled` — therefore does
not replace a `Path` entry: the child block carries both spellings,
and which one a reader that looks the name up case-insensitively
(such as the guard's PATH read) returns is an artefact of block
construction order, not of the caller's intent. The override then
silently fails to control the variable it names.

`set_env_var` is the one setting rule for the suite: delete every
existing key equal to the name under ASCII case folding, then set
the intended spelling. The result carries exactly one entry for
the variable, holding the intended value, under the spelling the
caller asked for. Folding is ASCII-only (A-Z to a-z), matching the
comparison the environment block itself is built with; variable
names outside that fold are compared exactly. A mapping with no
pre-existing variant of the name gains the new key and is
otherwise unchanged.
"""


def folded_name(name):
    """The ASCII case-folded form of an environment variable name."""
    return "".join(
        chr(ord(ch) + 32) if "A" <= ch <= "Z" else ch for ch in name)


def matching_keys(env, name):
    """The keys of `env` equal to `name` under ASCII case folding,
    in mapping order."""
    folded = folded_name(name)
    return [key for key in env if folded_name(key) == folded]


def set_env_var(env, name, value):
    """Set `name` to `value` in the mapping `env`, replacing every
    case variant of the name, so exactly one entry for the variable
    remains — the intended spelling carrying the intended value.
    Mutates and returns `env`."""
    for key in matching_keys(env, name):
        del env[key]
    env[name] = value
    return env

# Error policy belongs to these test-owned Python processes, not the product.
_WINDOWS_QUIET_ERROR_BITS = 0x8003
_windows_error_mode = None


def suppress_windows_error_dialogs():
    """Preserve existing error-mode bits and suppress OS error UI before spawn.

    Children inherit this process mode. The outer launcher must establish it
    before this Python process loads too. No alignment-fault handling is added;
    errors, exit statuses and captured stderr remain available to the harness.
    """
    global _windows_error_mode
    import os
    if os.name != "nt":
        return None
    if _windows_error_mode is not None:
        return _windows_error_mode
    import ctypes
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    get_mode = kernel32.GetErrorMode
    get_mode.argtypes = []
    get_mode.restype = ctypes.c_uint32
    set_mode = kernel32.SetErrorMode
    set_mode.argtypes = [ctypes.c_uint32]
    set_mode.restype = ctypes.c_uint32
    expected = get_mode() | _WINDOWS_QUIET_ERROR_BITS
    set_mode(expected)
    observed = get_mode()
    if observed & expected != expected:
        raise RuntimeError("Windows harness error-mode readback failed: "
                           f"expected bits {expected:#x}, observed {observed:#x}")
    _windows_error_mode = observed
    return observed


def hidden_helper_kwargs():
    """Hide an incidental helper's initial window without changing its flags.

    Do not apply this helper to a measured arm or an explicit console test.
    """
    suppress_windows_error_dialogs()
    import os
    if os.name != "nt":
        return {}
    import subprocess
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = subprocess.SW_HIDE
    return {"startupinfo": startup}
