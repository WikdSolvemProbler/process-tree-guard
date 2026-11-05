# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Pricing probes for the CO-7 decomposition's expensive class.

The decomposition may price only three items — console probe +
console finish, owner verification, resolver probe count — and only
from measurements this campaign itself made. This module is that
measurement. Each item has one probe: a pair of guard fast-path
shapes that differ only in the named item, run interleaved for a
fixed number of repetitions, with every raw run recorded as an
observation. The item's price is the median of the per-repetition
differences (loaded shape minus baseline shape):

- console probe + console finish: loaded = a guard fast run in the
  NO_WINDOW context (CREATE_NO_WINDOW gives the guard a console of
  its own, without a window: the probe opens the console devices
  and the finish pays its attached-console part — flush, process
  list, free — and walks from an attached console); baseline = the
  same run in the DETACHED context (DETACHED_PROCESS: no console
  is created for the guard and none is inherited, so the probe's
  console-device opens fail and the finish has no attached console
  to pay for; the guard never allocates a console of its own).
- owner verification: loaded = a guard fast run launched through
  exactly one intermediate launcher level — the recording
  launcher itself, spawned directly by the harness as the
  guard's parent — so the owner verification and the finish
  walk must examine that one extra non-boundary owner level;
  each such level costs the counted owner calls in the census.
  No second launcher wraps it: loaded − baseline is one inserted
  level by construction, and every owner observation records
  its inserted-level count (1 loaded, 0 baseline). Baseline =
  the same run launched directly. The loaded shape's recorded
  time is the guard process's own lifetime, not the launcher's
  outer lifetime (route (b); see the comment at
  GUARD_LAUNCHER_MODE below): the recording launcher records
  the guard child's PID and brackets only the guard process
  with monotonic timestamps, so the launcher's startup (even a
  500 ms sleep before it spawns the guard) and teardown never
  enter either shape.
- resolver probe count: loaded = a guard fast run whose child is
  named by its bare file name with the child's folder placed last
  in a controlled PATH behind RESOLVER_EMPTY_FOLDERS empty folders,
  so the resolver pays one filesystem probe per empty folder plus
  the hit; baseline = the same run with the child named by absolute
  path, which pays no search but does pay the hit's own single
  probe — the same probe the loaded shape pays at its hit folder,
  so it cancels and the pair differs by exactly
  RESOLVER_EMPTY_FOLDERS extra probes (the current-folder probe
  that once added a fifth is disabled by the controlled field
  below; probe_count records this extra count, baseline 0). The
  recorded price is the cost of the extra probes; the per-probe
  cost is carried in the trace.
  Both shapes control and record the whole field the resolver
  searches, not only PATH: the guard is spawned with an explicit
  working directory — a fresh, empty per-probe scratch directory,
  never the harness process's own folder, so a same-named program
  sitting in the harness's folder cannot be the hit — and with a
  child environment that pins PATHEXT to RESOLVER_PATHEXT and sets
  NoDefaultCurrentDirectoryInExePath, disabling the current-folder
  probe that would otherwise run ahead of the PATH walk. Every
  resolver observation records the working directory, the PATHEXT
  and the current-folder rule actually in force beside the
  intended PATH, so the field that was measured is stated in the
  evidence itself.
  The shapes' environments are built with the suite's shared
  case-insensitive setting rule (envcase.set_env_var): a parent
  environment spelling the variable `Path` — or carrying both
  spellings — must not leave a second, inherited entry in the
  child block ahead of the controlled one, or the probe would
  measure the inherited PATH instead of the laid-out field.

Every probe run is one timed lifetime under a finite deadline,
checked against the fast path's expected exit code: the outer
process's lifetime for every shape except owner-verification
loaded, whose timed lifetime is the guard process's own (above).
A probe run that fails to spawn, times out, or exits unexpectedly
is recorded as an infrastructure failure and contributes no pair:
an item whose pairs are incomplete is not priced, and the report
states it as not measured — never a number. A priced item also
needs at least the reduced shape's paired repetitions
(REDUCED_PROBE_REPS): fewer pairs is not a measurement, however
tidy the records look (provenance floor).

Provenance rule: prices reach the report only as a reference to
these recorded observations. prices_from_observations() builds the
evidence object report.decompose accepts: it names the probe, the
observation count behind each price, and the guard fingerprint the
probe runs measured. A bare mapping of item names to numbers —
an operator typing prices into the configuration — is not this
object and is never labelled measured; the report calls it
operator-supplied, not measured.

Stamp discipline: a price claims a difference between two
constructed shapes, so a priced item's records must carry the
construction stamps the probe itself stamps (see _shapes and
run_pricing_probes): the console pair's contexts (baseline
DETACHED, loaded NO_WINDOW), the owner pair's inserted_levels
(baseline 0, loaded 1), and the resolver pair's probe_count
(baseline 0, loaded RESOLVER_EMPTY_FOLDERS) together with the
pinned resolver field both resolver shapes are constructed under
(working_directory, pathext, current_folder_rule, and the loaded
shape's intended_path). The discipline holds the full stamp
vector run_pricing_probes stamps on every record, not only the
stamps each item's own argument names: every field a probe does
not construct for its records is stamped None there (the
resolver baseline's intended_path, and every other probe's
fields on a record of a different probe), so a non-null value
in such a field is a stamp its construction never writes and
is a defect of the same kind. The context stamp is cross-checked for
every probe pair, not only the console pair whose item is the
context difference: both owner-verification shapes and both
resolver shapes are likewise constructed under a held-constant
context (NO_WINDOW), so a record of either pair carrying another
context describes a pair that differs by a console context as
well as by the named item, and prices nothing. The resolver stamps are cross-checked
as the construction states them, not merely for presence: every
resolver record in a run shares the one working_directory the
probe constructs for the pair, and the loaded shape's
intended_path must be the laid-out field itself —
RESOLVER_EMPTY_FOLDERS empty folders followed by the child's
folder, that many entries and no other. pricing_stamp_defects()
cross-checks exactly those stamps — no other record field is a
stamp — and an item whose records fail any of them does not
price: it is absent from the evidence object, so the report
renders it "not measured", and the harness's package provenance
check names the defect by item and field. The records' own
classification is held beside the stamps (round-23): an "ok"
record whose exit_code differs from the fast path's expected
code (or was never observed) prices nothing and is named the
same way, each record's item field is held to the item its
probe prices, the loaded-only fields the timed run writes
only on the owner-verification loaded shape (guard_pid,
outer_elapsed_ms) are held null everywhere else, and every
record is held to exactly the field set its probe and shape
construct. On the owner loaded shape itself the two bracket
fields' values are held as well (round-24): guard_pid to the
PID form (the guard child's PID the recording launcher
records), and outer_elapsed_ms to a finite, non-negative
bracket that covers the record's own elapsed_ms — the
harness's bracket around the launcher, inside which the
launcher's bracket sits — on an "ok" record, and on a record
of another outcome wherever it carries a value (round-33
F6: a launcher that never ran writes neither field, and
such a record is not faulted for that absence). Integer
stamps are held to the integer type, not merely an equal
value, and the guard fingerprint's companion values (path
and size) are held as a pair — a fingerprint carrying
either is held on both (round-33 F6).

