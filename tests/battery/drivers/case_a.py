# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Driver for battery case (a): job-handle signalled-when-empty
teardown wait vs the Sleep(10) poll.

Windows tier: probe_win_a.py executes the charged runs at the API
level the adoption decision is about — for tree sizes 0/1/2/64/65
and a member that never dies it races both strategies (poll of the
job's active-process count; wait on the job handle) against the
same job under the single 5,000 ms budget: one job per trial,
terminated once, with both proofs armed on that one job (the job
handle's signalled state is non-consuming and the poll is a read,
so one job carries both waits), and records each strategy's
(empty, remainder) verdict and measured latency; then 1,000 race
repetitions in which the job empties between the non-empty check
and the wait's start.
Stub tier: the same decision logic over stub_probe.py's records.

Every decided field is computed from the observations: the
verdicts are the recorded (empty, remainder) pairs, budget
compliance is decided from the measured latencies and remainders
(a trial record without a remainder, or without the same-job
record, makes the condition BLOCKED-ERROR, never evidenced), and
no probe-written budget constant is consulted. The latency
comparison is noise-honest: the wait may exceed the poll by at
most one poll tick (the Sleep(10) granularity, 10 ms) per trial,
so a sub-tick or single-trial epsilon difference cannot refute
the adoption, while a systematic excess beyond one tick can.
The charged coverage set is enforced before anything is
evidenced: the dying trials at sizes 0/1/2/64/65 and a
never-dies trial must all be present in the records, and the
race must record its charged 1,000 repetitions — a truncated
observation set is BLOCKED-ERROR, never evidenced.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import driver_common as dc  # noqa: E402

BUDGET_MS = 5000
# The poll's own granularity is 10 ms; remainders are compared
# with room for that plus scheduling jitter.
TOLERANCE_MS = 150
BUDGET_SLACK_MS = 250
# The latency tolerance is derived from the poll's tick itself:
# the wait being replaced is judged against a Sleep(10) poll, so
# an excess of at most one poll tick (10 ms) per trial is
# scheduling noise, not a latency difference.
POLL_TICK_MS = 10
LATENCY_TOLERANCE_MS = POLL_TICK_MS

EQ = "verdict equivalence + latency + budget"
RACE = "no lost wakeup at the emptying race"
# The charged trial set: dying trials at these tree sizes, plus
# one trial whose member never dies. A record set missing any
# charged size, or carrying no never-dies trial at all, is a
# truncated observation set: the budget clause that tests the
# never-dies member is vacuous when no such trial exists, so a
# truncated set is BLOCKED-ERROR, never evidenced.
CHARGED_SIZES = (0, 1, 2, 64, 65)


def decide(obs):
    trials = obs.get("trials") or []
    dying_sizes = {t.get("size") for t in trials
                   if not t.get("never_dies")}
    missing_sizes = [s for s in CHARGED_SIZES
                     if s not in dying_sizes]
    has_never_dies = any(t.get("never_dies") for t in trials)
    if missing_sizes or not has_never_dies:
        eq = ("error",
              "the charged trial set is not present in the "
              "records: dying-trial sizes charged "
              f"{list(CHARGED_SIZES)}, sizes missing "
              f"{missing_sizes or 'none'}; a never-dies trial "
              f"is present: {has_never_dies} — a truncated "
              "observation set cannot evidence the condition "
              "(the budget clause is vacuous without the "
              "never-dies member); the verdict is BLOCKED-ERROR, "
              "never evidenced")
        return {EQ: eq, RACE: _race(obs)}
    missing = [t.get("size") for t in trials
               if "poll_remainder_ms" not in t
               or "wait_remainder_ms" not in t]
    unshared = [t.get("size") for t in trials
                if t.get("shared_job") is not True]
    if not trials or missing or unshared:
        eq = ("error",
              "trial records without the (empty, remainder) pair "
              "the charge requires, or without both proofs racing "
              "the same job, cannot evidence the condition: "
              f"trials missing a remainder: {missing or 'all'}; "
              f"trials not recorded as one shared job: "
              f"{unshared or 'none'}; the verdict is "
              "BLOCKED-ERROR, never evidenced")
        race_ok = _race(obs)
        return {EQ: eq, RACE: race_ok}
    verdicts_equal = all(t["poll_empty"] == t["wait_empty"]
                         for t in trials)
    remainders_equal = all(
        abs(t["poll_remainder_ms"] - t["wait_remainder_ms"])
        <= TOLERANCE_MS for t in trials)
    remainders_consistent = all(
        abs(t[f"{s}_remainder_ms"]
            - max(0.0, BUDGET_MS - t[f"{s}_latency_ms"]))
        <= TOLERANCE_MS
        for t in trials for s in ("poll", "wait"))
    latency_ok = all(
        t["wait_latency_ms"] - t["poll_latency_ms"]
        <= LATENCY_TOLERANCE_MS for t in trials)
    excesses = [round(t["wait_latency_ms"] - t["poll_latency_ms"], 3)
                for t in trials]
    latencies = sorted({t[f"{s}_latency_ms"] for t in trials
                        for s in ("poll", "wait")})
    budget_ok = all(lat <= BUDGET_MS + BUDGET_SLACK_MS
                    for lat in latencies) and all(
        t[f"{s}_latency_ms"] >= BUDGET_MS - BUDGET_SLACK_MS
        and t[f"{s}_remainder_ms"] <= TOLERANCE_MS
        for t in trials if not t["poll_empty"] or not t["wait_empty"]
        for s in ("poll", "wait"))
    eq_status = "evidenced" if (verdicts_equal and remainders_equal
                                and remainders_consistent
                                and latency_ok and budget_ok) \
        else "refuted"
    eq_detail = (
        f"{len(trials)} paired trials (sizes "
        + "/".join(str(t['size']) for t in trials)
        + f"); (empty, remainder) verdicts equal: {verdicts_equal} "
          f"with remainders agreeing: {remainders_equal} and each "
          f"remainder the unspent part of the single budget: "
          f"{remainders_consistent}; signalled-wait latency "
          f"exceeds poll latency by at most one poll tick "
          f"({LATENCY_TOLERANCE_MS} ms, the Sleep(10) "
          f"granularity) in every trial: {latency_ok} "
          f"(per-trial excesses {excesses} ms, both proofs "
          f"racing the same job in each trial); the single "
          f"5,000 ms budget bound every trial, computed from the "
          f"measured latencies {latencies} ms and the remainders "
          f"at return: {budget_ok}")
    return {EQ: (eq_status, eq_detail), RACE: _race(obs)}


def _race(obs):
    race = obs.get("race") or {}
    if "repetitions" not in race or "lost_wakeups" not in race:
        return ("error",
                "the race record carries no repetition count or "
                "lost-wakeup count to decide from — BLOCKED-ERROR, "
                "never evidenced")
    if race["repetitions"] < 1000:
        return ("error",
                f"{race['repetitions']} race repetitions "
                "recorded, below the charged 1,000: a truncated "
                "race cannot evidence the condition — "
                "BLOCKED-ERROR, never evidenced")
    ok = race.get("lost_wakeups") == 0
    return ("evidenced" if ok else "refuted",
            f"{race.get('repetitions')} race repetitions, "
            f"{race.get('lost_wakeups')} lost wakeups")


def collect_windows(args):
    observations, record = dc.run_probe(
        "probe_win_a.py", [], timeout_s=dc.BUILD_TIMEOUT_S)
    return observations, [record]


if __name__ == "__main__":
    sys.exit(dc.main("a", decide, collect_windows, dc.stub_collect("a")))
