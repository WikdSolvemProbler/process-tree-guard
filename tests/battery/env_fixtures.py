# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""The environment differential fixture set for battery case (l).

The named shapes live in the suite's fixtures/env-fixtures.json. The
500-entry case is produced here by a deterministic generator (no
randomness, no clock, no host state): entry i is a fixed name and
value derived from i alone, so the generated case is byte-stable
across runs and machines. load_fixtures() returns the file's cases
plus the generated one under the name "five-hundred-entries"; the
JSON file ships the generated case verbatim as well, and the suite
test asserts the two agree.
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURE_FILE = os.path.normpath(os.path.join(HERE, "..", "fixtures",
                                             "env-fixtures.json"))
GENERATED_NAME = "five-hundred-entries"
GENERATED_COUNT = 500


def generate_large_entries(count=GENERATED_COUNT):
    """The deterministic 500-entry environment: PTG_VAR_0000 .. with
    value-<i>, in index order."""
    return [[f"PTG_VAR_{i:04d}", f"value-{i}"] for i in range(count)]


def load_fixtures(path=FIXTURE_FILE):
    """Every fixture the differential runs over: the file's named
    shapes plus the generated 500-entry case (appended when the file
    does not already carry it, replaced by the generated entries when
    it does, so the parsed set always contains exactly one)."""
    with open(path, encoding="utf-8") as handle:
        fixtures = json.load(handle)
    generated = {"name": GENERATED_NAME,
                 "entries": generate_large_entries()}
    out = [f for f in fixtures if f["name"] != GENERATED_NAME]
    out.append(generated)
    return out


def fixture_names(path=FIXTURE_FILE):
    return [f["name"] for f in load_fixtures(path)]
