# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Validate the finite, during-only final-sample census decision."""

from pathlib import PureWindowsPath
import re

from laptop_ancestry import PRIMARY_STATE, census_admissions, census_allowed, require, validate_ancestry


ROUND_KEYS = {
    "begin_ns", "snapshot_begin_ns", "snapshot_end_ns", "snapshot_upper_ticks",
    "end_error", "entry_count", "entries", "rows", "finished_ns",
}
ENTRY_KEYS = {"pid", "parent_pid", "name"}
ROW_KEYS = ENTRY_KEYS | {"snapshot_upper_ticks", "state"}
OPEN_ERROR_KEYS = ROW_KEYS | {"error", "winerror", "reason", "operation", "handle_acquired"}
IMAGE_KEYS = ROW_KEYS | {"created_ticks", "image_path", "image_sha256", "alive_after", "job_member"}
SHA256 = re.compile(r"[0-9a-f]{64}")


def _pid(value, *, allow_zero=False):
    return type(value) is int and int(not allow_zero) <= value <= 0xFFFFFFFF


def _round(round_, requested_ns, observer_pid, guard_pid, worker_identity,
           relevant, compilers, vetoes):
    from image_policy import classify

    require(type(round_) is dict and set(round_) == ROUND_KEYS, "invalid complete census round")
    start, snapshot_begin, snapshot_end, finished, upper = (round_[key] for key in (
        "begin_ns", "snapshot_begin_ns", "snapshot_end_ns", "finished_ns", "snapshot_upper_ticks",
    ))
    require(all(type(value) is int for value in (start, snapshot_begin, snapshot_end, finished, upper))
            and requested_ns <= start <= snapshot_begin <= snapshot_end <= finished
            <= requested_ns + 1_000_000_000 and upper > 0,
            "census round outside the original deadline")
    entries, rows, count = round_["entries"], round_["rows"], round_["entry_count"]
    require(type(entries) is list and type(rows) is list and type(count) is int
            and count == len(entries) > 0 and type(round_["end_error"]) is int
            and round_["end_error"] == 18, "incomplete native process enumeration")
    require(all(type(entry) is dict and set(entry) == ENTRY_KEYS
                and _pid(entry["pid"], allow_zero=True) and _pid(entry["parent_pid"], allow_zero=True)
                and type(entry["name"]) is str and bool(entry["name"])
                and entry["name"] == entry["name"].casefold() for entry in entries),
            "invalid full snapshot entry")
    pids = {entry["pid"] for entry in entries}
    require(len(pids) == count and observer_pid in pids, "duplicate PID or absent observer")
    projection = [entry for entry in entries if entry["pid"] != observer_pid
                  and (entry["pid"] == guard_pid or entry["name"] in relevant)]
    require(len(rows) == len(projection), "incomplete sampled policy projection")

    worker_found = False
    for row, entry in zip(rows, projection, strict=True):
        require(type(row) is dict and ROW_KEYS <= set(row)
                and all(type(row[key]) is type(value) and row[key] == value
                        for key, value in entry.items())
                and row["pid"] > 0 and type(row["snapshot_upper_ticks"]) is int
                and row["snapshot_upper_ticks"] == upper, "census row differs from its native entry")
        state = row["state"]
        if state == "membership_unverified":
            continue
        stamp = row.get("created_ticks")
        require(type(stamp) is int and 0 < stamp <= upper, "invalid sampled creation identity")
        if row["pid"] == guard_pid:
            require(state == PRIMARY_STATE and set(row) == ROW_KEYS | {"created_ticks"},
                    "invalid primary guard sample")
            continue
        require(set(row) == IMAGE_KEYS and type(row["alive_after"]) is bool
                and type(row["job_member"]) is bool and type(row["image_path"]) is str
                and PureWindowsPath(row["image_path"]).name.casefold() == row["name"],
                "invalid native classification witness")
        member, digest = row["job_member"], row["image_sha256"]
        require(worker_identity is not None or not member, "outside census asserted job membership")
        if member or row["name"] in compilers:
            require(digest is None, "unexpected sampled digest")
        else:
            require(type(digest) is str and SHA256.fullmatch(digest), "missing sampled image digest")
        require(state == classify(row["name"], digest, enclosing=False, member=member,
                                  compiler_names=compilers, veto_sha256=vetoes),
                "sampled image classification differs")
        if worker_identity is not None and row["pid"] == worker_identity["pid"]:
            worker_found = (member and row["alive_after"]
                            and stamp == worker_identity["creation_filetime"])
    require(worker_identity is None or worker_found, "original worker proof missing or changed")
    return pids


