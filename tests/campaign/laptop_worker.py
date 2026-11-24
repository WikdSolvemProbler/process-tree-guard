# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Laptop worker journaling at settled group boundaries."""

import hashlib
import json
import os
import re
import stat
import time
from argparse import Namespace
from itertools import groupby
from pathlib import Path

from laptop_schedule import MODE, PRICING_GROUPS, SLOT_GROUPS, TAIL_GROUP, schedule_sha256
from plan import slot_key
from pricing import PROBES, PROBE_REPS, PROBE_TIMEOUT_DETAIL


CAPTURE_LIMIT_BYTES = 2 * 1024 ** 3


def group_plan(rows):
    """Bind every barrier to the exact previously settled slot or probe."""
    groups = []
    for group, members in groupby(rows, key=lambda row: row.group):
        first = next(members)
        slot = first.slot
        if slot.kind == "control":
            label = f"control:{slot.arm}:{slot.context}:{slot.rep}"
        else:
            label = f"{slot.kind}:{slot.path}:{slot.context}:{slot.rep}:{slot.variant or ''}"
        groups.append((group, label, first.ordinal - 1))
    for probe in PROBES:
        for rep in range(PROBE_REPS):
            offset = len(groups) - SLOT_GROUPS
            groups.append((len(groups), f"pricing:{probe}:{rep}", len(rows) + 2 * offset - 1))
    if len(groups) != TAIL_GROUP:
        raise ValueError("pricing pair population differs from the sealed schedule")
    groups.append((TAIL_GROUP, "tail", len(rows) + 2 * PRICING_GROUPS - 1))
    return groups