Uncertainty rule: a price is a distribution, not a point. The
evidence object carries, per item, the paired differences
themselves and their dispersion alongside the median: minimum,
maximum, first and third quartiles, interquartile range, and the
sign counts (how many differences are positive, negative, zero).
A median whose paired differences disagree in sign, or that is
itself negative for an item whose cost cannot be negative, is not
a measurement the report may print silently: report.decompose
flags it INCONSISTENCY (see report.py).

Tier: on this development machine the probes execute at stub tier
against the stub arm; their timings prove the plumbing only. The
Windows execution against the built guard is theory until run.
"""

import json
import math
import os
import statistics
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(
    os.path.dirname(os.path.abspath(__file__))))

from arms import expand_template, expected_exit, \
    materialized_child_args, materialized_list, resolve_template  # noqa: E402
from envcase import set_env_var  # noqa: E402
from runner import FAST_TIMEOUT_S, SLOT_DEADLINE_S, creation_flags  # noqa: E402

PROBE_REPS = 9            # paired repetitions per item, certifying shape
REDUCED_PROBE_REPS = 3    # paired repetitions in a reduced logic exercise
RESOLVER_EMPTY_FOLDERS = 4

# The resolver probe's controlled field, held constant across its
# paired shapes: the PATHEXT its child environment pins, and the
# current-folder rule in force (the variable that disables the
# resolver's current-folder probe, with the value that disables
# it). Every resolver observation records both, beside its working
# directory and intended PATH.
RESOLVER_PATHEXT = ".COM;.EXE;.BAT;.CMD"
NO_DEFAULT_CURRENT_DIRECTORY_VAR = "NoDefaultCurrentDirectoryInExePath"
NO_DEFAULT_CURRENT_DIRECTORY_VALUE = "1"
RESOLVER_CURRENT_FOLDER_RULE = (
    "current folder not probed "
    "(NoDefaultCurrentDirectoryInExePath=1)")

# Probe name -> the expensive-class item it prices.
PROBES = {
    "console-probe-finish": "console probe + console finish",
    "owner-verification": "owner verification",
    "resolver-probe-count": "resolver probe count",
}

# --- Route (b) for the owner-verification probe (P5) ---------------
# Why the guard process is timed itself, rather than the launcher's
# outer lifetime (the pre-fix shape) or a subtracted control pair
# (route (a)). Timing the launcher process prices the launcher's
# whole lifetime — its startup, its process creation of the guard,
# its waiting and its teardown — as "owner verification": a
# confound with no bound, proven at stub tier (a launcher that
# sleeps 500 ms before exec'ing the guard moved the reported owner
# price by ~471 ms, dollar for dollar). Route (a) — a control pair
# of the same launcher running the floor child directly, subtracted
# afterwards — removes that confound only in expectation: it prices
# two further process lifetimes per repetition and subtracts their
# noisy paired difference from another noisy paired difference,
# widening the very dispersion the evidence must report, and it
# makes every pricing file recorded without control records (the
# preserved reduced evidence included) unpriceable under the
# incomplete-pairs rule. Route (b) removes the confound by
# construction instead. The recording launcher below is spawned
# directly by the harness as the guard's direct parent, and it IS
# the loaded shape's one intermediate level: no configured
# launcher wraps it, so loaded − baseline is exactly one inserted
# ancestor level by construction (the earlier assembly wrapped
# the recording launcher in the configured launcher — cmd /c on
# Windows — and priced two inserted levels as one). It records
# the guard child's PID together with monotonic timestamps
# bracketing only the guard process: the start is taken just
# before the guard is created, the end just after it exits, both
# inside the recording launcher, whose own startup (interpreter
# and imports included) sits before the bracket and whose teardown
# sits after it. The harness takes that bracket as the loaded
# shape's elapsed time, so no launcher time enters either shape,
# no subtraction is estimated, and the probe's shapes, record
# counts and recorded format are unchanged. A launcher that
# refuses to run leaves no timing record at all: the harness
# records that as infrastructure failure, never as a price.
GUARD_LAUNCHER_MODE = "guard-launcher"


def _write_guard_record(path, payload):
    tmp = path + ".tmp"
    try:
        with open(tmp, "w", encoding="utf-8") as handle:
            json.dump(payload, handle)
        os.replace(tmp, path)
    except OSError:
        pass


def _read_guard_record(path):
    try:
        with open(path, encoding="utf-8") as handle:
            data = json.load(handle)
        return data if isinstance(data, dict) else None
    except (OSError, ValueError):
        return None


def _guard_launcher_main(argv):
    """The recording launcher: `pricing.py guard-launcher
    RECORD_PATH -- COMMAND...`. Spawns COMMAND (the guard run),
    records its PID and the monotonic bracket of its lifetime at
    RECORD_PATH, and exits with COMMAND's exit code."""
    if len(argv) < 3 or argv[1] != "--":
        return 97
    record_path, command = argv[0], argv[2:]
    start_ns = time.perf_counter_ns()
    try:
        proc = subprocess.Popen(command)
    except OSError as error:
        _write_guard_record(record_path, {
            "pid": None, "error": f"spawn failed: {error}"})
        return 97
    _write_guard_record(record_path,
                        {"pid": proc.pid, "start_ns": start_ns})
    exit_code = proc.wait()
    end_ns = time.perf_counter_ns()
    _write_guard_record(record_path, {
        "pid": proc.pid, "start_ns": start_ns, "end_ns": end_ns,
        "elapsed_ms": (end_ns - start_ns) / 1e6,
        "exit_code": exit_code})
    return exit_code


