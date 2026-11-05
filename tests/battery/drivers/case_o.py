# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Driver for battery case (o): IOCP teardown proof — the six-item
burden plus the forged-packet case, under the binding conditions.

Windows tier: probe_win_o.py associates a job with a completion
port in a sacrificial process and executes the burden: zero-packet
delivery for a job that empties before, at and after association;
the packet-stream composition recorded as a dependence census (the
dequeue count per tree size, never a claimed fixed count); the
never-dies member consuming the single 5,000 ms budget through
recomputed remainders; fallback invariance when association fails;
port close priced by counting the teardown calls whose return
values confirm them, with no handle leaks across 10,000 cycles
and no consumer left after disassociation; non-inheritance of
the port handle; and the forged-packet case — packets posted by
a separate child process over the shapes the port accepts must
not fake the proof: the packet is wake-only, the verdict comes
only from an authoritative job query at the decision instant,
and no reply field may influence the verdict.
Stub tier: the same decision logic over stub_probe.py's records.

Every decided field is computed from the observations: exactly-
once is the counted number of active-process-zero packets
dequeued per zero-trial, compared to 1; the port-close count is
the number of teardown operations whose own call confirmations
the record carries — an operation list of names without
per-operation confirmations is the probe's narration and is
BLOCKED-ERROR, and a failed call changes the confirmed count
and so the verdict; the fallback condition is decided from two
independent observations per point — the fallback path's
verdict on its job, and a poll verdict from a separate poll run
over a separately created identical job state — and two
verdicts carrying the same observation id are two readings of
one record, which this decider treats as BLOCKED-ERROR, never
evidenced; the non-inheritance condition requires the spawn
record (handle inheritance enabled, the port offered through
the inheritance path, the port's inherit flag clear, and a
deliberately inheritable control handle reported inherited by
the same machinery) — a bare port_inherited claim without it
is BLOCKED-ERROR; the forged verdicts are compared, packet by
packet, against the job query recorded at the same decision
instant. An observation the charge requires but the record
lacks — above all the packet-influence observation behind the
forged-packet case — makes its condition BLOCKED-ERROR, never
evidenced.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import driver_common as dc  # noqa: E402

BUDGET_MS = 5000
ZERO = ("zero-packet delivery, including the "
        "already-empty-at-association case")
COMP = "packet-stream composition (honest dependence census)"
NEVER = "the never-dies budget case"
FALLBACK = "fallback invariance"
CLOSE = "port close / disassociation effects"
NONINHERIT = "non-inheritance"
FORGED = "forged-packet case (binding conditions)"
FALLBACK_POINTS = ("null-port", "invalid-job", "closed-port")
TEARDOWN_OPS = ("create", "associate", "disassociate",
                "drain-check", "close")
ZERO_PHASES = ("before", "at", "after")
PORT_CLOSE_CYCLES = 10000
# The charged forged-packet set: the packet shapes the port
# accepts (bytes transferred, completion key, overlapped value —
# the FORGE_SHAPES probe_win_o.py posts), each in both phases
# (posted while a member lives, and after the job has emptied).
FORGE_SHAPES = [(4, 0, 0), (0, 7, 4096), (7, 0, 0),
                (1, 0xDEAD, 8192), (0xFFFFFFFF, 0xFFFF, 0),
                (8, 3, 16384)]
FORGE_PHASES = ("alive", "empty")


def _zero(obs):
    trials = (obs.get("zero_packet") or {}).get("trials")
    if not trials or any("zero_packets_dequeued" not in t
                         for t in trials):
        return ("error",
                "the zero trials record no count of "
                "active-process-zero packets dequeued; a job query "
                "alone cannot evidence exactly-once delivery — "
                "BLOCKED-ERROR, never evidenced")
    if any("phase" not in t or "empty" not in t for t in trials):
        return ("error",
                "a zero trial records no phase (before / at / "
                "after association) or no empty outcome; the "
                "charged phases and the job's emptiness at return "
                "cannot be checked — BLOCKED-ERROR, never "
                "evidenced")
    phases = [t["phase"] for t in trials]
    missing = [p for p in ZERO_PHASES if p not in phases]
    if missing:
        return ("error",
                "the charged zero-packet phases are not all "
                f"present in the records: phases observed "
                f"{phases}; missing {missing} — a truncated "
                "observation set is BLOCKED-ERROR, never "
                "evidenced")
    deliveries = [t["zero_packets_dequeued"] for t in trials]
    empties = [t["empty"] for t in trials]
    ok = deliveries == [1, 1, 1] and all(empties)
    return ("evidenced" if ok else "refuted",
            "active-process-zero packets actually dequeued for a "
            "job emptying before / at / after association: "
            f"{deliveries} (exactly once each required); phases "
            f"recorded: {phases}; the job queried empty at "
            f"return in every trial: {empties}")


