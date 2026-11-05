# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Driver for battery case (n): lazy console verdicts, gated on
before/after verdict equality across the Console::finish cycle.

Windows tier: probe_win_n.py runs the charged matrix — standard
handles (console / pipe / file / closed) x every role program x
runs with and without post-finish output pending — plus the
API-level verdict cells (the three standard handles' console
verdicts recorded eagerly, the FreeConsole/AttachConsole cycle
Console::finish performs, the verdicts recorded lazily after).
Pending output is real: in a pending cell the workload leaves
output unread on its handles at the finish point, and the probe
records the held byte count it measures; in a non-pending cell
it leaves none. Role cells record verdict-shaped evidence, not
exit codes alone: each cell carries the role's full decision
output bytes and the consulted verdict values (for the gate,
the decision its named-vendor payload produced — a guarded
payload whose verdict can vary; for the run roles, the standard
handles' console verdicts the workload child measured on the
handles the role arranged; the contain role runs inside a real
enclosing extendable run, never as a bare refusal). Adoption is
gated on full equality; on any inequality the fallback is to
pin, before Console::finish, only the verdicts with pending
post-finish output — the probe then re-runs the failing cells
with those verdicts pinned and records the re-measured lazy
verdicts alongside the pinned set.
Stub tier: the same decision logic over stub_probe.py's records.

Every decided field is computed from the observations: the
pending dimension is evidenced only where the recorded held
byte counts differ observably between the pending and
non-pending cells, and every role cell's held count must equal
the pending-pattern bytes actually measured for that cell —
in the drained stream, or in the producer's write record where
no drain exists — so a label-assigned held count refutes;
role-cell equality is computed by pairing each role program's
cells across the pending dimension and comparing the exit code,
the decision output bytes and the consulted verdict values
together; and the fallback's effective verdicts are computed
here from the pinned set and the re-measured lazy verdicts — a
fallback record that merely restates eager values is
BLOCKED-ERROR, never evidenced. A role cell recorded as unable
to run in its shape (blocked_error) makes the condition
BLOCKED-ERROR, never a pairing. The charged matrix itself is
counted from the records before anything is evidenced — every
handle configuration x every role program x pending and
non-pending, and every API cell: a truncated matrix is
BLOCKED-ERROR, never evidenced, and the evidenced detail
states the observed counts.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import driver_common as dc  # noqa: E402

COND = "verdict equality across the finish cycle"
HANDLE_INDEX = {"input": 0, "output": 1, "error": 2}
ROLE_FIELDS = ("decision_output_hex", "decision_verdict",
               "held_pattern_bytes", "held_measured")
# The charged matrix, counted from the records before anything
# is evidenced: every handle configuration x every role program
# x pending and non-pending for the role cells, and every handle
# configuration x pending and non-pending for the API cells.
MATRIX_HANDLES = ("console", "pipe", "file", "closed")
MATRIX_ROLES = ("full", "deadline", "gate", "contain")


def _effective(cell):
    """The fallback's effective verdicts, computed from the
    pinned set and the lazy verdicts re-measured after pinning."""
    return [cell["eager"][i] if name in cell["pinned_handles"]
            else cell["lazy"][i]
            for name, i in HANDLE_INDEX.items()]


