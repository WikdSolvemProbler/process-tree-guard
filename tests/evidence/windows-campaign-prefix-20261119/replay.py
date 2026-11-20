# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Replay descriptive diagnostics of the fixed interrupted Windows campaign prefix."""

import argparse
from collections import Counter
from hashlib import sha256
import json
import math
import statistics
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SEED = 20261029
RECORDED = 1881
PLANNED = 1940
MISSING = 59
PLAN_FIELDS = ("kind", "arm", "path", "context", "rep", "variant")
OBSERVATION_FIELDS = (
    *PLAN_FIELDS, "outcome", "elapsed_ms", "cpu_ms", "peak_working_set_bytes",
    "exit_code", "survivor", "output_stdout_match", "output_stderr_match",
    "output_stdout_len", "output_stderr_len", "slot_job",
)
JOB_FIELDS = ("active_before", "active_after", "terminated")
OUTCOMES = {"ok", "output-mismatch", "descendant-survived", "infra-failure", "exit-mismatch"}
ANALYSIS_FILES = (
    "tests/campaign/arms.py",
    "tests/campaign/plan.py",
    "tests/campaign/report.py",
    "tests/campaign/runner.py",
    "tests/campaign/windows_payload_observer.py",
    "tests/campaign/windows_slot_job.py",
    "tests/envcase.py",
)


def verify_rows(rows, slots):
    if len(rows) != RECORDED or len(slots) != PLANNED:
        raise ValueError("expected the fixed 1,881-row prefix of the 1,940-slot plan")
    for index, (row, slot) in enumerate(zip(rows, slots[:RECORDED], strict=True)):
        if not isinstance(row, dict) or row.keys() != set(OBSERVATION_FIELDS):
            raise ValueError(f"unexpected fields in slot {index}")
        if type(row["rep"]) is not int or any(
            row[field] != getattr(slot, field) for field in PLAN_FIELDS
        ):
            raise ValueError(f"plan identity or order differs in slot {index}")
        if type(row["outcome"]) is not str or row["outcome"] not in OUTCOMES:
            raise ValueError(f"unexpected outcome in slot {index}")
        for field in ("elapsed_ms", "cpu_ms"):
            value = row[field]
            if value is not None and (
                type(value) not in (int, float) or value < 0 or not math.isfinite(value)
            ):
                raise ValueError(f"invalid {field} in slot {index}")
        if row["peak_working_set_bytes"] is not None and (
            type(row["peak_working_set_bytes"]) is not int or row["peak_working_set_bytes"] < 0
        ):
            raise ValueError(f"invalid working-set bytes in slot {index}")
        if row["exit_code"] is not None and (
            type(row["exit_code"]) is not int or not -(2**31) <= row["exit_code"] < 2**32
        ):
            raise ValueError(f"invalid exit code in slot {index}")
        for field in ("survivor", "output_stdout_match", "output_stderr_match"):
            if row[field] is not None and type(row[field]) is not bool:
                raise ValueError(f"invalid {field} in slot {index}")
        for field in ("output_stdout_len", "output_stderr_len"):
            value = row[field]
            if value is not None and (type(value) is not int or value < 0):
                raise ValueError(f"invalid {field} in slot {index}")
        job = row["slot_job"]
        if not isinstance(job, dict) or job.keys() != set(JOB_FIELDS):
            raise ValueError(f"unexpected Job fields in slot {index}")
        if type(job["terminated"]) is not bool or any(
            type(job[field]) is not int or job[field] < 0
            for field in ("active_before", "active_after")
        ):
            raise ValueError(f"invalid Job observation in slot {index}")




EXPECTED_CANONICAL_REFUSAL = json.loads("{\"verdict\":\"REFUSED\",\"package_clean\":false,\"exact_findings\":[\"run.json records no such competitor identity mismatch, but the fingerprints and versions it records give one against the pinned identities in the configuration whose SHA-256 it records: nuewframe: competitor binary not present; identity unconfirmed (expected version 1.0.0); the recorded identity verdict disagrees with the package's evidence\",\"run.json records no such competitor identity mismatch, but the fingerprints and versions it records give one against the pinned identities in the configuration whose SHA-256 it records: processkit: competitor binary not present; identity unconfirmed (expected version 0.3.4); the recorded identity verdict disagrees with the package's evidence\",\"run.json records no such competitor identity mismatch, but the fingerprints and versions it records give one against the pinned identities in the configuration whose SHA-256 it records: sysprims: competitor binary not present; identity unconfirmed (expected version 0.2.4); the recorded identity verdict disagrees with the package's evidence\"],\"findings_sha256\":\"8d89a513b200b428dae1e67169574d307ff3b64462131b6831b636c96e2d46e8\",\"diagnostic_receipt_sha256\":\"5a1702028bd79e639e0a646f295379eaab4ccf69d2151cdd78eccf89e39d6888\",\"interpretation\":\"These are the canonical checker's refusals over the untouched provisional run.json and its empty recorded fingerprint/version maps. They do not assert that staged binaries were physically absent. No maps or verdicts are repaired, and full-package acceptance remains open.\"}")


