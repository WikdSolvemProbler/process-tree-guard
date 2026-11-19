"""Replay the published numerical projection of the November 6 Windows comparison."""

import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import random
import sys

HERE = Path(__file__).resolve().parent
SCHEDULE_SOURCE_SHA256 = "2f80bcc0ca933f216b0477545bb1b81d71ef4783b9e904d9728616e89bc169ba"
schedule_path = HERE / "schedule.py"
schedule_source = schedule_path.read_bytes().replace(b"\r\n", b"\n")
if hashlib.sha256(schedule_source).hexdigest() != SCHEDULE_SOURCE_SHA256:
    raise ValueError("schedule generator digest differs")
sys.path.insert(0, str(HERE))
import schedule

if Path(schedule.__file__).resolve() != schedule_path:
    raise ValueError("schedule generator is outside this replay package")

LENGTHS = (4, 2, 8)
REPLICATES = 100_000
SEED_DOMAIN = b"wake-msvc-directional-mbb-v1"
STRATA = tuple((mode, workload) for mode in schedule.MODES for workload in schedule.WORKLOADS)
ENDPOINTS = ("raw_q50", "raw_q90", "overhead_q50", "overhead_q90")
BLOCKS = schedule.MACROBLOCKS
LANE_BITS = 8
DOT_SHIFT = LANE_BITS * (BLOCKS - 1)
LANE_MASK = (1 << LANE_BITS) - 1

def require(condition, message):
    if not condition:
        raise ValueError(message)


def positive_integer(value):
    return type(value) is int and value > 0


def is_int(value, expected):
    return type(value) is int and value == expected


