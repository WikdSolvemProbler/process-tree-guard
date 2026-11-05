# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""The campaign plan, as configured: 36 controls + 224 warmups +
1,680 measured = 1,940 slots, in six arms, with the NO_WINDOW and
DETACHED console contexts kept separate.

This is the reference plan: its original decomposition is inherited
from MEASUREMENT.md section 4 of the guard tree, and its size is set
by the measurement argument, not by quota — 1,620 is the reference
decomposition of the ORIGINAL plan shape (36 + 144 + 1,440, the
fast/kill cells below), not a correctness requirement, a closure
criterion or a defect trigger, and an argued resize re-derives the
totals from the configured structure below. The round-26 resize is
such an argued change, derived here in full:

- Four product arms (the guard and the three competitors, as found)
  are measured on both original paths (fast, kill) in both contexts:
  4 arms x 2 paths x 2 contexts x 90 repetitions = 1,440 measured.
- The same sixteen cells are warmed first:
  4 arms x 2 paths x 2 contexts x 9 repetitions = 144 warmups.
- Two floor arms (controls) run the fast path only, because a floor
  has no kill semantics to exercise:
  2 arms x 2 contexts x 9 repetitions = 36 controls.
- Malformed-input cells (profile dimension (6), round-26 F1): the
  six malformed shapes the dimension's own text names — a
  nonexistent program, a malformed timeout, an out-of-range
  timeout, an empty command, an over-long argument and a
  non-Unicode argument — are presented identically to every product
  arm in both contexts. Each shape is a distinct failure mechanism
  (SPEC_Benchmark section 5), so each shape is its own cell
  dimension; 3 repetitions per cell is the smallest count that
  separates a systematic refusal (3 of 3) from a one-off flake
  without pricing the dimension at the latency cells' depth, whose
  question (a median) is not this dimension's question (the arm's
  recorded response to the shape):
  6 shapes x 4 arms x 2 contexts x 3 repetitions = 144 measured,
  plus one warmup per cell (6 x 4 x 2 x 1 = 48 warmups) so a cold
  first presentation is never the recorded one.
