# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Campaign driver: execute a plan, persist every observation, report.

Usage:
  python harness.py run --config campaign.json --out results/
  python harness.py run --config campaign.json --out results/ --reduced N
  python harness.py report --out results/

One attempt, no retries, exclusions or replacements: a slot runs
once and its observation stands, whatever it is. --reduced N is a
logic exercise (the first N slots of the seeded plan); it is
labelled as such in its output and can never be a result. The full
run with no --reduced is the only certifying shape, and only on
Windows.

Runs are namespaced: every run writes into its own
<out>/<run-id>/ directory (run id recorded in run.json), so a
re-run — over a complete, partial or stopped prior campaign alike —
never overwrites or deletes a prior run's evidence. The given out
directory only gains a new subdirectory. Raw observations are
appended to the run's observations.jsonl as slots finish, so a
stopped campaign leaves its partial evidence intact inside its own
namespace and is reported as incomplete with standing losses, never
as a result.

The manifest comes first: cmd_run writes a provisional run.json
(seed, full planned order, environment, configuration hash,
slots_run, provisional: true, campaign_status: incomplete) into
the namespace BEFORE the first slot executes, and rewrites it as
the run proceeds (slots_run after every slot), finalising it when
the run completes. A stopped campaign therefore resolves as a run
package from its namespace alone: cmd_report renders its partial
records as INCOMPLETE with exact slot accounting against the
recorded plan (the records present are a prefix of the seeded
order; every missing cell is a standing loss), and
check_run_package verifies the package as consistent-as-a-prefix.
A namespace holding observations but no manifest at all is still
reportable as INCOMPLETE (its completeness cannot be certified);
a namespace with neither a manifest nor observations is an error,
not a report. A stopped or provisional package never renders as
complete under any code path.

Pre-flight admission (certifying shape): before the namespace is
created and before any slot executes, a non-reduced run verifies
that every product arm's binary is present, that every configured
child program and the survivor probe's program are present,
fingerprints the
binaries present, probes competitor versions, and checks
competitor identities against the pin in the configuration —
the anchor — and, as an added consistency check, the staging
record. A binary matching the staging record but not the pin
fails identity. Any
failure aborts the campaign with a named admission failure and
the distinct exit code EXIT_ADMISSION_FAILURE, having written no
observations. Reduced runs keep their permissive behaviour (they
are logic exercises against stubs). The identity record computed
after the slots remains in the final run.json exactly as before.

Campaign outcome: the campaign's completeness verdict is carried
by its status, not by this driver's exit code. The reduced shape
is fenced in that verdict itself, not only in report.txt's banner:
campaign_outcome passes the run's reduced flag to
report.campaign_losses, which adds a standing loss naming the
reduced shape (report.REDUCED_SHAPE_LOSS), so any run made with
--reduced — at any N, the full plan size included — records
campaign_status incomplete for that named reason, however perfect
its records are; cmd_report's re-render applies the same fence
from the manifest's recorded reduced flag. run.json records
campaign_status ("complete" or "incomplete"), the standing-loss
count and the freeze-gate FAIL count, and report.txt carries the
same verdict in its STATUS line; cmd_run and cmd_report exit 0
for a completed execution/rendering whose outcome is incomplete,
and the run script (run.ps1) reads campaign_status from the run's
run.json and names an incomplete campaign as a failed stage in
its aggregate. Because run.ps1 acts on that recorded status,
check_run_package verifies a finalised package's recorded
outcome against the outcome its own records recompute to: a
manifest whose campaign_status or counts disagree with its
records is a named package defect, never a verdict. Exit codes here mean execution only: 0 = the
harness ran, EXIT_ADMISSION_FAILURE = admission refused the run,
other non-zero = the harness itself failed.

Order and environment: the warmup and measured blocks run in the
order plan.build_plan(seed) produces for the campaign's recorded
seed (the configuration's "seed", or --seed, or the plan's default).
run.json records the seed, the full execution order, the machine /
OS / build / CPU environment, and the SHA-256 of the configuration
file the run was made from. check_run_package() verifies a finished
or stopped package: the observations' slot sequence must equal the
order the recorded seed produces, and run.json's recorded order
must match it too — a tampered seed or order is detectable, never
silent.

Pricing: after the slots, the harness runs the CO-7 pricing probes
(pricing.py) against the guard arm and writes their raw
observations to pricing-observations.jsonl in the run directory.
Prices reach the report only as a reference to those records. A
"pricing" value typed into the configuration is never used as a
price: it is reported as operator-supplied, not measured. When a
report is re-rendered from a preserved namespace, the pricing
records' provenance is verified against run.json first (guard
fingerprint, recorded observation count, paired reps per item for
the run shape): a mismatch renders the affected items "not
measured" and check_run_package flags the namespace. The records'
probe-shape stamps are verified the same way (pricing.py's
pricing_stamp_defects: the construction each price claims — the
console context pair, the owner inserted-level counts, the
resolver probe counts and pinned resolver field): a record whose
stamps do not carry that construction prices nothing for its
item, the item renders "not measured", and check_run_package
names the stamp defect by item and field.

Profile decisions and the claim ledger: the evaluation
profile's scope decisions are package data — the configuration's
"profile_dimension_decisions" mapping (report.decisions_from_config)
— recorded verbatim in run.json beside the configuration's
SHA-256, so the decisions a run applied are traceable from its
own package; no source edit decides a dimension. After the slots,
cmd_run builds the claim ledger (report.claim_ledger: one claim
per SPEC_Benchmark section 3 dimension, its state and its basis
inside the package) and writes it as claim-ledger.json in the run
namespace, mirrored in run.json. campaign_outcome consumes the
ledger: complete requires every dimension observed-passing or
decided NOT-APPLICABLE, on top of the standing-loss standard.
check_run_package verifies a package's ledger against its records
and its recorded decisions and configuration hash: a ledger
claiming a decision the recorded configuration does not carry, or
a state the records contradict, is a named package defect.
Packages written before the ledger existed (no claim-ledger.json
and no ledger recorded in run.json) carry no ledger to verify and
are not faulted for its absence; their reports still render the
ledger from their records and recorded decisions.

Competitor identity: the pinned competitor identities in the
configuration (expected version per competitor, plus the SHA-256
pin of the as-found bundle binary) are checked
against the binaries actually present, by probed version and by
hash; the staging record is checked as well, as an added
consistency record, but the pin is the anchor — a binary
matching the staging record but not the pin fails identity.
Every arm's fingerprint and probed version are recorded.
In a certifying (non-reduced) run an identity mismatch is a
standing loss: the campaign measured the wrong binaries and is not
a result.
"""

import argparse
import hashlib
import json
import math
import os
import platform
import re
import sys
from datetime import datetime, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from arms import fingerprint, probe_version, check_competitor_identities, expected_exit, materialized_list  # noqa: E402
from plan import DEFAULT_SEED, build_plan, check_plan, slot_key  # noqa: E402
import pricing as pricing_mod           # noqa: E402
import report as report_mod             # noqa: E402
from runner import run_slot             # noqa: E402

PRODUCT_ARMS = ("guard", "processkit", "nuewframe", "sysprims")
COMPETITOR_ARMS = ("processkit", "nuewframe", "sysprims")

# Distinct non-zero exit for a pre-flight admission refusal: the
# campaign did not run at all (an infrastructure outcome, named),
# as opposed to a completed execution whose campaign outcome is
# incomplete — which exits 0 and carries its verdict in
# campaign_status / the report's STATUS line (see module docstring).
EXIT_ADMISSION_FAILURE = 4
EXIT_SLOT_CLEANUP_FAILURE = 5
WINDOWS_SLOT_ISOLATION = "windows-job-drain-after-observation"

CAMPAIGN_STATUS_COMPLETE = "complete"
CAMPAIGN_STATUS_INCOMPLETE = "incomplete"


def load_config(path):
    with open(path, encoding="utf-8") as handle:
        config = json.load(handle)
    # The configuration is an interchange input like the package
    # files: its root and the arms / children / pricing_probes
    # containers must be objects before anything consumes them,
    # or a container of another shape (a list root, a list of
    # arms) raises an attribute error out of the path anchoring
    # below instead of being refused by name — pricing_probes
    # included: the pricing stage reads it as a mapping, so a
    # string, list or number there would otherwise crash that
    # stage of a certifying run, after the slots. The entries
    # of arms / children / competitor_identities, and each arm
    # entry's templates container, are held to the same object
    # shape for the same reason (see the entry-level hold
    # below). A malformed
    # shape raises a named ValueError here; cmd_run surfaces it
    # as a pre-flight admission refusal.
    if not isinstance(config, dict):
        raise ValueError(
            f"campaign configuration in {path} is {config!r}, "
            "not a JSON object; a configuration must carry its "
            "arms and children as objects")
    for field in ("arms", "children", "pricing_probes"):
        container = config.get(field)
        if container is not None and not isinstance(container, dict):
            raise ValueError(
                f"campaign configuration in {path} carries "
                f"'{field}' as {container!r}, not an object; "
                f"a configuration's {field} must be an object "
                "mapping names to their entries")
    # The entry level is held the same way: consumers assume
    # every arms / children / competitor_identities entry is a
    # mapping (an arm's templates container included), so an
    # entry of another shape — a string, a list, a number — is
    # refused by name here, at load, in the same named-ValueError
    # idiom. Without this hold, a competitor_identities entry
    # raised out of arm_versions before admission_failures could
    # reach the naming check_competitor_identities already
    # carries for it, and on a reduced run (which skips
    # admission by design) an arms / children / templates entry
    # raised as a raw traceback out of cmd_run's manifest
    # construction or resolve_template instead of meeting this
    # named refusal. Genuine entries — objects all the way down
    # to the templates container — load clean.
    entry_shapes = (
        ("arms", "an arm entry must be an object carrying the "
                 "arm's executable and templates"),
        ("children", "a child entry must be an object carrying "
                     "the child program"),
        ("competitor_identities", "a competitor identity entry "
                                  "must be an object carrying "
                                  "the pinned identity"),
    )
    for field, shape in entry_shapes:
        container = config.get(field)
        if container is not None and not isinstance(container, dict):
            raise ValueError(
                f"campaign configuration in {path} carries "
                f"'{field}' as {container!r}, not an object; "
                f"a configuration's {field} must be an object "
                "mapping names to their entries")
        if isinstance(container, dict):
            for name, entry in container.items():
                if not isinstance(entry, dict):
                    raise ValueError(
                        f"campaign configuration in {path} "
                        f"carries '{field}' entry {name!r} as "
                        f"{entry!r}, not an object; {shape}")
    arms_container = config.get("arms")
    if isinstance(arms_container, dict):
        for name, entry in arms_container.items():
            if isinstance(entry, dict):
                templates = entry.get("templates")
                if templates is not None \
                        and not isinstance(templates, dict):
                    raise ValueError(
                        f"campaign configuration in {path} "
                        f"carries 'arms' entry {name!r} with "
                        f"'templates' as {templates!r}, not an "
                        f"object; an arm's templates must be an "
                        f"object mapping paths to their "
                        f"templates")
    # The field level is held the same way (round-21): the
    # scalar and per-entry fields consumers assume — the
    # survivor checker's token list, the recorded seed's integer
    # type (the type the manifest layer holds a recorded seed
    # to), each templates value's token-list shape, and each
    # child entry's expected-output strings — are refused by
    # name here, in the same idiom, rather than crashing a
    # consumer (admission_failures iterating an integer checker,
    # random.Random rejecting a list seed at the CLI), executing
    # as something else (a string template iterated per
    # character), or silently unbinding a check (a non-string
    # expected_stdout read as "spec names no expected bytes").
    checker_field = config.get("survivor_checker")
    if checker_field is not None \
            and not isinstance(checker_field, list):
        raise ValueError(
            f"campaign configuration in {path} carries "
            f"'survivor_checker' as {checker_field!r}, not a "
            "list; the survivor checker must be a list of argv "
            "tokens the runner invokes per kill slot")
    if isinstance(checker_field, list):
        for token in checker_field:
            if not isinstance(token, str):
                raise ValueError(
                    f"campaign configuration in {path} carries "
                    f"'survivor_checker' element {token!r}, not "
                    "a string; every survivor checker token must "
                    "be a string argv token")
    if "seed" in config:
        seed_value = config.get("seed")
        if not isinstance(seed_value, int) \
                or isinstance(seed_value, bool):
            raise ValueError(
                f"campaign configuration in {path} carries "
                f"'seed' as {seed_value!r}, not an integer; a "
                "configuration's seed must be an integer — the "
                "seed the run records in run.json, which the "
                "package checker holds to the integer type")
    if isinstance(arms_container, dict):
        for name, entry in arms_container.items():
            if isinstance(entry, dict) \
                    and isinstance(entry.get("templates"), dict):
                for path_name, template in \
                        entry["templates"].items():
                    if not isinstance(template, list):
                        raise ValueError(
                            f"campaign configuration in {path} "
                            f"carries 'arms' entry {name!r} with "
                            f"'templates' value for {path_name!r} "
                            f"as {template!r}, not a list; a "
                            "template must be a list of argv "
                            "tokens for its path")
                    for token in template:
                        if not isinstance(token, str):
                            raise ValueError(
                                f"campaign configuration in {path} "
                                f"carries 'arms' entry {name!r} "
                                f"with 'templates' value for "
                                f"{path_name!r} carrying element "
                                f"{token!r}, not a string; every "
                                "template token must be a string "
                                "argv token")
    children_container = config.get("children")
    if isinstance(children_container, dict):
        for name, entry in children_container.items():
            if isinstance(entry, dict):
                if "stdin" in entry \
                        and entry["stdin"] is not None \
                        and not isinstance(entry["stdin"], str):
                    raise ValueError(
                        f"campaign configuration in {path} "
                        f"carries 'children' entry {name!r} "
                        f"with 'stdin' as {entry['stdin']!r}, "
                        "not a string; a child entry's stdin "
                        "must be a string naming the workload's "
                        "input text when it is present (or null, "
                        "the shape that names no input) — a "
                        "value of another type is silently "
                        "dropped by the runner's string guard "
                        "and the workload runs on the null "
                        "device instead")
                for field in ("expected_stdout", "expected_stderr"):
                    if field in entry \
                            and not isinstance(entry[field], str):
                        raise ValueError(
                            f"campaign configuration in {path} "
                            f"carries 'children' entry {name!r} "
                            f"with '{field}' as {entry[field]!r}, "
                            "not a string; an expected-output "
                            "field must be a string naming the "
                            "expected bytes when it is present")
                if "args" in entry:
                    args_field = entry["args"]
                    if not isinstance(args_field, list):
                        raise ValueError(
                            f"campaign configuration in {path} "
                            f"carries 'children' entry {name!r} "
                            f"with 'args' as {args_field!r}, not "
                            "a list; a child's args must be a "
                            "list of argv tokens — a string would "
                            "execute as per-character tokens")
                    for token in args_field:
                        if not isinstance(token, str):
                            raise ValueError(
                                f"campaign configuration in "
                                f"{path} carries 'children' entry "
                                f"{name!r} with 'args' element "
                                f"{token!r}, not a string; every "
                                "child arg must be a string argv "
                                "token")
    # The element level in the identity and pricing entries,
    # and the expected_exit values (round-22): version_args and
    # guard_launcher are token lists whose elements are strings;
    # expected_exit values are the non-bool integers the runner
    # compares an observed exit code against.
    identities_container = config.get("competitor_identities")
    if isinstance(identities_container, dict):
        for name, entry in identities_container.items():
            if isinstance(entry, dict) and "version_args" in entry:
                version_args = entry["version_args"]
                if not isinstance(version_args, list) \
                        or any(not isinstance(token, str)
                               for token in version_args):
                    raise ValueError(
                        f"campaign configuration in {path} "
                        f"carries 'competitor_identities' entry "
                        f"{name!r} with 'version_args' as "
                        f"{version_args!r}, not a list of "
                        "strings; an identity's version_args must "
                        "be a list of string argv tokens")
    probes_container = config.get("pricing_probes")
    if isinstance(probes_container, dict) \
            and "guard_launcher" in probes_container:
        guard_launcher = probes_container["guard_launcher"]
        if not isinstance(guard_launcher, list) \
                or any(not isinstance(token, str)
                       for token in guard_launcher):
            raise ValueError(
                f"campaign configuration in {path} carries "
                f"'pricing_probes' with 'guard_launcher' as "
                f"{guard_launcher!r}, not a list of strings; "
                "the pricing probes' guard_launcher must be a "
                "list of string argv tokens")
    if isinstance(arms_container, dict):
        for name, entry in arms_container.items():
            if isinstance(entry, dict) and "expected_exit" in entry:
                table = entry["expected_exit"]
                if not isinstance(table, dict):
                    raise ValueError(
                        f"campaign configuration in {path} "
                        f"carries 'arms' entry {name!r} with "
                        f"'expected_exit' as {table!r}, not an "
                        "object; an arm's expected_exit must be "
                        "an object mapping paths to integer exit "
                        "codes")
                for path_name, value in table.items():
                    if not isinstance(value, int) \
                            or isinstance(value, bool):
                        raise ValueError(
                            f"campaign configuration in {path} "
                            f"carries 'arms' entry {name!r} with "
                            f"'expected_exit' value for "
                            f"{path_name!r} as {value!r}, not an "
                            "integer; an expected exit code must "
                            "be a non-bool integer — the type the "
                            "runner records and the report's "
                            "default table carries")
    # The program-path fields themselves (round-23): an arm's
    # executable and a child entry's program are paths every
    # consumer passes to subprocess / os.path.isfile, which
    # assume a string — a value of another type either crashes
    # the reduced CLI with a raw TypeError out of subprocess
    # (an integer) or crashes admission's own presence check
    # (a list). Both are refused by name here, in the same
    # idiom, before the anchoring below passes them through.
    if isinstance(arms_container, dict):
        for name, entry in arms_container.items():
            if isinstance(entry, dict) and "executable" in entry \
                    and not isinstance(entry["executable"], str):
                raise ValueError(
                    f"campaign configuration in {path} carries "
                    f"'arms' entry {name!r} with 'executable' as "
                    f"{entry['executable']!r}, not a string; an "
                    "arm's executable must be a string naming "
                    "the arm's binary path")
    if isinstance(children_container, dict):
        for name, entry in children_container.items():
            if isinstance(entry, dict) and "program" in entry \
                    and not isinstance(entry["program"], str):
                raise ValueError(
                    f"campaign configuration in {path} carries "
                    f"'children' entry {name!r} with 'program' "
                    f"as {entry['program']!r}, not a string; a "
                    "child entry's program must be a string "
                    "naming the child program's path")
    # The staging-record path beside them (round-24):
    # competitor_identities_record is a path every consumer
    # hands to open() — a value of another shape either crashes
    # admission itself (a list or dict raises TypeError out of
    # staged_identity_record's open, which catches OSError /
    # ValueError only) or silently deletes the staging-record
    # consistency check (an integer is read as "no staging
    # record"). It is refused by name here, in the same idiom.
    if "competitor_identities_record" in config:
        record_value = config.get("competitor_identities_record")
        if record_value is not None \
                and not isinstance(record_value, str):
            raise ValueError(
                f"campaign configuration in {path} carries "
                f"'competitor_identities_record' as "
                f"{record_value!r}, not a string; the staging "
                "record of competitor identities must be named "
                "by a string path — a value of another shape "
                "either crashes the identity check or silently "
                "drops it")
    # Relative executable and child-program paths in the config are
    # relative to the config file's own folder (the suite root, beside
    # build\), never to the caller's working directory: the one run
    # command must work from wherever it is invoked.
    base = os.path.dirname(os.path.abspath(path))

    def anchored(value):
        if isinstance(value, str) and value and not os.path.isabs(value):
            return os.path.normpath(os.path.join(base, value))
        return value

    for entry in config.get("arms", {}).values():
        if isinstance(entry, dict) and "executable" in entry:
            entry["executable"] = anchored(entry["executable"])
    for spec in config.get("children", {}).values():
        if isinstance(spec, dict) and "program" in spec:
            spec["program"] = anchored(spec["program"])
    checker = config.get("survivor_checker")
    if isinstance(checker, list):
        config["survivor_checker"] = [
            anchored(token) if isinstance(token, str)
            and ("/" in token or "\\" in token) else token
            for token in checker
        ]
    probes = config.get("pricing_probes")
    if isinstance(probes, dict) and isinstance(probes.get("launcher"), list):
        probes["launcher"] = [
            anchored(token) if isinstance(token, str)
            and ("/" in token or "\\" in token) else token
            for token in probes["launcher"]
        ]
    record = config.get("competitor_identities_record")
    if isinstance(record, str):
        config["competitor_identities_record"] = anchored(record)
    return config


def _arm_executable(config, arm):
    """The configured executable of one arm, or None when the
    arm's entry is absent or is not an object carrying one: a
    non-object entry names no executable (admission_failures
    names the entry itself), and reading one must not raise on
    the entry's shape."""
    arms = config.get("arms") if isinstance(config, dict) else None
    entry = arms.get(arm) if isinstance(arms, dict) else None
    return entry.get("executable") if isinstance(entry, dict) else None