def capture_size(path):
    """Check the owned filesystem tree without following reparse points."""
    total = 0
    pending = [path]
    while pending:
        with os.scandir(pending.pop()) as entries:
            for entry in entries:
                info = entry.stat(follow_symlinks=False)
                if (stat.S_ISLNK(info.st_mode)
                        or getattr(info, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT):
                    raise ValueError("capture tree contains a redirected entry")
                if stat.S_ISDIR(info.st_mode):
                    pending.append(entry.path)
                else:
                    total += info.st_size
                    if total > CAPTURE_LIMIT_BYTES:
                        raise ValueError("capture directory exceeds its frozen byte budget")
    return total


def append_record(stream, event):
    stream.write(json.dumps(event, allow_nan=False, separators=(",", ":")) + "\n")
    stream.flush()
    os.fsync(stream.fileno())


class WorkerExecution:
    def __init__(self, capture, rows, gate, journal, pricing_journal, manifest_sha):
        self.capture = capture
        self.rows = rows
        self.groups = group_plan(rows)
        self.gate = gate
        self.journal = journal
        self.pricing_journal = pricing_journal
        self.manifest_sha = manifest_sha
        self.last_index = -1
        self.next_group = 0
        self.active = None
        self.price_end = None
        self.copied_bytes = 0
        self.streams = self.capture / "streams"
        self.streams.mkdir()

    def planned_slots(self, seed):
        from laptop_schedule import SEED

        if seed != SEED:
            raise ValueError("worker seed differs from the frozen laptop protocol")
        return [row.slot for row in self.rows]

    def plan_identity(self):
        return {
            "plan_mode": MODE,
            "schedule_sha256": schedule_sha256(self.rows),
            "laptop_manifest_sha256": self.manifest_sha,
        }

    def emit(self, kind, **fields):
        append_record(self.journal, {"kind": kind, **fields})

    def checkpoint(self, group, label=None):
        expected_group, expected_label, settled_through = self.groups[group]
        if (group != self.next_group or group != expected_group
                or self.last_index != settled_through or self.active is not None
                or (label is not None and label != expected_label)):
            raise ValueError("worker barrier differs from the sealed group order")
        capture_size(self.capture)
        requested = time.perf_counter_ns()
        release = self.gate.checkpoint(group, expected_label, settled_through)
        if type(release) is not int or not requested <= release <= time.perf_counter_ns():
            raise ValueError("barrier returned an invalid worker release clock")
        barrier_sha = self.gate.last_receipt_sha256
        if not isinstance(barrier_sha, str) or re.fullmatch(r"[0-9a-f]{64}", barrier_sha) is None:
            raise ValueError("worker ACK omits the exact barrier receipt digest")
        self.emit("group_release", group=group, label=expected_label,
                  settled_through=settled_through, requested_ns=requested, release_ns=release,
                  barrier_sha256=barrier_sha)
        self.next_group += 1

    def begin(self, index, group, identity):
        if self.active is not None or index != self.last_index + 1 or group != self.next_group - 1:
            raise ValueError("worker item is not next in the active barrier group")
        started = time.perf_counter_ns()
        self.active = (index, group, identity, started)
        self.emit("item_begin", index=index, group=group, identity=identity, begun_ns=started)

    def settle(self, index, identity, bounds, *, pricing_ack=None):
        if self.active is None or self.active[0] != index or self.active[2] != identity:
            raise ValueError("worker cannot settle a different item")
        _, group, _, begun = self.active
        start, end = bounds
        settled = time.perf_counter_ns()
        if not begun <= start <= end <= settled:
            raise ValueError("worker timing falls outside its begin/settlement clock")
        receipt = {"index": index, "group": group, "identity": identity,
                   "settled_ns": settled, "timing_start_ns": start, "timing_end_ns": end}
        if pricing_ack is not None:
            received, deadline = pricing_ack["received_ns"], pricing_ack["deadline_ns"]
            if (type(received) is not int or type(deadline) is not int
                    or not end <= received <= min(deadline, settled)):
                raise ValueError("pricing ACK receipt crossed its deadline or stock return")
            receipt.update(pricing_membership_sha256=pricing_ack["receipt_sha256"],
                           pricing_ack_received_ns=received, pricing_ack_deadline_ns=deadline)
        self.emit("item_settled", **receipt)
        self.last_index = index
        self.active = None

    def before_slot(self, index, slot):
        row = self.rows[index]
        if row.ordinal != index or row.slot != slot:
            raise ValueError("worker slot differs from generated order")
        if row.position == 0:
            self.checkpoint(row.group)
        self.begin(index, row.group, slot_key(slot))

    def after_slot(self, index, record):
        self.settle(index, slot_key(self.rows[index].slot), record["laptop_timing_ns"])
        if record.get("laptop_stream_error"):
            raise ValueError(f"laptop stream evidence incomplete at planned slot {index}")

    def retain_streams(self, record, paths):
        if self.active is None:
            raise ValueError("stream retention has no active planned slot")
        index = self.active[0]
        artifacts = record["laptop_streams"] = {}
        for name, path in paths.items():
            destination = self.streams / f"{index:04d}-{name}.bin"
            digest = hashlib.sha256()
            size = 0
            with open(path, "rb") as source, destination.open("xb") as target:
                while data := source.read(65536):
                    if self.copied_bytes + len(data) > CAPTURE_LIMIT_BYTES:
                        raise ValueError("laptop stream byte budget exhausted")
                    target.write(data)
                    self.copied_bytes += len(data)
                    size += len(data)
                    digest.update(data)
                target.flush()
                os.fsync(target.fileno())
            checksum = digest.hexdigest()
            artifacts[name] = {"name": destination.name, "size": size, "sha256": checksum}
            observed = record.get(f"output_{name}_sha256")
            if observed is not None and (
                observed != checksum or record.get(f"output_{name}_len") != size
            ):
                raise ValueError(f"{name} changed between timing and retained custody")

    def before_pricing_pair(self, probe, rep):
        self.checkpoint(self.next_group, f"pricing:{probe}:{rep}")

    def before_pricing_slot(self, probe, rep, shape):
        self.begin(self.last_index + 1, self.next_group - 1, ["pricing", probe, rep, shape])
        self.price_start = time.perf_counter_ns()

    def after_pricing_slot(self, probe, rep, shape):
        self.price_end = time.perf_counter_ns()
        if self.active is None or self.active[2] != ["pricing", probe, rep, shape]:
            raise ValueError("pricing stopwatch identity differs")

    def retain_pricing_record(self, row):
        index = self.last_index + 1
        identity = ["pricing", row["probe"], row["rep"], row["shape"]]
        append_record(self.pricing_journal, {"index": index, "observation": row})
        if row.get("detail") == PROBE_TIMEOUT_DETAIL:
            raise ValueError(f"pricing item {index} exhausted the finite deadline")
        pricing_ack = self.gate.pricing_settlement(self.next_group - 1, index)
        self.settle(index, identity, (self.price_start, self.price_end), pricing_ack=pricing_ack)
        self.price_end = None

    def finish(self):
        self.checkpoint(TAIL_GROUP)


def run_worker(manifest, manifest_sha, capture, rows, pipe_name, nonce):
    from harness import cmd_run
    from laptop_barrier import BarrierClient

    inputs = manifest["inputs"]
    config = Path(inputs["config"])
    os.chdir(config.parent.parent)
    with (capture / "laptop-worker.jsonl").open("x", encoding="utf-8") as journal, (
        capture / "laptop-pricing.jsonl"
    ).open("x", encoding="utf-8") as pricing_journal, BarrierClient(
        pipe_name, nonce, manifest_sha
    ) as gate:
        worker = WorkerExecution(capture, rows, gate, journal, pricing_journal, manifest_sha)
        success = False
        try:
            args = Namespace(config=str(config), out=str(capture / "campaign"),
                             reduced=None, seed=manifest["seed"], run_id=manifest["run_id"])
            code = cmd_run(args, laptop=worker)
            success = code == 0 and worker.next_group == TAIL_GROUP + 1
            if code == 0 and not success:
                raise ValueError("worker did not reach the sealed tail barrier")
            return code
        finally:
            worker.emit("worker_terminal", success=success, settled_through=worker.last_index)

