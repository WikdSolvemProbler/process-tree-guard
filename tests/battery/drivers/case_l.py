# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Driver for battery case (l): environment block via
GetEnvironmentStringsW, judged by a vars_os differential.

Windows tier: probe_win_l.py loads the fixture set (the named
shapes plus the generated 500-entry case, from
battery/env_fixtures.py) and runs each fixture in two forms:
as-is, and with a designated set entry (the channel name) whose
existing case-variants must all be skipped and replaced. The
differential exercises the real forms, not two models of them:

- the vars_os side is produced by executing the tree's actual
  builder: a small Rust helper compiled with the tree's own
  src/engine/environment.rs included, run in a process whose
  environment is the fixture installed through a
  CreateProcessW block; the block it emits is
  environment_block's own output (the as-is form's block is
  that same output with a sentinel set entry removed, the
  sentinel chosen to collide with no fixture name), and the
  helper also emits the vars_os entry list it enumerated;
- the GetEnvironmentStringsW side is the helper's own
  unfiltered walk of GetEnvironmentStringsW in that same
  process — every entry it yields, not only names a fixture
  names — serialised by the reference model.

The decider compares the two enumerations themselves as well
as the two blocks: an entry either enumeration yields that the
other does not — a non-fixture entry included — is a named
difference for the losslessness judgement, decidable precisely
because neither side is filtered. One case matcher is used
everywhere a name is matched: the tree's eq_ignore_ascii_case
semantics (ASCII A-Z fold; every other character compares as
itself), implemented identically here, in the probe's model
and in the stub records — Unicode upper()/casefold()
approximations are not used for matching (the sort key remains
the builder's upper-case key, a separate question from the
skip matcher). The duplicate-case fixture, which a real
Windows environment cannot represent, is judged at the list
level and never installed where it would be destroyed before
measurement; its record says so. Adoption requires every
fixture byte-identical in both forms, or every difference
named and lossless with losslessness, preserved skip-all
matching, =C: parsing and stable-sort semantics shown; this
driver evidences only the byte-identical outcome. A fixture
record without the set-entry form, without its builder route,
or without the two enumerations is an unrun charged step:
BLOCKED-ERROR, never evidenced.
Stub tier: the same decision logic over stub_probe.py's records
of recorded block fixtures.
"""

import os
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import driver_common as dc  # noqa: E402
import env_fixtures  # noqa: E402

COND = "byte-level differential over the fixture set"


def ascii_fold(text):
    """The tree's eq_ignore_ascii_case fold
    (src/engine/environment.rs): ASCII A-Z fold to a-z; every
    other character, non-ASCII included, compares as itself."""
    return "".join(chr(ord(c) + 32) if "A" <= c <= "Z" else c
                   for c in text)


def ascii_eq(a, b):
    return ascii_fold(a) == ascii_fold(b)


def _parse(hex_text):
    """Entries of a serialised block, parsed back independently
    of either serialiser: UTF-16LE name=value strings, NUL-ended,
    the block closed by an empty string."""
    data = bytes.fromhex(hex_text)
    entries = []
    for chunk in data.decode("utf-16-le").split("\x00"):
        if not chunk:
            break
        cut = chunk.find("=", 1) if chunk.startswith("=") \
            else chunk.find("=")
        entries.append((chunk[:cut], chunk[cut + 1:]))
    return entries


def _differences(tree_entries, model_entries):
    """Named differences between the two entry lists, and
    whether they are lossless (same multiset of entries)."""
    diffs = []
    if len(tree_entries) != len(model_entries):
        diffs.append(f"entry count differs: tree side "
                     f"{len(tree_entries)}, model side "
                     f"{len(model_entries)}")
    for entry in model_entries:
        if entry not in tree_entries:
            diffs.append(f"entry missing from the tree-side "
                         f"block: {entry[0]}={entry[1]}")
    for entry in tree_entries:
        if entry not in model_entries:
            diffs.append(f"entry present only in the tree-side "
                         f"block: {entry[0]}={entry[1]}")
    if not diffs and tree_entries != model_entries:
        diffs.append("same entries in a different order after "
                     "the documented sort")
    lossless = sorted(tree_entries) == sorted(model_entries)
    return diffs, lossless


def _enumeration_differences(fixture):
    """Named differences between the two recorded enumerations
    themselves (multiset comparison; enumeration order alone is
    not a difference — the documented sort decides order)."""
    vars_os = Counter(tuple(e) for e in fixture["vars_os_entries"])
    gesw = Counter(tuple(e) for e in fixture["gesw_entries"])
    diffs = []
    for entry in sorted((gesw - vars_os).elements()):
        diffs.append(f"entry the GetEnvironmentStringsW "
                     f"enumeration yields and vars_os does not: "
                     f"{entry[0]}={entry[1]}")
    for entry in sorted((vars_os - gesw).elements()):
        diffs.append(f"entry the vars_os enumeration yields and "
                     f"GetEnvironmentStringsW does not: "
                     f"{entry[0]}={entry[1]}")
    return diffs


def decide(obs):
    fixtures = obs.get("fixtures") or []
    if not fixtures:
        return {COND: ("error", "no fixture observations — "
                                "BLOCKED-ERROR, never evidenced")}
    # Charged coverage: the fixture names in the records are
    # compared against env_fixtures.fixture_names() — the named
    # shapes plus the generated 500-entry case. A truncated set
    # (a probe that ran a fraction of the charged fixtures) is
    # BLOCKED-ERROR, never evidenced over the fixtures it ran.
    charged_names = env_fixtures.fixture_names()
    observed_names = [f.get("name") for f in fixtures]
    missing_names = [n for n in charged_names
                     if n not in observed_names]
    if missing_names or len(observed_names) != len(charged_names):
        return {COND: ("error",
                       "the charged fixture set is not present "
                       "in the records: fixtures observed "
                       f"{observed_names}; charged fixtures "
                       f"{charged_names}; missing "
                       f"{missing_names or 'none'} — a truncated "
                       "observation set is BLOCKED-ERROR, never "
                       "evidenced")}
    generated = next(
        (f for f in fixtures
         if f.get("name") == env_fixtures.GENERATED_NAME), None)
    if generated is not None:
        for field in ("vars_os_entries", "gesw_entries"):
            entries = generated.get(field)
            if entries is not None \
                    and len(entries) != env_fixtures.GENERATED_COUNT:
                return {COND: ("error",
                               "the generated 500-entry case "
                               f"records {len(entries)} entries "
                               f"in its {field}, not the charged "
                               f"{env_fixtures.GENERATED_COUNT} — "
                               "a truncated observation set is "
                               "BLOCKED-ERROR, never evidenced")}
    bad = []
    for fixture in fixtures:
        if fixture.get("representable", True) \
                and fixture.get("route") != "executed":
            return {COND: ("error",
                           f"fixture {fixture['name']}: a "
                           "representable fixture was not run "
                           "through the executed builder route "
                           "(the tree's environment_block run in "
                           "a process holding the fixture) — "
                           "BLOCKED-ERROR, never evidenced")}
        if "vars_os_entries" not in fixture \
                or "gesw_entries" not in fixture:
            return {COND: ("error",
                           f"fixture {fixture['name']}: the "
                           "record carries neither the vars_os "
                           "enumeration nor the unfiltered "
                           "GetEnvironmentStringsW enumeration; "
                           "a divergence in an entry no fixture "
                           "names would be undecidable — "
                           "BLOCKED-ERROR, never evidenced")}
        enum_diffs = _enumeration_differences(fixture)
        if enum_diffs:
            bad.append((fixture["name"], "enumeration",
                        enum_diffs, False))
        for form in ("as_is", "with_set"):
            record = fixture.get(form)
            if record is None or (form == "with_set"
                                  and not record.get(
                                      "set_performed")):
                return {COND: ("error",
                               f"fixture {fixture['name']}: the "
                               f"{form} form was not run (the "
                               "set-entry operation the case "
                               "exists to judge is a charged step) "
                               "— BLOCKED-ERROR, never evidenced")}
            tree_entries = _parse(record["tree_hex"])
            model_entries = _parse(record["model_hex"])
            if form == "with_set" and record.get("set_name"):
                set_name = record["set_name"]
                expected = [(set_name, record["set_value"])] \
                    if "set_value" in record else None
                for label, entries in (("tree", tree_entries),
                                       ("model", model_entries)):
                    matched = [e for e in entries
                               if ascii_eq(e[0], set_name)]
                    if len(matched) != 1 \
                            or matched[0][0] != set_name \
                            or (expected is not None
                                and matched != expected):
                        bad.append((
                            fixture["name"], form,
                            [f"the {label} side's set operation "
                             f"did not leave exactly the one "
                             f"set entry under the ASCII-exact "
                             f"matcher: {matched}"],
                            False))
            if record["tree_hex"] != record["model_hex"]:
                diffs, lossless = _differences(tree_entries,
                                               model_entries)
                bad.append((fixture["name"], form, diffs, lossless))
    if not bad:
        return {COND: ("evidenced",
                       f"{len(fixtures)} fixtures byte-identical "
                       "after the documented sort in both forms "
                       "(as-is, and with the channel entry set "
                       "and its case-variants skipped under the "
                       "ASCII-exact matcher), the tree side "
                       "produced by the executed builder and the "
                       "two unfiltered enumerations in agreement "
                       "on every fixture: "
                       + ", ".join(f["name"] for f in fixtures))}
    detail = "; ".join(
        f"{name} ({form}): " + "; ".join(diffs)
        + f" (lossless: {lossless})"
        for name, form, diffs, lossless in bad)
    return {COND: ("refuted",
                   f"{len(bad)} fixture forms differ: " + detail)}


def collect_windows(args):
    observations, record = dc.run_probe(
        "probe_win_l.py", ["--tree", args.tree],
        timeout_s=dc.BUILD_TIMEOUT_S)
    return observations, [record]


if __name__ == "__main__":
    sys.exit(dc.main("l", decide, collect_windows, dc.stub_collect("l")))
