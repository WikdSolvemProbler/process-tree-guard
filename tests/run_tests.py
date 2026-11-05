# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Run every test family in this suite and report the counted-once
total. Each unittest test method is one test, counted once; skipped
tests (guard families on a machine without the built package) are
reported as skipped, never as passed, and never silently dropped.

Usage: python run_tests.py [--count-only] [--tree-sha <commit-sha>]

The tree SHA may also be supplied through the PTG_TREE_SHA
environment variable; the command-line form wins. A full run emits
one certificate per test into certificates/ (see
certificates/README.md): the certificate states the test's name,
its outcome, the run identity and date, the tree SHA the run was
given, and — for benchmark tests — what the test was tested
against. A run made without a valid tree SHA produces certificates
that state on their face that they certify nothing: a test is
certified only when a certificate at the current tree SHA says so,
and a stale SHA is not certified.

Exit code: 0 when no test failed or errored.
"""

import datetime
import json
import os
import re
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TESTS = os.path.join(HERE, "tests")
CERTIFICATES = os.path.join(HERE, "certificates")
sys.path.insert(0, TESTS)
sys.path.insert(0, os.path.join(HERE, "campaign"))
sys.path.insert(0, os.path.join(HERE, "battery"))

# The benchmark test families: the families whose subject is the
# campaign and its competitor arms. Certificates for tests in these
# modules carry the tested-against record; every other family's
# certificates carry tested_against as null (no competitor arm
# applies to them).
BENCHMARK_MODULES = ("test_campaign_logic", "test_output_observation")

_SHA_FORM = re.compile(r"^[0-9a-f]{40}$")

CERTIFIED_STATEMENT = (
    "CERTIFIED: this certificate records the outcome of this test "
    "in the run named here, against the tree SHA stated here. The "
    "test is done only while this SHA is the current tree SHA; a "
    "stale SHA is not certified.")
UNCERTIFIED_STATEMENT = (
    "UNCERTIFIED: no valid tree SHA was supplied to this run (the "
    "--tree-sha argument and the PTG_TREE_SHA environment variable "
    "were both absent or not a full commit SHA), so this record "
    "certifies nothing. It is a run record only.")


def load_suite():
    loader = unittest.TestLoader()
    return loader.discover(TESTS, pattern="test_*.py", top_level_dir=TESTS)


def count(suite):
    return suite.countTestCases()


def tested_against():
    """The competitor arms of the campaign, from the campaign's own
    records (campaign.json's competitor_identities — name, version,
    binary, and SHA-256 pin for each arm as found)."""
    try:
        with open(os.path.join(HERE, "campaign.json"),
                  encoding="utf-8") as handle:
            config = json.load(handle)
        identities = config["competitor_identities"]
    except (OSError, KeyError, ValueError):
        return []
    arms = []
    for arm in sorted(identities):
        spec = identities[arm]
        arms.append({
            "arm": arm,
            "binary": spec.get("binary"),
            "version": spec.get("version"),
            "sha256": spec.get("sha256"),
        })
    return arms


class CertificateResult(unittest.TextTestResult):
    """A TextTestResult that records one certificate entry per test,
    in run order, beside the runner's own accounting."""

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.certificate_records = []

    def _record(self, test, outcome, tier_label=None, detail=None):
        self.certificate_records.append({
            "test": test.id(),
            "outcome": outcome,
            "tier_label": tier_label,
            "detail": detail,
        })

    def addSuccess(self, test):
        super().addSuccess(test)
        self._record(test, "pass")

    def addFailure(self, test, err):
        super().addFailure(test, err)
        self._record(test, "fail", detail=self._exc_info_to_string(err, test))

    def addError(self, test, err):
        super().addError(test, err)
        self._record(test, "fail", detail=self._exc_info_to_string(err, test))

    def addSkip(self, test, reason):
        super().addSkip(test, reason)
        # The tier label is the suite's own skip reason, verbatim.
        self._record(test, "skip", tier_label=reason)

    def addExpectedFailure(self, test, err):
        super().addExpectedFailure(test, err)
        self._record(test, "skip",
                     tier_label="expected failure (the suite's own "
                                "expected-failure form)")

    def addUnexpectedSuccess(self, test):
        super().addUnexpectedSuccess(test)
        self._record(test, "fail", detail="unexpected success")