def _is_path_value(value):
    """True when a value can be handed to os.path.isfile as a
    path: a string, bytes or a path-like object. A configured
    executable or program of any other type (an integer, a
    list) is a shape admission names — it must never reach
    isfile, whose stat call raises on a list."""
    return isinstance(value, (str, bytes, os.PathLike))


def fingerprints(config):
    out = {}
    for arm in PRODUCT_ARMS:
        exe = _arm_executable(config, arm)
        if exe and _is_path_value(exe) and os.path.isfile(exe):
            out[arm] = fingerprint(exe)
    return out


def arm_versions(config):
    """Probed version of every arm binary present, by the version
    arguments its identity entry names (default --version)."""
    out = {}
    pinned = config.get("competitor_identities", {}) or {}
    if not isinstance(pinned, dict):
        pinned = {}
    for arm in PRODUCT_ARMS:
        exe = _arm_executable(config, arm)
        if exe and _is_path_value(exe) and os.path.isfile(exe):
            # A pinned entry that is not an object names no
            # version arguments: it is read as unprobed here and
            # named by check_competitor_identities (admission's
            # own surface for it), never raised on — admission
            # calls this function before that naming is reached.
            pinned_entry = pinned.get(arm)
            if not isinstance(pinned_entry, dict):
                pinned_entry = {}
            # Round-45 F1: the stated-vs-default choice is
            # a PRESENCE choice, made under the
            # materialization discipline — the entry states
            # version_args, or it does not and the default
            # stands. No truthiness or length judgement is
            # taken over the stated token-list value: a
            # payload-carrying list subclass whose list
            # storage is empty states its args in full, and
            # the raw `or` this replaces discarded them for
            # the default (a confirmable identity read
            # unconfirmed; a stated-args refutation read
            # confirmed). A stated value of another shape
            # (None included) is not the default either:
            # it is read as unprobed below, as such shapes
            # always were.
            if "version_args" in pinned_entry:
                stated_args = pinned_entry["version_args"]
            else:
                stated_args = ["--version"]
            # Round-44 F2: the shape check reads the tokens
            # through the shared materialization — this
            # function is called by admission_failures, and
            # a raw any() here drained a payload-carrying
            # version_args before probe_version (which now
            # reads through the same materialization) saw it.
            if not isinstance(stated_args, list):
                # A version_args field of another shape is named
                # by admission_failures itself; here it is read
                # as unprobed, never raised on and never
                # tokenised per character.
                out[arm] = None
                continue
            tokens = materialized_list(stated_args)
            if any(not isinstance(token, str) for token in tokens):
                out[arm] = None
                continue
            # Round-45 F1: probe_version receives the
            # materialized tokens themselves — the tokens
            # this choice and this shape check judged are
            # the tokens probed.
            out[arm] = probe_version(exe, tokens)
    return out


def staged_identity_record(config):
    """The staging record build.ps1 wrote (staged hashes and probed
    versions per competitor), or None when no staging has recorded
    one yet. This record is an added consistency record, checked
    as well as the pin — the pin in the configuration is the
    anchor, and a binary matching this record but not the pin
    fails identity (see arms.check_competitor_identities)."""
    path = config.get("competitor_identities_record")
    if not path:
        base = os.path.dirname(os.path.abspath(
            config.get("_config_path", "campaign.json")))
        path = os.path.join(base, "build", "competitor-identities.json")
    try:
        # The record is written by build.ps1's Set-Content, whose
        # UTF8 encoding carries a byte-order mark under Windows
        # PowerShell; utf-8-sig reads the record with or without
        # that mark, where plain utf-8 would fail the parse and
        # silently drop the consistency check this record feeds.
        with open(path, encoding="utf-8-sig") as handle:
            data = json.load(handle)
        if not isinstance(data, dict):
            return None
        competitors = data.get("competitors", data)
        return competitors if isinstance(competitors, dict) else None
    except (OSError, ValueError, TypeError):
        # TypeError: a configured path of a non-path shape (a
        # list, a dict) raises it out of open() — such a value
        # names no record here; load_config refuses it by name
        # and admission_failures names it as a failure, so the
        # staging-record check is never silently dropped and
        # never crashes its consumer.
        return None


def identity_mismatches(config, fps, versions):
    return check_competitor_identities(
        config, fps, versions, staged_identity_record(config))


def admission_failures(config):
    """Pre-flight admission for a certifying (non-reduced) run:
    every product arm's binary present, fingerprints taken,
    competitor versions probed and identities checked against the
    pin (the anchor; the staging record is checked as well, as an
    added consistency record). Returns named failures (empty = admitted).
    Each failure names its arm, so a refusal says what was wrong
    before any slot burned time measuring it. An arm or child
    entry that is not an object is named as that entry's
    admission failure, in the same idiom — the pre-flight exists
    to name failures by arm, not to raise on the first malformed
    entry it meets. A pricing_probes container that is not an
    object is named the same way: the pricing stage consumes it
    as a mapping after the slots, and no run shape may reach
    that consumer on an unadmitted shape. The field level is
    named the same way when this layer meets it first (a
    configuration consumed without load_config): a
    survivor_checker that is not a token list (an integer it
    cannot iterate, an object whose keys it would iterate), a
    seed that is not an integer, a templates value that is not
    a token list, and a child entry's non-string expected-output
    field are each named here, and none of them raises.
    Round-44 F2: every token-list value this pre-flight
    reads — a child entry's args, an arm's template, an
    identity's version_args, the pricing probes'
    guard_launcher, the survivor_checker — is read through
    the shared materialization (arms.materialized_list),
    never by a raw iteration: this pre-flight is the FIRST
    reader in a run's own order, and a raw iteration of a
    payload-carrying list subclass (a list whose __iter__
    yields from a one-shot payload) drained the value out
    from under every later consumer — admission-first left
    the construction building [] and the derivation
    deriving the empty remainder, against full controls.
    The shape judgements are unchanged: a value that is
    not a list is named by type without being iterated,
    exactly as before."""
    failures = []
    if not isinstance(config, dict):
        return [
            "campaign configuration is not an object; pre-flight "
            "admission failure"]
    arms = config.get("arms")
    if not isinstance(arms, dict):
        arms = {}
    for arm in PRODUCT_ARMS:
        entry = arms.get(arm)
        if entry is not None and not isinstance(entry, dict):
            failures.append(
                f"{arm}: arm entry is {entry!r}, not an object "
                "carrying the arm's executable and templates; "
                "pre-flight admission failure")
            continue
        exe = entry.get("executable") if isinstance(entry, dict) else None
        if not exe:
            failures.append(
                f"{arm}: no executable configured; pre-flight "
                "admission failure")
        elif not _is_path_value(exe):
            failures.append(
                f"{arm}: executable is {exe!r}, not a string "
                "path naming the arm's binary; pre-flight "
                "admission failure")
        elif not os.path.isfile(exe):
            failures.append(
                f"{arm}: product binary not present at {exe}; "
                "pre-flight admission failure")
    # The workloads and the survivor probe are part of the
    # measuring system, not of any arm: a child program that is
    # not present makes every product arm fail its slots for a
    # harness reason (the guard, for one, answers a missing child
    # program with its own 125, which the runner would otherwise
    # record as the arm's exit-code failure and the freeze gate
    # would read as a product loss on exit-code correctness), and
    # a survivor probe that is not present makes every kill slot
    # an infrastructure failure after the fact. Refuse both here,
    # by name, before any slot executes.
    children = config.get("children")
    if not isinstance(children, dict):
        children = {}
    for kind, spec in children.items():
        if not isinstance(spec, dict):
            failures.append(
                f"child {kind}: child entry is {spec!r}, not an "
                "object carrying the child program; pre-flight "
                "admission failure")
            continue
        if "stdin" in spec and spec.get("stdin") is not None \
                and not isinstance(spec.get("stdin"), str):
            failures.append(
                f"child {kind}: stdin is {spec.get('stdin')!r}, "
                "not a string naming the workload's input text; "
                "pre-flight admission failure")
        for field in ("expected_stdout", "expected_stderr"):
            if field in spec and not isinstance(spec.get(field), str):
                failures.append(
                    f"child {kind}: {field} is {spec.get(field)!r}, "
                    "not a string naming the expected bytes; "
                    "pre-flight admission failure")
        if "args" in spec:
            args_field = spec.get("args")
            if not isinstance(args_field, list):
                failures.append(
                    f"child {kind}: args is {args_field!r}, not a "
                    "list of argv tokens; pre-flight admission "
                    "failure")
            else:
                for token in materialized_list(args_field):
                    if not isinstance(token, str):
                        failures.append(
                            f"child {kind}: args element "
                            f"{token!r} is not a string argv "
                            "token; pre-flight admission failure")
                        break
        program = spec.get("program")
        if not program:
            failures.append(
                f"child {kind}: no child program configured; "
                "pre-flight admission failure")
        elif not _is_path_value(program):
            failures.append(
                f"child {kind}: program is {program!r}, not a "
                "string path naming the child program; "
                "pre-flight admission failure")
        elif not os.path.isfile(program):
            failures.append(
                f"child {kind}: child program not present at "
                f"{program}; pre-flight admission failure")
    # The pricing stage is part of the certifying run too: it
    # consumes pricing_probes as a mapping after the slots, so a
    # pricing_probes of another shape (a string, a list, a
    # number) is named here, before any slot executes, rather
    # than crashing that stage after them. A configuration
    # carrying no pricing_probes names nothing to refuse.
    probes = config.get("pricing_probes")
    if probes is not None and not isinstance(probes, dict):
        failures.append(
            f"pricing_probes: pricing_probes is {probes!r}, not "
            "an object carrying the pricing probes' "
            "configuration; pre-flight admission failure")
    elif isinstance(probes, dict) and "guard_launcher" in probes:
        guard_launcher = probes.get("guard_launcher")
        if not isinstance(guard_launcher, list) \
                or any(not isinstance(token, str)
                       for token in materialized_list(
                           guard_launcher)):
            failures.append(
                f"pricing_probes: guard_launcher is "
                f"{guard_launcher!r}, not a list of string argv "
                "tokens; pre-flight admission failure")
    # The field level, named when this layer meets it: the seed
    # a run would record, and each arm's per-path template
    # values, in load_config's shapes.
    if "seed" in config and (not isinstance(config.get("seed"), int)
                             or isinstance(config.get("seed"), bool)):
        failures.append(
            f"seed: seed is {config.get('seed')!r}, not an "
            "integer; pre-flight admission failure")
    for arm, entry in arms.items():
        if not isinstance(entry, dict):
            continue
        templates = entry.get("templates")
        if templates is not None and not isinstance(templates, dict):
            failures.append(
                f"{arm}: templates is {templates!r}, not an "
                "object mapping paths to their templates; "
                "pre-flight admission failure")
        elif isinstance(templates, dict):
            for path_name, template in templates.items():
                if not isinstance(template, list):
                    failures.append(
                        f"{arm}: templates value for "
                        f"{path_name!r} is {template!r}, not a "
                        "list of argv tokens; pre-flight "
                        "admission failure")
                else:
                    for token in materialized_list(template):
                        if not isinstance(token, str):
                            failures.append(
                                f"{arm}: templates value for "
                                f"{path_name!r} carries element "
                                f"{token!r}, not a string argv "
                                "token; pre-flight admission "
                                "failure")
                            break
        if "expected_exit" in entry:
            table = entry.get("expected_exit")
            if not isinstance(table, dict):
                failures.append(
                    f"{arm}: expected_exit is {table!r}, not an "
                    "object mapping paths to integer exit codes; "
                    "pre-flight admission failure")
            else:
                for path_name, value in table.items():
                    if not isinstance(value, int) \
                            or isinstance(value, bool):
                        failures.append(
                            f"{arm}: expected_exit value for "
                            f"{path_name!r} is {value!r}, not a "
                            "non-bool integer; pre-flight "
                            "admission failure")
                        break
    identities = config.get("competitor_identities")
    if isinstance(identities, dict):
        for arm, entry in identities.items():
            if isinstance(entry, dict) and "version_args" in entry:
                version_args = entry.get("version_args")
                if not isinstance(version_args, list) \
                        or any(not isinstance(token, str)
                               for token in materialized_list(
                                   version_args)):
                    failures.append(
                        f"{arm}: version_args is "
                        f"{version_args!r}, not a list of string "
                        "argv tokens; pre-flight admission "
                        "failure")
    # The staging-record path's shape (round-24): a value that
    # is not a string path either crashed this pre-flight out of
    # staged_identity_record's open() (a list, a dict) or was
    # silently read as "no staging record", unbinding the
    # staging consistency check without a word (an integer).
    # It is named here, and the identity recomputation below
    # reads no record for it (staged_identity_record absorbs
    # the shape), so admission names it instead of raising or
    # absorbing it.
    if "competitor_identities_record" in config:
        record_value = config.get("competitor_identities_record")
        if record_value is not None \
                and not isinstance(record_value, str):
            failures.append(
                f"competitor_identities_record: "
                f"competitor_identities_record is "
                f"{record_value!r}, not a string path naming "
                "the staging record of competitor identities; "
                "pre-flight admission failure")
    checker = config.get("survivor_checker")
    # Round-44 F2: a list checker's tokens are read through
    # the shared materialization and its emptiness is
    # judged on those tokens — a payload-carrying list
    # subclass's empty list storage read as "no checker
    # configured" here, and a raw iteration would have
    # drained the payload the runner's probe later reads
    # through the same materialization.
    checker_tokens = materialized_list(checker) \
        if isinstance(checker, list) else None
    if checker_tokens is not None and not checker_tokens:
        failures.append(
            "survivor probe: no survivor_checker configured; "
            "pre-flight admission failure")
    elif checker_tokens is None and not checker:
        failures.append(
            "survivor probe: no survivor_checker configured; "
            "pre-flight admission failure")
    elif not isinstance(checker, list):
        failures.append(
            f"survivor probe: survivor_checker is {checker!r}, "
            "not a list of argv tokens; pre-flight admission "
            "failure")
    else:
        for token in checker_tokens:
            if not isinstance(token, str):
                failures.append(
                    f"survivor probe: survivor_checker element "
                    f"{token!r} is not a string argv token; "
                    "pre-flight admission failure")
                break
            if isinstance(token, str) and ("/" in token
                                           or "\\" in token) \
                    and not os.path.isfile(token):
                failures.append(
                    f"survivor probe: probe program not present "
                    f"at {token}; pre-flight admission failure")
    fps = fingerprints(config)
    versions = arm_versions(config)
    failures.extend(identity_mismatches(config, fps, versions))
    return failures