def verify_canonical_refusal(record):
    if type(record) is not dict or json.dumps(record, sort_keys=True, separators=(",", ":")) != json.dumps(EXPECTED_CANONICAL_REFUSAL, sort_keys=True, separators=(",", ":")):
        raise ValueError("retained canonical REFUSAL ledger differs")


def plan_key(slot):
    key = [getattr(slot, field) for field in PLAN_FIELDS[:-1]]
    if slot.variant is not None:
        key.append(slot.variant)
    return key


def verify_prefix(rows, slots, missing, recorded_state):
    verify_rows(rows, slots)
    if not isinstance(missing, list) or len(missing) != MISSING:
        raise ValueError("expected all 59 missing saved-order keys")
    if any(type(key) is not list or len(key) not in (5, 6)
           or type(key[4]) is not int or any(type(value) is not str for value in key[:4])
           or (len(key) == 6 and type(key[5]) is not str) for key in missing):
        raise ValueError("missing suffix keys have invalid primitive types")
    if missing != [plan_key(slot) for slot in slots[RECORDED:]]:
        raise ValueError("missing keys differ from the exact seeded suffix")
    expected_state = {
        "campaign_status": "incomplete", "provisional": True,
        "slots_run": RECORDED, "slots_planned": PLANNED,
        "standing_loss_count": None, "freeze_gate_fail_count": None,
        "fingerprints": {}, "arm_versions": {},
        "pricing_probe_observations": 0, "reduced": False, "seed": SEED,
    }
    if type(recorded_state) is not dict or recorded_state.keys() != expected_state.keys() or any(
        type(recorded_state[key]) is not type(value) or recorded_state[key] != value
        for key, value in expected_state.items()
    ):
        raise ValueError("original provisional/null-final state differs")


def summarize_prefix(rows, slots, analyzer):
    """Descriptive collected-prefix arithmetic only; no comparisons or gates."""
    measured = [row for row in rows if row["kind"] == "measured"]
    arm_counts = []
    for arm in sorted({row["arm"] for row in rows}):
        cells = [row for row in rows if row["arm"] == arm]
        arm_counts.append({
            "arm": arm, "recorded_slots": len(cells),
            "by_kind": dict(Counter(row["kind"] for row in cells)),
            "by_outcome": dict(Counter(row["outcome"] for row in cells)),
            "harness_termination_flags": sum(row["slot_job"]["terminated"] for row in cells),
        })
    floors = []
    descriptive_cells = []
    for context in analyzer.CONTEXTS:
        direct = analyzer._median(rows, "floor-direct", "fast", context, kind="control")
        spawn = analyzer._median(rows, "floor-spawn", "fast", context, kind="control")
        _, agreed = analyzer.floor_ms(rows, context)
        floors.append({
            "context": context,
            "direct_recorded": sum(row["kind"] == "control" and row["arm"] == "floor-direct" and row["context"] == context for row in rows),
            "spawn_recorded": sum(row["kind"] == "control" and row["arm"] == "floor-spawn" and row["context"] == context for row in rows),
            "direct_median_ms": direct, "spawn_median_ms": spawn,
            "relative_disagreement": (abs(direct - spawn) / max(direct, spawn)
                                      if direct is not None and spawn is not None and max(direct, spawn) > 0 else None),
            "agrees_within_5_percent": agreed,
            "distance_claim": None,
        })
        for path in ("fast", "kill"):
            for arm in analyzer.PRODUCT_ARMS:
                cells = [row for row in measured if row["arm"] == arm
                         and row["path"] == path and row["context"] == context]
                planned = sum(slot.kind == "measured" and slot.arm == arm
                              and slot.path == path and slot.context == context for slot in slots)
                counters = {}
                for field in ("cpu_ms", "peak_working_set_bytes"):
                    values = [row[field] for row in cells if row[field] is not None]
                    counters[field] = {"numeric_count": len(values),
                                       "null_count": len(cells) - len(values),
                                       "median": statistics.median(values) if values else None}
                descriptive_cells.append({
                    "arm": arm, "path": path, "context": context,
                    "planned_measured_slots": planned, "recorded_measured_slots": len(cells),
                    "recorded_outcomes": dict(Counter(row["outcome"] for row in cells)),
                    "eligible_ok_elapsed_samples": sum(row["outcome"] == "ok" and row["elapsed_ms"] is not None for row in cells),
                    "eligible_elapsed_median_ms": analyzer._median(rows, arm, path, context),
                    "arm_process_resource_descriptives": counters,
                })
    return {
        "scope": "DESCRIPTIVE_INTERRUPTED_PREFIX_ONLY_NO_WINNER_RANK_PARITY_OR_CERTIFICATE",
        "canonical_package_verdict": "REFUSED",
        "retained_canonical_findings": len(EXPECTED_CANONICAL_REFUSAL["exact_findings"]),
        "planned_slots": PLANNED, "recorded_slots": len(rows), "missing_slots": MISSING,
        "recorded_by_kind": dict(Counter(row["kind"] for row in rows)),
        "recorded_outcomes": dict(Counter(row["outcome"] for row in rows)),
        "per_arm_counts": arm_counts,
        "infrastructure_failure_slot_keys": [
            [row[field] for field in PLAN_FIELDS[:-1]] + ([row["variant"]] if row["variant"] is not None else [])
            for row in rows if row["outcome"] == "infra-failure"
        ],
        "floor_diagnostics": floors,
        "measured_fast_and_kill_descriptives": descriptive_cells,
        "recorded_slot_jobs": {
            "final_active_zero": sum(row["slot_job"]["active_after"] == 0 for row in rows),
            "harness_termination_flags": sum(row["slot_job"]["terminated"] for row in rows),
            "termination_by_outcome": dict(Counter(row["outcome"] for row in rows if row["slot_job"]["terminated"])),
        },
    }

