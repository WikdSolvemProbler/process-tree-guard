# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Output-observation family: profile dimension (1), "Correctness
and output quality", is observed on the campaign's fast path.

The fast child is an output workload (the stub relay here, testkit
"relay" in the shipped configuration): it emits a known stdout
line and a known stderr line. Every fast slot's streams are
captured to per-slot files and checked against the expected bytes
the configuration names; a slot that returns the expected exit
code but drops or alters the child's output is classified
"output-mismatch", never "ok". The guard's output mismatch fails
gate dimension "output correctness" (and with it dimension (1));
a competitor's is competitor-loss evidence. A fast record
carrying no output observation leaves the dimension UNEVALUATED.
Kill slots carry no output observation: the kill child's product
is the tree it leaves, judged by the survivor probe.
"""

import contextlib
import hashlib
import json
import os
import sys
import tempfile
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
CAMPAIGN = os.path.join(HERE, "..", "campaign")
sys.path.insert(0, CAMPAIGN)

import arms as arms_mod          # noqa: E402
import plan as plan_mod          # noqa: E402
import report as report_mod      # noqa: E402
import runner as runner_mod      # noqa: E402
from plan import Slot            # noqa: E402
from runner import run_slot      # noqa: E402

STUB = os.path.join(CAMPAIGN, "stubs.py")
PROBE = os.path.join(CAMPAIGN, "probe.py")

EXPECTED_STDOUT = "ptg-output-stdout\n"
EXPECTED_STDERR = "error-stream\n"
STDOUT_SHA = hashlib.sha256(EXPECTED_STDOUT.encode("utf-8")).hexdigest()
STDERR_SHA = hashlib.sha256(EXPECTED_STDERR.encode("utf-8")).hexdigest()
EMPTY_SHA = hashlib.sha256(b"").hexdigest()

FULL_COUNTS = {"control": plan_mod.EXPECTED_CONTROLS,
               "warmup": plan_mod.EXPECTED_WARMUPS,
               "measured": plan_mod.EXPECTED_MEASURED}


_SHIPPED_CONFIG = None


def shipped_config():
    """The suite's shipped campaign configuration
    (campaign.json), loaded once — the plan this fixture's
    argv are built through (round-36 F3)."""
    global _SHIPPED_CONFIG
    if _SHIPPED_CONFIG is None:
        cfg_path = os.path.join(HERE, "..", "campaign.json")
        with open(cfg_path, encoding="utf-8") as handle:
            _SHIPPED_CONFIG = json.load(handle)
    return _SHIPPED_CONFIG


def fixture_fast_argv(arm):
    """The fast vector the shipped construction builds for
    one product arm, at the plan's own length for the arm
    (round-36 F3 construction truth, as in the
    campaign-logic fixture)."""
    config = shipped_config()
    exe = arms_mod.arm_executable(arm, config)
    spec = config["children"]["fast"]
    return [exe] + arms_mod.expand_template(
        arms_mod.resolve_template(arm, "fast", config),
        runner_mod.FAST_TIMEOUT_S, spec["program"],
        list(spec.get("args", [])))


def output_config(tmp, fast_modes=None):
    """A stub-tier campaign configuration whose fast child is the
    relay output workload, with the expected bytes named as
    configuration data. fast_modes maps an arm to the stub mode
    its fast template uses (default "arm-fast": a faithful arm);
    the negative controls name "arm-fast-mute" (suppresses the
    child's output) or "arm-fast-garble" (alters its stdout)."""
    exe = os.path.join(tmp, "stub-arm")
    with open(exe, "w", encoding="utf-8") as handle:
        handle.write(f'#!/bin/sh\nexec {sys.executable} {STUB} "$@"\n')
    os.chmod(exe, 0o755)
    fast_modes = fast_modes or {}
    config = {
        "arms": {arm: {"executable": exe, "templates": {
            "fast": [fast_modes.get(arm, "arm-fast"), "{timeout_s}",
                     "--", "{child}", "{child_args}"],
            "kill": ["arm-kill-tree", "{timeout_s}", "--",
                     "{child}", "{child_args}"],
        }} for arm in plan_mod.PRODUCT_ARMS},
        "children": {
            "fast": {"program": sys.executable,
                     "args": [STUB, "relay", "0"],
                     "stdin": "ptg-output-stdout",
                     "expected_stdout": EXPECTED_STDOUT,
                     "expected_stderr": EXPECTED_STDERR},
            "floor_spawn": {"program": sys.executable,
                            "args": [STUB, "exit0b"]},
            "kill": {"program": sys.executable,
                     "args": [STUB, "tree", "{marker}", "{sleep_s}"]},
        },
        "survivor_checker": [sys.executable, PROBE],
    }
    return config


def fixture_record(arm, ms, path="fast", context="NO_WINDOW", rep=0,
                   outcome="ok", kind="measured"):
    record = {"kind": kind, "arm": arm, "path": path,
              "context": context, "rep": rep, "outcome": outcome,
              "elapsed_ms": ms,
              # Construction truth (round-31 F2 alignment): a
              # malformed record carries the refusal the fixture
              # arms give (125, as the campaign-logic fixture's
              # measured() carries) — for the designated arm that
              # code is now a pinned expectation on its
              # contract-pinned shapes, so a fixture claiming a
              # complete campaign cannot carry an acceptance (0)
              # there; competitor records at 125 are observations.
              "exit_code": arms_mod.expected_exit(arm, "kill") if path == "kill" else
              125 if path == "malformed" else 0,
              "survivor": False if path == "kill" else None,
              # Construction truth (round-25 alignment): the
              # runner writes argv as the executed argument
              # vector (a non-empty list of string tokens) on
              # every record; the empty list is a shape no run
              # writes. Construction truth (round-36 F3): a
              # product arm's fast record carries the shipped
              # construction's vector at the plan's length.
              "argv": fixture_fast_argv(arm) if (
                  path == "fast" and arm in plan_mod.PRODUCT_ARMS)
              else ["fixture-arm", "10", "--", "fixture-child"],
              "detail": "",
              "arm_pid": 4000 + rep,
              "child_pid": 5000 + rep if path == "kill" else None,
              "grandchild_pid": 6000 + rep if path == "kill" else None,
              "tree_marker": "fixture-marker" if path == "kill" else None,
              "output_stdout_sha256": None,
              "output_stderr_sha256": None,
              "output_stdout_len": None, "output_stderr_len": None,
              "output_stdout_match": None, "output_stderr_match": None,
              # Round-26 record contract: every record carries the
              # runner's resource instrumentation fields (dimension
              # (3)'s collection), and a scale record carries the
              # tree identity its survivor judgement rests on.
              "cpu_ms": 1.0, "peak_working_set_bytes": 4096,
              "resource_note": None, "variant": None,
              "descendant_pids": None}
    if path == "malformed":
        # Construction truth (round-37 F1): a malformed
        # record carries the variant its slot encodes — the
        # runner writes the slot's variant on the record, as
        # the campaign-logic fixture's measured() carries
        # variant_for on every record; carrying None pooled
        # every shape into one (3) cell and left the plan's
        # per-shape cells recordless in the census.
        record["variant"] = plan_mod.variant_for(
            path, rep, kind)
        # Construction truth (round-33 F2; round-36 F3): a
        # malformed record's argv is the runner's own
        # malformed_argv construction through the arm's
        # template under the shipped configuration — the
        # shape-distinguishing content at the plan's own
        # length for the arm, not a generic vector that
        # presents no shape.
        record["argv"] = runner_mod.malformed_argv(
            Slot(kind, arm, path, context, rep,
                 plan_mod.variant_for(path, rep, kind)),
            shipped_config())
    if path == "scale":
        record["tree_marker"] = "fixture-marker"
        record["child_pid"] = 5000 + rep
        record["survivor"] = False if outcome == "ok" else None
        # Construction truth (round-27 alignment): the record
        # carries its variant and publishes exactly the
        # variant's descendant count (wide-N: N; deep-D: D).
        record["variant"] = plan_mod.variant_for("scale", rep, kind)
        owed = plan_mod.expected_descendants(record["variant"])
        record["descendant_pids"] = [
            6000 + rep * 100 + index for index in range(owed)]
        record["exit_code"] = arms_mod.expected_exit(arm, "scale")
    if path == "fast" and outcome == "ok":
        record["output_stdout_sha256"] = STDOUT_SHA
        record["output_stderr_sha256"] = STDERR_SHA
        record["output_stdout_len"] = len(EXPECTED_STDOUT)
        record["output_stderr_len"] = len(EXPECTED_STDERR)
        record["output_stdout_match"] = True
        record["output_stderr_match"] = True
    return record


def complete_fixture():
    recs = []
    for slot in plan_mod.build_plan():
        if slot.arm == "floor-direct":
            ms = 51.0
        elif slot.arm == "floor-spawn":
            ms = 52.0
        elif slot.arm == "guard":
            ms = 62.0 if slot.path == "fast" else 2100.0
        else:
            ms = 65.0 if slot.path == "fast" else 2200.0
        recs.append(fixture_record(
            slot.arm, ms, path=slot.path, context=slot.context,
            rep=slot.rep, kind=slot.kind))
    return recs


def shipped_decisions():
    cfg_path = os.path.join(HERE, "..", "campaign.json")
    with open(cfg_path, encoding="utf-8") as handle:
        config = json.load(handle)
    return report_mod.decisions_from_config(config)


@contextlib.contextmanager
def collected_usability_artifact():
    """Dimension (8)'s manual instrument with its check
    performed (round-27: the state is computed from the
    artifact): one prompt class exercised with an observed
    outcome recorded, patched in as report's artifact."""
    artifact = {
        "instrument": "fixture manual usability check",
        "tier": "manual/Windows-theory",
        "records": [
            {"prompt_class": "Windows Hello request",
             "exercised": True,
             "observed": "fixture: prompt shown, answered "
                         "correctly, run proceeded",
             "outcome": "pass"},
            {"prompt_class": "credential dialog",
             "exercised": False,
             "observed": None, "outcome": "not-run"},
            {"prompt_class": "console password prompt",
             "exercised": False,
             "observed": None, "outcome": "not-run"},
        ],
    }
    with tempfile.NamedTemporaryFile(
            "w", suffix=".json", delete=False,
            encoding="utf-8") as handle:
        json.dump(artifact, handle)
        path = handle.name
    try:
        with mock.patch.object(report_mod, "USABILITY_ARTIFACT", path):
            yield path
    finally:
        os.unlink(path)


def gate_entry(records, dimension, context):
    return next(entry for entry in report_mod.freeze_gate(records)
                if entry["dimension"] == dimension
                and entry["context"] == context)


class OutputCaptureTests(unittest.TestCase):
    """Stub tier, exercised on any platform: the runner captures
    each fast slot's streams and judges them against the
    configuration's expected bytes."""

    def test_fast_slot_records_matching_output_observation(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = output_config(tmp)
            rec = run_slot(Slot("measured", "guard", "fast",
                                "NO_WINDOW", 0), config)
            self.assertEqual(rec["outcome"], "ok")
            self.assertEqual(rec["exit_code"], 0)
            self.assertEqual(rec["output_stdout_sha256"], STDOUT_SHA)
            self.assertEqual(rec["output_stderr_sha256"], STDERR_SHA)
            self.assertEqual(rec["output_stdout_len"],
                             len(EXPECTED_STDOUT))
            self.assertEqual(rec["output_stderr_len"],
                             len(EXPECTED_STDERR))
            self.assertTrue(rec["output_stdout_match"])
            self.assertTrue(rec["output_stderr_match"])

    def test_mute_arm_is_output_mismatch_with_the_expected_exit_code(self):
        # The negative control: the arm returns the expected exit
        # code (0) but suppresses both of the child's streams.
        # It must not be classified fully correct.
        with tempfile.TemporaryDirectory() as tmp:
            config = output_config(
                tmp, fast_modes={"processkit": "arm-fast-mute"})
            rec = run_slot(Slot("measured", "processkit", "fast",
                                "NO_WINDOW", 0), config)
            self.assertEqual(rec["exit_code"], 0)
            self.assertEqual(rec["outcome"], "output-mismatch")
            self.assertFalse(rec["output_stdout_match"])
            self.assertFalse(rec["output_stderr_match"])
            self.assertEqual(rec["output_stdout_sha256"], EMPTY_SHA)
            self.assertIn("output did not match", rec["detail"])

    def test_garble_arm_alters_stdout_and_is_output_mismatch(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = output_config(
                tmp, fast_modes={"sysprims": "arm-fast-garble"})
            rec = run_slot(Slot("measured", "sysprims", "fast",
                                "NO_WINDOW", 0), config)
            self.assertEqual(rec["exit_code"], 0)
            self.assertEqual(rec["outcome"], "output-mismatch")
            self.assertFalse(rec["output_stdout_match"])
            self.assertTrue(rec["output_stderr_match"])

    def test_kill_slot_carries_no_output_observation(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = output_config(tmp)
            rec = run_slot(Slot("measured", "guard", "kill",
                                "NO_WINDOW", 0), config)
            self.assertEqual(rec["outcome"], "ok")
            for field in ("output_stdout_sha256",
                          "output_stderr_sha256",
                          "output_stdout_len", "output_stderr_len",
                          "output_stdout_match",
                          "output_stderr_match"):
                self.assertIsNone(rec[field], field)

    def test_floor_slots_carry_the_observation_their_child_defines(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = output_config(tmp)
            direct = run_slot(Slot("control", "floor-direct", "fast",
                                   "NO_WINDOW", 0), config)
            self.assertEqual(direct["outcome"], "ok")
            self.assertTrue(direct["output_stdout_match"])
            self.assertTrue(direct["output_stderr_match"])
            # The floor-spawn child names no expected bytes: its
            # (empty) streams are digested, with no verdict.
            spawn = run_slot(Slot("control", "floor-spawn", "fast",
                                  "NO_WINDOW", 0), config)
            self.assertEqual(spawn["outcome"], "ok")
            self.assertEqual(spawn["output_stdout_sha256"], EMPTY_SHA)
            self.assertEqual(spawn["output_stderr_sha256"], EMPTY_SHA)
            self.assertIsNone(spawn["output_stdout_match"])
            self.assertIsNone(spawn["output_stderr_match"])


class OutputGateTests(unittest.TestCase):
    """The verdict wiring: dimension (1) rests on kill-path,
    exit-code and output correctness together."""

    def test_complete_fixture_dimension_1_rests_on_output(self):
        import harness as harness_mod
        # Round-27 F2: the completeness premise carries the
        # manual usability check performed (dimension (8)
        # completes only on its artifact's recorded evidence).
        self.enterContext(collected_usability_artifact())
        recs = complete_fixture()
        for context in plan_mod.CONTEXTS:
            entry = gate_entry(recs, "output correctness", context)
            self.assertEqual(entry["outcome"], "PASS", context)
        outcome = harness_mod.campaign_outcome(
            recs, FULL_COUNTS, decisions=shipped_decisions())
        self.assertEqual(outcome["campaign_status"], "complete")
        ledger = {entry["dimension"]: entry
                  for entry in outcome["claim_ledger"]}
        claim = ledger["Correctness and output quality"]
        self.assertEqual(claim["state"], "observed-passing")
        self.assertIn("output correctness", claim["basis"])

    def test_guard_output_mismatch_fails_dimension_1(self):
        import harness as harness_mod
        recs = complete_fixture()
        victim = next(r for r in recs
                      if r["arm"] == "guard" and r["path"] == "fast"
                      and r["kind"] == "measured"
                      and r["context"] == "NO_WINDOW")
        victim["outcome"] = "output-mismatch"
        victim["output_stdout_match"] = False
        victim["detail"] = "stdout output did not match the " \
                           "expected bytes for guard/fast"
        failed = gate_entry(recs, "output correctness", "NO_WINDOW")
        self.assertEqual(failed["outcome"], "FAIL")
        other = gate_entry(recs, "output correctness", "DETACHED")
        self.assertEqual(other["outcome"], "PASS")
        outcome = harness_mod.campaign_outcome(
            recs, FULL_COUNTS, decisions=shipped_decisions())
        self.assertEqual(outcome["campaign_status"], "incomplete")
        self.assertGreaterEqual(outcome["freeze_gate_fail_count"], 1)
        ledger = {entry["dimension"]: entry["state"]
                  for entry in outcome["claim_ledger"]}
        self.assertEqual(ledger["Correctness and output quality"],
                         "observed-failing")
        losses = report_mod.campaign_losses(
            recs, FULL_COUNTS, decisions=shipped_decisions())
        self.assertTrue(any("output mismatch in guard/fast" in loss
                            for loss in losses), losses)

    def test_missing_output_observation_is_unevaluated_never_passing(self):
        import harness as harness_mod
        recs = complete_fixture()
        for record in recs:
            for field in ("output_stdout_sha256",
                          "output_stderr_sha256",
                          "output_stdout_len", "output_stderr_len",
                          "output_stdout_match",
                          "output_stderr_match"):
                record[field] = None
        for context in plan_mod.CONTEXTS:
            entry = gate_entry(recs, "output correctness", context)
            self.assertEqual(entry["outcome"], "UNEVALUATED", context)
        outcome = harness_mod.campaign_outcome(
            recs, FULL_COUNTS, decisions=shipped_decisions())
        self.assertEqual(outcome["campaign_status"], "incomplete")
        ledger = {entry["dimension"]: entry["state"]
                  for entry in outcome["claim_ledger"]}
        self.assertEqual(ledger["Correctness and output quality"],
                         "UNEVALUATED")

    def test_competitor_output_mismatch_is_competitor_loss_evidence(self):
        import harness as harness_mod
        recs = complete_fixture()
        victim = next(r for r in recs
                      if r["arm"] == "nuewframe"
                      and r["path"] == "fast"
                      and r["kind"] == "measured"
                      and r["context"] == "NO_WINDOW")
        victim["outcome"] = "output-mismatch"
        victim["output_stdout_match"] = False
        victim["output_stderr_match"] = False
        victim["detail"] = "stdout and stderr output did not " \
                           "match the expected bytes for nuewframe/fast"
        competitor = report_mod.competitor_losses(recs)
        self.assertEqual(len(competitor), 1)
        self.assertTrue(competitor[0].startswith(
            report_mod.COMPETITOR_LOSS_PREFIX))
        self.assertIn("nuewframe", competitor[0])
        self.assertIn("output", competitor[0])
        # Competitor-loss evidence does not block the campaign:
        # the slot is a loss for the competitor, not for the
        # campaign, and dimension (1) still rests on the guard's
        # three observed components.
        self.enterContext(collected_usability_artifact())
        outcome = harness_mod.campaign_outcome(
            recs, FULL_COUNTS, decisions=shipped_decisions())
        self.assertEqual(outcome["campaign_status"], "complete")
        text = report_mod.render(recs, FULL_COUNTS, {"guard": {}},
                                 decisions=shipped_decisions())
        self.assertIn("Competitor losses (evidence", text)
        self.assertIn("output", text)

    def test_shipped_config_names_the_output_workload(self):
        cfg_path = os.path.join(HERE, "..", "campaign.json")
        with open(cfg_path, encoding="utf-8") as handle:
            config = json.load(handle)
        fast = config["children"]["fast"]
        self.assertEqual(fast["args"], ["relay", "0"])
        self.assertEqual(fast["expected_stdout"],
                         fast["stdin"] + "\n")
        self.assertEqual(fast["expected_stderr"], "error-stream\n")


if __name__ == "__main__":
    unittest.main()