def campaign_outcome(records, plan_counts, identity_losses=None,
                     pricing=None, decisions=None, reduced=False,
                     expected_output=None, config=None):
    """The campaign's completeness verdict, on the same standard
    report.render applies (report.campaign_losses): complete only
    when no campaign standing loss stands. The claim ledger is
    consumed here, not merely printed: complete also requires
    every profile dimension's ledger state to be observed-passing,
    NOT-APPLICABLE (decided by a decision in the run's
    configuration) or collected (dimension (8), by the manual
    instrument its configuration entry names — a state the
    report computes from that instrument's artifact, round-27
    F2: collected only when the artifact records a prompt class
    exercised with an observed outcome) — an
    UNEVALUATED dimension makes the campaign
    INCOMPLETE, never certified, and an observed-failing one is a
    freeze-gate loss already. decisions is the decisions mapping
    the run's configuration carries (report.decisions_from_config).
    reduced is the run's --reduced shape: when it is in force,
    campaign_losses adds report.REDUCED_SHAPE_LOSS, so a reduced
    run of any size — the full plan size included — records
    incomplete with that named standing loss, however perfect its
    records are. expected_output, when given, is the expected-bytes
    mapping (report.expected_output_from_config) the outcome's
    record-consistency discipline holds a match verdict's evidence
    to: cmd_run derives it from the run's configuration, and the
    package recomputations derive it from the configuration the
    manifest names (see _package_expected_output). Returns the
    fields run.json records: campaign_status,
    standing_loss_count, freeze_gate_fail_count (freeze-gate FAIL
    entries among the gate's per-dimension outcomes) and the
    claim ledger itself."""
    ledger = report_mod.claim_ledger(records, decisions, expected_output,
                                     config)
    losses = report_mod.campaign_losses(
        records, plan_counts, identity_losses=identity_losses,
        pricing=pricing, decisions=decisions, reduced=reduced,
        expected_output=expected_output, config=config)
    campaign = [loss for loss in losses
                if not loss.startswith(report_mod.COMPETITOR_LOSS_PREFIX)]
    gate_fails = [entry for entry in report_mod.freeze_gate(
        records, expected_output, config)
                  if entry["outcome"] == "FAIL"]
    ledger_complete = all(
        entry["state"] in (report_mod.LEDGER_OBSERVED_PASSING,
                           report_mod.LEDGER_NOT_APPLICABLE,
                           report_mod.LEDGER_COLLECTED)
        for entry in ledger)
    return {
        "campaign_status": (CAMPAIGN_STATUS_COMPLETE
                            if not campaign and ledger_complete
                            else CAMPAIGN_STATUS_INCOMPLETE),
        "standing_loss_count": len(campaign),
        "freeze_gate_fail_count": len(gate_fails),
        "claim_ledger": ledger,
    }


def _write_run_json(run_dir, meta, indent=2):
    with open(os.path.join(run_dir, "run.json"), "w",
              encoding="utf-8") as sink:
        json.dump(meta, sink, indent=indent)


def _read_records(run_dir):
    """The run's observation records, or [] when the observations
    file is absent (a manifest written before the first slot, with
    no slot yet recorded, is a package with zero records)."""
    records = []
    path = os.path.join(run_dir, "observations.jsonl")
    if not os.path.isfile(path):
        return records
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                records.append(json.loads(line))
    return records


def _is_number(value):
    return isinstance(value, (int, float)) \
        and not isinstance(value, bool)


def _is_plausible_elapsed(value):
    """An elapsed time the runner's monotonic clock can produce:
    a finite, non-negative number. NaN and the infinities parse
    as JSON numbers under Python's parser (and the shape checks
    below read them as numbers), but no timed lifetime is ever
    NaN, infinite or negative, and a median or paired difference
    computed over such a value is not a measurement. Total
    (round-29 F6): an int too large to convert to float is
    named implausible here, never an OverflowError — the
    checker cannot crash on a record."""
    if not _is_number(value):
        return False
    try:
        return math.isfinite(value) and value >= 0
    except OverflowError:
        return False


# The outcome classifications runner.run_slot writes. An
# observation record carrying any other outcome string is not a
# classification the suite makes: it feeds no median (only "ok"
# records count) and no loss list (each names its own outcome),
# so without a shape check it would vanish silently.
OBSERVATION_OUTCOMES = ("ok", "infra-failure", "exit-mismatch",
                       "descendant-survived", "output-mismatch")


def _observation_shape_problems(records):
    """Shape defects of observation records, named per record and
    field. The recomputations check_run_package performs (the
    freeze gate's medians, the outcome's standing losses, the
    ledger) index the slot-identity fields directly and read
    elapsed_ms as a number: a record that parses as JSON but is
    not an object, lacks a slot-identity field, carries a
    non-integer rep, a non-string outcome, or an elapsed_ms that
    is neither null nor a number would crash those recomputations
    instead of being named — and a checker that crashes on a
    malformed package hides every defect after the crash, the
    very failure this checker exists to prevent. Such records
    are package defects here, in their own right."""
    problems = []
    for index, record in enumerate(records):
        where = f"observations.jsonl record {index + 1}"
        if not isinstance(record, dict):
            problems.append(
                f"{where} is not a JSON object; an observation "
                "record must carry the slot identity and outcome "
                "fields the runner writes")
            continue
        for field in ("kind", "arm", "path", "context", "outcome"):
            if not isinstance(record.get(field), str):
                problems.append(
                    f"{where} field '{field}' is "
                    f"{record.get(field)!r}; an observation record "
                    "must carry that field as a string")
        outcome = record.get("outcome")
        if isinstance(outcome, str) \
                and outcome not in OBSERVATION_OUTCOMES:
            problems.append(
                f"{where} field 'outcome' is {outcome!r}; an "
                "observation record's outcome must be one of the "
                "classifications the runner writes ("
                + ", ".join(OBSERVATION_OUTCOMES) + ")")
        rep = record.get("rep")
        if not isinstance(rep, int) or isinstance(rep, bool):
            problems.append(
                f"{where} field 'rep' is {rep!r}; an observation "
                "record must carry its slot repetition as an "
                "integer")
        if "elapsed_ms" not in record \
                or (record.get("elapsed_ms") is not None
                    and not _is_number(record.get("elapsed_ms"))):
            problems.append(
                f"{where} field 'elapsed_ms' is "
                f"{record.get('elapsed_ms')!r}; an observation "
                "record must carry its elapsed time as a number "
                "or null")
        elif record.get("elapsed_ms") is not None \
                and not _is_plausible_elapsed(record.get("elapsed_ms")):
            problems.append(
                f"{where} field 'elapsed_ms' is "
                f"{record.get('elapsed_ms')!r}; an observation "
                "record's elapsed time must be a finite, "
                "non-negative number — the runner's monotonic "
                "clock produces no other kind, and a median over "
                "a NaN, infinite or negative time is not a "
                "measurement")
    return problems


def _pricing_shape_problems(probe_records):
    """Shape defects of pricing records, named per record and
    field: a pricing record that is not an object, a record
    whose kind is not "pricing-probe" (the pricing file holds
    no other kind; run_pricing_probes stamps that kind on every
    record it writes), a pricing-probe record — of ANY outcome,
    not only "ok" — whose probe, shape, rep or elapsed_ms is
    missing or of a type the pairing arithmetic cannot use,
    or a guard_fingerprint that is present but not an object.
    The non-"ok" extension matters: run_pricing_probes stamps
    probe, shape and rep and records an elapsed time on every
    record, failed ones included, so a failed record carrying
    those fields in garbled shapes is not a record the run
    wrote either — and while only "ok" records pair, an
    unvalidated failed record was silently absorbed by the
    file, counted toward the recorded total and named nowhere.
    The provenance and pricing recomputations read those fields
    directly; without this check a malformed-but-parseable
    pricing file crashed the checker instead of being named
    (the parse-level form of that defect was round 14's P1).
    The record's item stamp, an "ok" record's exit_code and
    the loaded-only fields (guard_pid / outer_elapsed_ms,
    written only on the owner-verification loaded shape) are
    held here as well (round-23): a record whose own fields
    contradict its classification or carry another shape's
    fields is a shape no run writes, named per field."""
    problems = []
    for index, record in enumerate(probe_records):
        where = f"pricing-observations.jsonl record {index + 1}"
        if not isinstance(record, dict):
            problems.append(
                f"{where} is not a JSON object; a pricing record "
                "must carry the probe fields the pricing probes "
                "write")
            continue
        if "guard_fingerprint" in record \
                and not isinstance(record.get("guard_fingerprint"),
                                   dict):
            problems.append(
                f"{where} field 'guard_fingerprint' is "
                f"{record.get('guard_fingerprint')!r}; a pricing "
                "record's guard fingerprint must be an object "
                "carrying the guard's sha256")
        if record.get("kind") != "pricing-probe":
            problems.append(
                f"{where} field 'kind' is "
                f"{record.get('kind')!r}; the pricing file holds "
                "pricing-probe records only — every record "
                "run_pricing_probes writes carries that kind — "
                "so a record of any other kind is not a pricing "
                "record of this run")
            continue
        # Every pricing-probe record run_pricing_probes writes
        # carries the guard fingerprint with a string sha256 —
        # that stamp is what ties the record to the binary the
        # price claims. A fingerprint object WITHOUT the sha256
        # (or no fingerprint at all) was named nowhere: the
        # provenance recomputation builds its stamp set only
        # from records carrying a string sha256, so an erased
        # stamp silently dropped out of the comparison while
        # the record still paired and priced.
        fingerprint = record.get("guard_fingerprint")
        if not isinstance(fingerprint, dict) \
                or not isinstance(fingerprint.get("sha256"), str):
            problems.append(
                f"{where} field 'guard_fingerprint' is "
                f"{fingerprint!r}; a pricing-probe record must "
                "carry the guard fingerprint as an object with "
                "its sha256 as a string — the stamp that ties "
                "the record to the priced binary")
        elif "path" in fingerprint or "size" in fingerprint:
            # The fingerprint's companion values (round-25):
            # where the record carries them, the path must be
            # the recorded path form (the absolute path string
            # arms.fingerprint records) and the size a
            # non-negative integer — the forms the stamp
            # checker (pricing.pricing_stamp_defects) holds and
            # the provenance check below holds to equality with
            # the run's recorded guard fingerprint.
            path_value = fingerprint.get("path")
            if not (isinstance(path_value, str) and path_value
                    and os.path.isabs(path_value)):
                problems.append(
                    f"{where} field 'guard_fingerprint.path' is "
                    f"{path_value!r}; a pricing record's guard "
                    "fingerprint path must be the vendored "
                    "guard's recorded path form — the absolute "
                    "path string the fingerprint call records "
                    "for the priced binary")
            size_value = fingerprint.get("size")
            if not (isinstance(size_value, int)
                    and not isinstance(size_value, bool)
                    and size_value >= 0):
                problems.append(
                    f"{where} field 'guard_fingerprint.size' is "
                    f"{size_value!r}; a pricing record's guard "
                    "fingerprint size must be a non-negative "
                    "integer — the size the fingerprint call "
                    "counted for the priced binary")
        probe = record.get("probe")
        if not isinstance(probe, str) \
                or probe not in pricing_mod.PROBES:
            problems.append(
                f"{where} field 'probe' is {probe!r}; a "
                "pricing-probe record must carry the name of "
                "one of the pricing probes the suite runs ("
                + ", ".join(sorted(pricing_mod.PROBES)) + "), "
                "so its pair can be formed")
        shape = record.get("shape")
        if shape not in ("loaded", "baseline"):
            problems.append(
                f"{where} field 'shape' is {shape!r}; a "
                "pricing-probe record must carry its shape "
                "as 'loaded' or 'baseline', so its pair can "
                "be formed")
        rep = record.get("rep")
        if not isinstance(rep, int) or isinstance(rep, bool):
            problems.append(
                f"{where} field 'rep' is {rep!r}; a "
                "pricing-probe record must carry its "
                "repetition as an integer, so its pair can "
                "be formed")
        if not _is_plausible_elapsed(record.get("elapsed_ms")):
            problems.append(
                f"{where} field 'elapsed_ms' is "
                f"{record.get('elapsed_ms')!r}; a "
                "pricing-probe record must carry its elapsed "
                "time as a finite, non-negative number, so "
                "its pair can be formed")
        # The record's own classification and identity, held
        # beside its pair-forming fields (round-23): the item
        # stamp is the item its probe prices (run_pricing_
        # probes stamps it from the same PROBES mapping that
        # keys the evidence); an "ok" record's exit_code is
        # the fast path's expected code — the timed run sets
        # "ok" only when the exit matches it, so an "ok"
        # record whose code differs or was never observed is
        # a shape no run writes; and the loaded-only fields
        # the timed run writes only on the owner-verification
        # loaded shape (guard_pid, outer_elapsed_ms) are
        # carried by no other probe or shape.
        if isinstance(probe, str) and probe in pricing_mod.PROBES \
                and record.get("item") != pricing_mod.PROBES[probe]:
            problems.append(
                f"{where} field 'item' is "
                f"{record.get('item')!r}; a pricing-probe "
                f"record for probe {probe!r} must carry the "
                f"item that probe prices, "
                f"{pricing_mod.PROBES[probe]!r} — the stamp "
                "run_pricing_probes writes from the same "
                "mapping that keys the pricing evidence")
        if record.get("outcome") == "ok":
            expected = expected_exit("guard", "fast")
            actual = record.get("exit_code")
            if not isinstance(actual, int) \
                    or isinstance(actual, bool) \
                    or actual != expected:
                problems.append(
                    f"{where} field 'exit_code' is "
                    f"{actual!r}; an 'ok' pricing-probe record "
                    f"must carry the fast path's expected exit "
                    f"code {expected!r} — the timed run records "
                    "'ok' only when the run's exit matches it, "
                    "so an ok record whose exit code differs "
                    "or was never observed claims a success "
                    "its own exit field refutes")
        if not (probe == "owner-verification"
                and shape == "loaded"):
            for field in ("guard_pid", "outer_elapsed_ms"):
                if record.get(field) is not None:
                    problems.append(
                        f"{where} field '{field}' is "
                        f"{record.get(field)!r}; that field is "
                        "written only on the owner-verification "
                        "loaded shape (the recording launcher's "
                        "bracket of the guard process), so a "
                        "record of any other probe or shape "
                        "carrying it is not a record the run "
                        "wrote")
        else:
            # On the owner loaded shape itself, the bracket
            # fields' values are held to the construction
            # (round-24, pricing.pricing_stamp_defects' idiom):
            # guard_pid in PID form, outer_elapsed_ms a finite,
            # non-negative bracket covering the record's own
            # elapsed_ms. An "ok" record must carry both in
            # those forms; another outcome is held wherever it
            # carries a value (a launcher that never ran writes
            # no guard_pid, and that absence is its shape).
            guard_pid = record.get("guard_pid")
            if record.get("outcome") == "ok" \
                    or guard_pid is not None:
                if not report_mod._is_pid_form(guard_pid):
                    problems.append(
                        f"{where} field 'guard_pid' is "
                        f"{guard_pid!r}; an owner-verification "
                        "loaded record's guard_pid must be the "
                        "guard child's PID as a positive integer "
                        "within the platform PID range — the "
                        "recording launcher records that PID in "
                        "its bracket record, so an erased, "
                        "negative or non-integer value is not a "
                        "record the run wrote")
            outer = record.get("outer_elapsed_ms")
            elapsed = record.get("elapsed_ms")
            outer_ok = _is_plausible_elapsed(outer) \
                and not (_is_plausible_elapsed(elapsed)
                         and outer < elapsed)
            if record.get("outcome") == "ok" \
                    or outer is not None:
                if not outer_ok:
                    problems.append(
                        f"{where} field 'outer_elapsed_ms' is "
                        f"{outer!r}; an owner-verification "
                        "loaded record's outer_elapsed_ms must "
                        "be the harness's bracket around the "
                        "recording launcher — a finite, "
                        "non-negative time not below the "
                        "record's own elapsed_ms, the launcher's "
                        "bracket inside it")
    return problems


