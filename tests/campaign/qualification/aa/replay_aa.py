# SPDX-License-Identifier: Apache-2.0
"""Independent read-only replay of one completed native qualification capsule."""

import argparse
import copy
import hashlib
from math import ceil
from statistics import median
from pathlib import Path
import sys

QUALIFIER_DIR = Path(__file__).resolve().parent
if (QUALIFIER_DIR / "__pycache__").exists():
    raise ValueError("qualification bytecode cache prohibited")
sys.path.insert(0, str(QUALIFIER_DIR))

from qualify import EMPTY, NS, OFFSET_NS, pairs, require, slot_oracle, stock_slot_drained


def cohort(rows, kind):
    return [row for row in rows if row["kind"] == kind]


def one(rows, kind):
    matched = cohort(rows, kind)
    require(len(matched) == 1, f"missing or repeated {kind}")
    return matched[0]


def record_identity(row):
    return {field: row[field] for field in ("pid", "creation_filetime")}


def bind_gate_worker(gate, expected):
    require(gate.get("identity") == gate.get("worker_before") == gate.get("worker_after") == expected
            and gate.get("client_pid") == expected["pid"],
            "retained barrier worker identity changed around the census")


def permit(lower, release, higher):
    require(lower <= release <= higher, "timed slot crosses a census permit boundary")


def positional(rows, field, total):
    require([row[field] for row in rows] == list(range(total)), f"missing or duplicate {field}")


def overlap(left, right):
    lo, hi = left
    a, b = right
    return lo < hi and a < b and lo < b and a < hi


def refuse(operation, reason):
    try:
        operation()
    except ValueError as error:
        require(reason in str(error), f"negative control rejected for wrong reason: {error}")
    else:
        raise ValueError(f"negative control passed: {reason}")


def anchors(rows, tail):
    from laptop_telemetry import OUTSIDE_WINDOWS, PERIOD, counter_time, validate_sample, within_transition

    timing = cohort(rows, "sample")
    positional(timing, "index", len(timing))
    require(timing, "no native baseline samples")
    trace_end = one(rows, "outer_trace_end")
    ends = [r["previous_outer_trace"] for r in timing[1:]] + [trace_end]
    require(timing[0]["previous_outer_trace"] is None, "first sample has a previous span")
    spans = []
    for item, end in zip(timing, ends, strict=True):
        require(end["index"] == item["index"] and item["outer_begin_ns"] <= counter_time(item)
                <= counter_time(item) + item["raw"]["read_ns"] <= end["outer_end_ns"],
                "bad baseline full observer span")
        spans.append([item["outer_begin_ns"], end["outer_end_ns"]])
    phases = {phase: [r for r in timing if r["phase"] == phase] for phase in ("pre", "during", "post")}
    require(timing == [r for phase in phases.values() for r in phase], "native calendar phase order differs")
    calendar = []
    for phase, sequence in phases.items():
        complete = len(sequence) - (2 if phase == "during" else 1)
        require(complete >= OUTSIDE_WINDOWS if phase == "during" else complete == OUTSIDE_WINDOWS,
                f"native {phase} calendar coverage is incomplete")
        require([r["endpoint"] for r in sequence] == [False] * (len(sequence) - 1) + [phase == "during"],
                "native sample endpoint population differs")
        require([r["complete_windows"] for r in cohort(rows, "phase_decision") if r["phase"] == phase]
                == [complete], "native phase decision differs from replay")
        for i, value in enumerate(sequence):
            previous = sequence[i - 1] if i else None
            last_start = sequence[i - 2] if i > 1 else None
            validate_sample(value, previous, last_start)
            target = counter_time(sequence[0]) + i * PERIOD
            if i and not value["endpoint"]:
                require(value["outer_begin_ns"] >= target, "ordinary sample ran before unchanged due time")
                calendar.append({"phase": phase, "due_ns": target, "actual_ns": value["outer_begin_ns"],
                                 "full_span_end_ns": spans[value["index"]][1]})
    during = phases["during"]
    within_transition(counter_time(phases["pre"][-1]), counter_time(during[0]))
    within_transition(counter_time(during[-1]), counter_time(phases["post"][0]))
    require(tail == counter_time(during[0]) + 65 * NS and counter_time(during[-1]) >= tail,
            "missing fixed 65-second minimum")
    return spans, calendar, phases


