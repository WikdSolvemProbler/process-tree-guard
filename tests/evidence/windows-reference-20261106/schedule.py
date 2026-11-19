"""Pure schedule generator for the Windows MSVC development comparison."""

import argparse
import hashlib
import itertools
import json
import re
import sys


MODES = ("manual", "auto")
WORKLOADS = ("startup", "wait50")
ARMS = ("main", "wake", "bare")
PERMUTATIONS = tuple(itertools.permutations(ARMS))
MACROBLOCKS = 40
CONTROL_SLOTS = 5
DOMAIN = b"wake-msvc-directional-schedule-v1\0"
UINT256_SIZE = 1 << 256


class ScheduleRandom:
    """Portable SHA-256 counter stream with unbiased shuffle indices."""

    def __init__(self, seed):
        self.seed = seed
        self.counter = 0

    def below(self, limit):
        ceiling = UINT256_SIZE - UINT256_SIZE % limit
        while True:
            digest = hashlib.sha256(
                DOMAIN + self.seed + self.counter.to_bytes(8, "big")
            ).digest()
            self.counter += 1
            value = int.from_bytes(digest, "big")
            if value < ceiling:
                return value % limit

    def shuffle(self, items):
        for index in range(len(items) - 1, 0, -1):
            other = self.below(index + 1)
            items[index], items[other] = items[other], items[index]


def make_schedule(seed_hex):
    """Emit the warmup and measured triplets with three control checkpoints."""
    if not isinstance(seed_hex, str) or not re.fullmatch("[0-9a-f]{64}", seed_hex):
        raise ValueError("seed must be 64 lowercase hexadecimal characters")

    random = ScheduleRandom(bytes.fromhex(seed_hex))
    rows = []
    next_triplet = 0

    def controls(checkpoint):
        # The run manifest binds each slot to an independently reviewed fixture.
        for slot in range(CONTROL_SLOTS):
            rows.append(dict(
                index=len(rows), phase="control", checkpoint=checkpoint,
                control_slot=slot,
            ))

    def triplets(phase, macroblock):
        nonlocal next_triplet
        order = list(itertools.product(MODES, WORKLOADS, PERMUTATIONS))
        random.shuffle(order)
        for mode, workload, arms in order:
            for position, arm in enumerate(arms):
                rows.append(dict(
                    index=len(rows), phase=phase, macroblock=macroblock,
                    triplet=next_triplet, mode=mode, workload=workload,
                    position=position, arm=arm,
                ))
            next_triplet += 1

    controls("pre")
    triplets("warmup", None)
    for macroblock in range(MACROBLOCKS):
        if macroblock == MACROBLOCKS // 2:
            controls("mid")
        triplets("measured", macroblock)
    controls("post")
    return dict(schema=1, seed_hex=seed_hex, rows=rows)


def schedule_bytes(seed_hex):
    """Return stable UTF-8 bytes for the manifest digest."""
    payload = json.dumps(make_schedule(seed_hex), sort_keys=True, separators=(",", ":"))
    return (payload + "\n").encode("utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", required=True, help="64 lowercase hexadecimal characters")
    args = parser.parse_args()
    try:
        payload = schedule_bytes(args.seed)
    except ValueError as error:
        parser.error(str(error))
    sys.stdout.buffer.write(payload)


if __name__ == "__main__":
    main()