def environment_record():
    return {
        "platform": platform.platform(),
        "os": platform.system(),
        "os_release": platform.release(),
        "os_version": platform.version(),
        "machine": platform.machine(),
        "cpu": platform.processor() or platform.machine(),
        "node": platform.node(),
        "python": platform.python_version(),
    }


def new_run_id(seed):
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return f"run-{stamp}-s{seed}"


def namespace_run_dir(out_dir, run_id):
    """The run's own directory inside out_dir, created fresh. An
    existing directory of the same id is never reused: a numeric
    suffix is added until the name is free, so no prior run's files
    are ever overwritten."""
    os.makedirs(out_dir, exist_ok=True)
    candidate = os.path.join(out_dir, run_id)
    suffix = 2
    while os.path.exists(candidate):
        candidate = os.path.join(out_dir, f"{run_id}-{suffix}")
        suffix += 1
    os.makedirs(candidate)
    return candidate


def _recorded_guard_sha(meta):
    """The guard SHA-256 run.json records, or None when the
    recorded fingerprints do not carry one as a string. The
    pricing provenance check compares the pricing records'
    fingerprint against this value; a manifest whose fingerprints
    are not the object cmd_run writes (a per-arm string, a list)
    records no usable anchor and the comparison fails as a
    mismatch — it is never allowed to crash the check."""
    fingerprints = meta.get("fingerprints")
    if not isinstance(fingerprints, dict):
        return None
    guard = fingerprints.get("guard")
    if not isinstance(guard, dict):
        return None
    sha = guard.get("sha256")
    return sha if isinstance(sha, str) else None


def _recorded_identity_losses(meta):
    """The manifest's recorded competitor-identity mismatches as
    a list of strings, or None when the recorded value is not a
    list of strings. The outcome recomputations consume the
    recorded list as loss strings (campaign_outcome partitions
    losses by prefix); a recorded value of any other shape
    carries no usable claim, so the recomputation rests on the
    recomputed identity alone."""
    recorded = meta.get("competitor_identity_mismatches")
    if not isinstance(recorded, list) \
            or not all(isinstance(entry, str) for entry in recorded):
        return None
    return recorded


def _recomputed_identity_mismatches(meta):
    """Recompute the competitor-identity mismatches from the
    fingerprints and probed versions a manifest itself records,
    against the pinned identities in the configuration the
    manifest names — but only when that configuration file is
    present and hashes to the SHA-256 the manifest records, so
    the pins applied are provably the run's own. Returns the
    recomputed mismatch list, or None when the configuration
    cannot be verified (absent, or its hash differs), in which
    case the recorded list is all the package offers."""
    config_path = meta.get("config_path")
    recorded_sha = meta.get("config_sha256")
    if not isinstance(config_path, str) \
            or not isinstance(recorded_sha, str) \
            or not config_path or not recorded_sha \
            or not os.path.isfile(config_path):
        return None
    try:
        with open(config_path, "rb") as handle:
            if hashlib.sha256(handle.read()).hexdigest() != recorded_sha:
                return None
        config = load_config(config_path)
    except (OSError, ValueError):
        return None
    fingerprints = meta.get("fingerprints")
    versions = meta.get("arm_versions")
    return identity_mismatches(
        config,
        fingerprints if isinstance(fingerprints, dict) else {},
        versions if isinstance(versions, dict) else {})


def _package_expected_output(meta):
    """The expected output bytes a preserved package's match
    verdicts are held to, derived from the configuration the
    manifest names — but only when that configuration file is
    present and hashes to the SHA-256 the manifest records, so
    the expectations applied are provably the run's own (the
    same anchor _recomputed_identity_mismatches uses). Returns
    the expected-output mapping
    (report.expected_output_from_config), or None when the
    configuration cannot be verified (absent, or its hash
    differs): the package's match evidence is then held to the
    digest form the runner writes, and no expected-bytes
    comparison is claimed for it."""
    config_path = meta.get("config_path")
    recorded_sha = meta.get("config_sha256")
    if not isinstance(config_path, str) \
            or not isinstance(recorded_sha, str) \
            or not config_path or not recorded_sha \
            or not os.path.isfile(config_path):
        return None
    try:
        with open(config_path, "rb") as handle:
            if hashlib.sha256(handle.read()).hexdigest() != recorded_sha:
                return None
        config = load_config(config_path)
    except (OSError, ValueError):
        return None
    return report_mod.expected_output_from_config(config)


# The configuration digest recorded by the committed reduced stub run
# in tests/evidence/reduced-run/run.json. It identifies only the known
# historical exit contract, never a verified configuration anchor.
LEGACY_REDUCED_STUB_CONFIG_SHA256 = (
    "ca7872d19554552c55d2df72323bec76eed20906faef4dd53c34844010919f5a")


def _package_plan_config(meta):
    """The plan a preserved package's argv length budget is
    derived from (round-36 F3): the anchored configuration
    the manifest names, when it verifies — the run's own
    plan, the same anchor _package_expected_output trusts.
    When the anchor cannot be verified, an unverified-plan
    sentinel: no budget is derived from the package's own
    recorded templates (that would compare the package
    against itself), and the absent anchor is already named
    unverifiable by _recorded_run_field_problems — since
    round-37 F3 for the argv length budget itself, in that
    function's degraded-verification list. Since round-38
    F2 the sentinel also degrades the verdict itself:
    report's dimension-(6) sentinel cap renders that
    dimension UNEVALUATED under this footing whenever it
    would otherwise PASS, so a recomputation under the
    sentinel — campaign_outcome and cmd_report alike — can
    no longer render COMPLETE over the unheld budget, and
    the two surfaces agree with the checker's problems."""
    anchored = _anchored_config(meta)
    if anchored is not None:
        return anchored
    if meta.get("reduced") is True and \
            meta.get("config_sha256") == LEGACY_REDUCED_STUB_CONFIG_SHA256:
        return report_mod.NO_VERIFIED_LEGACY_STUB_PLAN
    return report_mod.NO_VERIFIED_PLAN


def _report_status_problems(run_dir, meta, records, probe_records):
    """Verify report.txt's STATUS line against the outcome the
    package's own records recompute to. Only the verdict line is
    checked, and only when the pricing evidence the recomputation
    rests on is itself verified (or the run recorded no pricing
    observations at all): a package whose pricing provenance is
    already defective is named for that, not for a verdict this
    check cannot recompute on the same footing as the run did."""
    path = os.path.join(run_dir, "report.txt")
    if not os.path.isfile(path):
        return []
    try:
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
    except OSError as error:
        return [f"report.txt unreadable: {error}"]
    status_lines = [line for line in text.splitlines()
                    if line.startswith("STATUS:")]
    if not status_lines:
        return ["report.txt carries no STATUS line; the package's "
                "human-facing conclusion states no verdict"]
    evidence, _problems = _verified_pricing_evidence(meta, probe_records)
    if evidence is None and (probe_records
                             or meta.get("pricing_probe_observations")):
        return []
    # Identity rests on the recomputation alone (round-25
    # P1): when the configuration anchor is absent the
    # recomputation is None and the outcome is computed with
    # no identity losses — never with the manifest's own
    # recorded list, which would compare the package against
    # itself; the absent anchor's degradation is named by
    # _recorded_run_field_problems.
    identity = _recomputed_identity_mismatches(meta)
    outcome = campaign_outcome(
        records, meta.get("plan_counts") or check_plan(build_plan()),
        identity_losses=None if meta.get("reduced") else identity,
        pricing=evidence, decisions=_recorded_decisions(meta),
        reduced=bool(meta.get("reduced")),
        expected_output=_package_expected_output(meta),
        config=_package_plan_config(meta))
    actual_complete = outcome["campaign_status"] == CAMPAIGN_STATUS_COMPLETE \
        and not meta.get("provisional")
    claims_complete = status_lines[0].startswith("STATUS: complete")
    if claims_complete == actual_complete:
        return []
    return [
        f"report.txt STATUS line claims "
        f"{'a complete campaign' if claims_complete else 'an incomplete campaign'} "
        f"but the package's own records recompute the outcome as "
        f"{outcome['campaign_status']!r}; the report's verdict "
        "disagrees with the package's evidence"]


def _report_verdict_problems(run_dir, meta, records, probe_records):
    """Verify report.txt's verdict-bearing sections against the
    outcome the package's own records recompute to (round-25
    P5): the freeze-gate lines, the evaluation-profile lines,
    the claim-ledger lines and the standing-losses list.
    Rounds 23/24 held the STATUS line, the fingerprint lines
    and the platform line, but the sections a reader actually
    reads for the verdict — a gate line flipped to PASS, a
    ledger state flipped to PASS, a standing-loss line
    deleted — were recomputed nowhere and passed above a
    genuine STATUS line. Each section is now compared, line
    for line, with the section render() would produce from
    this package's records (the same recomputation
    _report_status_problems applies for the STATUS line, with
    the same pricing-evidence gating: a package whose pricing
    provenance is defective is named for that, not for a
    verdict this check cannot recompute on the run's footing).
    The identity losses fed to the loss recomputation are the
    recomputed ones when the anchor verifies and the recorded
    ones otherwise — this is the report body's internal
    consistency against the package's records; competitor
    identity's own verification is named separately (see
    _recorded_run_field_problems)."""
    path = os.path.join(run_dir, "report.txt")
    if not os.path.isfile(path):
        return []
    try:
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
    except OSError as error:
        return [f"report.txt unreadable: {error}"]
    evidence, _problems = _verified_pricing_evidence(
        meta, probe_records)
    if evidence is None and (probe_records
                             or meta.get("pricing_probe_observations")):
        return []
    expected_output = _package_expected_output(meta)
    decisions = _recorded_decisions(meta)
    identity = _recomputed_identity_mismatches(meta)
    if identity is None:
        identity = _recorded_identity_losses(meta)
    lines = text.splitlines()

    def section(header):
        out = []
        on = False
        for line in lines:
            if line.startswith(header):
                on = True
                continue
            if on:
                if line == "":
                    break
                out.append(line)
        return out

    problems = []

    def compare(section_name, stated, expected):
        if stated == expected:
            return
        detail = None
        for index, (got, want) in enumerate(
                zip(stated, expected)):
            if got != want:
                detail = (f"line {index + 1} states {got!r} but "
                          f"the package's records recompute "
                          f"{want!r}")
                break
        if detail is None:
            detail = (f"it carries {len(stated)} line(s) but "
                      f"the package's records recompute "
                      f"{len(expected)} line(s) — a line the "
                      "recomputation gives is missing from the "
                      "report, or the report carries one it "
                      "does not give")
        problems.append(
            f"report.txt {section_name} section disagrees with "
            f"the package's own recomputed outcome: {detail}; "
            "the report's verdict-bearing sections must state "
            "the outcomes the package's records earn")

    expected_gate = [
        f"  [{entry['context']}] {entry['dimension']}: "
        f"{entry['outcome']} — {entry['detail']}"
        for entry in report_mod.freeze_gate(
            records, expected_output, _package_plan_config(meta))]
    compare("freeze gate", section("Freeze gate ("), expected_gate)
    profile_section = [
        line for line in section("Evaluation profile (")
        if re.match(r"^  \(\d+\) ", line)]
    expected_profile = [
        f"  ({index}) {entry['dimension']}: {entry['state']} "
        f"— {entry['detail']}"
        for index, entry in enumerate(
            report_mod.profile_dimension_states(
                records, decisions, expected_output,
                _package_plan_config(meta)), 1)]
    compare("evaluation profile", profile_section, expected_profile)
    expected_ledger = [
        f"  ({index}) {entry['dimension']}: {entry['state']} "
        f"— basis: {entry['basis']}"
        for index, entry in enumerate(
            report_mod.claim_ledger(
                records, decisions, expected_output,
                _package_plan_config(meta)), 1)]
    compare("claim ledger", section("Claim ledger ("),
            expected_ledger)
    losses = report_mod.campaign_losses(
        records, meta.get("plan_counts") or check_plan(build_plan()),
        identity_losses=None if meta.get("reduced") else identity,
        pricing=evidence, decisions=decisions,
        reduced=bool(meta.get("reduced")),
        expected_output=expected_output,
        config=_package_plan_config(meta))
    competitor = [loss for loss in losses
                  if loss.startswith(
                      report_mod.COMPETITOR_LOSS_PREFIX)]
    campaign = [loss for loss in losses
                if not loss.startswith(
                    report_mod.COMPETITOR_LOSS_PREFIX)]
    if competitor:
        compare("competitor losses",
                section("Competitor losses ("),
                [f"  - {loss}" for loss in competitor])
    if campaign:
        stated_losses = []
        on = False
        for line in lines:
            if line == "Standing losses:":
                on = True
                continue
            if on:
                stated_losses.append(line)
        compare("standing losses", stated_losses,
                [f"  - {loss}" for loss in campaign])
    elif "Standing losses:" in lines:
        problems.append(
            "report.txt standing losses section is present but "
            "the package's own records recompute no standing "
            "losses; the report's verdict-bearing sections must "
            "state the outcomes the package's records earn")
    return problems


# The run-id form new_run_id generates: the run's seed is part
# of the id, so a generated-form id naming another seed names
# another run.
_GENERATED_RUN_ID = re.compile(r"run-\d{8}T\d{6}Z-s(\d+)")


def _anchored_config(meta):
    """The configuration a manifest names, loaded only when the
    file is present and hashes to the SHA-256 the manifest
    records — the anchor _package_expected_output trusts — or
    None when it cannot be verified, in which case no claim is
    held against it."""
    config_path = meta.get("config_path")
    recorded_sha = meta.get("config_sha256")
    if not isinstance(config_path, str) \
            or not isinstance(recorded_sha, str) \
            or not config_path or not recorded_sha \
            or not os.path.isfile(config_path):
        return None
    try:
        with open(config_path, "rb") as handle:
            if hashlib.sha256(handle.read()).hexdigest() != recorded_sha:
                return None
        return load_config(config_path)
    except (OSError, ValueError):
        return None


