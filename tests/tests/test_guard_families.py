# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Guard families, driving the built programs through their CLI
surfaces. These run on Windows against the built package; here they
are skipped when the binaries are absent, and the negative-control
family below proves the absence is detected for the right reason.

Families (see FAILURE-TREE.md for the branch each covers):
  F1 fast path        F4 CLI and error contract
  F2 kill path        F5 extendable runs
  F3 hook verdicts    F6 automatic mode and roles
"""

import base64
import json
import os
import sys
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import guard_driver as drv  # noqa: E402

PRESENT = drv.guard_present()
needs_guard = unittest.skipUnless(
    PRESENT, "guard binaries not built here (Windows package); see README tiers")

# ------------------------------------------------------- F2 partial output

PARTIAL_OUTPUT_LINE = "line-before-kill"
PARTIAL_OUTPUT_TIMEOUT_S = 1
PARTIAL_OUTPUT_SLEEP_S = 30


def partial_output_command():
    """The shell command for F2's partial-output fixture: write a
    line, flush it, then sleep far past the guard's deadline, so
    the only way the run ends is the guard's kill at the deadline
    and the line in the captured output is genuinely pre-kill
    output. The command text is plain PowerShell."""
    return ("[System.Console]::WriteLine('line-before-kill'); "
            "[System.Console]::Out.Flush(); "
            f"Start-Sleep -Seconds {PARTIAL_OUTPUT_SLEEP_S}")


def partial_output_shell_args():
    """The guard invocation for the partial-output fixture, in the
    guard's own shell form (the shape F1's
    `test_shell_form_runs_the_decoded_command` uses: `--timeout`,
    `--shell powershell`, `--command-b64`). Construction runs
    anywhere; only the verdict needs the guard.

    The payload is the Base64 of the command's UTF-8 bytes — the
    encoding the guard's `--command-b64` decode requires (UTF-8,
    nonempty, no NUL; see the tree's `decode_command`) and the
    encoding every other shell-form use in this suite uses
    (F1, F3/F8, the scanner corpus's `ZWNobyBPSw==`). A UTF-16LE
    spelling would decode to NUL-bearing bytes the guard refuses
    with 125, so it is used nowhere in this suite."""
    command = partial_output_command()
    encoded = base64.b64encode(command.encode("utf-8")).decode()
    return {
        "command": command,
        "encoded": encoded,
        "args": ["--timeout", str(PARTIAL_OUTPUT_TIMEOUT_S),
                 "--shell", "powershell", "--command-b64", encoded],
        "line": PARTIAL_OUTPUT_LINE.encode("utf-8"),
        "timeout_s": PARTIAL_OUTPUT_TIMEOUT_S,
        "sleep_s": PARTIAL_OUTPUT_SLEEP_S,
    }


class PartialOutputConstructionTests(unittest.TestCase):
    """Runs anywhere: the partial-output fixture's payload and
    argument construction, unit-checked the way F7's cap
    construction is checked in `CapConstructionTests`."""

    def test_partial_output_payload_construction(self):
        fixture = partial_output_shell_args()
        # The payload round-trips as UTF-8 with no NUL, exactly
        # what the guard's --command-b64 decode accepts.
        decoded = base64.b64decode(
            fixture["encoded"], validate=True).decode("utf-8")
        self.assertEqual(decoded, fixture["command"])
        self.assertNotIn("\x00", decoded)
        # The argument construction is the shell form, at a 1 s
        # deadline, carrying exactly that payload.
        self.assertEqual(
            fixture["args"],
            ["--timeout", "1", "--shell", "powershell",
             "--command-b64", fixture["encoded"]])
        self.assertEqual(fixture["timeout_s"], 1)

    def test_partial_output_fixture_writes_flushes_then_outlives(self):
        fixture = partial_output_shell_args()
        command = fixture["command"]
        line_at = command.index(PARTIAL_OUTPUT_LINE)
        flush_at = command.index("Flush")
        sleep_at = command.index("Start-Sleep")
        # Write, then flush, then sleep — in that order.
        self.assertLess(line_at, flush_at)
        self.assertLess(flush_at, sleep_at)
        self.assertIn(f"-Seconds {PARTIAL_OUTPUT_SLEEP_S}", command)
        # The fixture outlives its deadline by a wide margin, so
        # a return can only be the guard's kill, never the
        # child's own exit: sleep 30 s against a 1 s deadline,
        # well under the driver's 60 s deadline only because
        # the guard kills it.
        self.assertGreater(fixture["sleep_s"], fixture["timeout_s"])
        self.assertEqual(fixture["line"],
                         PARTIAL_OUTPUT_LINE.encode("utf-8"))


# ------------------------------------------------- F1 exact output

OUTPUT_LINE_ONE = "output-line-one"
OUTPUT_LINE_TWO = "output-line-two"
OUTPUT_STDERR_LINE = "error-stream"


def output_passthrough_fixture():
    """The child and the expected bytes for F1's exact-output
    test, built from testkit's `relay` mode (the `"relay"` arm of
    testkit.rs, read in full): relay reads its standard input to
    the end, writes those bytes plus exactly one newline byte to
    stdout, writes its fixed `error-stream` line to stderr, and
    exits with the code it is named (`relay 0` here). The driver
    feeds the guard the fixture's stdin bytes and the guard
    passes its standard input to the child — the same relay
    shape the campaign's fast child uses and the tree's own
    stream check drives — so the child's streams are known by
    construction: feeding it two ordered lines with no final
    newline makes its stdout exactly those two lines, each
    newline-terminated, in that order, and its stderr exactly
    the one fixed line. The expected bytes are fixed data here,
    identical in shape to what the child emits by construction.
    Construction runs anywhere; only the verdict needs the
    guard."""
    stdin = f"{OUTPUT_LINE_ONE}\n{OUTPUT_LINE_TWO}".encode("utf-8")
    return {
        "stdin": stdin,
        "child_args": ["relay", "0"],
        "expected_stdout": stdin + b"\n",
        "expected_stderr": f"{OUTPUT_STDERR_LINE}\n".encode("utf-8"),
    }


def assert_output_exact(case, out, err, fixture=None):
    """The exact-output assertion, shared by F1's Windows test
    and the anywhere-tier exercise below: the guard's captured
    stdout must equal the child's expected stdout bytes exactly
    and its captured stderr the child's expected stderr bytes
    exactly — byte for byte, both streams, in order. A capture
    with stderr suppressed, the stdout lines reversed, a line
    truncated, or any byte added or altered fails one of the
    two equalities by construction."""
    fixture = fixture or output_passthrough_fixture()
    case.assertEqual(out, fixture["expected_stdout"])
    case.assertEqual(err, fixture["expected_stderr"])


class OutputPassthroughAssertionTests(unittest.TestCase):
    """Runs anywhere: the exact-output fixture's construction and
    the assertion logic F1's Windows test applies to the guard's
    captures, exercised here against fabricated captures. The
    fabricated variants demonstrate that the assertion itself
    rejects them; a variant guard (or wrapper) failing the real
    F1 test is a Windows-run exercise (see that test)."""

    def test_output_fixture_construction(self):
        fixture = output_passthrough_fixture()
        self.assertEqual(fixture["child_args"], ["relay", "0"])
        self.assertEqual(fixture["stdin"],
                         b"output-line-one\noutput-line-two")
        # Relay appends exactly one newline to the bytes it is
        # fed, so the expected stdout is the stdin bytes plus
        # that newline: two lines, in the fixture's order.
        self.assertEqual(fixture["expected_stdout"],
                         fixture["stdin"] + b"\n")
        self.assertEqual(fixture["expected_stdout"],
                         b"output-line-one\noutput-line-two\n")
        self.assertEqual(fixture["expected_stdout"].splitlines(),
                         [b"output-line-one", b"output-line-two"])
        self.assertGreaterEqual(
            len(fixture["expected_stdout"].splitlines()), 2)
        # The stderr expectation is exactly one fixed line.
        self.assertEqual(fixture["expected_stderr"],
                         b"error-stream\n")
        self.assertEqual(fixture["expected_stderr"].splitlines(),
                         [b"error-stream"])

    def test_exact_captures_pass_the_output_assertion(self):
        fixture = output_passthrough_fixture()
        assert_output_exact(self, fixture["expected_stdout"],
                            fixture["expected_stderr"], fixture)

    def test_suppressed_stderr_fails_the_output_assertion(self):
        fixture = output_passthrough_fixture()
        with self.assertRaises(AssertionError):
            assert_output_exact(self, fixture["expected_stdout"],
                                b"", fixture)

    def test_reversed_stdout_lines_fail_the_output_assertion(self):
        fixture = output_passthrough_fixture()
        reversed_out = b"output-line-two\noutput-line-one\n"
        with self.assertRaises(AssertionError):
            assert_output_exact(self, reversed_out,
                                fixture["expected_stderr"], fixture)

    def test_truncated_or_altered_bytes_fail_the_output_assertion(self):
        fixture = output_passthrough_fixture()
        variants = (
            # Stdout truncated after the first line.
            (b"output-line-one\n", fixture["expected_stderr"]),
            # An extra byte appended to stdout.
            (fixture["expected_stdout"] + b"x",
             fixture["expected_stderr"]),
            # The stderr line altered.
            (fixture["expected_stdout"], b"error-stream altered\n"),
        )
        for out, err in variants:
            with self.subTest(out=out, err=err):
                with self.assertRaises(AssertionError):
                    assert_output_exact(self, out, err, fixture)


def read_pid_file(path, timeout=10.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with open(path, encoding="utf-8") as handle:
                return int(handle.read().strip())
        except (OSError, ValueError):
            time.sleep(0.05)
    raise AssertionError(f"pid file {path} never appeared")


class NegativeControlTests(unittest.TestCase):
    """Runs anywhere: the harness must detect a missing binary as a
    missing binary, and a stub child that misbehaves as a failure."""

    def test_missing_binary_raises_binary_missing(self):
        if PRESENT:
            self.skipTest("binaries present; absence control not applicable")
        with self.assertRaises(drv.BinaryMissing):
            drv.locate("full")

    def test_driver_deadline_is_finite(self):
        argv = [sys.executable, "-c", "import time; time.sleep(30)"]
        with self.assertRaises(AssertionError):
            drv.run(argv, deadline=1.0)

    def test_build_dirs_never_fall_back_to_the_tree_target_folder(self):
        # The families certify the staged package (build/, or the
        # folder PTG_SUITE_BUILD names): a binary in the guard
        # tree's cargo target folder was built by another route
        # from an unrecorded tree state, and locating one there
        # would let the families pass against a guard the
        # campaign never staged. No build directory the driver
        # searches may be a cargo target folder.
        for folder in drv.build_dirs():
            parts = folder.replace("\\", "/").split("/")
            self.assertNotIn("target", parts, folder)


@needs_guard
class F1FastPathTests(unittest.TestCase):
    def test_exit_codes_preserved(self):
        for code in (0, 1, 7, 42, 255):
            exit_code, *_ = drv.guard(
                ["--timeout", "10", "--"] + drv.testkit(["exit", str(code)]))
            self.assertEqual(exit_code, code)

    def test_child_output_passes_through(self):
        """Exact passthrough, both streams: the child is testkit
        `relay 0` fed the fixture's two ordered stdin lines, so
        it emits those two lines in order on stdout and its one
        fixed line on stderr (see `output_passthrough_fixture`).
        The guard's captured stdout and stderr must equal the
        child's bytes exactly — order and stderr included, not
        a substring of either. A variant guard (or wrapper)
        that suppresses stderr, or that emits the stdout lines
        in reverse order, must fail this test; that variant
        failure is a Windows-run exercise — this test is
        Windows-tier and skips where the guard is not built,
        while the assertion logic it applies is exercised
        anywhere against fabricated captures by
        `OutputPassthroughAssertionTests` above."""
        fixture = output_passthrough_fixture()
        exit_code, out, err, _ = drv.guard(
            ["--timeout", "10", "--"] + drv.testkit(fixture["child_args"]),
            stdin=fixture["stdin"])
        self.assertEqual(exit_code, 0)
        assert_output_exact(self, out, err, fixture)

    def test_missing_program_is_guard_error_125(self):
        exit_code, _, err, _ = drv.guard(
            ["--timeout", "10", "--", "ptg-no-such-program-9f3b.exe"])
        self.assertEqual(exit_code, 125)
        self.assertIn(b"PROCESS_GUARD_ERROR", err)

    def test_shell_form_runs_the_decoded_command(self):
        import base64
        encoded = base64.b64encode("echo via-shell".encode("utf-8")).decode()
        exit_code, out, _, _ = drv.guard(
            ["--timeout", "10", "--shell", "powershell", "--command-b64", encoded])
        self.assertEqual(exit_code, 0)
        self.assertIn(b"via-shell", out)


@needs_guard
class F2KillPathTests(unittest.TestCase):
    def test_sleeper_killed_at_the_deadline_with_124(self):
        start = time.perf_counter()
        exit_code, _, err, _ = drv.guard(
            ["--timeout", "1", "--"] + drv.testkit(["sleep", "30"]))
        elapsed = time.perf_counter() - start
        self.assertEqual(exit_code, 124)
        self.assertIn(b"PROCESS_TIMEOUT", err)
        self.assertLess(elapsed, 15.0)

    def test_descendant_tree_dies_with_the_child(self):
        with tempfile.TemporaryDirectory() as tmp:
            marker = os.path.join(tmp, "grandchild.pid")
            child = drv.testkit(
                ["spawn", "0", "30", drv.locate("testkit"),
                 "mark", marker, "30"])
            exit_code, *_ = drv.guard(["--timeout", "1", "--"] + child)
            self.assertEqual(exit_code, 124)
            grandchild = read_pid_file(marker)
            for _ in range(100):
                if not drv.pid_alive(grandchild):
                    break
                time.sleep(0.05)
            self.assertFalse(drv.pid_alive(grandchild),
                             "grandchild survived the tree kill")

    def test_partial_output_survives_the_kill(self):
        fixture = partial_output_shell_args()
        start = time.perf_counter()
        exit_code, out, _, _ = drv.guard(fixture["args"])
        elapsed = time.perf_counter() - start
        # All three: the kill happened (124, not the child's own
        # exit), the pre-kill line survived in the captured
        # stdout, and the elapsed time is a kill at the deadline
        # — at least the timeout, well under the driver's 60 s
        # deadline and well under the fixture's 30 s sleep.
        self.assertEqual(exit_code, 124)
        self.assertIn(fixture["line"], out)
        self.assertGreaterEqual(elapsed, fixture["timeout_s"])
        self.assertLess(elapsed, 15.0)


@needs_guard
class F3HookTests(unittest.TestCase):
    def hook(self, vendor, payload, role="full"):
        if role == "gate":
            return drv.role("gate", [vendor], stdin=payload)
        return drv.guard(["hook", vendor], stdin=payload)

    def envelope(self, command):
        return json.dumps({"tool_input": {"command": command}}).encode()

    def test_pretooluse_allows_a_guarded_command(self):
        exe = drv.locate("full")
        import base64
        b64 = base64.b64encode(b"echo hi").decode()
        command = f"{exe} --timeout 60 --shell powershell --command-b64 {b64}"
        exit_code, out, _, _ = self.hook("pretooluse", self.envelope(command))
        self.assertEqual(exit_code, 0)
        self.assertEqual(out.strip(), b"{}")

    def test_pretooluse_denies_an_unguarded_command_with_2(self):
        exit_code, *_ = self.hook("pretooluse", self.envelope("echo hi"))
        self.assertEqual(exit_code, 2)

    def test_pretooluse_denies_malformed_payload_with_2(self):
        exit_code, *_ = self.hook("pretooluse", b"{not json")
        self.assertEqual(exit_code, 2)

    def test_cursor_allow_shape(self):
        exe = drv.locate("full")
        import base64
        b64 = base64.b64encode(b"echo hi").decode()
        command = f"{exe} --timeout 60 --shell powershell --command-b64 {b64}"
        payload = json.dumps({"command": command}).encode()
        exit_code, out, _, _ = self.hook("cursor", payload)
        self.assertEqual(exit_code, 0)
        self.assertEqual(json.loads(out), {"permission": "allow"})

    def test_unknown_vendor_is_a_usage_error_never_silent(self):
        exit_code, _, err, _ = drv.guard(["hook", "no-such-vendor"],
                                         stdin=b"{}")
        self.assertEqual(exit_code, 125)
        self.assertTrue(err.strip())

    def test_gate_role_matches_full_role_on_allow_and_deny(self):
        import base64
        b64 = base64.b64encode(b"echo hi").decode()
        # Allow case: each role is named by its own binary's envelope,
        # as a host would spell it; the verdicts and answer bytes must
        # still match across roles. Deny case: one shared payload.
        answers = {}
        for role in ("gate", "full"):
            exe = drv.locate(role)
            command = (f"{exe} --timeout 60 --shell powershell "
                       f"--command-b64 {b64}")
            code, out, _, _ = self.hook("pretooluse",
                                        self.envelope(command), role=role)
            answers[role] = (code, out)
            self.assertEqual(code, 0, role)
        self.assertEqual(answers["gate"], answers["full"])
        deny = {}
        for role in ("gate", "full"):
            code, out, _, _ = self.hook("pretooluse",
                                        self.envelope("echo hi"), role=role)
            deny[role] = (code, out)
            self.assertEqual(code, 2, role)
        self.assertEqual(deny["gate"], deny["full"])


@needs_guard
class F4CliContractTests(unittest.TestCase):
    def test_no_command_is_125_never_2(self):
        exit_code, *_ = drv.guard(["--timeout", "5"])
        self.assertEqual(exit_code, 125)

    def test_bad_timeouts_are_125(self):
        for value in ("0", "-1", "86401", "abc", "nan", "inf"):
            exit_code, *_ = drv.guard(
                ["--timeout", value, "--"] + drv.testkit(["exit", "0"]))
            self.assertEqual(exit_code, 125, value)

    def test_unknown_option_is_125(self):
        exit_code, *_ = drv.guard(["--bogus", "1"])
        self.assertEqual(exit_code, 125)

    def test_extension_options_need_extendable(self):
        exit_code, _, err, _ = drv.guard(
            ["--max-extension", "60", "--timeout", "5", "--"] +
            drv.testkit(["exit", "0"]))
        self.assertEqual(exit_code, 125)
        self.assertIn(b"need --extendable", err)

    def test_help_is_zero(self):
        for role_name, args in (("full", ["--help"]), ("deadline", ["--help"]),
                                ("contain", ["--help"]), ("gate", ["--help"])):
            exit_code, out, _, _ = drv.role(role_name, args)
            self.assertEqual(exit_code, 0, role_name)
            self.assertIn(b"usage:", out)

    def test_bat_program_is_refused_like_cmd(self):
        """The .bat half of the .cmd/.bat refusal (round-26 F7):
        constructed exactly like the in-tree .cmd case (G27 — a
        script file written into a fresh folder and named as the
        program), with the extension changed to .bat. The guard
        must refuse it at 125 with the message naming .cmd/.bat,
        and the script must not run. Windows-tier: this test is
        gated by needs_guard like its family, skips where the
        built package is absent (its skip is counted in the
        suite's skip census with that reason), and executes on
        the ready-to-fly Windows run — the revival path
        FAILURE-TREE.md section 4 names for this branch."""
        with tempfile.TemporaryDirectory() as tmp:
            script = os.path.join(tmp, "x.bat")
            with open(script, "w", encoding="utf-8") as handle:
                handle.write("@echo hi\n")
            exit_code, out, err, _ = drv.guard(
                ["--timeout", "5", "--", script])
            self.assertEqual(exit_code, 125)
            self.assertIn(b".cmd/.bat", err)
            self.assertNotIn(b"hi", out)


@needs_guard
class F5ExtendableTests(unittest.TestCase):
    def test_child_extension_outlives_the_initial_deadline(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = os.path.join(tmp, "extension.log")
            # extend-self SECONDS HOLD: asks for a 5 s hold, then holds 3 s.
            child = drv.testkit(["extend-self", "5", "3"])
            exit_code, *_ = drv.guard(
                ["--extendable", "--extension-log", log, "--timeout", "2",
                 "--"] + child)
            self.assertEqual(exit_code, 0)
            with open(log, encoding="utf-8") as handle:
                self.assertIn("decision=granted", handle.read())

    def test_run_without_extension_ends_at_its_deadline(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = os.path.join(tmp, "extension.log")
            exit_code, *_ = drv.guard(
                ["--extendable", "--extension-log", log, "--timeout", "1",
                 "--"] + drv.testkit(["sleep", "30"]))
            self.assertEqual(exit_code, 124)

    def test_extend_outside_a_run_is_refused(self):
        exit_code, *_ = drv.guard(["extend", "5"])
        self.assertEqual(exit_code, 125)

    def test_contain_outside_a_run_is_refused(self):
        exit_code, *_ = drv.role(
            "contain", ["--timeout", "5", "--"] + drv.testkit(["exit", "0"]))
        self.assertEqual(exit_code, 125)


@needs_guard
class F6AutoAndRoleTests(unittest.TestCase):
    def test_auto_runs_a_program_form_command(self):
        exit_code, *_ = drv.guard(
            ["auto", "--timeout", "10", "--"] + drv.testkit(["exit", "3"]))
        self.assertEqual(exit_code, 3)

    def test_auto_is_never_extendable(self):
        exit_code, _, err, _ = drv.guard(["auto", "--extendable", "echo", "x"])
        self.assertEqual(exit_code, 125)
        self.assertIn(b"never extendable", err)

    def test_deadline_role_has_no_hook_mode(self):
        exit_code, *_ = drv.role("deadline", ["hook", "pretooluse"], stdin=b"{}")
        self.assertEqual(exit_code, 125)


if __name__ == "__main__":
    unittest.main()
