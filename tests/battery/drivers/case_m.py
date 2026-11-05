# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Driver for battery case (m): pseudo-handle constants at the
four counted fetch sites — per-site A/B on Windows.

For one site at a time this driver substitutes the constant for
the fetch inside the site's enclosing function only
(GetCurrentProcess() -> the -1 pseudo handle,
GetCurrentThread() -> the -2 pseudo handle, expressed in the
tree's Handle type), builds the variant, and runs the tree's
own test suite against it. A site is evidenced only when its
variant builds, the full test run is green with that site
substituted, the audit's enumerated handle consumers for the
site were checked, and no consumer treats the constant as an
invalid handle (no consumer line tests the fetched value
against INVALID_HANDLE_VALUE — the numeric collision the audit
names).

Deadline and build-tree discipline: the four sites share ONE
scratch copy of the tree and ONE cargo target directory. Each
site's substitution is reverted before the next site's is
applied, so a variant can never contaminate another site; the
shared target only caches compilation of identical sources,
which cargo fingerprints, so a substituted site always rebuilds
what its substitution changed. Sharing the target removes the
cold full build per site that made the driver's worst case a
hope; the deadline arithmetic is nevertheless kept honest as
if every site paid in full: each site may consume a build and
a test run of BUILD_TIMEOUT_S each, and battery.py's kill
deadline for this driver is REQUIRED_WINDOWS_TIMEOUT_S, the
sum over all four sites (see battery.py, case (m)).
A build or test run that hits its deadline, or fails to spawn,
is not an A/B result: collect_windows records `timed_out` /
`spawn_failed` per site beside the booleans (run_command
records a timeout as exit_code None with the deadline stderr,
and a spawn failure likewise), stops attempting further sites
(recording them not_attempted), and decide returns "error"
(BLOCKED-ERROR) for such sites — "refuted" is reserved for a
site whose build/test actually ran and failed. The charged
site set (battery.PSEUDO_SITES) is likewise enforced before
anything is evidenced, and the evidenced detail is computed
from the records.
Stub tier: the same decision logic over stub_probe.py's records.
"""

import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import driver_common as dc  # noqa: E402
import battery as battery_mod  # noqa: E402

CONSTANTS = {"GetCurrentProcess()": "(usize::MAX as Handle)",
             "GetCurrentThread()": "((usize::MAX - 1) as Handle)"}

# The driver's deadline budget: per site, one build and one test
# run, each allowed BUILD_TIMEOUT_S; over the four counted sites
# that is the sum battery.py must allow this driver.
PER_SITE_TIMEOUT_S = 2 * dc.BUILD_TIMEOUT_S
REQUIRED_WINDOWS_TIMEOUT_S = len(battery_mod.PSEUDO_SITES) \
    * PER_SITE_TIMEOUT_S


def consumers_checked_for(entry):
    """Whether the audit actually enumerated handle consumers
    for one site. A site whose consumer list is empty — or whose
    audit entry is missing — was not checked, whatever else is
    known about the site; the site's existence (a located needle
    line) is not a consumer enumeration."""
    return bool(entry) and len(entry.get("consumers") or []) > 0


COND_M = "per-site A/B behaviour on Windows"


def _record_timed_out(record):
    """Whether one run_command evidence record is a deadline
    expiry: its exit code is None with the deadline stderr, per
    driver_common.run_command's timeout record."""
    if not isinstance(record, dict):
        return False
    stderr = str(record.get("stderr") or "")
    return "deadline" in stderr and "expired" in stderr


def _record_spawn_failed(record):
    """Whether one run_command evidence record is a spawn
    failure: exit code None with the spawn-failed stderr."""
    if not isinstance(record, dict):
        return False
    return str(record.get("stderr") or "").startswith("spawn failed")


def site_execution_error(site):
    """Whether a site's build or test run never actually
    evaluated the variant: it timed out, it failed to spawn, or
    it was never attempted because an earlier command's
    execution failure stopped the run. Such a site is an
    execution failure (BLOCKED-ERROR), never a refutation —
    a deadline expiry records exit_code None, which the old
    boolean collapse read as a completed failing run."""
    if site.get("timed_out") or site.get("spawn_failed") \
            or site.get("not_attempted"):
        return True
    if site.get("build_timed_out") or site.get("tests_timed_out") \
            or site.get("test_timed_out") or site.get("timeout"):
        return True
    for key in ("build", "tests", "build_record", "test_record",
                "tests_record"):
        record = site.get(key)
        if isinstance(record, dict) \
                and record.get("exit_code") is None \
                and (_record_timed_out(record)
                     or _record_spawn_failed(record)):
            return True
    for key in ("build_error", "tests_error", "test_error"):
        text = str(site.get(key) or "")
        if "deadline" in text or "spawn failed" in text:
            return True
    return False