def _recorded_run_field_problems(meta, namespace=None):
    """The manifest's beside-fields (round-23): run_id, the
    recorded environment and config_templates are evidence the
    package carries about its own run, held here to the run's
    own anchors. A generated-form run_id embeds its run's seed
    (new_run_id), so one naming another seed is a foreign id;
    the recorded environment must agree with the manifest's
    own platform / python fields (cmd_run records both from
    the same environment_record) and with itself (its os is
    the platform string's own first component, as
    platform.platform() composes it); and the recorded
    templates must be the templates of the configuration the
    manifest's hash names, when that configuration is
    verifiable (the _package_expected_output anchor).
    Round-24 completes the hold one step further: a run_id is
    also held to the namespace the package is verified in
    (cmd_run records run_id as the namespace directory's own
    name — a foreign name, or a generated-form id naming
    another run's date, is not this package's id); the
    recorded environment must carry exactly the fields
    environment_record writes and its anchored components
    (os_release and, off Windows, machine are components of
    the platform string platform.platform() composes from
    them) must agree with that string; and a package whose
    recorded config_templates cannot be checked against the
    run's configuration — the package carries neither the
    configuration's content nor a live anchor for it — is
    NAMED as unverifiable here, never silently passed (the
    preserved reduced-run package is that case: its
    config_path names a deleted scratch file)."""
    problems = []
    run_id = meta.get("run_id")
    if run_id is not None:
        if not isinstance(run_id, str) or not run_id:
            problems.append(
                f"run.json records run_id {run_id!r}; a run id "
                "must be a non-empty string naming the run's "
                "namespace")
        else:
            generated = _GENERATED_RUN_ID.fullmatch(run_id)
            seed = meta.get("seed")
            if generated is not None \
                    and isinstance(seed, int) \
                    and not isinstance(seed, bool) \
                    and int(generated.group(1)) != seed:
                problems.append(
                    f"run.json records run_id {run_id!r}, whose "
                    f"generated form names seed "
                    f"{generated.group(1)}, but the run's "
                    f"recorded seed is {seed}; the run id is "
                    "not this run's id")
            if namespace is not None and run_id != namespace:
                problems.append(
                    f"run.json records run_id {run_id!r} but "
                    f"the package is verified in the namespace "
                    f"{namespace!r}; cmd_run records a run's id "
                    "as its own namespace's name, so an id "
                    "naming another namespace — a foreign name, "
                    "or a generated-form id for another run — "
                    "is not this package's id")
    environment = meta.get("environment")
    if environment is not None and not isinstance(environment, dict):
        problems.append(
            f"run.json records environment {environment!r}, "
            "not an object; the environment a run records is "
            "the object environment_record writes (platform, "
            "os, release, version, machine, cpu, node, python)")
    elif isinstance(environment, dict):
        for key, value in environment.items():
            if not isinstance(value, str):
                problems.append(
                    f"run.json records environment {key} "
                    f"{value!r}, not a string; every field of "
                    "the recorded environment is a string the "
                    "run's own platform probes produced")
        # Exactly the recorded field set (round-24): a run's
        # environment is the object environment_record writes
        # — eight fields, no fewer (an erased field is a
        # written field gone) and no more. An empty object is
        # the minimal-manifest shape and names no environment
        # at all, so the set hold applies only when a record
        # is carried. The fields with no second anchor in the
        # package (node, cpu, os_version — primary records,
        # stated at their true breadth in FAILURE-TREE) are
        # held by this set and stringness discipline; the
        # components the platform string itself anchors are
        # held to it below.
        if environment:
            expected_fields = {
                "platform", "os", "os_release", "os_version",
                "machine", "cpu", "node", "python"}
            for field in sorted(expected_fields - set(environment)):
                problems.append(
                    f"run.json records an environment with no "
                    f"{field} field; the environment a run "
                    "records is the object environment_record "
                    "writes, which carries every one of its "
                    "fields — an erased environment field is a "
                    "written field gone")
            for field in sorted(set(environment) - expected_fields):
                problems.append(
                    f"run.json records environment field "
                    f"{field!r}, which environment_record "
                    "never writes; the recorded environment "
                    "carries exactly that function's fields")
        platform_value = environment.get("platform")
        if isinstance(platform_value, str) \
                and meta.get("platform") is not None \
                and platform_value != meta.get("platform"):
            problems.append(
                f"run.json records environment platform "
                f"{platform_value!r} but its own platform field "
                f"records {meta.get('platform')!r}; the two are "
                "recorded from the same environment record, so "
                "a package whose environment names another "
                "machine disagrees with its own manifest")
        python_value = environment.get("python")
        if isinstance(python_value, str) \
                and meta.get("python") is not None \
                and python_value != meta.get("python"):
            problems.append(
                f"run.json records environment python "
                f"{python_value!r} but its own python field "
                f"records {meta.get('python')!r}; the two are "
                "recorded from the same environment record")
        os_value = environment.get("os")
        if isinstance(os_value, str) \
                and isinstance(platform_value, str) \
                and platform_value.split("-", 1)[0] != os_value:
            problems.append(
                f"run.json records environment os {os_value!r} "
                f"but its recorded platform is "
                f"{platform_value!r}; the platform string the "
                "run records is composed from that same os "
                "name, so an environment claiming another os "
                "for the recorded platform is fabricated")
        # The platform string's own anchored components
        # (round-24): platform.platform() composes the string
        # from the same uname fields environment_record
        # records separately — the release sits in it as a
        # '-'-delimited component, and (off Windows, whose
        # platform string carries no machine component) so
        # does the machine. A recorded os_release or machine
        # that is not a component of the package's own
        # platform string contradicts the manifest itself.
        # Round-25 completes the hold positionally: membership
        # is not position — the two genuine values SWAPPED
        # (the release recorded as the machine and the machine
        # as the release) are each components of the string
        # and passed the round-24 hold. The composition order
        # platform.platform() writes is os, then release, then
        # machine: the string opens with the os name, the
        # release follows it immediately, and the machine
        # follows the release. Each anchored field is held to
        # its own component at its own position below, beside
        # the membership hold.
        if isinstance(platform_value, str) \
                and isinstance(os_value, str) and os_value \
                and platform_value.startswith(os_value + "-"):
            remainder = platform_value[len(os_value) + 1:]
            release_value = environment.get("os_release")
            machine_value = environment.get("machine")
            release_position_ok = True
            if isinstance(release_value, str) and release_value:
                if remainder == release_value:
                    release_position_ok = True
                    remainder = ""
                elif remainder.startswith(release_value + "-"):
                    release_position_ok = True
                    remainder = remainder[len(release_value) + 1:]
                else:
                    release_position_ok = False
                    problems.append(
                        f"run.json records environment os_release "
                        f"{release_value!r} but its recorded "
                        f"platform is {platform_value!r}, whose "
                        "release component — the component the "
                        "platform string composes immediately "
                        "after its os component — is not that "
                        "value; each environment field is held "
                        "to its own component of the recorded "
                        "platform, so a release recorded in "
                        "another field's position (the machine "
                        "and release values swapped, each "
                        "genuine in the other's place) is "
                        "fabricated in this field")
            if release_position_ok \
                    and isinstance(machine_value, str) \
                    and machine_value and os_value != "Windows" \
                    and remainder:
                if not (remainder == machine_value
                        or remainder.startswith(
                            machine_value + "-")):
                    problems.append(
                        f"run.json records environment machine "
                        f"{machine_value!r} but its recorded "
                        f"platform is {platform_value!r}, whose "
                        "machine component — the component the "
                        "platform string composes immediately "
                        "after its release component — is not "
                        "that value; each environment field is "
                        "held to its own component of the "
                        "recorded platform, so a machine "
                        "recorded in another field's position "
                        "(the machine and release values "
                        "swapped, each genuine in the other's "
                        "place) is fabricated in this field")
        if isinstance(platform_value, str):
            release_value = environment.get("os_release")
            if isinstance(release_value, str) and release_value \
                    and f"-{release_value}-" not in platform_value \
                    and not platform_value.endswith(
                        f"-{release_value}"):
                problems.append(
                    f"run.json records environment os_release "
                    f"{release_value!r} but its recorded "
                    f"platform is {platform_value!r}; the "
                    "platform string is composed from that "
                    "same release, so a release that is not a "
                    "component of the recorded platform is "
                    "fabricated")
            machine_value = environment.get("machine")
            if isinstance(machine_value, str) and machine_value \
                    and os_value != "Windows" \
                    and f"-{machine_value}-" not in platform_value \
                    and not platform_value.endswith(
                        f"-{machine_value}"):
                problems.append(
                    f"run.json records environment machine "
                    f"{machine_value!r} but its recorded "
                    f"platform is {platform_value!r}; the "
                    "platform string is composed from that "
                    "same machine name, so a machine that is "
                    "not a component of the recorded platform "
                    "is fabricated")
    templates = meta.get("config_templates")
    if templates is not None:
        if not isinstance(templates, dict):
            problems.append(
                f"run.json records config_templates "
                f"{templates!r}, not an object mapping each "
                "arm to the templates the run used")
        else:
            for arm, entry in templates.items():
                if entry is not None and not isinstance(entry, dict):
                    problems.append(
                        f"run.json records config_templates "
                        f"for {arm!r} as {entry!r}, not an "
                        "object mapping paths to their "
                        "templates")
                elif isinstance(entry, dict):
                    for path_name, template in entry.items():
                        if not isinstance(template, list) \
                                or any(not isinstance(token, str)
                                       for token in template):
                            problems.append(
                                f"run.json records "
                                f"config_templates for {arm!r} "
                                f"path {path_name!r} as "
                                f"{template!r}, not a list of "
                                "string argv tokens — the shape "
                                "the run records from its "
                                "configuration")
            config = _anchored_config(meta)
            if config is not None:
                expected_templates = {}
                arms = config.get("arms")
                if isinstance(arms, dict):
                    for arm in PRODUCT_ARMS:
                        entry = arms.get(arm)
                        expected_templates[arm] = (
                            entry.get("templates")
                            if isinstance(entry, dict) else None)
                if templates != expected_templates:
                    problems.append(
                        f"run.json records config_templates "
                        f"{templates!r} but the configuration "
                        f"whose SHA-256 it records carries "
                        f"templates {expected_templates!r}; the "
                        "recorded templates are not the "
                        "templates the run's own configuration "
                        "used")
            else:
                # The unverifiable case, named (round-24 P5):
                # the package records templates but carries
                # neither the configuration's content nor a
                # live anchor for it (the configuration file
                # its config_path names is absent, or does not
                # hash to the recorded config_sha256), so the
                # recorded content cannot be checked against
                # the run's configuration at all. That is a
                # package fact, reported by name — a package
                # whose templates cannot be anchored does not
                # check indistinguishably from one whose
                # templates were verified.
                problems.append(
                    f"run.json records config_templates "
                    f"{templates!r} but they are unverifiable "
                    f"from this package: the configuration it "
                    f"names (config_path "
                    f"{meta.get('config_path')!r}) is absent "
                    "or does not hash to the recorded "
                    "config_sha256, and the package carries "
                    "no configuration content of its own, so "
                    "the recorded templates cannot be checked "
                    "against the run's configuration")
    # The naming duty is GENERAL (round-25 P1): round 24 named
    # only config_templates, while the same absent anchor
    # silently degraded three sibling verifications —
    # competitor identity fell back to the manifest's own
    # recorded mismatch list, the expected-output pairing fell
    # back to digest form only, and the profile decisions were
    # taken from run.json itself, so the checker compared the
    # package against itself in three places it claimed to
    # verify. Every verification the absent anchor degrades is
    # named here: when the package claims a configuration
    # anchor (a config_path naming the configuration, or recorded
    # templates) and that anchor cannot be verified (the file
    # is absent or does not hash to the recorded SHA-256),
    # the checker names each degraded verification as
    # unverifiable, and no verification in this checker falls
    # back to the package's own recorded claim as its source
    # (identity recomputations rest on the recomputation or
    # on nothing; decision verification rests on the anchored
    # configuration or is named here — see
    # _claim_ledger_problems). The genuine preserved package's
    # named-unverifiable set is exactly this degraded set, no
    # more and no less.
    # The duty attaches where the package records the anchor
    # it claims: a config_path naming the run's configuration
    # file, or recorded templates claiming configuration
    # content. A manifest recording neither (a bare hash with
    # no named file and no templates — the minimal-manifest
    # shape) claims no anchor for these verifications to
    # degrade from.
    anchor_claimed = bool(
        (isinstance(meta.get("config_path"), str)
         and meta.get("config_path"))
        or templates is not None)
    if anchor_claimed and _anchored_config(meta) is None:
        anchor_note = (
            f"the configuration this package names "
            f"(config_path {meta.get('config_path')!r}) is "
            "absent or does not hash to the recorded "
            "config_sha256, and the package carries no "
            "configuration content of its own")
        degraded = [
            ("competitor identity",
             "run.json's competitor identity verdict is "
             "unverifiable from this package: " + anchor_note
             + ", so the recorded fingerprints and probed "
             "versions cannot be checked against the pinned "
             "identities in the run's configuration — the "
             "checker does not fall back to the manifest's "
             "own recorded mismatch list, which would compare "
             "the package against itself"),
            ("expected-output pairing",
             "the expected-output pairing for this package's "
             "match verdicts is unverifiable: " + anchor_note
             + ", so no expected-bytes mapping can be derived "
             "from the run's configuration and a True verdict "
             "is held to the digest form only — a breadth "
             "reduction named here, not a silent fallback"),
            ("profile decisions",
             "run.json's profile decisions are unverifiable "
             "from this package: " + anchor_note + ", so the "
             "recorded profile_dimension_decisions cannot be "
             "checked against the decisions in the run's "
             "configuration — the checker does not source "
             "decisions from the package under check and "
             "present them as verified"),
            ("argv length budget",
             "the argv length budget for this package's "
             "malformed records is unverifiable from this "
             "package: " + anchor_note + ", so the package "
             "footing derives no fast-vector length for any "
             "arm (the NO_VERIFIED_PLAN sentinel — no budget "
             "is taken from the package's own recorded "
             "templates, which would compare the package "
             "against itself) and the length limb of the "
             "malformed argv exactness hold — the only limb "
             "that names added content on the timeout "
             "shapes — does not fire for this package's "
             "records: dimension (6)'s exactness stands "
             "degraded for this package, named here, not "
             "silently unheld (round-37 F3)"),
        ]
        if not any("config_templates" in problem
                   and "unverifiable" in problem
                   for problem in problems):
            degraded.append((
                "config content",
                "run.json's config content (config_templates) "
                "is unverifiable from this package: "
                + anchor_note + ", so the recorded templates "
                "cannot be checked against the run's "
                "configuration"))
        for _name, text in degraded:
            problems.append(text)
    return problems