- Scaling cells (profile dimension (4), round-26 F4): tree width
  and depth as plan parameters, driven through the campaign's
  existing kill machinery (the slot is a kill slot whose child
  builds a tree of the named size; the survivor probe judges it
  exactly as on the kill path). The sizes are a geometric ladder
  bracketing the two boundaries the guard's own failure tree
  names: width 8 (below) and width 72 (above) the 64-member
  teardown window boundary of FAILURE-TREE section 2, and depth 4
  (a shallow chain) and depth 40 (the in-tree deep chain's size).
  The level conventions, stated exactly (round-27 F5): the
  in-tree deep chain starts at level 1 — the tree's battery B35
  runs testkit "deep 1 40" and asserts exactly 40 levels — and
  the campaign's deep chain uses the same convention: its child
  spec starts the chain at level 1 (campaign.json's scale_deep
  args, mirrored by the stub config in the tests), so deep-D
  builds levels 1 through D, D processes in all, each publishing
  its PID. A wide-N tree is one root publishing N descendant
  PIDs. The published count a scale record must carry is
  therefore exact (see expected_descendants): N for wide-N, D
  for deep-D; a record publishing fewer did not present the
  named tree (round-27 F1).
  4 scales x 4 arms x 2 contexts x 3 repetitions = 96 measured
  (3 repetitions, as for the malformed cells: the question is
  whether the tree dies, not a median), plus one warmup per cell
  (4 x 4 x 2 x 1 = 32 warmups).

The new totals are what that argument yields — 36 controls,
224 warmups (144 + 48 + 32), 1,680 measured (1,440 + 144 + 96),
1,940 in all — never a quota, and no slot was trimmed or padded
to reach a number. Once a run starts, that run's plan is frozen:
its arms and slots are fixed for the run, with no mid-run drift.

Within a malformed or scaling cell, repetitions are numbered
across the cell's variants in plan order (variant-major): rep r of
a (kind, arm, path, context) cell is variant r // REPS in
MALFORMED_SHAPES / SCALE_VARIANTS order, repetition r % REPS
inside that variant. A slot's variant therefore re-derives from
the plan alone, and each record's rep stays unique inside its
cell for the exact slot accounting.

Execution order: the control block runs first, in its fixed order
(the controls prove the harness before anything is measured). The
warmup and measured blocks are shuffled under a recorded seed
(build_plan's seed argument; DEFAULT_SEED when none is given), so a
fixed guard-first order cannot charge every within-block warm-up,
thermal or load gradient to the same arms in the same direction.
The shuffle is deterministic per seed: the same seed always yields
the same order, the seed and the full order are preserved in the
run's run.json, and the run's configured per-cell totals are
unchanged — shuffling permutes slots, it never adds, drops or
duplicates one.
"""

import random
from dataclasses import dataclass

PRODUCT_ARMS = ("guard", "processkit", "nuewframe", "sysprims")
FLOOR_ARMS = ("floor-direct", "floor-spawn")
ARMS = PRODUCT_ARMS + FLOOR_ARMS
PATHS = ("fast", "kill")
CONTEXTS = ("NO_WINDOW", "DETACHED")

MEASURED_REPS = 90
WARMUP_REPS = 9
CONTROL_REPS = 9

# The malformed-input shapes of profile dimension (6), in plan
# order. Each is presentable to all four arms identically through
# the arm templates (see runner.malformed_argv).
MALFORMED_SHAPES = (
    "nonexistent-program",
    "malformed-timeout",
    "out-of-range-timeout",
    "empty-command",
    "over-long-argument",
    "non-unicode-argument",
)
MALFORMED_REPS = 3
MALFORMED_WARMUP_REPS = 1

# The designated arm of the campaign: the software under
# evaluation, whose documented behaviour — alone among the arms —
# pins expectations the campaign may hold it to. Competitors are
# evaluated as found; their answers are observations, never
# expectations.
DESIGNATED_ARM = "guard"

# The designated arm's pinned expectation per malformed shape
# (round-31 F2): the exit code the guard's own contract promises
# for a shape, or no entry where its contract pins none. The
# guard's exit-code table (the guard tree's README, "Exit
# codes") states 125 for a guard error — invalid arguments and
# executable not found are its named cases — which pins the four
# refusal shapes: a nonexistent program (executable not found),
# a malformed timeout, an out-of-range timeout and an empty
# command (invalid arguments). A non-Unicode argument is pinned
# by the same 125 case through the guard's own argument rule:
# every argument the guard receives must be valid Unicode (the
# guard's text_args conversion refuses any other with a guard
# error, and this suite's F9 family pins that refusal at 125 in
# every position). The over-long argument is deliberately
# ABSENT: the guard's contract states no argument-length limit
# and a 4096-character argument is an ordinary argument the
# child itself answers, so that shape remains an unpinned
# observation for every arm, the designated arm included.
MALFORMED_DESIGNATED_EXPECTED_EXITS = {
    "nonexistent-program": 125,
    "malformed-timeout": 125,
    "out-of-range-timeout": 125,
    "empty-command": 125,
    "non-unicode-argument": 125,
}


def malformed_expected_exit(arm, variant):
    """The pinned expected exit code for one malformed slot, or
    None when no expectation is pinned for it: an expectation
    exists only for the DESIGNATED arm and only for a shape in
    MALFORMED_DESIGNATED_EXPECTED_EXITS. Every competitor slot,
    and the designated arm's over-long-argument slot, returns
    None — its recorded code is an observation, and no rule
    gated on a pinned expectation may fire for it."""
    if arm != DESIGNATED_ARM:
        return None
    return MALFORMED_DESIGNATED_EXPECTED_EXITS.get(variant)


def slot_variant(path, rep, kind):
    """The variant the PLAN encodes for one slot identity
    (path, rep, kind), total: variant_for's re-derivation, with
    every failure of that derivation (an unusable rep — not a
    non-bool integer — or a rep outside its kind's band for
    the path, a negative rep included: variant_for checks the
    band before indexing, so a negative rep derives None,
    never another variant through negative indexing) resolving
    to None instead of raising.
    This — the slot's identity in the plan — is the shape a
    malformed or scale record is held to (round-32 F2): a
    record's own carried variant is a claim ABOUT the slot,
    never the slot's identity itself."""
    if path not in ("malformed", "scale"):
        return None
    if not isinstance(rep, int) or isinstance(rep, bool):
        return None
    try:
        return variant_for(path, rep, kind)
    except (IndexError, TypeError):
        return None


def malformed_pin_violation(arm, variant, exit_code):
    """THE designated malformed-pin formulation (round-32 F2):
    ONE predicate, read by the runner's classification (through
    arms.expected_exit, which delegates to
    malformed_expected_exit with the SLOT's variant) and by the
    report checker alike (which calls this function with the
    PLAN-DERIVED variant, slot_variant(path, rep, kind) — never
    a record's self-carried label), over warmup and measured
    slots alike, exactly as plan.scale_count_matches is the one
    scale-count formulation the runner and the checker share
    (round-30 F1's pattern). Returns the pinned expected exit
    code when this slot answers a contract-pinned shape with an
    observed integer code other than the pinned one — a pin
    violation — and None in every other case: a competitor arm,
    an unpinned shape, a code that meets the pin, or a code not
    observed as an integer (a missing or untyped code carries
    no answer; the record holds name it in their own idiom)."""
    expected = malformed_expected_exit(arm, variant)
    if expected is None:
        return None
    if isinstance(exit_code, int) and not isinstance(exit_code, bool) \
            and exit_code != expected:
        return expected
    return None

# The scaling variants of profile dimension (4), in plan order:
# (name, width, depth) — exactly one of width/depth is set; the
# child machinery builds a tree of that width or that depth.
SCALE_VARIANTS = (
    ("wide-8", 8, None),
    ("wide-72", 72, None),
    ("deep-4", None, 4),
    ("deep-40", None, 40),
)
SCALE_NAMES = tuple(name for name, _w, _d in SCALE_VARIANTS)
SCALE_REPS = 3
SCALE_WARMUP_REPS = 1


def expected_descendants(variant):
    """The exact published descendant count a scale record of one
    variant must carry (round-27 F1/F5): a wide-N tree's root
    publishes N descendant PIDs (its ids file), and a deep-D
    chain — started at level 1, the in-tree convention — has D
    levels, each publishing its PID into the record's
    descendant_pids. None for a name that is not a scale variant."""
    for name, width, depth in SCALE_VARIANTS:
        if name == variant:
            return width if width is not None else depth
    return None


def scale_excluded_roots(variant, arm_pid, child_pid):
    """The roots the scale count excludes, per variant (round-29
    F4): for a WIDE variant both roots — arm_pid (the arm process
    itself) and child_pid (the scale tree's root, which the
    runner records separately from the wide child's child.pid
    file; the wide child's ids file carries the N descendants
    themselves, so the published list's raw length is the variant
    count N); for a DEEP variant arm_pid alone — the published
    list carries the level-1 root itself (the per-level files,
    levels 1 through D, so the published list's raw length is
    the variant count D), and a root also named in child_pid
    neither adds nor subtracts."""
    if isinstance(variant, str) and variant.startswith("deep-"):
        return (arm_pid,)
    return (arm_pid, child_pid)


def scale_distinct_count(variant, published_pids, arm_pid, child_pid):
    """The distinct-after-exclusion cardinality of a published
    PID list under scale_excluded_roots: a PID repeated any
    number of times counts once; an excluded root offered as a
    descendant counts zero times. None when published_pids is
    not a list at all."""
    if not isinstance(published_pids, list):
        return None
    excluded = scale_excluded_roots(variant, arm_pid, child_pid)
    distinct = []
    for pid in published_pids:
        if pid in excluded:
            continue
        if pid not in distinct:
            distinct.append(pid)
    return len(distinct)


def scale_count_matches(variant, published_pids, arm_pid, child_pid):
    """THE scale count rule (round-30 F1): ONE predicate, computed
    identically by the runner's scale classification and by the
    report checker's count hold — both call this function, so a
    runner-"ok" scale record is a record the checker accepts, by
    the same numbers. The rule, per variant, with owed =
    expected_descendants(variant):

    - the published list's LENGTH equals owed — the variant's
      raw published count (wide: the ids file carries exactly
      the N descendants, the root recorded separately in
      child_pid and never part of the list; deep: the per-level
      files carry exactly the D level PIDs, level 1 included);
      AND
    - its DISTINCT-AFTER-EXCLUSION cardinality equals owed,
      under scale_excluded_roots (wide excludes arm_pid and
      child_pid; deep excludes arm_pid alone).

    Excluded-root over-publication therefore fails BOTH terms'
    conjunction in both places: a wide list carrying the root
    beside the full tree has length owed+1, and a deep list
    carrying arm_pid beside the full chain likewise — neither
    classifies "ok" in the runner nor passes the checker.
    Non-root over-publication fails the distinct term in both
    places, as under- and repeated publication fail it too.
    False for a name that is not a scale variant, and for a
    published value that is not a list."""
    owed = expected_descendants(variant)
    if owed is None or not isinstance(published_pids, list):
        return False
    return len(published_pids) == owed and scale_distinct_count(
        variant, published_pids, arm_pid, child_pid) == owed

# The expected counts are DERIVED from the configured structure
# above, never independent literals: controls are the floor arms on
# their one (fast) path, warmups and measured slots are the product
# arms on both original paths, each in both contexts at its rep
# constant, plus the malformed and scaling cells at theirs. An
# argued resize of an arm set, a path/context set, a shape/scale
# set or a rep constant therefore re-derives these counts with it.
EXPECTED_CONTROLS = len(FLOOR_ARMS) * len(CONTEXTS) * CONTROL_REPS
EXPECTED_WARMUPS = (
    len(PRODUCT_ARMS) * len(PATHS) * len(CONTEXTS) * WARMUP_REPS
    + len(MALFORMED_SHAPES) * len(PRODUCT_ARMS) * len(CONTEXTS)
    * MALFORMED_WARMUP_REPS
    + len(SCALE_VARIANTS) * len(PRODUCT_ARMS) * len(CONTEXTS)
    * SCALE_WARMUP_REPS)
EXPECTED_MEASURED = (
    len(PRODUCT_ARMS) * len(PATHS) * len(CONTEXTS) * MEASURED_REPS
    + len(MALFORMED_SHAPES) * len(PRODUCT_ARMS) * len(CONTEXTS)
    * MALFORMED_REPS
    + len(SCALE_VARIANTS) * len(PRODUCT_ARMS) * len(CONTEXTS)
    * SCALE_REPS)
EXPECTED_TOTAL = EXPECTED_CONTROLS + EXPECTED_WARMUPS + EXPECTED_MEASURED

# The campaign seed used when neither the configuration nor the
# command line names one. A campaign's seed is recorded in its
# run.json beside the shuffled order it produced.
DEFAULT_SEED = 20261029


@dataclass(frozen=True)
class Slot:
    kind: str        # "control" | "warmup" | "measured"
    arm: str
    path: str        # "fast" | "kill" | "malformed" | "scale"
    context: str     # "NO_WINDOW" | "DETACHED"
    rep: int         # 0-based repetition inside its cell
    variant: str = None  # malformed shape / scale name, else None


def slot_key(slot):
    """The identity of one slot as a JSON-friendly list, used to
    preserve the execution order in run.json and to check the
    observations against it. Ordinary slots keep the five-element
    form; a malformed or scaling slot appends its variant, so two
    variants of one cell never share an order entry."""
    key = [slot.kind, slot.arm, slot.path, slot.context, slot.rep]
    if slot.variant is not None:
        key.append(slot.variant)
    return key


def variant_for(path, rep, kind):
    """The variant a (path, rep, kind) triple names, re-derived
    from the configured structure: variant-major rep numbering
    (see the module docstring). None for the original paths.

    The band is checked BEFORE any indexing (round-34 F2): a
    rep outside its kind's band for the path — above all a
    NEGATIVE rep, which Python's negative indexing would
    silently resolve to another variant (rep -1 deriving the
    last variant of the band) — raises IndexError here, so
    slot_variant's total form derives None for it: a rep the
    plan holds no slot for derives no variant, never another
    variant."""
    if path == "malformed":
        reps = MALFORMED_REPS if kind == "measured" else MALFORMED_WARMUP_REPS
        if not 0 <= rep < len(MALFORMED_SHAPES) * reps:
            raise IndexError(
                f"rep {rep!r} is outside the malformed band for "
                f"kind {kind!r}")
        return MALFORMED_SHAPES[rep // reps]
    if path == "scale":
        reps = SCALE_REPS if kind == "measured" else SCALE_WARMUP_REPS
        if not 0 <= rep < len(SCALE_NAMES) * reps:
            raise IndexError(
                f"rep {rep!r} is outside the scale band for "
                f"kind {kind!r}")
        return SCALE_NAMES[rep // reps]
    return None


def build_plan(seed=DEFAULT_SEED):
    """Return the full ordered slot list for one seed. Controls come
    first in fixed order (they prove the harness, and a stopped
    campaign is visibly incomplete); the warmup and measured blocks
    are each shuffled under the seed, deterministically, so the
    order is reproducible from the recorded seed alone and a
    different seed yields a different order."""
    slots = []
    for arm in FLOOR_ARMS:
        for context in CONTEXTS:
            for rep in range(CONTROL_REPS):
                slots.append(Slot("control", arm, "fast", context, rep))
    measured_cells = [
        (arm, path, context)
        for arm in PRODUCT_ARMS for path in PATHS for context in CONTEXTS
    ]
    warmups = []
    for arm, path, context in measured_cells:
        for rep in range(WARMUP_REPS):
            warmups.append(Slot("warmup", arm, path, context, rep))
    for arm in PRODUCT_ARMS:
        for context in CONTEXTS:
            for rep in range(len(MALFORMED_SHAPES) * MALFORMED_WARMUP_REPS):
                warmups.append(Slot(
                    "warmup", arm, "malformed", context, rep,
                    variant_for("malformed", rep, "warmup")))
            for rep in range(len(SCALE_VARIANTS) * SCALE_WARMUP_REPS):
                warmups.append(Slot(
                    "warmup", arm, "scale", context, rep,
                    variant_for("scale", rep, "warmup")))
    measured = []
    for rep in range(MEASURED_REPS):
        for arm, path, context in measured_cells:
            measured.append(Slot("measured", arm, path, context, rep))
    for arm in PRODUCT_ARMS:
        for context in CONTEXTS:
            for rep in range(len(MALFORMED_SHAPES) * MALFORMED_REPS):
                measured.append(Slot(
                    "measured", arm, "malformed", context, rep,
                    variant_for("malformed", rep, "measured")))
            for rep in range(len(SCALE_VARIANTS) * SCALE_REPS):
                measured.append(Slot(
                    "measured", arm, "scale", context, rep,
                    variant_for("scale", rep, "measured")))
    rng = random.Random(seed)
    rng.shuffle(warmups)
    rng.shuffle(measured)
    return slots + warmups + measured


def check_plan(slots):
    """Raise ValueError unless the plan matches its own configured
    decomposition — the counts derived above from the configured
    arm/path/context structure and rep constants. This validates
    construction against configuration (a dropped, duplicated or
    misplaced slot), not against an independent total mandate."""
    counts = {"control": 0, "warmup": 0, "measured": 0}
    for slot in slots:
        counts[slot.kind] += 1
    if counts["control"] != EXPECTED_CONTROLS:
        raise ValueError(f"controls {counts['control']} != {EXPECTED_CONTROLS}")
    if counts["warmup"] != EXPECTED_WARMUPS:
        raise ValueError(f"warmups {counts['warmup']} != {EXPECTED_WARMUPS}")
    if counts["measured"] != EXPECTED_MEASURED:
        raise ValueError(f"measured {counts['measured']} != {EXPECTED_MEASURED}")
    if len(slots) != EXPECTED_TOTAL:
        raise ValueError(f"total {len(slots)} != {EXPECTED_TOTAL}")
    if len(set(slots)) != len(slots):
        raise ValueError("duplicate slots in plan")
    return counts
