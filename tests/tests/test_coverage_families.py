# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Coverage families for the hook and resolver branches the
failure tree previously recorded as unimplemented:

  F7  the 16 MiB payload cap (hook surfaces)
  F8  the context vendors' policy output, and worked examples for
      the two rewriting vendors (antigravity, powershell-auto)
  F9  non-Unicode argument handling
  F10 resolver probe order (PATH / PATHEXT / current-folder rule)

The payload construction, the install-layout fixtures and the
resolver field layout run anywhere and are checked anywhere; the
guard-driving assertions are Windows-tier and skip where the
binaries are absent, with the same discipline as the other
families.
"""

import base64
import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.dirname(HERE))

import guard_driver as drv  # noqa: E402
from envcase import matching_keys, set_env_var  # noqa: E402

PRESENT = drv.guard_present()
needs_guard = unittest.skipUnless(
    PRESENT, "guard binaries not built here (Windows package); see README tiers")
needs_windows = unittest.skipUnless(
    sys.platform == "win32", "Windows-tier family")

MAX_INPUT = 16 * 1024 * 1024
POLICY_FALLBACK = "POLICY.md not found beside the installed process guard."


# ------------------------------------------------------------- F7 cap

def cap_payloads(exe):
    """The cap family's payloads for envelope executable `exe`.
    Construction runs anywhere; only the verdicts need the guard."""
    good = (f"{exe} --timeout 60 --shell powershell "
            f"--command-b64 ZWNobyBPSw==")
    base = json.dumps({"tool_input": {"command": good}}).encode()
    at_cap = base + b" " * (MAX_INPUT - len(base))
    over_cap = base + b" " * (MAX_INPUT + 4096 - len(base))
    unterminated = b'{"tool_input":{"command":"' + b"a" * MAX_INPUT
    beyond = (b'{"x":"' + b"a" * MAX_INPUT +
              b'","tool_input":{"command":' +
              json.dumps(good).encode() + b"}}")
    return {"base": base, "at_cap": at_cap, "over_cap": over_cap,
            "unterminated": unterminated, "beyond": beyond}


class CapConstructionTests(unittest.TestCase):
    def test_cap_payload_construction_sizes(self):
        payloads = cap_payloads("C:/Tools/ptg/bin/process-tree-guard.exe")
        self.assertEqual(len(payloads["at_cap"]), MAX_INPUT)
        self.assertGreater(len(payloads["over_cap"]), MAX_INPUT)
        self.assertGreater(len(payloads["unterminated"]), MAX_INPUT)
        self.assertGreater(len(payloads["beyond"]), MAX_INPUT)
        # The valid payload's own bytes end well inside the cap:
        # its verdict is decided by content, not by truncation.
        self.assertLess(len(payloads["base"]), 4096)


@needs_guard
class F7PayloadCapTests(unittest.TestCase):
    """The documented cap behaviour (MEASUREMENT section 3, the
    in-tree `streaming_verdict_keeps_the_payload_cap` shapes): the
    scanner reads at most the first 16 MiB, and those bytes decide.
    A complete valid payload is therefore allowed at and past the
    cap when the deciding bytes stand inside it; a payload whose
    watched string never closes inside it, or only begins past it,
    is denied."""

    @classmethod
    def setUpClass(cls):
        cls.payloads = cap_payloads(drv.spelling(drv.locate("full")))

    def hook(self, payload):
        return drv.guard(["hook", "pretooluse"], stdin=payload)

    def test_payload_exactly_at_the_cap_is_allowed(self):
        exit_code, out, _, _ = self.hook(self.payloads["at_cap"])
        self.assertEqual(exit_code, 0)
        self.assertEqual(out.strip(), b"{}")

    def test_payload_past_the_cap_decides_on_its_first_cap_bytes(self):
        exit_code, out, _, _ = self.hook(self.payloads["over_cap"])
        self.assertEqual(exit_code, 0)
        self.assertEqual(out.strip(), b"{}")

    def test_unterminated_payload_past_the_cap_is_denied(self):
        exit_code, *_ = self.hook(self.payloads["unterminated"])
        self.assertEqual(exit_code, 2)

    def test_watched_string_beyond_the_cap_is_denied(self):
        exit_code, *_ = self.hook(self.payloads["beyond"])
        self.assertEqual(exit_code, 2)


# ------------------------------------------------------- F8 vendors

class Install:
    """A throwaway install folder: the full binary under its own
    name, optionally the automatic wrapper beside it, and
    optionally a POLICY.md in the folder above (the policy file's
    documented place). The layout code runs anywhere."""

    def __init__(self, root, with_wrapper=True, policy_text=None):
        self.dir = os.path.join(root, "install")
        os.makedirs(self.dir)
        self.exe = os.path.join(self.dir, "process-tree-guard.exe")
        shutil.copyfile(drv.locate("full"), self.exe)
        self.wrapper = os.path.join(self.dir, "process-tree-guard-auto.exe")
        if with_wrapper:
            shutil.copyfile(drv.locate("full"), self.wrapper)
        if policy_text is not None:
            with open(os.path.join(root, "POLICY.md"), "wb") as handle:
                handle.write(policy_text.encode("utf-8"))
        self.exe_spelling = drv.spelling(self.exe)
        self.wrapper_spelling = drv.spelling(self.wrapper)

    def hook(self, vendor, payload=b""):
        return drv.run([self.exe, "hook", vendor], stdin=payload)


@needs_guard
class F8ContextVendorTests(unittest.TestCase):
    POLICY = "WORKED EXAMPLE POLICY\nsecond line, with \"quotes\"."

    def test_cursor_context_prints_the_installed_policy(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp, policy_text=self.POLICY)
            exit_code, out, _, _ = install.hook("cursor-context")
            self.assertEqual(exit_code, 0)
            self.assertEqual(json.loads(out),
                             {"additional_context": self.POLICY})

    def test_antigravity_context_prints_the_installed_policy(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp, policy_text=self.POLICY)
            exit_code, out, _, _ = install.hook("antigravity-context")
            self.assertEqual(exit_code, 0)
            self.assertEqual(
                json.loads(out),
                {"injectSteps": [{"systemMessage":
                                   {"systemMessage": self.POLICY}}]})

    def test_context_vendors_fall_back_when_no_policy_is_installed(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp, policy_text=None)
            exit_code, out, _, _ = install.hook("cursor-context")
            self.assertEqual(exit_code, 0)
            self.assertEqual(json.loads(out),
                             {"additional_context": POLICY_FALLBACK})
            exit_code, out, _, _ = install.hook("antigravity-context")
            self.assertEqual(exit_code, 0)
            answer = json.loads(out)
            self.assertEqual(
                answer["injectSteps"][0]["systemMessage"]["systemMessage"],
                POLICY_FALLBACK)

    def test_context_shape_on_the_staged_package(self):
        # Whatever policy the staged layout resolves, the answer is
        # one JSON object of the documented shape carrying a string.
        exit_code, out, _, _ = drv.guard(["hook", "cursor-context"])
        self.assertEqual(exit_code, 0)
        answer = json.loads(out)
        self.assertIsInstance(answer.get("additional_context"), str)
        self.assertTrue(answer["additional_context"])


@needs_guard
class F8AntigravityTests(unittest.TestCase):
    """Worked examples for the rewriting vendor: a plain
    run_command is rewritten to the automatic wrapper beside the
    executable, an already-wrapped command passes unchanged, other
    tools are allowed untouched, and malformed payloads are denied
    — every outcome a JSON decision at exit code 0."""

    def call(self, command, name="run_command"):
        return json.dumps(
            {"toolCall": {"name": name, "args": {"CommandLine": command}},
             "stepIdx": 1}).encode()

    def wrapped(self, install, command):
        b64 = base64.b64encode(command.encode()).decode()
        return f"{install.wrapper_spelling} --command-b64 {b64}"

    def test_antigravity_rewrites_a_plain_run_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp)
            exit_code, out, _, _ = install.hook("antigravity",
                                                self.call("echo hi"))
            self.assertEqual(exit_code, 0)
            answer = json.loads(out)
            self.assertEqual(answer["decision"], "allow")
            line = answer["overwrite"]["CommandLine"]
            self.assertEqual(line, self.wrapped(install, "echo hi"))
            payload = line.rsplit(" ", 1)[1]
            self.assertEqual(base64.b64decode(payload), b"echo hi")

    def test_antigravity_passes_an_already_wrapped_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp)
            command = self.wrapped(install, "echo hi")
            exit_code, out, _, _ = install.hook("antigravity",
                                                self.call(command))
            self.assertEqual(exit_code, 0)
            answer = json.loads(out)
            self.assertEqual(answer, {"decision": "allow"})

    def test_antigravity_allows_other_tools_untouched(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp)
            payload = json.dumps(
                {"toolCall": {"name": "view_file", "args": {}}}).encode()
            exit_code, out, _, _ = install.hook("antigravity", payload)
            self.assertEqual(exit_code, 0)
            self.assertEqual(json.loads(out), {"decision": "allow"})

    def test_antigravity_denies_a_malformed_payload(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp)
            exit_code, out, _, _ = install.hook("antigravity", b"{not json")
            self.assertEqual(exit_code, 0)
            self.assertEqual(json.loads(out)["decision"], "deny")

    def test_antigravity_denies_when_the_wrapper_is_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp, with_wrapper=False)
            exit_code, out, _, _ = install.hook("antigravity",
                                                self.call("echo hi"))
            self.assertEqual(exit_code, 0)
            answer = json.loads(out)
            self.assertEqual(answer["decision"], "deny")
            self.assertIn("process-tree-guard-auto.exe", answer["reason"])


@needs_guard
class F8PowershellAutoTests(unittest.TestCase):
    """Worked examples for the powershell-auto vendor: a plain
    command is wrapped as an `ask` (or `allow` under
    bypassPermissions), an already-wrapped command passes with an
    empty answer, and an unclassifiable payload is denied — all at
    exit code 0, in the PreToolUse decision form."""

    def wrapped(self, install, command):
        b64 = base64.b64encode(command.encode()).decode()
        return (f"& '{install.wrapper_spelling}' --shell powershell "
                f"--command-b64 {b64}; exit $LASTEXITCODE")

    def payload(self, command, bypass=False):
        data = {"tool_input": {"command": command}}
        if bypass:
            data["permission_mode"] = "bypassPermissions"
        return json.dumps(data).encode()

    def decision(self, install, payload):
        exit_code, out, _, _ = install.hook("powershell-auto", payload)
        self.assertEqual(exit_code, 0)
        return json.loads(out)["hookSpecificOutput"]

    def test_powershell_auto_wraps_a_plain_command_as_ask(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp)
            answer = self.decision(install, self.payload("echo hi"))
            self.assertEqual(answer["permissionDecision"], "ask")
            updated = answer["updatedInput"]
            self.assertEqual(updated["command"],
                             self.wrapped(install, "echo hi"))

    def test_powershell_auto_allows_under_bypass_permissions(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp)
            answer = self.decision(
                install, self.payload("echo hi", bypass=True))
            self.assertEqual(answer["permissionDecision"], "allow")
            self.assertEqual(answer["updatedInput"]["command"],
                             self.wrapped(install, "echo hi"))

    def test_powershell_auto_passes_an_already_wrapped_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp)
            command = self.wrapped(install, "echo hi")
            exit_code, out, _, _ = install.hook(
                "powershell-auto", self.payload(command))
            self.assertEqual(exit_code, 0)
            self.assertEqual(json.loads(out), {})

    def test_powershell_auto_denies_an_unclassifiable_payload(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(tmp)
            answer = self.decision(install, b"{not json")
            self.assertEqual(answer["permissionDecision"], "deny")
            self.assertNotIn("updatedInput", answer)


# -------------------------------------------------- F9 non-Unicode

def _quote_arg(arg):
    out = ['"']
    backslashes = 0
    for ch in arg:
        if ch == "\\":
            backslashes += 1
            continue
        if ch == '"':
            out.append("\\" * (backslashes * 2 + 1))
            out.append('"')
        else:
            out.append("\\" * backslashes)
            out.append(ch)
        backslashes = 0
    out.append("\\" * (backslashes * 2))
    out.append('"')
    return "".join(out)


def run_raw_argv(argv, deadline_s=30.0):
    """Run argv on Windows with the command line built as a raw
    UTF-16 buffer, so an argument can carry a code unit no Python
    string API will pass through subprocess (a lone surrogate —
    the non-Unicode argument this family exists for). The buffer
    construction is plain byte work; the execution is Windows-only.
    Returns (exit_code, stdout_bytes, stderr_bytes) under a finite
    deadline."""
    import ctypes
    from ctypes import wintypes

    line = " ".join(_quote_arg(arg) for arg in argv)
    raw = line.encode("utf-16-le", "surrogatepass") + b"\x00\x00"
    buffer = ctypes.create_string_buffer(raw, len(raw))
    command_line = ctypes.cast(buffer, ctypes.POINTER(ctypes.c_wchar))

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

    class SecurityAttributes(ctypes.Structure):
        _fields_ = [("length", wintypes.DWORD),
                    ("descriptor", ctypes.c_void_p),
                    ("inherit", wintypes.BOOL)]

    class StartupInfo(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("reserved", ctypes.c_wchar_p),
                    ("desktop", ctypes.c_wchar_p), ("title", ctypes.c_wchar_p),
                    ("x", wintypes.DWORD), ("y", wintypes.DWORD),
                    ("x_size", wintypes.DWORD), ("y_size", wintypes.DWORD),
                    ("x_chars", wintypes.DWORD), ("y_chars", wintypes.DWORD),
                    ("fill", wintypes.DWORD), ("flags", wintypes.DWORD),
                    ("show", wintypes.WORD), ("reserved2_len", wintypes.WORD),
                    ("reserved2", ctypes.c_void_p),
                    ("std_input", wintypes.HANDLE),
                    ("std_output", wintypes.HANDLE),
                    ("std_error", wintypes.HANDLE)]

    class ProcessInfo(ctypes.Structure):
        _fields_ = [("process", wintypes.HANDLE), ("thread", wintypes.HANDLE),
                    ("pid", wintypes.DWORD), ("tid", wintypes.DWORD)]

    attrs = SecurityAttributes(ctypes.sizeof(SecurityAttributes), None, True)
    pipes = {}
    for name in ("out", "err", "in"):
        read_end = wintypes.HANDLE()
        write_end = wintypes.HANDLE()
        if not kernel32.CreatePipe(ctypes.byref(read_end),
                                   ctypes.byref(write_end),
                                   ctypes.byref(attrs), 0):
            raise ctypes.WinError(ctypes.get_last_error())
        pipes[name] = (read_end, write_end)
    startup = StartupInfo()
    startup.cb = ctypes.sizeof(StartupInfo)
    startup.flags = 0x100  # STARTF_USESTDHANDLES
    startup.std_input = pipes["in"][0]
    startup.std_output = pipes["out"][1]
    startup.std_error = pipes["err"][1]
    info = ProcessInfo()
    if not kernel32.CreateProcessW(
            None, command_line, None, None, True, 0, None, None,
            ctypes.byref(startup), ctypes.byref(info)):
        raise ctypes.WinError(ctypes.get_last_error())
    for _read, write in pipes.values():
        kernel32.CloseHandle(write)
    # Exactly-once ownership: this sweep owns every write end, and
    # the finally sweep below owns every read end — including
    # pipes["in"][0], whose second, early close used to sit here
    # and was then closed again by that sweep (a double close can
    # land on a recycled, unrelated handle). The early close is
    # not load-bearing: nothing between here and the finally reads
    # or waits on that end — the child's stdin EOF follows the
    # write end closed above, and the wait and the captures use
    # the process handle and the out/err read ends — so its single
    # close is the finally sweep's, still after the spawn.
    try:
        wait = kernel32.WaitForSingleObject(
            info.process, int(deadline_s * 1000))
        if wait != 0:
            kernel32.TerminateProcess(info.process, 1)
            raise AssertionError(
                f"raw invocation exceeded its {deadline_s:g} s deadline")
        code = wintypes.DWORD()
        kernel32.GetExitCodeProcess(info.process, ctypes.byref(code))
        captured = {}
        for name in ("out", "err"):
            chunks = []
            chunk = ctypes.create_string_buffer(65536)
            read = wintypes.DWORD()
            while kernel32.ReadFile(pipes[name][0], chunk, 65536,
                                    ctypes.byref(read), None) and read.value:
                chunks.append(chunk.raw[:read.value])
            captured[name] = b"".join(chunks)
        return code.value, captured["out"], captured["err"]
    finally:
        for read, _write in pipes.values():
            kernel32.CloseHandle(read)
        kernel32.CloseHandle(info.thread)
        kernel32.CloseHandle(info.process)


class F9PipeOwnershipTests(unittest.TestCase):
    """Source shape, anywhere-tier: run_raw_argv closes each pipe
    handle it creates in exactly one place. The helper itself is
    Windows-tier in execution, but its handle ownership is a
    property of its source and is checked here: the post-spawn
    sweep closing the loop's `write` variable covers every pipe's
    write end, the finally sweep closing `read` covers every
    read end, and no other CloseHandle site may name a pipe end —
    a direct close such as pipes["in"][0] beside either sweep is
    a second close of a handle the sweep already owns."""

    def test_each_pipe_handle_is_closed_in_exactly_one_site(self):
        import ast
        import inspect

        helper = ast.parse(inspect.getsource(run_raw_argv))
        sites = {}
        for node in ast.walk(helper):
            if not isinstance(node, ast.Call) or not node.args:
                continue
            if not (isinstance(node.func, ast.Attribute)
                    and node.func.attr == "CloseHandle"):
                continue
            arg = node.args[0]
            covered = []
            if isinstance(arg, ast.Name) and arg.id in ("read", "write"):
                # A sweep over pipes.values(): `read` / `write` is
                # that end of every pipe the helper created.
                end = 0 if arg.id == "read" else 1
                covered = [(name, end) for name in ("out", "err", "in")]
            elif (isinstance(arg, ast.Subscript)
                    and isinstance(arg.value, ast.Subscript)
                    and isinstance(arg.value.value, ast.Name)
                    and arg.value.value.id == "pipes"
                    and isinstance(arg.value.slice, ast.Constant)
                    and isinstance(arg.slice, ast.Constant)):
                # A direct pipes[name][end] close: that one end.
                covered = [(arg.value.slice.value, arg.slice.value)]
            for end_key in covered:
                sites[end_key] = sites.get(end_key, 0) + 1
        expected = {(name, end): 1
                    for name in ("out", "err", "in") for end in (0, 1)}
        self.assertEqual(sites, expected)


@needs_guard
@needs_windows
class F9NonUnicodeArgvTests(unittest.TestCase):
    """An argument that is not valid Unicode is a guard error
    (125, `arguments must be valid Unicode`) — never a lossy
    conversion, never a run. The bad argument carries a lone
    UTF-16 surrogate, constructed as raw units in run_raw_argv."""

    BAD = "ptg-\ud800.exe"

    def test_non_unicode_program_argument_is_a_guard_error(self):
        exe = drv.locate("full")
        code, _, err = run_raw_argv([exe, "--timeout", "5", "--", self.BAD])
        self.assertEqual(code, 125)
        self.assertIn(b"arguments must be valid Unicode", err)

    def test_non_unicode_first_argument_is_a_guard_error(self):
        exe = drv.locate("full")
        code, _, err = run_raw_argv([exe, self.BAD])
        self.assertEqual(code, 125)
        self.assertIn(b"arguments must be valid Unicode", err)

    def test_unicode_argument_reaches_the_program_search(self):
        # Control: a non-ASCII but valid Unicode program name is
        # converted fine and fails later, at the program search —
        # a 125 whose text is not the Unicode refusal.
        exe = drv.locate("full")
        code, _, err = run_raw_argv(
            [exe, "--timeout", "5", "--", "ptg-t\u00f6\u00f6l.exe"])
        self.assertEqual(code, 125)
        self.assertNotIn(b"valid Unicode", err)


# ---------------------------------------------------- F10 resolver

@needs_guard
@needs_windows
class F10ResolverOrderTests(unittest.TestCase):
    """The documented resolver order (the tree's `engine::which`):
    the current folder first (unless
    NoDefaultCurrentDirectoryInExePath is set), then each PATH
    folder in order; within a folder, the name as named when it
    already carries a PATHEXT suffix, else the name with each
    PATHEXT extension in order. The field is laid out with two
    distinguishable programs — the test helper (exits with the
    code its `exit` argument names) and the deadline role (a
    usage error, 125, on the same arguments) — copied under one
    probe name into controlled folders."""

    @classmethod
    def setUpClass(cls):
        cls.root = tempfile.mkdtemp(prefix="ptg-resolver-")
        cls.dir_a = os.path.join(cls.root, "a")
        cls.dir_b = os.path.join(cls.root, "b")
        cls.dir_c = os.path.join(cls.root, "c")
        for folder in (cls.dir_a, cls.dir_b, cls.dir_c):
            os.makedirs(folder)
        shutil.copyfile(drv.locate("testkit"),
                        os.path.join(cls.dir_a, "ptgprobe.exe"))
        shutil.copyfile(drv.locate("deadline"),
                        os.path.join(cls.dir_b, "ptgprobe.exe"))
        shutil.copyfile(drv.locate("testkit"),
                        os.path.join(cls.dir_c, "ptgprobe.exe"))
        shutil.copyfile(drv.locate("deadline"),
                        os.path.join(cls.dir_c, "ptgprobe.COM"))

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.root, ignore_errors=True)

    def probe(self, name="ptgprobe", path=None, pathext=".EXE", extra=None):
        env = {}
        if path is not None:
            set_env_var(env, "PATH", path)
        if pathext is not None:
            set_env_var(env, "PATHEXT", pathext)
        if extra:
            for key, value in extra.items():
                set_env_var(env, key, value)
        exit_code, _, err, _ = drv.guard(
            ["--timeout", "10", "--", name, "exit", "7"], env_extra=env)
        return exit_code, err

    def test_path_folders_are_searched_in_order(self):
        code, _ = self.probe(path=self.dir_a + ";" + self.dir_b)
        self.assertEqual(code, 7)          # the helper in A wins
        code, _ = self.probe(path=self.dir_b + ";" + self.dir_a)
        self.assertEqual(code, 125)        # the deadline role in B wins

    def test_current_folder_is_searched_before_path(self):
        previous = os.getcwd()
        os.chdir(self.dir_b)
        try:
            code, _ = self.probe(path=self.dir_a)
        finally:
            os.chdir(previous)
        self.assertEqual(code, 125)        # B's copy, from the cwd

    def test_no_default_current_directory_removes_the_current_folder(self):
        previous = os.getcwd()
        os.chdir(self.dir_b)
        try:
            code, _ = self.probe(
                path=self.dir_a,
                extra={"NoDefaultCurrentDirectoryInExePath": "1"})
        finally:
            os.chdir(previous)
        self.assertEqual(code, 7)          # cwd skipped; A wins

    def test_pathext_membership_gates_the_search(self):
        code, err = self.probe(path=self.dir_a, pathext=".BAT;.CMD")
        self.assertEqual(code, 125)
        self.assertIn(b"PROCESS_GUARD_ERROR", err)
        code, _ = self.probe(path=self.dir_a, pathext=".EXE")
        self.assertEqual(code, 7)

    def test_pathext_extensions_are_tried_in_order(self):
        # Both candidates are the same kind of program file under
        # different extensions (the loader keys on content); the
        # PATHEXT order alone decides which one runs.
        code, _ = self.probe(path=self.dir_c, pathext=".COM;.EXE")
        self.assertEqual(code, 125)        # the .COM copy (deadline role)
        code, _ = self.probe(path=self.dir_c, pathext=".EXE;.COM")
        self.assertEqual(code, 7)          # the .EXE copy (the helper)

    def test_name_with_a_pathext_suffix_is_probed_as_named(self):
        code, _ = self.probe(name="ptgprobe.exe", path=self.dir_a,
                             pathext=".EXE")
        self.assertEqual(code, 7)
        # With .EXE absent from PATHEXT the spelled name is not
        # "as named": the search appends the listed extensions to
        # it and finds nothing.
        code, _ = self.probe(name="ptgprobe.exe", path=self.dir_a,
                             pathext=".COM")
        self.assertEqual(code, 125)


class F10ResolverEnvCaseTests(unittest.TestCase):
    """The resolver field's environment, observed in a real child
    on any platform: the child is this Python interpreter reporting
    its own environment, spawned through guard_driver.run — the
    merge F10's probes pass through — with this process's
    environment replaced by a controlled one. A Windows parent
    environment conventionally spells the variable `Path`; the
    environment handed to the child must carry exactly one
    path-spelled entry, holding the laid-out field, or the guard's
    case-insensitive PATH read can return the inherited entry
    instead of the field the test laid out."""

    REPORT = ("import json, os, sys; "
              "sys.stdout.write(json.dumps(dict(os.environ)))")

    def child_env(self, parent_env, extra):
        with mock.patch.dict(os.environ, parent_env, clear=True):
            exit_code, out, err, _ = drv.run(
                [sys.executable, "-c", self.REPORT], env_extra=extra)
        self.assertEqual(exit_code, 0, err)
        return json.loads(out)

    def path_entries(self, env):
        return [(key, env[key]) for key in matching_keys(env, "PATH")]

    def test_child_env_replaces_a_Path_spelled_parent_entry(self):
        env = self.child_env(
            {"Path": "inherited-value", "PTG_MARKER": "kept"},
            {"PATH": "controlled-value"})
        self.assertEqual(self.path_entries(env),
                         [("PATH", "controlled-value")])
        self.assertEqual(env.get("PTG_MARKER"), "kept")

    def test_child_env_collapses_Path_and_PATH_parent_variants(self):
        env = self.child_env(
            {"Path": "inherited-a", "PATH": "inherited-b",
             "PTG_MARKER": "kept"},
            {"PATH": "controlled-value"})
        self.assertEqual(self.path_entries(env),
                         [("PATH", "controlled-value")])
        self.assertEqual(env.get("PTG_MARKER"), "kept")

    def test_child_env_without_a_parent_variant_is_unchanged(self):
        # Negative control: with no pre-existing variant key the
        # override simply lands, and unrelated entries pass
        # through untouched.
        env = self.child_env({"PTG_MARKER": "kept"},
                             {"PATH": "controlled-value"})
        self.assertEqual(self.path_entries(env),
                         [("PATH", "controlled-value")])
        self.assertEqual(env.get("PTG_MARKER"), "kept")


if __name__ == "__main__":
    unittest.main()