def _guard_argv(config, child=None, child_args=None,
                guard_record_path=None, guard_launcher=None):
    """The assembled argv for one guard fast-path run in one probe
    shape. With guard_record_path (owner-verification loaded
    shape), the recording launcher is the single prefix command —
    guard_launcher when a test supplies one, else this module in
    GUARD_LAUNCHER_MODE — so the assembled argv inserts exactly
    one process level between the harness and the guard; without
    it, the guard is the first program in the argv and the shape
    inserts no level."""
    exe = config.get("arms", {}).get("guard", {}).get("executable")
    if child is None:
        spec = config["children"]["fast"]
        # Round-43 F3: the spec's args are read through the
        # shared materialization (arms.materialized_child_args),
        # as the runner's construction and the checker's
        # derivation read them — a raw list() here consumed a
        # one-shot args value out from under both, so the
        # priced vector depended on which consumer read first.
        child, child_args = spec["program"], \
            materialized_child_args(spec)
    argv = [exe] + expand_template(
        resolve_template("guard", "fast", config),
        FAST_TIMEOUT_S, child, child_args)
    if guard_record_path is not None:
        # Round-44 F2: the prefix is the launcher's tokens
        # through the shared materialization, and its
        # emptiness is judged on those tokens — a
        # payload-carrying list subclass's empty list
        # storage read as "no launcher" here.
        launcher_tokens = materialized_list(guard_launcher) \
            if isinstance(guard_launcher, (list, tuple)) else None
        prefix = launcher_tokens if launcher_tokens else [
            sys.executable, os.path.abspath(__file__),
            GUARD_LAUNCHER_MODE]
        argv = prefix + [guard_record_path, "--"] + argv
    return argv


def _time_guard_run(config, context, child=None, child_args=None,
                    env=None, cwd=None, guard_record_path=None,
                    guard_launcher=None):
    """One timed guard fast-path run in one probe shape. Returns an
    observation dict; outcome is "ok" only when the run completed
    with the fast path's expected exit code.

    cwd, when given, is the working directory the guard child is
    spawned with (the resolver probe's controlled scratch folder);
    the spawn never silently inherits a different folder for a
    shape that names one.

    When guard_record_path is given (owner-verification loaded
    shape, route (b)), the guard run is wrapped in this module's
    recording launcher — the shape's single intermediate level,
    spawned directly by the harness — and the record's elapsed_ms
    is the guard process's own lifetime from the recording
    launcher's bracket — the outer lifetime is kept beside it as
    outer_elapsed_ms, and the guard child's PID as guard_pid. A
    run whose timing record never appears (a launcher that
    refused to run the command) is an infrastructure failure, not
    a price."""
    argv = _guard_argv(config, child=child, child_args=child_args,
                       guard_record_path=guard_record_path,
                       guard_launcher=guard_launcher)
    record = {"argv": argv, "context": context,
              "outcome": None, "elapsed_ms": None, "exit_code": None}
    start = time.perf_counter_ns()
    try:
        proc = subprocess.Popen(
            argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL,
            creationflags=creation_flags(context), env=env, cwd=cwd)
    except OSError as error:
        record["outcome"] = "infra-failure"
        record["detail"] = f"spawn failed: {error}"
        record["elapsed_ms"] = (time.perf_counter_ns() - start) / 1e6
        return record
    try:
        record["exit_code"] = proc.wait(timeout=SLOT_DEADLINE_S)
    except subprocess.TimeoutExpired:
        try:
            proc.kill()
            proc.wait(timeout=5.0)
        except (OSError, subprocess.TimeoutExpired):
            pass
        record["outcome"] = "infra-failure"
        record["detail"] = "probe run exceeded its finite deadline"
        record["elapsed_ms"] = (time.perf_counter_ns() - start) / 1e6
        return record
    outer_elapsed_ms = (time.perf_counter_ns() - start) / 1e6
    if guard_record_path is not None:
        record["outer_elapsed_ms"] = outer_elapsed_ms
        guard_record = _read_guard_record(guard_record_path)
        if guard_record is None \
                or guard_record.get("elapsed_ms") is None:
            record["outcome"] = "infra-failure"
            record["detail"] = (
                "guard-process timing record missing: the "
                "recording launcher — the loaded shape's one "
                "intermediate level — did not run the guard, so "
                "the guard's own lifetime was not measured")
            record["elapsed_ms"] = outer_elapsed_ms
            return record
        record["guard_pid"] = guard_record.get("pid")
        record["elapsed_ms"] = guard_record["elapsed_ms"]
        guard_exit = guard_record.get("exit_code", record["exit_code"])
        expected = expected_exit("guard", "fast", config)
        if expected is not None and guard_exit != expected:
            record["outcome"] = "exit-mismatch"
            record["detail"] = (
                f"exit code {guard_exit} != expected {expected} "
                "for guard/fast")
        else:
            record["outcome"] = "ok"
        return record
    record["elapsed_ms"] = outer_elapsed_ms
    expected = expected_exit("guard", "fast", config)
    if expected is not None and record["exit_code"] != expected:
        record["outcome"] = "exit-mismatch"
        record["detail"] = (f"exit code {record['exit_code']} != expected "
                            f"{expected} for guard/fast")
    else:
        record["outcome"] = "ok"
    return record