def validate_census(rounds, *, phase, requested_ns, observer_pid, worker_identity,
                    image_policy, compiler_names, workload_names, ancestry, held_before, held_after):
    """Return admission or a request for one replacement; all other cases fail closed."""
    from image_policy import ALLOWED

    require(type(rounds) is list and len(rounds) in (1, 2)
            and phase in ("pre", "during", "post") and type(requested_ns) is int
            and requested_ns >= 0, "invalid census request")
    if phase == "during":
        require(type(worker_identity) is dict and set(worker_identity) == {"pid", "creation_filetime"}
                and _pid(worker_identity["pid"]) and type(worker_identity["creation_filetime"]) is int
                and worker_identity["creation_filetime"] > 0, "missing original worker identity")
    else:
        require(worker_identity is None and len(rounds) == 1, "outside census requires one strict round")
    validate_ancestry(ancestry, image_policy, observer_pid)
    roles = {role["pid"] for role in ancestry["roles"]}
    if worker_identity is not None:
        require(worker_identity["pid"] not in roles | {observer_pid}, "worker is an enclosing role")
    guard_pid = ancestry["roles"][0]["pid"]
    compilers = set(compiler_names)
    relevant = compilers | set(workload_names) | set(image_policy["pinned_basenames"])
    vetoes = set(image_policy["veto_sha256"])
    admissions = []

    def assess(round_):
        pids = _round(round_, requested_ns, observer_pid, guard_pid, worker_identity,
                      relevant, compilers, vetoes)
        admitted = census_admissions(round_["rows"], ancestry, held_before, held_after)
        admissions.append(admitted)
        return pids, [row for row in round_["rows"] if not census_allowed([row], admitted, ALLOWED)]

    first = rounds[0]
    _, blockers = assess(first)
    if not blockers:
        require(len(rounds) == 1, "replacement attempted after a clear first census")
        return {"decision": "admit", "replacement_pid": None, "ancestry_admissions": admissions}

    require(phase == "during", "first census contains a veto")
    pids = []
    for row in blockers:
        pid = row["pid"]
        require(set(row) == OPEN_ERROR_KEYS and row["state"] == "membership_unverified"
                and row["error"] == "OSError" and row["operation"] == "open_process"
                and row["handle_acquired"] is False and type(row["winerror"]) is int
                and row["winerror"] == 87 and type(row["reason"]) is str and bool(row["reason"])
                and row["name"] not in compilers and pid not in roles | {worker_identity["pid"]},
                "first census veto does not permit replacement")
        pids.append(pid)
    decision = "replacement_required"
    if len(rounds) == 2:
        second = rounds[1]
        second_pids, second_blockers = assess(second)
        require(first["finished_ns"] <= second["begin_ns"]
                and first["snapshot_upper_ticks"] <= second["snapshot_upper_ticks"],
                "replacement census clocks overlap or reverse")
        require(not second_pids.intersection(pids), "unverified PID is present in replacement snapshot")
        require(not second_blockers, "replacement census contains a veto")
        decision = "admit"
    target = {"replacement_pid": pids[0]} if len(pids) == 1 else {"replacement_pids": sorted(pids)}
    return {"decision": decision, **target, "ancestry_admissions": admissions}