def emit_certificates(records, tree_sha):
    """Write one certificate per recorded test, plus the run index,
    into certificates/. Certificates from earlier runs for tests
    this run did not produce are removed, so the folder always
    states the latest run's mapping, nothing older."""
    os.makedirs(CERTIFICATES, exist_ok=True)
    certified = bool(tree_sha)
    now = datetime.datetime.now(datetime.timezone.utc)
    run_date = now.strftime("%Y-%m-%dT%H:%M:%SZ")
    run_id = "run-{}-{}".format(
        now.strftime("%Y%m%dT%H%M%SZ"),
        tree_sha[:8] if certified else "no-sha")
    arms = tested_against()
    seen = set()
    for record in records:
        test_id = record["test"]
        seen.add(test_id)
        module = test_id.split(".")[0]
        certificate = {
            "schema": "ptg-test-certificate/1",
            "test": test_id,
            "name": test_id.rsplit(".", 1)[-1],
            "outcome": record["outcome"],
            "tier_label": record["tier_label"],
            "detail": record["detail"],
            "tree_sha": tree_sha,
            "certified": certified,
            "certification_statement": (
                CERTIFIED_STATEMENT if certified
                else UNCERTIFIED_STATEMENT),
            "run_id": run_id,
            "run_date_utc": run_date,
            "tested_against": (
                arms if module in BENCHMARK_MODULES else None),
        }
        path = os.path.join(CERTIFICATES, test_id + ".json")
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(certificate, handle, indent=2, sort_keys=True)
            handle.write("\n")
    for name in sorted(os.listdir(CERTIFICATES)):
        if name.endswith(".json") and name != "index.json":
            if name[:-len(".json")] not in seen:
                os.remove(os.path.join(CERTIFICATES, name))
    outcomes = {}
    for record in records:
        outcomes[record["outcome"]] = outcomes.get(record["outcome"], 0) + 1
    index = {
        "schema": "ptg-test-certificate-index/1",
        "run_id": run_id,
        "run_date_utc": run_date,
        "tree_sha": tree_sha,
        "certified": certified,
        "certification_statement": (
            CERTIFIED_STATEMENT if certified else UNCERTIFIED_STATEMENT),
        "certificates": len(records),
        "outcomes": outcomes,
        "tested_against": arms,
    }
    with open(os.path.join(CERTIFICATES, "index.json"), "w",
              encoding="utf-8") as handle:
        json.dump(index, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return run_id


def resolve_tree_sha(argv):
    """The run's tree SHA: --tree-sha on the command line, else the
    PTG_TREE_SHA environment variable, else None. A value that is
    not a full commit SHA is no SHA: the run certifies nothing."""
    args = list(argv or [])
    candidate = None
    rest = []
    i = 0
    while i < len(args):
        if args[i] == "--tree-sha" and i + 1 < len(args):
            candidate = args[i + 1]
            i += 2
        elif args[i].startswith("--tree-sha="):
            candidate = args[i].split("=", 1)[1]
            i += 1
        else:
            rest.append(args[i])
            i += 1
    if candidate is None:
        candidate = os.environ.get("PTG_TREE_SHA")
    if candidate and _SHA_FORM.match(candidate.strip()):
        return candidate.strip(), rest
    return None, rest


def main(argv=None):
    tree_sha, rest = resolve_tree_sha(
        sys.argv[1:] if argv is None else argv)
    suite = load_suite()
    total = count(suite)
    if "--count-only" in rest:
        print(total)
        return 0
    runner = unittest.TextTestRunner(verbosity=1,
                                     resultclass=CertificateResult)
    result = runner.run(suite)
    run_id = emit_certificates(result.certificate_records, tree_sha)
    print(f"counted-once total: {total} tests "
          f"({result.testsRun} run, {len(result.skipped)} skipped, "
          f"{len(result.failures)} failures, {len(result.errors)} errors)")
    print(f"certificates: {len(result.certificate_records)} records "
          f"in certificates/ (run {run_id}; "
          f"{'certified at tree SHA ' + tree_sha if tree_sha else 'UNCERTIFIED — no tree SHA supplied, records certify nothing'})")
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
