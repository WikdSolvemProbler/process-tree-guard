# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Replay the numerical projection of the November 17 Windows campaign."""

from collections import Counter
from hashlib import sha256
import json
import math
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SEED = 20261029
PLAN_FIELDS = ("kind", "arm", "path", "context", "rep", "variant")
OBSERVATION_FIELDS = (
    *PLAN_FIELDS, "outcome", "elapsed_ms", "cpu_ms", "peak_working_set_bytes",
    "exit_code", "survivor", "output_stdout_match", "output_stderr_match",
    "output_stdout_len", "output_stderr_len", "slot_job",
)
JOB_FIELDS = ("active_before", "active_after", "terminated")
OUTCOMES = {"ok", "output-mismatch", "descendant-survived", "infra-failure"}
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
    if len(rows) != 1940 or len(rows) != len(slots):
        raise ValueError("expected all 1,940 seeded slots")
    for index, (row, slot) in enumerate(zip(rows, slots, strict=True)):
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
            if type(value) not in (int, float) or value < 0 or not math.isfinite(value):
                raise ValueError(f"invalid {field} in slot {index}")
        if type(row["peak_working_set_bytes"]) is not int or row["peak_working_set_bytes"] < 0:
            raise ValueError(f"invalid working-set bytes in slot {index}")
        if type(row["exit_code"]) is not int or not -(2**31) <= row["exit_code"] < 2**32:
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


def summarize(rows, analyzer, config=None):
    """Replay numerical calculations and count the recorded labels."""
    measured = [row for row in rows if row["kind"] == "measured"]
    comparisons = []
    floors = []
    survivors = []
    streams = []
    for context in analyzer.CONTEXTS:
        for path in ("fast", "kill"):
            medians = {
                arm: analyzer._median(rows, arm, path, context)
                for arm in analyzer.PRODUCT_ARMS
            }
            excluded = []
            for arm in analyzer.PRODUCT_ARMS:
                if arm == "guard" or path != "kill":
                    continue
                samples = [
                    row for row in measured if row["arm"] == arm
                    and row["path"] == path and row["context"] == context
                ]
                if samples and all(
                    row["outcome"] == "descendant-survived"
                    and row["survivor"] is True for row in samples
                ):
                    excluded.append(arm)
            rivals = {arm: value for arm, value in medians.items()
                      if arm != "guard" and arm not in excluded}
            outcome, _ = analyzer._latency_comparison(medians["guard"], rivals)
            comparisons.append({
                "context": context,
                "path": path,
                "median_ms": medians,
                "recorded_kill_outcome_exclusions": excluded,
                "arithmetic_outcome": outcome,
            })
        direct = analyzer._median(rows, "floor-direct", "fast", context, kind="control")
        spawn = analyzer._median(rows, "floor-spawn", "fast", context, kind="control")
        _, agreed = analyzer.floor_ms(rows, context)
        floors.append({
            "context": context,
            "direct_median_ms": direct,
            "spawn_median_ms": spawn,
            "relative_disagreement": abs(direct - spawn) / max(direct, spawn),
            "agreed": agreed,
        })
        for arm in analyzer.PRODUCT_ARMS:
            cells = [row for row in measured
                     if row["arm"] == arm and row["context"] == context]
            fast = [row for row in cells if row["path"] == "fast"]
            streams.append({
                "arm": arm,
                "context": context,
                "measured_fast_slots": len(fast),
                "stdout_matches": sum(row["output_stdout_match"] is True for row in fast),
                "stderr_matches": sum(row["output_stderr_match"] is True for row in fast),
            })
            for path in ("kill", "scale"):
                slots = [row for row in cells if row["path"] == path]
                survivors.append({
                    "arm": arm,
                    "context": context,
                    "path": path,
                    "slots": len(slots),
                    "flagged_survivor": sum(row["survivor"] is True for row in slots),
                    "unobserved": sum(row["survivor"] is None for row in slots),
                })
    losses, unavailable, compared, ranks = analyzer._paired_cost_comparison_detail(rows, config)
    return {
        "slot_counts": dict(Counter(row["kind"] for row in rows)),
        "recorded_outcomes": dict(Counter(row["outcome"] for row in rows)),
        "latency": comparisons,
        "floors": floors,
        "resource_arithmetic": {
            "compared_units": len(compared),
            "unavailable_units": len(unavailable),
            "loss_units": len(losses),
            "failing_ranks": sum(count for _, count in ranks),
            "failing_ranks_by_unit": dict(ranks),
        },
        "recorded_fast_stream_flags": streams,
        "recorded_survivor_flags": survivors,
        "recorded_slot_jobs": {
            "empty_at_final_observation": sum(row["slot_job"]["active_after"] == 0 for row in rows),
            "harness_termination_flag": sum(row["slot_job"]["terminated"] for row in rows),
        },
    }


def main():
    manifest = json.loads((HERE / "replay.json").read_bytes())
    if manifest["seed"] != SEED or manifest["analysis_sha256"].keys() != set(ANALYSIS_FILES):
        raise ValueError("unexpected analysis inputs")
    for name in ANALYSIS_FILES:
        data = (ROOT / name).read_bytes().replace(b"\r\n", b"\n")
        if sha256(data).hexdigest() != manifest["analysis_sha256"][name]:
            raise ValueError(f"checkout has a different {name}")
    data = (HERE / "slots.jsonl").read_bytes().replace(b"\r\n", b"\n")
    if sha256(data).hexdigest() != manifest["slots_sha256"]:
        raise ValueError("projection digest differs")
    sys.path.insert(0, str(ROOT / "tests/campaign"))
    import plan
    import report

    rows = [json.loads(line) for line in data.splitlines()]
    verify_rows(rows, plan.build_plan(SEED))
    results = summarize(rows, report)
    if results != manifest["results"]:
        raise ValueError("recomputed results differ from the published record")
    print(json.dumps(results, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
