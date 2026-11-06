# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Battery family: the five theory items. The source-audit parts run
anywhere and must be evidenced; the Windows parts must be honestly
blocked off Windows — a case that passed here would be a defect in
the case, because its adoption conditions cannot be evidenced here.
"""

import json
import os
import re
import shutil
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "battery"))
sys.path.insert(0, os.path.join(HERE, "..", "campaign"))

import battery as battery_mod  # noqa: E402
import env_fixtures  # noqa: E402

TREE = os.path.normpath(os.path.join(HERE, "..", ".."))
# Source-audit tests replay the driver-blocked tier on every host. Native
# theory probes run explicitly through battery.py, outside test discovery.
AUDIT_CONTEXT = {"tree": TREE, "platform": sys.platform, "driver_mode": "blocked"}
REPORTS = {case.ident: case.run(AUDIT_CONTEXT) for case in battery_mod.CASES}


def conditions(ident):
    return {c["condition"]: c for c in REPORTS[ident]["conditions"]}


class BatteryShapeTests(unittest.TestCase):
    def test_five_cases_present(self):
        self.assertEqual([c.ident for c in battery_mod.CASES],
                         ["a", "l", "m", "n", "o"])

    def test_no_case_passes_without_windows_evidence(self):
        # No platform exemption: on any platform, a case may reach
        # PASS only from a driver run with its evidence records
        # preserved. With evidence withheld — a simulated win32
        # context in which no driver run is available — every case
        # must stay BLOCKED-ERROR, never PASS; this is the check
        # that catches a missing driver on a Windows run.
        ctx = {"tree": TREE, "platform": "win32", "evidence": "none"}
        for case in battery_mod.CASES:
            rep = case.run(ctx)
            self.assertEqual(rep["verdict"], "BLOCKED-ERROR",
                             case.ident)
        # The source-audit tier cannot establish a native driver pass.
        for ident, rep in REPORTS.items():
            self.assertEqual(rep["verdict"], "BLOCKED-WINDOWS", ident)

    def test_no_case_fails_on_its_audit_parts(self):
        for ident, rep in REPORTS.items():
            for cond in rep["conditions"]:
                self.assertNotEqual(cond["status"], "refuted",
                                    f"{ident}: {cond['condition']}")


class CaseATests(unittest.TestCase):
    def test_baseline_poll_located(self):
        cond = conditions("a")["baseline poll located in source"]
        self.assertEqual(cond["status"], "evidenced")
        self.assertIn("Sleep(10)", cond["detail"])
        self.assertIn("5,000 ms", cond["detail"])


class CaseLTests(unittest.TestCase):
    def test_vars_os_baseline_located(self):
        cond = conditions("l")["vars_os baseline located in source"]
        self.assertEqual(cond["status"], "evidenced")

    def test_env_fixtures_cover_the_differential_shapes(self):
        path = os.path.join(HERE, "..", "fixtures", "env-fixtures.json")
        with open(path, encoding="utf-8") as handle:
            fixtures = {f["name"]: f["entries"] for f in json.load(handle)}
        for name in ("empty", "empty-value", "equals-in-value", "drive-entries",
                     "duplicate-case", "unicode-and-case", "channel-entry"):
            self.assertIn(name, fixtures)
        names = [k for k, _ in fixtures["duplicate-case"]]
        self.assertEqual([n.upper() for n in names], ["PATH", "PATH"])
        # The 500-entry case ships in the parsed set, and it is
        # exactly the deterministic generator's output: removing
        # the generated case from the set turns this test red.
        self.assertIn("five-hundred-entries", fixtures)
        self.assertEqual(len(fixtures["five-hundred-entries"]), 500)
        self.assertEqual(fixtures["five-hundred-entries"],
                         env_fixtures.generate_large_entries())
        loaded = {f["name"]: f["entries"]
                  for f in env_fixtures.load_fixtures()}
        self.assertEqual(len(loaded["five-hundred-entries"]), 500)

    def test_reference_sort_model_is_stable_and_case_folded(self):
        # The adoption semantics the differential must preserve, stated
        # as a reference model of the tree's actual block builder
        # (src/engine/environment.rs): entries are sorted by their
        # upper-case name key with a stable sort, and entries whose
        # keys compare equal are ALL retained in the order vars_os
        # yielded them — the builder deduplicates nothing; the only
        # entries it drops are case-insensitive matches of the one
        # entry being set, which is then appended (and so sorts last
        # among equals). A model that kept one entry per folded name
        # would judge the differential against semantics the tree
        # does not have.
        entries = [("Path", "a"), ("PATH", "b"), ("apple", "c"), ("Zoo", "d")]
        ordered = sorted(entries, key=lambda kv: kv[0].upper())
        self.assertEqual([k.upper() for k, _ in ordered],
                         ["APPLE", "PATH", "PATH", "ZOO"])
        self.assertEqual([k for k, _ in ordered],
                         ["apple", "Path", "PATH", "Zoo"])
        self.assertEqual([v for _, v in ordered], ["c", "a", "b", "d"])


class CaseMTests(unittest.TestCase):
    def test_per_site_audit_evidenced(self):
        cond = conditions("m")["per-site comparison audit (source)"]
        self.assertEqual(cond["status"], "evidenced")
        self.assertEqual(len(battery_mod.PSEUDO_SITES), 4)

    def test_audit_names_the_invalid_handle_collision(self):
        cond = conditions("m")["per-site comparison audit (source)"]
        self.assertIn("INVALID_HANDLE_VALUE", cond["detail"])


class CaseNTests(unittest.TestCase):
    def test_verdict_surface_located(self):
        cond = conditions("n")["verdict surface located in source"]
        self.assertEqual(cond["status"], "evidenced")

    def test_fallback_is_part_of_the_pass_criteria(self):
        cond = conditions("n")["verdict equality across the finish cycle"]
        self.assertIn("pin", cond["detail"])
        self.assertIn("pending", cond["detail"])


class CaseOTests(unittest.TestCase):
    def test_six_item_burden_plus_forged_packet(self):
        conds = conditions("o")
        self.assertEqual(len(conds), 8)  # precedent + six burden items + forged
        forged = conds["forged-packet case (binding conditions)"]
        self.assertIn("wake-only", forged["detail"])
        self.assertIn("authoritative job query", forged["detail"])
        self.assertIn("no reply field", forged["detail"])

    def test_burden_prices_the_port_close(self):
        cond = conditions("o")["port close / disassociation effects"]
        self.assertIn("minimum honest count is 5", cond["detail"])


AUDIT_FILES = (
    "src/engine/wait.rs",
    "src/engine/environment.rs",
    "src/engine/owners.rs",
    "src/cli_process/harden.rs",
    "src/diagnostics.rs",
    "src/cli_process.rs",
    "src/extend/server.rs",
)


def scratch_tree(mutate=None):
    """A scratch copy of just the tree files the audits read, with
    `mutate(root)` applied. Returns the scratch root."""
    root = tempfile.mkdtemp(prefix="battery-audit-scratch-")
    for rel in AUDIT_FILES:
        dest = os.path.join(root, rel)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        shutil.copy(os.path.join(TREE, rel), dest)
    if mutate is not None:
        mutate(root)
    return root


def run_case(ident, tree):
    case = {c.ident: c for c in battery_mod.CASES}[ident]
    ctx = {**AUDIT_CONTEXT, "tree": tree}
    return {c["condition"]: c for c in case.run(ctx)["conditions"]}


def replace_in_function(root, rel, function, old, new):
    path = os.path.join(root, rel)
    with open(path, encoding="utf-8") as handle:
        src = handle.read()
    span = battery_mod.extract_function(src, function)
    assert span is not None and old in span["body"], (rel, function)
    body = span["body"].replace(old, new, 1)
    src = src[:span["body_start"]] + body + src[span["body_end"]:]
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(src)


class FunctionScopedAuditTests(unittest.TestCase):
    """Every audit is anchored to a function-scoped extraction:
    the needle must occur inside the named function's body, and
    the audit names file, function and line for its site."""

    def test_audits_name_file_function_and_line(self):
        expected = [
            ("a", "baseline poll located in source",
             "src/engine/wait.rs", "wait_until_empty"),
            ("l", "vars_os baseline located in source",
             "src/engine/environment.rs", "environment_block"),
            ("n", "verdict surface located in source",
             "src/cli_process.rs", "Console::finish"),
            ("o", "extendable precedent located in source",
             "src/extend/server.rs", "Extension::start"),
        ]
        for ident, cond_name, rel, func in expected:
            detail = conditions(ident)[cond_name]["detail"]
            self.assertIn(rel, detail, ident)
            self.assertIn(func, detail, ident)
            self.assertRegex(detail, r"line \d+", ident)
        detail = conditions("m")["per-site comparison audit (source)"]["detail"]
        for rel, func in (("src/cli_process/harden.rs", "harden_self"),
                          ("src/engine/owners.rs", "watched_owner_handles"),
                          ("src/diagnostics.rs", "cli_exit")):
            self.assertIn(rel, detail)
            self.assertIn(func, detail)
        self.assertRegex(detail, r"line \d+")

    def test_case_m_audit_enumerates_handle_consumers(self):
        detail = conditions("m")["per-site comparison audit (source)"]["detail"]
        self.assertIn("handle consumers:", detail)
        self.assertIn("deny_acl (line 213)", detail)
        self.assertIn("parent_process_id_on (line 489)", detail)
        self.assertIn("TerminateProcess (line 335)", detail)

    def test_unmutated_scratch_tree_audits_evidenced(self):
        root = scratch_tree()
        try:
            for ident, cond_name in (
                    ("a", "baseline poll located in source"),
                    ("l", "vars_os baseline located in source"),
                    ("m", "per-site comparison audit (source)"),
                    ("n", "verdict surface located in source"),
                    ("o", "extendable precedent located in source")):
                cond = run_case(ident, root)[cond_name]
                self.assertEqual(cond["status"], "evidenced",
                                 f"{ident}: {cond['detail']}")
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_counted_owner_fetch_deleted_refutes_case_m(self):
        # Delete the counted owner fetch inside
        # watched_owner_handles; the other GetCurrentProcess()
        # sites in owners.rs are kept, so a file-level substring
        # check would stay green — the function-scoped audit must
        # turn refuted.
        def mutate(root):
            replace_in_function(root, "src/engine/owners.rs",
                                "watched_owner_handles",
                                "GetCurrentProcess()",
                                "std::ptr::null_mut()")
            with open(os.path.join(root, "src/engine/owners.rs"),
                      encoding="utf-8") as handle:
                self.assertIn("GetCurrentProcess()", handle.read())
        root = scratch_tree(mutate)
        try:
            cond = run_case("m", root)[
                "per-site comparison audit (source)"]
            self.assertEqual(cond["status"], "refuted")
            self.assertIn("watched_owner_handles", cond["detail"])
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_sleep_relocated_to_unrelated_function_refutes_case_a(self):
        def mutate(root):
            replace_in_function(root, "src/engine/wait.rs",
                                "wait_until_empty",
                                "Sleep(10)", "Sleep(11)")
            path = os.path.join(root, "src/engine/wait.rs")
            with open(path, "a", encoding="utf-8") as handle:
                handle.write("\nfn unrelated_relocated_poll() "
                             "{ unsafe { Sleep(10) }; }\n")
        root = scratch_tree(mutate)
        try:
            cond = run_case("a", root)[
                "baseline poll located in source"]
            self.assertEqual(cond["status"], "refuted")
        finally:
            shutil.rmtree(root, ignore_errors=True)

    def test_console_finish_renamed_refutes_case_n(self):
        def mutate(root):
            path = os.path.join(root, "src/cli_process.rs")
            with open(path, encoding="utf-8") as handle:
                src = handle.read()
            self.assertIn("fn finish(&self", src)
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(src.replace("fn finish(&self",
                                         "fn finish_renamed(&self", 1))
        root = scratch_tree(mutate)
        try:
            cond = run_case("n", root)[
                "verdict surface located in source"]
            self.assertEqual(cond["status"], "refuted")
        finally:
            shutil.rmtree(root, ignore_errors=True)


def stub_ctx(variant, **extra):
    ctx = {"tree": TREE, "platform": sys.platform,
           "driver_mode": "stub", "driver_variant": variant}
    ctx.update(extra)
    return ctx


def run_case_full(ident, ctx):
    case = {c.ident: c for c in battery_mod.CASES}[ident]
    return case.run(ctx)


class DriverTierTests(unittest.TestCase):
    """The drivers' decision logic, exercised in the stub tier on
    any platform. The stub tier evidences nothing about Windows;
    it proves the negative controls: a deliberately broken variant
    fails its charged condition, and a missing driver blocks."""

    def test_stub_good_variant_evidences_every_condition(self):
        for ident in ("a", "l", "m", "n", "o"):
            rep = run_case_full(ident, stub_ctx("good"))
            self.assertEqual(rep["verdict"], "PASS", ident)
            for cond in rep["conditions"]:
                self.assertEqual(cond["status"], "evidenced",
                                 f"{ident}: {cond['condition']}")
                if "evidence" in cond:
                    self.assertGreater(cond["evidence"]["records"], 0)

    def test_stub_broken_case_a_fails_the_budget_condition(self):
        rep = run_case_full("a", stub_ctx("broken"))
        self.assertEqual(rep["verdict"], "FAIL")
        cond = {c["condition"]: c for c in rep["conditions"]}[
            "verdict equivalence + latency + budget"]
        self.assertEqual(cond["status"], "refuted")
        self.assertIn("10000", cond["detail"])

    def test_stub_broken_case_o_fails_the_forged_packet_condition(self):
        rep = run_case_full("o", stub_ctx("broken"))
        self.assertEqual(rep["verdict"], "FAIL")
        cond = {c["condition"]: c for c in rep["conditions"]}[
            "forged-packet case (binding conditions)"]
        self.assertEqual(cond["status"], "refuted")
        self.assertIn("packet", cond["detail"])

    def test_stub_broken_variants_fail_l_m_n(self):
        charged = {"l": "byte-level differential over the fixture set",
                   "m": "per-site A/B behaviour on Windows",
                   "n": "verdict equality across the finish cycle"}
        for ident, cond_name in charged.items():
            rep = run_case_full(ident, stub_ctx("broken"))
            self.assertEqual(rep["verdict"], "FAIL", ident)
            cond = {c["condition"]: c for c in rep["conditions"]}[cond_name]
            self.assertEqual(cond["status"], "refuted", ident)

    def test_missing_driver_reports_blocked_error_never_pass(self):
        empty = tempfile.mkdtemp(prefix="battery-no-drivers-")
        try:
            for ident in ("a", "l", "m", "n", "o"):
                rep = run_case_full(
                    ident, stub_ctx("good", driver_dir=empty))
                self.assertEqual(rep["verdict"], "BLOCKED-ERROR",
                                 ident)
        finally:
            shutil.rmtree(empty, ignore_errors=True)

    def test_stub_run_preserves_evidence_records(self):
        results = tempfile.mkdtemp(prefix="battery-evidence-")
        try:
            rep = run_case_full(
                "a", stub_ctx("good", results_dir=results))
            self.assertEqual(rep["verdict"], "PASS")
            path = os.path.join(results, "battery-case-a.json")
            with open(path, encoding="utf-8") as handle:
                payload = json.load(handle)
            self.assertGreaterEqual(len(payload["records"]), 1)
            record = payload["records"][0]
            for key in ("command", "exit_code", "stdout", "stderr"):
                self.assertIn(key, record)
            self.assertEqual(record["exit_code"], 0)
            self.assertIn("stub_probe.py",
                          " ".join(record["command"]))
        finally:
            shutil.rmtree(results, ignore_errors=True)

    def test_every_case_has_a_driver_file(self):
        for ident in ("a", "l", "m", "n", "o"):
            path = os.path.join(HERE, "..", "battery", "drivers",
                                f"case_{ident}.py")
            self.assertTrue(os.path.isfile(path), path)

    def test_run_ps1_invokes_the_battery_entry_point(self):
        with open(os.path.join(HERE, "..", "run.ps1"),
                  encoding="utf-8") as handle:
            text = handle.read()
        self.assertIn("battery\\battery.py", text)

    def test_driver_exit_code_is_two_for_error_conditions(self):
        # The driver exit-code contract (driver_common): 0 when
        # every condition is evidenced, 1 when any is refuted, 2
        # when any is blocked/error. A variant whose charged
        # condition lands "error" — an execution outcome, not a
        # verdict — must exit 2, never 0: exit 0 claims every
        # condition evidenced, which an error condition refutes
        # by definition.
        import subprocess
        drivers = os.path.join(HERE, "..", "battery", "drivers")
        for script, variant in (("case_m.py", "broken-timeout"),
                                ("case_a.py", "broken-no-remainder")):
            done = subprocess.run(
                [sys.executable, os.path.join(drivers, script),
                 "--mode", "stub", "--variant", variant],
                capture_output=True, text=True, timeout=300)
            self.assertEqual(done.returncode, 2,
                             f"{script} {variant}: {done.stdout}")
            payload = json.loads(done.stdout)
            statuses = [c["status"]
                        for c in payload["conditions"].values()]
            self.assertIn("error", statuses, script)

    def test_malformed_driver_payloads_block_the_case(self):
        # Round-18: the payload's shape is part of the driver
        # contract. A payload that is not an object, conditions
        # that are not an object, a condition that is not an
        # object or lacks its detail crashed the case run
        # (item assignment / KeyError), and a condition whose
        # status is outside the driver's own set (evidenced /
        # refuted / error) was silently mapped to the
        # BLOCKED-WINDOWS tier verdict. Every malformed shape
        # is driver error: BLOCKED-ERROR, conditions in error.
        good_conditions = {
            "verdict equivalence + latency + budget":
                {"status": "evidenced", "detail": "printed"},
            "no lost wakeup at the emptying race":
                {"status": "evidenced", "detail": "printed"},
        }
        payloads = (
            ["not", "an", "object"],
            "a bare string payload",
            {"case": "a", "tier": "stub", "records": [],
             "conditions": ["not", "an", "object"]},
            {"case": "a", "tier": "stub", "records": [],
             "conditions": {
                 "verdict equivalence + latency + budget":
                     "a bare string condition",
                 "no lost wakeup at the emptying race":
                     {"status": "evidenced", "detail": "printed"}}},
            {"case": "a", "tier": "stub", "records": [],
             "conditions": {
                 "verdict equivalence + latency + budget":
                     {"status": "evidenced"},
                 "no lost wakeup at the emptying race":
                     {"status": "evidenced", "detail": "printed"}}},
            {"case": "a", "tier": "stub", "records": [],
             "conditions": {
                 "verdict equivalence + latency + budget":
                     {"status": "bogus", "detail": "printed"},
                 "no lost wakeup at the emptying race":
                     {"status": "evidenced", "detail": "printed"}}},
            {"case": "a", "tier": "stub",
             "records": {"not": "a list"},
             "conditions": good_conditions},
        )
        for payload in payloads:
            scratch = tempfile.mkdtemp(prefix="battery-driver-shape-")
            try:
                with open(os.path.join(scratch, "case_a.py"), "w",
                          encoding="utf-8") as handle:
                    handle.write(
                        "import json, sys\n"
                        f"print(json.dumps({payload!r}))\n"
                        "sys.exit(0)\n")
                rep = run_case_full(
                    "a", stub_ctx("good", driver_dir=scratch))
                self.assertEqual(rep["verdict"], "BLOCKED-ERROR",
                                 payload)
                for cond in rep["conditions"]:
                    if cond["condition"] != \
                            "baseline poll located in source":
                        self.assertEqual(cond["status"], "error",
                                         (payload, cond))
            finally:
                shutil.rmtree(scratch, ignore_errors=True)

    def test_driver_verdicts_printed_before_a_bad_exit_are_not_used(self):
        # A driver that prints a complete, all-evidenced payload
        # and then exits outside its contract (a crash after
        # printing) has produced no usable evidence: battery.py
        # must block the case, never pass it on the strength of
        # the printed JSON alone.
        scratch = tempfile.mkdtemp(prefix="battery-driver-bad-exit-")
        try:
            payload = {
                "case": "a", "tier": "stub", "records": [{"x": 1}],
                "conditions": {
                    "verdict equivalence + latency + budget":
                        {"status": "evidenced", "detail": "printed"},
                    "no lost wakeup at the emptying race":
                        {"status": "evidenced", "detail": "printed"},
                },
            }
            with open(os.path.join(scratch, "case_a.py"), "w",
                      encoding="utf-8") as handle:
                handle.write(
                    "import json, sys\n"
                    f"print(json.dumps({payload!r}))\n"
                    "sys.exit(3)\n")
            rep = run_case_full(
                "a", stub_ctx("good", driver_dir=scratch))
            self.assertEqual(rep["verdict"], "BLOCKED-ERROR")
            for cond in rep["conditions"]:
                if cond["condition"] != "baseline poll located in source":
                    self.assertEqual(cond["status"], "error",
                                     cond["condition"])
        finally:
            shutil.rmtree(scratch, ignore_errors=True)


def load_driver_module(ident):
    import importlib.util
    path = os.path.join(HERE, "..", "battery", "drivers",
                        f"case_{ident}.py")
    spec = importlib.util.spec_from_file_location(
        f"driver_case_{ident}", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def verdict_and_conditions(ident, variant):
    rep = run_case_full(ident, stub_ctx(variant))
    return rep["verdict"], {c["condition"]: c
                            for c in rep["conditions"]}


class Round3ProbeTests(unittest.TestCase):
    """Round-3 landings: every decided field is computed from
    observations. Each broken variant below violates exactly one
    charged condition in the raw observations; each negative
    control withholds an observation the charge requires and
    must land BLOCKED-ERROR, never evidenced."""

    # ---- (a) P7: (empty, remainder) pairs, measured budget
    def test_stub_broken_case_a_remainder_mismatch_refutes(self):
        verdict, conds = verdict_and_conditions(
            "a", "broken-remainder")
        self.assertEqual(verdict, "FAIL")
        cond = conds["verdict equivalence + latency + budget"]
        self.assertEqual(cond["status"], "refuted")

    def test_stub_broken_case_a_early_wait_return_refutes(self):
        # The never-dies wait returns after 1,000 ms with
        # 4,000 ms of the budget unspent: the budget did not
        # bind, decided from the measured latency and remainder.
        verdict, conds = verdict_and_conditions("a", "broken-early")
        self.assertEqual(verdict, "FAIL")
        cond = conds["verdict equivalence + latency + budget"]
        self.assertEqual(cond["status"], "refuted")

    def test_case_a_trial_without_remainder_is_blocked_error(self):
        verdict, conds = verdict_and_conditions(
            "a", "broken-no-remainder")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["verdict equivalence + latency + budget"]
        self.assertEqual(cond["status"], "error")

    # ---- (o) P4: counted packets, counted teardown, forged
    def test_stub_broken_case_o_zero_packets_refute(self):
        verdict, conds = verdict_and_conditions("o", "broken-zero")
        self.assertEqual(verdict, "FAIL")
        cond = conds["zero-packet delivery, including the "
                     "already-empty-at-association case"]
        self.assertEqual(cond["status"], "refuted")

    def test_stub_broken_case_o_four_op_teardown_refutes(self):
        verdict, conds = verdict_and_conditions("o", "broken-ops")
        self.assertEqual(verdict, "FAIL")
        cond = conds["port close / disassociation effects"]
        self.assertEqual(cond["status"], "refuted")
        self.assertIn("4 operations", cond["detail"])

    def test_case_o_forged_claim_without_observation_blocked(self):
        # verdict_source claims "authoritative job query" but no
        # packet-influence observation is present.
        verdict, conds = verdict_and_conditions(
            "o", "broken-forged-unobserved")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["forged-packet case (binding conditions)"]
        self.assertEqual(cond["status"], "error")

    # ---- (n) P5: pending actually varied, fallback computed
    def test_stub_broken_case_n_pending_ignored_refutes(self):
        verdict, conds = verdict_and_conditions(
            "n", "broken-pending")
        self.assertEqual(verdict, "FAIL")
        cond = conds["verdict equality across the finish cycle"]
        self.assertEqual(cond["status"], "refuted")

    def test_case_n_pinned_copy_is_blocked_error(self):
        # The fallback record merely restates eager values; no
        # re-measured lazy verdicts to compute effective from.
        verdict, conds = verdict_and_conditions(
            "n", "broken-pinned-copy")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["verdict equality across the finish cycle"]
        self.assertEqual(cond["status"], "error")

    # ---- (l) P6: set operation performed, duplicates kept
    def test_stub_broken_case_l_stale_channel_entry_refutes(self):
        verdict, conds = verdict_and_conditions(
            "l", "broken-stale")
        self.assertEqual(verdict, "FAIL")
        cond = conds["byte-level differential over the fixture set"]
        self.assertEqual(cond["status"], "refuted")
        self.assertIn("channel-entry", cond["detail"])

    def test_case_l_missing_set_step_is_blocked_error(self):
        verdict, conds = verdict_and_conditions(
            "l", "broken-no-set")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["byte-level differential over the fixture set"]
        self.assertEqual(cond["status"], "error")

    # ---- (m) P8: consumers really checked, deadline covers sum
    def test_stub_broken_case_m_unchecked_consumers_refute(self):
        verdict, conds = verdict_and_conditions(
            "m", "broken-consumers")
        self.assertEqual(verdict, "FAIL")
        cond = conds["per-site A/B behaviour on Windows"]
        self.assertEqual(cond["status"], "refuted")

    def test_case_m_consumers_checked_requires_enumeration(self):
        module = load_driver_module("m")
        # The failure side: an empty consumer list, and an entry
        # that merely locates the site (the old vacuous form),
        # both fail the check.
        self.assertFalse(module.consumers_checked_for(
            {"consumers": [], "line": 294}))
        self.assertFalse(module.consumers_checked_for(
            {"line": 294}))
        self.assertFalse(module.consumers_checked_for({}))
        self.assertFalse(module.consumers_checked_for(None))
        # The real tree's four sites all enumerate consumers.
        for entry in battery_mod.pseudo_site_report(TREE):
            self.assertTrue(
                module.consumers_checked_for(entry), entry["name"])

    def test_case_m_driver_timeout_covers_sub_deadlines(self):
        module = load_driver_module("m")
        self.assertEqual(module.PER_SITE_TIMEOUT_S, 2 * 1800)
        self.assertEqual(module.REQUIRED_WINDOWS_TIMEOUT_S,
                         4 * (1800 + 1800))
        case = {c.ident: c for c in battery_mod.CASES}["m"]
        self.assertGreaterEqual(
            case.driver_timeout_s,
            module.REQUIRED_WINDOWS_TIMEOUT_S)

    def test_driver_error_mid_run_is_blocked_error(self):
        # A driver that dies mid-run (here: after starting, with
        # no verdicts produced) leaves its case BLOCKED-ERROR,
        # never PASS from partial work.
        scratch = tempfile.mkdtemp(prefix="battery-driver-dies-")
        try:
            with open(os.path.join(scratch, "case_a.py"), "w",
                      encoding="utf-8") as handle:
                handle.write(
                    "import sys\n"
                    "print('partial run started', file=sys.stderr)\n"
                    "sys.exit(3)\n")
            rep = run_case_full(
                "a", stub_ctx("good", driver_dir=scratch))
            self.assertEqual(rep["verdict"], "BLOCKED-ERROR")
        finally:
            shutil.rmtree(scratch, ignore_errors=True)


class Round4ProbeTests(unittest.TestCase):
    """Round-4 landings: every decided field computed from
    observations, and the decider may not trust probe-written
    labels for the quantity it decides. Each new variant
    violates exactly one round-4 requirement in the raw
    observations."""

    # ---- (a) P7: tick-derived tolerance, same job
    def test_stub_noisy_case_a_still_evidences(self):
        # +0.02 ms on the size-0 trial and +5 ms on a dying
        # trial are inside one 10 ms poll tick: they may not
        # refute the adoption.
        verdict, conds = verdict_and_conditions("a", "noisy")
        self.assertEqual(verdict, "PASS")
        cond = conds["verdict equivalence + latency + budget"]
        self.assertEqual(cond["status"], "evidenced")

    def test_stub_broken_case_a_systematic_2x_latency_refutes(self):
        verdict, conds = verdict_and_conditions(
            "a", "broken-latency-2x")
        self.assertEqual(verdict, "FAIL")
        cond = conds["verdict equivalence + latency + budget"]
        self.assertEqual(cond["status"], "refuted")

    # ---- (n) P5: measured held bytes, verdict-value pairing
    def test_stub_broken_case_n_gate_label_bytes_refute(self):
        # Gate cells report 8,192 label held bytes while the
        # measured pending-pattern count is zero.
        verdict, conds = verdict_and_conditions(
            "n", "broken-gate-label")
        self.assertEqual(verdict, "FAIL")
        cond = conds["verdict equality across the finish cycle"]
        self.assertEqual(cond["status"], "refuted")

    def test_stub_broken_case_n_role_verdict_values_refute(self):
        # One role pairing's consulted verdict values differ
        # across the pending pairing; exit codes stay equal.
        verdict, conds = verdict_and_conditions(
            "n", "broken-role-verdict")
        self.assertEqual(verdict, "FAIL")
        cond = conds["verdict equality across the finish cycle"]
        self.assertEqual(cond["status"], "refuted")

    # ---- (l) P6: unfiltered enumerations, one ASCII matcher
    def test_stub_broken_case_l_nonfixture_divergence_refutes(self):
        # The GESW enumeration carries an entry no fixture names
        # and vars_os does not; both blocks omit it, so only the
        # recorded enumerations can decide it.
        verdict, conds = verdict_and_conditions(
            "l", "broken-nonfixture")
        self.assertEqual(verdict, "FAIL")
        cond = conds["byte-level differential over the fixture set"]
        self.assertEqual(cond["status"], "refuted")
        self.assertIn("HOST_EXTRA_ENTRY", cond["detail"])

    def test_stub_broken_case_l_matcher_divergence_refutes(self):
        verdict, conds = verdict_and_conditions(
            "l", "broken-matcher")
        self.assertEqual(verdict, "FAIL")
        cond = conds["byte-level differential over the fixture set"]
        self.assertEqual(cond["status"], "refuted")
        self.assertIn("unicode-and-case", cond["detail"])

    def test_case_l_ascii_matcher_is_the_tree_semantics(self):
        module = load_driver_module("l")
        # ASCII variants match; non-ASCII case variants do not,
        # where upper()/casefold() approximations would match.
        self.assertTrue(module.ascii_eq("Path", "PATH"))
        self.assertTrue(module.ascii_eq("=C:", "=c:"))
        self.assertFalse(module.ascii_eq(
            "PROCEſS_TREE_GUARD_CHANNEL",
            "PROCESS_TREE_GUARD_CHANNEL"))
        self.assertFalse(module.ascii_eq("NAME-Ü", "name-ü"))

    # ---- (o) P8: the non-inheritance trial must be able to see
    # inheritance
    def test_stub_broken_case_o_inheritable_port_refutes(self):
        verdict, conds = verdict_and_conditions(
            "o", "broken-inheritable")
        self.assertEqual(verdict, "FAIL")
        cond = conds["non-inheritance"]
        self.assertEqual(cond["status"], "refuted")

    def test_case_o_noninheritance_without_spawn_record_blocked(self):
        verdict, conds = verdict_and_conditions(
            "o", "broken-no-inheritance-record")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["non-inheritance"]
        self.assertEqual(cond["status"], "error")

    # ---- (o) P9: confirmed teardown count, independent
    # fallback observations
    def test_stub_broken_case_o_unconfirmed_teardown_refutes(self):
        verdict, conds = verdict_and_conditions(
            "o", "broken-teardown-unconfirmed")
        self.assertEqual(verdict, "FAIL")
        cond = conds["port close / disassociation effects"]
        self.assertEqual(cond["status"], "refuted")
        self.assertIn("4 operations", cond["detail"])

    def test_case_o_fallback_shared_observation_blocked(self):
        verdict, conds = verdict_and_conditions(
            "o", "broken-fallback-shared")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["fallback invariance"]
        self.assertEqual(cond["status"], "error")


class Round5ProbeTests(unittest.TestCase):
    """Round-5 landings. P6: a deadline expiry or spawn failure
    is an execution failure (BLOCKED-ERROR), never an adoption
    FAIL. P8: every decider enforces its charged coverage set —
    a truncated observation set is BLOCKED-ERROR for the affected
    condition, never evidenced, in the style of the broken-*
    controls above."""

    # ---- (m) P6: timeout / spawn are execution failures
    def test_stub_broken_case_m_timeout_is_blocked_error(self):
        verdict, conds = verdict_and_conditions(
            "m", "broken-timeout")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["per-site A/B behaviour on Windows"]
        self.assertEqual(cond["status"], "error")

    def test_stub_broken_case_m_spawn_failure_is_blocked_error(self):
        verdict, conds = verdict_and_conditions(
            "m", "broken-spawn")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["per-site A/B behaviour on Windows"]
        self.assertEqual(cond["status"], "error")

    def test_case_m_decide_distinguishes_execution_from_failure(self):
        module = load_driver_module("m")
        cond_name = "per-site A/B behaviour on Windows"

        def sites(**overrides):
            built = [{"name": name, "build_ok": True,
                      "tests_ok": True, "consumers_checked": True,
                      "invalid_handle_misuse": False,
                      "timed_out": False, "spawn_failed": False,
                      "not_attempted": False}
                     for name, *_ in battery_mod.PSEUDO_SITES]
            for index, changes in overrides.items():
                built[int(index)].update(changes)
            return {"sites": built}

        # Negative control: exit_code 0 build and a completed
        # failing test run stays "refuted" — a real A/B failure
        # is not laundered into an execution error.
        status, _ = module.decide(sites(**{"1": {
            "tests_ok": False}}))[cond_name]
        self.assertEqual(status, "refuted")
        # A spawn-failure record (exit_code None, "spawn failed")
        # lands BLOCKED-ERROR, never FAIL.
        status, _ = module.decide(sites(**{"0": {
            "build_ok": False, "tests_ok": False,
            "build": {"exit_code": None,
                      "stderr": "spawn failed: no such file"}}})
        )[cond_name]
        self.assertEqual(status, "error")
        # A deadline record (exit_code None, deadline stderr)
        # likewise.
        status, _ = module.decide(sites(**{"2": {
            "build_ok": False, "tests_ok": False,
            "tests": {"exit_code": None,
                      "stderr": "deadline of 1800 s expired"}}})
        )[cond_name]
        self.assertEqual(status, "error")
        # A site never attempted because an earlier command
        # timed out is "error", never "refuted".
        status, _ = module.decide(sites(**{"3": {
            "build_ok": False, "tests_ok": False,
            "not_attempted": True}}))[cond_name]
        self.assertEqual(status, "error")

    # ---- (a) P8: charged sizes + a never-dies trial
    def test_stub_truncated_case_a_is_blocked_error(self):
        verdict, conds = verdict_and_conditions(
            "a", "broken-truncated")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["verdict equivalence + latency + budget"]
        self.assertEqual(cond["status"], "error")

    def test_stub_case_a_without_never_dies_is_blocked_error(self):
        # All five dying sizes but no never-dies trial: the
        # budget clause must not be vacuous.
        verdict, conds = verdict_and_conditions(
            "a", "broken-no-never-dies")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["verdict equivalence + latency + budget"]
        self.assertEqual(cond["status"], "error")

    # ---- (l) P8: charged fixture set
    def test_stub_truncated_case_l_is_blocked_error(self):
        verdict, conds = verdict_and_conditions(
            "l", "broken-truncated")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["byte-level differential over the fixture set"]
        self.assertEqual(cond["status"], "error")

    # ---- (m) P8: charged site set
    def test_stub_truncated_case_m_is_blocked_error(self):
        verdict, conds = verdict_and_conditions(
            "m", "broken-truncated")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["per-site A/B behaviour on Windows"]
        self.assertEqual(cond["status"], "error")

    # ---- (n) P8: charged role/API matrix
    def test_stub_truncated_case_n_is_blocked_error(self):
        verdict, conds = verdict_and_conditions(
            "n", "broken-truncated")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["verdict equality across the finish cycle"]
        self.assertEqual(cond["status"], "error")

    # ---- (o) P8: phases, the empty field, cycles, forged set
    def test_stub_case_o_zero_phases_truncated_blocked(self):
        verdict, conds = verdict_and_conditions(
            "o", "broken-truncated-zero")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["zero-packet delivery, including the "
                     "already-empty-at-association case"]
        self.assertEqual(cond["status"], "error")

    def test_stub_case_o_zero_trial_not_empty_refutes(self):
        # The empty field is read: a zero trial whose job did
        # not query empty at return is a refutation, not a pass.
        verdict, conds = verdict_and_conditions(
            "o", "broken-zero-empty")
        self.assertEqual(verdict, "FAIL")
        cond = conds["zero-packet delivery, including the "
                     "already-empty-at-association case"]
        self.assertEqual(cond["status"], "refuted")

    def test_stub_case_o_cycles_below_charged_blocked(self):
        verdict, conds = verdict_and_conditions(
            "o", "broken-cycles")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["port close / disassociation effects"]
        self.assertEqual(cond["status"], "error")

    def test_stub_case_o_forged_truncated_blocked(self):
        verdict, conds = verdict_and_conditions(
            "o", "broken-forged-truncated")
        self.assertEqual(verdict, "BLOCKED-ERROR")
        cond = conds["forged-packet case (binding conditions)"]
        self.assertEqual(cond["status"], "error")


class ProbeWinLHandleTests(unittest.TestCase):
    """P2's owed anywhere-tier test (the probe fix itself landed
    in probe_win_l.py): every value assigned to a STARTUPINFO
    standard-handle field must be produced by
    msvcrt.get_osfhandle — a CRT file descriptor is not a Win32
    HANDLE. Reverting any one assignment to the raw descriptor
    turns this test red."""

    def test_every_standard_handle_comes_from_get_osfhandle(self):
        path = os.path.join(HERE, "..", "battery", "drivers",
                            "probe_win_l.py")
        with open(path, encoding="utf-8") as handle:
            src = handle.read()
        fields = ["hStdError", "hStdInput", "hStdOutput"]
        assigned = re.findall(
            r"startup\.(hStdInput|hStdOutput|hStdError)\s*=", src)
        self.assertEqual(sorted(assigned), fields)
        converted = re.findall(
            r"startup\.(hStdInput|hStdOutput|hStdError)\s*=\s*"
            r"ctypes\.c_void_p\(\s*msvcrt\.get_osfhandle\(", src)
        self.assertEqual(sorted(converted), fields)


class RecordedBatteryEvidenceTests(unittest.TestCase):
    """Round-21 adjudication P2: the recorded battery evidence
    is held to a fresh run against the current tree. The
    battery's audits compute their source coordinates
    dynamically, so the suite stayed current while the recorded
    record — evidence/battery-linux.json, the file VERIFICATION
    item 4 points to — went stale at the coordinate level when
    the tree moved under it (four of five evidenced details
    carrying superseded line numbers). The recorded record must
    equal what the battery produces against the tree beside the
    suite now, verdicts and coordinates alike."""

    def test_recorded_battery_evidence_matches_a_fresh_run(self):
        with open(os.path.join(HERE, "..", "evidence",
                               "battery-linux.json"),
                  encoding="utf-8") as handle:
            recorded = json.load(handle)
        # Replay the recorded Linux tier even when this suite runs on Windows.
        linux_context = {**AUDIT_CONTEXT, "platform": "linux"}
        fresh = [case.run(linux_context) for case in battery_mod.CASES]
        # Each record names the folder it audited ("tree"), which
        # is where the record was made, not something the audits
        # found: it must equal the folder audited now, and every
        # other field must equal the recorded value.
        self.assertEqual([report["tree"] for report in fresh],
                         [TREE] * len(fresh))

        def findings(reports):
            return [{key: value for key, value in report.items()
                     if key != "tree"} for report in reports]

        self.assertEqual(findings(recorded), findings(fresh))


if __name__ == "__main__":
    unittest.main()
