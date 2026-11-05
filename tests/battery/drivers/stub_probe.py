# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Stub-tier probe for the battery drivers: emits, for one case,
the observation record its Windows probe would emit, in the same
schema. The "good" variant satisfies the case's adoption
conditions; each broken variant violates exactly one charged
condition, in the raw observations the decider computes from —
never in a summary field the decider would have to trust:

- (a) broken: both strategies' measured latencies on the
  never-dies trial are 10,000 ms — a doubled budget, seen in the
  latencies themselves. broken-remainder: the wait's remainder
  disagrees with the poll's on one trial. broken-early: the
  never-dies wait returns after 1,000 ms with 4,000 ms of budget
  unspent. broken-no-remainder: trials carry no remainder fields
  at all (the decider must block, never evidence).
  broken-latency-2x: the wait is systematically twice the poll's
  latency on every dying trial. noisy (a good variant): the wait
  exceeds the poll by 0.02 ms on the size-0 trial and by 5 ms on
  one dying trial — inside one poll tick, so it must NOT refute.
  broken-truncated: only the size-0 dying trial and the
  never-dies trial are recorded — the charged sizes are absent
  (the decider must block, never evidence).
  broken-no-never-dies: all five dying sizes, but no never-dies
  trial at all — the budget clause would be vacuous (the decider
  must block, never evidence).
- (l) broken: the duplicate-case fixture's tree-side block drops
  the second case-duplicate entry. broken-stale: the set-entry
  form keeps a stale channel entry beside the set one.
  broken-no-set: no fixture carries the set-entry form (the
  decider must block, never evidence). broken-nonfixture: the
  unfiltered GetEnvironmentStringsW enumeration carries an entry
  the vars_os enumeration does not (a name no fixture names),
  while both serialised blocks omit it — decidable only because
  the enumerations themselves are recorded unfiltered.
  broken-matcher: the model side's set operation skips a
  non-ASCII case-variant entry the tree's ASCII-exact matcher
  keeps, so the with-set blocks differ.
  broken-truncated: only the first fixture is recorded — the
  charged fixture set is absent (the decider must block, never
  evidence).
- (m) broken: one substituted site's test run is red.
  broken-consumers: one site reports its consumers unchecked.
  broken-timeout: the first site's build record timed out and
  the remaining sites were never attempted (the decider must
  block as an execution failure, never refute).
  broken-spawn: one site's build failed to spawn and the site
  after it was never attempted (likewise BLOCKED-ERROR).
  broken-truncated: only one site is recorded — the charged
  site set is absent (the decider must block, never evidence).
  Every site record carries timed_out / spawn_failed /
  not_attempted beside the booleans, as collect_windows
  records them.
- (n) broken: one cell's lazy verdicts differ and the fallback's
  pinned re-run still differs on an unpinned handle.
  broken-pending: the pending cells produced no held output —
  the pending dimension was never actually varied.
  broken-pinned-copy: the fallback record restates eager values
  with no re-measured lazy verdicts (the decider must block).
  broken-gate-label: the gate cells report label-assigned held
  bytes (8,192) while the measured pending-pattern count in
  their drained streams is zero. broken-role-verdict: one role
  pairing's consulted verdict values differ across the pending
  pairing while its exit codes stay equal.
  broken-truncated: one role pairing (full/pipe, both pending
  sides) and zero API cells — the charged matrix is absent
  (the decider must block, never evidence).
- (o) broken: a forged packet's claim flips the verdict (the
  verdict tracks the packet, not the job query). broken-zero:
  the zero trials dequeue zero active-process-zero packets.
  broken-ops: the teardown sequence is observed at 4 operations.
  broken-forged-unobserved: the forged record carries a
  verdict-source string but no packet-influence observation
  (the decider must block, never evidence).
  broken-teardown-unconfirmed: the teardown sequence names all
  five operations but the disassociation call's confirmation is
  false, so the confirmed count is 4. broken-inheritable: the
  inheritance-enabled trial reports the port handle inherited.
  broken-no-inheritance-record: the non-inheritance record
  claims port_inherited false without the inheritance-enabled
  spawn record (the decider must block, never evidence).
  broken-fallback-shared: every fallback point's two verdicts
  carry the same observation id — two readings of one record
  (the decider must block, never evidence).
  broken-truncated-zero: the zero trials all carry phase
  "before" — the charged before/at/after phases are absent
  (the decider must block, never evidence). broken-zero-empty:
  one zero trial's job did not query empty at return (refuted).
  broken-cycles: the port-close record ran 1 cycle, below the
  charged 10,000 (the decider must block, never evidence).
  broken-forged-truncated: only three forged trials are
  recorded — the charged packet-shape x phase set is absent
  (the decider must block, never evidence). The good variant's
  forged record carries the full charged set: the six packet
  shapes probe_win_o.py posts, in both phases (alive, empty).