def _composition(obs):
    counts = (obs.get("composition") or {}).get("dequeue_counts")
    if not counts or any(
            size not in counts or any(
                k not in counts[size] for k in
                ("exit_packets", "zero_packets", "other_packets",
                 "total_dequeued"))
            for size in ("1", "64", "65", "300")):
        return ("error",
                "the composition record carries no per-size "
                "dequeue counts to compute the census from — "
                "BLOCKED-ERROR, never evidenced")
    consistent = all(
        c["total_dequeued"] == c["exit_packets"] + c["zero_packets"]
        + c["other_packets"]
        and c["zero_packets"] >= 1 for c in counts.values())
    return ("evidenced" if consistent else "refuted",
            "dequeue counts recorded as the dependence, not "
            "claimed as a fixed count (exit packets + zero "
            "packet + other packets = total dequeued, per "
            f"size): {counts}")


def _never_dies(obs):
    nd = obs.get("never_dies") or {}
    if "elapsed_ms" not in nd or "remainder_ms" not in nd:
        return ("error",
                "the never-dies record carries no measured "
                "elapsed time or remainder to decide the budget "
                "case from — BLOCKED-ERROR, never evidenced")
    expected = max(0.0, BUDGET_MS - nd["elapsed_ms"])
    ok = (not nd["empty"] and 4500 <= nd["elapsed_ms"] <= 6000
          and abs(nd["remainder_ms"] - expected) <= 250)
    return ("evidenced" if ok else "refuted",
            "a member that never dies consumed the single "
            f"5,000 ms budget: measured elapsed "
            f"{nd['elapsed_ms']} ms, remainder at return "
            f"{nd['remainder_ms']} ms (the unspent part of the "
            f"budget: {expected} ms), empty={nd['empty']} — the "
            "same (empty, remainder) contract as the poll")


def _fallback(obs):
    points = (obs.get("fallback") or {}).get("points")
    if not points:
        return ("error",
                "the fallback record carries no per-point "
                "observations — BLOCKED-ERROR, never evidenced")
    seen = {p.get("point") for p in points}
    missing = [p for p in FALLBACK_POINTS if p not in seen]
    if missing:
        return ("error",
                "association failure was not injected at every "
                f"point the implementation exposes: missing "
                f"{missing} — an unrun charged step is "
                "BLOCKED-ERROR, never evidenced")
    shared = [p["point"] for p in points
              if not p.get("fallback_obs_id")
              or not p.get("poll_obs_id")
              or p["fallback_obs_id"] == p["poll_obs_id"]]
    if shared:
        return ("error",
                "at these fallback points the two verdicts are "
                "not two independent observations (missing "
                "observation ids, or both verdicts carrying the "
                f"same observation id): {shared} — two readings "
                "of one record evidence nothing about a "
                "fallback; BLOCKED-ERROR, never evidenced")
    equal = all(p["association_failed"]
                and p["fallback_verdict"] == p["poll_verdict"]
                for p in points)
    return ("evidenced" if equal else "refuted",
            "association failure injected at each point the "
            "implementation exposes ("
            + ", ".join(p["point"] for p in points)
            + "); at every point the failure was observed and "
              "the fallback verdict — the behaviour with the "
              "port absent, observed on its own job — equalled "
              "the poll verdict from a separate poll run over "
              f"a separately created identical job state: "
              f"{equal}")


def _port_close(obs):
    pc = obs.get("port_close") or {}
    ops = pc.get("ops")
    if not isinstance(ops, list) or not ops:
        return ("error",
                "the port-close record carries no observed "
                "teardown operation list to count — BLOCKED-ERROR, "
                "never evidenced")
    if any(not isinstance(op, dict) or "op" not in op
           or "confirmed" not in op for op in ops):
        return ("error",
                "the teardown record lists operation names "
                "without each call's own confirmation; a list of "
                "names is the probe's narration of its script, "
                "not an observation — BLOCKED-ERROR, never "
                "evidenced")
    if any(field not in pc for field in
           ("cycles", "leaks", "consumers_after_close")):
        return ("error",
                "the port-close record carries no cycle count, "
                "leak count, or post-close consumer count to "
                "decide from — BLOCKED-ERROR, never evidenced")
    if pc["cycles"] < PORT_CLOSE_CYCLES:
        return ("error",
                f"the port-close record ran {pc['cycles']} "
                f"cycles, below the charged {PORT_CLOSE_CYCLES:,} "
                "leak-checked cycles: a truncated observation "
                "set is BLOCKED-ERROR, never evidenced")
    confirmed = [op["op"] for op in ops if op["confirmed"]]
    unconfirmed = [op["op"] for op in ops if not op["confirmed"]]
    ops_count = len(confirmed)
    sequence_ok = all(op in confirmed for op in TEARDOWN_OPS)
    ok = (ops_count >= 5 and sequence_ok and pc["leaks"] == 0
          and pc["consumers_after_close"] == 0)
    return ("evidenced" if ok else "refuted",
            "the teardown sequence actually performed, counted "
            "from the calls whose return values confirm them ("
            + ", ".join(confirmed)
            + f") counts {ops_count} operations (minimum honest "
              f"count is 5, not 4); calls performed but not "
              f"confirmed: {unconfirmed}; the full sequence "
              f"was confirmed: {sequence_ok}; {pc['leaks']} "
              f"handle leaks across {pc['cycles']} cycles, "
              f"{pc['consumers_after_close']} consumers of the "
              "port left after disassociation")