def _shapes(config, probe, scratch):
    """Yield (shape, kwargs) pairs for one probe: loaded first, then
    baseline. Keys that are not _time_guard_run parameters
    (probe_count, intended_path, inserted_levels, working_directory,
    pathext, current_folder_rule) are observation-record fields:
    run_pricing_probes pops them before the run and stamps them on
    the record, so the record states the construction the run was
    made under. scratch is a directory the resolver probe fills
    with empty PATH folders and its controlled working directory."""
    if probe == "console-probe-finish":
        yield "loaded", {"context": "NO_WINDOW"}
        yield "baseline", {"context": "DETACHED"}
    elif probe == "owner-verification":
        # The recording launcher is the loaded shape's single
        # intermediate level: it is spawned directly by the harness
        # (inside _time_guard_run's argv assembly, from
        # guard_record_path), and no configured launcher wraps it —
        # wrapping it (cmd /c on Windows) inserted a second level
        # the difference was then misreported as the price of one.
        # pricing_probes.guard_launcher may name a substitute
        # recording-launcher prefix (the stub tier's sleeping and
        # refusing controls); the configured pricing_probes.launcher
        # no longer takes part in this item.
        guard_launcher = (config.get("pricing_probes", {})
                          or {}).get("guard_launcher")
        # Round-44 F2: the shape check reads the tokens
        # through the shared materialization — a raw any()
        # over a payload-carrying list subclass drained
        # the launcher the loaded shape then carries.
        if guard_launcher is not None and (
                not isinstance(guard_launcher, list)
                or any(not isinstance(token, str)
                       for token in materialized_list(
                           guard_launcher))):
            raise ValueError(
                f"pricing_probes.guard_launcher is "
                f"{guard_launcher!r}, not a list of argv tokens; "
                "the recording launcher prefix must be a list of "
                "string tokens")
        loaded = {"context": "NO_WINDOW",
                  "guard_record_path": os.path.join(
                      scratch, "owner-guard-lifetime.json"),
                  "inserted_levels": 1}
        # Round-44 F2: presence is judged on the
        # materialized tokens, as in _guard_argv — a
        # payload-carrying launcher's empty list storage
        # is not "no launcher".
        launcher_present = bool(materialized_list(guard_launcher)) \
            if isinstance(guard_launcher, list) else bool(guard_launcher)
        if launcher_present:
            loaded["guard_launcher"] = guard_launcher
        yield "loaded", loaded
        yield "baseline", {"context": "NO_WINDOW", "inserted_levels": 0}
    elif probe == "resolver-probe-count":
        spec = config["children"]["fast"]
        child = spec["program"]
        child_dir = os.path.dirname(os.path.abspath(child))
        empties = []
        for index in range(RESOLVER_EMPTY_FOLDERS):
            folder = os.path.join(scratch, f"empty-{index}")
            os.makedirs(folder, exist_ok=True)
            empties.append(folder)
        # The controlled working directory: a fresh, empty folder
        # inside this probe's scratch, shared by both shapes. The
        # guard child is spawned there explicitly, never in the
        # harness process's folder.
        resolver_cwd = os.path.join(scratch, "resolver-cwd")
        os.makedirs(resolver_cwd, exist_ok=True)
        field = {"working_directory": resolver_cwd,
                 "pathext": RESOLVER_PATHEXT,
                 "current_folder_rule": RESOLVER_CURRENT_FOLDER_RULE}

        def controlled_env():
            env = dict(os.environ)
            set_env_var(env, "PATHEXT", RESOLVER_PATHEXT)
            set_env_var(env, NO_DEFAULT_CURRENT_DIRECTORY_VAR,
                        NO_DEFAULT_CURRENT_DIRECTORY_VALUE)
            return env

        env = controlled_env()
        controlled_path = os.pathsep.join(empties + [child_dir])
        set_env_var(env, "PATH", controlled_path)
        yield "loaded", {"context": "NO_WINDOW",
                         "child": os.path.basename(child),
                         # Round-43 F3: shared materialization,
                         # as at _guard_argv above.
                         "child_args": materialized_child_args(spec),
                         "env": env, "cwd": resolver_cwd,
                         "intended_path": controlled_path,
                         "probe_count": RESOLVER_EMPTY_FOLDERS,
                         **field}
        yield "baseline", {"context": "NO_WINDOW",
                           "env": controlled_env(), "cwd": resolver_cwd,
                           "probe_count": 0, **field}


def run_pricing_probes(config, guard_fingerprint, reduced=False,
                       probes=None):
    """Execute every pricing probe and return the raw observation
    records. Each record names its probe, item, shape and repetition,
    and carries the guard fingerprint the probe measured, so a price
    derived from these records is traceable to them. probes, when
    given, restricts the run to the named probes (a logic exercise
    for a single item); the harness's campaign runs pass it as None
    and execute all of them."""
    reps = REDUCED_PROBE_REPS if reduced else PROBE_REPS
    records = []
    scratch = tempfile.mkdtemp(prefix="ptg-pricing-")
    try:
        for probe, item in PROBES.items():
            if probes is not None and probe not in probes:
                continue
            shapes = list(_shapes(config, probe, scratch))
            for rep in range(reps):
                for shape, kwargs in shapes:
                    kwargs = dict(kwargs)
                    probe_count = kwargs.pop("probe_count", None)
                    intended_path = kwargs.pop("intended_path", None)
                    inserted_levels = kwargs.pop("inserted_levels", None)
                    working_directory = kwargs.pop(
                        "working_directory", None)
                    pathext = kwargs.pop("pathext", None)
                    current_folder_rule = kwargs.pop(
                        "current_folder_rule", None)
                    record = _time_guard_run(config, **kwargs)
                    record.update({
                        "kind": "pricing-probe", "probe": probe,
                        "item": item, "shape": shape, "rep": rep,
                        "guard_fingerprint": guard_fingerprint,
                        "probe_count": probe_count,
                        "intended_path": intended_path,
                        "inserted_levels": inserted_levels,
                        "working_directory": working_directory,
                        "pathext": pathext,
                        "current_folder_rule": current_folder_rule,
                    })
                    records.append(record)
    finally:
        for root, _dirs, files in os.walk(scratch, topdown=False):
            for name in files:
                try:
                    os.unlink(os.path.join(root, name))
                except OSError:
                    pass
            try:
                os.rmdir(root)
            except OSError:
                pass
    return records


