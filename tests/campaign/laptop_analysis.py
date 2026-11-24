# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Replay the prespecified laptop A comparison statistics."""

import argparse
import hashlib
import json
import sys
from bisect import bisect_left
from collections import defaultdict
from pathlib import Path
from statistics import median

from laptop_schedule import ARMS, MACROBLOCKS, SEED_BYTES, ScheduleRandom, build_schedule, canonical_bytes


BOOTSTRAP_REPS = 100_000
LENGTHS = (1, 3, 6)
ENDPOINTS = ("p50", "p90")
RIVALS = tuple(arm for arm in ARMS if arm != "guard")


def ranked(ordered, numerator, denominator):
    """Nearest rank in a pre-sorted sample."""
    return ordered[(len(ordered) * numerator + denominator - 1) // denominator - 1]


def quantiles(values):
    """Return exact quantiles in half-nanosecond units."""
    if not values:
        return None
    ordered = sorted(values)
    mid = len(ordered) // 2
    return ordered[mid] + ordered[(len(ordered) - 1) // 2], 2 * ranked(ordered, 9, 10)


def span_index(spans):
    merged = []
    for start, end in sorted(spans):
        if type(start) is not int or type(end) is not int or not 0 <= start <= end:
            raise ValueError("invalid replayed observer span")
        if start == end:
            continue
        if merged and start < merged[-1][1]:
            previous = merged.pop()
            start, end = previous[0], max(end, previous[1])
        merged.append((start, end))
    return [span[0] for span in merged], [span[1] for span in merged]


def collect(schedule, observations, spans):
    if len(observations) != len(schedule):
        raise ValueError("full comparison analysis requires every scheduled observation")
    starts, ends = span_index(spans)
    cells = defaultdict(dict)
    for plan_row, record in zip(schedule, observations):
        slot = plan_row.slot
        fields = ("kind", "arm", "path", "context", "rep", "variant")
        if any(record.get(key) != getattr(slot, key) for key in fields):
            raise ValueError("analysis rows are not in the physical schedule")
        if slot.kind != "measured" or slot.path not in ("fast", "kill"):
            continue
        start, end = record["laptop_timing_ns"]
        if type(start) is not int or type(end) is not int or not 0 <= start <= end:
            raise ValueError("invalid raw slot clock")
        index = bisect_left(starts, end) - 1
        overlapped = start < end and index >= 0 and ends[index] > start
        quartet = cells[slot.context, slot.path].setdefault(plan_row.group, {})
        quartet[slot.arm] = (end - start if record["outcome"] == "ok" else None,
                             plan_row.position, plan_row.macroblock, overlapped)
    return cells


def point_of_pairs(pairs, endpoint):
    if not pairs:
        return None
    a = quantiles([current[0] for current, _ in pairs])
    b = quantiles([rival[0] for _, rival in pairs])
    return a[endpoint] - b[endpoint]


def grouped(pairs):
    halves = [[pair for pair in pairs if pair[0][2] // 15 == half] for half in (0, 1)]
    orders = [[pair for pair in pairs if (pair[0][1] < pair[1][1]) == first]
              for first in (True, False)]
    return halves, orders


def pair_diagnostics(groups, endpoint):
    halves, orders = groups
    return {"halves_ns_x2": [point_of_pairs(group, endpoint) for group in halves],
            "precedence_ns_x2": [point_of_pairs(group, endpoint) for group in orders]}


def dependence(pairs):
    differences = [[] for _ in range(MACROBLOCKS)]
    for current, rival in pairs:
        differences[current[2]].append(current[0] - rival[0])
    centers = [median(values) if values else None for values in differences]
    acf = [None] * 6
    if None not in centers:
        mean = sum(centers) / len(centers)
        residuals = [center - mean for center in centers]
        denominator = sum(value * value for value in residuals)
        if denominator:
            acf = [sum(a * b for a, b in zip(residuals, residuals[lag:])) / denominator
                   for lag in range(1, 7)]
    return {"block_median_difference_ns": centers, "acf_lags_1_6": acf}


def bootstrap(cells, points, repetitions):
    intervals = {key: {} for key in points}
    if not points or not repetitions:
        return intervals
    data = {}
    for cell, arm in sorted({(cell, arm) for cell, rival, _ in points for arm in ("guard", rival)}):
        buckets = [[] for _ in range(MACROBLOCKS)]
        for group in cells[cell].values():
            ns, _, block, _ = group[arm]
            buckets[block].append(ns)
        if any(len(values) != 3 or None in values for values in buckets):
            raise ValueError("inference arm lacks a complete timing population")
        data[cell, arm] = buckets

    for length in LENGTHS:
        seed = hashlib.sha256(SEED_BYTES + length.to_bytes(4, "big")).digest()
        rng = ScheduleRandom(b"ptg/laptop-182-001/bootstrap", seed)
        draws = {key: [] for key in points}
        for _ in range(repetitions):
            starts = [rng.below(MACROBLOCKS - length + 1) for _ in range(MACROBLOCKS // length)]
            indexes = [index for start in starts for index in range(start, start + length)]
            by_arm = {key: quantiles([value for index in indexes for value in blocks[index]])
                      for key, blocks in data.items()}
            for key, values in draws.items():
                cell, rival, endpoint = key
                values.append(by_arm[cell, "guard"][endpoint] - by_arm[cell, rival][endpoint])
        for key, values in draws.items():
            values.sort()
            theta = points[key]
            intervals[key][str(length)] = [
                2 * theta - ranked(values, 959, 960),
                2 * theta - ranked(values, 1, 960),
            ]
    return intervals


def direction(intervals, observed, clean, path):
    signs = (observed["halves_ns_x2"] + observed["precedence_ns_x2"]
             + clean["halves_ns_x2"] + clean["precedence_ns_x2"] + [clean["point_ns_x2"]])
    if len(intervals) != len(LENGTHS) or None in signs:
        return "unresolved", False
    ends = list(intervals.values())
    scale = 2 * (2_000_000 if path == "fast" else 10_000_000)
    if all(row[1] < 0 for row in ends) and max(signs) <= 0:
        return "faster", all(row[1] < -scale for row in ends)
    if all(row[0] > 0 for row in ends) and min(signs) >= 0:
        return "slower", all(row[0] > scale for row in ends)
    return "unresolved", False


def analyze(observations, observer_spans_ns, *, repetitions=BOOTSTRAP_REPS):
    """Recompute scope-aware points and intervals without changing the stock gate."""
    if type(repetitions) is not int or not 0 <= repetitions <= BOOTSTRAP_REPS:
        raise ValueError("invalid offline bootstrap length")
    cells = collect(build_schedule(), observations, observer_spans_ns)
    points = {}
    cohorts = {}
    contexts = []
    for cell, table in sorted(cells.items()):
        if len(table) != 90:
            raise ValueError("ordinary measured cell does not hold 90 matched quartets")
        arms = {}
        for arm in ARMS:
            values = [group[arm][0] for group in table.values() if group[arm][0] is not None]
            arms[arm] = {
                "count": len(values), "quantiles_ns_x2": quantiles(values),
                "mean_ns": sum(values) / len(values) if values else None,
                "min_ns": min(values, default=None), "max_ns": max(values, default=None),
            }
        contexts.append({"context": cell[0], "path": cell[1], "arms": arms, "contrasts": []})
        for rival in RIVALS:
            pairs = [(group["guard"], group[rival]) for group in table.values()
                     if group["guard"][0] is not None and group[rival][0] is not None]
            cohorts[cell, rival] = pairs
            if len(pairs) == 90:
                for endpoint in range(len(ENDPOINTS)):
                    points[cell, rival, endpoint] = point_of_pairs(pairs, endpoint)

    intervals = bootstrap(cells, points, repetitions)
    for record in contexts:
        cell = (record["context"], record["path"])
        for rival in RIVALS:
            pairs = cohorts[cell, rival]
            matched_groups = grouped(pairs)
            clean_pairs = [pair for pair in pairs if not (pair[0][3] or pair[1][3])]
            clean_groups = grouped(clean_pairs)
            support = (len(clean_pairs) >= 30
                       and min(map(len, clean_groups[0])) >= 10
                       and min(map(len, clean_groups[1])) >= 5)
            a = record["arms"]["guard"]["quantiles_ns_x2"]
            b = record["arms"][rival]["quantiles_ns_x2"]
            details = []
            for endpoint, name in enumerate(ENDPOINTS):
                delta = a[endpoint] - b[endpoint] if a is not None and b is not None else None
                stats = pair_diagnostics(matched_groups, endpoint)
                clean = pair_diagnostics(clean_groups, endpoint)
                clean.update(point_ns_x2=point_of_pairs(clean_pairs, endpoint),
                             pairs=len(clean_pairs), sufficient_support=support)
                bounds = intervals.get((cell, rival, endpoint), {})
                result, practical = direction(bounds if support else {}, stats, clean, cell[1])
                details.append({
                    "endpoint": name, "point_ns_x2": delta,
                    "point_ms": delta / 2_000_000 if delta is not None else None,
                    "percent_of_rival": 100 * delta / b[endpoint] if delta is not None and b[endpoint] else None,
                    "intervals_ns_x2": bounds, "diagnostics": stats,
                    "observer_exclusion_sensitivity": clean,
                    "direction": result, "practically_meaningful": practical,
                })
            differences = [a[0] - b[0] for a, b in pairs]
            record["contrasts"].append({
                "rival": rival, "complete_oracle_pairs": len(pairs),
                "paired_median_ms": median(differences) / 1_000_000 if pairs else None,
                "dependence": dependence(pairs), "endpoints": details,
            })
    return {"schema": "ptg-laptop-182-001-a-analysis-v1",
            "bootstrap_repetitions_per_length": repetitions,
            "nominal_family_size": 24, "contexts": contexts}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observations", type=Path, required=True)
    parser.add_argument("--observer-spans", type=Path, required=True)
    parser.add_argument("--points-only", action="store_true")
    args = parser.parse_args()
    observations = [json.loads(line) for line in args.observations.read_bytes().splitlines()]
    spans = json.loads(args.observer_spans.read_bytes())
    result = analyze(observations, spans, repetitions=0 if args.points_only else BOOTSTRAP_REPS)
    sys.stdout.buffer.write(canonical_bytes(result))


if __name__ == "__main__":
    main()