def _noninherit(obs):
    ni = obs.get("non_inheritance") or {}
    required = ("port_inherited", "inheritance_enabled",
                "port_inherit_flag",
                "control_inheritable_detected", "spawn_mechanism")
    if any(field not in ni for field in required):
        return ("error",
                "the non-inheritance record does not state the "
                "creation attributes it used (inheritance-enabled "
                "spawn, the port's inherit flag, the inheritable "
                "control's outcome, the spawn mechanism); a bare "
                "port_inherited claim cannot evidence the "
                "condition — BLOCKED-ERROR, never evidenced")
    if ni["inheritance_enabled"] is not True:
        return ("error",
                "the checking child was not spawned with handle "
                "inheritance enabled, so no handle could have "
                "crossed whether or not the port was inheritable "
                "— BLOCKED-ERROR, never evidenced")
    if not ni["control_inheritable_detected"]:
        return ("error",
                "the same spawn machinery did not report a "
                "deliberately inheritable control handle as "
                "inherited, so the trial cannot see inheritance "
                "at all — BLOCKED-ERROR, never evidenced")
    return ("evidenced" if not ni["port_inherited"] else "refuted",
            "the checking child was spawned with handle "
            "inheritance enabled and the port handle offered "
            f"through the inheritance path ({ni['spawn_mechanism']}); "
            f"the port's inherit flag at spawn: "
            f"{ni['port_inherit_flag']}; the deliberately "
            "inheritable control handle was reported inherited "
            "by the same machinery; the port handle inherited "
            f"by the child or any descendant: "
            f"{ni['port_inherited']}")


def _forged_phase(trial):
    """The phase a forged trial ran in: the record's own phase
    field when it carries one, otherwise the job state its query
    recorded at the decision instant (a member alive, or the job
    already empty)."""
    if trial.get("phase") in FORGE_PHASES:
        return trial["phase"]
    return "empty" if trial["query_active"] == 0 else "alive"


def _forged(obs):
    fg = obs.get("forged") or {}
    trials = fg.get("trials")
    if not trials or "probe_pid" not in fg or any(
            "verdict_empty" not in t or "query_active" not in t
            or "claim_empty" not in t or "poster_pid" not in t
            or "packet_bytes" not in t or "packet_key" not in t
            or "packet_overlapped" not in t
            for t in trials):
        return ("error",
                "the forged-packet record carries a claim but no "
                "packet-influence observation (per-packet poster, "
                "packet fields, job query and verdict); a "
                "verdict-source string is not evidence — "
                "BLOCKED-ERROR, never evidenced")
    observed = {((t["packet_bytes"], t["packet_key"],
                  t["packet_overlapped"]), _forged_phase(t))
                for t in trials}
    expected = {(shape, phase) for shape in FORGE_SHAPES
                for phase in FORGE_PHASES}
    missing = sorted(expected - observed)
    if missing:
        return ("error",
                "the charged forged-packet set is not present "
                f"in the records: {len(trials)} trials covering "
                f"{len(observed)} packet-shape x phase "
                f"combinations of the charged {len(expected)} "
                f"({len(FORGE_SHAPES)} packet shapes x "
                f"{len(FORGE_PHASES)} phases, alive and empty); "
                f"missing combinations include {missing[:4]} — "
                "a truncated observation set is BLOCKED-ERROR, "
                "never evidenced")
    tracks = all(t["verdict_empty"] == (t["query_active"] == 0)
                 for t in trials)
    influenced = any(
        t["verdict_empty"] == t["claim_empty"]
        and t["claim_empty"] != (t["query_active"] == 0)
        for t in trials)
    separate = all(t["poster_pid"] != fg["probe_pid"]
                   for t in trials)
    ok = tracks and not influenced and separate
    return ("evidenced" if ok else "refuted",
            "packets posted by a separate child process over "
            "the shapes the port accepts "
            f"({len(trials)} trials covering all "
            f"{len(FORGE_SHAPES)} charged packet shapes in both "
            "phases, alive and empty, counted from the records); "
            "computed packet by "
            "packet against the job query at the decision "
            f"instant: the verdict tracks the job query: "
            f"{tracks}; a packet field influenced the verdict: "
            f"{influenced}; every packet was posted from a "
            f"process other than the deciding one: {separate}")


def decide(obs):
    return {
        ZERO: _zero(obs),
        COMP: _composition(obs),
        NEVER: _never_dies(obs),
        FALLBACK: _fallback(obs),
        CLOSE: _port_close(obs),
        NONINHERIT: _noninherit(obs),
        FORGED: _forged(obs),
    }


def collect_windows(args):
    observations, record = dc.run_probe(
        "probe_win_o.py", [], timeout_s=dc.BUILD_TIMEOUT_S)
    return observations, [record]


if __name__ == "__main__":
    sys.exit(dc.main("o", decide, collect_windows, dc.stub_collect("o")))