def _quartiles(diffs):
    """(first quartile, third quartile) of the paired differences,
    by the inclusive method; a single difference is its own
    quartiles."""
    if len(diffs) < 2:
        value = diffs[0] if diffs else 0.0
        return value, value
    quartiles = statistics.quantiles(diffs, n=4, method="inclusive")
    return quartiles[0], quartiles[2]


# The PID form report.py's record discipline states (report.MAX_PID,
# the widest signed PID space): a positive integer within it. The
# owner-loaded record's guard_pid is a PID in exactly this sense —
# the guard child's PID the recording launcher recorded.
_MAX_PID = 2 ** 31 - 1


def _is_pid_form(value):
    return isinstance(value, int) and not isinstance(value, bool) \
        and 1 <= value <= _MAX_PID


def _is_finite_number(value):
    """Total finiteness for a non-boolean number (round-30 F2):
    an int too large to convert to float (10**400, which JSON
    parses) is NOT finite for a timed lifetime — the check
    resolves it to implausible instead of raising OverflowError,
    as math.isfinite does on such an int. Every value the pricing
    producers touch — huge, non-finite, wrong-typed — resolves
    to a boolean, so a defect is NAMED, never raised."""
    if not (isinstance(value, (int, float))
            and not isinstance(value, bool)):
        return False
    try:
        return math.isfinite(value)
    except OverflowError:
        return False


def _is_plausible_time(value):
    """A time a monotonic clock can produce (round-30 F2): a
    finite, non-negative, non-boolean number — total, in
    _is_finite_number's idiom."""
    return _is_finite_number(value) and value >= 0