def decide(obs):
    cells = obs.get("cells") or []
    if not cells or any("held_output_bytes" not in c for c in cells):
        return {COND: ("error",
                       "the matrix record carries no per-cell "
                       "held-output observation; the pending "
                       "dimension cannot be evidenced — "
                       "BLOCKED-ERROR, never evidenced")}
    if any(c["kind"] == "role"
           and c.get("role_present") is False for c in cells):
        return {COND: ("error",
                       "a charged role program was not present "
                       "in the build folder; its cells ran "
                       "nothing — BLOCKED-ERROR, never evidenced")}
    role_cells = [c for c in cells if c["kind"] == "role"]
    if any(c.get("blocked_error") for c in role_cells):
        return {COND: ("error",
                       "a role program could not run in its "
                       "cell's shape (for example the contain "
                       "role outside an enclosing extendable "
                       "run); its cells are BLOCKED-ERROR, never "
                       "paired as equality")}
    if any(any(field not in c for field in ROLE_FIELDS)
           for c in role_cells):
        return {COND: ("error",
                       "a role cell carries no decision output, "
                       "consulted verdict values, or measured "
                       "pending-pattern count; exit codes alone "
                       "cannot evidence verdict equality — "
                       "BLOCKED-ERROR, never evidenced")}
    api_cells_all = [c for c in cells if c["kind"] == "api"]
    expected_role = {(h, r, p) for h in MATRIX_HANDLES
                     for r in MATRIX_ROLES for p in (False, True)}
    observed_role = {(c.get("handles"), c.get("role"),
                      c.get("pending_output")) for c in role_cells}
    expected_api = {(h, p) for h in MATRIX_HANDLES
                    for p in (False, True)}
    observed_api = {(c.get("handles"), c.get("pending_output"))
                    for c in api_cells_all}
    if observed_role != expected_role \
            or len(role_cells) != len(expected_role) \
            or observed_api != expected_api \
            or len(api_cells_all) != len(expected_api):
        missing_role = sorted(
            f"{h}/{r}/pending={p}" for h, r, p
            in expected_role - observed_role)
        missing_api = sorted(
            f"{h}/pending={p}" for h, p
            in expected_api - observed_api)
        return {COND: ("error",
                       "the charged matrix is not present in "
                       f"the records: observed {len(role_cells)} "
                       f"role cells and {len(api_cells_all)} API "
                       f"cells, against the charged "
                       f"{len(expected_role)} role cells (console "
                       "/ pipe / file / closed x every role "
                       "program x pending and non-pending) and "
                       f"{len(expected_api)} API cells; missing "
                       f"role cells: {missing_role or 'none'}; "
                       f"missing API cells: "
                       f"{missing_api or 'none'} — a truncated "
                       "observation set is BLOCKED-ERROR, never "
                       "evidenced")}
    pending_varied = all(
        c["held_output_bytes"] > 0 for c in cells
        if c["pending_output"]) and all(
        c["held_output_bytes"] == 0 for c in cells
        if not c["pending_output"])
    if not pending_varied:
        return {COND: ("refuted",
                       "the pending dimension was not actually "
                       "varied: a pending cell produced no held "
                       "post-finish output, or a non-pending cell "
                       "produced some; cells recorded under "
                       "different pending labels are the same run "
                       "and evidence nothing")}
    label_assigned = [
        f"{c['handles']}/{c['role']}/pending={c['pending_output']}"
        for c in role_cells
        if c["held_output_bytes"] != c["held_pattern_bytes"]]
    if label_assigned:
        return {COND: ("refuted",
                       "role cells whose held_output_bytes does "
                       "not equal the pending-pattern bytes "
                       "measured for the cell (drained from its "
                       "stream or counted in the producer's "
                       "write record): the held count was "
                       "assigned from the pending label, not "
                       "measured — " + ", ".join(label_assigned))}
    def decision_of(cell):
        return (cell["exit_code"], cell["decision_output_hex"],
                json.dumps(cell["decision_verdict"],
                           sort_keys=True))
    role_pairs = {}
    for cell in role_cells:
        key = (cell["handles"], cell["role"])
        role_pairs.setdefault(key, {})[
            cell["pending_output"]] = decision_of(cell)
    if any(set(pair) != {False, True} for pair in role_pairs.values()):
        return {COND: ("error",
                       "a role-program cell is missing one side "
                       "of the pending pairing — BLOCKED-ERROR, "
                       "never evidenced")}
    role_unequal = [f"{h}/{r}" for (h, r), pair in role_pairs.items()
                    if pair[False] != pair[True]]
    api_cells = [c for c in cells if c["kind"] == "api"]
    unequal = [c for c in api_cells if c["eager"] != c["lazy"]]
    if not unequal and not role_unequal:
        return {COND: ("evidenced",
                       f"observed {len(api_cells)} handle-"
                       "configuration x pending-output API cells "
                       f"and {len(role_cells)} role cells forming "
                       f"all {len(role_pairs)} role-program "
                       "pairings — the full charged matrix, "
                       "counted from the records (console / pipe "
                       "/ file / closed x every role program x "
                       "pending and non-pending, the pending "
                       "cells' held output measured in bytes and "
                       "equal to the pending-pattern bytes counted "
                       "for each cell, and each pairing's exit "
                       "code, decision output bytes and consulted "
                       "verdict values equal across the pairing): "
                       "the verdicts consulted and their values "
                       "under lazy probing equal those under eager "
                       "probing across the whole Console::finish "
                       "FreeConsole/AttachConsole cycle; the pin "
                       "fallback was not needed")}
    fallback = obs.get("fallback") or {}
    fb_cells = fallback.get("cells")
    if fallback.get("needed") and fb_cells is None:
        return {COND: ("error",
                       "the fallback record restates a conclusion "
                       "without the pinned re-run's re-measured "
                       "lazy verdicts and pinned set; effective "
                       "verdicts cannot be computed from it — "
                       "BLOCKED-ERROR, never evidenced")}
    preserved = bool(fb_cells) and all(
        _effective(fb) == fb["eager"] for fb in fb_cells)
    covered = {(fb["handles"], fb["pending_output"])
               for fb in fb_cells or []}
    preserved = preserved and all(
        (c["handles"], c["pending_output"]) in covered
        for c in unequal) and not role_unequal
    if preserved:
        return {COND: ("evidenced",
                       f"{len(unequal)} API cells unequal under "
                       "lazy probing; the fallback — pin, before "
                       "Console::finish, only the verdicts with "
                       "pending post-finish output — preserves "
                       "equality on the failing cells, its "
                       "effective verdicts computed from the "
                       "pinned set and the re-measured lazy "
                       "verdicts")}
    return {COND: ("refuted",
                   f"{len(unequal)} of {len(api_cells)} observed "
                   f"API cells (and {len(role_cells)} observed "
                   "role cells) unequal under lazy probing ("
                   + ", ".join(
                       f"{c['handles']}/pending="
                       f"{c['pending_output']}" for c in unequal)
                   + f"); role pairings unequal: {role_unequal}; "
                     "the fallback — pin, before "
                     "Console::finish, only the verdicts with "
                     "pending post-finish output — preserves "
                     f"equality on the failing cells: {preserved}")}


def collect_windows(args):
    observations, record = dc.run_probe(
        "probe_win_n.py", ["--build-dir", args.build_dir],
        timeout_s=dc.BUILD_TIMEOUT_S)
    return observations, [record]


if __name__ == "__main__":
    sys.exit(dc.main("n", decide, collect_windows, dc.stub_collect("n")))