def verified_inputs(root, manifest):
    """Verify the pinned source revision's analysis and configuration bytes."""
    if manifest["seed"] != SEED or manifest["analysis_sha256"].keys() != set(ANALYSIS_FILES):
        raise ValueError("unexpected analysis inputs")
    for name in ANALYSIS_FILES:
        data = (root / name).read_bytes().replace(b"\r\n", b"\n")
        if sha256(data).hexdigest() != manifest["analysis_sha256"][name]:
            raise ValueError(f"source has a different {name}")
    configuration = manifest["configuration"]
    if configuration["path"] != "tests/campaign.json":
        raise ValueError("unexpected configuration path")
    data = (root / configuration["path"]).read_bytes()
    if sha256(data).hexdigest() != configuration["sha256"]:
        raise ValueError("source has a different campaign configuration")
    return json.loads(data)



def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, default=ROOT,
                        help="checkout of the frozen a95 analysis inputs")
    args = parser.parse_args()
    manifest = json.loads((HERE / "replay.json").read_bytes())
    if manifest.get("diagnostics") is None or manifest.get("slots_sha256") is None:
        raise SystemExit("SOURCE-ONLY PROPOSAL: projection and expected diagnostics do not exist")
    if sha256(Path(__file__).read_bytes().replace(b"\r\n", b"\n")).hexdigest() != manifest["replay_sha256"]:
        raise ValueError("prefix replay source digest differs")
    root = args.source_root.resolve()
    verified_inputs(root, manifest)
    data = (HERE / "slots.jsonl").read_bytes().replace(b"\r\n", b"\n")
    missing_data = (HERE / "missing.json").read_bytes().replace(b"\r\n", b"\n")
    if sha256(data).hexdigest() != manifest["slots_sha256"] or sha256(missing_data).hexdigest() != manifest["missing_sha256"]:
        raise ValueError("prefix or missing-suffix digest differs")
    sys.path.insert(0, str(root / "tests/campaign"))
    import plan
    import report
    rows = [json.loads(line) for line in data.splitlines()]
    slots = plan.build_plan(SEED)
    verify_canonical_refusal(manifest["canonical_package_validation"])
    verify_prefix(rows, slots, json.loads(missing_data), manifest["original_recorded_state"])
    diagnostics = summarize_prefix(rows, slots, report)
    if diagnostics != manifest["diagnostics"]:
        raise ValueError("recomputed prefix descriptive diagnostics differ")
    print(json.dumps(diagnostics, indent=2, sort_keys=True, allow_nan=False))


if __name__ == "__main__":
    main()