def pricing_stamp_defects(records):
    """Cross-check the probe-shape stamps of pricing records
    against the construction each price claims. Returns a mapping
    of item -> list of defect dicts, each naming the probe, the
    shape, the repetition, the field, the expected stamp and the
    stamp the record actually carries. An item absent from the
    mapping has every stamp its price claims.

    The expectations are derived from this module's own probe
    construction, not restated from the report side:

    - console-probe-finish: the context pair _shapes yields —
      baseline "DETACHED", loaded "NO_WINDOW" (the two context
      literals _shapes passes to _time_guard_run for this probe;
      they live only in that builder).
    - owner-verification: inserted_levels as _shapes stamps it —
      baseline 0, loaded 1 (the two literals in that builder: the
      recording launcher is the loaded shape's one inserted level);
      and the context both shapes are constructed under —
      NO_WINDOW for baseline and loaded alike (the context
      literal that builder passes for this probe): a pair whose
      records differ in context as well as in the inserted level
      folds the console item's own effect into this item's price.
    - resolver-probe-count: the context both shapes are
      constructed under — NO_WINDOW for baseline and loaded
      alike (the context literal that builder passes for this
      probe), for the same reason; probe_count baseline 0, loaded
      RESOLVER_EMPTY_FOLDERS (the constant the builder stamps);
      and the pinned field _shapes constructs for BOTH resolver
      shapes — pathext is RESOLVER_PATHEXT and
      current_folder_rule is RESOLVER_CURRENT_FOLDER_RULE (the
      module constants the builder stamps), and
      working_directory is the fresh scratch folder the builder
      constructs once per run and shares by both shapes and
      every repetition (its exact path is per-run, so the
      check is that every resolver record carries a non-empty
      path string AND that all of them carry the same one — a
      pair whose shapes name different working directories was
      not constructed as the pair the price claims). The loaded
      shape alone also stamps intended_path — the controlled
      PATH the builder lays out — so a loaded record must carry
      the laid-out field itself: RESOLVER_EMPTY_FOLDERS empty
      folders followed by the child's folder, exactly
      RESOLVER_EMPTY_FOLDERS + 1 non-empty entries in path-list
      order (the separator is the one the record's own platform
      wrote: ";" on a Windows record, os.pathsep elsewhere),
      with the empty folders named empty-<index> in order under
      one shared scratch parent — the parent of the record's own
      resolver-cwd working directory — so a field of arbitrary
      non-empty entries of the right count does not pass.
      The baseline stamps None there: a baseline record
      carrying a non-null intended_path carries a stamp its
      construction never writes and is a defect, exactly as a
      loaded record carrying a field that does not lay out is.

    The full stamp vector is held, not only each probe's own
    stamps: run_pricing_probes stamps every field on every
    record, with the construction's value or None, so for each
    probe the fields its construction does not stamp for a
    shape must be None on that shape's records — the console
    pair carries no probe_count / intended_path /
    inserted_levels / working_directory / pathext /
    current_folder_rule; the owner pair carries no probe_count /
    intended_path / working_directory / pathext /
    current_folder_rule; the resolver pair carries no
    inserted_levels, and its baseline carries no intended_path.
    A record carrying a non-null value in one of those fields
    carries a stamp no run of its probe and shape writes.

    The record's own classification and identity are held the
    same way (round-23): the item a record carries is the item
    its probe prices (run_pricing_probes stamps item from the
    same PROBES mapping that keys the evidence — a record
    whose item names another probe's item is not a record of
    the item it prices); an "ok" record's exit_code is the
    fast path's expected code (the runner's _time_guard_run
    sets "ok" only when the exit matches it — an "ok" record
    whose exit_code differs, or was never observed, claims a
    success its own exit field refutes); the loaded-only
    fields _time_guard_run writes only on the owner-verification
    loaded shape (guard_pid and outer_elapsed_ms, from the
    recording launcher's bracket) are stamps no other probe or
    shape writes, so a non-null value in either on any other
    record is a defect of the same kind; and a record carries
    exactly its constructed field set — the timed run's fields
    (argv, context, outcome, elapsed_ms, exit_code, and detail
    on a failed run), the stamps run_pricing_probes updates
    onto every record, and, on the owner loaded shape only,
    the two loaded-only fields — so a field outside that set
    is a field no run of its probe and shape writes.
    Round-25 completes the fingerprint hold: where a record's
    guard_fingerprint carries its companion values (path and
    size beside the sha256), each is held to the form
    arms.fingerprint records — the path an absolute path
    string, the size a non-negative integer — and the package
    checker holds them to equality with the run's recorded
    guard fingerprint, so a fabricated companion prices
    nothing."""
    defects = {}
    resolver_wds = {
        record.get("working_directory") for record in records
        if isinstance(record, dict)
        and record.get("kind") == "pricing-probe"
        and record.get("probe") == "resolver-probe-count"
        and isinstance(record.get("working_directory"), str)
        and record.get("working_directory")}
    shared_wd = len(resolver_wds) == 1
    for record in records:
        if not isinstance(record, dict):
            continue
        if record.get("kind") != "pricing-probe":
            continue
        probe = record.get("probe")
        if not isinstance(probe, str) or probe not in PROBES:
            continue
        shape = record.get("shape")
        if shape not in ("baseline", "loaded"):
            continue
        found = []

        def expect(field, expected):
            actual = record.get(field)
            if isinstance(expected, int) \
                    and not isinstance(expected, bool):
                # An integer stamp is carried by an integer: a
                # value of another type that merely compares
                # equal to it (1.0 for 1, above all) is not the
                # stamp the probe writes — the same rule the
                # manifest-count discipline applies to a recorded
                # count, which must carry the integer type, not
                # merely an equal value.
                mismatch = not isinstance(actual, int) \
                    or isinstance(actual, bool) \
                    or actual != expected
            else:
                mismatch = actual != expected
            if mismatch:
                found.append({
                    "probe": probe, "shape": shape,
                    "rep": record.get("rep"), "field": field,
                    "expected": expected, "actual": actual})

        def expect_present(field, expected):
            actual = record.get(field)
            if not (isinstance(actual, str) and actual):
                found.append({
                    "probe": probe, "shape": shape,
                    "rep": record.get("rep"), "field": field,
                    "expected": expected, "actual": actual})

        # The guard fingerprint's companion values (round-25):
        # run_pricing_probes stamps the fingerprint object the
        # harness's fingerprint() call produced — {path, sha256,
        # size}, the path in that function's recorded form (the
        # absolute path of the vendored guard binary) and the
        # size the same call counted (a non-negative integer).
        # Rounds 23/24 held the manifest's and the report's
        # copies of those beside-values but read neither of the
        # pricing records' own, so a fabricated path, a
        # fabricated size, or a size of another type priced at
        # full value. The companions are held AS A PAIR
        # (round-33 F6): a fingerprint carrying either companion
        # is held here on both, each to the
        # recorded form; the package checker holds them further,
        # to equality with the run's recorded guard fingerprint
        # (harness._pricing_provenance). A record whose
        # fingerprint carries neither companion (the tests'
        # sha-only fixture shape) claims no beside-value and is
        # not faulted for the absence here.
        fingerprint = record.get("guard_fingerprint")
        if isinstance(fingerprint, dict) and (
                "path" in fingerprint or "size" in fingerprint):
            path_value = fingerprint.get("path")
            if not (isinstance(path_value, str) and path_value
                    and os.path.isabs(path_value)):
                found.append({
                    "probe": probe, "shape": shape,
                    "rep": record.get("rep"),
                    "field": "guard_fingerprint.path",
                    "expected": "the vendored guard's recorded "
                                "path form: the absolute path "
                                "string arms.fingerprint records "
                                "for the guard binary the price "
                                "claims",
                    "actual": path_value})
            size_value = fingerprint.get("size")
            if not (isinstance(size_value, int)
                    and not isinstance(size_value, bool)
                    and size_value >= 0):
                found.append({
                    "probe": probe, "shape": shape,
                    "rep": record.get("rep"),
                    "field": "guard_fingerprint.size",
                    "expected": "a non-negative integer: the "
                                "size arms.fingerprint counted "
                                "for the guard binary the record "
                                "claims (the package checker "
                                "holds it to equality with the "
                                "run's recorded size)",
                    "actual": size_value})
        # The record's own identity and classification: the
        # item stamp is the item its probe prices, and an "ok"
        # record's exit_code is the fast path's expected code
        # (see the docstring) — both stamped by the construction
        # on every record it writes.
        expect("item", PROBES[probe])
        if record.get("outcome") == "ok":
            expect("exit_code", expected_exit("guard", "fast"))
        # The loaded-only fields: _time_guard_run writes
        # guard_pid / outer_elapsed_ms only when a guard record
        # path is given — the owner-verification loaded shape —
        # so on every other probe and shape a non-null value in
        # either is a stamp that shape never writes.
        if not (probe == "owner-verification" and shape == "loaded"):
            expect("guard_pid", None)
            expect("outer_elapsed_ms", None)
        else:
            # On the one shape that legitimately carries them,
            # their VALUES are held to the construction
            # (round-24): guard_pid is the guard child's PID
            # from the recording launcher's record — a positive
            # integer in PID form — and outer_elapsed_ms is the
            # harness's own bracket around the launcher: a
            # finite, non-negative time that necessarily covers
            # the record's inner elapsed_ms, the launcher's
            # bracket inside it. An "ok" record must carry both
            # in those forms (the bracket is what its price
            # rests on); a record of another outcome is held
            # wherever it carries a value (an infrastructure
            # failure whose launcher never ran carries no
            # guard_pid by construction, and is not faulted for
            # that absence).
            guard_pid = record.get("guard_pid")
            if record.get("outcome") == "ok" \
                    or guard_pid is not None:
                if not _is_pid_form(guard_pid):
                    found.append({
                        "probe": probe, "shape": shape,
                        "rep": record.get("rep"),
                        "field": "guard_pid",
                        "expected": "the guard child's PID as a "
                                    "positive integer within the "
                                    "platform PID range (1 to "
                                    f"{_MAX_PID}) — the recording "
                                    "launcher records the guard "
                                    "child's PID in its bracket "
                                    "record",
                        "actual": guard_pid})
            outer = record.get("outer_elapsed_ms")
            elapsed = record.get("elapsed_ms")
            # Total (round-30 F2): the finiteness terms go through
            # _is_plausible_time, so an extreme value (10**400) in
            # either field is NAMED as this outer_elapsed_ms stamp
            # defect — the defect vocabulary this function exists
            # to produce, whose "expected" text already states the
            # finite, non-negative, covers-the-inner-time form —
            # instead of raising OverflowError out of
            # math.isfinite. An implausible inner elapsed cannot
            # make the bracket "cover" it: the comparison term
            # engages only when the inner value is itself a
            # plausible time.
            outer_ok = _is_plausible_time(outer) \
                and not (_is_plausible_time(elapsed)
                         and outer < elapsed)
            if record.get("outcome") == "ok" or outer is not None:
                if not outer_ok:
                    found.append({
                        "probe": probe, "shape": shape,
                        "rep": record.get("rep"),
                        "field": "outer_elapsed_ms",
                        "expected": "the harness's bracket "
                                    "around the recording "
                                    "launcher: a finite, "
                                    "non-negative time not below "
                                    "the record's own elapsed_ms "
                                    "(the launcher's bracket "
                                    "inside it)",
                        "actual": outer})
        # Exactly the constructed field set: the timed run's
        # fields, the stamps run_pricing_probes updates onto
        # every record, and (owner loaded only) the loaded-only
        # fields. Any other key is a field no run writes.
        constructed = {
            "argv", "context", "outcome", "elapsed_ms",
            "exit_code", "detail", "kind", "probe", "item",
            "shape", "rep", "guard_fingerprint", "probe_count",
            "intended_path", "inserted_levels",
            "working_directory", "pathext", "current_folder_rule"}
        if probe == "owner-verification" and shape == "loaded":
            constructed |= {"guard_pid", "outer_elapsed_ms"}
        for field in sorted(set(record) - constructed):
            found.append({
                "probe": probe, "shape": shape,
                "rep": record.get("rep"), "field": field,
                "expected": "no field beyond the constructed "
                            "field set of the record's probe "
                            "and shape (the timed run's fields "
                            "and the stamps run_pricing_probes "
                            "writes, plus the loaded-only fields "
                            "on the owner-verification loaded "
                            "shape only)",
                "actual": record.get(field)})
        # The None stamps of the full vector: fields this
        # probe's construction does not stamp for this shape
        # must be None (run_pricing_probes stamps them None).
        none_fields = {
            "console-probe-finish": (
                "probe_count", "intended_path", "inserted_levels",
                "working_directory", "pathext",
                "current_folder_rule"),
            "owner-verification": (
                "probe_count", "intended_path",
                "working_directory", "pathext",
                "current_folder_rule"),
            "resolver-probe-count": ("inserted_levels",),
        }[probe]
        if probe == "resolver-probe-count" and shape == "baseline":
            none_fields = none_fields + ("intended_path",)
        for field in none_fields:
            expect(field, None)
        if probe == "console-probe-finish":
            expect("context",
                   "NO_WINDOW" if shape == "loaded" else "DETACHED")
        elif probe == "owner-verification":
            expect("context", "NO_WINDOW")
            expect("inserted_levels", 1 if shape == "loaded" else 0)
        elif probe == "resolver-probe-count":
            expect("context", "NO_WINDOW")
            expect("probe_count",
                   RESOLVER_EMPTY_FOLDERS if shape == "loaded" else 0)
            expect("pathext", RESOLVER_PATHEXT)
            expect("current_folder_rule", RESOLVER_CURRENT_FOLDER_RULE)
            expect_present(
                "working_directory",
                "a non-empty path string (the controlled scratch "
                "working directory the probe constructs)")
            if not shared_wd and record.get("working_directory"):
                found.append({
                    "probe": probe, "shape": shape,
                    "rep": record.get("rep"),
                    "field": "working_directory",
                    "expected": "the one working directory every "
                                "resolver record of the run shares "
                                "(the controlled scratch folder both "
                                "shapes are constructed under)",
                    "actual": record.get("working_directory")})
            if shape == "loaded":
                intended = record.get("intended_path")
                parts = []
                if isinstance(intended, str) and intended:
                    separator = ";" if ";" in intended else os.pathsep
                    parts = intended.split(separator)
                # The laid-out field, checked as the construction
                # lays it out — not merely counted: the first
                # RESOLVER_EMPTY_FOLDERS entries are the probe's
                # own empty folders, named empty-<index> in order
                # under one shared scratch parent; the working
                # directory the same record carries is that
                # parent's resolver-cwd folder; and exactly one
                # further entry, the child's folder, follows. A
                # field of five arbitrary non-empty entries (five
                # system folders, say) carries none of that
                # construction and must not price, however tidy
                # its entry count looks.
                def _norm(part):
                    return part.replace("\\", "/").rstrip("/")
                laid_out = len(parts) == RESOLVER_EMPTY_FOLDERS + 1 \
                    and all(parts)
                if laid_out:
                    normed = [_norm(part) for part in parts]
                    empties = normed[:RESOLVER_EMPTY_FOLDERS]
                    parents = {entry.rsplit("/", 1)[0]
                               if "/" in entry else ""
                               for entry in empties}
                    names = [entry.rsplit("/", 1)[-1]
                             for entry in empties]
                    wd = record.get("working_directory")
                    wd_norm = _norm(wd) if isinstance(wd, str) else ""
                    laid_out = (
                        names == [f"empty-{index}" for index in range(
                            RESOLVER_EMPTY_FOLDERS)]
                        and len(parents) == 1
                        and wd_norm.rsplit("/", 1)[-1] == "resolver-cwd"
                        and (wd_norm.rsplit("/", 1)[0]
                             if "/" in wd_norm else "") == parents.pop())
                if not laid_out:
                    found.append({
                        "probe": probe, "shape": shape,
                        "rep": record.get("rep"),
                        "field": "intended_path",
                        "expected": f"the controlled PATH the probe "
                                    f"lays out: {RESOLVER_EMPTY_FOLDERS} "
                                    f"empty folders (empty-0.."
                                    f"empty-{RESOLVER_EMPTY_FOLDERS - 1} "
                                    f"under one scratch parent, the "
                                    f"parent of the record's "
                                    f"resolver-cwd working directory) "
                                    f"followed by the child's folder "
                                    f"({RESOLVER_EMPTY_FOLDERS + 1} "
                                    f"non-empty path-list entries)",
                        "actual": intended})
        if found:
            defects.setdefault(PROBES[probe], []).extend(found)
    return defects