def _report_body_problems(run_dir, meta):
    """report.txt's body beside its STATUS line (round-23): the
    report prints the anchor arm's fingerprint and every arm's
    fingerprint from the manifest; a fingerprint the report
    prints that the manifest does not record — a wrong anchor
    fingerprint above a genuine STATUS line — is a package
    defect, named here, not a conclusion the package may
    carry. Round-24 completes the body hold: the fingerprint
    lines' beside-values the report prints from the same
    manifest entries (each arm's path, size and probed
    version, the anchor line's path and size) and the
    platform line it prints from the manifest's platform are
    held to those entries — a report whose fingerprint is
    genuine but whose size, version, path or platform
    contradicts the manifest above it is named the same way."""
    path = os.path.join(run_dir, "report.txt")
    if not os.path.isfile(path):
        return []
    try:
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
    except OSError as error:
        return [f"report.txt unreadable: {error}"]
    recorded = set()
    fingerprints = meta.get("fingerprints")
    if isinstance(fingerprints, dict):
        for entry in fingerprints.values():
            if isinstance(entry, dict) \
                    and isinstance(entry.get("sha256"), str):
                recorded.add(entry["sha256"])
    problems = []
    for value in sorted(set(re.findall(r"sha256=([0-9a-f]{64})",
                                       text))):
        if value not in recorded:
            problems.append(
                f"report.txt records fingerprint sha256="
                f"{value}, which is not among the fingerprints "
                f"run.json records {sorted(recorded)!r}; the "
                "report's anchor fingerprint disagrees with "
                "the manifest it was rendered from")
    lines = text.splitlines()
    # The platform line: report.render prints the manifest's
    # platform there, word for word.
    meta_platform = meta.get("platform")
    for line in lines:
        if line.startswith("platform: "):
            stated = line[len("platform: "):]
            if isinstance(meta_platform, str) \
                    and stated != meta_platform:
                problems.append(
                    f"report.txt records platform {stated!r} "
                    f"but run.json records platform "
                    f"{meta_platform!r}; the report's platform "
                    "line disagrees with the manifest it was "
                    "rendered from")
            break
    if isinstance(fingerprints, dict):
        versions = meta.get("arm_versions")
        if not isinstance(versions, dict):
            versions = {}
        arm_line = re.compile(
            r"^  (\w+): (\S.*) sha256=([0-9a-f]{64}) "
            r"size=(\S+) version=(.*)$")
        anchor_line = re.compile(
            r"^  (\S.*) sha256=([0-9a-f]{64}) size=(\S+)$")

        def beside_problems(owner, stated_path, stated_size,
                            entry):
            out = []
            if not isinstance(entry, dict):
                return out
            recorded_path = entry.get("path", "not recorded")
            if stated_path != recorded_path:
                out.append(
                    f"report.txt records {owner} path "
                    f"{stated_path!r} but run.json records "
                    f"its path as {recorded_path!r}; the "
                    "report's fingerprint line disagrees with "
                    "the manifest it was rendered from")
            recorded_size = entry.get("size", "not recorded")
            if stated_size != str(recorded_size):
                out.append(
                    f"report.txt records {owner} size="
                    f"{stated_size} but run.json records its "
                    f"size as {recorded_size!r}; the report's "
                    "fingerprint line disagrees with the "
                    "manifest it was rendered from")
            return out

        for line in lines:
            match = arm_line.match(line)
            if match:
                arm, stated_path, _sha, stated_size, \
                    stated_version = match.groups()
                entry = fingerprints.get(arm)
                if isinstance(entry, dict):
                    problems.extend(beside_problems(
                        f"arm {arm!r}", stated_path,
                        stated_size, entry))
                    probed = versions.get(arm)
                    expected_version = \
                        probed if probed else "not probed"
                    if stated_version != expected_version:
                        problems.append(
                            f"report.txt records arm {arm!r} "
                            f"version={stated_version} but "
                            f"run.json records its probed "
                            f"version as {expected_version!r}; "
                            "the report's fingerprint line "
                            "disagrees with the manifest it "
                            "was rendered from")
                continue
            match = anchor_line.match(line)
            if match:
                stated_path, _sha, stated_size = match.groups()
                problems.extend(beside_problems(
                    "the anchor arm's", stated_path,
                    stated_size, fingerprints.get("guard")))
    return problems


def _manifest_count_problems(meta):
    """Shape defects of the manifest's recorded counts, named per
    field. cmd_run writes every count in run.json as an integer
    (a length, a slot total, a standing-loss or freeze-gate
    count, the pricing-observation total). The verifications
    compare those fields by equality, and in Python a float whose
    value is integral compares equal to the integer (46.0 == 46):
    a manifest carrying a count as a float — a shape no run
    writes — would pass every comparison unnamed, where the same
    count as a string is named. A recorded count must therefore
    carry the integer type, not merely an equal value; a null
    carries no count (the provisional outcome counts) and is not
    faulted here."""
    problems = []
    for field in ("slots_run", "slots_planned",
                  "pricing_probe_observations",
                  "standing_loss_count", "freeze_gate_fail_count"):
        if field in meta and meta.get(field) is not None \
                and (not isinstance(meta.get(field), int)
                     or isinstance(meta.get(field), bool)):
            problems.append(
                f"run.json records {field} {meta.get(field)!r}; "
                "a recorded count must be an integer — the counts "
                "cmd_run writes are integers, and a value of "
                "another type is not the recorded count, however "
                "it compares against the recomputed count")
    # The same rule one level down: the recorded plan's own
    # counts (plan_counts.control / .warmup / .measured) are
    # compared against the derived plan by dict equality, and
    # 36.0 == 36 there too — a float plan count would pass the
    # plan verification unnamed where a float slots_planned is
    # named above. A recorded plan count must likewise carry
    # the integer type, not merely an equal value.
    plan_counts = meta.get("plan_counts")
    if isinstance(plan_counts, dict):
        for field in ("control", "warmup", "measured"):
            if field in plan_counts \
                    and plan_counts.get(field) is not None \
                    and (not isinstance(plan_counts.get(field), int)
                         or isinstance(plan_counts.get(field), bool)):
                problems.append(
                    f"run.json records plan_counts {field} "
                    f"{plan_counts.get(field)!r}; a recorded plan "
                    "count must be an integer — the counts "
                    "check_plan derives are integers, and a value "
                    "of another type is not the recorded count, "
                    "however it compares against the derived count")
    return problems


def _slot_job_drained(receipt):
    return (
        isinstance(receipt, dict)
        and type(receipt.get("active_before")) is int
        and receipt["active_before"] >= 0
        and receipt.get("terminated") is bool(receipt["active_before"])
        and type(receipt.get("active_after")) is int
        and receipt["active_after"] == 0
    )


def _slot_isolation_problems(meta, records):
    declared = meta.get("slot_isolation")
    has_receipts = any(isinstance(record, dict) and "slot_job" in record
                       for record in records)
    if declared is None and not has_receipts:
        return []
    if declared != WINDOWS_SLOT_ISOLATION:
        return ["run.json has no matching Windows slot isolation declaration"]
    return [
        f"observation {index} lacks a successful slot Job drain receipt"
        for index, record in enumerate(records, 1)
        if not isinstance(record, dict) or not _slot_job_drained(record.get("slot_job"))
    ]


def check_run_package(run_dir):
    """Verify a run package's order evidence: run.json's recorded
    seed must reproduce the recorded order, and the observations'
    slot sequence must be a prefix of that order (equal to it for a
    finished run; a proper prefix for a stopped, provisional one,
    whose recorded slots_run must then match the record count).
    Returns a list of problems; empty means the package's order
    evidence is consistent (for a stopped package:
    consistent-as-a-prefix). A package with observations but no
    run.json has no recorded seed, so its order is unverifiable —
    that is reported as a problem, while cmd_report still renders
    the namespace as INCOMPLETE. The pricing file is verified too:
    its records' guard fingerprint and count must match run.json's
    recorded fingerprint and pricing_probe_observations count, and
    each item's paired count must meet the run shape's expected
    reps — a mismatch is a package defect (see
    _pricing_provenance_problems)."""
    problems = []
    try:
        with open(os.path.join(run_dir, "run.json"), encoding="utf-8") as handle:
            meta = json.load(handle)
    except (OSError, ValueError) as error:
        if os.path.isfile(os.path.join(run_dir, "observations.jsonl")):
            return [f"run.json unreadable: {error}; the namespace "
                    "holds observations but no manifest, so its "
                    "order is unverifiable"]
        return [f"run.json unreadable: {error}"]
    if not isinstance(meta, dict):
        return ["run.json is not a JSON object; a run manifest "
                "must carry the seed, order and outcome fields "
                "the package is verified against"]
    problems.extend(_manifest_count_problems(meta))
    problems.extend(_recorded_run_field_problems(
        meta,
        namespace=os.path.basename(
            os.path.normpath(os.path.abspath(run_dir)))))
    seed = meta.get("seed")
    recorded_order = meta.get("order")
    if seed is None or recorded_order is None:
        return problems + [
            "run.json records no seed/order; order unverifiable"]
    if not isinstance(seed, int) or isinstance(seed, bool) \
            or not isinstance(recorded_order, list):
        return problems + [
            "run.json records a seed that is not an integer "
            "or an order that is not a list; order "
            "unverifiable"]
    # Order-entry shape, for the same reason as the count types
    # above: the recorded order is compared to the seeded order
    # by list equality, and list equality compares each entry's
    # repetition by value — 0.0 == 0 and True == 1 — so an order
    # whose repetitions are floats or booleans (shapes no run
    # writes; slot_key writes an integer) would verify as the
    # seeded order unnamed. An entry must carry the shape
    # slot_key writes: [kind, arm, path, context, repetition]
    # with four strings and an integer repetition.
    for index, entry in enumerate(recorded_order):
        if not isinstance(entry, list) or len(entry) not in (5, 6) \
                or not all(isinstance(part, str)
                           for part in entry[:4]) \
                or not isinstance(entry[4], int) \
                or isinstance(entry[4], bool) \
                or (len(entry) == 6
                    and not isinstance(entry[5], str)):
            return problems + [
                f"run.json order entry {index + 1} is "
                f"{entry!r}; a recorded order entry must be "
                "[kind, arm, path, context, repetition] with "
                "four strings and an integer repetition — the "
                "shape slot_key writes, with a malformed or "
                "scaling slot appending its variant as a sixth, "
                "string element — so an order whose "
                "repetitions merely compare equal to the seeded "
                "order's (a float, a boolean) is not the "
                "recorded order; order unverifiable"]
    full = [slot_key(s) for s in build_plan(seed)]
    expected = full[: len(recorded_order)] if meta.get("reduced") else full
    if recorded_order != expected:
        problems.append(
            "run.json order does not match the order its recorded "
            f"seed {seed} produces (seed/order mismatch)")
    try:
        records = _read_records(run_dir)
    except (OSError, ValueError) as error:
        return problems + [f"observations.jsonl unreadable: {error}"]
    problems.extend(_slot_isolation_problems(meta, records))
    # Record shape: a malformed-but-parseable observation record
    # is a package defect the checker names; the recomputations
    # below index the fields it validates, so on any shape defect
    # the checker reports what it has and stops, rather than
    # crashing and hiding every later check.
    shape_problems = _observation_shape_problems(records)
    if shape_problems:
        return problems + shape_problems
    observed = []
    for r in records:
        entry = [r.get("kind"), r.get("arm"), r.get("path"),
                 r.get("context"), r.get("rep")]
        if r.get("variant") is not None:
            entry.append(r.get("variant"))
        observed.append(entry)
    if observed != expected[: len(observed)]:
        problems.append(
            "observations.jsonl slot sequence is not a prefix of "
            f"the order the recorded seed {seed} produces")
    if len(records) > len(expected):
        problems.append("more observations than the plan holds")
    if meta.get("provisional") and meta.get("slots_run") != len(records):
        problems.append(
            f"provisional run.json records slots_run "
            f"{meta.get('slots_run')} but observations.jsonl holds "
            f"{len(records)} record(s)")
    if not meta.get("provisional") \
            and meta.get("slots_run") is not None \
            and meta.get("slots_run") != len(records):
        problems.append(
            f"finalised run.json records slots_run "
            f"{meta.get('slots_run')} but observations.jsonl holds "
            f"{len(records)} record(s); the recorded run size "
            "disagrees with the package's own records")
    # The recorded plan is a claim with an in-package anchor: the
    # plan counts must be the configured plan's own derived
    # counts (check_plan over the plan the recorded seed builds),
    # and slots_planned must be the length of the recorded order
    # itself. A manifest recording any other plan — a smaller
    # one, say, under which its partial records might look
    # complete — disagrees with the order evidence the same
    # manifest carries, and is named here rather than trusted.
    recorded_counts = meta.get("plan_counts")
    if recorded_counts is not None:
        try:
            derived_counts = check_plan(build_plan(seed))
        except (ValueError, TypeError):
            derived_counts = None
        if derived_counts is not None \
                and recorded_counts != derived_counts:
            problems.append(
                f"run.json records plan_counts "
                f"{recorded_counts!r} but the configured plan "
                f"its recorded seed produces derives "
                f"{derived_counts!r}; the recorded plan disagrees "
                "with the package's own order evidence")
    if meta.get("slots_planned") is not None \
            and meta.get("slots_planned") != len(recorded_order):
        problems.append(
            f"run.json records slots_planned "
            f"{meta.get('slots_planned')} but its recorded order "
            f"holds {len(recorded_order)} slot(s); the recorded "
            "plan size disagrees with the package's own order")
    # Competitor identity, recomputed: run.json's recorded
    # mismatch list is a claim, not evidence — the fingerprints
    # and probed versions run.json itself records either earn it
    # or contradict it. When the configuration file the manifest
    # names is present and hashes to the recorded SHA-256, the
    # identity check is recomputed against that configuration's
    # pins and every recomputed mismatch the manifest failed to
    # record is a package defect.
    recomputed_identity = _recomputed_identity_mismatches(meta)
    if recomputed_identity is not None:
        recorded_identity = _recorded_identity_losses(meta) or []
        for mismatch in recomputed_identity:
            if mismatch not in recorded_identity:
                problems.append(
                    "run.json records no such competitor identity "
                    f"mismatch, but the fingerprints and versions "
                    f"it records give one against the pinned "
                    f"identities in the configuration whose "
                    f"SHA-256 it records: {mismatch}; the recorded "
                    "identity verdict disagrees with the package's "
                    "evidence")
    # Report verdict: report.txt is the human-facing conclusion
    # of the package; when it is present, its STATUS line must
    # agree with the outcome the package's own records recompute
    # to (the same recomputation _outcome_problems applies), so a
    # report claiming a complete campaign its records do not earn
    # is a package defect, not a conclusion. (Applied below, once
    # the pricing records have been read.)
    # Pricing provenance (P9): a preserved namespace's pricing
    # records must be the records its run.json says were made —
    # same guard fingerprint, same recorded count, and the paired
    # count per item the run shape expects. A mismatch is a
    # package defect here and renders the affected items
    # "not measured" in cmd_report. The pricing records' probe-shape
    # stamps are verified too (see _pricing_stamp_problems).
    probe_records = []
    pricing_shape = []
    try:
        probe_records = _read_pricing_records(run_dir)
    except (OSError, ValueError) as error:
        problems.append(f"pricing-observations.jsonl unreadable: {error}")
    else:
        # Pricing record shape, for the same reason as the
        # observation shape above: a malformed-but-parseable
        # pricing record is named, and the pricing-consuming
        # recomputations below (provenance, recorded outcome,
        # report verdict) are skipped for it — the shape defect
        # is the package's defect, already named, and no verdict
        # is recomputed on records that cannot pair.
        pricing_shape = _pricing_shape_problems(probe_records)
        if pricing_shape:
            problems.extend(pricing_shape)
        else:
            problems.extend(
                _pricing_provenance_problems(meta, probe_records))
    # Claim ledger: the package's ledger must agree with its own
    # records, with the decisions its run.json records, and with
    # the configuration hash its run.json records.
    problems.extend(_claim_ledger_problems(run_dir, meta, records))
    # Record consistency: a stored package's records must agree
    # with their own observed fields (report.record_consistency_
    # losses — the same discipline campaign_outcome applies), so
    # a package whose "ok" records carry a contradicting
    # exit_code or survivor field, a missing elapsed_ms, or no
    # tree identity on a kill record is flagged here as well as
    # rendered incomplete.
    for loss in report_mod.record_consistency_losses(
            records, _package_expected_output(meta),
            _package_plan_config(meta)):
        problems.append(f"observations.jsonl {loss}")
    # Recorded outcome: a finalised package's run.json carries
    # the outcome its run computed (campaign_status and the two
    # counts beside it — the fields run.ps1 reads its campaign
    # verdict from). Those fields are part of the package's
    # evidence, so they must agree with the outcome the package's
    # own records recompute to; a manifest that claims a status
    # or counts its records do not earn is a package defect here,
    # not a verdict anything downstream may act on.
    if not pricing_shape:
        problems.extend(_outcome_problems(meta, records, probe_records))
        problems.extend(_report_status_problems(run_dir, meta, records,
                                                probe_records))
        problems.extend(_report_verdict_problems(
            run_dir, meta, records, probe_records))
    problems.extend(_report_body_problems(run_dir, meta))
    return problems