def relay_copy(output, name, row):
    retained = row["aa_streams"]
    require(set(retained) == {"stdout", "stderr"}, "relay retained stream shape differs")
    for io, ref in retained.items():
        expected = f"{name}.{io}"
        require(ref["path"] == expected, "retained relay name differs")
        raw = (output / expected).read_bytes()
        require(len(raw) == ref["size"] == row[f"output_{io}_len"]
                and hashlib.sha256(raw).hexdigest() == ref["sha256"] == row[f"output_{io}_sha256"],
                "retained bytes differ from stock stream observation")


def stock_settlement_evidence(row):
    evidence = {"context": row["context"], "path": row["path"], "slot_job": row["slot_job"]}
    if row["path"] == "kill":
        # Preserve the direct-child PID's original presence or null value.
        evidence["pre_drain_kill"] = {
            field: row[field] for field in ("arm_pid", "child_pid", "grandchild_pid", "survivor")
            if field in row
        }
        evidence["pre_drain_kill"]["tree_marker_recorded"] = True
    return evidence


def replay(output, manifest, manifest_sha, addendum_sha):
    from laptop_barrier import barrier_occupation_ns, require_admission
    from laptop_schedule import canonical_bytes
    from laptop_telemetry import (
        counter_time, decode_sample, INSTRUMENTATION_WORKER_FLAGS, inventory_rows,
        load_rows, resource_stop, score_window,
    )
    from quiet_gate import sample_errors
    import laptop_launch as launch

    plan = pairs()
    source = Path(__file__).with_name("qualify.py")
    receipt = launch.parse_json((output / "recipe.json").read_bytes())
    expected_plan = canonical_bytes(plan)
    require((output / "plan.json").read_bytes() == expected_plan
            and receipt["plan_sha256"] == hashlib.sha256(expected_plan).hexdigest()
            and receipt["driver_sha256"] == hashlib.sha256(source.read_bytes()).hexdigest()
            and receipt["replay_sha256"] == hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
            and receipt["manifest_sha256"] == manifest_sha
            and receipt["addendum_sha256"] == addendum_sha
            and receipt["offset_ns"] == OFFSET_NS and receipt["minimum_during_ns"] == 65 * NS,
            "qualification recipe binds different source, plan or addendum")
    worker = [row for row, _ in load_rows(output / "worker.jsonl")]
    observer = [row for row, _ in load_rows(output / "supervisor.jsonl")]
    barriers = load_rows(output / "barrier.jsonl")
    require(worker[-1]["kind"] == "worker_terminal" and observer[-1]["kind"] == "terminal",
            "missing terminal receipts")
    require(one(worker, "worker_terminal")["passed"] is True
            and one(observer, "terminal")["capture_finished"] is True,
            "capture failed or is incomplete")
    drain = one(observer, "natural_drain")
    require(drain["code"] == 0 and {key: drain[key] for key in EMPTY} == EMPTY,
            "original qualification Job did not settle naturally")
    candidate = manifest["files"]["guard"]["path"]
    fixture = manifest["files"]["fixture"]["path"]
    trusted = manifest["files"]["primary_guard"]["path"]
    backup = one(worker, "backup_timeout")["observation"]
    real_timeout = one(worker, "candidate_timeout")["observation"]
    slot_oracle(backup, "kill", trusted, fixture)
    slot_oracle(real_timeout, "kill", candidate, fixture)
    malformed = one(worker, "candidate_malformed")["observation"]
    require(malformed["exit_code"] == 125 and malformed["outcome"] == "ok"
            and Path(malformed["argv"][0]).samefile(candidate),
            "wrong shape refusal not preserved")
    require(stock_slot_drained(malformed.get("slot_job")), "malformed slot Job did not prove cleanup")
    stock_controls = [
        {"control": name, **stock_settlement_evidence(row)}
        for name, row in (("candidate_malformed", malformed), ("candidate_timeout", real_timeout),
                          ("backup_timeout", backup))
    ]
    live = one(worker, "owned_survivor")
    settled = one(worker, "owned_survivor_settled")
    require(live["creation_flags"] == INSTRUMENTATION_WORKER_FLAGS
            and live["identity"] == settled["identity"] and settled["exit_code"] != 0
            and settled["job"] == {"active_before": 1, "terminated": True, "active_after": 0},
            "owned live process and independent drain witnesses differ")

    baseline_spans, calendar, phases = anchors(observer, receipt["tail_ns"])
    during = phases["during"]
    origin = one(observer, "started")
    require(origin["guard_ancestry"] == receipt["guard_ancestry"]
            and origin["observer_pid"] == receipt["supervisor_pid"]
            and origin["driver_sha256"] == receipt["driver_sha256"]
            and origin["addendum_sha256"] == addendum_sha
            and origin["manifest_sha256"] == manifest_sha, "supervisor origin differs")
    hello = one(observer, "worker_dispatched")
    identity = record_identity(hello)
    require(hello["creation_flags"] == receipt["worker_creation_flags"] == INSTRUMENTATION_WORKER_FLAGS
            and hello["job_member"] is True
            and hello["image_sha256"] == manifest["files"]["python"]["sha256"]
            and Path(hello["image_path"]).samefile(manifest["files"]["python"]["path"])
            and hello["worker_recipe_sha256"] == hashlib.sha256(canonical_bytes(receipt["worker_argv"])).hexdigest(),
            "held worker executable or recipe differs")
    dispatch = cohort(worker, "lane")
    wakes = cohort(observer, "wake")
    release = cohort(worker, "pair_release")
    tail = one(worker, "tail_release")
    pad = one(worker, "fixed_tail")
    require(pad["requested_ns"] == receipt["tail_ns"] <= pad["finished_ns"] <= counter_time(during[-1])
            and tail["released_ns"] <= pad["idle_start_ns"] <= pad["finished_ns"],
            "fixed post-settlement tail worker proof differs")
    positional(dispatch, "lane", 2 * len(plan))
    positional(wakes, "lane", 2 * len(plan))
    positional(release, "pair", len(plan))
    require(len(barriers) == len(plan) + 1, "missing or duplicate pair boundaries")
    outer = cohort(observer, "outside_census")
    outside = {(row["phase"], row["index"]): row for row in outer}
    require(len(outside) == len(outer), "duplicate outside census receipt")
    groups = [(i, "aa-pair", 2 * i - 1) for i in range(len(plan))]
    groups.append((len(plan), "tail", 2 * len(plan) - 1))
    inventories = inventory_rows(output, manifest["image_policy"], origin, phases, groups, outside)
    invasive = {digest: row for row, digest in inventories}
    occupied = barrier_occupation_ns(
        [row for row, _ in barriers], one(observer, "barrier_settled")["ack_settled_ns"])
    require(10 * occupied <= counter_time(during[-1]) - counter_time(during[0]),
            "qualification barrier occupation exceeds 10%")
    previous_request = hello["dispatch_ns"]
    previous_settlement = hello["launched_ns"]
    extra = [row["previous_trace"] for row in wakes[1:]] + [one(observer, "wake_trace_end")]
    require(wakes[0]["previous_trace"] is None, "first extra probe contains stale trace")
    exposure = []
    ordinary_during = [point for point in calendar if point["phase"] == "during"]
    for i, (gate, digest) in enumerate(barriers):
        request = gate["requested_ns"]
        require_admission(previous_request, request, gate["admitted_ns"], hello["deadline_ns"])
        previous_request = request
        end = i == len(plan)
        witness = tail if end else release[i]
        permit_ns = witness["released_ns"] if end else witness["release_ns"]
        area = gate["census"]
        census = invasive[area["sha256"]]
        bind_gate_worker(gate, identity)
        require(gate["manifest_sha256"] == manifest_sha
                and gate["nonce"] == receipt["worker_argv"][receipt["worker_argv"].index("--nonce") + 1]
                and identity == gate["identity"] == census["identity"]
                and identity["pid"] == gate["client_pid"] and witness["barrier_sha256"] == digest
                and gate["group"] == i and gate["settled_through"] == 2 * i - 1
                and gate["label"] == census["label"] == ("tail" if end else "aa-pair")
                and census["errors"] == []
                and previous_settlement <= request == census["requested_ns"] <= gate["admitted_ns"]
                <= area["begin_ns"] == census["begin_ns"] <= census["finished_ns"] <= area["end_ns"]
                <= gate["authorized_ns"] <= permit_ns <= request + NS,
                "invasive witness, participant or non-overlap chain differs")
        previous_settlement = permit_ns
        if end:
            break
        for position, assigned in enumerate(plan[i]["order"]):
            index = 2 * i + position
            lane, wake, trace = dispatch[index], wakes[index], extra[index]
            row = lane["observation"]
            recipe = plan[i]
            path = "fast" if recipe["recipe"] == "relay" else "kill"
            require((row["path"], row["context"], row["rep"]) == (path, recipe["context"], recipe["rep"])
                    and lane["mode"] == wake["mode"] == assigned and trace["lane"] == index
                    and lane["dispatch_ns"] == wake["dispatch_ns"]
                    and wake["requested_ns"] == wake["dispatch_ns"] + OFFSET_NS, "A/A assignment changed")
            low, high = row["laptop_timing_ns"]
            permit(previous_settlement, lane["dispatch_ns"], low)
            permit(low, high, lane["settled_ns"])
            require(lane["dispatch_ns"] <= wake["outer_begin_ns"] <= wake["received_ns"]
                    <= wake["wake_ns"] <= trace["outer_end_ns"]
                    and wake["requested_ns"] <= wake["wake_ns"], "A/A wake chronology differs")
            for point in ordinary_during:
                if point["due_ns"] <= wake["requested_ns"]:
                    require(point["full_span_end_ns"] <= wake["wake_ns"],
                            "earlier or tied ordinary target serviced after assigned probe")
                else:
                    require(not wake["received_ns"] <= point["actual_ns"] < wake["wake_ns"],
                            "later ordinary target serviced before available assigned probe")
            previous_settlement = lane["settled_ns"]
            native = wake["reading"]
            require((native is not None) == (assigned == "on"), "wrong A/A API assignment")
            native_span = None
            if native is not None:
                sample = decode_sample(native["raw"])
                require(native["errors"] == sample_errors(sample, during=True)
                        + resource_stop(native["resources"]) == [], "extra native counter invalid")
                require(wake["wake_ns"] <= native["outer_begin_ns"] <= sample.monotonic_ns
                        <= sample.monotonic_ns + sample.read_ns <= native["outer_end_ns"]
                        <= trace["outer_end_ns"], "extra native read spans differ")
                native_span = [native["outer_begin_ns"], native["outer_end_ns"]]
            slot_oracle(row, path, candidate if path == "fast" else trusted, fixture)
            if path == "fast":
                relay_copy(output, f"lane-{index:03d}", row)
            exposure.append({"lane": index, "pair": i, "mode": assigned,
                             "recipe": recipe["recipe"], "timed_ns": [low, high],
                             **stock_settlement_evidence(row),
                             "dispatch_lag_ns": low - lane["dispatch_ns"],
                             "wake_delay_ns": wake["wake_ns"] - wake["requested_ns"],
                             "ordinary_overlap": [j for j, span in enumerate(baseline_spans)
                                                  if overlap(span, [low, high])],
                             "extra_wake_marker_overlap": overlap([wake["outer_begin_ns"], trace["outer_end_ns"]],
                                                            [low, high]),
                             "extra_native_overlap": native_span is not None and overlap(native_span, [low, high])})

    refuse(lambda: positional(dispatch[:-1], "lane", len(dispatch)), "missing or duplicate")
    duplicate = dispatch[:1] + dispatch[:-1]
    refuse(lambda: positional(duplicate, "lane", len(dispatch)), "missing or duplicate")
    wrong = copy.deepcopy(real_timeout)
    wrong["argv"][0] = fixture
    refuse(lambda: slot_oracle(wrong, "kill", candidate, fixture), "wrong executable")
    refuse(lambda: permit(10, 9, 11), "crosses a census permit")
    slow = copy.deepcopy(during[1])
    slow["raw"]["monotonic_ns"] += 4 * NS
    require("sample_gap" in score_window(during[0], slow, during=True)["errors"], "oversleep control passed")
    cpu = copy.deepcopy(during[1])
    cpu["raw"]["system"]["idle"] = during[0]["raw"]["system"]["idle"] - 1
    require("system_cpu_counter_order" in score_window(during[0], cpu, during=True)["errors"],
            "decreasing raw CPU counter control passed")
    contrasts, summaries = summarize(exposure, plan)
    return {"scope": "native A/A calibration and named controls", "manifest_sha256": manifest_sha,
            "addendum_sha256": addendum_sha, "driver_sha256": receipt["driver_sha256"],
            "replay_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "plan_sha256": receipt["plan_sha256"], "baseline_endpoints": calendar,
            "paired_contrasts": contrasts, "context_recipe_distributions": summaries,
            "stock_controls": stock_controls, "independent_sleeper_drain": settled["job"],
            "forced_slot_drains": {
                "stock_controls": sum(row["slot_job"]["terminated"] for row in stock_controls),
                "aa_lanes": sum(row["slot_job"]["terminated"] for row in exposure),
            },
            "barrier_occupied_ns": occupied, "exposure": exposure,
            "named_negative_controls": ["missing", "duplicate", "wrong_image", "barrier_crossing",
                                        "oversleep", "decreasing_cpu_counter"]}