def prices_from_observations(records):
    """Build the pricing evidence object from raw probe observations,
    or None when no complete pair set prices any item.

    The object is a reference to the observations, not a table of
    numbers: it carries the kind tag, the guard fingerprint the
    probes measured, the probe name and paired-observation count
    behind each price, and the total observation count. An item
    with any failed or missing pair is absent — not measured.
    An item whose records fail the probe-shape stamps (see
    pricing_stamp_defects) is likewise absent — not measured:
    pairing by the shape label alone would price a shape
    difference the records say never existed. Each priced item
    also carries its uncertainty: the paired
    differences, their dispersion (minimum, maximum, quartiles,
    interquartile range) and their sign counts, beside the median
    (the median itself is unchanged by carrying the spread)."""
    # A record that cannot pair — not an object, or an "ok" probe
    # record whose rep is missing or not an integer, whose probe
    # or shape is missing or not a string, or whose
    # elapsed time is missing, not a number, or not a time the
    # probe's monotonic clock can produce (NaN, infinite or
    # negative — each parses as a JSON number, and a paired
    # difference over one is not a measurement) — prices nothing:
    # it is skipped here, exactly as a failed or missing pair is,
    # never allowed to crash the pairing arithmetic. (The package
    # checker names such records as defects in its own right;
    # this function's discipline is only that they cannot price.)
    def pairable(record):
        # Total (round-30 F2): the elapsed term goes through
        # _is_plausible_time, so an extreme value (10**400) makes
        # the record unpairable — skipped here, exactly as a
        # failed or missing pair is — instead of raising
        # OverflowError out of math.isfinite. The item such a
        # record would price is absent — not measured — and the
        # record itself is NAMED in the existing defect
        # vocabulary: pricing_stamp_defects names an extreme
        # outer_elapsed_ms on the owner-loaded shape (the stamp
        # whose expected form is a finite time), and the
        # harness's pricing shape check names an implausible
        # elapsed_ms on any pricing record.
        return isinstance(record, dict) \
            and isinstance(record.get("rep"), int) \
            and not isinstance(record.get("rep"), bool) \
            and isinstance(record.get("probe"), str) \
            and isinstance(record.get("shape"), str) \
            and _is_plausible_time(record.get("elapsed_ms"))

    ok = [r for r in records if isinstance(r, dict)
          and r.get("outcome") == "ok"
          and r.get("kind") == "pricing-probe" and pairable(r)]
    fingerprints = {r.get("guard_fingerprint", {}).get("sha256")
                    for r in records
                    if isinstance(r, dict)
                    and isinstance(r.get("guard_fingerprint"), dict)
                    and isinstance(
                        r.get("guard_fingerprint", {}).get("sha256"),
                        str)}
    stamp_defects = pricing_stamp_defects(records)
    items = {}
    for probe, item in PROBES.items():
        if item in stamp_defects:
            # Stamp discipline: the records do not carry the
            # construction this item's price claims, so the item
            # does not price — absent here, "not measured" in the
            # report, and named by the package provenance check.
            continue
        loaded = {r["rep"]: r["elapsed_ms"] for r in ok
                  if r.get("probe") == probe and r.get("shape") == "loaded"}
        baseline = {r["rep"]: r["elapsed_ms"] for r in ok
                    if r.get("probe") == probe
                    and r.get("shape") == "baseline"}
        reps = sorted(set(loaded) & set(baseline))
        ran = [r for r in records if isinstance(r, dict)
               and r.get("probe") == probe]
        if not reps or len(ran) != 2 * len(reps):
            continue
        if len(reps) < REDUCED_PROBE_REPS:
            # Provenance floor: a price needs at least the reduced
            # shape's paired repetitions. One tidy pair is not a
            # measurement (P9).
            continue
        diffs = [loaded[rep] - baseline[rep] for rep in reps]
        median = statistics.median(diffs)
        q1, q3 = _quartiles(diffs)
        positive = sum(1 for d in diffs if d > 0)
        negative = sum(1 for d in diffs if d < 0)
        zero = len(diffs) - positive - negative
        entry = {
            "ms": median,
            "probe": probe,
            "paired_observations": len(reps),
            "paired_differences_ms": diffs,
            "min_ms": min(diffs),
            "max_ms": max(diffs),
            "q1_ms": q1,
            "q3_ms": q3,
            "iqr_ms": q3 - q1,
            "positive_count": positive,
            "negative_count": negative,
            "zero_count": zero,
            "sign_consistent": not (positive and negative),
            "trace": (
                f"probe {probe}, {len(reps)} paired observations, "
                f"median {median:.3f} ms, spread min {min(diffs):.3f} ms "
                f"to max {max(diffs):.3f} ms, quartiles "
                f"{q1:.3f} / {q3:.3f} ms, IQR {q3 - q1:.3f} ms, "
                f"signs +{positive} / -{negative} / 0:{zero}"),
        }
        if probe == "resolver-probe-count":
            # The paired shapes differ by RESOLVER_EMPTY_FOLDERS
            # filesystem probes — the difference the records' own
            # probe_count fields carry (loaded:
            # RESOLVER_EMPTY_FOLDERS, baseline: 0): one probe per
            # empty PATH folder. The hit itself is probed in both
            # shapes and cancels, and the controlled field disables
            # the current-folder probe (NoDefaultCurrentDirectory-
            # InExePath is set in both shapes' environments), so no
            # fifth probe enters the difference.
            extra_probes = RESOLVER_EMPTY_FOLDERS
            per_probe = entry["ms"] / extra_probes
            entry["trace"] += (
                f", {extra_probes} extra resolver probes "
                f"({per_probe:.3f} ms per probe)")
        items[item] = entry
    if not items:
        return None
    return {
        "kind": "pricing-probe-evidence",
        "guard_fingerprint": sorted(f for f in fingerprints if f),
        "observation_count": len(records),
        "items": items,
    }


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == GUARD_LAUNCHER_MODE:
        sys.exit(_guard_launcher_main(sys.argv[2:]))
    sys.exit(97)
