# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Deterministic observed-laptop execution order for protocol 182.001."""

import hashlib
import json
from collections import Counter, defaultdict
from dataclasses import dataclass

from plan import PRODUCT_ARMS, build_plan, check_plan, slot_key


MODE = "laptop-182-001"
SEED = 20261029
SEED_BYTES = SEED.to_bytes(32, "big")
ARMS = ("guard", "nuewframe", "processkit", "sysprims")
WILLIAMS_ROWS = ((0, 1, 3, 2), (1, 2, 0, 3), (2, 3, 1, 0), (3, 0, 2, 1))
MACROBLOCKS = 30
SLOT_GROUPS = 512
PRICING_GROUPS = 27
TAIL_GROUP = SLOT_GROUPS + PRICING_GROUPS


class ScheduleRandom:
    """Use the reference schedule's SHA-256 counter and rejection sampler."""

    def __init__(self, domain, seed):
        self.domain = domain + b"\0"
        self.seed = seed
        self.counter = 0

    def below(self, limit):
        space = 1 << 256
        ceiling = space - space % limit
        while True:
            digest = hashlib.sha256(
                self.domain + self.seed + self.counter.to_bytes(8, "big")
            ).digest()
            self.counter += 1
            value = int.from_bytes(digest, "big")
            if value < ceiling:
                return value % limit

    def shuffle(self, items):
        for index in range(len(items) - 1, 0, -1):
            other = self.below(index + 1)
            items[index], items[other] = items[other], items[index]


@dataclass(frozen=True)
class PlannedSlot:
    ordinal: int
    group: int
    position: int
    macroblock: int | None
    slot: object

    def projection(self):
        return {
            "ordinal": self.ordinal,
            "group": self.group,
            "position": self.position,
            "macroblock": self.macroblock,
            "slot": slot_key(self.slot),
        }


def build_schedule(seed=SEED):
    """Reorder precisely the original slots, keeping their public identities."""
    if type(seed) is not int or seed != SEED:
        raise ValueError(f"{MODE} requires the fixed seed {SEED}")
    if set(PRODUCT_ARMS) != set(ARMS):
        raise ValueError("laptop arm set differs from the original campaign")
    anchored = build_plan(seed)
    check_plan(anchored)
    rng = ScheduleRandom(b"ptg/laptop-182-001/schedule", SEED_BYTES)
    cells = {"warmup": defaultdict(dict), "measured": defaultdict(dict)}
    rows = []

    def append_group(slots, macroblock=None):
        group = rows[-1].group + 1 if rows else 0
        for position, slot in enumerate(slots):
            rows.append(PlannedSlot(len(rows), group, position, macroblock, slot))

    for slot in anchored:
        if slot.kind == "control":
            append_group([slot])
        else:
            cell = (slot.context, slot.path, slot.variant or "")
            repetitions = cells[slot.kind][cell]
            repetitions.setdefault(slot.rep, {})[slot.arm] = slot

    quartets = {"warmup": {}, "measured": {}}
    for phase in ("warmup", "measured"):
        for cell, repetitions in sorted(cells[phase].items()):
            reps = sorted(repetitions)
            for offset in range(0, len(reps), len(WILLIAMS_ROWS)):
                group = reps[offset:offset + len(WILLIAMS_ROWS)]
                arm_order = list(ARMS)
                rng.shuffle(arm_order)
                orders = list(WILLIAMS_ROWS)
                rng.shuffle(orders)
                for rep, order in zip(group, orders):
                    quartets[phase][cell + (rep,)] = [
                        repetitions[rep][arm_order[index]] for index in order
                    ]

    warmups = sorted(quartets["warmup"])
    rng.shuffle(warmups)
    for quartet in warmups:
        append_group(quartets["warmup"][quartet])

    measured = quartets["measured"]
    minor = sorted(key for key in measured if key[1] in ("malformed", "scale"))
    rng.shuffle(minor)
    for block in range(MACROBLOCKS):
        ordinary = sorted(
            key for key in measured
            if key[1] in ("fast", "kill") and key[3] // 3 == block
        )
        group_keys = ordinary + minor[2 * block:2 * block + 2]
        rng.shuffle(group_keys)
        for key in group_keys:
            append_group(measured[key], macroblock=block)

    if rows[-1].group + 1 != SLOT_GROUPS or (
        Counter(tuple(slot_key(row.slot)) for row in rows)
        != Counter(tuple(slot_key(slot)) for slot in anchored)
    ):
        raise ValueError("laptop schedule differs from the anchored slot population")
    return rows


def canonical_bytes(payload):
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False)
    return (encoded + "\n").encode("utf-8")


def schedule_bytes(rows):
    return canonical_bytes({
        "schema": MODE,
        "seed": SEED,
        "slots": [row.projection() for row in rows],
    })


def schedule_sha256(rows):
    return hashlib.sha256(schedule_bytes(rows)).hexdigest()