def summarize(exposure, plan):
    pairs = []
    groups = {}
    for index, recipe in enumerate(plan):
        pair = exposure[2 * index:2 * index + 2]
        require([item["mode"] for item in pair] == recipe["order"], "summary pair assignment differs")
        elapsed = {item["mode"]: item["timed_ns"][1] - item["timed_ns"][0] for item in pair}
        paired = {"pair": index, "context": recipe["context"], "recipe": recipe["recipe"],
                  "order": recipe["order"], "on_ns": elapsed["on"], "off_ns": elapsed["off"],
                  "on_minus_off_ns": elapsed["on"] - elapsed["off"]}
        pairs.append(paired)
        groups.setdefault((recipe["context"], recipe["recipe"]), []).append(paired)

    def distribution(values):
        ordered = sorted(values)
        return {"n": len(ordered), "min_ns": ordered[0], "median_ns": median(ordered),
                "p90_ns": ordered[ceil(0.90 * len(ordered)) - 1], "max_ns": ordered[-1]}

    summaries = []
    for (context, recipe), group in sorted(groups.items()):
        raw = [item for item in exposure if item["context"] == context and item["recipe"] == recipe]
        summaries.append({
            "context": context, "recipe": recipe,
            "on": distribution([item["on_ns"] for item in group]),
            "off": distribution([item["off_ns"] for item in group]),
            "on_minus_off": distribution([item["on_minus_off_ns"] for item in group]),
            "on_actual_native_overlap_count": sum(item["extra_native_overlap"] for item in raw),
            "dispatch_to_stock_start": distribution([item["dispatch_lag_ns"] for item in raw]),
            "wake_delay": distribution([item["wake_delay_ns"] for item in raw]),
        })
    return pairs, summaries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--manifest-sha", required=True)
    parser.add_argument("--addendum", required=True, type=Path)
    parser.add_argument("--addendum-sha", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    require(sys.flags.isolated and sys.flags.dont_write_bytecode, "replay requires Python -I -B")
    tests = args.stage.resolve(strict=True) / "tests"
    require(not any((folder / "__pycache__").exists() for folder in (tests, tests / "campaign")),
            "staged Python bytecode cache prohibited")
    sys.path.insert(0, str(tests / "campaign"))
    import laptop_launch as launch

    manifest = launch.load_manifest(args.manifest, args.manifest_sha)
    launch.code_refs(manifest)
    require(launch.file_identity(args.addendum, args.addendum_sha)
            == manifest["protocol"]["ADDENDUM-A-A-001.md"],
            "replay addendum differs from the sealed campaign protocol")
    launch.bind_observers(manifest)
    receipt = replay(args.output, manifest, args.manifest_sha, args.addendum_sha)
    launch.write_exclusive(args.receipt, receipt)


if __name__ == "__main__":
    main()