def distinct_keys(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def parse(raw):
    def reject_constant(value):
        raise ValueError(f"nonfinite JSON constant: {value}")

    def finite_float(value):
        parsed = float(value)
        require(math.isfinite(parsed), f"nonfinite JSON number: {value}")
        return parsed

    return json.loads(raw, object_pairs_hook=distinct_keys, parse_constant=reject_constant, parse_float=finite_float)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canon(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


def valid_sha(value):
    return isinstance(value, str) and len(value) == 64 and all(
        char in "0123456789abcdef" for char in value)


def nearest_rank(size, numerator, denominator):
    return (size * numerator + denominator - 1) // denominator


def series(triplet):
    _, values = triplet
    main, wake, bare = (values[arm] for arm in schedule.ARMS)
    return main, wake, main - bare, wake - bare


def contrasts(marginals):
    return tuple(
        marginals[main][rank] - marginals[main + 1][rank]
        for main in (0, 2) for rank in (0, 1))


def point(triplets):
    ranks = [nearest_rank(len(triplets), p, 10) - 1 for p in (5, 9)]
    marginals = []
    for column in zip(*(series(t) for t in triplets)):
        ordered = sorted(column)
        marginals.append(tuple(ordered[index] for index in ranks))
    return contrasts(marginals)


class WeightedRanks:
    """Rank a resampled marginal using packed macroblock prefix counts."""

    def __init__(self, entries):
        self.values = []
        self.prefixes = []
        prefix = 0
        for value, block in sorted(entries):
            self.values.append(value)
            prefix += 1 << (LANE_BITS * block)
            self.prefixes.append(prefix)
        require(len(entries) <= LANE_MASK, "packed rank lanes would carry")
        self.ranks = tuple(nearest_rank(len(entries), p, 10) for p in (5, 9))

    def at(self, packed_weights, rank):
        # Reversing the weight lanes makes the middle convolution lane the
        # exact weighted count. Every lane is at most 40 blocks * 6 values,
        # so the base-256 product has no carries between lanes.
        left, right = 0, len(self.prefixes) - 1
        while left < right:
            middle = (left + right) // 2
            count = (self.prefixes[middle] * packed_weights >> DOT_SHIFT) & LANE_MASK
            if count < rank:
                left = middle + 1
            else:
                right = middle
        return self.values[left]

    def pair(self, packed_weights):
        return tuple(self.at(packed_weights, rank) for rank in self.ranks)


def packed_draw(rng, length):
    limit = BLOCKS - length + 1
    counts = [0] * BLOCKS
    for _ in range(BLOCKS // length):
        start = rng.getrandbits(6)
        while start >= limit:
            start = rng.getrandbits(6)
        for block in range(start, start + length):
            counts[block] += 1
    return int.from_bytes(bytes(reversed(counts)), "little")


def bootstrap(data, points, length, repeats):
    rankers = {}
    for stratum in STRATA:
        entries = [[] for _ in range(4)]
        for index, block in enumerate(data[stratum]):
            for triplet in block:
                for column, value in enumerate(series(triplet)):
                    entries[column].append((value, index))
        rankers[stratum] = [WeightedRanks(marginal) for marginal in entries]
    samples = {stratum: [[] for _ in ENDPOINTS] for stratum in STRATA}
    seed = int.from_bytes(hashlib.sha256(SEED_DOMAIN + bytes([length])).digest(), "big")
    rng = random.Random(seed)
    for _ in range(repeats):
        weights = packed_draw(rng, length)
        for stratum in STRATA:
            values = contrasts([ranker.pair(weights) for ranker in rankers[stratum]])
            for column, value in zip(samples[stratum], values):
                column.append(value)
    lo = nearest_rank(repeats, 1, 640) - 1
    hi = nearest_rank(repeats, 639, 640) - 1
    intervals = {}
    for stratum in STRATA:
        intervals[stratum] = []
        for estimate, column in zip(points[stratum], samples[stratum]):
            column.sort()
            intervals[stratum].append([2 * estimate - column[hi], 2 * estimate - column[lo]])
    return intervals


def classify(interval):
    lower, upper = interval
    if upper < 0:
        return "win"
    if lower > 0:
        return "slower"
    return "unresolved"


def paired_description(data):
    output = []
    for mode, workload in STRATA:
        differences = sorted(values["main"] - values["wake"]
                             for block in data[mode, workload] for _, values in block)
        q50, q90 = (differences[nearest_rank(len(differences), p, 10) - 1] for p in (5, 9))
        output.append(dict(mode=mode, workload=workload, q50_ns=q50, q90_ns=q90))
    return output


def summarize(data, host_reasons, repeats):
    points = {stratum: point([triplet for block in data[stratum] for triplet in block])
              for stratum in STRATA}
    intervals = {length: bootstrap(data, points, length, repeats) for length in LENGTHS} if not host_reasons else {}
    output = []
    for stratum in STRATA:
        blocks = data[stratum]
        triplets = [triplet for block in blocks for triplet in block]
        half_points = [point([triplet for block in half for triplet in block])
                       for half in (blocks[:BLOCKS // 2], blocks[BLOCKS // 2:])]
        permutation_points = {"/".join(order): point([t for t in triplets if t[0] == order])
                              for order in schedule.PERMUTATIONS}
        for index, endpoint in enumerate(ENDPOINTS):
            record = dict(mode=stratum[0], workload=stratum[1], endpoint=endpoint,
                          estimate_ns=points[stratum][index],
                          half_estimate_ns=[half[index] for half in half_points],
                          permutation_estimate_ns={k: v[index] for k, v in permutation_points.items()},
                          intervals_ns={}, interval_classification={}, reasons=list(host_reasons),
                          verdict="unresolved")
            if intervals:
                for length in LENGTHS:
                    key = f"L{length}"
                    interval = intervals[length][stratum][index]
                    record["intervals_ns"][key] = interval
                    record["interval_classification"][key] = classify(interval)
                primary = record["interval_classification"]["L4"]
                if any(value != primary for value in record["interval_classification"].values()):
                    record["reasons"].append("block_length_classification_differs")
                diagnostics = [*record["half_estimate_ns"], *record["permutation_estimate_ns"].values()]
                if (primary == "win" and any(value > 0 for value in diagnostics)) or (
                        primary == "slower" and any(value < 0 for value in diagnostics)):
                    record["reasons"].append("opposite_half_or_permutation_sign")
                if not record["reasons"]:
                    record["verdict"] = primary
            output.append(record)
    return output


HOST_FLAGS = ("interrupted", "suspend_resume", "major_instrumentation_drift")
PHASE_COUNTS = {"control": 15, "warmup": 72, "measured": 2880}


def lf(path):
    return path.read_bytes().replace(b"\r\n", b"\n")


def load_projection():
    expected = parse(lf(HERE / "expected.json"))
    require(is_int(expected["schema"], 1), "unsupported numerical projection")
    protocol = expected["protocol"]
    require(protocol["schedule_source_sha256"] == SCHEDULE_SOURCE_SHA256,
            "recorded schedule generator digest differs")
    generated = schedule.schedule_bytes(protocol["schedule_seed_hex"])
    require(sha(generated) == protocol["schedule_sha256"], "regenerated schedule differs")
    plan = parse(generated)["rows"]
    require(Counter(row["phase"] for row in plan) == PHASE_COUNTS == protocol["phase_counts"],
            "incorrect planned population")
    bootstrap_config = protocol["bootstrap"]
    require(bootstrap_config["replicates"] == REPLICATES and
            tuple([bootstrap_config["primary_length"], *bootstrap_config["sensitivity_lengths"]]) == LENGTHS and
            bootstrap_config["tail"] == "1/640", "bootstrap configuration differs")
    flags = protocol["archived_host_flags"]
    require(set(flags) == set(HOST_FLAGS) and all(type(flag) is bool for flag in flags.values())
            and not flags["interrupted"], "archived host flags differ")
    reasons = [flag for flag in HOST_FLAGS if flags[flag]]
    raw = lf(HERE / "slots.jsonl")
    require(sha(raw) == expected["slots_sha256"], "projected slot digest differs")
    slots = raw.splitlines()
    require(len(slots) == len(plan) == 2967, "projected slot population differs")
    measured = defaultdict(dict)
    for projection, scheduled in zip(slots, plan):
        row = parse(projection)
        require(set(row) == {"index", "status", "elapsed_ns"} and
                is_int(row["index"], scheduled["index"]) and row["status"] == "PASS",
                f"slot {scheduled['index']} identity or status differs")
        elapsed = row["elapsed_ns"]
        budget = 3_000_000_000 if scheduled.get("workload") == "startup" else 10_000_000_000
        require(positive_integer(elapsed) and elapsed <= budget,
                f"slot {scheduled['index']} duration is invalid")
        if scheduled["phase"] == "measured":
            key = (scheduled["mode"], scheduled["workload"], scheduled["macroblock"], scheduled["triplet"])
            members = measured[key]
            position = scheduled["position"]
            require(position not in members, "duplicate triplet position")
            members[position] = (scheduled["arm"], elapsed)

    data = {stratum: [[] for _ in range(BLOCKS)] for stratum in STRATA}
    for (mode, workload, block, _), members in measured.items():
        require(set(members) == {0, 1, 2}, "incomplete triplet")
        ordered = [members[position] for position in range(3)]
        permutation = tuple(arm for arm, _ in ordered)
        require(permutation in schedule.PERMUTATIONS, "invalid triplet launch order")
        data[mode, workload][block].append((permutation, dict(ordered)))
    for blocks in data.values():
        require(all(Counter(order for order, _ in block) == Counter(schedule.PERMUTATIONS)
                    for block in blocks), "incorrect macroblock launch order coverage")
    return expected, data, reasons


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check-inputs", action="store_true", help="verify the published data and schedule only")
    parser.add_argument("--out", type=Path, help="write the regenerated numerical report to a new file")
    args = parser.parse_args()
    if args.check_inputs and args.out:
        parser.error("--out requires a full replay")
    expected, data, reasons = load_projection()
    if args.check_inputs:
        print("Verified all 2967 projected slots and the frozen schedule.")
        return
    endpoints = summarize(data, reasons, REPLICATES)
    paired = paired_description(data)
    report = {"endpoints": endpoints, "paired_differences_descriptive_only": paired}
    require(report == expected["numerical_results"], "replayed numbers differ from the archived result")
    if args.out:
        with args.out.open("x", encoding="utf8", newline="\n") as handle:
            handle.write(json.dumps(report, sort_keys=True, indent=2, allow_nan=False) + "\n")
    verdicts = Counter(row["verdict"] for row in endpoints)
    print("Verified all 16 contrasts and 4 paired summaries. " +
          ", ".join(f"{count} {name}" for name, count in sorted(verdicts.items())) + ".")


if __name__ == "__main__":
    main()