def decide(obs):
    sites = obs.get("sites") or []
    if not sites:
        return {COND_M: ("error",
                         "no site observations — BLOCKED-ERROR, "
                         "never evidenced")}
    # Charged coverage: the site names in the records are
    # checked against battery.PSEUDO_SITES. A truncated set is
    # BLOCKED-ERROR, never evidenced over the sites it ran, and
    # the detail states what the records actually show rather
    # than certifying sites no record covers.
    charged_names = [site[0] for site in battery_mod.PSEUDO_SITES]
    observed_names = [s.get("name") for s in sites]
    missing_names = [n for n in charged_names
                     if n not in observed_names]
    if missing_names or len(observed_names) != len(charged_names):
        return {COND_M: ("error",
                         "the charged site set is not present in "
                         f"the records: sites observed "
                         f"{observed_names}; charged sites "
                         f"{charged_names}; missing "
                         f"{missing_names or 'none'} — a truncated "
                         "observation set is BLOCKED-ERROR, never "
                         "evidenced")}
    error_sites = [s for s in sites if site_execution_error(s)]
    if error_sites:
        return {COND_M: ("error",
                         "sites whose build or test run timed "
                         "out, failed to spawn, or was never "
                         "attempted because an earlier command's "
                         "execution failure stopped the run: "
                         + ", ".join(str(s.get("name"))
                                     for s in error_sites)
                         + " — the variant was never evaluated "
                           "at those sites, so the outcome is an "
                           "execution failure (BLOCKED-ERROR), "
                           "never a refutation")}
    bad = [s for s in sites
           if not (s["build_ok"] and s["tests_ok"]
                   and s["consumers_checked"]
                   and not s["invalid_handle_misuse"])]
    if not bad:
        per_site = "; ".join(
            f"{s['name']}: build_ok={s['build_ok']}, "
            f"tests_ok={s['tests_ok']}, consumers_checked="
            f"{s['consumers_checked']}, invalid_handle_misuse="
            f"{s['invalid_handle_misuse']}" for s in sites)
        return {COND_M: (
            "evidenced",
            f"{len(sites)} sites recorded, each substituted one "
            f"at a time ({', '.join(observed_names)}): "
            + per_site)}
    return {COND_M: (
        "refuted",
        "sites failing their A/B: " + "; ".join(
            f"{s['name']} (build {s['build_ok']}, tests "
            f"{s['tests_ok']}, consumers checked "
            f"{s['consumers_checked']}, invalid-handle misuse "
            f"{s['invalid_handle_misuse']})" for s in bad))}


def substitute_site(tree, site):
    """Replace the needle inside the site's enclosing function, in
    the copied tree at `tree`. Returns True when exactly the scoped
    substitution was made."""
    _name, rel, func, needle = site
    path = os.path.join(tree, rel)
    with open(path, encoding="utf-8") as handle:
        src = handle.read()
    span = battery_mod.extract_function(src, func)
    if span is None or needle not in span["body"]:
        return False
    body = span["body"].replace(needle, CONSTANTS[needle], 1)
    src = src[:span["body_start"]] + body + src[span["body_end"]:]
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(src)
    return True


def collect_windows(args):
    records = []
    sites = []
    site_report = {s["name"]: s for s in
                   battery_mod.pseudo_site_report(args.tree)}
    scratch = tempfile.mkdtemp(prefix="battery-case-m-")
    try:
        # One scratch copy and one shared cargo target for all
        # four sites; each site's substitution is reverted before
        # the next site runs, so the shared tree and target can
        # never carry one variant into another.
        variant = os.path.join(scratch, "tree")
        shutil.copytree(args.tree, variant,
                        ignore=shutil.ignore_patterns(
                            "target", ".git"))
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = os.path.join(scratch, "target")
        originals = {}
        # Once a command has timed out or failed to spawn the
        # run is an execution failure: later sites are never
        # attempted (attempting them would only burn the
        # remaining deadline budget), and they are recorded as
        # not attempted so the decider reports them "error",
        # never "refuted".
        execution_blocked = False
        for site in battery_mod.PSEUDO_SITES:
            name, rel = site[0], site[1]
            path = os.path.join(variant, rel)
            if rel not in originals:
                with open(path, encoding="utf-8") as handle:
                    originals[rel] = handle.read()
            entry = site_report.get(name, {})
            misuse = any("INVALID_HANDLE_VALUE" in text
                         for text in entry.get("consumer_lines", []))
            if execution_blocked:
                sites.append({
                    "name": name, "build_ok": False,
                    "tests_ok": False,
                    "consumers_checked":
                        consumers_checked_for(entry),
                    "invalid_handle_misuse": misuse,
                    "timed_out": False, "spawn_failed": False,
                    "not_attempted": True})
                continue
            try:
                if not substitute_site(variant, site):
                    sites.append({
                        "name": name, "build_ok": False,
                        "tests_ok": False,
                        "consumers_checked":
                            consumers_checked_for(entry),
                        "invalid_handle_misuse": misuse,
                        "timed_out": False, "spawn_failed": False,
                        "not_attempted": False})
                    continue
                build = dc.run_command(
                    ["cargo", "build", "--release"],
                    timeout_s=dc.BUILD_TIMEOUT_S, cwd=variant,
                    env=env)
                records.append(build)
                timed_out = _record_timed_out(build)
                spawn_failed = _record_spawn_failed(build)
                tests_ok = False
                if build["exit_code"] == 0:
                    tests = dc.run_command(
                        ["cargo", "test", "--release"],
                        timeout_s=dc.BUILD_TIMEOUT_S, cwd=variant,
                        env=env)
                    records.append(tests)
                    tests_ok = tests["exit_code"] == 0
                    timed_out = timed_out or _record_timed_out(tests)
                    spawn_failed = (spawn_failed
                                    or _record_spawn_failed(tests))
                if timed_out or spawn_failed:
                    execution_blocked = True
                sites.append({
                    "name": name,
                    "build_ok": build["exit_code"] == 0,
                    "tests_ok": tests_ok,
                    "consumers_checked":
                        consumers_checked_for(entry),
                    "invalid_handle_misuse": misuse,
                    "timed_out": timed_out,
                    "spawn_failed": spawn_failed,
                    "not_attempted": False})
            finally:
                with open(path, "w", encoding="utf-8") as handle:
                    handle.write(originals[rel])
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    return {"sites": sites, "shared_target_dir": True,
            "per_site_timeout_s": PER_SITE_TIMEOUT_S,
            "required_driver_timeout_s":
                REQUIRED_WINDOWS_TIMEOUT_S}, records


if __name__ == "__main__":
    sys.exit(dc.main("m", decide, collect_windows, dc.stub_collect("m")))