def _outcome_problems(meta, records, probe_records):
    """Verify a package's recorded campaign outcome against the
    outcome its own records recompute to (campaign_outcome over
    the records, the recorded decisions, the recorded identity
    mismatches, the verified pricing evidence and the recorded
    reduced shape). Only finalised packages are held to this:
    cmd_run writes the standing-loss and freeze-gate FAIL counts
    (both non-None) only at finalisation, beside campaign_status,
    so a package carrying both counts is claiming the outcome
    they record — and a package claiming campaign_status
    "complete" without them is claiming a verdict no finalised
    run records that way. Provisional packages record no counts
    and must record the incomplete status; packages carrying
    neither counts nor a complete claim (minimal fixtures, and
    packages predating the outcome fields) carry no recorded
    outcome to verify."""
    problems = []
    status = meta.get("campaign_status")
    if meta.get("provisional"):
        if status is not None \
                and status != CAMPAIGN_STATUS_INCOMPLETE:
            problems.append(
                f"run.json is provisional but records "
                f"campaign_status {status!r}; a stopped campaign "
                "records the incomplete status, never a verdict")
        return problems
    standing = meta.get("standing_loss_count")
    gate_fails = meta.get("freeze_gate_fail_count")
    if status == CAMPAIGN_STATUS_COMPLETE \
            and (standing is None or gate_fails is None):
        problems.append(
            "run.json records campaign_status 'complete' without "
            "the finalised outcome counts (standing_loss_count "
            "and freeze_gate_fail_count) a finished run records "
            "beside it; the recorded outcome is unverifiable")
        return problems
    if standing is None or gate_fails is None:
        return problems
    pricing, _ignored = _verified_pricing_evidence(meta, probe_records)
    # The identity losses the recomputation rests on are the ones
    # the package's own recorded fingerprints and versions earn
    # against its recorded configuration's pins, when that
    # configuration is verifiable (see
    # _recomputed_identity_mismatches) — never merely the list
    # the manifest claims, which a forged manifest could empty.
    # Round-25 P1: when the anchor is absent there is no
    # fallback to the recorded list at all; the recomputation
    # rests on no identity losses and the degradation is named
    # by _recorded_run_field_problems.
    identity = _recomputed_identity_mismatches(meta)
    outcome = campaign_outcome(
        records, meta.get("plan_counts") or check_plan(build_plan()),
        identity_losses=None if meta.get("reduced") else identity,
        pricing=pricing, decisions=_recorded_decisions(meta),
        reduced=bool(meta.get("reduced")),
        expected_output=_package_expected_output(meta),
        config=_package_plan_config(meta))
    recomputed = {
        "campaign_status": outcome["campaign_status"],
        "standing_loss_count": outcome["standing_loss_count"],
        "freeze_gate_fail_count": outcome["freeze_gate_fail_count"],
    }
    recorded = {
        "campaign_status": status,
        "standing_loss_count": standing,
        "freeze_gate_fail_count": gate_fails,
    }
    for field in ("campaign_status", "standing_loss_count",
                  "freeze_gate_fail_count"):
        if recorded[field] != recomputed[field]:
            problems.append(
                f"run.json records {field} {recorded[field]!r} but "
                f"the package's own records recompute it as "
                f"{recomputed[field]!r}; the recorded campaign "
                "outcome disagrees with the package's evidence")
    return problems


def _recorded_decisions(meta):
    """The profile-dimension decisions run.json records for the
    run — the decisions of the configuration whose SHA-256
    run.json also records — or {} when the manifest records none."""
    decisions = meta.get("profile_dimension_decisions")
    return dict(decisions) if isinstance(decisions, dict) else {}


def _claim_ledger_problems(run_dir, meta, records):
    """Verify a run package's claim ledger (claim-ledger.json)
    against its records, its recorded configuration decisions and
    its recorded configuration hash. A ledger claiming a decision
    the recorded configuration does not carry, or a state the
    records contradict, is a package defect, named per dimension.
    A package whose run.json records no ledger and which holds no
    ledger file predates the ledger (or is provisional): there is
    no ledger to verify and its absence is not faulted here."""
    problems = []
    ledger_path = os.path.join(run_dir, "claim-ledger.json")
    embedded = meta.get("claim_ledger")
    if not os.path.isfile(ledger_path):
        if embedded is not None:
            problems.append(
                "run.json records a claim ledger but "
                "claim-ledger.json is missing from the run package")
        return problems
    try:
        with open(ledger_path, encoding="utf-8") as handle:
            data = json.load(handle)
    except (OSError, ValueError) as error:
        return [f"claim-ledger.json unreadable: {error}"]
    claims = data.get("claims") if isinstance(data, dict) else None
    if not isinstance(claims, list) or not all(
            isinstance(c, dict) for c in claims):
        return ["claim-ledger.json holds no list of claim objects"]
    if data.get("config_sha256") != meta.get("config_sha256"):
        problems.append(
            "claim ledger's config_sha256 does not match the "
            "configuration hash run.json records; the ledger was "
            "not made from this run's configuration")
    dimensions = [c.get("dimension") for c in claims]
    if not all(isinstance(dimension, str) for dimension in dimensions):
        problems.append(
            "claim ledger carries a claim whose dimension is "
            f"not a string: {dimensions!r}; every claim must "
            "name one of the evaluation-profile dimensions")
        return problems
    if sorted(dimensions) != sorted(report_mod.PROFILE_DIMENSIONS) \
            or len(set(dimensions)) != len(dimensions):
        problems.append(
            "claim ledger must carry exactly one claim per "
            "evaluation-profile dimension; it carries "
            f"{dimensions}")
    decisions = _recorded_decisions(meta)
    # When the configuration file the run names is still present
    # and hashes to the value run.json records, its decisions must
    # be the recorded ones: the recorded decisions are the run's,
    # not an after-the-fact edit's.
    config_path = meta.get("config_path")
    recorded_sha = meta.get("config_sha256")
    if isinstance(config_path, str) \
            and isinstance(recorded_sha, str) \
            and config_path and recorded_sha \
            and os.path.isfile(config_path):
        try:
            with open(config_path, "rb") as handle:
                actual_sha = hashlib.sha256(handle.read()).hexdigest()
            if actual_sha == recorded_sha:
                with open(config_path, encoding="utf-8") as handle:
                    file_decisions = report_mod.decisions_from_config(
                        json.load(handle))
                if file_decisions != decisions:
                    problems.append(
                        "run.json's recorded profile-dimension "
                        "decisions do not match the decisions in "
                        "the configuration whose SHA-256 run.json "
                        "records")
        except (OSError, ValueError):
            pass
    # The verification SOURCE for decisions is the anchored
    # configuration, never the package under check (round-25
    # P1): when the configuration the manifest names is present
    # and hashes to the recorded SHA-256, the expected ledger
    # is computed from ITS decisions. When the anchor is
    # absent, the recorded decisions serve only the package's
    # internal consistency (the ledger file against the
    # manifest's own recorded claim), and
    # _recorded_run_field_problems names profile decisions
    # unverifiable — the recorded mapping is never presented
    # as a verified source.
    anchored = _anchored_config(meta)
    if anchored is not None:
        decisions = report_mod.decisions_from_config(anchored)
    expected = {entry["dimension"]: entry
                for entry in report_mod.claim_ledger(
                    records, decisions, _package_expected_output(meta),
                    _package_plan_config(meta))}
    for claim in claims:
        dimension = claim.get("dimension")
        wanted = expected.get(dimension)
        if wanted is None:
            problems.append(
                f"claim ledger names a dimension the evaluation "
                f"profile does not hold: {dimension!r}")
            continue
        if claim.get("state") == report_mod.LEDGER_NOT_APPLICABLE:
            reason = report_mod.decision_reason(dimension, decisions)
            if reason is None or claim.get("reason") != reason:
                problems.append(
                    f"claim ledger: dimension '{dimension}' claims "
                    "NOT-APPLICABLE, but the configuration recorded "
                    "in run.json carries no decision with that "
                    "reason for it; a ledger decision the "
                    "configuration does not carry is a package "
                    "defect")
                continue
        if claim.get("basis") != wanted["basis"]:
            problems.append(
                f"claim ledger: dimension '{dimension}' carries "
                f"basis {claim.get('basis')!r} but the basis the "
                f"records and the recorded configuration "
                f"decisions give is {wanted['basis']!r}; the "
                "ledger's stated basis disagrees with the "
                "package's evidence")
        if claim.get("state") != wanted["state"]:
            problems.append(
                f"claim ledger: dimension '{dimension}' claims "
                f"state {claim.get('state')!r} but the records and "
                f"the recorded configuration decisions give "
                f"{wanted['state']!r}; the ledger contradicts the "
                "package's evidence")
    if embedded is not None and embedded != claims:
        problems.append(
            "claim-ledger.json does not match the claim ledger "
            "recorded in run.json")
    return problems


def _pricing_paired_counts(probe_records):
    """Complete ok pair counts per pricing item, keyed by item:
    the reps for which both the loaded and the baseline shape of
    the item's probe ran ok."""
    counts = {}
    for probe, item in pricing_mod.PROBES.items():
        def paired_reps(shape):
            # Only an integer rep can pair (the shape the probes
            # write); any other rep — a list or mapping is
            # unhashable — forms no pair and must never raise
            # out of this counting.
            return {r.get("rep") for r in probe_records
                    if isinstance(r, dict)
                    and r.get("probe") == probe
                    and r.get("shape") == shape
                    and r.get("outcome") == "ok"
                    and isinstance(r.get("rep"), int)
                    and not isinstance(r.get("rep"), bool)}
        counts[item] = len(paired_reps("loaded")
                           & paired_reps("baseline"))
    return counts


def _pricing_provenance(meta, probe_records):
    """Verify pricing provenance for a preserved namespace against
    its run.json (P9). Returns (global_problems, item_problems):
    global_problems are defects that taint every price in the file
    — the records' guard fingerprint differs from run.json's
    recorded guard fingerprint, or the record count differs from
    run.json's recorded pricing_probe_observations count;
    item_problems maps an item to its defect detail when the
    records behind its price are not the records its run shape
    writes. The run shape is an exact anchor, not a floor:
    run_pricing_probes writes exactly two records per repetition
    (loaded and baseline) for every probe, at PROBE_REPS
    repetitions (REDUCED_PROBE_REPS for a reduced run, as
    recorded in run.json) — so a probe carrying MORE records
    than that (extra complete pairs the run never made, or an
    extra unpaired record) is as unverifiable as one carrying
    fewer, and a paired count above the shape's repetitions
    prices a price over records the run never made. A namespace
    with no pricing records and none recorded has nothing to
    verify."""
    global_problems = []
    item_problems = {}
    recorded_count = meta.get("pricing_probe_observations")
    if not probe_records:
        if recorded_count:
            global_problems.append(
                f"run.json records {recorded_count} pricing-probe "
                "observation(s) but pricing-observations.jsonl "
                "holds none; pricing provenance unverified")
        return global_problems, item_problems
    recorded_sha = _recorded_guard_sha(meta)
    record_shas = {r.get("guard_fingerprint", {}).get("sha256")
                   for r in probe_records
                   if isinstance(r, dict)
                   and isinstance(r.get("guard_fingerprint"), dict)
                   and isinstance(
                       r.get("guard_fingerprint", {}).get("sha256"),
                       str)}
    if recorded_sha is None or record_shas != {recorded_sha}:
        global_problems.append(
            "pricing-observations.jsonl guard fingerprint does not "
            "match run.json's recorded guard fingerprint; pricing "
            "provenance unverified")
    # The fingerprint's companion values (round-25): a pricing
    # record that carries a path or a size beside its sha256
    # claims the vendored guard binary at that path and size;
    # each carried companion must equal the run's recorded
    # guard fingerprint's own value — the hold the manifest's
    # fingerprint and the report's fingerprint lines have
    # carried since rounds 23/24, extended to the pricing
    # records' own copies. A record carrying neither companion
    # (the tests' sha-only fixture shape) claims no beside
    # value and is held by the sha256 comparison above.
    recorded_guard = None
    fingerprints_meta = meta.get("fingerprints")
    if isinstance(fingerprints_meta, dict) \
            and isinstance(fingerprints_meta.get("guard"), dict):
        recorded_guard = fingerprints_meta["guard"]
    if recorded_guard is not None:
        companion_mismatch = None
        for index, record in enumerate(probe_records):
            if not isinstance(record, dict):
                continue
            record_fp = record.get("guard_fingerprint")
            if not isinstance(record_fp, dict):
                continue
            for field in ("path", "size"):
                if field in record_fp \
                        and record_fp.get(field) \
                        != recorded_guard.get(field):
                    companion_mismatch = (index, field, record_fp)
                    break
            if companion_mismatch is not None:
                break
        if companion_mismatch is not None:
            index, field, record_fp = companion_mismatch
            global_problems.append(
                f"pricing-observations.jsonl record "
                f"{index + 1} guard fingerprint {field} is "
                f"{record_fp.get(field)!r} but run.json records "
                f"the guard fingerprint's {field} as "
                f"{recorded_guard.get(field)!r}; the pricing "
                "records' fingerprint companion values disagree "
                "with the run's recorded guard fingerprint, so "
                "pricing provenance is unverified")
    if recorded_count is not None and recorded_count != len(probe_records):
        global_problems.append(
            f"pricing-observations.jsonl holds {len(probe_records)} "
            f"record(s) but run.json records "
            f"pricing_probe_observations={recorded_count}; pricing "
            "provenance unverified")
    expected_reps = (pricing_mod.REDUCED_PROBE_REPS
                     if meta.get("reduced") else pricing_mod.PROBE_REPS)
    paired_counts = _pricing_paired_counts(probe_records)
    for probe, item in pricing_mod.PROBES.items():
        total = sum(1 for record in probe_records
                    if isinstance(record, dict)
                    and record.get("probe") == probe)
        if total != 2 * expected_reps:
            item_problems[item] = (
                f"has {total} pricing record(s) for probe "
                f"{probe}, but this run shape records exactly "
                f"{2 * expected_reps} ({expected_reps} paired "
                "repetition(s), two shapes each); the records "
                "are not the records this run shape writes, so "
                "pricing provenance is unverified for that item")
            continue
        paired = paired_counts[item]
        if paired < expected_reps:
            item_problems[item] = (
                f"has {paired} paired observation(s), below the "
                f"{expected_reps} paired repetition(s) this run "
                "shape records; pricing provenance unverified "
                "for that item")
    return global_problems, item_problems


def _pricing_stamp_problems(probe_records, *, platform=None):
    """The probe-shape stamp defects of a namespace's pricing
    records as problem strings, in the shape check_run_package
    reports package defects: each names the item and the field
    whose stamp does not carry the construction the item's price
    claims (pricing.pricing_stamp_defects)."""
    problems = []
    defects = pricing_mod.pricing_stamp_defects(probe_records, platform=platform)
    for item in sorted(defects):
        for defect in defects[item]:
            problems.append(
                f"pricing item '{item}' stamp defect: "
                f"{defect['shape']} record rep {defect['rep']} "
                f"field '{defect['field']}' is "
                f"{defect['actual']!r}, expected "
                f"{defect['expected']!r}; the record does not "
                "carry the construction the price claims, so "
                "pricing provenance is unverified for that item")
    return problems


def _pricing_provenance_problems(meta, probe_records):
    """The provenance defects of a namespace as problem strings,
    in the shape check_run_package reports package defects:
    the global defects and short paired counts from
    _pricing_provenance, plus every probe-shape stamp defect
    (see _pricing_stamp_problems)."""
    global_problems, item_problems = _pricing_provenance(meta, probe_records)
    problems = list(global_problems)
    for item, detail in sorted(item_problems.items()):
        problems.append(f"pricing item '{item}' {detail}")
    problems.extend(_pricing_stamp_problems(
        probe_records, platform=meta.get("platform")))
    return problems


def _verified_pricing_evidence(meta, probe_records):
    """Pricing evidence a report rendered from a preserved
    namespace may use, with its provenance problems. A global
    provenance failure (fingerprint or record count) prices
    nothing: every item renders "not measured". An item whose
    paired count is short of the run shape's expected reps is
    dropped individually; the rest still price. An item whose
    records fail the probe-shape stamps is likewise dropped
    individually — prices_from_observations already withholds
    it, and it is popped here as well so the discipline holds
    at this layer too — and its stamp defect is named in the
    problems. Returns (evidence_or_None, problems)."""
    problems = _pricing_provenance_problems(meta, probe_records)
    if not probe_records:
        return None, problems
    global_problems, item_problems = _pricing_provenance(meta, probe_records)
    if global_problems:
        return None, problems
    platform = meta.get("platform")
    evidence = pricing_mod.prices_from_observations(
        probe_records, platform=platform)
    if evidence is None:
        return None, problems
    for item in item_problems:
        evidence["items"].pop(item, None)
    for item in pricing_mod.pricing_stamp_defects(probe_records, platform=platform):
        evidence["items"].pop(item, None)
    if not evidence["items"]:
        return None, problems
    return evidence, problems


def _read_pricing_records(run_dir):
    """The run's pricing-probe observation records, or [] when the
    pricing file is absent."""
    records = []
    path = os.path.join(run_dir, "pricing-observations.jsonl")
    if not os.path.isfile(path):
        return records
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                records.append(json.loads(line))
    return records