Usage: stub_probe.py <case> <variant>
Prints the observations as JSON on stdout.
"""

import json
import sys

BUDGET_MS = 5000
CHANNEL = "PROCESS_TREE_GUARD_CHANNEL"
SET_VALUE = "\\\\.\\pipe\\set-by-case-l"


def case_a(variant):
    trials = []
    for size in (0, 1, 2, 64, 65):
        trials.append({
            "size": size, "never_dies": False,
            "event": f"single-job-{size}", "shared_job": True,
            "poll_empty": True, "wait_empty": True,
            "poll_latency_ms": 12.0, "wait_latency_ms": 1.0,
            "poll_remainder_ms": 4988.0, "wait_remainder_ms": 4999.0,
        })
    never = {
        "size": 1, "never_dies": True, "event": "single-job-never",
        "shared_job": True,
        "poll_empty": False, "wait_empty": False,
        "poll_latency_ms": 5000.0, "wait_latency_ms": 5000.0,
        "poll_remainder_ms": 0.0, "wait_remainder_ms": 0.0,
    }
    trials.append(never)
    if variant == "broken":
        never["poll_latency_ms"] = 10000.0
        never["wait_latency_ms"] = 10000.0
    elif variant == "broken-remainder":
        trials[2]["wait_remainder_ms"] = 2000.0
    elif variant == "broken-early":
        never["wait_latency_ms"] = 1000.0
        never["wait_remainder_ms"] = 4000.0
    elif variant == "broken-no-remainder":
        for trial in trials:
            del trial["poll_remainder_ms"]
            del trial["wait_remainder_ms"]
    elif variant == "noisy":
        # Sub-tick scheduling noise: +0.02 ms on the size-0 trial
        # (decided by two single syscalls) and +5 ms on one dying
        # trial (inside one 10 ms poll tick). Neither may refute.
        trials[0]["wait_latency_ms"] = 12.02
        trials[0]["wait_remainder_ms"] = 4987.98
        trials[2]["wait_latency_ms"] = 17.0
        trials[2]["wait_remainder_ms"] = 4983.0
    elif variant == "broken-latency-2x":
        # A systematic 2x latency on every dying trial: 24 ms
        # against the poll's 12 ms — beyond one poll tick.
        for trial in trials:
            if not trial["never_dies"]:
                trial["wait_latency_ms"] = 24.0
                trial["wait_remainder_ms"] = 4976.0
    elif variant == "broken-truncated":
        # Only the size-0 dying trial and the never-dies trial:
        # charged sizes 1/2/64/65 were never recorded.
        trials = [trials[0], never]
    elif variant == "broken-no-never-dies":
        # All five dying sizes, but no never-dies trial at all.
        trials = trials[:5]
    return {"trials": trials,
            "race": {"repetitions": 1000, "lost_wakeups": 0}}


def _serialise(entries):
    ordered = sorted(entries, key=lambda kv: kv[0].upper())
    out = bytearray()
    for name, value in ordered:
        out += f"{name}={value}".encode("utf-16-le") + b"\x00\x00"
    out += b"\x00\x00"
    return bytes(out).hex()


def _ascii_fold(text):
    """The tree's eq_ignore_ascii_case fold: ASCII A-Z fold to
    a-z; every other character, non-ASCII included, compares as
    itself. Identical in the probe and in the model."""
    return "".join(chr(ord(c) + 32) if "A" <= c <= "Z" else c
                   for c in text)


def _ascii_eq(a, b):
    return _ascii_fold(a) == _ascii_fold(b)


def _apply_set(entries, name, value, matcher=_ascii_eq):
    kept = [(k, v) for k, v in entries if not matcher(k, name)]
    kept.append((name, value))
    return kept


def _casefold_eq(a, b):
    return a.casefold() == b.casefold()


def case_l(variant):
    raw = {
        "empty": [],
        "empty-value": [("EMPTYVAL", "")],
        "equals-in-value": [("A", "x=y=z")],
        "drive-entries": [("=C:", "C:\\work"), ("=D:", "D:\\")],
        "duplicate-case": [("Path", "C:\\a"), ("PATH", "C:\\b")],
        "unicode-and-case": [("NAME-ü", "välue-✓"),
                             ("ALREADYUPPER", "v"), ("lower", "w"),
                             ("MiXeD", "z"),
                             ("PROCEſS_TREE_GUARD_CHANNEL", "decoy")],
        "channel-entry": [(CHANNEL, "\\\\.\\pipe\\example")],
        "five-hundred-entries": [(f"PTG_VAR_{i:04d}", f"value-{i}")
                                 for i in range(500)],
    }
    fixtures = []
    for name, entries in raw.items():
        entries = [tuple(e) for e in entries]
        tree_as_is = list(entries)
        model_as_is = list(entries)
        tree_set = _apply_set(entries, CHANNEL, SET_VALUE)
        model_set = _apply_set(entries, CHANNEL, SET_VALUE)
        vars_os_entries = [list(e) for e in entries]
        gesw_entries = [list(e) for e in entries]
        if variant == "broken" and name == "duplicate-case":
            tree_as_is = tree_as_is[:1]  # second duplicate dropped
        if variant == "broken-stale" and name == "channel-entry":
            tree_set = list(entries) + [(CHANNEL, SET_VALUE)]
        if variant == "broken-nonfixture" and name == "empty-value":
            # The unfiltered GESW enumeration carries an entry no
            # fixture names and vars_os does not yield; both
            # serialisations above omit it (the old filtered
            # shape), so only the recorded enumerations can
            # decide it.
            gesw_entries = [list(e) for e in entries] + [
                ["HOST_EXTRA_ENTRY", "injected"]]
        if variant == "broken-matcher" and name == "unicode-and-case":
            # The model side skips with a Unicode casefold: it
            # drops the non-ASCII case-variant entry the tree's
            # ASCII-exact matcher keeps.
            model_set = _apply_set(entries, CHANNEL, SET_VALUE,
                                   matcher=_casefold_eq)
        fixture = {
            "name": name,
            "representable": name != "duplicate-case",
            "route": ("list-level" if name == "duplicate-case"
                      else "executed"),
            "builder_route": (
                "rust-helper: the tree's src/engine/environment.rs "
                "compiled in; environment_block executed in a "
                "process whose environment is the fixture"
                if name != "duplicate-case" else
                "list-level over the fixture's entry list"),
            "vars_os_entries": vars_os_entries,
            "gesw_entries": gesw_entries,
            "as_is": {"tree_hex": _serialise(tree_as_is),
                      "model_hex": _serialise(model_as_is)},
        }
        if variant != "broken-no-set":
            fixture["with_set"] = {
                "set_name": CHANNEL, "set_value": SET_VALUE,
                "set_performed": True,
                "tree_hex": _serialise(tree_set),
                "model_hex": _serialise(model_set)}
        fixtures.append(fixture)
    if variant == "broken-truncated":
        # Only the first fixture was recorded; the rest of the
        # charged fixture set never ran.
        fixtures = fixtures[:1]
    return {"fixtures": fixtures}


def case_m(variant):
    sites = []
    for name in ("hardening process fetch", "hardening thread fetch",
                 "owner facts self fetch", "exit path process fetch"):
        sites.append({"name": name, "build_ok": True, "tests_ok": True,
                      "consumers_checked": True,
                      "invalid_handle_misuse": False,
                      "timed_out": False, "spawn_failed": False,
                      "not_attempted": False})
    if variant == "broken":
        sites[2]["tests_ok"] = False
    elif variant == "broken-consumers":
        sites[1]["consumers_checked"] = False
    elif variant == "broken-timeout":
        # The first site's build hit its deadline (the record
        # collect_windows would carry for it); the run stopped
        # there, so the remaining sites were never attempted.
        sites[0].update({"build_ok": False, "tests_ok": False,
                         "timed_out": True})
        for site in sites[1:]:
            site.update({"build_ok": False, "tests_ok": False,
                         "not_attempted": True})
    elif variant == "broken-spawn":
        # One site's build failed to spawn; the site after it
        # was never attempted.
        sites[2].update({"build_ok": False, "tests_ok": False,
                         "spawn_failed": True})
        sites[3].update({"build_ok": False, "tests_ok": False,
                         "not_attempted": True})
    elif variant == "broken-truncated":
        sites = sites[:1]
    return {"sites": sites}


def case_n(variant):
    cells = []
    eager_by_handles = {"console": [True, True, True],
                        "pipe": [False, False, False],
                        "file": [False, False, False],
                        "closed": [False, False, False]}
    for role in ("full", "deadline", "gate", "contain"):
        for handles in ("console", "pipe", "file", "closed"):
            for pending in (False, True):
                held = 8192 if pending else 0
                cells.append({
                    "kind": "role", "handles": handles, "role": role,
                    "pending_output": pending,
                    "held_output_bytes": held,
                    "held_pattern_bytes": held,
                    "held_measured": (
                        "pattern bytes counted in the drained "
                        "stream" if handles in ("pipe", "file")
                        else "producer write record"),
                    "decision_output_hex":
                        f"{role} decision\n".encode().hex(),
                    "decision_verdict": (
                        "allow" if role == "gate"
                        else list(eager_by_handles[handles])),
                    "exit_code": 0})
    for handles in ("console", "pipe", "file", "closed"):
        for pending in (False, True):
            eager = list(eager_by_handles[handles])
            cells.append({
                "kind": "api", "handles": handles,
                "pending_output": pending,
                "held_output_bytes": 4096 if pending else 0,
                "eager": eager, "lazy": list(eager)})
    fallback = {"needed": False, "cells": []}
    if variant == "broken":
        for cell in cells:
            if (cell["kind"] == "api" and cell["handles"] == "pipe"
                    and cell["pending_output"]):
                cell["eager"] = [True, False, False]
                cell["lazy"] = [False, False, False]
        fallback = {"needed": True, "cells": [{
            "handles": "pipe", "pending_output": True,
            "eager": [True, False, False],
            "lazy": [False, False, False],
            "pinned_handles": ["output", "error"]}]}
    elif variant == "broken-pending":
        for cell in cells:
            if cell["pending_output"]:
                cell["held_output_bytes"] = 0
                if "held_pattern_bytes" in cell:
                    cell["held_pattern_bytes"] = 0
    elif variant == "broken-gate-label":
        # Gate cells carry the label's 8,192 held bytes while
        # their drained streams contain no pending pattern at
        # all: the held count was assigned, not measured.
        for cell in cells:
            if (cell["kind"] == "role" and cell["role"] == "gate"
                    and cell["pending_output"]):
                cell["held_pattern_bytes"] = 0
    elif variant == "broken-role-verdict":
        # One role pairing's consulted verdict values differ
        # across the pending pairing; exit codes stay equal.
        for cell in cells:
            if (cell["kind"] == "role" and cell["role"] == "deadline"
                    and cell["handles"] == "pipe"
                    and cell["pending_output"]):
                cell["decision_verdict"] = [True, False, False]
    elif variant == "broken-pinned-copy":
        for cell in cells:
            if (cell["kind"] == "api" and cell["handles"] == "pipe"
                    and cell["pending_output"]):
                cell["lazy"] = [True, True, True]
        fallback = {"needed": True, "preserves_equality": True,
                    "effective": [False, False, False]}
    elif variant == "broken-truncated":
        # One role pairing (full/pipe, both pending sides) and
        # zero API cells: the rest of the charged matrix never
        # ran.
        cells = [c for c in cells
                 if c["kind"] == "role" and c["role"] == "full"
                 and c["handles"] == "pipe"]
    return {"cells": cells, "fallback": fallback}


def case_o(variant):
    # The full charged forged set: the six packet shapes
    # probe_win_o.py posts (FORGE_SHAPES there), each in both
    # phases — posted while a member lives (query_active 1) and
    # after the job has emptied (query_active 0). Every verdict
    # tracks the job query, never the packet's claim.
    forge_shapes = [(4, 0, 0), (0, 7, 4096), (7, 0, 0),
                    (1, 0xDEAD, 8192), (0xFFFFFFFF, 0xFFFF, 0),
                    (8, 3, 16384)]
    forged_trials = []
    for phase, query_active in (("alive", 1), ("empty", 0)):
        for nbytes, key, overlapped in forge_shapes:
            forged_trials.append({
                "poster_pid": 4201, "phase": phase,
                "packet_bytes": nbytes, "packet_key": key,
                "packet_overlapped": overlapped,
                "claim_empty": nbytes == 4,
                "query_active": query_active,
                "verdict_empty": query_active == 0})
    if variant == "broken":
        forged_trials[0]["verdict_empty"] = True  # claim flips it
    elif variant == "broken-forged-truncated":
        forged_trials = forged_trials[:3]
    zero_trials = [
        {"phase": "before", "zero_packets_dequeued": 1,
         "total_dequeued": 2, "empty": True},
        {"phase": "at", "zero_packets_dequeued": 1,
         "total_dequeued": 3, "empty": True},
        {"phase": "after", "zero_packets_dequeued": 1,
         "total_dequeued": 2, "empty": True},
    ]
    if variant == "broken-zero":
        for trial in zero_trials:
            trial["zero_packets_dequeued"] = 0
    elif variant == "broken-truncated-zero":
        for trial in zero_trials:
            trial["phase"] = "before"
    elif variant == "broken-zero-empty":
        zero_trials[1]["empty"] = False
    ops = [{"op": name, "confirmed": True}
           for name in ("create", "associate", "disassociate",
                        "drain-check", "close")]
    if variant == "broken-ops":
        ops = [op for op in ops if op["op"] != "disassociate"]
    elif variant == "broken-teardown-unconfirmed":
        for op in ops:
            if op["op"] == "disassociate":
                op["confirmed"] = False
    fallback_points = [
        {"point": point, "association_failed": True,
         "fallback_verdict": True, "poll_verdict": True,
         "fallback_obs_id": f"fallback-job-{point}",
         "poll_obs_id": f"poll-job-{point}"}
        for point in ("null-port", "invalid-job", "closed-port")]
    if variant == "broken-fallback-shared":
        for point in fallback_points:
            point["poll_obs_id"] = point["fallback_obs_id"]
    non_inheritance = {
        "port_inherited": False, "inheritance_enabled": True,
        "port_inherit_flag": False,
        "control_inheritable_detected": True,
        "spawn_mechanism": "handle inheritance enabled, port "
                           "handle offered through a "
                           "STARTUPINFOEX handle list; port "
                           "created non-inheritable"}
    if variant == "broken-inheritable":
        non_inheritance["port_inherited"] = True
        non_inheritance["port_inherit_flag"] = True
    elif variant == "broken-no-inheritance-record":
        non_inheritance = {"port_inherited": False}
    forged = {"probe_pid": 4100, "trials": forged_trials}
    if variant == "broken-forged-unobserved":
        forged = {"verdict_source": "authoritative job query"}
    return {
        "zero_packet": {"trials": zero_trials},
        "composition": {"dequeue_counts": {
            "1": {"exit_packets": 1, "zero_packets": 1,
                  "other_packets": 0, "total_dequeued": 2},
            "64": {"exit_packets": 64, "zero_packets": 1,
                   "other_packets": 0, "total_dequeued": 65},
            "65": {"exit_packets": 65, "zero_packets": 1,
                   "other_packets": 0, "total_dequeued": 66},
            "300": {"exit_packets": 300, "zero_packets": 1,
                    "other_packets": 0, "total_dequeued": 301}}},
        "never_dies": {"empty": False, "elapsed_ms": 5000.0,
                       "remainder_ms": 0.0},
        "fallback": {"points": fallback_points},
        "port_close": {"ops": ops,
                       "ops_per_cycle": sum(
                           1 for op in ops if op["confirmed"]),
                       "cycles": 1 if variant == "broken-cycles"
                       else 10000, "leaks": 0,
                       "consumers_after_close": 0},
        "non_inheritance": non_inheritance,
        "forged": forged,
    }


CASES = {"a": case_a, "l": case_l, "m": case_m, "n": case_n, "o": case_o}


def main(argv):
    if len(argv) != 3 or argv[1] not in CASES:
        print("usage: stub_probe.py <case> <variant>", file=sys.stderr)
        return 2
    print(json.dumps(CASES[argv[1]](argv[2])))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