def _pricing_for_render(config, probe_records):
    """Probe-derived pricing evidence when the probes priced at
    least one item; otherwise whatever the configuration carries —
    which decompose reports as operator-supplied, not measured.
    Configuration pricing is operator input: if it arrives shaped
    like probe evidence (the kind tag, a guard fingerprint), those
    claim keys are stripped before it is handed on, so a number
    typed into the configuration can never be reported as
    measured probe evidence — only prices_from_observations,
    built from recorded probe observations, may carry the tag."""
    evidence = pricing_mod.prices_from_observations(
        probe_records, platform=sys.platform)
    if evidence is not None:
        return evidence
    pricing = config.get("pricing")
    if isinstance(pricing, dict) and (
            pricing.get("kind") == "pricing-probe-evidence"
            or pricing.get("guard_fingerprint")):
        pricing = {key: value for key, value in pricing.items()
                   if key not in ("kind", "guard_fingerprint")}
    return pricing


def cmd_run(args):
    try:
        config = load_config(args.config)
    except ValueError as error:
        # A configuration that does not parse, or whose root or
        # arms / children containers are not objects, is refused
        # by name in the admission idiom, before any namespace
        # exists — never a traceback out of the loader.
        print(f"campaign admission failure: {error}",
              file=sys.stderr)
        print("campaign refused at pre-flight admission; no "
              "slot executed and no observations were written",
              file=sys.stderr)
        return EXIT_ADMISSION_FAILURE
    config["_config_path"] = os.path.abspath(args.config)
    with open(args.config, "rb") as handle:
        config_sha256 = hashlib.sha256(handle.read()).hexdigest()
    seed = args.seed
    if seed is None:
        seed = config.get("seed", DEFAULT_SEED)
    slots = build_plan(seed)
    counts = check_plan(slots)
    reduced = args.reduced is not None
    if reduced:
        slots = slots[: args.reduced]
    # Pre-flight admission, certifying shape only: before the
    # namespace exists and before any slot executes. A refusal
    # names every failure and writes no observations.
    if not reduced:
        failures = admission_failures(config)
        if failures:
            for failure in failures:
                print(f"campaign admission failure: {failure}",
                      file=sys.stderr)
            print("campaign refused at pre-flight admission; no "
                  "slot executed and no observations were written",
                  file=sys.stderr)
            return EXIT_ADMISSION_FAILURE
    run_id = args.run_id or new_run_id(seed)
    run_dir = namespace_run_dir(args.out, run_id)
    config["_windows_slot_jobs"] = sys.platform == "win32" and not reduced
    config["_processkit_event_dir"] = os.path.join(run_dir, "processkit-events")
    env = environment_record()
    decisions = report_mod.decisions_from_config(config)
    meta = {
        "run_id": os.path.basename(run_dir),
        "plan_counts": counts, "reduced": reduced,
        "slots_run": 0,
        "slots_planned": len(slots),
        "seed": seed,
        "order": [slot_key(s) for s in slots],
        "environment": env,
        "platform": env["platform"],
        "python": env["python"],
        "config_sha256": config_sha256,
        "config_path": os.path.abspath(args.config),
        "slot_isolation": (WINDOWS_SLOT_ISOLATION
                           if config["_windows_slot_jobs"] else None),
        "profile_dimension_decisions": decisions,
        "fingerprints": {},
        "arm_versions": {},
        "competitor_identity_mismatches": [],
        "pricing_probe_observations": 0,
        "provisional": True,
        "campaign_status": CAMPAIGN_STATUS_INCOMPLETE,
        "standing_loss_count": None,
        "freeze_gate_fail_count": None,
        "config_templates": {
            arm: config.get("arms", {}).get(arm, {}).get("templates")
            for arm in PRODUCT_ARMS
        },
    }
    # The manifest is written BEFORE the first slot executes and
    # updated as the run proceeds, so a stopped campaign's
    # namespace resolves and reports its partial state.
    _write_run_json(run_dir, meta)
    records = []
    obs_path = os.path.join(run_dir, "observations.jsonl")
    with open(obs_path, "w", encoding="utf-8") as sink:
        for slot in slots:
            record = run_slot(slot, config)
            records.append(record)
            sink.write(json.dumps(record) + "\n")
            sink.flush()
            meta["slots_run"] = len(records)
            _write_run_json(run_dir, meta, indent=None)
            if config["_windows_slot_jobs"]:
                isolation = record.get("slot_job", {})
                if not _slot_job_drained(isolation):
                    print(f"slot Job did not prove cleanup: {isolation}",
                          file=sys.stderr)
                    return EXIT_SLOT_CLEANUP_FAILURE
    fps = fingerprints(config)
    versions = arm_versions(config)
    mismatches = identity_mismatches(config, fps, versions)
    guard_fp = fps.get("guard", {})
    probe_records = pricing_mod.run_pricing_probes(config, guard_fp,
                                                   reduced=reduced)
    with open(os.path.join(run_dir, "pricing-observations.jsonl"),
              "w", encoding="utf-8") as sink:
        for record in probe_records:
            sink.write(json.dumps(record) + "\n")
    pricing = _pricing_for_render(config, probe_records)
    expected_output = report_mod.expected_output_from_config(config)
    outcome = campaign_outcome(
        records, counts,
        identity_losses=None if reduced else mismatches,
        pricing=pricing, decisions=decisions, reduced=reduced,
        expected_output=expected_output, config=config)
    # The claim ledger is part of the run package: written as its
    # own file in the namespace and mirrored in run.json, both
    # under the configuration hash the run records.
    with open(os.path.join(run_dir, "claim-ledger.json"), "w",
              encoding="utf-8") as sink:
        json.dump({"config_sha256": config_sha256,
                   "claims": outcome["claim_ledger"]}, sink, indent=2)
        sink.write("\n")
    meta.update({
        "slots_run": len(records),
        "fingerprints": fps,
        "arm_versions": versions,
        "competitor_identity_mismatches": mismatches,
        "pricing_probe_observations": len(probe_records),
        "provisional": False,
        "campaign_status": outcome["campaign_status"],
        "standing_loss_count": outcome["standing_loss_count"],
        "freeze_gate_fail_count": outcome["freeze_gate_fail_count"],
        "claim_ledger": outcome["claim_ledger"],
    })
    _write_run_json(run_dir, meta)
    text = report_mod.render(
        records, counts, fps,
        pricing=pricing,
        platform=env["platform"], arm_versions=versions,
        identity_losses=None if reduced else mismatches,
        decisions=decisions, reduced=reduced,
        expected_output=expected_output, config=config)
    if reduced:
        text = "REDUCED LOGIC EXERCISE — not a campaign, not a result.\n\n" + text
    with open(os.path.join(run_dir, "report.txt"), "w", encoding="utf-8") as sink:
        sink.write(text)
    print(text)
    print(f"campaign status: {outcome['campaign_status']} "
          f"({outcome['standing_loss_count']} standing loss(es), "
          f"{outcome['freeze_gate_fail_count']} freeze-gate FAIL(s))")
    print(f"run directory: {run_dir}")
    return 0


def _is_run_namespace(path):
    """A directory is a run namespace when it holds a manifest
    (run.json, provisional or finalised) or observations — a
    stopped campaign may have either one without the other."""
    return os.path.isdir(path) and (
        os.path.isfile(os.path.join(path, "run.json"))
        or os.path.isfile(os.path.join(path, "observations.jsonl")))


def resolve_run_dir(out):
    """A run directory is used as given; a parent directory resolves
    to its newest run namespace inside it. A directory with neither
    a manifest nor observations is not a run package."""
    if _is_run_namespace(out):
        return out
    candidates = []
    if os.path.isdir(out):
        for name in os.listdir(out):
            path = os.path.join(out, name)
            if _is_run_namespace(path):
                candidates.append(path)
    if not candidates:
        raise SystemExit(
            f"no run package found in {out}: no run.json manifest "
            "and no observations.jsonl in it or its namespaces")
    # "Newest" is a property of the namespaces' evidence, not of
    # their names: a run made under a caller-chosen --run-id whose
    # name sorts early (or a copied-in older namespace whose name
    # sorts late) must not shadow the run that actually finished
    # last. Order by the manifest's modification time (the
    # namespace directory's, for a manifest-less namespace), with
    # the name as a deterministic tie-break.
    def newest_key(path):
        manifest = os.path.join(path, "run.json")
        try:
            stamp = os.path.getmtime(manifest)
        except OSError:
            stamp = os.path.getmtime(path)
        return (stamp, path)
    return max(candidates, key=newest_key)


def _reconstructed_meta(records):
    """A minimal manifest for a namespace that holds observations
    but no run.json: the configured plan's counts, no fingerprints, no
    recorded platform. Completeness cannot be certified from
    observations alone, so the caller adds that as a standing
    loss; the render is INCOMPLETE with exact slot accounting."""
    return {
        "run_id": "reconstructed-from-observations",
        "plan_counts": check_plan(build_plan()),
        "reduced": False,
        "slots_run": len(records),
        "seed": None,
        "order": None,
        "environment": {},
        "platform": "",
        "python": "",
        "fingerprints": {},
        "arm_versions": {},
        "competitor_identity_mismatches": [],
        "profile_dimension_decisions": {},
        "provisional": False,
        "campaign_status": CAMPAIGN_STATUS_INCOMPLETE,
    }


def _manifest_render_problems(meta):
    """Shape defects of the manifest fields cmd_report renders
    from, named per field. cmd_run writes plan_counts and
    fingerprints as objects (each fingerprint an object),
    platform as a string, arm_versions as an object and order
    as a list (or null, in a reconstructed manifest). The render
    indexes those fields directly — meta["plan_counts"],
    fingerprints.get(...) per arm, each fingerprint's own
    .get(...) — so a manifest missing one, or carrying it in
    another container's shape, raised KeyError / AttributeError
    out of the report instead of being named, and a provisional
    manifest whose order is not a list raised TypeError in the
    stopped-campaign message's length. check_run_package
    tolerates several of these shapes (its recomputations fall
    back where the render cannot), so the report path names
    them itself, in the same refusal idiom as the non-object
    manifest and the malformed records."""
    problems = []
    if not isinstance(meta.get("plan_counts"), dict):
        problems.append(
            f"field 'plan_counts' is {meta.get('plan_counts')!r}; "
            "a run manifest must carry its recorded plan counts "
            "as an object")
    fingerprints = meta.get("fingerprints")
    if not isinstance(fingerprints, dict):
        problems.append(
            f"field 'fingerprints' is {fingerprints!r}; a run "
            "manifest must carry its arm fingerprints as an "
            "object")
    else:
        for arm, entry in fingerprints.items():
            if not isinstance(entry, dict):
                problems.append(
                    f"field 'fingerprints.{arm}' is {entry!r}; "
                    "each arm's fingerprint must be an object "
                    "carrying its path, sha256 and size")
    if not isinstance(meta.get("platform"), str):
        problems.append(
            f"field 'platform' is {meta.get('platform')!r}; "
            "a run manifest must carry its platform as a string")
    arm_versions = meta.get("arm_versions")
    if arm_versions is not None \
            and not isinstance(arm_versions, dict):
        problems.append(
            f"field 'arm_versions' is {arm_versions!r}; a run "
            "manifest must carry its probed arm versions as an "
            "object")
    order = meta.get("order")
    if order is not None and not isinstance(order, list):
        problems.append(
            f"field 'order' is {order!r}; a run manifest must "
            "carry its recorded order as a list (or null, when "
            "the manifest records none)")
    return problems


def cmd_report(args):
    run_dir = resolve_run_dir(args.out)
    # The package's three reads are guarded exactly as
    # check_run_package guards them: a torn file — the stopped
    # campaign's package is precisely what this command exists
    # to render, and cmd_run rewrites run.json in place and
    # appends to the record files as it goes — is refused with
    # the defect named (the file and the parse error), never
    # raised as a raw traceback that renders nothing.
    try:
        records = _read_records(run_dir)
    except (OSError, ValueError) as error:
        raise SystemExit(
            f"observations.jsonl in {run_dir} is unreadable: "
            f"{error}; the package's records cannot be rendered "
            "from a torn file")
    manifest_path = os.path.join(run_dir, "run.json")
    extra_losses = []
    if os.path.isfile(manifest_path):
        try:
            with open(manifest_path, encoding="utf-8") as handle:
                meta = json.load(handle)
        except (OSError, ValueError) as error:
            raise SystemExit(
                f"run.json in {run_dir} is unreadable: {error}; "
                "the package's manifest cannot be rendered from "
                "a torn file")
        if not isinstance(meta, dict):
            raise SystemExit(
                f"run.json in {run_dir} is not a JSON object; "
                "a run manifest must carry the fields a report "
                "is rendered from")
        manifest_problems = _manifest_render_problems(meta)
        if manifest_problems:
            raise SystemExit(
                f"run.json in {run_dir} holds manifest fields a "
                "report cannot be rendered from: "
                + "; ".join(manifest_problems))
    else:
        if not records:
            raise SystemExit(
                f"no run package found in {run_dir}: no run.json "
                "manifest and no observations to report")
        meta = _reconstructed_meta(records)
        extra_losses.append(
            "run.json manifest missing: the campaign's completeness "
            "cannot be certified from observations alone")
    extra_losses.extend(_slot_isolation_problems(meta, records))
    if meta.get("provisional"):
        extra_losses.append(
            f"campaign stopped before completion: run.json is "
            f"provisional with {meta.get('slots_run', len(records))} "
            f"of {len(meta.get('order') or [])} planned slot(s) "
            "recorded; the records present are a prefix of the "
            "seeded order and every missing cell is a standing loss")
    try:
        probe_records = _read_pricing_records(run_dir)
    except (OSError, ValueError) as error:
        raise SystemExit(
            f"pricing-observations.jsonl in {run_dir} is "
            f"unreadable: {error}; the package's pricing records "
            "cannot be rendered from a torn file")
    # Record shape, before any rendering recomputation: the
    # render path (report.py's medians, slot accounting and
    # freeze gate) indexes the fields the runner writes, and
    # check_run_package names a malformed-but-parseable record
    # as a package defect — a report must not instead raise on
    # the same record and hide the package behind a traceback.
    # Like the non-object manifest above, a malformed package is
    # refused here with the defects named (the checker's own
    # shape verdicts, verbatim), never rendered.
    shape_problems = _observation_shape_problems(records) \
        + _pricing_shape_problems(probe_records)
    if shape_problems:
        raise SystemExit(
            f"run package in {run_dir} holds records a report "
            "cannot be rendered from: "
            + "; ".join(shape_problems))
    # Pricing provenance (P9): prices rebuilt from a preserved
    # namespace are believed only when the records are the ones
    # run.json records — guard fingerprint, record count, and the
    # paired count per item for this run shape. Any mismatch
    # renders every affected item "not measured" (and
    # check_run_package flags the namespace).
    pricing, _pricing_problems = _verified_pricing_evidence(
        meta, probe_records)
    mismatches = _recorded_identity_losses(meta)
    identity_losses = None if meta.get("reduced") else mismatches
    combined_losses = list(identity_losses or []) + extra_losses
    text = report_mod.render(
        records, meta["plan_counts"], meta["fingerprints"],
        pricing=pricing, platform=meta["platform"],
        arm_versions=meta.get("arm_versions"),
        identity_losses=combined_losses or None,
        decisions=_recorded_decisions(meta),
        reduced=bool(meta.get("reduced")),
        expected_output=_package_expected_output(meta),
        config=_package_plan_config(meta))
    if meta.get("reduced"):
        text = "REDUCED LOGIC EXERCISE — not a campaign, not a result.\n\n" + text
    print(text)
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description="process-tree-guard campaign harness")
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run")
    run.add_argument("--config", required=True)
    run.add_argument("--out", required=True)
    run.add_argument("--reduced", type=int, default=None,
                     help="logic exercise only: run the first N plan slots")
    run.add_argument("--seed", type=int, default=None,
                     help="slot-order seed (default: the config's seed)")
    run.add_argument("--run-id", default=None,
                     help="run namespace name (default: timestamp + seed)")
    run.set_defaults(func=cmd_run)
    rep = sub.add_parser("report")
    rep.add_argument("--out", required=True)
    rep.set_defaults(func=cmd_report)
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    # A redirected Windows console can use a legacy code page. Keep the
    # saved UTF-8 report intact and escape characters the console cannot print.
    sys.stdout.reconfigure(errors="backslashreplace")
    sys.exit(main())
