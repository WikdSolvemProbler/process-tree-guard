# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Reporting in the benchmark specification's terms.

Binding rules implemented here:

- Floor-based interpretation, per console context: the guard is
  reported by its distance above the machine/harness floor of the
  same context (the agreement of the two minimal floor arms in that
  context), decomposed against the counted-ops census. The headline
  is two context-labelled distances, NO_WINDOW and DETACHED kept
  separate exactly as the frozen design keeps them; no pooled
  number across contexts is produced anywhere. A raw median race
  against one competitor is never the headline.
- The anchor arm is the current gauntlet binary; its fingerprint is
  recorded in every report. The stale pairing may appear only with
  its provenance, carried verbatim in STALE_PAIRING_PROVENANCE.
- A decomposition prices per item only the expensive class: console
  probe + console finish, owner verification, resolver probe count.
  Prices come only from the campaign's own pricing probes, accepted
  as a reference to the recorded probe observations (the evidence
  object pricing.prices_from_observations builds: kind tag, guard
  fingerprint, probe name and paired-observation count per item).
  A bare mapping of item names to numbers — operator-supplied
  pricing typed into the configuration — is rejected as pricing and
  reported as "operator-supplied, not measured"; an item with no
  complete probe is reported as not measured, never estimated from
  the counted-op delta and never given a number.
- Prices carry uncertainty: every priced item is reported with the
  dispersion of its paired differences (spread and sign counts, in
  its trace) alongside its median. A negative price, paired
  differences that disagree in sign, or priced items whose sum
  exceeds the distance they decompose is an INCONSISTENCY: it is
  flagged as such on the item's own line and as a standing loss
  (the campaign is INCOMPLETE), the decomposition states that its
  parts do not sum, and no unexplained remainder is printed for
  that context — a negative remainder is never printed as a
  measurement.
- Kill claims about the timeout competitor are stated exactly, in
  the words of NUEWFRAME_KILL_FACTS, and nowhere looser.
- Completeness is exact slot accounting against plan.build_plan():
  every plan slot (by kind/arm/path/context/rep) must be present
  exactly once. A total that merely matches while cells are missing,
  duplicated or unexpected is not complete.
- Only records classified "ok" enter medians. Exit-mismatch and
  infrastructure-failure records are listed as standing losses,
  never timed as successes.
- Record consistency: a record's classification must agree with
  the observations stored in that same record (see
  record_consistency_losses()). A record whose exit_code
  contradicts arms.py's expected-exit table without the
  "exit-mismatch" classification, an "ok" kill record whose
  survivor field is not False or which carries no descendant
  tree identity (grandchild_pid, the identity the shipped kill
  workload records at the slot's marker and the survivor probe
  judges; child_pid is recorded where the workload records it
  and must be an integer PID whenever it is present; and
  arm_pid, the arm process's own PID, which the runner records
  for every spawned slot — and each identity must be a PID in
  form as well as in type: a positive integer within the
  platform PID range), a
  "descendant-survived" kill record carrying no tree identity
  in that same PID form (the survival classification rests on
  the same descendant identity the probe judged, so a survival
  resting on no PID — grandchild_pid absent, null, non-integer
  or outside the range, or a present child_pid outside it —
  earns no survival, no competitor loss and no kill-latency
  exclusion), a record whose exit fault is claimed from an
  exit_code that is not an integer at all (the runner records
  an integer exit code or none, so a non-integer code differing
  from the expected code earns no exit fault), and an "ok" record
  of a kind whose elapsed time feeds a median (measured, and
  control — the floor median rests on control records) with
  no elapsed_ms are each a named
  standing loss identifying the field and the slot; the campaign
  is INCOMPLETE and the freeze gate does not PASS the dimension
  that record feeds. One precedence is exempt from the exit_code
  rule: on the kill path the runner classifies a returned arm
  that left a survivor as "descendant-survived" even when its
  exit code also mismatches (survivor-before-exit precedence —
  one label cannot carry both faults, and survival is the
  graver one); such a record is not an inconsistency, and its
  exit fault is listed separately by standing_losses() under
  its own name. The
  discipline runs in both directions: a negative classification
  must also be earned by the record's own fields. An
  "exit-mismatch" record whose exit_code is the expected code
  (or was never observed), a "descendant-survived" record whose
  survivor field is not True, an "output-mismatch" record in
  which neither stream records a False match verdict, an "ok"
  fast record in which a stream does record a False match
  verdict, and a fast record that carries a match verdict
  without the digest and length that verdict rests on are each
  the same kind of named standing loss, and none of them mints
  the evidence its label claims (no competitor loss, no
  kill-latency exclusion, no output-correctness PASS). The
  harness's package checker applies the same discipline to a
  stored package's records.
- Classifications are also paired to the path that writes them:
  "descendant-survived" is written only on the kill path and
  "output-mismatch" only on the fast path, so a record carrying
  either classification on the other path is the same kind of
  named standing loss — a classification resting on observations
  its path never makes. The survivor observation is held under
  every kill label, not only "ok" and "descendant-survived": a
  returned kill arm whose survivor field is True is classified
  "descendant-survived" by the runner's precedence before its
  exit code is compared, so a survivor of True under an
  "exit-mismatch" (or "output-mismatch") label is named, and
  kill-path correctness does not read an arm clean over it (the
  timed-out infrastructure failure, the one shape that records
  a survivor of True under another label, keeps that label).
  And a "descendant-survived" record must carry the exit code
  the runner records from its wait before it classifies
  survival: a survived record whose exit_code was never
  observed is named, and its survival is earned nowhere (no
  competitor loss, no kill-latency exclusion), exactly as a
  survival resting on no PID-form identity is. The same
  presence hold covers every classification the runner derives
  beside an observed exit: an "output-mismatch" record whose
  exit_code was never observed is named likewise, and its
  output loss is earned nowhere — the runner records the exit
  code from its wait, compares it against the expected code,
  and only on a match classifies a False stream verdict as
  "output-mismatch".
- Fields are paired to the path as outcomes are: on the fast
  path the runner leaves survivor None (it is set only in the
  kill block), and on the kill path every output_* field stays
  null (kill-path slots carry no output observation). A record
  carrying a non-null value in a field its path never writes —
  a fast record with a survivor value, a kill record with any
  output verdict, digest or length — is a record inconsistency
  of the same kind, named in the same idiom. The tree-identity
  fields are paired the same way, in both directions: on the
  fast path child_pid, grandchild_pid and tree_marker stay
  null (they are written only in the kill block — the marker
  is created only for kill slots), so a fast record carrying
  any of them states the identity of a descendant tree its
  path never spawned; and on the kill path tree_marker is
  always written (the slot's own marker path, created before
  the arm spawns), so a kill record whose tree_marker is
  missing, null or not a non-empty string no longer names
  the marker file its survivor judgement rests on, and is
  named the same way — an ok or descendant-survived kill
  record resting on an erased marker earns no survival
  evidence (see _evidenced_survived).
- The freeze gate is stated per dimension (FREEZE_DIMENSIONS, per
  context): the guard must win or tie every competitor median on
  both paths, leave no descendant alive, and produce no exit
  mismatch. Every freeze-gate statement in a report also prints
  the full evaluation profile (PROFILE_DIMENSIONS, the eight
  dimensions of SPEC_Benchmark section 3) with a state per
  dimension: the five gate dimensions map into profile dimensions
  (1) and (2) and carry their PASS/FAIL there; every other
  profile dimension defaults to UNEVALUATED, and may render
  NOT-APPLICABLE only via a decision recorded as package data in
  the run's configuration (campaign.json's
  "profile_dimension_decisions", the configuration whose SHA-256
  run.json records) — never via an edit to this source file.
  While any profile dimension is
  UNEVALUATED the campaign is INCOMPLETE (never a certified
  result) even if the observed dimensions pass — that rule is
  wired into campaign_losses(), the status logic render() and
  the harness's campaign_outcome both use, not only into the
  report text.
- Claim ledger (SPEC_Benchmark section 11): claim_ledger() gives
  every profile dimension one claim — its state
  (observed-passing / observed-failing / UNEVALUATED /
  NOT-APPLICABLE) and its basis, a pointer to evidence inside
  the run package (the freeze gate over observations.jsonl) or
  to the configuration decision and its reason. The harness
  writes the ledger into each run namespace (claim-ledger.json,
  mirrored in run.json); campaign_outcome consumes it — a
  campaign is complete only when every dimension is
  observed-passing or decided NOT-APPLICABLE — and render()
  prints it in full. A campaign whose accounting is incomplete, whose
  floor does not agree in a context, or whose gate is not met on
  every dimension is reported INCOMPLETE with its standing losses
  listed. It is never reported as a result.
- Kill-path correctness is a comparative dimension, evaluated for
  every product arm: an arm is clean when none of its measured kill
  slots left a descendant alive, and losing when at least one did;
  the guard must win or tie it (and the guard's own
  descendant-survived slot fails it outright). A rival arm whose
  measured kill records in a context are all classified
  "descendant-survived" — non-empty, with no "ok" record — has lost
  kill-path correctness there and is excluded from that context's
  kill-latency comparison: an arm that does not kill has no kill
  latency to lose to, so its missing median cannot veto the gate.
  A rival with no kill records at all, or with only
  infra-failure / exit-mismatch kill records, is unevaluated there
  — never converted into a correctness loss for the rival or a
  pass for the campaign. Competitor descendant-survived kill slots
  are listed as competitor losses (evidence of the guard's win on
  that dimension), not as campaign standing losses: they are
  carried in the standing-losses listing under a distinct prefix
  and never block a complete report. The guard's own
  descendant-survived slots remain campaign standing losses. The
  same evidence rule holds for the other components of profile
  dimension (1): a competitor's measured exit-mismatch slots
  (like its measured output-mismatch slots) are competitor
  losses, not campaign standing losses — a competitor's
  measured product fault is evidence of the guard's win on
  dimension (1), while the guard's own exit mismatches remain
  campaign standing losses and fail exit-code correctness.
  (Warmup and control records are not measurements: a fault
  there stays a campaign standing loss for every arm, as an
  infrastructure-failure fault does.)
- Output correctness is a gate dimension of profile dimension
  (1), observed on the fast path: every fast slot's record
  carries, per stream, the emitted bytes' SHA-256 and length and
  a match verdict against the configuration's expected bytes
  (see runner.py). A guard fast slot classified
  "output-mismatch" fails the dimension and stands as a campaign
  loss, in the same class as an exit mismatch; a competitor's
  measured "output-mismatch" fast slots are competitor losses
  (evidence, under the same prefix as competitor kill losses),
  not campaign standing losses. A guard measured fast record
  carrying no output observation leaves the dimension
  UNEVALUATED: dimension (1) reads observed-passing only when
  kill-path correctness, exit-code correctness and output
  correctness all pass. Kill-path slots carry no output
  observation — the kill child's product is the tree it leaves,
  judged by the survivor probe, not a byte stream.
- A match verdict's evidence is held to the bytes it claims:
  the digest beside a verdict must carry the 64-hex form the
  runner's hexdigest writes, and when the expected bytes are
  supplied to the outcome computation (see
  expected_output_from_config — the digest/length pair of the
  run configuration's expected bytes, children.fast's
  expected_stdout / expected_stderr), a True verdict's digest
  and length must equal the expected bytes' digest and length.
  A verdict whose accompanying evidence describes other bytes
  is a record inconsistency of the same kind as the ones above:
  the output gate certifies from the evidence, not from the
  verdict boolean alone.
- Round-27 holds on the collected dimensions: a scale record's
  published descendant count is held to its variant (wide-N
  publishes exactly N; deep-D, levels 1..D, exactly D) in the
  runner's classification and in the ok-record rule alike —
  under-publication is the tree not presented in full, an
  infrastructure failure, never "ok". The resource fields are
  held per record (a present counter must be finite and
  non-negative; an absent one must carry its reason) and the
  resource state is computed per arm, with a reachable FAIL:
  an arm with no numeric measurement of a counter cannot pass
  on another arm's measurements, and the ledger states the
  per-arm coverage a PASS observed — and, round-31 F1 as
  rebuilt round-32 F1, the designated arm must also win or
  tie the cost comparison against every competitor (per
  counter, per (path, context, variant) cell — round-34 F1:
  sorted-rank dominance over equal numeric populations at a
  per-unit coverage floor: the designated arm's sorted values
  at or below the competitor's at every rank — one
  order-invariant criterion, no pairing by rep index, which
  implies the total-cost and mean comparisons at equal
  cardinality), or
  the dimension FAILs on cost; a cell one side alone
  measures, whose populations are unequal, or that a side
  measures on only a minority of its planned records renders
  it UNEVALUATED, named in the profile detail and the ledger
  basis. A malformed
  record's exit code is held present and integer: no code is
  pinned for that path as a whole, so the recorded code is
  the dimension's observation itself — except the designated
  arm's contract-pinned shapes (round-31 F2; round-32 F2: the
  shape as its SLOT encodes it, warmup and measured alike),
  whose records are held to the pinned refusal code, 125, and whose
  violation FAILs dimension (6). Dimension (8)'s state is
  computed from the manual
  artifact it names (usability-evidence.json): "collected"
  requires at least one prompt class exercised with an
  observed outcome recorded — round-28: the entry must name
  one of the artifact's declared prompt classes, carry the
  boolean exercised=true, record a non-null observed outcome,
  and carry an outcome in the declared vocabulary (pass /
  fail / not-run) other than "not-run"; the as-shipped
  all-"not-run" artifact renders the dimension UNEVALUATED
  in every ledger.
"""

import hashlib
import json
import math
import os
import re
import statistics
from collections import Counter
from collections.abc import Mapping

from arms import expected_exit, expand_template, \
    materialized_candidate_pending, \
    materialized_child_args_proven, materialized_child_args_stated, \
    materialized_list_proven
from plan import CONTEXTS, CONTROL_REPS, DESIGNATED_ARM, \
    MALFORMED_SHAPES, MEASURED_REPS, PRODUCT_ARMS, WARMUP_REPS, \
    build_plan, \
    expected_descendants, malformed_expected_exit, \
    malformed_pin_violation, scale_count_matches, \
    scale_distinct_count, slot_variant, variant_for

# Census anchors: the vendored enumeration of MEASUREMENT.md
# section 2's named calls in campaign/census.json (this package) —
# plain run path 45 named calls entry-to-exit, hook verdict path
# 18, and the run-core subtotal (job setup to process exit) 36 —
# recomputed from that enumeration and enforced by the
# counted-ops census tests in tests/test_campaign_logic.py
# (CensusConsistencyTests), which also re-derive the enumeration
# from the guard tree's MEASUREMENT.md section 2 when the tree is
# present beside the suite.
CENSUS = {"plain_total": 45, "hook_total": 18, "subtotal_run_core": 36}

STALE_PAIRING_PROVENANCE = (
    "The pairing \"66.5 ms vs 50.9 ms\" (N=5) cites a 66.5 ms anchor that "
    "predates the gauntlet: it is the retained pre-gauntlet binary, not "
    "the current gauntlet binary. It is recorded here for provenance "
    "only and is not a measurement of the current tree."
)

NUEWFRAME_KILL_FACTS = (
    "On Windows, nuewframe/timeout does not kill descendants at all: "
    "its force step is TerminateProcess on the direct child handle "
    "only. Its first timeout action is a 5 s wait (the kill_after "
    "default) behind a graceful step that is unimplemented on Windows "
    "(it logs that graceful termination is not implemented on Windows "
    "and waits for kill-after). In the preserved bundle's kill trials "
    "it killed nothing: the child outlived the 2 s deadline and died "
    "naturally inside the grace window."
)

# The expensive class: the only items a decomposition may price.
EXPENSIVE_CLASS = ("console probe + console finish", "owner verification",
                   "resolver probe count")

# Floor agreement tolerance, re-derived from the bundle's own floor
# observation: the two minimal tools measured 50.9 ms and 52.5 ms on
# the bundle's machine — a spread of 1.6 ms, which is 3.0% of the
# larger median. The tolerance is set at 5%: it admits the observed
# agreement with headroom for run-to-run variation on one machine,
# and it is half the previous 10%, which would have called two tools
# 5 ms apart at this scale "agreeing" when the bundle's tools in
# fact agree within 1.6 ms.
FLOOR_AGREEMENT_TOLERANCE = 0.05

# The widest PID a record may carry: a process identifier the
# runner records (_read_pid reads one from a marker file) is a
# positive integer within the platform's PID space. No narrower
# bound is named anywhere in this suite, so the record-consistency
# discipline holds the tree-identity fields to the widest signed
# PID space, 2**31 - 1: a positive integer no larger than this is
# a PID the platform can assign; zero, a negative value, or a
# value above it is not an identity any descendant ever carried.
MAX_PID = 2 ** 31 - 1


def _is_pid_form(value):
    """True when a value is a PID in form as well as in type: a
    positive integer (never a boolean) within the platform PID
    range, 1 to MAX_PID — an identity a descendant the survivor
    probe judged could actually have carried."""
    return isinstance(value, int) and not isinstance(value, bool) \
        and 1 <= value <= MAX_PID

# The freeze gate's dimensions, evaluated per context. The gate is
# met only when every dimension passes in every context.
FREEZE_DIMENSIONS = ("fast-path latency", "kill-path latency",
                     "kill-path correctness", "exit-code correctness",
                     "output correctness")

# The full evaluation profile: the eight dimensions of
# SPEC_Benchmark section 3, enumerated in that order. Every
# freeze-gate statement in a report prints this full enumeration
# with a state per dimension (see profile_dimension_states()).
# The phrase "every applicable dimension" may appear only
# together with this enumeration.
PROFILE_DIMENSIONS = (
    "Correctness and output quality",
    "Latency, throughput, and startup time",
    "CPU, memory, storage, network, energy, and monetary cost",
    "Scaling, concurrency, and resource exhaustion",
    "Reliability, prolonged operation, recovery, and data integrity",
    "Security boundaries and resistance to malformed inputs",
    "Compatibility, portability, installation, and integration",
    "Usability and accessibility for interactive products",
)

# Profile-dimension decisions are package data, not source: they
# live in the shipped campaign.json under this key, mapping a
# dimension's exact PROFILE_DIMENSIONS string to the reason
# string that prints in the report beside the dimension. The
# configuration is the file whose SHA-256 run.json records for
# the run, so a decision is traceable from the run package
# itself; editing this source file decides nothing, and no
# shipped path reads a decisions table from here. A decision
# decides a dimension away only when its reason is a non-empty
# string — an entry without one decides nothing (see
# _decision_reason).
DECISIONS_CONFIG_KEY = "profile_dimension_decisions"

# Claim-ledger states (SPEC_Benchmark section 11): a dimension
# the gate observes is observed-passing or observed-failing; a
# dimension carried by a configuration decision is
# NOT-APPLICABLE; anything else is UNEVALUATED.
LEDGER_OBSERVED_PASSING = "observed-passing"
LEDGER_OBSERVED_FAILING = "observed-failing"
LEDGER_UNEVALUATED = "UNEVALUATED"
LEDGER_NOT_APPLICABLE = "NOT-APPLICABLE"
# Round-26: the state of a dimension collected by a named manual
# instrument (dimension (8)) — collected, neither observed by the
# campaign nor decided away.
LEDGER_COLLECTED = "collected"


def decisions_from_config(config):
    """The profile-dimension decisions a configuration carries:
    the mapping under DECISIONS_CONFIG_KEY, or {} when the
    configuration carries none (or carries a non-mapping there,
    which decides nothing). The returned mapping is the run's
    decisions; callers pass it explicitly to
    profile_dimension_states(), claim_ledger(),
    campaign_losses() and render()."""
    if isinstance(config, dict):
        decisions = config.get(DECISIONS_CONFIG_KEY)
        if isinstance(decisions, dict):
            return dict(decisions)
    return {}


# Round-26 collected dimensions: an entry in the same mapping
# whose reason begins with this prefix does not decide its
# dimension away — it declares the dimension COLLECTED and names
# the collecting instrument after the prefix. The shipped
# configuration carries such an entry for dimensions (3), (4),
# (6) and (8): (3), (4) and (6) are observed from the run's own
# records (the runner's resource fields; the plan's scaling
# slots; the plan's malformed slots), so their states are
# computed from those records below and a run whose records do
# not carry the collection renders them UNEVALUATED, exactly as
# an unobserved dimension — the entry names the instrument, it
# does not mint the evidence. Dimension (8)'s instrument is the
# formalized manual usability check: its evidence is recorded in
# the suite artifact the entry names, tier-labelled manual, and
# its state is computed from that artifact (round-27 F2;
# round-29 F1: THREE-VALUED — a qualifying "fail" renders FAIL /
# observed-failing with a standing loss, a qualifying "pass"
# with no fail renders "collected", and no qualifying entry
# renders UNEVALUATED); while every recorded outcome stands at
# "not-run" (or the artifact is absent or unreadable) it renders
# UNEVALUATED — the honest not-yet-collected state, in the same
# vocabulary the other unmeasured dimensions render — never
# "collected" from the configuration's text alone.
COLLECTED_PREFIX = "COLLECTED:"

DIM_RESOURCE = "CPU, memory, storage, network, energy, and monetary cost"
DIM_SCALING = "Scaling, concurrency, and resource exhaustion"
DIM_SECURITY = "Security boundaries and resistance to malformed inputs"
DIM_USABILITY = "Usability and accessibility for interactive products"


def _collection_entry(dimension, decisions=None):
    """The collecting instrument a configuration declares for one
    dimension (the text after COLLECTED_PREFIX in its entry), or
    None when the dimension carries no collection declaration."""
    if not isinstance(decisions, dict):
        return None
    entry = decisions.get(dimension)
    if isinstance(entry, str) and entry.startswith(COLLECTED_PREFIX):
        instrument = entry[len(COLLECTED_PREFIX):].strip()
        if instrument:
            return instrument
    return None


def _is_number(value):
    return isinstance(value, (int, float)) \
        and not isinstance(value, bool)


# Dimension (8)'s evidence artifact (round-27 F2): the suite file
# the dimension's collection entry names, read by the campaign
# code itself — the ledger's "collected" is computed from this
# artifact's records, never from the configuration's text alone.
USABILITY_ARTIFACT = os.path.normpath(os.path.join(
    os.path.dirname(os.path.abspath(__file__)), os.pardir,
    "usability-evidence.json"))


def _usability_artifact_records():
    """The prompt-class records of the usability artifact, or []
    when the artifact is absent, unreadable or carries no record
    list — an absent artifact collects nothing."""
    try:
        with open(USABILITY_ARTIFACT, encoding="utf-8") as handle:
            artifact = json.load(handle)
    except (OSError, ValueError):
        return []
    if isinstance(artifact, dict) \
            and isinstance(artifact.get("records"), list):
        return [record for record in artifact["records"]
                if isinstance(record, dict)]
    return []


# The usability instrument's declared structure (round-28 F1):
# the prompt classes the artifact declares — the three classes
# of usability-evidence.json — and the outcome vocabulary its
# schema declares. Both are stated in the artifact's own schema
# and in campaign.json's dimension (8) entry, and the shipped
# artifact's structure test holds the three statements equal.
USABILITY_PROMPT_CLASSES = (
    "Windows Hello request",
    "credential dialog",
    "console password prompt",
)
USABILITY_OUTCOME_VOCABULARY = ("pass", "fail", "not-run")

# The three-valued rule for dimension (8) (round-29 F1, reversing
# the round-28 landing's choice on the record): collection and
# verdict are separate, as on dimensions (3)/(4)/(6). This parsed
# structure is the rule the equality test holds across code, the
# artifact's schema (usability-evidence.json's collected_rule)
# and campaign.json (its usability_collected_rule) — parsed
# structures, not substrings (round-29 F5).
USABILITY_COLLECTED_RULE = {
    "prompt_classes": list(USABILITY_PROMPT_CLASSES),
    "outcome_vocabulary": list(USABILITY_OUTCOME_VOCABULARY),
    "qualifying_entry": {
        "prompt_class": "one of prompt_classes",
        "exercised": True,
        "observed": "recorded (a non-empty string)",
        "outcome": "in outcome_vocabulary and not 'not-run'",
    },
    "states": {
        "any_qualifying_fail": "FAIL",
        "qualifying_pass_and_no_fail": "COLLECTED",
        "no_qualifying_entry": "UNEVALUATED",
    },
    "fail_effect": {
        "profile_state": "FAIL",
        "ledger_state": "observed-failing",
        "standing_loss": True,
        "fail_dominates_pass": True,
    },
}

# The same rule as one sentence, stated verbatim in
# campaign.json's dimension (8) entry and in VERIFICATION.md:
USABILITY_COLLECTED_RULE_TEXT = (
    "Dimension (8) is three-valued: a qualifying entry (a "
    "declared prompt class, exercised as the boolean true, an "
    "observed outcome recorded) with outcome 'fail' renders "
    "the dimension FAIL \u2014 profile state FAIL, ledger state "
    "observed-failing, and a standing loss, and a fail "
    "dominates any pass; qualifying entries with outcome "
    "'pass' and no fail render COLLECTED; and no qualifying "
    "entry renders UNEVALUATED. The outcome vocabulary stays "
    "pass, fail, not-run \u2014 what changes is fail's effect: a "
    "fail is collected evidence recorded as the loss it is, "
    "never as the dimension satisfied.")


def _usability_state():
    """Dimension (8)'s three-valued state (round-29 F1):
    "FAIL" when any qualifying entry (declared prompt class,
    exercised the boolean True, observed recorded) carries
    outcome "fail" — a fail dominates any pass; "COLLECTED"
    when a qualifying entry carries "pass" and none carries
    "fail"; "UNEVALUATED" when no entry qualifies. Collection
    is the existence of qualifying evidence; the verdict on
    that evidence is this separate state, exactly as the
    sibling collected dimensions keep the two apart."""
    outcomes = []
    for record in _usability_artifact_records():
        if record.get("prompt_class") not in \
                USABILITY_PROMPT_CLASSES:
            continue
        if record.get("exercised") is not True:
            continue
        observed = record.get("observed")
        if not (isinstance(observed, str) and observed.strip()):
            continue
        outcome = record.get("outcome")
        if outcome in USABILITY_OUTCOME_VOCABULARY \
                and outcome != "not-run":
            outcomes.append(outcome)
    if "fail" in outcomes:
        return "FAIL"
    if outcomes:
        return "COLLECTED"
    return "UNEVALUATED"


def _usability_collected():
    """True when the usability artifact records a collection,
    under the rule its own documents state exactly (round-28
    F1): an entry counts iff (i) it names one of the prompt
    classes the artifact declares (USABILITY_PROMPT_CLASSES);
    (ii) its exercised field is the boolean True — the schema
    defines the field as a boolean, so a missing field, False,
    or a descriptive string never counts; (iii) an observed
    outcome is recorded — observed is a non-empty string (the
    schema's recorded observation, never null); and (iv) its
    outcome belongs to the outcome vocabulary the artifact
    declares (USABILITY_OUTCOME_VOCABULARY), excluding
    "not-run". Unknown outcome strings, invented, empty or
    missing prompt classes, and missing/false/string exercised
    values mint nothing. The artifact as shipped records every
    outcome "not-run" with nothing observed, so the dimension
    stands not yet collected. This is the COLLECTION test
    only (round-29 F1): a qualifying "fail" is collected
    evidence too — its verdict is _usability_state()'s FAIL,
    never this function's COLLECTED-as-satisfied."""
    for record in _usability_artifact_records():
        if record.get("prompt_class") not in \
                USABILITY_PROMPT_CLASSES:
            continue
        if record.get("exercised") is not True:
            continue
        observed = record.get("observed")
        if not (isinstance(observed, str) and observed.strip()):
            continue
        outcome = record.get("outcome")
        if outcome in USABILITY_OUTCOME_VOCABULARY \
                and outcome != "not-run":
            return True
    return False


def _scale_variant_of(record):
    """The scale variant of a record AS THE PLAN ENCODES IT
    (round-33 F1, reversing the round-32 narrowing): the variant
    its slot identity (path, rep, kind) re-derives from the plan
    — plan.slot_variant — never the record's self-carried label.
    A record's carried variant is a claim about its slot, held
    against this derivation by the record-consistency discipline
    (a carried label that disagrees with the slot's encoded
    shape is named as a variant inconsistency on the scale path
    exactly as on the malformed path); it is never the authority
    a verdict reads. The carried label is the fallback only when
    the slot identity derives no variant at all (a rep of no
    usable form) — deriving it must never raise in a checker.
    Round-34 F2: that fallback is a keying fallback only — a
    rep that derives nothing because it is not a non-bool
    integer in its band is NAMED as a rep inconsistency by
    the record-consistency discipline itself (see
    _rep_well_formed), so no verdict consumer reads the
    fallback over an unnamed record."""
    derived = slot_variant("scale", record.get("rep"),
                           record.get("kind"))
    if derived is not None:
        return derived
    return record.get("variant")


def _distinct_descendant_count(record):
    """The distinct descendant count a scale record publishes
    (round-28 F2; round-29 F4: VARIANT-AWARE). The exclusion
    matches the runner's shapes per variant:

    - WIDE variants: the runner records the tree's root
      separately in child_pid (from the wide child's child.pid
      file; its ids file carries the N descendants), so the
      published set excludes BOTH roots — the record's arm_pid
      (the arm process itself) and its child_pid — and the
      count is that set's distinct cardinality.
    - DEEP variants: the published list carries the level-1
      root itself (the deep child's per-level files all land
      in descendant_pids, level 1 included, and child_pid
      stays None in the runner's deep records), so the count
      is the list's distinct cardinality, the arm_pid alone
      excluded; a root ALSO named in child_pid neither adds
      nor subtracts — it is the same level-1 PID the list
      already carries, counted once there.

    A PID repeated any number of times counts once; a root
    the variant excludes, offered as a descendant, counts
    zero times. Returns None when descendant_pids is not a
    list at all. The arithmetic is plan.scale_distinct_count's
    (round-30 F1: one formulation, shared with the runner)."""
    return scale_distinct_count(
        _scale_variant_of(record), record.get("descendant_pids"),
        record.get("arm_pid"), record.get("child_pid"))


def _scale_count_matches(record, owed):
    """The scale count hold (round-29 breadth; round-30 F1: the
    ONE shared predicate — plan.scale_count_matches, the same
    function the runner's classification calls): the published
    list's length equals the variant's raw published count AND
    that count is distinct descendant PIDs under the
    variant-aware exclusion (see _distinct_descendant_count)
    — a list of the right length made of one PID repeated, or
    of an excluded root, published no tree of that size, and a
    list carrying an excluded root beside the full tree is
    over-published in length and matches in neither place."""
    variant = _scale_variant_of(record)
    return expected_descendants(variant) == owed \
        and scale_count_matches(
            variant, record.get("descendant_pids"),
            record.get("arm_pid"), record.get("child_pid"))


def _is_finite_number(value):
    """Total finiteness for a non-boolean number (round-29
    F6): an int too large to convert to float (10**400, which
    JSON parses) is NOT finite for a platform counter — the
    check names it instead of raising OverflowError, as
    math.isfinite does on such an int. The checker cannot
    crash on a record: every value the predicates touch —
    huge, non-finite, wrong-typed — resolves to a boolean."""
    if not _is_number(value):
        return False
    try:
        return math.isfinite(value)
    except OverflowError:
        return False


def _is_plausible_resource(value):
    """A resource counter a platform can produce: a finite,
    non-negative number (never a boolean). A negative or NaN
    CPU time or working set is a shape no counter produces.
    cpu_ms == 0 stays admissible under this rule (rusage
    granularity makes an exact zero CPU time merely
    implausible, not impossible — round-28 F3 charges only
    the peak). Total (round-29 F6): a huge int is named
    implausible, never an exception."""
    return _is_finite_number(value) and value >= 0


def _is_plausible_peak(value):
    """A peak working set a platform can produce (round-28
    F3; round-29 F2: TYPE binds as well). Both of the
    runner's peak sources produce an INTEGER — int(peak)
    from the rusage ru_maxrss (kilobytes scaled) on POSIX,
    int(counters.peak_working_set) from a c_size_t on
    Windows — so a present peak must be integer-valued (an
    int, or a float with no fractional part) AND finite AND
    strictly positive. A fractional or subnormal peak (0.5
    bytes, 5e-324) is a shape no honest record carries,
    exactly as a peak of 0 is: no spawned process has a peak
    working set of 0 bytes, and no counter yields half a
    byte. Such a value is held as an inconsistency per
    record (see _record_inconsistencies), never as an
    absent-with-reason value and never as numeric coverage.
    Total (round-29 F6): a huge int is named implausible,
    never an exception."""
    if not _is_finite_number(value) or value <= 0:
        return False
    if isinstance(value, int):
        return True
    return float(value).is_integer()


def _resource_field_plausible(field, value):
    """The plausibility rule for one resource field: the peak
    working set must be integer-valued, finite and strictly
    positive (round-28 F3; round-29 F2); cpu_ms finite and
    non-negative."""
    if field == "peak_working_set_bytes":
        return _is_plausible_peak(value)
    return _is_plausible_resource(value)


# The runner's CLOSED collection-failure vocabulary
# (round-35 F1): every resource_note string the runner's
# note-generation sites can write (campaign/runner.py — the
# record construction's not-spawned note, the spawn failure's
# never-spawned note, the POSIX not-reaped note, the two
# Windows counter-call failure notes, the no-accounting
# platform note, and the one parameterized family, the
# Windows exception note "resource counters unavailable:
# {error}", admitted as its exact prefix with a non-empty
# suffix). A null counter value is accounted ONLY by a note
# from this vocabulary: any other note, however plausible —
# including one that declares the censoring in words — is a
# string no runner writes, and a record carrying it is
# UNACCOUNTED in the verdict layer (round-35 P1: the note
# "banana", and a self-declared censoring note, both
# accounted a null field before this landing).
#
# The vocabulary is PAIRED PER FIELD (round-36 F2): membership
# alone does not account a null — the note must be the one the
# runner's construction pairs with THAT field's failure. Read
# off runner.py's write sites, the pairing is a two-row table:
# every vocabulary note except one accompanies BOTH counters
# standing null (the GetProcessTimes failure returns (None,
# None); the parameterized Windows exception note returns
# (None, None); the not-spawned, never-spawned, not-reaped and
# no-accounting notes all stand over a record whose counters
# were never written, so both stand null), and the ONE
# single-null shape the runner ever writes is a peak null
# beside a numeric cpu under the GetProcessMemoryInfo note
# (that call's failure returns (cpu, None)). A null in any
# other note/field shape — a cpu null under the
# GetProcessMemoryInfo note, a lone null under a both-null
# note, both nulls under the peak-only note — is a shape no
# runner writes, and the field it stands on is UNACCOUNTED
# (round-36 P2: all four shapes accounted both fields before
# this landing, on measured and warmup records alike).
RESOURCE_NOTE_VOCABULARY = frozenset({
    "arm not spawned yet; no process accounted",
    "arm never spawned; no process to account",
    "arm not reaped; resource counters unavailable",
    "GetProcessTimes failed on this arm",
    "GetProcessMemoryInfo failed on this arm",
    "platform exposes no per-child resource accounting "
    "the runner can read (no wait4)",
})
RESOURCE_NOTE_PREFIXES = ("resource counters unavailable: ",)

# The one single-field note (round-36 F2): the note the runner
# pairs with a PEAK null alone — GetProcessMemoryInfo failing
# after GetProcessTimes succeeded, so the record carries a
# numeric cpu beside the null peak. It accounts no cpu null,
# in any shape.
RESOURCE_NOTE_PEAK_ONLY = "GetProcessMemoryInfo failed on this arm"


def _resource_note_in_vocabulary(note):
    """True when a resource_note is one of the runner's own
    collection-failure notes (the closed vocabulary above):
    an exact vocabulary string, or the parameterized Windows
    exception note with a non-empty error suffix. Total: a
    non-string note is in no vocabulary."""
    if not isinstance(note, str):
        return False
    if note in RESOURCE_NOTE_VOCABULARY:
        return True
    return any(note.startswith(prefix)
               and len(note) > len(prefix)
               for prefix in RESOURCE_NOTE_PREFIXES)


def _resource_field_accounted(record, field):
    """True when one resource field of a record is accounted
    in a form the runner actually writes (round-29 F3;
    round-35 F1; round-36 F2). The runner constructs EVERY
    record with both resource keys present (runner.py's
    record construction) and only ever varies their VALUES:
    a plausible counter, or null with a resource_note naming
    why — and the note must be one of the runner's own
    collection-failure notes (the closed vocabulary at
    RESOURCE_NOTE_VOCABULARY: the infra states — arm not
    spawned / never spawned / not reaped — recorded by note,
    and the platform counter failures it records by note on
    a completed slot: a Windows call failing, or a platform
    with no per-child accounting). A null carrying any other
    note, or none, is UNACCOUNTED. A field whose KEY is
    missing is therefore accounted in no state — with or
    without a retained note, on an infra record as on a
    completed measurement — the runner never deletes a key.

    Round-36 F2: the accounting is PER FIELD — the note must
    be the one the runner pairs with this field's failure
    (the pairing at RESOURCE_NOTE_VOCABULARY). The
    GetProcessMemoryInfo note accounts a PEAK null only, and
    only beside a plausible cpu value — the one single-null
    shape the runner writes; every other vocabulary note
    accounts a null only when BOTH counters stand null, the
    shape each of those notes accompanies at its write site.
    A null in any other note/field shape is UNACCOUNTED, on
    measured, warmup and control records alike."""
    if field not in record:
        return False
    value = record.get(field)
    if _resource_field_plausible(field, value):
        return True
    if value is not None:
        return False
    note = record.get("resource_note")
    if not _resource_note_in_vocabulary(note):
        return False
    if note == RESOURCE_NOTE_PEAK_ONLY:
        return field == "peak_working_set_bytes" \
            and _resource_field_plausible(
                "cpu_ms", record.get("cpu_ms"))
    other = ("peak_working_set_bytes" if field == "cpu_ms"
             else "cpu_ms")
    return other in record and record.get(other) is None


_PLANNED_KEYS_CACHE = {}


def _planned_slot_keys():
    """The slot identities build_plan() carries, as a set of
    (kind, arm, path, context, rep) tuples — the slot-accounting
    identity, variant-blind exactly as that accounting is
    (round-40 F3). Read through this module's build_plan name,
    so a caller that substitutes the plan (the gate's own
    probes do) substitutes it for every consumer; the set is
    cached against the build_plan FUNCTION OBJECT in force, so
    a substitution — a different function object — recomputes
    it, and the unsubstituted plan is built once."""
    cached = _PLANNED_KEYS_CACHE.get("entry")
    if cached is not None and cached[0] is build_plan:
        return cached[1]
    keys = {(slot.kind, slot.arm, slot.path, slot.context,
             slot.rep) for slot in build_plan()}
    _PLANNED_KEYS_CACHE["entry"] = (build_plan, keys)
    return keys


def _record_matches_a_plan_slot(record, planned_keys=None):
    """True when a record's slot identity — (kind, arm, path,
    context, rep), never its carried variant label — is one the
    plan carries (round-40 F3). Total: an identity carrying an
    unhashable component matches no slot. NOTE (round-41 F1):
    this is a MEMBERSHIP test — it states that the plan carries
    the record's slot, not that this record is one of the
    slot's planned occurrences; the planned-population test is
    _planned_occurrence_flags, which is multiplicity-exact."""
    key = (record.get("kind"), record.get("arm"),
           record.get("path"), record.get("context"),
           record.get("rep"))
    try:
        return key in (planned_keys if planned_keys is not None
                       else _planned_slot_keys())
    except TypeError:
        return False


_PLANNED_COUNTS_CACHE = {}


def _planned_slot_counts():
    """The plan's slot identities as a Counter — how many
    records of each (kind, arm, path, context, rep) identity
    build_plan() carries (round-41 F1: slot accounting's own
    population semantics — exactly the plan's count per slot,
    one per slot in the shipped plan). Cached against the
    build_plan FUNCTION OBJECT in force, exactly as
    _planned_slot_keys caches its set, so a substituted plan
    substitutes here too."""
    cached = _PLANNED_COUNTS_CACHE.get("entry")
    if cached is not None and cached[0] is build_plan:
        return cached[1]
    counts = Counter(
        (slot.kind, slot.arm, slot.path, slot.context, slot.rep)
        for slot in build_plan())
    _PLANNED_COUNTS_CACHE["entry"] = (build_plan, counts)
    return counts


def _planned_occurrence_flags(records):
    """One boolean per record, in record order: True when the
    record is one of its slot's PLANNED occurrences — among
    the first build_plan()-count records carrying its slot
    identity (round-41 F1). The round-40 filter admitted by
    set membership, so a second copy of one planned slot was
    admitted at full multiplicity and reproduced every effect
    round 40's P3 charged against records in no slot at all
    (a median moved, an earned FAIL softened at the identical
    coverage, a pricing loss erased at the identical
    distance): slot accounting's population is a Counter —
    exactly the plan's count per slot — and a record beyond
    that count is a record the plan does not contain either.
    Total: a record whose identity carries an unhashable
    component is no slot's occurrence."""
    counts = _planned_slot_counts()
    seen = Counter()
    flags = []
    for record in records:
        key = (record.get("kind"), record.get("arm"),
               record.get("path"), record.get("context"),
               record.get("rep"))
        try:
            occurrence = seen[key] < counts.get(key, 0)
            seen[key] += 1
        except TypeError:
            occurrence = False
        flags.append(occurrence)
    return flags


def _planned_records(records):
    """The planned population every evidence consumer reads:
    the records that are planned occurrences of their slots
    (round-40 F3, multiplicity-exact round-41 F1 — see
    _planned_occurrence_flags). A record that is no planned
    occurrence — one matching no plan slot, or a copy beyond
    its slot's planned count — exists in the verdict only as
    named incompleteness (slot accounting names it, duplicates
    included; the (3) gate renders out-of-plan cells
    UNPLANNED); it is not evidence, and it moves no coverage
    count, no median, no floor, no kill state and no
    competitor-loss entry."""
    flags = _planned_occurrence_flags(records)
    return [record for record, planned in zip(records, flags)
            if planned]


def _resource_coverage(records):
    """Per-arm numeric coverage of the measured records, for the
    resource dimension's computed state and its ledger text:
    {arm: (measured_count, numeric_cpu_count, numeric_peak_count)}
    counting only plausible values as measurements (cpu_ms
    finite and non-negative; peak_working_set_bytes
    integer-valued, finite and strictly positive — round-28
    F3: a peak of 0 counts toward no arm's numeric coverage;
    round-29 F2: a fractional peak counts toward none
    either). Round-40 F3: only records matching a plan slot
    count — an unplanned record supplies no arm's coverage,
    so one fabricated wide-999 record no longer softens the
    FAIL an arm's planned records earn by carrying no numeric
    counter."""
    coverage = {}
    for record in _planned_records(records):
        if record.get("kind") != "measured":
            continue
        arm = record.get("arm")
        total, cpu, peak = coverage.get(arm, (0, 0, 0))
        coverage[arm] = (
            total + 1,
            cpu + (1 if _is_plausible_resource(record.get("cpu_ms"))
                   else 0),
            peak + (1 if _is_plausible_peak(
                record.get("peak_working_set_bytes")) else 0))
    return coverage


RESOURCE_COUNTERS = ("cpu_ms", "peak_working_set_bytes")


def _paired_cost_comparison_detail(records):
    """The arm-comparison verdict of dimension (3) (round-31
    F1, rebuilt round-32 F1, rebuilt round-33 F3, rebuilt
    round-34 F1; censoring held round-35 F1; the magnitude
    return added round-35 F4; the unit enumeration made
    PLAN-DRIVEN and the censored count bound to accounted
    nulls, round-36 F1/F4). No statistic here pairs records, and no
    verdict in this function can move under a relabelling of
    repetitions.

    The statistic, stated exactly, derived from SPEC_Benchmark
    section 3 (dimension (3) is the cost dimension) and
    section 7 (the freeze gate: win or tie on every applicable
    dimension): the comparison unit is one (competitor,
    counter, path, context, variant) cell — the variant is
    part of the unit, so records of different shapes (the
    scale variants above all) are never pooled into one
    comparison, and on the scale path the variant a unit is
    keyed by is the one the record's SLOT encodes in the plan
    (round-33 F1: plan.slot_variant, as _scale_variant_of
    derives it), never the record's carried label. Inside a
    unit, each arm contributes the plausible numeric values
    of the counter on its measured records in that cell (the
    plausibility predicates of the coverage hold).

    Cardinality honesty (round-33 F3): the comparison is made
    only over EQUAL numeric populations. A unit whose two
    sides carry different numeric counts is not compared at
    all — it is NAMED in the returned unpaired list (the
    competitor, the counter, the cell and the variant, with
    each side's numeric count and planned count) and the
    dimension renders UNEVALUATED over it, exactly as a
    one-sided unit is: a comparison over unequal populations
    compares counts, not costs. A per-unit numeric-coverage
    floor stands beside it: an arm numeric on only a MINORITY
    of its planned records in the unit — its numeric count
    less than half the unit's PLANNED population (build_plan()'s
    count for the cell, the same anchor as the enumeration
    below — round-38 F1: the floor follows the plan, as its
    own unpaired string has always stated it; rounds 36–37
    bound it to the records a side CARRIES in the cell, so a
    unit whose records were 89% deleted compared on the
    surviving tenth) — renders the unit unpaired likewise,
    even when both sides are equally sparse: a comparison
    made on a minority of the unit's slots evaluates nothing
    about the unit. A cell the plan carries no slot for is
    not a comparison population at all (round-39 F2): its
    records render as UNPLANNED in the unpaired list, with
    the plan count stated as 0 — never compared, and never
    rendered with a planned count taken from the records
    it carries (rounds 36–38 fell back to the carried
    count there, so one fabricated record per arm in an
    invented variant compared as a "1 planned (1/1)"
    population the plan does not contain). Round-40 F1:
    the cell a record keys into is derived from its SLOT
    on every path, never read from its carried variant
    label — a malformed case-relabel renders as its
    planned cell's records (the label is named once, in
    the variant channel), and a fast record's carried
    string manufactures no cell, fast slots encoding no
    variant. Round-40 F2: the plan-0 rule binds per
    (cell, arm) — a zero-planned arm's records are
    UNPLANNED even inside a planned cell, and no compared
    unit renders a count over a zero denominator.
    Round-41 F1: the values and censored counts a unit
    compares are the MULTIPLICITY-EXACT planned
    population's — a copy beyond its slot's planned count
    contributes no value and no censored count (the
    round-40 membership filter admitted copies at full
    multiplicity); carried records beyond the planned
    population are counted only where the UNPLANNED
    branches below state how many records stand in a cell.

    At equal, floor-passing populations the comparison is
    SORTED-RANK DOMINANCE (round-34 F1), per counter
    (cpu_ms and peak_working_set_bytes alike): the designated
    arm's sorted numeric values must be at or below the
    competitor's at EVERY rank — equality at a rank is a tie
    at that rank, and a designated value above the
    competitor's at any rank is a cost loss naming the
    counter, the failing ranks and the values at them. This
    single order-invariant criterion is the whole statistic:
    at equal cardinality a vector that is rankwise at or
    below another has its total, its mean, its median, its
    maximum and every quantile at or below the other's, so
    total-cost sensitivity (round-32) and peak-mean
    sensitivity (round-34 P3) are implied by full-vector
    dominance with no aggregate beside it — the rounds 32–34
    regress, in which each round's aggregate set was blind
    one ring further out (the median to the total, the pair
    to the tail, the triple to the band between the median
    and the maximum), ends here because no band of the
    distribution is left unjudged.

    No tolerance band widens any tie: section 7's gate is
    win-or-tie, and the gate's latency discipline admits no
    band either. Dominance is a function of each arm's
    multiset of values in the cell alone — no pairing by rep
    index across independent runs exists, so permuting any
    arm's repetition labels cannot move the verdict; every
    shape's standing is judged in its own unit.

    Censoring (round-35 F1): a unit's numeric population is
    not the unit's population — the records whose counter
    stands null (accounted, when accounted at all, only by
    a collection-failure note from the runner's closed
    vocabulary) are CENSORED, and dominance over the numeric
    remainder certifies the censored remainder only under
    an assumption nothing in the record supports, because
    a counter's failure is free to track the cost it failed
    to measure. The comparison is therefore held to the
    worst-case imputation under which every censored value
    is resolved AGAINST the designated arm's claim: a
    censored designated value stands at +infinity (no field
    the record carries bounds it above — the peak carries
    no bound in the record at all, and elapsed_ms is wall
    time, which a multi-threaded arm's cpu can exceed), and
    a censored competitor value stands at the counter's
    minimum. Under that imputation a unit carrying any
    censored record on either side for a counter cannot
    certify dominance over its full planned population, so
    such a unit — past the one-sided, coverage-floor and
    cardinality gates above, whose namings it keeps — is
    rendered in the unpaired list as a CENSORED unit, with
    both sides' censored counts: never compared, never a
    loss, and the dimension UNEVALUATED over it. The
    consequence is stated, not absorbed: any accounted
    null in a unit flips that unit, however sparse the
    censoring — a PASS certifies only units whose every
    planned record was measured on that counter.

    Enumeration (round-36 F1, re-anchored round-37 F1):
    the units are enumerated from the PLANNED population
    build_plan() carries — every (path, context, variant)
    cell of the plan's measured slots, variant via the
    slot's own encoding — unioned with the cells the
    measured records carry, whether or not any record, or
    any plausible value of the counter, survives in a cell.
    Round 36 built the planned map from the judged records
    themselves, so a cell whose records were DELETED
    entirely (not censored) vanished exactly as a fully
    censored cell had before it, and the dimension PASSed
    over it; round 37's reviewer executed both deletions
    (all 360 measured fast/DETACHED records; all 720
    measured kill records) against that anchor. A planned
    unit with no records on a side now renders in the
    unpaired list at the one-sided gate with its planned
    counts FROM THE PLAN and zero present — never absent,
    never PASS-eligible. The planned counts the unit
    strings state are the plan's counts for the cell —
    0 for a cell the plan does not carry, whose records
    render UNPLANNED and uncompared (round-39 F2); the
    coverage floor beside them binds the SAME
    planned population (round-38 F1 — rounds 36–37 bound it
    to the records a side carries in the cell, and round
    38's reviewer executed the split: 10 numeric records
    surviving of 90 planned compared and PASSed, because a
    side numeric on all it carries is numeric on more than
    half of what it carries at 11% of what the plan
    carries; on every genuine campaign set the two
    populations coincide, because slot accounting holds a
    campaign's records to the plan exactly). Before round
    36 the loop iterated the cells
    built from plausible values alone, so a cell censored
    on EVERY record of both arms produced no entry at all:
    six units vanished, the dimension PASSed over them and
    the campaign rendered COMPLETE. A unit with zero
    plausible values on either side renders in the unpaired
    list at the one-sided gate, with its planned counts
    and, where its records stand null under an accounting
    note, its censored counts — never absent, never
    PASS-eligible.

    Censored counting (round-36 F4): a record counts toward
    a unit's censored count for a counter only when the
    counter's key is PRESENT, its value is None and that
    null is accounted FOR THAT FIELD under the per-field
    pairing (_resource_field_accounted, round-36 F2). A
    null under a note that accounts nothing (a string no
    runner writes, or a note paired with the other field's
    failure), and a record whose counter key is missing,
    contribute nothing to any censored count: they are
    named in the record-inconsistency channel as what they
    are, and no surface describes a unit as accounted-by-
    note unless the records counted are.

    Pairing-complete honesty: a unit in which exactly one of
    the two arms carries a plausible numeric value of the
    counter is NAMED in the returned unpaired list likewise,
    and the dimension renders UNEVALUATED over it (see
    _collected_record_state): an unmade comparison evaluates
    nothing, the dimension cannot PASS silently on such a
    cell, and a campaign whose dimension is UNEVALUATED
    cannot render COMPLETE. The unpaired list is rendered by
    name in the profile detail and the ledger basis at both
    verdict consumers (round-33 F4), the way losses are named;
    the compared units are rendered with their per-unit
    numeric and planned counts in the same two PASS surfaces
    (round-34 F4), so a PASS states the evidence each of its
    comparisons rested on.

    Returns (losses, unpaired, compared, failing): losses is
    a list of human-readable cost-loss strings, one per
    (competitor, counter, path, context, variant) unit whose
    designated sorted values fail dominance, naming the
    failing ranks; unpaired is a list of human-readable
    strings naming each one-sided, cardinality-shortfall,
    coverage-floor or censored unit; compared is a list of
    human-readable strings naming each unit that WAS
    compared, with each side's numeric and planned counts;
    failing_pairs is a list of (unit name, failing-rank
    count) pairs, one per loss, carrying the magnitude the
    standing-loss entry renders (round-35 F4)."""
    measured = [r for r in records if r.get("kind") == "measured"]
    # Round-41 F1: the comparison's evidence population is
    # the multiplicity-exact planned population (see
    # _planned_occurrence_flags) — its plausible values, its
    # per-cell record lists and its censored counts are built
    # from planned occurrences only, so a duplicate copy of
    # one planned slot moves no comparison. The CARRIED
    # population (every measured record) is counted beside it
    # for one purpose only: the UNPLANNED branches below state
    # how many carried records stand in a cell the plan does
    # not carry, or carries for no record of that arm — the
    # naming those branches exist for.
    planned_measured = [r for r in _planned_records(records)
                        if r.get("kind") == "measured"]
    losses = []
    unpaired = []
    compared = []
    failing_pairs = []
    # Round-37 F1: the planned population is build_plan()'s —
    # the same source the (4)/(6) limbs of
    # _collected_record_state trust — counted per
    # (path, context, variant) cell and arm from the plan's
    # measured slots (variant via the slot's own encoding),
    # never derived from the records under judgment.
    plan_cells = {}
    for slot in build_plan():
        if slot.kind != "measured":
            continue
        plan_cell = (slot.path, slot.context, slot.variant)
        plan_counts = plan_cells.setdefault(plan_cell, {})
        plan_counts[slot.arm] = plan_counts.get(slot.arm, 0) + 1
    for competitor in PRODUCT_ARMS:
        if competitor == DESIGNATED_ARM:
            continue
        for counter in RESOURCE_COUNTERS:
            plausible = (_is_plausible_resource if counter == "cpu_ms"
                         else _is_plausible_peak)
            cells = {}
            planned = {}
            carried = {}

            def comparison_cell(record):
                # Round-40 F1: the gate keys by the
                # SLOT-DERIVED variant on every path — the
                # round-33/34 derivation discipline extended
                # to the gate itself. A record's variant is
                # the slot it answers, never its carried
                # label: malformed records key by
                # _record_variant (the slot's encoded shape,
                # falling back to the carried label only
                # where the slot derivation yields no variant
                # at all — the out-of-band-rep genus, whose
                # rep the rep hold already names); scale
                # records by _scale_variant_of, as before;
                # fast/kill slots encode no variant, so their
                # records key by None, their plan variant —
                # a carried string on a fast record
                # manufactures no cell.
                if record.get("path") == "scale":
                    variant = _scale_variant_of(record)
                elif record.get("path") == "malformed":
                    variant = _record_variant(record)
                    if variant is None:
                        variant = record.get("variant")
                else:
                    variant = None
                return (record.get("path"),
                        record.get("context"), variant)

            for record in measured:
                arm = record.get("arm")
                if arm not in (DESIGNATED_ARM, competitor):
                    continue
                cell = comparison_cell(record)
                carried.setdefault(cell, {})
                carried[cell][arm] = carried[cell].get(arm, 0) + 1
            for record in planned_measured:
                arm = record.get("arm")
                if arm not in (DESIGNATED_ARM, competitor):
                    continue
                cell = comparison_cell(record)
                planned.setdefault(cell, {}).setdefault(
                    arm, []).append(record)
                if not plausible(record.get(counter)):
                    continue
                cells.setdefault(cell, {}).setdefault(
                    arm, []).append(record.get(counter))
            # Round-36 F1, re-anchored round-37 F1: the
            # enumeration is PLAN-DRIVEN from build_plan() —
            # every cell in the plan's planned population is
            # a unit, unioned with the cells the records
            # carry, whether or not any record or plausible
            # value survives in it (a fully censored cell's
            # units, and a fully DELETED cell's units alike,
            # render at the one-sided gate below, with their
            # planned counts and, where records stand null
            # under an accounting note, their accounted
            # censored counts stated). The planned counts are
            # the plan's for the cell, and the coverage floor
            # below binds those same planned counts (round-38
            # F1). Round-39 F2: no fallback anywhere — a
            # cell the plan carries no slot for states plan
            # count 0 and renders UNPLANNED at its own gate
            # below, never compared.
            for cell in sorted(set(planned) | set(carried)
                               | set(plan_cells), key=repr):
                path, context, variant = cell
                by_arm = cells.get(cell, {})
                designated_values = by_arm.get(DESIGNATED_ARM, [])
                rival_values = by_arm.get(competitor, [])
                designated_records = planned.get(cell, {}).get(
                    DESIGNATED_ARM, [])
                rival_records = planned.get(cell, {}).get(
                    competitor, [])
                designated_carried = carried.get(cell, {}).get(
                    DESIGNATED_ARM, 0)
                rival_carried = carried.get(cell, {}).get(
                    competitor, 0)
                designated_planned = plan_cells.get(
                    cell, {}).get(DESIGNATED_ARM, 0)
                rival_planned = plan_cells.get(
                    cell, {}).get(competitor, 0)
                where = (f"{competitor} on {counter} in "
                         f"{path}/{context} variant {variant!r}")
                counts = (
                    f"the designated arm carries "
                    f"{len(designated_values)} numeric "
                    f"{counter} record(s) of its "
                    f"{designated_planned} planned record(s) "
                    f"and {competitor} carries "
                    f"{len(rival_values)} of its "
                    f"{rival_planned} in that cell")
                if cell not in plan_cells:
                    # Round-39 F2: a cell the plan does not
                    # contain is NOT a comparison population.
                    # Its records are UNPLANNED — named here,
                    # in the unpaired (unaccounted) class,
                    # with the plan count stated as 0 — and
                    # no later gate compares them: the
                    # one-sided, floor, cardinality and
                    # censored gates all judge planned
                    # populations, and a fabricated planned
                    # count (rounds 36–38's carried fallback)
                    # would certify a population the plan
                    # never carried.
                    unpaired.append(
                        f"{where}: {counts} — UNPLANNED: the "
                        f"plan carries no slot for that cell "
                        f"(plan count 0), and "
                        f"{designated_carried} designated "
                        f"and {rival_carried} {competitor} "
                        f"record(s) stand in it as unplanned "
                        f"records — a cell the plan does not "
                        f"contain is not a comparison "
                        f"population, so no cost comparison "
                        f"can be made there, and the dimension "
                        f"is not evaluated on that cell")
                    continue
                if not designated_planned or not rival_planned:
                    # Round-40 F2: the plan-0 rule binds per
                    # (cell, arm) — the grain the counts
                    # themselves use. A cell the plan carries
                    # for one arm only is not a comparison
                    # population for the arm it counts as 0:
                    # that arm's records in the cell are
                    # UNPLANNED even inside a planned cell,
                    # and no compared unit renders a count
                    # over a zero denominator ("(4/0)").
                    zero_sides = []
                    if not designated_planned:
                        zero_sides.append(
                            "the designated arm (plan count 0)")
                    if not rival_planned:
                        zero_sides.append(
                            f"{competitor} (plan count 0)")
                    unpaired.append(
                        f"{where}: {counts} — UNPLANNED: the "
                        f"plan carries no slot in that cell "
                        f"for {' and '.join(zero_sides)}, and "
                        f"{designated_carried} designated "
                        f"and {rival_carried} {competitor} "
                        f"record(s) stand in it — a population "
                        f"the plan counts as 0 for an arm is "
                        f"not a comparison population for that "
                        f"arm, so no cost comparison can be "
                        f"made there, and the dimension is "
                        f"not evaluated on that cell")
                    continue
                # Round-36 F4: the censored counts bind
                # ACCOUNTED nulls only — the counter's key
                # present, its value None, and the null
                # accounted for that field under the per-field
                # pairing. Unaccounted and key-missing nulls
                # count toward nothing here; the inconsistency
                # channel names them as what they are.
                designated_censored = sum(
                    1 for record in designated_records
                    if counter in record
                    and record.get(counter) is None
                    and _resource_field_accounted(record, counter))
                rival_censored = sum(
                    1 for record in rival_records
                    if counter in record
                    and record.get(counter) is None
                    and _resource_field_accounted(record, counter))
                if not designated_values or not rival_values:
                    censored_clause = ""
                    if designated_censored or rival_censored:
                        censored_clause = (
                            f" — {designated_censored} designated "
                            f"and {rival_censored} {competitor} "
                            f"record(s) in that cell are censored "
                            f"on {counter} (the counter stands "
                            f"null, accounted only by a "
                            f"collection-failure note)")
                    unpaired.append(
                        f"{where}: {counts}{censored_clause} "
                        f"— no cost "
                        f"comparison can be made there, so the "
                        f"dimension is not evaluated on that cell")
                    continue
                # Round-38 F1: the coverage floor binds the
                # PLAN population — designated_planned /
                # rival_planned, build_plan()'s counts for the
                # cell (0 for a cell the plan does not
                # carry — but such a cell never reaches this
                # floor: it renders UNPLANNED above, round-39
                # F2) — not the
                # records a side carries: 10 numeric of 90
                # planned is a minority of the plan however
                # complete the carried tenth is.
                if len(designated_values) * 2 < designated_planned \
                        or len(rival_values) * 2 < rival_planned:
                    unpaired.append(
                        f"{where}: {counts} — a side is "
                        f"numeric on only a minority of its "
                        f"planned records in that cell, so no "
                        f"cost comparison can be made there, "
                        f"and the dimension is not evaluated "
                        f"on that cell")
                    continue
                if len(designated_values) != len(rival_values):
                    unpaired.append(
                        f"{where}: {counts} — the two sides' "
                        f"numeric populations are unequal, so "
                        f"no cost comparison can be made "
                        f"there (a comparison over unequal "
                        f"populations compares counts, not "
                        f"costs), and the dimension is not "
                        f"evaluated on that cell")
                    continue
                if designated_censored or rival_censored:
                    # Round-35 F1: the censored complement is
                    # resolved against the designated arm's
                    # claim under the worst-case imputation
                    # (see the docstring) — dominance over
                    # the full population cannot be certified
                    # there, so the unit is unpaired as a
                    # censored unit, never a loss. Round-36
                    # F4: the counts above bind accounted
                    # nulls only, so the accounting this
                    # entry asserts is true of the records
                    # counted.
                    unpaired.append(
                        f"{where}: {counts} — "
                        f"{designated_censored} designated "
                        f"and {rival_censored} {competitor} "
                        f"record(s) in that cell are censored "
                        f"on {counter} (the counter stands "
                        f"null, accounted only by a "
                        f"collection-failure note): under the "
                        f"worst-case imputation every censored "
                        f"value is resolved against the "
                        f"designated arm's claim (a censored "
                        f"designated value is unbounded "
                        f"above and a censored competitor "
                        f"value stands at the counter's "
                        f"minimum), so no cost comparison can "
                        f"be made there, and the dimension is "
                        f"not evaluated on that cell")
                    continue
                compared.append(
                    f"{where}: the designated arm carries "
                    f"{len(designated_values)} numeric "
                    f"{counter} record(s) of its "
                    f"{designated_planned} planned record(s) "
                    f"({len(designated_values)}/"
                    f"{designated_planned}) and {competitor} "
                    f"carries {len(rival_values)} of its "
                    f"{rival_planned} ({len(rival_values)}/"
                    f"{rival_planned}) in that cell — compared "
                    f"by sorted-rank dominance")
                designated_sorted = sorted(designated_values)
                rival_sorted = sorted(rival_values)
                failing = [
                    (rank, designated_value, rival_value)
                    for rank, (designated_value, rival_value)
                    in enumerate(
                        zip(designated_sorted, rival_sorted), 1)
                    if designated_value > rival_value]
                if failing:
                    failing_pairs.append((where, len(failing)))
                    ranks = "; ".join(
                        f"rank {rank}: designated "
                        f"{designated_value!r} against "
                        f"{competitor}'s {rival_value!r}"
                        for rank, designated_value, rival_value
                        in failing)
                    losses.append(
                        f"{where}: the designated arm's sorted "
                        f"{counter} values do not dominate "
                        f"{competitor}'s at every rank — "
                        f"sorted-rank dominance fails at "
                        f"{ranks} — over "
                        f"{len(designated_values)} designated "
                        f"and {len(rival_values)} competitor "
                        f"numeric record(s), the designated "
                        f"arm costs more than {competitor} on "
                        f"{counter} there at the failing "
                        f"rank(s)")
    return losses, unpaired, compared, failing_pairs


def _paired_cost_comparison_full(records):
    """The (losses, unpaired, compared) form of the
    dimension-(3) arm comparison (see
    _paired_cost_comparison_detail, whose fourth return —
    the failing (unit, counter) pairs with their
    failing-rank counts — the standing-loss entry renders
    through the detail form). The three-value shape is the
    round-34 wiring: the PASS surfaces unpack exactly
    three."""
    losses, unpaired, compared, _failing_pairs = \
        _paired_cost_comparison_detail(records)
    return losses, unpaired, compared


def _paired_cost_comparison(records):
    """The (losses, unpaired) form of the dimension-(3)
    arm comparison (see _paired_cost_comparison_full, whose
    third return — the compared units with their per-unit
    counts — the PASS surfaces render through the full form).
    The two-value shape is retained from the round-31 wiring:
    every standing caller and probe unpacks exactly two."""
    losses, unpaired, _compared = \
        _paired_cost_comparison_full(records)
    return losses, unpaired


def _record_variant(record):
    """A malformed/scale record's variant AS THE PLAN ENCODES
    IT (round-32 F2): the variant its slot identity (path,
    rep, kind) re-derives from the plan — plan.slot_variant —
    never the record's self-carried label. A record's carried
    variant is a claim about its slot, held against this
    derivation by the record-consistency discipline (a carried
    label that disagrees with the slot's encoded shape is
    named as a variant inconsistency); it is never the
    authority a verdict reads. None when the slot identity
    derives no variant (a rep of no usable form, or a path
    with no variants) — deriving it must never raise in a
    checker. Round-34 F2: a None derived from an unusable rep
    on a variant path is itself a finding — the
    record-consistency discipline names the rep (see
    _rep_well_formed) — never a silent skip of the holds
    conditioned on this derivation."""
    return slot_variant(record.get("path"), record.get("rep"),
                        record.get("kind"))


def _rep_well_formed(record):
    """True when a record's rep is a slot identity the plan
    can read (round-34 F2): a non-bool integer within its
    kind's band for the record's path — for a malformed or
    scale record, exactly the reps plan.slot_variant derives
    a variant from; for a record of any other path, the reps
    of its kind's plain cells (measured / warmup / control
    rep counts of the plan's own constants). A float twin of
    a genuine rep (3.0 for 3) is NOT well-formed here even
    though Python equality, hashing and every Counter in this
    checker read it as the genuine slot: slot identity is
    type-exact, as the STAMP discipline already holds for
    stamp values ("a value of another type that merely
    compares equal to the stamp (1.0 for 1, above all) is not
    the stamp") and as the package checker's order-entry hold
    holds for repetitions. A bool is not an integer rep, and
    a negative or out-of-band integer names no slot of its
    kind. Total: any rep value resolves to a boolean."""
    rep = record.get("rep")
    if not isinstance(rep, int) or isinstance(rep, bool):
        return False
    path = record.get("path")
    if path in ("malformed", "scale"):
        return slot_variant(path, rep, record.get("kind")) \
            is not None
    band = {"measured": MEASURED_REPS, "warmup": WARMUP_REPS,
            "control": CONTROL_REPS}.get(record.get("kind"))
    return band is not None and 0 <= rep < band


def malformed_argv_presents(argv, shape):
    """True when an executed argument vector PRESENTS one
    malformed shape (round-33 F2): the shape-distinguishing
    content runner.malformed_argv's construction writes for
    that shape, derived from the arms' own presentation
    templates (the round-26 presentation identity). Each
    shape is fully distinguishable inside the argv it is
    presented through:

    - nonexistent-program: the child token is the
      construction's nonexistent program,
      "ptg-no-such-program-9f3b.exe" (runner.NONEXISTENT_CHILD);
    - malformed-timeout: a timeout token is the construction's
      malformed timeout text, "not-a-timeout";
    - out-of-range-timeout: a timeout token is the
      construction's out-of-range text, "999999999" (seconds)
      or "999999999000" (milliseconds);
    - empty-command: the child token is empty — an empty
      token after the executable;
    - over-long-argument: a child-argument token carries the
      construction's length property — at least 4096
      characters (the construction's OVER_LONG_ARGUMENT is
      exactly 4096; a longer token is the same property);
    - non-unicode-argument: a child-argument token carries
      the construction's encoding property — a lone surrogate
      code point (the construction's NON_UNICODE_ARGUMENT
      carries U+D800), which no valid-Unicode argument holds.

    Total: an argv that is not a non-empty list of string
    tokens presents no shape (its form is the standing form
    hold's naming, not this predicate's), and an unknown
    shape is presented by nothing."""
    if not (isinstance(argv, list) and argv
            and all(isinstance(token, str) for token in argv)):
        return False
    if shape == "nonexistent-program":
        return "ptg-no-such-program-9f3b.exe" in argv
    if shape == "malformed-timeout":
        return "not-a-timeout" in argv
    if shape == "out-of-range-timeout":
        return "999999999" in argv or "999999999000" in argv
    if shape == "empty-command":
        return "" in argv[1:]
    if shape == "over-long-argument":
        return any(len(token) >= 4096 for token in argv)
    if shape == "non-unicode-argument":
        return any(
            any(0xD800 <= ord(char) <= 0xDFFF for char in token)
            for token in argv)
    return False


_SHIPPED_CONFIG_CACHE = {}


def _shipped_config():
    """The suite's shipped campaign configuration —
    campaign.json at the suite root beside this file's
    campaign/ directory — loaded once and cached, or None
    when it cannot be read as an object. This is the run
    configuration whose SHA-256 a run package records for
    itself (the path precedent is USABILITY_ARTIFACT, the
    suite artifact report.py already reads beside the
    suite)."""
    if "config" not in _SHIPPED_CONFIG_CACHE:
        config = None
        try:
            path = os.path.join(
                os.path.dirname(os.path.dirname(
                    os.path.abspath(__file__))), "campaign.json")
            with open(path, encoding="utf-8") as handle:
                loaded = json.load(handle)
            if isinstance(loaded, dict):
                config = loaded
        except (OSError, ValueError):
            config = None
        _SHIPPED_CONFIG_CACHE["config"] = config
    return _SHIPPED_CONFIG_CACHE["config"]


NO_VERIFIED_PLAN = object()
"""Sentinel for _plan_fast_argv_length's config argument:
the records under judgment belong to a run whose plan no
verified configuration supplies (a preserved package whose
named configuration anchor is absent — the same absence
_package_expected_output answers by holding nothing). The
length limb then does not fire: a budget cannot be taken
from the package's own recorded templates, which would
compare the package against itself, and the absent anchor
is already named unverifiable by the package checker.
Round-38 F2: the sentinel may not CERTIFY either — a set
judged under it renders dimension (6) UNEVALUATED in the
verdict layer whenever the dimension would otherwise
PASS, the underived budget named in the dimension's own
detail and basis (see _security_sentinel_degradation), so
no verdict consumer can render COMPLETE over the unheld
limb; rounds 36–37 named the absence only in the package
checker's problem list, and round 38's reviewer executed
a junk-tailed set certifying COMPLETE at both layers
under this sentinel."""


def _security_sentinel_degradation(config):
    """The dimension-(6) degradation a NO_VERIFIED_PLAN
    judgment carries (round-38 F2), as a naming string — or
    None under any other footing. Under the sentinel no
    argv length budget derives for any arm, so the
    malformed records' argv exactness is held to no length
    budget; rounds 36–37 named that absence only in the
    package checker's degraded-verification list, and
    round 38's reviewer executed the consequence at both
    verdict surfaces: a junk-tailed set judged under the
    sentinel rendered campaign_outcome COMPLETE with zero
    standing losses, and a package under the same footing
    printed "STATUS: complete campaign" in cmd_report while
    its checker named five problems. The degradation
    therefore lives in the verdict itself: both consumers
    that render dimension (6)'s collected state
    (profile_dimension_states and claim_ledger) cap the
    state at UNEVALUATED whenever it would otherwise PASS
    under the sentinel, and carry this naming in the
    dimension's detail and basis — so the standing-loss
    entry built from the basis, campaign_outcome's ledger
    test and render()'s STATUS all see the one outcome.
    The cap binds only a would-be PASS: a dimension
    already UNEVALUATED or FAILING under the sentinel
    renders exactly as it rendered before (the preserved
    reduced package's (6) is UNEVALUATED on its incomplete
    collection, so its renders, its 82 standing losses and
    its five degraded problems do not move), and a FAIL —
    a designated pinned-expectation violation — stays a
    FAIL, named by its own channel."""
    if config is not NO_VERIFIED_PLAN:
        return None
    return ("the records are judged under NO_VERIFIED_PLAN "
            "— no verified plan exists for them (the footing "
            "a preserved package whose configuration anchor "
            "is absent is judged under) — so the argv length "
            "budget for every arm is underived and the "
            "malformed records' argv exactness is held to "
            "no length budget: dimension (6) is not "
            "evaluated over that unheld limb and cannot "
            "render PASS under this footing (round-38 F2: "
            "the sentinel names its degradation in the "
            "verdict, never certifies over it)")


def _plan_fast_argv_derivation(arm, config=None):
    """The ONE derivation of an arm's fast-vector length
    budget (round-38 F3): returns (length, gap) — the
    derived length and None when the construction derives,
    or None and the plan fact that names WHY it does not.
    The gap is read off the derivation's own steps, in the
    order the derivation takes them, so every configuration
    that derives no budget is named with the fact that is
    true of it: which component of the construction is
    absent, and which is present but unusable (a component
    stated in a wrong type is named as what it is, never
    stated as absent — round 38's reviewer executed the
    round-37 helper's catch-all stating a stated template
    and stated args unstated for a configuration missing
    only its child spec's program, a component the old
    helper never examined).

    The derivation's steps, and the failure each names:
    the arm's fast template (absent; present in a form the
    construction cannot iterate as tokens; carrying a
    token the expansion's own tests refuse); the fast
    child spec (absent; present but not a mapping); its
    args (absent; present in a form the construction's
    list() coercion cannot read); its program (absent);
    and the expansion itself through arms.expand_template
    (a stated construction the expansion refuses). The
    length this derivation derives
    is a COUNT — 1 + the number of tokens in the expanded
    vector — and a count is type-independent on the
    program and args limbs (round-39 F1): the expansion
    appends the program verbatim at {child} and extends
    verbatim at {child_args}, so a non-string program or
    a non-string args element moves the expanded count
    exactly as a string token does, and neither limb is
    ever named underived for its element type. Round-40
    F4: the derivation mirrors the construction's
    tokenization for every config shape the construction
    accepts — an args value stated as a string tokenizes
    per character and as a dict by its keys, under the
    list() coercion the runner applies; a template stated
    as a string tokenizes per character under the
    iteration the runner's resolve applies; and a
    sequence template token (a list or tuple) is appended
    verbatim by the expansion, so it derives. A length
    derives wherever the construction builds a vector;
    the gap fact names only shapes where the construction
    itself cannot build one (a non-iterable template or
    args value, a template token the expansion's own
    tests refuse). Round-41 F4: the derivation never
    CONSUMES a one-shot iterator — a value stated as a
    generator is materialized ONCE, at this read, the
    same materialization the construction's own reads
    perform (arms.resolve_template, runner.child_for
    and its siblings), so the budget is a function of the
    configuration: repeated derivations on the same
    configuration object agree, and a derivation cannot
    starve the construction that shares the object (round
    40's list() coercion read a generator args value to
    exhaustion here and left runner.child_for building an
    empty vector from the same object, while a second
    derivation of the same object derived a shorter
    budget — the pair disagreed about one configuration).
    Round-42 F3/F4: the materialization never requires a
    mutable mapping and is confined to the values that
    need it — a one-shot iterator's list is carried by
    arms' identity cache (arms.materialized_list), shared
    by both sides of the mirror, and written back into
    the configuration object only where the carrying
    mappings accept the assignment (a refusal — a
    MappingProxyType, a frozen dict — is absorbed, never
    raised, and the cache serves every later read
    identically, so configurations the as-found
    construction accepted derive and run here, repeated
    derivations agreeing, with no 5-then-1); a value
    already materialized or re-iterable (a string, a
    dict, a tuple) is read through a local list() and
    never rewritten in place, so a derivation cannot
    repair or alter the configuration a second consumer
    judges. The child spec this derivation reads is held
    as a mapping, not only a dict: a mutation-refusing
    mapping the construction reads (the runner reads its
    spec through .get) derives here identically, and a
    spec that is no mapping at all keeps the gap below —
    a shape that cannot be materialized identically on
    both sides names its gap on both sides (the
    construction builds no vector from a non-mapping
    spec either).
    Round-43 F3/F4: the mirror has three sides and the
    materialization reaches all of them — the pricing
    probes read the fast child spec through the same
    shared read (arms.materialized_child_args), so
    pricing-first, derivation-first and runner-first
    orders all agree on the same configuration object;
    and the confinement's dichotomy is restated to the
    flow's unit: materialization is keyed on one-shot-ness
    of the value, not iterator type identity — any value
    that can be consumed only once (a one-shot iterator,
    a one-shot iterable, a payload-carrying subclass, a
    factory mapping's minted values) is materialized
    once at the shared read whatever its type, while a
    provably re-iterable value (a string, a dict, a
    tuple, a plain list) is still never rewritten.
    Round-44 F4: the write-backs (the template limb's
    and the args limb's alike) fire only for a value
    PROVEN one-shot — an iterator in fact, or successive
    iter() calls handing back the same iterator; a
    merely unprovable value, which the conservative
    test still caches, is never rewritten in place by
    this derivation, so a genuinely re-iterable custom
    value survives it with its live backing intact.
    Round-45 F3/F4/F5: proven is the repaired breadth —
    a traversal that cannot restart by construction
    (a built-in one-way iterator, or an iterable
    handing that same iterator back) — so a restarting
    iterator is never rewritten by either limb; the
    classification is derived from the materialization
    itself, with no speculative iter() calls stacked
    ahead of either read, so a value whose iter()
    spends an element derives the budget of exactly
    the tokens one honest read renders; and the
    budget's basis is the one shared read's tokens,
    the same tokens the runner's construction builds
    for the unproven class exactly as for the proven —
    the derivation and the construction cannot read
    two different values from one configuration
    object.
    Under NO_VERIFIED_PLAN the pair is
    (None, None): the sentinel is not a configuration
    stating anything, and its degradation is named by the
    verdict layer's sentinel cap (see
    _security_sentinel_degradation), not by a plan fact
    about a configuration that does not exist."""
    if config is NO_VERIFIED_PLAN:
        return None, None
    effective = _shipped_config() if config is None else config
    if effective is None:
        return None, ("the shipped configuration from which a "
                      "budget would derive cannot be read")
    judged = ("the configuration this record set is judged "
              "under")
    try:
        template = effective["arms"][arm]["templates"]["fast"]
    except (KeyError, TypeError, AttributeError):
        return None, (
            f"{judged} states no fast template for arm "
            f"{arm!r} — a plan fact of that configuration, "
            f"which carries no fast construction for the arm "
            f"— so no fast-vector length derives for it")
    # Round-40 F4: the template limb mirrors the
    # construction's own tokenization. The runner resolves
    # a template by ITERATING the value it is given
    # (arms.resolve_template), so a template stated as a
    # string tokenizes per character and any other iterable
    # states its elements as its tokens; the derivation
    # takes the same list() the construction's iteration
    # amounts to, and names the gap only where that reading
    # itself fails (a template value that is not iterable
    # at all). A token's own usability is the expansion's
    # question, judged below where the expansion runs —
    # sequence tokens (a list or tuple token) are appended
    # verbatim by it, exactly as the runner's construction
    # appends them.
    # Round-44 F4: the write-back below is gated on the
    # template being PROVEN one-shot, not on the
    # conservative one-shot test — the materialized read
    # itself (materialized_list, the identity cache) is
    # unchanged.
    # Round-45 F3/F4: the gate is the proven
    # classification at its repaired breadth (a traversal
    # that cannot restart by construction), derived from
    # the materialization's own evidence — the read below
    # is the one iter() call an honest read makes, and no
    # speculative classification is stacked ahead of it,
    # so a template whose iter() spends an element
    # derives from exactly the tokens one honest read
    # renders.
    try:
        template_tokens, one_shot_template = \
            materialized_list_proven(template)
    except TypeError as error:
        # Round-49 F3: as at the args limb, a TypeError
        # carrying no captured failed value is not the
        # capture's unreadability — it is a TypeError the
        # traversal itself raised, and it propagates as
        # itself, exactly as one honest read propagates
        # it; only the capture step's failure (carrying
        # the failed value's type name) is named here.
        if getattr(error, "value_type_name", None) is None:
            raise
        return None, (
            f"{judged} states a fast template for arm {arm!r} "
            f"as a {type(template).__name__}, a value the "
            f"construction cannot iterate as tokens — the "
            f"template is present but unusable, so no "
            f"fast-vector length derives for the arm")
    if one_shot_template:
        # Round-41 F4, as confined by round-42 F3/F4:
        # materialize ONCE, here at the read both sides
        # share — a one-shot iterator (a generator above
        # all) is carried by arms' identity cache as the
        # list it materialized to, and written back into
        # the configuration object where its mappings
        # accept the assignment, so neither a second
        # derivation nor the construction downstream can
        # read an exhausted value; a mapping that refuses
        # the write-back (a MappingProxyType, a frozen
        # dict) is absorbed, never raised, and the cache
        # serves the later reads. A re-iterable template
        # (a string, a tuple) reaches no write-back at
        # all: it was read locally above and the caller's
        # object stands as its owner stated it. (The
        # shipped configuration's templates are JSON lists
        # already; no materialization occurs there.)
        try:
            arms_map = effective.get("arms")
            entry = arms_map.get(arm) \
                if isinstance(arms_map, dict) else None
            templates = entry.get("templates") \
                if isinstance(entry, dict) else None
            if isinstance(templates, dict):
                templates["fast"] = list(template_tokens)
        except Exception:
            pass
    try:
        spec = effective["children"]["fast"]
    except (KeyError, TypeError, AttributeError):
        return None, (
            f"{judged} states a fast template for arm {arm!r} "
            f"but no fast child spec, so the template cannot "
            f"be expanded through the construction and no "
            f"fast-vector length derives for the arm")
    if not isinstance(spec, Mapping):
        return None, (
            f"{judged} states a fast template for arm {arm!r} "
            f"but its fast child spec is a "
            f"{type(spec).__name__}, not the mapping the "
            f"construction reads — the child spec is present "
            f"but unusable, so no fast-vector length derives "
            f"for the arm")
    # Round-47 F2 judged presence on the spec's KEY SET,
    # never on an access to the value. Round-48 F3/F4
    # withdraw that judgement entirely: the key set is an
    # observable the construction does not consult, and
    # it disagrees with the construction's own read in
    # both directions — a get serving args its key set
    # does not list was named "states no args" while the
    # construction built the full vector from the same
    # spec, and a one-shot key set, consumed judging the
    # first field, named the second field's stated value
    # unstated (round-48 P7) — while a mapping whose key
    # iteration raises fell back to a contains access,
    # `key in spec`, an access whose mint the derivation
    # discarded, whose eager mint drained the source
    # ahead of the capture and left the derivation
    # pricing the drained remainder (round-48 P6). No
    # iteration of the spec's keys and no contains access
    # over the spec is spent here at all now: presence is
    # judged over the SAME materialized evidence the
    # construction builds from. For args, the shared
    # read's own stated element is the judgement — a
    # value the read serves is stated; a spec the read's
    # own access finds no value on states no args. For
    # program, the judgement is the construction's own
    # access, whose value the expansion below uses: no
    # access in this derivation is discarded, and every
    # reason it names is true of the configuration as
    # the construction reads it.
    # Round-40 F4: the args limb mirrors the construction's
    # own tokenization — the runner builds child args as
    # list(spec.get("args", [])) on the fast, malformed and
    # floor paths alike, so an args value stated as a string
    # tokenizes per character, a dict by its keys, and any
    # other iterable by its elements; the derivation applies
    # the same list() coercion (its own expansion call has
    # applied list(child_args) since before round 39) and
    # names the gap only where that coercion raises — an
    # args value no iteration can read, the one shape where
    # the construction itself builds no vector.
    # Round-43 F3/F4: the read is the shared one — arms'
    # materialized_child_args, the same read the runner's
    # construction and the pricing probes make — and the
    # materialize-once test is one-shot-ness of the value
    # (is_one_shot_value), not iterator identity, so a
    # one-shot iterable or a payload-carrying subclass
    # derives the full vector on every derivation, in any
    # reader order, exactly as a generator does.
    # Round-44 F4: as at the template limb, the
    # write-back below is gated on the args being PROVEN
    # one-shot; the shared read itself is unchanged, and
    # a merely unprovable args value (is_one_shot_value's
    # conservative third disjunct) is never rewritten in
    # the caller's spec — it may be genuinely re-iterable
    # over a live backing its owner still mutates.
    # Round-45 F3/F4/F5: the gate is the proven
    # classification at its repaired breadth, derived by
    # the shared read from its own materialization — no
    # speculative classification is stacked ahead of the
    # read, and the tokens this derivation prices are the
    # shared read's tokens, the same tokens the runner's
    # construction builds and the pricing probes read,
    # for the unproven class exactly as for the proven
    # (the runner no longer reads an unproven value
    # locally: one semantics, one shared read).
    # Round-49 F4: the write-back below is gated ALSO on
    # no candidate pending on the slot (arms'
    # materialized_candidate_pending): this derivation
    # may itself BE the ambiguous read, and a pending
    # candidate renders the interim value unproven for
    # write-back purposes — the derivation prices the
    # interim exactly as the shared read serves it, and
    # writes nothing back, so the deferred settlement
    # the following reads take is the settlement the
    # materialized-only sequence reaches.
    # Round-50 F2: the gate is judged on the state this
    # derivation's read FOUND, not the state it left —
    # the no-pending state is taken here, BEFORE the
    # read below, and the write-back consults both
    # takings. A read that settles a pending candidate
    # clears the pendency the round-49 gate asked about
    # after the fact, so judged after the read alone the
    # gate stood open on the settling read itself and
    # the settled entry was planted into a minting
    # mapping's store in the same call.
    args_candidate_at_read_start = \
        materialized_candidate_pending(spec)
    try:
        child_args, one_shot_args, args_stated = \
            materialized_child_args_stated(spec)
    except TypeError as error:
        # Round-48 F3: the type named is the type of the
        # value THE READ FAILED ON, captured at the
        # failing read and carried by the error itself
        # (arms.UnusableValueTypeError) — no second
        # spec["args"] access exists in this handler: a
        # minting spec's second mint is a value this read
        # never saw, and the round-47 handler named its
        # type for a read that failed on the first mint
        # (round-48 P8). A TypeError carrying no captured
        # type is not the failing read's shape and
        # propagates, exactly as a capture raise does.
        failed_type = getattr(error, "value_type_name", None)
        if failed_type is None:
            raise
        return None, (
            f"{judged} states a fast template for arm {arm!r} "
            f"but its fast child spec states its args as a "
            f"{failed_type}, a value the construction's "
            f"list() coercion cannot read as tokens — the "
            f"args are present but unusable, so the "
            f"template cannot be expanded through the "
            f"construction and no fast-vector length derives "
            f"for the arm")
    if not args_stated:
        return None, (
            f"{judged} states a fast template for arm {arm!r} "
            f"but its fast child spec states no args, so the "
            f"template cannot be expanded through the "
            f"construction and no fast-vector length derives "
            f"for the arm")
    if one_shot_args \
            and not args_candidate_at_read_start \
            and not materialized_candidate_pending(spec):
        # Round-41 F4, as confined by round-42 F3/F4: the
        # same materialize-once for the args limb — the
        # runner reads this same spec object (cmd_run
        # shares one configuration between the checker and
        # the runner), so the list the derivation
        # materialized is carried to the construction by
        # arms' identity cache, and written back here only
        # where the spec accepts the assignment (a
        # mutation-refusing spec is absorbed, never
        # raised). A re-iterable args value (a string, a
        # dict, a tuple) is never rewritten: the
        # derivation judged it as its owner stated it and
        # leaves it exactly so for the next consumer.
        # Round-49 F4: the gate is ALSO no-pending — while
        # a deferred candidate pends on the slot, the
        # interim value this read served is unproven for
        # write-back purposes, so no write-back fires:
        # the interim candidate is never planted into the
        # caller's spec, and the stored value a following
        # settlement will judge (its traversal's
        # persistence above all) is never replaced by a
        # plain list mid-pendency. A deferred verdict is
        # settled by reads, never by writes.
        # Round-50 F2: the gate is judged on the state
        # the consumer's read found, not the state it
        # left — the no-pending consultation above is
        # taken before this derivation's own read as well
        # as after it, so a derivation whose read SETTLES
        # a candidate (its read found the candidate
        # pending) plants nothing in its own call, and a
        # derivation whose read CREATES a candidate (the
        # after-taking finds it pending) plants nothing
        # either: a settling read plants nothing, and the
        # consumer-first and materialized-only sequences
        # reach the same settlement.
        try:
            spec["args"] = list(child_args)
        except Exception:
            pass
    # Round-39 F1: no element-type check on the args limb
    # — the derivation consumes the args' COUNT (the
    # {child_args} splice extends the expanded vector by
    # len(child_args)), and a count derives for elements
    # of any type, exactly as the as-found derivation
    # derived it. A non-string args element is a defect
    # of the configuration as an executable construction,
    # named by the channels that name configuration
    # defects — never by withdrawing a derivable budget.
    # Round-48 F4: program presence is judged by the
    # construction's own access — child_for reads
    # spec["program"] — and the value that access serves
    # is the value the expansion below uses: a spec whose
    # program access serves a value states a program,
    # whatever its key set lists, and a spec serving none
    # (KeyError, the mapping's no-value answer) states
    # none. No key-set evidence is consulted, so no
    # stated program is ever named unstated because a
    # key set was one-shot or unreadable.
    try:
        program = spec["program"]
    except KeyError:
        return None, (
            f"{judged} states a fast template for arm {arm!r} "
            f"and its fast child spec's args, but its fast "
            f"child spec states no program — the construction "
            f"expands the template through the child spec's "
            f"program at {{child}}, so the template cannot be "
            f"expanded through the construction and no "
            f"fast-vector length derives for the arm")
    # Round-39 F1: no type check on the program limb
    # either — the expansion appends the program verbatim
    # as the ONE {child} token, so its type moves the
    # expanded count not at all and a length derives for
    # a program of any type, as the as-found derivation
    # derived it.
    try:
        expanded = expand_template(
            template_tokens, 10.0, program, child_args)
    except (KeyError, TypeError, ValueError, AttributeError) \
            as error:
        # Round-40 F4: an expansion failure is named at the
        # token that earns it, when one token earns it alone
        # — the expansion's own string tests refuse an
        # integer token ("{timeout_s}" in 123 raises), and a
        # dict token carrying a timeout key into .replace —
        # while sequence tokens pass this scan (the expansion
        # appends them verbatim) and never name a gap. A
        # failure no single token earns keeps the whole-
        # expansion naming below.
        for position, token in enumerate(template_tokens):
            try:
                expand_template(
                    [token], 10.0, program, child_args)
            except (KeyError, TypeError, ValueError,
                    AttributeError):
                return None, (
                    f"{judged} states a fast template for "
                    f"arm {arm!r} whose token at position "
                    f"{position} is {token!r}, a token the "
                    f"expansion's own tests refuse — the "
                    f"template is present but unusable, so "
                    f"no fast-vector length derives for "
                    f"the arm")
        return None, (
            f"{judged} states the construction for arm "
            f"{arm!r} — its fast template, its fast child "
            f"spec's args and its program — but the template "
            f"does not expand through the stated child spec "
            f"({type(error).__name__}), so no fast-vector "
            f"length derives for the arm")
    return 1 + len(expanded), None


def _plan_fast_argv_length(arm, config=None):
    """The length of an arm's fast-path argument vector as
    the PLAN builds it (round-36 F3): one executable token
    (the token runner.run_slot prepends) plus the arm's
    fast template from the run's configuration expanded
    through arms.expand_template with the configuration's
    fast child spec (its program at {child}, its args at
    {child_args}) — the very construction runner.run_slot
    and runner.malformed_argv build through. The timeout's
    value is length-irrelevant (every spelling is one
    token); the fast timeout constant is used.

    Round-35 F3 read this length from the record set under
    judgment (the modal fast length among its records);
    round-36 P3 executed the consequence: a uniformly
    extended fast population moves its own budget until the
    junk tail IS the expected length, and the timeout
    shapes' victims are named by nothing. The plan's
    templates stand outside the judged set, so no
    population of records can move this budget. Which plan
    binds follows the expected_output discipline: the
    caller that knows the run's configuration supplies it
    (cmd_run its own; a package recomputation the anchored
    configuration the manifest names and hashes to); a
    bare record set — the verdict layer's campaign — is
    judged against the shipped configuration
    (campaign.json, _shipped_config); and NO_VERIFIED_PLAN
    states that no verified plan exists for the records,
    in which case no length is derived. A configuration
    states a length only where it states the construction
    itself — the arm's own fast template and the fast
    child spec (its arguments and its program): a
    configuration carrying neither (a manifest anchor
    written for another derivation, the expected bytes
    above all) states no length, and the checker invents
    none from defaults another plan carries —
    resolve_template's fallback serves the runner's
    construction, not this budget.
    None when no length can be derived: the argv
    exactness length limb then does not fire, as it did
    not fire when a set carried no fast record. Round-37
    F2: that silence is NAMED, never silent — a malformed
    record judged under a configuration that derives no
    budget for its arm names the arm's budget as underived
    (see _plan_fast_argv_budget_gap and
    _record_inconsistencies), and under NO_VERIFIED_PLAN
    the verdict layer names the same degradation by
    capping dimension (6) at UNEVALUATED (round-38 F2; see
    _security_sentinel_degradation), beside the package
    checker's degraded-verification entry (round-37 F3).
    Round-38 F3: the length and its failure reason come
    from the ONE derivation (_plan_fast_argv_derivation),
    whose steps this function and the gap fact both read."""
    length, _gap = _plan_fast_argv_derivation(arm, config)
    return length


def _plan_fast_argv_budget_gap(arm, config=None):
    """Why no argv length budget derives for an arm under a
    configuration (round-37 F2), as a plan-fact string — or
    None when a budget DOES derive, and None under
    NO_VERIFIED_PLAN. Round 36's derivation refinement
    derives no budget for an arm whose configuration states
    no construction, and round 37's reviewer executed the
    consequence: the silence was named nowhere, so a
    junk-tailed package under a minimal anchored
    configuration (arms {}, a fast child carrying expected
    bytes but no args) certified COMPLETE. The absence is
    now named wherever it decides a verdict (see
    _record_inconsistencies): this helper states the plan
    fact the naming carries — which part of the
    construction the configuration judged under does not
    state for the arm, or states in a form the construction
    cannot use. Round-38 F3: the fact is the ONE
    derivation's own failure reason
    (_plan_fast_argv_derivation) — the round-37 form
    re-checked only the template and the args, and its
    catch-all stated both unstated for every other failure
    of the derivation (above all a missing program, which
    the derivation requires and the helper never examined;
    a non-string template token and wrong-type components
    were likewise stated as absent though present). The
    sentinel remains the one case with no naming here: it
    is not a configuration stating no template but the
    named absence of any verified plan, and that absence's
    budget degradation is named by the verdict layer's
    sentinel cap (round-38 F2) and at the package layer in
    the degraded-verification list
    (_recorded_run_field_problems, round-37 F3)."""
    _length, gap = _plan_fast_argv_derivation(arm, config)
    return gap


def _malformed_argv_exactness_problems(argv, shape, fast_length):
    """The ways a form-valid malformed vector is not the ONE
    vector runner.malformed_argv builds for its slot
    (round-35 F3; the length budget re-anchored to the plan,
    round-36 F3), as a list of short problem strings (empty
    when the vector is exact under this derivation).
    Exactness is derived from the construction's invariants
    and from the plan the construction builds
    through:

    - LENGTH: the construction builds through the arm's own
      fast template with the fast child's spec, so a genuine
      vector is no longer than the arm's fast vector —
      the timeout shapes substitute the timeout token in
      place (same length), the nonexistent-program and
      empty-command shapes DROP child_args (no longer), and
      the over-long and non-unicode shapes APPEND exactly
      one mark token to child_args (the fast length + 1).
      More tokens than that budget is content no slot's
      construction wrote — an addition, above all a junk
      tail of unmarked extra tokens. The fast length is the
      PLAN's (_plan_fast_argv_length: the run's
      configuration's template and child spec, expanded by
      the construction itself), never a statistic of the
      record set under judgment — round-36 P3: the round-35
      modal derivation let a uniformly extended fast
      population move its own budget until the junk tail was
      the expected length.
    - TAIL: every template the suite carries
      (arms.DEFAULT_TEMPLATES, the stub configuration, the
      shipped campaign.json and the perfect fixture alike)
      expands {child} then {child_args} as its final
      elements, so the shapes that drop or append child_args
      END with their construction's mark: nonexistent-program
      with the nonexistent program token, empty-command with
      the empty token, over-long-argument with a token of at
      least 4096 characters, non-unicode-argument with a
      lone-surrogate token. Content appended after the mark
      moves it off the tail and is named by the same limb.
      The timeout shapes carry no tail mark; added content
      on them is caught by the length limb."""
    problems = []
    if fast_length is not None:
        budget = fast_length + 1 if shape in (
            "over-long-argument", "non-unicode-argument") \
            else fast_length
        if len(argv) > budget:
            problems.append(
                f"it carries {len(argv)} tokens, more than "
                f"the {budget} the slot's construction "
                f"allows (the arm's fast vector in the "
                f"run's plan carries "
                f"{fast_length} token(s); the timeout shapes "
                f"substitute in place, the nonexistent and "
                f"empty shapes drop the child arguments, and "
                f"the two mark shapes append exactly one "
                f"token — no construction writes more)")
    tail = argv[-1]
    if shape == "nonexistent-program" \
            and tail != "ptg-no-such-program-9f3b.exe":
        problems.append(
            "its final token is not the construction's "
            "nonexistent program token (that shape drops the "
            "child arguments, so the program token closes "
            "the vector — content after it is content no "
            "construction wrote)")
    elif shape == "empty-command" and tail != "":
        problems.append(
            "its final token is not the construction's "
            "empty child token (that shape drops the child "
            "arguments, so the empty token closes the "
            "vector — content after it is content no "
            "construction wrote)")
    elif shape == "over-long-argument" \
            and len(tail) < 4096:
        problems.append(
            "its final token is not the construction's "
            "over-long argument (that shape appends its "
            "4096-character argument last — content after "
            "it is content no construction wrote)")
    elif shape == "non-unicode-argument" and not any(
            0xD800 <= ord(char) <= 0xDFFF for char in tail):
        problems.append(
            "its final token is not the construction's "
            "non-unicode argument (that shape appends its "
            "lone-surrogate argument last — content after "
            "it is content no construction wrote)")
    return problems


def _designated_malformed_violations(records):
    """The designated arm's violations of its pinned malformed
    expectations (round-31 F2; round-32 F2), over WARMUP AND
    MEASURED malformed records alike: a list of the records in
    which the designated arm answered a shape its contract
    pins with an observed integer exit code other than the
    pinned one. The shape is the one the record's SLOT encodes
    (plan.slot_variant on (path, rep, kind)), never the
    record's carried variant label — relabelling a violating
    record's variant moves nothing here, and the mismatch
    itself is named separately as a variant inconsistency.
    The expectation itself is THE one shared formulation,
    plan.malformed_pin_violation — the same expectation the
    runner faults a warmup or measured slot on (through
    arms.expected_exit), applied by the checker to both kinds
    alike, so a record set the runner would never write is
    not checker-clean. Competitor records are never returned
    — their codes are observations and no expectation is
    pinned for them — and neither are designated records of
    an unpinned shape, nor records whose code was not
    observed as an integer (a missing or untyped code carries
    no answer; the round-27 F4 hold names it, and the
    dimension is not evaluated by it)."""
    violations = []
    for record in records:
        if record.get("kind") not in ("warmup", "measured") \
                or record.get("path") != "malformed" \
                or record.get("arm") != DESIGNATED_ARM:
            continue
        if malformed_pin_violation(
                DESIGNATED_ARM, _record_variant(record),
                record.get("exit_code")) is not None:
            violations.append(record)
    return violations


def _collected_record_state(dimension, records):
    """The record-observed state of a collected dimension whose
    evidence is the run's records: "PASS", "FAIL" or
    "UNEVALUATED" (the collection is absent or incomplete in
    these records, so the dimension is not evaluated by them).

    - (3) resource cost (round-27 F3): computed PER ARM over the
      measured records. A record is accounted when each resource
      field's KEY is present and its value is either a plausible
      numeric value (cpu_ms finite and non-negative;
      peak_working_set_bytes integer-valued, finite and
      strictly positive — round-28 F3: a peak of 0 is no
      measurement; round-29 F2: a fractional peak is none
      either) or null with its resource_note reason FROM THE
      RUNNER'S CLOSED COLLECTION-FAILURE VOCABULARY (round-35
      F1: any other note, or none, leaves the record
      UNACCOUNTED — the runner writes a closed set of
      collection-failure notes, and a note outside it is a
      string no runner writes; round-29
      F3: a missing KEY is accounted in no state — the runner
      never deletes a key — with or without a retained note);
      a record carrying a field in no accountable form leaves
      the collection incomplete and the dimension UNEVALUATED, as
      does a run with no numeric measurement anywhere (notes
      alone collect nothing). When the collection stands but an
      arm's measured records carry no numeric cpu_ms or no
      numeric peak_working_set_bytes at all, that arm is
      unmeasured on the cost dimension and the dimension FAILs —
      one arm's measurements cannot pass another arm's cost.
      Coverage is necessary but no longer sufficient (round-31
      F1, rebuilt round-32 F1, rebuilt round-33 F3, rebuilt
      round-34 F1): the state
      then computes the ARM-COMPARISON verdict
      (_paired_cost_comparison) — no pairing by rep index: per
      counter, per (path, context, variant) cell and per
      competitor, the comparison is made only over EQUAL
      numeric populations at a per-unit coverage floor (a side
      numeric on only a minority of its planned records in the
      cell renders the unit unpaired), and the designated arm's
      SORTED plausible numeric values must be at or below the
      competitor's at EVERY rank (sorted-rank dominance, per
      counter — cpu_ms and peak_working_set_bytes alike;
      equality at a rank is a tie at that rank, and total-cost
      and peak-mean sensitivity are implied by full-vector
      dominance at equal cardinality, no aggregate standing
      beside it); a designated value
      above the competitor's at any rank on either counter in
      any compared
      cell is a cost loss and the dimension FAILs, and a cell
      that is one-sided, cardinality-unequal or below the
      coverage floor is NAMED in the comparison's unpaired
      list and renders the dimension UNEVALUATED (the
      comparison cannot be made there, and PASS may not be
      rendered silently over it). Round-35 F1: a cell whose
      counter population is CENSORED on either side — records
      standing null under a collection-failure note — is
      NAMED in the unpaired list as a censored unit likewise
      and renders the dimension UNEVALUATED: under the
      worst-case imputation (every censored value resolved
      against the designated arm's claim) dominance over the
      full population cannot be certified there, however
      sparse the censoring. Round-36 F1/F2/F4: the comparison
      enumerates its units from the planned population, so a
      cell censored on every record of both arms is named in
      the unpaired list (never silently absent, never a PASS);
      the note accounting is per field (a null is accounted
      only by the note the runner pairs with that field's
      failure — the GetProcessMemoryInfo note accounts a peak
      null beside a numeric cpu, every other vocabulary note
      accounts a null only when both counters stand null);
      and a unit's censored counts bind accounted nulls only
      — a null the pairing does not account, and a missing
      key, are named in the inconsistency channel as what
      they are and count toward no censored class.
      Round-37 F1: the enumeration's planned population is
      build_plan()'s measured slots (the anchor this
      function's (4)/(6) limbs already use), not the judged
      records — a planned cell whose records are deleted
      entirely renders its units unpaired with their planned
      counts and zero present, and the dimension cannot PASS
      over it.
      PASS only when every arm carries numeric coverage of both
      counters AND the designated arm wins or ties every
      comparison (the ledger states that coverage and, on a
      cost FAIL, the losses; see claim_ledger).
    - (4) scaling: PASS when every scaling slot of the plan is
      present exactly as a measured record, every such record
      is "ok", and every such record publishes exactly its
      variant's descendant count in distinct PIDs under the
      variant-aware exclusion (round-29 F4: wide excludes the
      arm_pid and child_pid roots the runner records
      separately; deep counts the published list itself, whose
      level-1 root it carries, a root also named in child_pid
      neither adding nor subtracting) (round-27 F1 as
      tightened by round-28 F2: an "ok" record publishing
      fewer — or one PID repeated to the count, or an excluded
      root offered as a descendant — did not present the named
      tree — the
      record-consistency discipline names it, and the dimension
      is not evaluated by it); FAIL when a scaling record
      carries a product fault (descendant-survived /
      exit-mismatch); otherwise UNEVALUATED.
    - (6) malformed inputs: PASS when every malformed slot of
      the plan is present exactly as a measured record and every
      such record is "ok" (presented, and answered with the
      recorded exit code — each such record must carry that
      code as an integer, round-27 F4: the code IS this
      dimension's observation, so a record without it carries
      no observation) AND the designated arm meets its pinned
      expectations (round-31 F2; round-32 F2: the shape is the
      one the record's SLOT encodes in the plan, never its
      carried variant label, and the one shared formulation —
      plan.malformed_pin_violation — binds warmup records as
      the runner binds them): a designated record of either
      kind answering a contract-pinned shape with an observed
      integer code other than the pinned refusal (125) FAILs
      the dimension outright — before the "ok" reading, so a
      designated acceptance (exit 0) under an "ok" label is a
      FAIL, never a PASS — while competitor codes of any value
      are observations and move nothing; otherwise UNEVALUATED
      — an infrastructure failure on this path means a shape
      was not presented, which evaluates nothing about any arm."""
    if dimension == DIM_RESOURCE:
        measured = [r for r in records if r.get("kind") == "measured"]
        if not measured:
            return "UNEVALUATED"
        for record in measured:
            if not _resource_field_accounted(record, "cpu_ms"):
                return "UNEVALUATED"
            if not _resource_field_accounted(
                    record, "peak_working_set_bytes"):
                return "UNEVALUATED"
        coverage = _resource_coverage(records)
        if not any(cpu or peak for _total, cpu, peak
                   in coverage.values()):
            return "UNEVALUATED"
        for _arm, (_total, cpu, peak) in coverage.items():
            if not cpu or not peak:
                return "FAIL"
        cost_losses, unpaired = _paired_cost_comparison(records)
        if cost_losses:
            return "FAIL"
        if unpaired:
            return "UNEVALUATED"
        return "PASS"
    if dimension in (DIM_SCALING, DIM_SECURITY):
        path = "scale" if dimension == DIM_SCALING else "malformed"
        expected = Counter(
            (s.arm, s.context, s.rep) for s in build_plan()
            if s.kind == "measured" and s.path == path)
        try:
            actual = Counter(
                (r.get("arm"), r.get("context"), r.get("rep"))
                for r in records
                if r.get("kind") == "measured" and r.get("path") == path)
        except TypeError:
            # Total (round-30 F2): a record whose rep is unhashable
            # cannot be counted against the plan at all, so the
            # collection cannot match it — UNEVALUATED, the state
            # for a collection that does not stand — while
            # slot_accounting_losses names the record itself.
            return "UNEVALUATED"
        if not expected or actual != expected:
            return "UNEVALUATED"
        path_records = [r for r in records
                        if r.get("kind") == "measured"
                        and r.get("path") == path]
        outcomes = [r.get("outcome") for r in path_records]
        if dimension == DIM_SECURITY \
                and _designated_malformed_violations(records):
            # Round-31 F2: the designated arm answered a
            # contract-pinned shape with a code other than the
            # pinned refusal — a product fault on this
            # dimension, FAIL on the (4)/(8) shape, whatever
            # label the record carries and whatever the
            # competitors answered.
            return "FAIL"
        if all(outcome == "ok" for outcome in outcomes):
            if dimension == DIM_SCALING:
                for record in path_records:
                    variant = _scale_variant_of(record)
                    owed = expected_descendants(variant)
                    published = record.get("descendant_pids")
                    if owed is not None and not (
                            isinstance(published, list)
                            and _scale_count_matches(record, owed)):
                        return "UNEVALUATED"
            if dimension == DIM_SECURITY:
                for record in path_records:
                    code = record.get("exit_code")
                    if not isinstance(code, int) \
                            or isinstance(code, bool):
                        return "UNEVALUATED"
            return "PASS"
        if dimension == DIM_SCALING and any(
                outcome in ("descendant-survived", "exit-mismatch")
                for outcome in outcomes):
            return "FAIL"
        return "UNEVALUATED"
    return "UNEVALUATED"

# The digest form the runner writes for every observed stream:
# hashlib.sha256(...).hexdigest() — 64 lowercase hex characters.
_SHA256_HEX = re.compile(r"[0-9a-f]{64}")


def _is_sha256_hex(value):
    return isinstance(value, str) \
        and _SHA256_HEX.fullmatch(value) is not None


def _stream_expectation(spec, key):
    """The (digest, length) pair of one stream's expected bytes
    under one child spec, or None when the spec names no expected
    bytes for that stream (the runner records a digest there but
    no verdict, so no verdict's evidence is held to a pair)."""
    expected = spec.get(key) if isinstance(spec, dict) else None
    if not isinstance(expected, str):
        return None
    data = expected.encode("utf-8")
    return hashlib.sha256(data).hexdigest(), len(data)


def expected_output_from_config(config):
    """The expected output bytes of a run's configuration, as the
    (digest, length) pair per stream a match verdict's evidence
    is held to: {"fast": {...}, "floor_spawn": {...}}, each
    mapping "stdout" / "stderr" to a pair or None. The governing
    spec per arm is chosen as the runner chooses it
    (runner.output_spec_for): the floor-spawn arm's slots are
    governed by the floor_spawn child spec when one is
    configured, every other fast slot by the fast child spec —
    the configuration's children entries whose expected_stdout /
    expected_stderr the verdicts claim to match. Returns None
    when the configuration carries no children object to derive
    expectations from."""
    if not isinstance(config, dict):
        return None
    children = config.get("children")
    if not isinstance(children, dict):
        return None
    fast_spec = children.get("fast")
    if not isinstance(fast_spec, dict):
        fast_spec = {}
    floor_spec = children.get("floor_spawn")
    if not isinstance(floor_spec, dict) or not floor_spec:
        floor_spec = fast_spec

    def streams(spec):
        return {
            "stdout": _stream_expectation(spec, "expected_stdout"),
            "stderr": _stream_expectation(spec, "expected_stderr"),
        }

    return {"fast": streams(fast_spec),
            "floor_spawn": streams(floor_spec)}


def _expected_pair(expected_output, record, stream):
    """The (digest, length) pair a record's match verdict for one
    stream is held to, under an expected-output mapping of the
    shape expected_output_from_config builds, or None when no
    expectation governs that record and stream."""
    if not isinstance(expected_output, dict):
        return None
    spec = expected_output.get(
        "floor_spawn" if record.get("arm") == "floor-spawn"
        else "fast")
    if not isinstance(spec, dict):
        return None
    pair = spec.get(stream)
    return pair if isinstance(pair, tuple) and len(pair) == 2 else None

# Which freeze-gate dimensions map into which profile dimension.
# Profile dimension (1) carries kill-path correctness, exit-code
# correctness and output correctness; profile dimension (2)
# carries fast-path latency and kill-path latency. All other
# profile dimensions have no gate dimension mapped into them.
_PROFILE_GATE_MAP = {
    "Correctness and output quality": (
        "kill-path correctness", "exit-code correctness",
        "output correctness"),
    "Latency, throughput, and startup time": (
        "fast-path latency", "kill-path latency"),
}

# Prefix marking a competitor-loss evidence entry in the
# standing-losses listing. Such an entry records a competitor's
# descendant-survived kill slot — the guard's win on kill-path
# correctness — and is not a campaign standing loss: render()
# separates entries carrying this prefix and they never block a
# complete report.
COMPETITOR_LOSS_PREFIX = (
    "competitor loss (evidence, not a campaign standing loss): ")

# The standing loss a reduced run carries by shape alone. A run
# made with --reduced is a logic exercise, whatever N it ran:
# truncating at the full plan size changes nothing about the
# records, so the reduced flag itself — not missing slots — must
# name the reason the run cannot certify. campaign_losses()
# adds this entry whenever the run's reduced shape is in force,
# so every outcome computed from it (the harness's recorded
# campaign_status and the report's STATUS line alike) is
# incomplete for a reason the loss list itself states.
REDUCED_SHAPE_LOSS = (
    "reduced run shape (--reduced): this run was made with "
    "--reduced, a logic exercise that can never be a result; "
    "the full run with no --reduced is the only certifying shape")


def _is_plausible_elapsed(value):
    """An elapsed time the runner's monotonic clock can produce
    (round-30 F2): a finite, non-negative, non-boolean number —
    the same rule harness._is_plausible_elapsed states and the
    shape discipline names records against. Total, through
    _is_finite_number: a huge int resolves to implausible, never
    an exception."""
    return _is_finite_number(value) and value >= 0


def _median(records, arm, path, context, kind="measured"):
    """Median of one cell's records of one kind in one context only.
    Warmup records are not measurements and control records are not
    product records: pooling kinds would let un-warmed or control
    observations move a headline median, and pooling contexts would
    average away a context-specific cost. Only "ok" records count:
    an exit mismatch or an infrastructure failure is not a time.

    THE AGGREGATION RULE for extreme values (round-30 F2): only
    PLAUSIBLE elapsed values enter the median — a finite,
    non-negative, non-boolean number, the values the elapsed
    shape discipline admits (see _is_plausible_elapsed). An
    implausible value (a huge int such as 10**400, a non-finite
    or negative number, a wrong-typed value) is EXCLUDED here by
    that stated rule, and is NAMED where the discipline names
    record values: the harness's shape check names an implausible
    elapsed_ms on the record itself. A cell whose values are all
    implausible has no median (None), exactly as a cell with no
    ok records has none — so no aggregation in the outcome /
    render / freeze-gate / decompose chain ever averages two
    extreme values (statistics.median's even-count averaging of
    two 10**400 ints raised OverflowError before this rule), and
    a single extreme value is never returned AS a cell's median
    or floor. The honest fixtures are untouched by the rule:
    every elapsed value they carry is plausible, so their
    medians, floors and outcomes recompute byte-identically.

    Round-40 F3: only records matching a plan slot enter the
    median — the populations decompose, floor_ms and the
    freeze gate read through this function are planned
    populations, so ninety fabricated fast-999 records move
    no median, erase no pricing standing loss and shift no
    rendered decomposition distance."""
    values = [r["elapsed_ms"] for r in _planned_records(records)
              if r["arm"] == arm and r["path"] == path
              and r["outcome"] == "ok"
              and r.get("kind") == kind
              and r["context"] == context
              and _is_plausible_elapsed(r.get("elapsed_ms"))]
    return statistics.median(values) if values else None


def floor_ms(records, context):
    """The machine/harness floor of one context: the agreement of
    the two minimal floor arms in that context. Returns
    (floor, agreed). When the two disagree beyond tolerance there is
    no floor for that context and no distance may be reported there.
    A context with no floor records has no floor: (None, False)."""
    if context is None:
        raise ValueError("floors are per-context; name the context")
    direct = _median(records, "floor-direct", "fast", context, kind="control")
    spawn = _median(records, "floor-spawn", "fast", context, kind="control")
    if direct is None or spawn is None:
        return None, False
    floor = max(direct, spawn)
    if floor <= 0:
        return None, False
    agreed = abs(direct - spawn) / floor <= FLOOR_AGREEMENT_TOLERANCE
    return floor, agreed


def _slot_key(record):
    return (record.get("kind"), record.get("arm"), record.get("path"),
            record.get("context"), record.get("rep"))


def _cell_name(key):
    kind, arm, path, context, _rep = key
    return f"{kind} {arm}/{path}/{context}"


def _rep_summary(reps):
    try:
        reps = sorted(reps)
    except TypeError:
        # Total (round-30 F2): reps of mixed, mutually unorderable
        # types are summarised in repr order rather than raising —
        # the summary names them, it does not judge them.
        reps = sorted(reps, key=repr)
    if len(reps) <= 6:
        return "reps " + ", ".join(str(r) for r in reps)
    return f"reps {reps[0]}..{reps[-1]}"


def slot_accounting_losses(records):
    """Exact per-cell accounting against the run's configured plan:
    each plan slot present exactly once, by kind/arm/path/context/rep.

    Total over record values (round-30 F2): a record whose slot
    key carries an UNHASHABLE component (a rep that is a list, say)
    cannot be counted into the plan's accounting at all — it is
    NAMED here as a loss in its own right (one loss per such
    record, stating its cell as far as the record states it)
    instead of raising TypeError out of the Counter, and the
    remaining records are accounted exactly as before."""
    losses = []
    countable = []
    for record in records:
        key = _slot_key(record)
        try:
            hash(key)
        except TypeError:
            losses.append(
                f"1 record(s) match no plan slot: "
                f"{_cell_name(key[:4] + (0,))} "
                f"rep {key[4]!r} is not a hashable slot identity "
                f"the plan's accounting can count")
            continue
        countable.append(record)
    records = countable
    expected = Counter(
        (s.kind, s.arm, s.path, s.context, s.rep) for s in build_plan())
    actual = Counter(_slot_key(r) for r in records)
    missing = {}
    for key, count in expected.items():
        short = count - actual.get(key, 0)
        if short > 0:
            missing.setdefault(key[:4], []).extend([key[4]] * short)
    for cell, reps in sorted(missing.items()):
        losses.append(
            f"missing {len(reps)} slot(s): {_cell_name(cell + (0,))} "
            f"{_rep_summary(reps)}")
    duplicated = {}
    for key, count in actual.items():
        extra = count - expected.get(key, 0)
        if extra > 0 and key in expected:
            duplicated.setdefault(key[:4], []).append((key[4], count))
    for cell, items in sorted(duplicated.items()):
        for rep, count in sorted(items):
            losses.append(
                f"duplicate records for {_cell_name(cell + (0,))} "
                f"rep {rep}: {count} copies of one plan slot")
    unexpected = {}
    for key, count in actual.items():
        if key not in expected:
            unexpected.setdefault(key[:4], []).extend([key[4]] * count)
    for cell, reps in sorted(unexpected.items()):
        losses.append(
            f"{len(reps)} record(s) match no plan slot: "
            f"{_cell_name(cell + (0,))} {_rep_summary(reps)}")
    return losses


def _canonical_executable_path(token):
    """One executable token under the campaign platform's
    path semantics (round-42 F1, extended by round-43 F1),
    as the component sequence the platform resolves it to:
    separators unified to the platform's own, case folded
    (the platform's filesystem is case-insensitive), empty
    and current-directory components dropped (a leading
    ./, a root prefix and a drive letter state where the
    spelling starts, not another binary), trailing dots
    and trailing spaces stripped from every component
    (the platform's normalization strips them when it
    resolves a spelling — build. and sysprims.exe. name
    build and sysprims.exe), and an omitted extension on
    the final component completed with the loader's own
    completion (.exe — a program name stated without an
    extension is completed when the loader resolves it,
    so sysprims stated against a configured sysprims.exe
    names that binary). The strip applies only to a
    component carrying other content (round-44 F1): a
    component made solely of dots and spaces is NOT
    stripped — stripped, it would vanish, and a vanished
    component manufactures an identity the unresolved
    classes below refuse. Such a component canonicalizes
    as itself when it is exactly "." or "..", and
    otherwise stands in the sequence verbatim (a padded
    ".. " or ".. .", an all-dots "...", an all-space
    component), so it never agrees with a configured
    sequence the platform spelling does not denote.
    Two spelling classes are NOT resolved, and are named
    here as the breadth's limits: parent-relative
    components — the plain ".." and every padded spelling
    of it (".. ", ".. ."), which stand in the canonical
    sequence unresolved exactly as the plain spelling
    does — resolving one would need the campaign
    directory the record set does not carry; and 8.3
    short names (SYSPRI~1.EXE) — resolving one would
    need filesystem state (the long name the directory
    entry carries) the record set does not carry either.
    The canonicalization is stated at exactly this
    breadth."""
    components = []
    for raw in token.replace("/", "\\").split("\\"):
        if raw in (".", ".."):
            component = raw
        elif raw and all(char in ". " for char in raw):
            # Solely dots/spaces, and not exactly "." or
            # "..": the strip would delete it outright.
            # It stands verbatim — unresolved, never an
            # identity (round-44 F1).
            component = raw
        else:
            component = raw.rstrip(". ")
        if component in ("", "."):
            continue
        if not components and len(component) == 2 \
                and component[1] == ":":
            continue
        components.append(component.casefold())
    if components and "." not in components[-1] \
            and any(char not in ". " for char in components[-1]):
        components[-1] = components[-1] + ".exe"
    return "\\".join(components)


def _same_executable(token, executable):
    """True when token denotes executable's binary under
    the platform's path semantics (round-42 F1, confined
    by round-43 F1): their canonical component sequences
    are equal, or the token's sequence is the configured
    sequence with a prefix prepended — the absolute
    spelling (a canonical token sequence that ends with
    the configured sequence in full, at a component
    boundary: an absolute path whose tail is the
    configured relative path names the binary that
    relative path names, from the campaign root the
    platform resolves it against; the device prefixes
    \\?\\ and \\.\\ are such prefixes). Identification
    never runs the other way: a token that is a proper
    suffix of the configured sequence — a basename only
    (sysprims.exe), a partial tail (competitors\\
    sysprims.exe) — states strictly less about which
    binary ran than a token with an explicit different
    directory does, and identifies nothing here; a
    genuinely different directory never agrees either."""
    if not isinstance(token, str) or not isinstance(executable, str) \
            or not token or not executable:
        return False
    canonical_token = _canonical_executable_path(token)
    canonical_executable = _canonical_executable_path(executable)
    if not canonical_token or not canonical_executable:
        return False
    return canonical_token == canonical_executable \
        or canonical_token.endswith("\\" + canonical_executable)


def _record_inconsistencies(records, expected_output=None,
                            config=None):
    """Every disagreement between a record's classification and
    the observations stored in that same record, as a list of
    {"record", "field", "slot", "detail"} items. expected_output,
    when given, is the expected-bytes mapping
    expected_output_from_config builds from the run's
    configuration; a True match verdict's digest and length are
    then held to the expected bytes themselves. The rules:

    - exit_code: a record not classified "exit-mismatch" whose
      exit_code differs from the expected exit for its arm and
      path (arms.expected_exit's table) contradicts its label —
      an honestly classified record with that exit code would be
      "exit-mismatch". A record with no observed exit_code is
      flagged only when it is classified "ok" (the runner always
      records the exit code of an ok slot; infrastructure
      failures carry none by shape). Kill records classified
      "descendant-survived" are exempt: the runner applies
      survivor-before-exit precedence there (see the module
      docstring), so the mismatching exit code does not
      contradict that label; standing_losses() lists the exit
      fault separately instead.
    - survivor: an "ok" kill record whose survivor field is not
      False claims a clean kill its own survivor observation
      contradicts (or never made — the runner never classifies
      a kill slot "ok" with survivor unanswered).
    - grandchild_pid / child_pid / arm_pid: an "ok" kill record missing
      its grandchild_pid, or carrying it null or non-integer,
      rests its survivor classification on a descendant identity
      that is not in the record — the grandchild is the
      descendant the shipped kill workload records at the slot's
      marker file and the survivor probe judges. arm_pid — the
      arm process's own PID, the first identity of the group
      README enumerates, recorded by the runner for every
      spawned slot — is held to the same PID form on "ok" and
      "descendant-survived" kill records alike (round-24).
      child_pid is
      recorded where the workload records it (the stub tree
      records it; the shipped testkit tree records only the
      grandchild, at its marker), so a missing or null child_pid
      contradicts nothing; a child_pid that is present but not
      an integer PID is a field the record cannot evidence and
      is flagged the same way. An integer identity is held to
      the PID form as well: a grandchild_pid (or a present
      child_pid) that is not a positive integer within the
      platform PID range (1 to MAX_PID) names no descendant the
      survivor probe ever judged, and is flagged the same way.
      The same hold reaches the "descendant-survived" kill
      records the ok rule's shape stopped short of: that
      classification rests on exactly the same identity (the
      runner invokes the survivor probe with the recorded tree
      PIDs — no recorded PID, no probe answer, never a
      survival), so a descendant-survived kill record carrying
      no PID-form grandchild_pid (or a present child_pid
      outside the PID form) is flagged the same way, and its
      survival is earned nowhere (see _evidenced_survived).
    - exit_code types on the fault-claiming labels: an
      "exit-mismatch" or "descendant-survived" record whose
      exit_code differs from the expected code but is not an
      integer (a string, a float, a boolean, a list) claims an
      exit fault from a value no instrument recorded — the
      runner records an integer exit code or none — and is
      flagged here; _evidenced_exit_mismatch earns the fault
      only from a non-boolean integer differing from the
      expected code. (An INTEGER mismatching code under a
      descendant-survived label stays exempt, as above: it is
      listed by standing_losses() under its own name, not as
      an inconsistency.)
    - elapsed_ms: an "ok" record of a kind whose elapsed time
      feeds a median — measured, and control (the floor median
      is the median of the floor arms' control records) — whose
      elapsed_ms is None or missing carries no time for that
      median; the median must never silently drop it, so the
      missing time is named here instead. Warmup records feed
      no median, so an unconsumed warmup elapsed time certifies
      nothing and is not held here.
    - The negative classifications, in the other direction: an
      "exit-mismatch" record whose exit_code is the expected
      code, or was never observed, claims a mismatch its own
      exit_code field refutes; a "descendant-survived" record
      whose survivor field is not True claims a survival its
      own survivor observation refutes (the runner classifies
      a kill slot "descendant-survived" only from a probe
      answer of True); an "output-mismatch" record in which
      neither stream's match verdict is False claims an output
      failure neither stream records.
    - Output evidence on the fast path: an "ok" fast record in
      which a stream's match verdict is False contradicts its
      label in the same way an "ok" kill record with survivor
      True does; and a fast record carrying a match verdict
      (True or False) for a stream but no digest or no length
      for that stream rests the verdict on evidence the record
      does not carry — the runner writes the digest and the
      length with every verdict it records. The digest beside a
      verdict must also be a digest: the 64-hex form the
      runner's hexdigest writes, not any non-empty string. And
      when the expected bytes are supplied (expected_output), a
      True verdict asserts the emitted bytes ARE the expected
      bytes, so its digest and length must equal the expected
      bytes' digest and length; evidence describing other bytes
      under a True verdict certifies output the record's own
      fields refute. (A False verdict's evidence describes the
      bytes actually emitted, which differ from the expected
      bytes by the verdict's own claim, so it is held to the
      digest form, not to the expected pair.)
    - Verdict-field types: a match verdict that is neither a
      boolean nor null is not a verdict the runner writes, and
      the output gate would read it as observed; an exit_code
      that equals the expected code in value but is not an
      integer (a float, above all) is not the exit code the
      runner records — comparisons by equality alone would let
      both pass unnamed.
    - Outcome x path pairing: a "descendant-survived" record
      whose path is not "kill", and an "output-mismatch" record
      whose path is not "fast", carry a classification their
      path never writes — the runner writes the survival
      classification only on the kill path, from a survivor
      answer of True, and the output classification only on the
      fast path, after the exit code matched, from a False
      stream verdict — and are flagged here, in the same shape
      as the label-earning rules above.
    - survivor under the remaining kill labels: a kill record
      classified "exit-mismatch" or "output-mismatch" whose
      survivor field is True states a survival its label does
      not carry — the runner's survivor-before-exit precedence
      classifies a returned kill arm with survivor True as
      "descendant-survived" before its exit code is ever
      compared, so that observation under a returned-arm label
      is a shape no run produces (the timed-out
      infrastructure failure is the one shape that records
      survivor True under another label, and it keeps the
      "infra-failure" label, not a returned-arm one).
    - exit_code presence on the survived label: a
      "descendant-survived" record whose exit_code is None
      carries a survival classification the runner derived
      beside an exit code it always records (the wait comes
      before the probe and the label), so the unobserved code
      is flagged here in the "ok" rule's shape; the
      infrastructure-failure shape that carries no exit code
      is a different label and is not held here.
    - exit_code presence on the output label: an
      "output-mismatch" record whose exit_code is None carries
      an output classification the runner derived beside an
      exit code it always records — the wait comes first, the
      code is compared against the expected code, and only on
      a match is a False stream verdict classified
      "output-mismatch" — so the unobserved code is flagged
      here in the same shape, and the label mints no output
      loss (see _evidenced_output_mismatch).
    - Per-path field discipline: a fast record whose survivor
      field is not None carries an observation the fast path
      never makes (survivor is set only in the runner's kill
      block), and a kill record carrying any non-null output_*
      field (a stream verdict, digest or length) carries an
      observation the kill path never makes (kill-path slots
      carry no output observation) — each non-null field is
      flagged here, in the pairing idiom above.
    - argv / arm_pid on every record (round-25): the runner
      writes both for every slot — argv as the executed argument
      vector (a non-empty list of string tokens) before the
      spawn attempt, arm_pid as the spawned arm's own PID — so
      every record is held to those forms, fast and control
      records included; the spawn-failure infrastructure
      failure, whose arm never spawned, is the one shape that
      carries no arm_pid by construction.
    - Tree-identity field pairing, in both directions: a fast
      record carrying a non-null child_pid, grandchild_pid or
      tree_marker carries a tree identity its path never
      writes (all three are written only in the runner's kill
      block; the marker is created only for kill slots), and
      a kill record whose tree_marker is not a non-empty
      string has lost the marker its slot always writes (the
      runner creates the slot's marker path and records it
      before the arm spawns, on every kill slot, whatever the
      outcome) — the record's survivor judgement rests on a
      marker file the record no longer names.
    - Resource fields (round-27 F3; round-28 F3/F4;
      round-29 F2/F3/F6): every record — measured, warmup
      and control alike — is held to the runner's resource
      instrumentation: each present cpu_ms /
      peak_working_set_bytes must be a plausible counter
      (cpu_ms finite and non-negative;
      peak_working_set_bytes integer-valued, finite and
      strictly positive — a peak of 0, or a fractional peak,
      is no measurement any instrument recorded, held as an
      inconsistency like a negative or NaN value; the
      predicates are total — a huge value is named, never
      an exception); a null field must carry its
      resource_note reason from the runner's CLOSED
      collection-failure vocabulary (round-35 F1 — any other
      note, or none, leaves the record UNACCOUNTED and is
      named here); and a field whose KEY is
      deleted from the record is named in its own right,
      with or without a retained note, in every record
      state — the runner constructs every record with both
      keys and never deletes one, so the note excuses a
      null value only, never a missing key. The hold keys
      on the record, not on the fields' presence.
    - Published count on the scale path (round-27 F1;
      round-28 F2; round-29 F4): an "ok" scale record must
      publish exactly its variant's descendant count in
      DISTINCT PIDs under the variant-aware exclusion
      (wide-N publishes N distinct, the arm_pid and
      child_pid roots the runner records separately
      excluded; deep-D, levels 1..D, publishes D distinct
      in the list itself, which carries the level-1 root —
      a root also named in child_pid neither adds nor
      subtracts) — the record claims the named tree was
      built and torn down, and a tree published at any
      other distinct count — one PID repeated to the
      length, or an excluded root offered as a descendant
      — was not presented in full.
    - Exit code on the malformed path (round-27 F4): no
      expected code is pinned for that path as a whole, so the
      recorded exit code is dimension (6)'s observation itself; a
      malformed record classified from a returned arm (any
      outcome but "infra-failure") must carry it, and any code
      a malformed record carries must be the integer form the
      runner records. Round-31 F2 adds the designated-only
      exception, round-32 F2 binds it to the slot's encoded
      shape and to both slot kinds: a WARMUP OR MEASURED
      malformed record of the designated arm whose shape —
      re-derived from its (path, rep, kind) slot identity,
      never read from its carried variant label — its
      contract pins is held to the pinned refusal code by
      the ordinary exit rules (a differing integer code
      under another label, or the pinned code under an
      "exit-mismatch" label, contradicts the record),
      scoped exactly to where an expectation is pinned —
      no competitor or unpinned-shape record acquires one.
    - Variant identity on the malformed path — the pin's own
      path (round-32 F2): a carried variant label that disagrees
      with the shape its record's slot encodes in the plan
      is named in its own right — the label is a claim about
      the slot, and the slot's (path, rep, kind) identity,
      whose rep numbering is variant-major, is the authority
      every derivation reads. Round-41 F5: an ABSENT label
      (missing, null, empty or untyped) is named in the same
      item channel — the runner stamps the encoded shape on
      every malformed and scale record it writes, so absence
      is an erased written field, never a legitimate state.
    - Arm identity against the record's second encoding
      (round-41 F3): the argv's executable token is held
      against the configuration's per-arm executables — a
      record whose argv[0] is ANOTHER product arm's
      configured executable, and not its filed arm's own, is
      named in its own right (see the argv block below)."""
    found = []
    fast_argv_lengths = {}
    arm_executables = {}
    for record in records:
        outcome = record.get("outcome")
        arm = record.get("arm")
        path = record.get("path")
        slot = (f"{record.get('kind')} {arm}/{path}/"
                f"{record.get('context')} rep {record.get('rep')}")
        expected = expected_exit(arm, path)
        actual = record.get("exit_code")
        if expected is not None and actual is not None \
                and actual == expected \
                and (not isinstance(actual, int)
                     or isinstance(actual, bool)):
            found.append({
                "record": record, "field": "exit_code", "slot": slot,
                "detail": f"exit_code is {actual!r}, which equals "
                          f"the expected exit {expected} for "
                          f"{arm}/{path} in value but is not the "
                          f"integer exit code the runner records; "
                          f"a verdict field is not carried by a "
                          f"value of another type that merely "
                          f"compares equal"})
        if expected is not None and outcome != "exit-mismatch" \
                and outcome != "descendant-survived" \
                and actual != expected \
                and (actual is not None or outcome == "ok"):
            found.append({
                "record": record, "field": "exit_code", "slot": slot,
                "detail": f"exit_code is {actual!r} but the expected "
                          f"exit for {arm}/{path} is {expected} and "
                          f"the outcome is {outcome!r}, not "
                          f"'exit-mismatch'"})
        if outcome == "exit-mismatch" and expected is not None \
                and (actual is None or actual == expected):
            found.append({
                "record": record, "field": "exit_code", "slot": slot,
                "detail": f"exit_code is {actual!r}, which does not "
                          f"differ from the expected exit "
                          f"{expected} for {arm}/{path}, but the "
                          f"outcome is 'exit-mismatch'; a mismatch "
                          f"classification must be earned by an "
                          f"observed exit code that differs"})
        if outcome in ("exit-mismatch", "descendant-survived") \
                and expected is not None and actual is not None \
                and (not isinstance(actual, int)
                     or isinstance(actual, bool)) \
                and actual != expected:
            found.append({
                "record": record, "field": "exit_code", "slot": slot,
                "detail": f"exit_code is {actual!r}, which differs "
                          f"from the expected exit {expected} for "
                          f"{arm}/{path} but is not the integer "
                          f"exit code the runner records (an "
                          f"integer code or none); an exit fault "
                          f"is earned only by an observed integer "
                          f"exit code that differs — a verdict "
                          f"field is not carried by a value of "
                          f"another type"})
        if outcome == "descendant-survived" and path not in ("kill", "scale"):
            found.append({
                "record": record, "field": "outcome", "slot": slot,
                "detail": f"the outcome is 'descendant-survived' "
                          f"but the record's path is {path!r}, not "
                          f"'kill'; that classification is written "
                          f"only on the kill path, from a survivor "
                          f"answer of True — a classification its "
                          f"path never writes rests on observations "
                          f"the path never makes"})
        if outcome == "output-mismatch" and path != "fast":
            found.append({
                "record": record, "field": "outcome", "slot": slot,
                "detail": f"the outcome is 'output-mismatch' "
                          f"but the record's path is {path!r}, not "
                          f"'fast'; that classification is written "
                          f"only on the fast path, after the exit "
                          f"code matched, from a False stream "
                          f"verdict — a classification its path "
                          f"never writes rests on observations the "
                          f"path never makes (a kill record carries "
                          f"no output observation at all)"})
        if outcome == "descendant-survived" and actual is None:
            found.append({
                "record": record, "field": "exit_code", "slot": slot,
                "detail": f"exit_code is None but the outcome is "
                          f"'descendant-survived'; the runner "
                          f"records the exit code from its wait "
                          f"before it classifies survival, so a "
                          f"survived record always carries the "
                          f"observed exit code the classification "
                          f"was derived beside — the shape that "
                          f"carries no exit code is an "
                          f"infrastructure failure, a different "
                          f"label"})
        if outcome == "output-mismatch" and actual is None:
            found.append({
                "record": record, "field": "exit_code", "slot": slot,
                "detail": f"exit_code is None but the outcome is "
                          f"'output-mismatch'; the runner records "
                          f"the exit code from its wait, compares "
                          f"it against the expected code, and only "
                          f"on a match classifies a False stream "
                          f"verdict as 'output-mismatch', so an "
                          f"output-mismatch record always carries "
                          f"the observed exit code the "
                          f"classification was derived beside — "
                          f"the shape that carries no exit code "
                          f"is an infrastructure failure, a "
                          f"different label"})
        if path == "fast" and record.get("survivor") is not None:
            found.append({
                "record": record, "field": "survivor", "slot": slot,
                "detail": f"survivor is {record.get('survivor')!r} "
                          f"but the record's path is 'fast'; the "
                          f"runner sets the survivor field only "
                          f"in its kill block, so a fast record "
                          f"carries no survivor observation — a "
                          f"non-null value in a field its path "
                          f"never writes rests on an observation "
                          f"the path never makes"})
        if path == "fast":
            for field in ("child_pid", "grandchild_pid",
                          "tree_marker"):
                if record.get(field) is not None:
                    found.append({
                        "record": record, "field": field,
                        "slot": slot,
                        "detail": f"{field} is "
                                  f"{record.get(field)!r} but the "
                                  f"record's path is 'fast'; the "
                                  f"tree-identity fields are "
                                  f"written only in the runner's "
                                  f"kill block (the marker is "
                                  f"created only for kill slots), "
                                  f"so a fast record carries no "
                                  f"descendant tree identity — a "
                                  f"non-null value in a field its "
                                  f"path never writes states the "
                                  f"identity of a tree the path "
                                  f"never spawned"})
        if path in ("kill", "scale"):
            marker = record.get("tree_marker")
            if not (isinstance(marker, str) and marker):
                found.append({
                    "record": record, "field": "tree_marker",
                    "slot": slot,
                    "detail": f"tree_marker is {marker!r} but "
                              f"the record's path is 'kill'; the "
                              f"runner creates the slot's marker "
                              f"path and records it as tree_marker "
                              f"before the arm spawns, on every "
                              f"kill slot, so a kill record always "
                              f"names the marker file its survivor "
                              f"judgement rests on — a missing, "
                              f"null or unformed tree_marker is a "
                              f"written field erased, not a value "
                              f"its path never writes"})
        if path in ("kill", "scale"):
            for field in ("output_stdout_match",
                          "output_stderr_match",
                          "output_stdout_sha256",
                          "output_stderr_sha256",
                          "output_stdout_len",
                          "output_stderr_len"):
                if record.get(field) is not None:
                    found.append({
                        "record": record, "field": field,
                        "slot": slot,
                        "detail": f"{field} is "
                                  f"{record.get(field)!r} but the "
                                  f"record's path is 'kill'; "
                                  f"kill-path slots carry no "
                                  f"output observation (the kill "
                                  f"child's product is the tree "
                                  f"it leaves, judged by the "
                                  f"survivor probe), so every "
                                  f"output field stays null on "
                                  f"this path — a non-null value "
                                  f"in a field its path never "
                                  f"writes rests on an observation "
                                  f"the path never makes"})
        if path in ("kill", "scale") \
                and outcome in ("exit-mismatch", "output-mismatch") \
                and record.get("survivor") is True:
            found.append({
                "record": record, "field": "survivor", "slot": slot,
                "detail": f"survivor is True but the outcome is "
                          f"{outcome!r}; the runner classifies a "
                          f"returned kill arm that left a survivor "
                          f"as 'descendant-survived' before its "
                          f"exit code is ever compared "
                          f"(survivor-before-exit precedence), so "
                          f"a survivor observation of True under "
                          f"a returned-arm label is a shape no run "
                          f"produces — the one shape that records "
                          f"survivor True under another label is "
                          f"the timed-out infrastructure failure, "
                          f"which carries the 'infra-failure' "
                          f"label"})
        if outcome == "descendant-survived" and path in ("kill", "scale") \
                and record.get("survivor") is not True:
            found.append({
                "record": record, "field": "survivor", "slot": slot,
                "detail": f"survivor is {record.get('survivor')!r} "
                          f"but the outcome is "
                          f"'descendant-survived'; that "
                          f"classification is earned only by a "
                          f"survivor observation of True (the "
                          f"probe answered that a descendant is "
                          f"alive)"})
        if outcome == "descendant-survived" and path in ("kill", "scale"):
            if path == "kill":
                value = record.get("grandchild_pid")
                if not isinstance(value, int) \
                        or isinstance(value, bool):
                    found.append({
                        "record": record, "field": "grandchild_pid",
                        "slot": slot,
                        "detail": "grandchild_pid is missing or null "
                                  "but a descendant-survived kill "
                                  "record must carry the descendant "
                                  "tree identity the survivor probe "
                                  "judged in grandchild_pid (the "
                                  "shipped kill workload records the "
                                  "grandchild's PID at the slot's "
                                  "marker file, and the probe is "
                                  "invoked with the recorded tree "
                                  "PIDs — with no recorded PID the "
                                  "probe is never invoked and cannot "
                                  "have answered True)"})
                elif not 1 <= value <= MAX_PID:
                    found.append({
                        "record": record, "field": "grandchild_pid",
                        "slot": slot,
                        "detail": f"grandchild_pid is {value!r} but a "
                                  f"descendant-survived kill record "
                                  f"must carry the descendant tree "
                                  f"identity as a positive integer "
                                  f"PID within the platform PID range "
                                  f"(1 to {MAX_PID}); no descendant "
                                  f"the survivor probe judged ever "
                                  f"carried an identity outside that "
                                  f"range"})
            else:
                pids = record.get("descendant_pids")
                root = record.get("child_pid")
                if not (isinstance(pids, list) and pids and all(
                        isinstance(v, int)
                        and not isinstance(v, bool)
                        and 1 <= v <= MAX_PID for v in pids)) \
                        and not (isinstance(root, int)
                                 and not isinstance(root, bool)
                                 and 1 <= root <= MAX_PID):
                    found.append({
                        "record": record,
                        "field": "descendant_pids",
                        "slot": slot,
                        "detail": "descendant_pids is "
                                  f"{pids!r} but a descendant-survived "
                                  "scale record must carry "
                                  "the descendant tree "
                                  "identity the survivor "
                                  "probe judged in "
                                  "descendant_pids, or the "
                                  "tree root it judged in "
                                  "child_pid (the scaling "
                                  "workload publishes every "
                                  "tree PID at the slot's "
                                  "marker directory, and the "
                                  "probe is invoked with the "
                                  "recorded tree PIDs — with "
                                  "no recorded PIDs the probe "
                                  "cannot have judged the "
                                  "tree"})
            value = record.get("child_pid")
            if value is not None and (not isinstance(value, int)
                                      or isinstance(value, bool)):
                found.append({
                    "record": record, "field": "child_pid",
                    "slot": slot,
                    "detail": f"child_pid is {value!r} but a "
                              f"recorded child identity must be "
                              f"the integer PID the runner read "
                              f"from the slot's marker files; "
                              f"child_pid is absent (null) where "
                              f"the workload records no child PID "
                              f"and contradicts nothing there"})
            elif value is not None and not 1 <= value <= MAX_PID:
                found.append({
                    "record": record, "field": "child_pid",
                    "slot": slot,
                    "detail": f"child_pid is {value!r} but a "
                              f"recorded child identity must be "
                              f"a positive integer PID within "
                              f"the platform PID range (1 to "
                              f"{MAX_PID}) — the PID the runner "
                              f"read from the slot's marker "
                              f"files; no descendant the survivor "
                              f"probe judged ever carried an "
                              f"identity outside that range"})
            value = record.get("arm_pid")
            if not isinstance(value, int) \
                    or isinstance(value, bool):
                found.append({
                    "record": record, "field": "arm_pid",
                    "slot": slot,
                    "detail": "arm_pid is missing or null "
                              "but a descendant-survived kill "
                              "record must carry the arm's own "
                              "PID the runner records for every "
                              "spawned slot (arm_pid is the "
                              "arm process the survivor probe is "
                              "invoked alongside — with no "
                              "recorded arm PID the record's "
                              "tree identity is incomplete)"})
            elif not 1 <= value <= MAX_PID:
                found.append({
                    "record": record, "field": "arm_pid",
                    "slot": slot,
                    "detail": f"arm_pid is {value!r} but a "
                              f"descendant-survived kill record "
                              f"must carry the arm's own identity "
                              f"as a positive integer PID within "
                              f"the platform PID range "
                              f"(1 to {MAX_PID}); no arm the "
                              f"runner spawned ever carried an "
                              f"identity outside that range"})
        if path == "fast":
            for stream in ("stdout", "stderr"):
                match = record.get(f"output_{stream}_match")
                if match is not None \
                        and not isinstance(match, bool):
                    found.append({
                        "record": record,
                        "field": f"output_{stream}_match",
                        "slot": slot,
                        "detail": f"output_{stream}_match is "
                                  f"{match!r}, which is neither "
                                  f"the runner's boolean verdict "
                                  f"nor null (no verdict recorded); "
                                  f"the output gate reads any "
                                  f"non-null verdict as observed, "
                                  f"so a verdict of another type "
                                  f"would certify output the "
                                  f"record never judged"})
                if outcome == "ok" and match is False:
                    found.append({
                        "record": record,
                        "field": f"output_{stream}_match",
                        "slot": slot,
                        "detail": f"output_{stream}_match is False "
                                  f"but the outcome is 'ok'; a fast "
                                  f"record whose {stream} did not "
                                  f"match the expected bytes is an "
                                  f"output mismatch, not a success"})
                if match is not None:
                    sha = record.get(f"output_{stream}_sha256")
                    length = record.get(f"output_{stream}_len")
                    if not (isinstance(sha, str) and sha) \
                            or not isinstance(length, int) \
                            or isinstance(length, bool) or length < 0:
                        field = (f"output_{stream}_sha256"
                                 if not (isinstance(sha, str) and sha)
                                 else f"output_{stream}_len")
                        found.append({
                            "record": record, "field": field,
                            "slot": slot,
                            "detail": f"output_{stream}_match is "
                                      f"{match!r} but the record "
                                      f"carries no digest and "
                                      f"length evidencing that "
                                      f"verdict for {stream} "
                                      f"(sha256 is {sha!r}, length "
                                      f"is {length!r}); the runner "
                                      f"records the emitted bytes' "
                                      f"digest and length with "
                                      f"every verdict"})
                    elif not _is_sha256_hex(sha):
                        found.append({
                            "record": record,
                            "field": f"output_{stream}_sha256",
                            "slot": slot,
                            "detail": f"output_{stream}_match is "
                                      f"{match!r} but the digest "
                                      f"evidencing that verdict "
                                      f"for {stream} is {sha!r}, "
                                      f"which is not the 64-hex "
                                      f"SHA-256 the runner records "
                                      f"for the emitted bytes "
                                      f"(hashlib's hexdigest "
                                      f"form); a verdict rests on "
                                      f"a digest of the bytes it "
                                      f"judged, in the form the "
                                      f"runner writes"})
                    elif match is True:
                        pair = _expected_pair(
                            expected_output, record, stream)
                        if pair is not None and sha != pair[0]:
                            found.append({
                                "record": record,
                                "field": f"output_{stream}_sha256",
                                "slot": slot,
                                "detail": f"output_{stream}_match "
                                          f"is True but the "
                                          f"digest evidencing "
                                          f"that verdict for "
                                          f"{stream} is {sha!r}, "
                                          f"not the expected "
                                          f"bytes' digest "
                                          f"{pair[0]!r}; a True "
                                          f"verdict asserts the "
                                          f"emitted bytes are "
                                          f"the expected bytes, "
                                          f"and this evidence "
                                          f"describes other "
                                          f"bytes"})
                        elif pair is not None and length != pair[1]:
                            found.append({
                                "record": record,
                                "field": f"output_{stream}_len",
                                "slot": slot,
                                "detail": f"output_{stream}_match "
                                          f"is True but the "
                                          f"length evidencing "
                                          f"that verdict for "
                                          f"{stream} is "
                                          f"{length!r}, not the "
                                          f"expected bytes' "
                                          f"length {pair[1]!r}; a "
                                          f"True verdict asserts "
                                          f"the emitted bytes are "
                                          f"the expected bytes, "
                                          f"and this evidence "
                                          f"describes other "
                                          f"bytes"})
            if outcome == "output-mismatch" \
                    and record.get("output_stdout_match") is not False \
                    and record.get("output_stderr_match") is not False:
                found.append({
                    "record": record, "field": "output_stdout_match",
                    "slot": slot,
                    "detail": f"the outcome is 'output-mismatch' "
                              f"but neither stream records a False "
                              f"match verdict (stdout "
                              f"{record.get('output_stdout_match')!r}, "
                              f"stderr "
                              f"{record.get('output_stderr_match')!r}); "
                              f"an output-mismatch classification "
                              f"must be earned by a stream that did "
                              f"not match"})
        if outcome == "ok" and path in ("kill", "scale"):
            if record.get("survivor") is not False:
                found.append({
                    "record": record, "field": "survivor",
                    "slot": slot,
                    "detail": f"survivor is "
                              f"{record.get('survivor')!r} but an "
                              f"ok kill record must record survivor "
                              f"False (no descendant survived)"})
            if path == "kill":
                value = record.get("grandchild_pid")
                if not isinstance(value, int) \
                        or isinstance(value, bool):
                    found.append({
                        "record": record, "field": "grandchild_pid",
                        "slot": slot,
                        "detail": "grandchild_pid is missing or null "
                                  "but an ok kill record must carry "
                                  "the descendant tree identity the "
                                  "runner records in grandchild_pid "
                                  "(the shipped kill workload records "
                                  "the grandchild's PID at the slot's "
                                  "marker file, and the survivor probe "
                                  "judges that PID)"})
                elif not 1 <= value <= MAX_PID:
                    found.append({
                        "record": record, "field": "grandchild_pid",
                        "slot": slot,
                        "detail": f"grandchild_pid is {value!r} but an "
                                  f"ok kill record must carry the "
                                  f"descendant tree identity as a "
                                  f"positive integer PID within the "
                                  f"platform PID range (1 to "
                                  f"{MAX_PID}); no descendant the "
                                  f"survivor probe judged ever carried "
                                  f"an identity outside that range"})
            else:
                pids = record.get("descendant_pids")
                if not (isinstance(pids, list) and pids and all(
                        isinstance(v, int)
                        and not isinstance(v, bool)
                        and 1 <= v <= MAX_PID for v in pids)):
                    found.append({
                        "record": record,
                        "field": "descendant_pids",
                        "slot": slot,
                        "detail": "descendant_pids is "
                                  f"{pids!r} but a ok "
                                  "scale record must carry "
                                  "the descendant tree "
                                  "identity the survivor "
                                  "probe judged in "
                                  "descendant_pids (the "
                                  "scaling workload publishes "
                                  "every tree PID at the "
                                  "slot's marker directory, "
                                  "and the probe is invoked "
                                  "with the recorded tree "
                                  "PIDs — with no recorded "
                                  "PIDs the probe cannot have "
                                  "judged the tree either"})
                else:
                    # The count hold (round-27 F1, round-28
                    # F2, round-29 F4): an ok scale record
                    # claims the NAMED tree died, so its
                    # published count must be the variant's
                    # exact count of DISTINCT descendant PIDs
                    # — wide-N publishes N, deep-D (levels
                    # 1..D) publishes D — under the
                    # variant-aware exclusion: for wide, the
                    # record's arm_pid (the arm itself, dead
                    # by construction on a completed slot)
                    # and its child_pid (the scale tree's
                    # root, recorded separately by the
                    # runner) are excluded; for deep, the
                    # published list carries the level-1 root
                    # itself and only arm_pid is excluded — a
                    # root also named in child_pid neither
                    # adds nor subtracts. A list of the right
                    # length made of one PID repeated, or of
                    # an excluded root offered as a
                    # descendant, published a tree of one, not
                    # the named tree. Any other count is a
                    # tree that was never presented in full,
                    # whose teardown this record cannot
                    # evidence.
                    variant = _scale_variant_of(record)
                    owed = expected_descendants(variant)
                    if owed is not None and not \
                            _scale_count_matches(record, owed):
                        found.append({
                            "record": record,
                            "field": "descendant_pids",
                            "slot": slot,
                            "detail": "descendant_pids carries "
                                      f"{len(pids)} published "
                                      f"PID(s), "
                                      f"{_distinct_descendant_count(record)} "
                                      f"of them distinct descendant "
                                      f"PIDs (variant-aware: "
                                      f"{'the published list itself, which carries the level-1 root; arm_pid excluded, a root also named in child_pid neither adds nor subtracts' if isinstance(variant, str) and variant.startswith('deep-') else 'the tree roots arm_pid and child_pid, recorded separately, excluded'}), but variant "
                                      f"{variant} publishes "
                                      f"exactly {owed} distinct "
                                      f"descendants; an ok "
                                      "scale record claims the "
                                      "named tree was built and "
                                      "torn down, and a tree "
                                      "published at any other "
                                      "distinct count was not "
                                      "presented in full — its "
                                      "teardown is unverified"})
            value = record.get("child_pid")
            if value is not None and (not isinstance(value, int)
                                      or isinstance(value, bool)):
                found.append({
                    "record": record, "field": "child_pid",
                    "slot": slot,
                    "detail": f"child_pid is {value!r} but a "
                              f"recorded child identity must be "
                              f"the integer PID the runner read "
                              f"from the slot's marker files; "
                              f"child_pid is absent (null) where "
                              f"the workload records no child PID "
                              f"and contradicts nothing there"})
            elif value is not None and not 1 <= value <= MAX_PID:
                found.append({
                    "record": record, "field": "child_pid",
                    "slot": slot,
                    "detail": f"child_pid is {value!r} but a "
                              f"recorded child identity must be "
                              f"a positive integer PID within "
                              f"the platform PID range (1 to "
                              f"{MAX_PID}) — the PID the runner "
                              f"read from the slot's marker "
                              f"files; no descendant the survivor "
                              f"probe judged ever carried an "
                              f"identity outside that range"})
            value = record.get("arm_pid")
            if not isinstance(value, int) \
                    or isinstance(value, bool):
                found.append({
                    "record": record, "field": "arm_pid",
                    "slot": slot,
                    "detail": "arm_pid is missing or null "
                              "but an ok kill record must carry "
                              "the arm's own PID the runner "
                              "records for every spawned slot "
                              "(the first identity of the tree "
                              "the record's survivor judgement "
                              "names — the arm process itself)"})
            elif not 1 <= value <= MAX_PID:
                found.append({
                    "record": record, "field": "arm_pid",
                    "slot": slot,
                    "detail": f"arm_pid is {value!r} but an "
                              f"ok kill record must carry the "
                              f"arm's own identity as a positive "
                              f"integer PID within the platform "
                              f"PID range (1 to {MAX_PID}); no "
                              f"arm the runner spawned ever "
                              f"carried an identity outside that "
                              f"range"})
        # argv / arm_pid, on every record (round-25): the runner
        # writes both fields for every slot — argv (the executed
        # argument vector, a non-empty list of string tokens) is
        # recorded before the spawn attempt, and arm_pid (the arm
        # process's own PID) is recorded for every spawned slot.
        # Round 24 held arm_pid only on kill records, resting the
        # fast path's withdrawal on the perfect fixture, whose
        # shape is not the runner's (its fast records carried no
        # arm_pid and its argv was the empty list no run writes);
        # the hold now reaches every record the runner writes
        # the fields on. The one shape that carries no arm_pid
        # by construction is the spawn-failure infrastructure
        # failure (no arm ever spawned, so no PID exists); every
        # other record whose arm_pid is missing, null or outside
        # PID form, and every record whose argv is not the
        # executed vector's form, is named here. (Kill records
        # classified "ok" or "descendant-survived" are already
        # held for arm_pid above, in their own idiom, and are
        # not named twice.)
        argv = record.get("argv")
        if not (isinstance(argv, list) and argv
                and all(isinstance(token, str) for token in argv)):
            found.append({
                "record": record, "field": "argv", "slot": slot,
                "detail": f"argv is {argv!r} but every record the "
                          f"runner writes carries the executed "
                          f"argument vector as a non-empty list "
                          f"of string tokens — the vector the "
                          f"slot actually spawned (or attempted); "
                          f"a record carrying no vector, an empty "
                          f"vector, or tokens of another type "
                          f"states a command line no run executed"})
        # Arm identity against the record's SECOND encoding
        # (round-41 F3): the record states its arm twice — in
        # its arm field and in its executed vector, whose
        # first token is the executable the slot spawned.
        # Every consumer before this hold read only the filed
        # label: the medians, the kill state, the (3)
        # comparison's by-arm cells, the gate's label readers
        # and the competitor-evidence branches all key on
        # record["arm"], so two records whose VECTORS were
        # swapped pairwise — each record's label re-filed
        # under the other arm while its argv kept the arm
        # that actually ran — rendered slot accounting exact,
        # no inconsistencies, standing losses 0 and every
        # dimension PASS over two wrong binaries. The vector
        # is the encoding closest to the execution, so it is
        # held against the configuration here: the executed
        # token must not be ANOTHER product arm's configured
        # executable. The limb fires only on that positive
        # cross-arm disagreement — a token that is no arm's
        # configured executable (a testkit child program, a
        # renamed or stub binary) states nothing about which
        # arm ran and is not named, and the filed arm's own
        # executable always agrees. Under NO_VERIFIED_PLAN
        # there is no configuration to hold the token against
        # and the limb does not fire (the sentinel's absence
        # is named at the verdict layer, as for the argv
        # budget). The per-arm executables are read once per
        # call, from the judged configuration (the shipped
        # configuration for a bare record set, as the
        # fast-length budget reads it). Round-42 F1: the
        # comparison is held under the platform's path
        # semantics, not by exact string — the token and
        # each configured executable are canonicalized
        # (_canonical_executable_path: case folded,
        # separators unified, current-directory and root
        # prefixes dropped) and compared as the binaries
        # they denote (_same_executable: canonical agreement,
        # or one spelling an absolute form of the other), so
        # a pairwise swap whose kept tokens are respelled —
        # uppercase, forward slashes, a ./ prefix, an
        # absolute path — names the same disagreement the
        # exact-token swap names, while a token denoting no
        # arm's executable under that identity keeps the
        # round-41 non-naming and the filed arm's own
        # executable agrees in every spelling. Round-43 F1:
        # the canonicalization extends to the deterministic
        # Win32 forms of the same class — trailing dots and
        # spaces stripped per component, an omitted final
        # extension completed — and identification is
        # confined to equality after dropping only those
        # prefix forms: a proper suffix of the configured
        # sequence (a basename only, a partial tail) never
        # identifies the campaign's binary, and a genuinely
        # different directory never agrees. The 8.3
        # short-name class stands beside .. as a named
        # limit at its proven boundary (see
        # _canonical_executable_path). Round-44 F1: the
        # strip is confined to components carrying other
        # content — a component made solely of dots and
        # spaces canonicalizes as itself when it is
        # exactly "." or ".." and otherwise stands
        # verbatim in the sequence, so a padded
        # parent-relative spelling (".. ", ".. .") and
        # an all-dots component stand unresolved exactly
        # as the plain ".." does and never satisfy this
        # identity hold: the plain ".." spelling of a
        # platform denotation and its padded spellings
        # render the same non-naming here.
        if arm in PRODUCT_ARMS \
                and isinstance(argv, list) and argv \
                and all(isinstance(token, str) for token in argv) \
                and config is not NO_VERIFIED_PLAN:
            if "map" not in arm_executables:
                judged = config if isinstance(config, dict) \
                    else _shipped_config()
                arms_map = judged.get("arms") \
                    if isinstance(judged, dict) else None
                executables = {}
                for product_arm in PRODUCT_ARMS:
                    entry = arms_map.get(product_arm) \
                        if isinstance(arms_map, dict) else None
                    executable = entry.get("executable") \
                        if isinstance(entry, dict) else None
                    executables[product_arm] = executable \
                        if isinstance(executable, str) \
                        and executable else None
                arm_executables["map"] = executables
            executables = arm_executables["map"]
            token = argv[0]
            if not _same_executable(token, executables.get(arm)):
                for other_arm in PRODUCT_ARMS:
                    if other_arm != arm \
                            and executables.get(other_arm) \
                            is not None \
                            and _same_executable(
                                token, executables[other_arm]):
                        found.append({
                            "record": record, "field": "argv",
                            "slot": slot,
                            "detail": f"argv[0] is {token!r} "
                                      f"but that token is the "
                                      f"configured executable "
                                      f"of arm {other_arm!r}, "
                                      f"not of the arm this "
                                      f"record is filed under, "
                                      f"{arm!r} (whose configured "
                                      f"executable is "
                                      f"{executables.get(arm)!r}); "
                                      f"the record states its "
                                      f"arm twice — in its arm "
                                      f"field and in the "
                                      f"executable token of the "
                                      f"vector it executed — and "
                                      f"the two encodings "
                                      f"disagree: the vector "
                                      f"this record carries is "
                                      f"the other arm's command "
                                      f"line, so every verdict "
                                      f"keyed on the filed label "
                                      f"alone credits one arm "
                                      f"with another arm's "
                                      f"execution"})
                        break
        if not (path in ("kill", "scale")
                and outcome in ("ok", "descendant-survived")):
            pid = record.get("arm_pid")
            if pid is None and outcome != "infra-failure":
                found.append({
                    "record": record, "field": "arm_pid",
                    "slot": slot,
                    "detail": "arm_pid is missing or null but "
                              "the runner records the arm "
                              "process's own PID for every "
                              "spawned slot, on the fast and "
                              "control paths as on the kill path; "
                              "the one shape that carries no "
                              "arm_pid is the spawn-failure "
                              "infrastructure failure, whose arm "
                              "never spawned"})
            elif pid is not None and not _is_pid_form(pid):
                found.append({
                    "record": record, "field": "arm_pid",
                    "slot": slot,
                    "detail": f"arm_pid is {pid!r} but a record's "
                              f"arm_pid must be the arm process's "
                              f"own identity as a positive integer "
                              f"PID within the platform PID range "
                              f"(1 to {MAX_PID}) — the PID the "
                              f"runner records from the spawn "
                              f"itself, on every spawned slot; no "
                              f"arm the runner spawned ever "
                              f"carried an identity outside that "
                              f"form"})
        if outcome == "ok" \
                and record.get("kind") in ("measured", "control") \
                and record.get("elapsed_ms") is None:
            found.append({
                "record": record, "field": "elapsed_ms", "slot": slot,
                "detail": "elapsed_ms is None or missing but an ok "
                          "measured or control record must carry "
                          "the elapsed time its median is computed "
                          "from"})
        # Resource fields (round-27 F3, round-28 F3/F4;
        # round-29 F2/F3/F6): every record the runner writes
        # — measured, warmup and control, on every path — is
        # CONSTRUCTED with both resource keys present (the
        # runner's record construction carries cpu_ms and
        # peak_working_set_bytes from the start) and only ever
        # varies their values: a plausible counter, or nulls
        # with a resource_note naming why — absent-with-reason,
        # never a silent or implausible value. Its note
        # semantics, from the code: the note names a
        # collection failure in the infra states (arm not
        # spawned yet / never spawned / not reaped) and in the
        # platform counter failures it records by note on a
        # completed slot (a Windows counter call failing —
        # possibly for one field only, the other carrying its
        # numeric value beside the note — or a platform with
        # no per-child accounting); on a full collection the
        # note is None and both values are numeric. The runner
        # NEVER deletes a key, in any state. So the hold binds
        # (round-29 F3): a MISSING KEY is named in its own
        # right — with or without a retained note, on a
        # completed measurement and on an infra record alike;
        # the note excuses a null VALUE only, which is the
        # shape the runner's collection failures actually
        # take. Round-35 F1: the excusing note is itself held
        # to the runner's CLOSED collection-failure vocabulary
        # (RESOURCE_NOTE_VOCABULARY) — a null carrying any
        # other note, or none, is UNACCOUNTED and named here.
        # Round-36 F2: the excuse is paired PER FIELD — of the
        # platform counter failures, exactly one is single-
        # field in the runner's construction (a
        # GetProcessMemoryInfo failure leaves a numeric cpu
        # beside a null peak); every other failure note stands
        # over both counters null — so a null is excused only
        # by the note its own field's failure writes, here as
        # in _resource_field_accounted.
        # A present value must be a plausible counter:
        # cpu_ms finite and non-negative; the peak
        # integer-valued, finite and strictly positive
        # (round-28 F3 / round-29 F2: a peak of 0, or a
        # fractional peak, is a shape no platform counter
        # produces for a spawned process, held here as an
        # inconsistency like the negative and NaN values
        # round-27 named — not as an absent value, and it
        # counts toward no coverage). The predicates are
        # total (round-29 F6): a huge, non-finite or
        # wrong-typed value is named, never an exception.
        # The hold binds PRESENCE on every record (round-28
        # F4): it is keyed on the record, not on the fields'
        # presence — a record whose resource keys were
        # deleted entirely is named here in its own right, on
        # the warmup and control kinds as on the measured
        # kind, not rescued by the dimension state or
        # anything downstream.
        note = record.get("resource_note")
        # Round-35 F1: a null value is excused only by a note
        # from the runner's CLOSED collection-failure
        # vocabulary — any other note, however plausible,
        # names the record UNACCOUNTED here as in
        # _resource_field_accounted. Round-36 F2: the excuse
        # is PER FIELD — the note must be the one the runner
        # pairs with that field's failure (the
        # GetProcessMemoryInfo note accounts a peak null
        # beside a numeric cpu only; every other vocabulary
        # note accounts a null only when both counters stand
        # null), on warmup and control records exactly as on
        # measured ones, so the check below asks
        # _resource_field_accounted of each field in turn
        # rather than holding one per-record note verdict.
        for field in ("cpu_ms", "peak_working_set_bytes"):
            value = record.get(field)
            if field not in record:
                found.append({
                    "record": record, "field": field,
                    "slot": slot,
                    "detail": f"{field} is absent — its key "
                              f"is missing from the record — "
                              f"and a missing key is named "
                              f"with or without a retained "
                              f"resource_note, in every "
                              f"record state: the runner "
                              f"constructs every record with "
                              f"both resource keys present and "
                              f"never deletes one, in its "
                              f"infra states (whose collection "
                              f"failures it records as null "
                              f"values with a note) as on a "
                              f"completed measurement; a note "
                              f"excuses a null value only, "
                              f"never a deleted key"})
            elif value is not None \
                    and not _resource_field_plausible(field, value):
                found.append({
                    "record": record, "field": field,
                    "slot": slot,
                    "detail": f"{field} is {value!r} but a "
                              f"recorded resource counter "
                              f"must be "
                              f"{'an integer-valued, finite, strictly positive number — a peak working set of 0 bytes, or a fractional byte count, is a shape no platform counter produces for a spawned process (both of the runner sources return an integer)' if field == 'peak_working_set_bytes' else 'a finite, non-negative number'}"
                              f" — the value the platform's "
                              f"per-process accounting "
                              f"produces; a negative, zero-peak, "
                              f"fractional-peak, NaN, infinite, "
                              f"huge or untyped value "
                              f"is no measurement any "
                              f"instrument recorded"})
            elif value is None and not _resource_field_accounted(
                    record, field):
                found.append({
                    "record": record, "field": field,
                    "slot": slot,
                    "detail": f"{field} is absent "
                              f"({'its key is missing from the record' if field not in record else 'null'}) "
                              f"and resource_note carries "
                              f"no collection-failure reason "
                              f"the runner writes FOR THIS "
                              f"FIELD: a null counter is "
                              f"accounted only by the note "
                              f"the runner pairs with that "
                              f"field's failure, from its "
                              f"closed collection-failure "
                              f"vocabulary (the "
                              f"GetProcessMemoryInfo note "
                              f"accounts a peak null beside "
                              f"a numeric cpu only; every "
                              f"other vocabulary note — its "
                              f"infra-state notes, the "
                              f"GetProcessTimes and "
                              f"parameterized counter-failure "
                              f"notes and its "
                              f"no-accounting-platform note "
                              f"— accounts a null only when "
                              f"both counters stand null — "
                              f"round-35 F1, round-36 F2), "
                              f"and resource_note is "
                              f"{note!r}; the runner records "
                              f"each resource field as a "
                              f"counter value or names why "
                              f"IT is absent in that "
                              f"vocabulary, on every record "
                              f"kind — absent-with-reason, "
                              f"never silently missing and "
                              f"never excused by a note no "
                              f"runner writes or by a note "
                              f"paired with the other "
                              f"field's failure"})
        # Rep well-formedness (round-34 F2; round-35 F2
        # extends the hold to EVERY record kind): on the
        # malformed AND scale paths every verdict consumer
        # reads the record's shape through its slot identity —
        # (path, rep, kind) re-derived by plan.slot_variant —
        # and every hold conditioned on that derivation SKIPPED
        # a record whose rep the derivation cannot read, while
        # every identity Counter in the checker read the rep
        # by equality, under which a float twin of a genuine
        # rep (3.0 == 3, hash-equal, Counter-equal) IS the
        # genuine slot. A rep that is not a non-bool integer
        # in its kind's band for the path — a float, a string,
        # a bool, a negative or out-of-band integer — therefore
        # carried a record past the variant hold, the argv
        # hold and the pin while slot accounting, dimensions
        # (4)/(6)'s collections and dimension (3)'s planned
        # populations counted it as its twin's slot, and on
        # the scale path _scale_variant_of fell back to the
        # carried label as the authority a verdict reads. The
        # verdict layer now names the rep itself, here, in the
        # same standing-loss channel as a disagreeing label:
        # derivation-None is itself the finding, never a skip.
        # (The package checker's shape channel names a float
        # rep too, but that channel is not the verdict layer
        # and does not discharge this hold — round 33's integer
        # relabel was likewise named there and sustained here.)
        # Round-35 F2: round 34 scoped this hold to the two
        # variant paths on the premise that slot accounting
        # already names a fast/kill rep — false for the
        # aliasing genus at its centre: a float rep on a fast
        # record and a bool rep on a kill record are
        # Counter-equal to their genuine slots, so slot
        # accounting returned [] and NOTHING in the verdict
        # layer named them while their records fed every
        # median (round-35 P2, executed). The hold now binds
        # every record the plan bands — fast, kill, malformed,
        # scale and control records alike, band or no band
        # (the variant paths derive their band from
        # plan.slot_variant; the plain paths from the plan's
        # per-kind rep counts, both inside _rep_well_formed).
        # Exactly one rep item per record stands here: slot
        # accounting's missing/unexpected entries, where they
        # fire (a negative or out-of-band rep), name the slot
        # POPULATION's defect — a different fact from this
        # record's ill-formed identity, the two-channel naming
        # round 34 already carried for the variant paths —
        # and for the float/bool twins slot accounting is
        # silent, so this item is the only naming.
        if not _rep_well_formed(record) \
                and (path in ("malformed", "scale")
                     or record.get("kind") in
                     ("measured", "warmup", "control")):
            found.append({
                "record": record, "field": "rep",
                "slot": slot,
                "detail": f"rep is {record.get('rep')!r} but a "
                          f"{path} record's rep must be a "
                          f"non-boolean integer within its "
                          f"kind's band for the path — the slot "
                          f"identity (path, rep, kind) from "
                          f"which the plan encodes the record's "
                          f"shape; a rep of another type that "
                          f"merely compares equal to a genuine "
                          f"rep (a float twin, above all), a "
                          f"boolean, a negative rep or a rep "
                          f"outside the band names no slot the "
                          f"plan holds, and every verdict "
                          f"consumer that derives the record's "
                          f"shape from its slot reads nothing "
                          f"for it"})
        # Variant identity (round-32 F2; round-33 F1 extends
        # the hold to the scale path, reversing the round-32
        # narrowing): on the malformed AND scale paths a
        # record's shape is encoded by its SLOT
        # identity in the plan — (path, rep, kind) re-derives
        # it (plan.slot_variant), the rep numbering being
        # variant-major — so a carried variant label that
        # DISAGREES with the slot's encoded shape is a claim
        # the slot itself refutes, named here in its own right.
        # Before this hold, nothing read the carried label
        # against the rep that redundantly encodes the shape,
        # and dimension (6)'s pin read the label itself, so
        # relabelling a violating record to the one unpinned
        # shape evaded the pin entirely; on the scale path the
        # same relabel moved dimension (4)'s count hold and
        # dimension (3)'s cell key, which read the carried
        # label as their authority. A missing, empty or
        # untyped carried variant was not named here before
        # round 41 (round-41 F5 restates that sentence): the
        # runner stamps the slot's encoded shape on EVERY
        # malformed and scale record it writes
        # (runner._execute writes "variant": variant for each
        # slot it builds, on both paths), so no record state
        # on those paths legitimately lacks the label — an
        # erased label is a written field erased, and the
        # absence is named below beside the disagreement, so
        # no verdict passes over an unnamed absence (before
        # this limb, erasing every malformed record's label
        # left (3) and (6) PASS with no variant item at all:
        # the case-relabel channel named a WRONG label, and
        # an erased one was named by nothing).
        # Round-34 F2: where the rep itself is not well-formed
        # the derivation derives nothing and this hold does
        # not fire — the rep hold above names that record in
        # its own right, so the skip is a naming, not a silence.
        if path in ("malformed", "scale"):
            carried_variant = record.get("variant")
            encoded_variant = slot_variant(
                path, record.get("rep"), record.get("kind"))
            if isinstance(carried_variant, str) \
                    and carried_variant \
                    and encoded_variant is not None \
                    and _rep_well_formed(record) \
                    and carried_variant != encoded_variant:
                found.append({
                    "record": record, "field": "variant",
                    "slot": slot,
                    "detail": f"variant is {carried_variant!r} "
                              f"but the shape this record's "
                              f"slot encodes in the plan — "
                              f"(path, rep, kind) re-deriving "
                              f"variant-major from the plan — "
                              f"is {encoded_variant!r}; a "
                              f"record's carried variant is a "
                              f"claim about its slot, and a "
                              f"label that disagrees with the "
                              f"slot's encoded shape names a "
                              f"shape the slot never presented"})
            if encoded_variant is not None \
                    and _rep_well_formed(record) \
                    and not (isinstance(carried_variant, str)
                             and carried_variant):
                # Round-41 F5: variant ABSENCE is named. The
                # runner stamps the slot's encoded shape as
                # the record's variant on every malformed and
                # scale slot it writes, so a record whose
                # variant is missing, null, empty or untyped
                # carries an erased written field — named
                # here in its own right, beside the
                # disagreement above, so dimensions (3) and
                # (6) cannot pass over an unnamed absence.
                if "variant" not in record:
                    carried_text = "missing — the record " \
                        "carries no variant key"
                elif carried_variant is None:
                    carried_text = "None"
                elif isinstance(carried_variant, str):
                    carried_text = "the empty string"
                else:
                    carried_text = f"{carried_variant!r}, " \
                        f"a value of another type"
                found.append({
                    "record": record, "field": "variant",
                    "slot": slot,
                    "detail": f"variant is {carried_text} but "
                              f"the shape this record's slot "
                              f"encodes in the plan — (path, "
                              f"rep, kind) re-deriving "
                              f"variant-major from the plan — "
                              f"is {encoded_variant!r}, and the "
                              f"runner stamps that shape as "
                              f"the record's variant on every "
                              f"malformed and scale slot it "
                              f"writes; a record carrying no "
                              f"variant label on a path whose "
                              f"slots encode one carries an "
                              f"erased written field, and its "
                              f"absence is named here in its "
                              f"own right"})
        # Malformed argv content (round-33 F2; round-34 F3
        # adds the EXCLUSIVITY conjunct; round-35 F3 adds the
        # EXACTNESS limb to the same ONE-vector item): a malformed
        # record carries TWO encodings of the shape it
        # presented — the rep, which the variant hold above
        # binds to the slot, and the argv, the executed vector
        # closest to the execution itself. The standing argv
        # hold binds the vector's FORM only; this hold binds
        # its CONTENT against the shape its slot encodes: the
        # shape-distinguishing construction runner.malformed_argv
        # writes for that shape (see malformed_argv_presents).
        # A record whose argv presents another shape — the
        # genuine over-long-argument vector under a
        # nonexistent-program slot, above all — is named here
        # in its own right, whatever its exit code answers.
        # The hold fires only on a form-valid vector (a form
        # failure is the form hold's naming) and only where
        # the slot encodes a known shape. Round-34 F3: the
        # hold is TWO conjuncts, as the predicate's own claim
        # ("each shape is fully distinguishable inside the
        # argv it is presented through") and the agreement
        # test's pin on genuine argv (own shape True, every
        # other shape False) both state: the vector must
        # present its slot's encoded shape AND no other shape.
        # Round 33 held only the first conjunct, so a hybrid
        # vector presenting ALL SIX shapes at once — one
        # invocation whose timeout is both malformed and
        # out-of-range, whose child is both the nonexistent
        # program and empty, carrying both argument marks —
        # was named by nothing: a presentation no run writes
        # and no single shape describes. A vector presenting
        # another shape's distinguishing marks beside its own
        # is named here for every shape it is not.
        if path == "malformed":
            encoded_shape = slot_variant(
                path, record.get("rep"), record.get("kind"))
            argv_value = record.get("argv")
            if encoded_shape in (
                    "nonexistent-program", "malformed-timeout",
                    "out-of-range-timeout", "empty-command",
                    "over-long-argument", "non-unicode-argument") \
                    and isinstance(argv_value, list) and argv_value \
                    and all(isinstance(token, str)
                            for token in argv_value) \
                    and not malformed_argv_presents(
                        argv_value, encoded_shape):
                found.append({
                    "record": record, "field": "argv",
                    "slot": slot,
                    "detail": f"argv is {argv_value!r} but it "
                              f"does not present the "
                              f"{encoded_shape} shape this "
                              f"record's slot encodes in the "
                              f"plan: a malformed record's argv "
                              f"is the executed argument vector "
                              f"the runner built for that slot "
                              f"through the arm's own template "
                              f"(runner.malformed_argv), and it "
                              f"carries the shape's "
                              f"shape-distinguishing content — "
                              f"the program token, timeout text "
                              f"or argument marks that template "
                              f"presents the shape through; an "
                              f"argv presenting another shape "
                              f"(or none) is a presentation the "
                              f"runner would never write for "
                              f"this slot"})
            if encoded_shape in (
                    "nonexistent-program", "malformed-timeout",
                    "out-of-range-timeout", "empty-command",
                    "over-long-argument", "non-unicode-argument") \
                    and isinstance(argv_value, list) and argv_value \
                    and all(isinstance(token, str)
                            for token in argv_value):
                other_shapes = [
                    shape for shape in MALFORMED_SHAPES
                    if shape != encoded_shape
                    and malformed_argv_presents(argv_value, shape)]
                # Round-35 F3: the ONE-vector conjunct binds
                # EXACTNESS beside exclusivity. Round 34 held
                # the vector's marks (its own shape present,
                # no other shape's marks), but a vector
                # presenting its own shape and no other, yet
                # carrying content belonging to NO shape — an
                # unmarked junk tail appended to a genuine
                # vector — was named by nothing, while the
                # item's own text claimed the vector "is the
                # ONE vector runner.malformed_argv built for
                # that one slot". The exactness derivation
                # (_malformed_argv_exactness_problems) holds
                # the vector to its slot's construction: its
                # length against the arm's own fast vector,
                # and its closing token against the shape's
                # construction mark. Both failures — another
                # shape's marks, or content no construction
                # wrote — are named by THIS ONE item, so a
                # record is named once for not being the one
                # vector, however many ways it fails.
                # Round-36 F3: the length the derivation reads
                # is the PLAN's fast length for the arm
                # (_plan_fast_argv_length: the run's
                # configuration where the caller supplies it,
                # the shipped plan for a bare record set), not
                # a statistic of this record set — a uniformly
                # extended fast population cannot move its own
                # budget.
                if arm not in fast_argv_lengths:
                    fast_argv_lengths[arm] = _plan_fast_argv_length(
                        arm, config)
                exactness = _malformed_argv_exactness_problems(
                    argv_value, encoded_shape,
                    fast_argv_lengths[arm])
                if fast_argv_lengths[arm] is None:
                    # Round-37 F2: an arm with no derivable
                    # argv budget is NAMED, never silent. The
                    # length limb is the only limb that names
                    # added content on the timeout shapes, so
                    # a configuration stating no construction
                    # for this arm left those records' argv
                    # exactness wholly unheld — and, at the
                    # package layer under a minimal anchored
                    # configuration, certified a junk-tailed
                    # package COMPLETE. The naming is its own
                    # item (the ONE-vector item above keeps its
                    # own claim), stating the plan fact
                    # _plan_fast_argv_budget_gap derives — the
                    # ONE derivation's own failure reason
                    # (round-38 F3). Under NO_VERIFIED_PLAN the
                    # helper returns None: the sentinel's
                    # absence is named per record nowhere, but
                    # it is named at the verdict layer by the
                    # dimension-(6) sentinel cap (round-38 F2)
                    # and at the package layer in the degraded
                    # list (round-37 F3).
                    budget_gap = _plan_fast_argv_budget_gap(
                        arm, config)
                    if budget_gap is not None:
                        found.append({
                            "record": record, "field": "argv",
                            "slot": slot,
                            "detail": f"argv is {argv_value!r} "
                                      f"but the argv length "
                                      f"budget for arm {arm!r} "
                                      f"is underived: "
                                      f"{budget_gap} — the "
                                      f"length limb of this "
                                      f"record's argv exactness "
                                      f"hold does not fire for "
                                      f"this arm (on the "
                                      f"timeout shapes it is "
                                      f"the only limb that "
                                      f"names added content), "
                                      f"so this record's argv "
                                      f"is held to no length "
                                      f"budget under the "
                                      f"configuration it is "
                                      f"judged under; the "
                                      f"underived budget is "
                                      f"named here, never "
                                      f"silent"})
                if other_shapes or exactness:
                    failings = []
                    if other_shapes:
                        failings.append(
                            f"it also presents the "
                            f"{', '.join(other_shapes)} shape(s) "
                            f"beside the {encoded_shape} shape "
                            f"this record's slot encodes in the "
                            f"plan")
                    if exactness:
                        failings.append(
                            "it is not the exact vector its "
                            "slot's construction builds: "
                            + "; ".join(exactness))
                    found.append({
                        "record": record, "field": "argv",
                        "slot": slot,
                        "detail": f"argv is {argv_value!r} but "
                                  + "; and ".join(failings)
                                  + f": a malformed record's "
                                  f"argv is the ONE vector "
                                  f"runner.malformed_argv built "
                                  f"for that one slot, and each "
                                  f"shape is fully "
                                  f"distinguishable inside the "
                                  f"argv it is presented through "
                                  f"— a vector presenting another "
                                  f"shape's distinguishing marks, "
                                  f"or carrying content no "
                                  f"construction wrote for the "
                                  f"slot (an unmarked addition "
                                  f"above all), is a presentation "
                                  f"no run writes for this slot, "
                                  f"and no single shape "
                                  f"describes"})
        # Malformed exit code (round-27 F4): on the malformed
        # path no expected code is pinned as a whole — the arm's
        # recorded exit code IS dimension (6)'s observation, the
        # entire content of the record (round-31 F2's
        # designated-only pinned shapes are held separately,
        # below). Every malformed record the
        # runner classifies from a returned arm carries that
        # code as an integer; a missing, null or untyped code
        # leaves the record claiming a presentation whose answer
        # it does not carry. (The infrastructure failure whose
        # arm never returned — a shape the harness could not
        # present — carries no observation by shape, as on the
        # other paths; a code it does carry is held to the
        # integer form all the same.)
        if path == "malformed":
            if actual is not None and (
                    not isinstance(actual, int)
                    or isinstance(actual, bool)):
                found.append({
                    "record": record, "field": "exit_code",
                    "slot": slot,
                    "detail": f"exit_code is {actual!r} but a "
                              f"malformed record's exit code is "
                              f"dimension (6)'s observation — "
                              f"the arm's recorded answer to "
                              f"the shape — and the runner "
                              f"records it as an integer or "
                              f"not at all; an answer of "
                              f"another type is no observation "
                              f"any arm gave"})
            elif actual is None and outcome != "infra-failure":
                found.append({
                    "record": record, "field": "exit_code",
                    "slot": slot,
                    "detail": "exit_code is None but the "
                              "outcome is "
                              f"{outcome!r}; a malformed record "
                              "classified from a returned arm "
                              "always carries the exit code "
                              "that is dimension (6)'s "
                              "observation for its shape — "
                              "expected_exit pins no code for "
                              "this path, so the recorded code "
                              "is the evidence, and a record "
                              "without it evidences no answer"})
            # Designated pinned expectation (round-31 F2;
            # round-32 F2): a WARMUP OR MEASURED malformed
            # record of the designated arm whose shape — the
            # shape its SLOT encodes in the plan, never its
            # carried label — its contract pins is held to the
            # pinned code by THE one shared formulation
            # (plan.malformed_pin_violation, the expectation
            # the runner faults both kinds on through
            # arms.expected_exit), scoped exactly to where an
            # expectation is pinned — the designated arm, a
            # pinned shape, a warmup or measured record. A
            # designated record answering a pinned shape with
            # an integer code other than the pinned refusal
            # under a label other than "exit-mismatch"
            # contradicts its label (an honestly classified
            # record with that code would be "exit-mismatch" —
            # the runner writes exactly that, for warmups as
            # for measured slots); a designated
            # "exit-mismatch" record whose code IS the pinned
            # refusal claims a mismatch its own code refutes.
            # No competitor record and no unpinned-shape record
            # acquires an expectation here: their codes remain
            # observations under the F4 hold above.
            if record.get("kind") in ("warmup", "measured"):
                variant = _record_variant(record)
                pinned = malformed_expected_exit(arm, variant)
                violation = malformed_pin_violation(
                    arm, variant, actual)
                if violation is not None \
                        and outcome != "exit-mismatch":
                    found.append({
                        "record": record, "field": "exit_code",
                        "slot": slot,
                        "detail": f"exit_code is {actual!r} "
                                  f"but the designated arm's "
                                  f"pinned expected exit for "
                                  f"the {variant} shape is "
                                  f"{violation} (its documented "
                                  f"refusal) and the outcome "
                                  f"is {outcome!r}, not "
                                  f"'exit-mismatch'"})
                elif pinned is not None \
                        and isinstance(actual, int) \
                        and not isinstance(actual, bool) \
                        and actual == pinned \
                        and outcome == "exit-mismatch":
                    found.append({
                        "record": record, "field": "exit_code",
                        "slot": slot,
                        "detail": f"exit_code is {actual!r}, "
                                  f"which is the designated "
                                  f"arm's pinned expected exit "
                                  f"for the {variant} shape, "
                                  f"but the outcome is "
                                  f"'exit-mismatch'; a mismatch "
                                  f"classification must be "
                                  f"earned by an observed exit "
                                  f"code that differs from the "
                                  f"pinned expectation"})
    return found


def record_consistency_losses(records, expected_output=None,
                              config=None):
    """Named standing losses for record/label disagreements (see
    _record_inconsistencies): each names the inconsistent field
    and the slot carrying it. These losses are part of
    standing_losses(), so campaign_outcome and render() both
    treat a campaign carrying such a record as INCOMPLETE."""
    return [
        f"record inconsistency in {item['slot']}: {item['detail']}"
        for item in _record_inconsistencies(records, expected_output,
                                            config)
    ]


def _kill_records(records, arm, context):
    """One arm's measured kill-path records in one context.
    Round-40 F3: only records matching a plan slot are kill
    evidence — an unplanned kill record (a kill-999 rep, say)
    flips no kill state and excludes no arm from the kill
    latency comparison; it stands as named incompleteness in
    the channels that name records, not as a measurement."""
    return [r for r in _planned_records(records)
            if r.get("kind") == "measured"
            and r.get("arm") == arm
            and r.get("path") == "kill"
            and r.get("context") == context]


def _evidenced_survived(record):
    """True when a kill record's survival claim is earned by its
    own fields: the "descendant-survived" classification AND the
    survivor observation of True the runner derived it from AND
    a tree identity in PID form — a grandchild_pid that is a
    positive integer within MAX_PID (the descendant identity
    the shipped kill workload records at the slot's marker and
    the survivor probe judges), with a present child_pid held
    to the same form (a missing or null child_pid is the
    shipped shape and contradicts nothing). The runner invokes
    the probe with the recorded tree PIDs: no PID-form
    identity, no probe answer, no survival. The classification
    also rests on the exit code the runner records from its
    wait before it probes and classifies: a survived record
    whose exit_code was never observed (None) rests on an
    observation no run makes, and evidences no survival here
    either. It likewise rests on the slot's tree_marker: the
    probe judges the tree the slot's marker file records, so
    a survived record whose tree_marker is missing, null or
    unformed names no marker the judgement could rest on and
    evidences no survival here either. A record carrying
    the label without the observation (or with survivor False),
    or resting on no PID-form identity, is a record
    inconsistency, named by _record_inconsistencies; it
    evidences no survival here — no competitor loss, no
    kill-path correctness loss for its arm, and no exclusion
    from the kill-latency comparison."""
    return record.get("outcome") == "descendant-survived" \
        and record.get("survivor") is True \
        and record.get("exit_code") is not None \
        and isinstance(record.get("tree_marker"), str) \
        and bool(record.get("tree_marker")) \
        and _is_pid_form(record.get("grandchild_pid")) \
        and _is_pid_form(record.get("arm_pid")) \
        and (record.get("child_pid") is None
             or _is_pid_form(record.get("child_pid")))


def _survivor_true_under_a_returned_label(record):
    """True when a kill record's survivor field states a
    survival under a returned-arm label other than
    "descendant-survived": an "exit-mismatch" or
    "output-mismatch" kill record with survivor True and a
    tree identity in PID form (the same identity hold
    _evidenced_survived applies). The runner's precedence
    classifies such an arm "descendant-survived" before its
    exit code is compared, so the label cannot be produced
    from that observation — _record_inconsistencies names it —
    but the survival the record's own field states is not read
    away here: kill-path correctness does not read an arm
    clean over an uncontradicted survivor True. The timed-out
    infrastructure failure is the one shape that records
    survivor True under another label; it is not a returned
    arm and is not held here."""
    return record.get("path") == "kill" \
        and record.get("outcome") in ("exit-mismatch",
                                      "output-mismatch") \
        and record.get("survivor") is True \
        and isinstance(record.get("tree_marker"), str) \
        and bool(record.get("tree_marker")) \
        and _is_pid_form(record.get("grandchild_pid")) \
        and _is_pid_form(record.get("arm_pid")) \
        and (record.get("child_pid") is None
             or _is_pid_form(record.get("child_pid")))


def _descendant_loser(records, arm, context):
    """True when an arm's measured kill records in a context are
    non-empty and every one evidences a survived descendant
    (see _evidenced_survived): the arm produced no "ok" kill
    record because it does not kill descendants there. Such an
    arm has lost kill-path correctness in that context and has
    no kill latency to compare."""
    kills = _kill_records(records, arm, context)
    return bool(kills) and all(
        _evidenced_survived(r) for r in kills)


def _kill_state(records, arm, context):
    """One arm's kill-path correctness state in one context, as
    (state, kills, survived). States: "absent" (no kill records
    ran), "losing" (at least one slot left a descendant alive,
    evidenced by its survivor field — under its
    descendant-survived label, or under a returned-arm label
    carrying an uncontradicted survivor True, see
    _survivor_true_under_a_returned_label), "clean" (at least
    one "ok" slot and no survivor), and "unevaluated" (records
    ran but none was classified "ok" or evidences a survival —
    only infra-failure / exit-mismatch records, or survival
    labels their own survivor fields refute, from which
    correctness cannot be judged)."""
    kills = _kill_records(records, arm, context)
    survived = [r for r in kills if _evidenced_survived(r)
                or _survivor_true_under_a_returned_label(r)]
    if not kills:
        return "absent", kills, survived
    if survived:
        return "losing", kills, survived
    if any(r.get("outcome") == "ok" for r in kills):
        return "clean", kills, survived
    return "unevaluated", kills, survived


def _loser_phrases(losers):
    return "; ".join(
        f"{arm} losing kill-path correctness "
        f"({survived} of {total} measured kill slot(s) left a "
        f"descendant alive)"
        for arm, survived, total in losers)


def freeze_gate(records, expected_output=None, config=None):
    """Per-dimension, per-context freeze-gate outcomes. Each entry:
    {dimension, context, outcome, detail} with outcome PASS, FAIL or
    UNEVALUATED. The gate is met only when every entry passes."""
    entries = []
    # Round-41 F2: EVERY reader in this gate judges the planned
    # population — the medians and the kill state already did
    # (through _median / _kill_records); round 41 binds the
    # label readers (guard_all / guard_fast below) and the
    # inconsistency readers (inc / inc_arms here) to the same
    # population. _record_inconsistencies itself still computes
    # over every record — it is the naming hold, and an
    # unplanned record's contradictions are named as standing
    # losses through record_consistency_losses — but the gate
    # reads only PLANNED records' items: a fabricated record
    # matching no plan slot, or a copy beyond its slot's
    # planned count, moves no gate dimension and mints no
    # freeze-gate standing loss. Before this binding, one
    # fabricated guard rep-999 record flipped exit-code
    # correctness to FAIL (an exit-mismatch label), output
    # correctness to FAIL (an output-mismatch label whose own
    # stream verdicts contradict it) and softened exit-code
    # correctness to UNEVALUATED (an infra-failure label) —
    # the gate judged records that never ran.
    planned_records = _planned_records(records)
    planned_ids = {id(record) for record in planned_records}
    for context in CONTEXTS:
        inconsistencies = _record_inconsistencies(
            records, expected_output, config)

        def inc(arm, path, fields):
            """Measured record inconsistencies of one arm, path
            and field set in this context, over the gate's
            planned population (round-41 F2)."""
            return [item for item in inconsistencies
                    if id(item["record"]) in planned_ids
                    and item["record"].get("arm") == arm
                    and item["record"].get("path") == path
                    and item["record"].get("context") == context
                    and item["record"].get("kind") == "measured"
                    and item["field"] in fields]

        def inc_arms(path, fields):
            """Non-guard product arms carrying measured record
            inconsistencies of one path and field set in this
            context, over the gate's planned population
            (round-41 F2)."""
            return sorted({
                item["record"].get("arm") for item in inconsistencies
                if id(item["record"]) in planned_ids
                and item["record"].get("arm") != "guard"
                and item["record"].get("arm") in PRODUCT_ARMS
                and item["record"].get("path") == path
                and item["record"].get("context") == context
                and item["record"].get("kind") == "measured"
                and item["field"] in fields})

        guard = _median(records, "guard", "fast", context)
        rivals = {arm: _median(records, arm, "fast", context)
                  for arm in PRODUCT_ARMS if arm != "guard"}
        guard_fast_timing = inc("guard", "fast", {"elapsed_ms"})
        rival_fast_timing = inc_arms("fast", {"elapsed_ms"})
        if guard_fast_timing:
            entries.append({
                "dimension": "fast-path latency", "context": context,
                "outcome": "FAIL",
                "detail": f"{len(guard_fast_timing)} guard ok "
                          "measured fast record(s) in this context "
                          "carry no elapsed_ms (named as "
                          "record-inconsistency standing losses); "
                          "the fast-path median cannot be certified "
                          "from them"})
        elif rival_fast_timing:
            entries.append({
                "dimension": "fast-path latency", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "ok measured fast record(s) of "
                          f"{', '.join(rival_fast_timing)} in this "
                          "context carry no elapsed_ms (named as "
                          "record-inconsistency standing losses); "
                          "the fast-path comparison cannot be "
                          "certified from them"})
        elif guard is None or any(v is None for v in rivals.values()):
            entries.append({
                "dimension": "fast-path latency", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "a median needed for the comparison is "
                          "missing in this context"})
        else:
            slower = [f"{arm} {med:.3f} ms" for arm, med in
                      sorted(rivals.items()) if guard > med]
            if slower:
                entries.append({
                    "dimension": "fast-path latency", "context": context,
                    "outcome": "FAIL",
                    "detail": f"guard median {guard:.3f} ms is "
                              f"slower than " + ", ".join(slower)})
            else:
                entries.append({
                    "dimension": "fast-path latency", "context": context,
                    "outcome": "PASS",
                    "detail": f"guard median {guard:.3f} ms wins or "
                              "ties every competitor median"})
        # Kill-path latency: a descendant-loser rival is excluded —
        # it has no kill latency to lose to. Every other rival with
        # a missing median leaves the dimension UNEVALUATED.
        guard = _median(records, "guard", "kill", context)
        rivals = {}
        excluded = []
        for arm in PRODUCT_ARMS:
            if arm == "guard":
                continue
            if _descendant_loser(records, arm, context):
                excluded.append(arm)
            else:
                rivals[arm] = _median(records, arm, "kill", context)
        excluded_note = (
            f"; excluded from this comparison: "
            f"{', '.join(sorted(excluded))} (every kill slot "
            f"classified descendant-survived — losing kill-path "
            f"correctness, no kill latency to compare)"
            if excluded else "")
        guard_kill_timing = inc("guard", "kill", {"elapsed_ms"})
        rival_kill_timing = inc_arms("kill", {"elapsed_ms"})
        if guard_kill_timing:
            entries.append({
                "dimension": "kill-path latency", "context": context,
                "outcome": "FAIL",
                "detail": f"{len(guard_kill_timing)} guard ok "
                          "measured kill record(s) in this context "
                          "carry no elapsed_ms (named as "
                          "record-inconsistency standing losses); "
                          "the kill-path median cannot be certified "
                          "from them" + excluded_note})
        elif rival_kill_timing:
            entries.append({
                "dimension": "kill-path latency", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "ok measured kill record(s) of "
                          f"{', '.join(rival_kill_timing)} in this "
                          "context carry no elapsed_ms (named as "
                          "record-inconsistency standing losses); "
                          "the kill-path comparison cannot be "
                          "certified from them" + excluded_note})
        elif guard is None or any(v is None for v in rivals.values()):
            entries.append({
                "dimension": "kill-path latency", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "a median needed for the comparison is "
                          "missing in this context" + excluded_note})
        else:
            slower = [f"{arm} {med:.3f} ms" for arm, med in
                      sorted(rivals.items()) if guard > med]
            if slower:
                entries.append({
                    "dimension": "kill-path latency", "context": context,
                    "outcome": "FAIL",
                    "detail": f"guard median {guard:.3f} ms is "
                              f"slower than " + ", ".join(slower)
                              + excluded_note})
            else:
                entries.append({
                    "dimension": "kill-path latency", "context": context,
                    "outcome": "PASS",
                    "detail": f"guard median {guard:.3f} ms wins or "
                              "ties every competitor median"
                              + excluded_note})
        # Kill-path correctness: comparative, for every product arm.
        guard_state, _guard_kills, guard_survived = _kill_state(
            records, "guard", context)
        if guard_state == "absent":
            entries.append({
                "dimension": "kill-path correctness", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "no guard kill slots ran in this context"})
        elif guard_state == "losing":
            entries.append({
                "dimension": "kill-path correctness", "context": context,
                "outcome": "FAIL",
                "detail": f"{len(guard_survived)} guard kill slot(s) "
                          "left a descendant alive"})
        elif guard_state == "unevaluated":
            entries.append({
                "dimension": "kill-path correctness", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "guard kill slots ran in this context but "
                          "none was classified ok or "
                          "descendant-survived; kill-path correctness "
                          "cannot be judged from them"})
        elif inc("guard", "kill",
                 {"survivor", "child_pid", "grandchild_pid"}):
            bad = inc("guard", "kill",
                      {"survivor", "child_pid", "grandchild_pid"})
            fields = ", ".join(sorted({item["field"] for item in bad}))
            entries.append({
                "dimension": "kill-path correctness", "context": context,
                "outcome": "FAIL",
                "detail": f"{len(bad)} guard ok measured kill "
                          "record inconsistency field(s) in this "
                          f"context ({fields}; named as "
                          "record-inconsistency standing losses); "
                          "kill-path correctness cannot be "
                          "certified from records whose own fields "
                          "contradict their ok classification"})
        elif inc_arms("kill",
                      {"survivor", "child_pid", "grandchild_pid"}):
            arms = inc_arms("kill",
                            {"survivor", "child_pid", "grandchild_pid"})
            entries.append({
                "dimension": "kill-path correctness", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "kill-path correctness cannot be fully "
                          f"evaluated in this context: "
                          f"{', '.join(arms)} carry ok measured "
                          "kill records whose own fields contradict "
                          "their ok classification (named as "
                          "record-inconsistency standing losses)"})
        else:
            losers = []
            unevaluated = []
            for arm in PRODUCT_ARMS:
                if arm == "guard":
                    continue
                state, kills, survived = _kill_state(
                    records, arm, context)
                if state == "losing":
                    losers.append((arm, len(survived), len(kills)))
                elif state in ("absent", "unevaluated"):
                    unevaluated.append(arm)
            if unevaluated:
                detail = (
                    "kill-path correctness cannot be fully evaluated "
                    f"in this context: {', '.join(sorted(unevaluated))} "
                    "unevaluated (no kill records, or no ok / "
                    "descendant-survived kill record)")
                if losers:
                    detail += "; " + _loser_phrases(losers)
                entries.append({
                    "dimension": "kill-path correctness",
                    "context": context, "outcome": "UNEVALUATED",
                    "detail": detail})
            else:
                detail = ("no guard kill slot left a descendant alive; "
                          "the guard wins or ties every competitor on "
                          "kill-path correctness")
                if losers:
                    detail += "; " + _loser_phrases(losers)
                entries.append({
                    "dimension": "kill-path correctness",
                    "context": context, "outcome": "PASS",
                    "detail": detail})
        # Round-41 F2: guard_all / guard_fast — the gate's
        # label readers — are drawn from the planned
        # population computed at this function's head, like
        # its inc / inc_arms readers.
        guard_all = [r for r in planned_records
                     if r.get("kind") == "measured"
                     and r.get("arm") == "guard"
                     and r.get("context") == context]
        mismatched = [r for r in guard_all
                      if r.get("outcome") == "exit-mismatch"]
        infra = [r for r in guard_all
                 if r.get("outcome") == "infra-failure"]
        if not guard_all:
            entries.append({
                "dimension": "exit-code correctness", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "no guard slots ran in this context"})
        elif infra:
            entries.append({
                "dimension": "exit-code correctness", "context": context,
                "outcome": "UNEVALUATED",
                "detail": f"{len(infra)} guard slot(s) in this context "
                          "were infrastructure failures; their exit "
                          "codes were not observed, so exit-code "
                          "correctness cannot be judged from them"})
        elif mismatched:
            entries.append({
                "dimension": "exit-code correctness", "context": context,
                "outcome": "FAIL",
                "detail": f"{len(mismatched)} guard slot(s) produced "
                          "an unexpected exit code"})
        elif inc("guard", "fast", {"exit_code"}) \
                or inc("guard", "kill", {"exit_code"}):
            bad = inc("guard", "fast", {"exit_code"}) \
                + inc("guard", "kill", {"exit_code"})
            entries.append({
                "dimension": "exit-code correctness", "context": context,
                "outcome": "FAIL",
                "detail": f"{len(bad)} guard measured record(s) in "
                          "this context carry an exit_code that "
                          "contradicts their classification (named "
                          "as record-inconsistency standing losses); "
                          "exit-code correctness cannot be certified "
                          "from the classification label alone"})
        else:
            entries.append({
                "dimension": "exit-code correctness", "context": context,
                "outcome": "PASS",
                "detail": "every guard slot produced its expected "
                          "exit code"})
        # Output correctness: the campaign's fast-path slots carry
        # an output observation per stream (see runner.py) — the
        # emitted bytes' digest and length and, against the
        # configuration's expected bytes, a match verdict. The
        # guard is judged on its measured fast records here: a
        # slot classified "output-mismatch" fails the dimension
        # outright; a slot carrying no match verdict on either
        # stream leaves the dimension UNEVALUATED — output the
        # campaign did not observe is not output it may certify.
        guard_fast = [r for r in planned_records
                      if r.get("kind") == "measured"
                      and r.get("arm") == "guard"
                      and r.get("path") == "fast"
                      and r.get("context") == context]
        out_mismatched = [r for r in guard_fast
                          if r.get("outcome") == "output-mismatch"]
        unobserved = [r for r in guard_fast
                      if r.get("outcome") != "output-mismatch"
                      and (r.get("output_stdout_match") is None
                           or r.get("output_stderr_match") is None)]
        if not guard_fast:
            entries.append({
                "dimension": "output correctness", "context": context,
                "outcome": "UNEVALUATED",
                "detail": "no guard fast slots ran in this context"})
        elif out_mismatched:
            entries.append({
                "dimension": "output correctness", "context": context,
                "outcome": "FAIL",
                "detail": f"{len(out_mismatched)} guard fast slot(s) "
                          "produced stdout or stderr that did not "
                          "match the expected bytes"})
        elif inc("guard", "fast",
                 {"output_stdout_match", "output_stderr_match",
                  "output_stdout_sha256", "output_stderr_sha256",
                  "output_stdout_len", "output_stderr_len"}):
            bad = inc("guard", "fast",
                      {"output_stdout_match", "output_stderr_match",
                       "output_stdout_sha256", "output_stderr_sha256",
                       "output_stdout_len", "output_stderr_len"})
            fields = ", ".join(sorted({item["field"] for item in bad}))
            entries.append({
                "dimension": "output correctness", "context": context,
                "outcome": "FAIL",
                "detail": f"{len(bad)} guard measured fast record "
                          "inconsistency field(s) in this context "
                          f"({fields}; named as record-inconsistency "
                          "standing losses); output correctness "
                          "cannot be certified from records whose "
                          "own output fields contradict their "
                          "classification or rest a match verdict "
                          "on a digest the record does not carry"})
        elif unobserved:
            entries.append({
                "dimension": "output correctness", "context": context,
                "outcome": "UNEVALUATED",
                "detail": f"{len(unobserved)} guard fast slot(s) in "
                          "this context carry no output observation "
                          "for one or both streams; their output "
                          "was not observed, so output correctness "
                          "cannot be judged from them"})
        else:
            entries.append({
                "dimension": "output correctness", "context": context,
                "outcome": "PASS",
                "detail": "every guard fast slot's stdout and stderr "
                          "matched the expected bytes"})
    return entries


def _decision_reason(dimension, decisions=None):
    """The documented scope-decision reason for one profile
    dimension, from the decisions mapping the run's configuration
    carries (see decisions_from_config), or None when the
    dimension is not decided away. A decision decides a dimension
    away only when its reason is a non-empty string:
    SPEC_Benchmark section 1 permits removing a dimension only by
    an explicit, documented scope decision, so an entry carrying
    no reason — an empty or blank string, or a non-string
    placeholder — decides nothing and the dimension stays
    UNEVALUATED."""
    if not isinstance(decisions, dict):
        return None
    reason = decisions.get(dimension)
    if isinstance(reason, str) and reason.strip() \
            and not reason.startswith(COLLECTED_PREFIX):
        return reason
    return None


def decision_reason(dimension, decisions=None):
    """Public form of _decision_reason, for the harness's ledger
    verification: the documented reason deciding a dimension
    away under the given decisions mapping, or None."""
    return _decision_reason(dimension, decisions)


def profile_dimension_states(records, decisions=None,
                               expected_output=None, config=None):
    """The full evaluation profile, enumerated: one entry per
    PROFILE_DIMENSIONS dimension, in order, each
    {dimension, state, detail} with state PASS, FAIL, UNEVALUATED
    or NOT-APPLICABLE.

    The two dimensions the freeze gate maps into carry the gate's
    verdict aggregated over both contexts (FAIL if any mapped
    gate entry fails, UNEVALUATED if any is unevaluated or none
    ran, PASS only when every mapped entry passes). Every other
    dimension defaults to UNEVALUATED — the campaign collects no
    observations for it — unless the decisions mapping the run's
    configuration carries (decisions_from_config) has an entry
    for it with a documented (non-empty) reason, in which case it
    renders NOT-APPLICABLE with that entry's reason as its
    detail. A dimension with no configuration decision can never
    render NOT-APPLICABLE, and neither can one whose decision
    carries no reason (see _decision_reason)."""
    gate = freeze_gate(records, expected_output, config)
    states = []
    for dimension in PROFILE_DIMENSIONS:
        mapped = _PROFILE_GATE_MAP.get(dimension)
        if mapped:
            entries = [e for e in gate if e["dimension"] in mapped]
            outcomes = {e["outcome"] for e in entries}
            if "FAIL" in outcomes:
                state = "FAIL"
            elif entries and outcomes == {"PASS"}:
                state = "PASS"
            else:
                state = "UNEVALUATED"
            detail = ("from the freeze gate: " + "; ".join(
                f"{name} " + ", ".join(
                    f"{e['context']} {e['outcome']}" for e in entries
                    if e["dimension"] == name)
                for name in mapped))
            states.append({"dimension": dimension, "state": state,
                           "detail": detail})
        elif _collection_entry(dimension, decisions) is not None:
            instrument = _collection_entry(dimension, decisions)
            if dimension == DIM_USABILITY:
                usability_state = _usability_state()
                if usability_state == "FAIL":
                    states.append({
                        "dimension": dimension, "state": "FAIL",
                        "detail": "collected by the named manual "
                                  f"instrument: {instrument}; the "
                                  "artifact it names records a "
                                  "prompt class exercised with an "
                                  "observed outcome of 'fail' — "
                                  "the dimension is collected "
                                  "and FAILS on that evidence "
                                  "(round-29 F1: a fail is a "
                                  "standing loss, never the "
                                  "dimension satisfied; a fail "
                                  "dominates any pass)"})
                elif usability_state == "COLLECTED":
                    states.append({
                        "dimension": dimension, "state": "COLLECTED",
                        "detail": "collected by the named manual "
                                  f"instrument: {instrument}; the "
                                  "artifact it names records at "
                                  "least one prompt class exercised "
                                  "with an observed outcome of "
                                  "'pass' and no 'fail'"})
                else:
                    states.append({
                        "dimension": dimension,
                        "state": "UNEVALUATED",
                        "detail": "the named manual instrument: "
                                  f"{instrument}; its artifact "
                                  "records no prompt class "
                                  "exercised with an observed "
                                  "outcome (every outcome stands "
                                  "at 'not-run'), so the "
                                  "dimension is not yet collected"})
            elif dimension == DIM_RESOURCE:
                state = _collected_record_state(dimension, records)
                coverage = _resource_coverage(records)
                summary = "; ".join(
                    f"{arm}: {cpu} numeric cpu_ms and {peak} "
                    f"numeric peak_working_set_bytes over {total} "
                    f"measured record(s)"
                    for arm, (total, cpu, peak)
                    in sorted(coverage.items()))
                detail = ("collected by the named instrument: "
                          f"{instrument}; state computed per "
                          f"arm from this run's records — "
                          f"{summary}")
                cost_losses, unpaired, compared_units = \
                    _paired_cost_comparison_full(records)
                if state == "FAIL" and cost_losses:
                    detail += ("; FAIL on the arm comparison: "
                               + "; ".join(cost_losses))
                if state == "PASS" and compared_units:
                    # Round-34 F4: a PASS renders the per-unit
                    # numeric and planned counts every one of
                    # its comparisons rested on — the unpaired
                    # discipline of round-33 F4 extended from
                    # unjudged units to all units, so a PASS at
                    # exactly half coverage reads as one.
                    detail += ("; compared cost units (per-unit "
                               "numeric/planned counts the "
                               "comparison rested on): "
                               + "; ".join(compared_units))
                if unpaired:
                    # Round-33 F4: the unpaired population is
                    # RENDERED BY NAME here — the profile is a
                    # verdict consumer, and a name that reaches
                    # no profile detail, ledger basis, loss or
                    # report is naming in a test-only channel.
                    detail += ("; unpaired cost units (no cost "
                               "comparison can be made there): "
                               + "; ".join(unpaired))
                states.append({
                    "dimension": dimension, "state": state,
                    "detail": detail})
            else:
                state = _collected_record_state(dimension, records)
                detail = ("collected by the named instrument: "
                          f"{instrument}; state from this "
                          "run's records for that collection")
                if dimension == DIM_SECURITY:
                    # Round-38 F2: the sentinel cap (see
                    # _security_sentinel_degradation) — a
                    # would-be PASS under NO_VERIFIED_PLAN
                    # renders UNEVALUATED, degraded-named here.
                    degradation = _security_sentinel_degradation(
                        config)
                    if degradation is not None and state == "PASS":
                        state = "UNEVALUATED"
                        detail += f"; DEGRADED: {degradation}"
                states.append({
                    "dimension": dimension, "state": state,
                    "detail": detail})
        elif _decision_reason(dimension, decisions) is not None:
            states.append({
                "dimension": dimension, "state": "NOT-APPLICABLE",
                "detail": _decision_reason(dimension, decisions)})
        else:
            states.append({
                "dimension": dimension, "state": "UNEVALUATED",
                "detail": "the campaign collects no observations "
                          "for this dimension and no decision in "
                          "the run's configuration decides it away"})
    return states


def claim_ledger(records, decisions=None, expected_output=None,
                 config=None):
    """The claim ledger (SPEC_Benchmark section 11): one claim
    per profile dimension, in PROFILE_DIMENSIONS order, each
    {dimension, state, basis} — plus "reason" for a decided
    dimension, and for a collected dimension (round-26) the
    same field carrying the configuration's collection entry
    verbatim. The state vocabulary is the ledger's own: a
    gate-observed dimension is "observed-passing" or
    "observed-failing" (or UNEVALUATED when the gate could not
    judge it); a dimension the run's configuration decides away
    is NOT-APPLICABLE; a dimension collected by its named
    manual instrument is "collected" (record-collected
    dimensions take their state from their records, in the
    observed vocabulary); anything else is UNEVALUATED.

    Every basis points inside the run package: for an observed
    dimension, to the freeze gate over observations.jsonl, naming
    each mapped gate dimension's outcome per context; for a
    decided dimension, to the decision in the configuration whose
    SHA-256 run.json records, with its reason carried verbatim;
    for a collected dimension, to the collection entry in that
    same configuration, carried verbatim, with the state taken
    from the instrument's evidence; for an UNEVALUATED
    dimension, to the absence itself — no
    observation source in the package and no configuration
    decision."""
    gate = freeze_gate(records, expected_output, config)
    ledger = []
    for dimension in PROFILE_DIMENSIONS:
        mapped = _PROFILE_GATE_MAP.get(dimension)
        if mapped:
            entries = [e for e in gate if e["dimension"] in mapped]
            outcomes = {e["outcome"] for e in entries}
            if "FAIL" in outcomes:
                state = LEDGER_OBSERVED_FAILING
            elif entries and outcomes == {"PASS"}:
                state = LEDGER_OBSERVED_PASSING
            else:
                state = LEDGER_UNEVALUATED
            basis = ("evidence in this package: observations.jsonl "
                     "via the freeze gate — " + "; ".join(
                         f"{name} " + ", ".join(
                             f"{e['context']} {e['outcome']}"
                             for e in entries if e["dimension"] == name)
                         for name in mapped))
            ledger.append({"dimension": dimension, "state": state,
                           "basis": basis})
        elif _collection_entry(dimension, decisions) is not None:
            instrument = _collection_entry(dimension, decisions)
            if dimension == DIM_USABILITY:
                usability_state = _usability_state()
                if usability_state == "FAIL":
                    ledger.append({
                        "dimension": dimension,
                        "state": LEDGER_OBSERVED_FAILING,
                        "basis": "collected by the manual instrument "
                                 "named in the run's configuration "
                                 f"('profile_dimension_decisions'): "
                                 f"{decisions[dimension]}; the "
                                 "instrument's recorded evidence lives "
                                 "in the suite artifact it names, "
                                 "tier-labelled manual, and that "
                                 "artifact records a prompt class "
                                 "exercised with an observed "
                                 "outcome of 'fail' — the "
                                 "dimension is collected and "
                                 "observed-failing on that evidence "
                                 "(round-29 F1: a fail renders a "
                                 "standing loss, never the "
                                 "dimension satisfied; a fail "
                                 "dominates any pass) — it is not a "
                                 "campaign observation and claims no "
                                 "campaign measurement",
                        "reason": decisions[dimension]})
                elif usability_state == "COLLECTED":
                    ledger.append({
                        "dimension": dimension,
                        "state": LEDGER_COLLECTED,
                        "basis": "collected by the manual instrument "
                                 "named in the run's configuration "
                                 f"('profile_dimension_decisions'): "
                                 f"{decisions[dimension]}; the "
                                 "instrument's recorded evidence lives "
                                 "in the suite artifact it names, "
                                 "tier-labelled manual, and that "
                                 "artifact records at least one "
                                 "prompt class exercised with an "
                                 "observed outcome of 'pass' and no "
                                 "'fail' — it is not a "
                                 "campaign observation and claims no "
                                 "campaign measurement",
                        "reason": decisions[dimension]})
                else:
                    ledger.append({
                        "dimension": dimension,
                        "state": LEDGER_UNEVALUATED,
                        "basis": "the manual instrument named in "
                                 "the run's configuration "
                                 f"('profile_dimension_decisions'): "
                                 f"{decisions[dimension]}; its "
                                 "recorded evidence — the suite "
                                 "artifact it names "
                                 "(usability-evidence.json) — "
                                 "records no prompt class exercised "
                                 "with an observed outcome (every "
                                 "outcome stands at 'not-run'), so "
                                 "the dimension is not yet "
                                 "collected",
                        "reason": decisions[dimension]})
            else:
                state = _collected_record_state(dimension, records)
                capped = False
                degradation = None
                if dimension == DIM_SECURITY:
                    # Round-38 F2: the sentinel cap (see
                    # _security_sentinel_degradation) — a
                    # would-be PASS under NO_VERIFIED_PLAN
                    # renders UNEVALUATED, degraded-named in
                    # the basis below, so the standing-loss
                    # entry built from this basis names the
                    # underived budget.
                    degradation = _security_sentinel_degradation(
                        config)
                    if degradation is not None and state == "PASS":
                        state = "UNEVALUATED"
                        capped = True
                ledger_state = {
                    "PASS": LEDGER_OBSERVED_PASSING,
                    "FAIL": LEDGER_OBSERVED_FAILING,
                    "UNEVALUATED": LEDGER_UNEVALUATED,
                }[state]
                basis = ("evidence in this package: "
                         "observations.jsonl via the named "
                         "collecting instrument in the run's "
                         "configuration "
                         f"('profile_dimension_decisions'): "
                         f"{decisions[dimension]}")
                if capped:
                    basis += f"; DEGRADED: {degradation}"
                if dimension == DIM_SECURITY and state == "FAIL":
                    violations = _designated_malformed_violations(
                        records)
                    named = "; ".join(
                        f"{_record_variant(record)} in "
                        f"{record.get('context')} rep "
                        f"{record.get('rep')}: exit "
                        f"{record.get('exit_code')!r} against the "
                        f"pinned refusal "
                        f"{malformed_expected_exit(DESIGNATED_ARM, _record_variant(record))}"
                        for record in violations)
                    basis += ("; FAIL: the designated arm "
                              "violated its pinned malformed "
                              f"expectations — {named}")
                if dimension == DIM_RESOURCE:
                    # The ledger states what the state observed
                    # (round-27 F3): PASS is per-arm numeric
                    # coverage of both counters, FAIL names the
                    # arm(s) a PASS would have had to observe.
                    # Round-31 F1, rebuilt round-32 F1,
                    # round-33 F3/F4 and round-34 F1/F4: PASS
                    # additionally observes
                    # the arm comparison — the designated arm
                    # wins or ties every cost comparison (per
                    # counter and (path, context, variant)
                    # cell, sorted-rank dominance over equal
                    # numeric
                    # populations at the coverage floor — the
                    # designated arm's sorted values at or
                    # below the competitor's at every rank) — a
                    # cost FAIL names its cells, the
                    # unpaired units are named in the basis
                    # whenever they stand, and a PASS renders
                    # every compared unit's per-unit numeric
                    # and planned counts (round-34 F4).
                    coverage = _resource_coverage(records)
                    summary = "; ".join(
                        f"{arm}: {cpu} numeric cpu_ms and {peak} "
                        f"numeric peak_working_set_bytes over "
                        f"{total} measured record(s)"
                        for arm, (total, cpu, peak)
                        in sorted(coverage.items()))
                    cost_losses, unpaired, compared_units = \
                        _paired_cost_comparison_full(records)
                    if state == "PASS":
                        basis += ("; PASS observed per-arm numeric "
                                  f"coverage: {summary}; and the "
                                  "designated arm wins or ties "
                                  "every cost comparison "
                                  "(per counter and (path, context, "
                                  "variant) cell — sorted-rank "
                                  "dominance over equal numeric "
                                  "populations at the per-unit "
                                  "coverage floor — the designated "
                                  "arm's sorted values at or below "
                                  "the competitor's at every rank)")
                        if compared_units:
                            basis += ("; compared cost units "
                                      "(per-unit numeric/planned "
                                      "counts the comparison "
                                      "rested on): "
                                      + "; ".join(compared_units))
                    elif state == "FAIL":
                        if cost_losses:
                            basis += ("; FAIL on the arm "
                                      "comparison — cost loss: "
                                      + "; ".join(cost_losses)
                                      + f" — coverage: {summary}")
                        else:
                            basis += ("; FAIL: an arm's measured "
                                      "records carry no numeric "
                                      "measurement of a resource "
                                      f"counter — coverage: "
                                      f"{summary}")
                    if unpaired:
                        # Round-33 F4: the unpaired population
                        # is RENDERED BY NAME in the ledger basis
                        # — the UNEVALUATED standing loss is built
                        # from this basis, so the names reach the
                        # loss and the rendered report, the way
                        # cost losses are carried on FAIL.
                        basis += ("; unpaired cost units (no "
                                  "cost comparison can be made "
                                  "there): "
                                  + "; ".join(unpaired))
                ledger.append({
                    "dimension": dimension, "state": ledger_state,
                    "basis": basis,
                    "reason": decisions[dimension]})
        elif _decision_reason(dimension, decisions) is not None:
            reason = _decision_reason(dimension, decisions)
            ledger.append({
                "dimension": dimension,
                "state": LEDGER_NOT_APPLICABLE,
                "basis": "configuration decision: "
                         f"'{DECISIONS_CONFIG_KEY}' in the run's "
                         "configuration (campaign.json), whose "
                         "SHA-256 run.json records for this run; "
                         f"reason: {reason}",
                "reason": reason})
        else:
            ledger.append({
                "dimension": dimension, "state": LEDGER_UNEVALUATED,
                "basis": "no evidence in this package: the campaign "
                         "collects no observations for this "
                         "dimension and the run's configuration "
                         "carries no decision for it"})
    return ledger


def profile_dimension_losses(records, decisions=None,
                               expected_output=None, config=None):
    """Standing losses from the evaluation profile, taken from
    the claim ledger: every profile dimension whose ledger state
    is UNEVALUATED. (An observed-failing dimension is already a
    freeze-gate standing loss; a NOT-APPLICABLE dimension,
    decided away by a decision in the run's configuration, is no
    loss.) While any entry here stands, the campaign is
    INCOMPLETE — never a certified result — even if every
    observed dimension passes.

    The FAIL entries below are ONE entry per failing dimension
    (round-35 F4 states this breadth exactly, where round 34's
    framing claimed per-unit-per-counter losses): the entry for
    dimension (3)'s dominance failure carries the failure's
    MAGNITUDE — every failing (unit, counter) pair by name with
    its failing-rank count, and the aggregate failing-rank
    total — so the list a reader totals does not render a
    3-rank failure and a 2,520-rank failure as the identical
    line. The counting consumers' meanings are unchanged:
    standing_loss_count counts ENTRIES (a dimension failure is
    one entry however many pairs it names), and
    freeze_gate_fail_count counts gate FAIL entries only."""
    losses = [
        f"evaluation profile dimension UNEVALUATED: "
        f"{entry['dimension']} — {entry['basis']}; the campaign "
        f"is INCOMPLETE until every profile dimension is "
        f"evaluated or decided NOT-APPLICABLE by a decision in "
        f"the run's configuration"
        for entry in claim_ledger(records, decisions, expected_output,
                                  config)
        if entry["state"] == LEDGER_UNEVALUATED
    ]
    # A collected dimension (round-26) whose own records fail it
    # is a standing loss in its own right: the gate dimensions'
    # observed-failing states are already freeze-gate losses, but
    # (3)/(4)/(6) are observed only here, and (8) likewise
    # (round-29 F1: DIM_USABILITY joins this FAIL loop — a
    # usability fail stands as a campaign standing loss).
    for entry in profile_dimension_states(records, decisions,
                                          expected_output, config):
        if entry["dimension"] in (DIM_RESOURCE, DIM_SCALING,
                                  DIM_SECURITY, DIM_USABILITY) \
                and entry["state"] == "FAIL":
            text = (
                f"evaluation profile dimension FAIL: "
                f"{entry['dimension']} — collected by the named "
                "instrument and failed on this run's records for "
                "that collection")
            if entry["dimension"] == DIM_RESOURCE:
                # Round-35 F4: a dominance failure's entry
                # carries its magnitude — the failing (unit,
                # counter) pairs and the aggregate failing-rank
                # total — so two packages differing by orders
                # of magnitude in failing ranks cannot render
                # identical loss lists. (A coverage failure —
                # an arm carrying no numeric counter — has no
                # failing pairs to total: its entry stands as
                # the generic entry, and the ledger basis names
                # the coverage it failed on.)
                _losses, _unpaired, _compared, failing_pairs = \
                    _paired_cost_comparison_detail(records)
                if failing_pairs:
                    total_ranks = sum(
                        count for _where, count in failing_pairs)
                    text += (
                        f"; cost comparison magnitude: "
                        f"{len(failing_pairs)} failing (unit, "
                        f"counter) pair(s) totalling "
                        f"{total_ranks} failing rank(s) — "
                        + "; ".join(
                            f"{where}: {count} failing rank(s)"
                            for where, count in failing_pairs))
            losses.append(text)
    return losses


def _competitor_loss_entry(record):
    return (
        COMPETITOR_LOSS_PREFIX
        + f"{record['arm']} kill path left a descendant alive in "
          f"{record['context']} ({record.get('kind')} rep "
          f"{record['rep']})")


def _competitor_output_loss_entry(record):
    return (
        COMPETITOR_LOSS_PREFIX
        + f"{record['arm']} fast path produced output that did "
          f"not match the expected bytes in {record['context']} "
          f"({record.get('kind')} rep {record['rep']})")


def _evidenced_exit_mismatch(record):
    """True when a record's exit fault is earned by its own
    fields: an observed exit code that differs from the expected
    exit for its arm and path (arms.expected_exit's table). A
    record labelled "exit-mismatch" whose exit code is the
    expected code (or was never observed) evidences no exit
    fault here; it is a record inconsistency instead. A
    "descendant-survived" kill record with a differing exit
    code evidences the fault under its survival label (the
    runner's survivor-before-exit precedence), so standing
    losses can list the exit fault beside the survival. The
    code must be the type the runner records: a non-boolean
    integer — a differing exit_code of another type (a string,
    a float, a boolean, a list) is a record inconsistency,
    named by _record_inconsistencies, and earns no fault
    here."""
    expected = expected_exit(record.get("arm"), record.get("path"))
    actual = record.get("exit_code")
    return expected is not None and actual is not None \
        and isinstance(actual, int) \
        and not isinstance(actual, bool) \
        and actual != expected


def _competitor_exit_loss_entry(record):
    expected = expected_exit(record.get("arm"), record.get("path"))
    return (
        COMPETITOR_LOSS_PREFIX
        + f"{record['arm']} {record['path']} path produced exit "
          f"code {record.get('exit_code')} instead of the "
          f"expected {expected} in {record['context']} "
          f"({record.get('kind')} rep {record['rep']})")


def _evidenced_output_mismatch(record):
    """True when a fast record's output fault is earned by its
    own fields: the "output-mismatch" classification AND a
    stream recording the False verdict that earns it AND the
    exit code the runner records from its wait before it
    compares the code and classifies the verdict — an observed
    exit code (not None). A label resting on no False verdict,
    or on an exit code that was never observed, is a record
    inconsistency instead (see _record_inconsistencies) and
    mints no competitor loss here."""
    return record.get("outcome") == "output-mismatch" \
        and record.get("path") == "fast" \
        and record.get("exit_code") is not None \
        and (record.get("output_stdout_match") is False
             or record.get("output_stderr_match") is False)


def competitor_losses(records):
    """Evidence, not campaign standing losses: every competitor
    (non-guard product arm) measured kill slot whose
    descendant-survived classification is evidenced by its
    survivor field, listed per slot, every competitor
    measured fast slot classified "output-mismatch" with a
    stream recording the False verdict that earns it, and every
    competitor measured slot whose exit fault is evidenced by
    its own exit_code field — classified "exit-mismatch", or
    classified "descendant-survived" on the kill path with a
    mismatching exit code riding under the survival label. A label
    its own fields refute mints no entry here; it is a record
    inconsistency instead. Kill-path
    correctness is defined over measured slots (see
    _kill_records), so a descendant-survived kill record of a
    warmup or control kind is not competitor-loss evidence and
    never appears here. Each entry is a competitor loss on the
    dimension it names — kill-path correctness, exit-code
    correctness or output correctness, all components of
    profile dimension (1) — the guard's win on that dimension —
    so these entries carry COMPETITOR_LOSS_PREFIX and never
    block a complete report. Round-40 F3: only records
    matching a plan slot mint an entry — an unplanned record
    is not competitor-loss evidence on any of the three
    components, exactly as it is not kill-state evidence."""
    planned = _planned_records(records)
    losses = [_competitor_loss_entry(record) for record in planned
              if _evidenced_survived(record)
              and record.get("path") == "kill"
              and record.get("kind") == "measured"
              and record.get("arm") in PRODUCT_ARMS
              and record.get("arm") != "guard"]
    losses += [_competitor_output_loss_entry(record) for record in planned
               if _evidenced_output_mismatch(record)
               and record.get("kind") == "measured"
               and record.get("arm") in PRODUCT_ARMS
               and record.get("arm") != "guard"]
    losses += [_competitor_exit_loss_entry(record) for record in planned
               if record.get("outcome") in (
                   "exit-mismatch", "descendant-survived")
               and _evidenced_exit_mismatch(record)
               and record.get("kind") == "measured"
               and record.get("arm") in PRODUCT_ARMS
               and record.get("arm") != "guard"]
    return losses


def standing_losses(records, plan_counts=None, expected_output=None,
                    config=None):
    """Every way the campaign falls short of a certifiable result:
    exact slot accounting, per-record failures, per-context floor
    validity, and the freeze gate per dimension. plan_counts is
    accepted for the driver's convenience and is not the accounting
    standard — the configured plan itself is (build_plan(), frozen
    for a run once that run starts). Competitor descendant-survived
    measured kill slots are listed here as
    competitor losses (evidence, under COMPETITOR_LOSS_PREFIX),
    not as campaign standing losses; render() separates them and
    they do not make a campaign incomplete. Record/label
    disagreements (record_consistency_losses) are standing
    losses here, so no certifying path trusts a classification
    its own record contradicts. A competitor's measured exit
    fault (evidenced by its own exit_code) is listed under
    COMPETITOR_LOSS_PREFIX, like its measured output mismatches
    and descendant survivals; the guard's exit faults, and every
    arm's warmup/control exit faults, are campaign losses."""
    losses = slot_accounting_losses(records)
    losses = losses + record_consistency_losses(
        records, expected_output, config)
    # Round-40 F3: competitor-loss ENTRIES are evidence, and
    # evidence is a planned population — the competitor
    # branches below mint their prefixed entry only for a
    # record matching a plan slot; an unplanned record's
    # fault is listed as a campaign loss in its own name,
    # as named incompleteness, never as competitor evidence.
    # Round-41 F6: that claim holds at full breadth — EVERY
    # unplanned record's evidenced fault is listed in its
    # own name: exit faults (the exit-mismatch branch and
    # the descendant-survived slot's exit component, as
    # before), output mismatches and descendant survivals
    # alike, each under its evidenced predicate. Round 40
    # listed only the exit faults; an unplanned competitor
    # output mismatch or descendant survival minted no
    # prefixed entry and no campaign entry either, so its
    # fault stood named nowhere in the fault listings.
    # Round-41 F1: the planned test the competitor branches
    # apply is the occurrence test — multiplicity-exact, so
    # a duplicate copy of a planned record mints no
    # competitor evidence either; its fault is listed in
    # its own name like any unplanned record's.
    # Round-42 F2: the own-name listing is KIND-COMPLETE —
    # the round-41 fall-throughs carried the measured-kind
    # gate of the evidence branches they mirror, so an
    # unplanned WARMUP competitor record's evidenced output
    # mismatch or descendant survival stood in no fault
    # listing (its standing list byte-identical to a
    # faultless duplicate's), and an unplanned CONTROL
    # record's output fault reached no branch at all (the
    # guard branch names the guard only; the competitor
    # branches require a product arm). The fall-throughs
    # below carry no kind condition and reach every arm:
    # an unplanned record's evidenced fault is listed in
    # its own name at warmup and control grain as at
    # measured grain, under its evidenced predicate, never
    # under COMPETITOR_LOSS_PREFIX. The planned
    # population's kind discipline is untouched: a PLANNED
    # warmup/control record's fault mints no entry here,
    # exactly as before.
    # Round-43 F2: the listing is PATH-COMPLETE for the
    # survival fault — the competitor survival fall-through
    # kept the competitor-loss definition's path == "kill"
    # condition, so a product arm's evidenced fast-path
    # survival (whose _evidenced_survived predicate carries
    # no path test) stood in no fault listing at any kind,
    # planned or unplanned, while the identical floor
    # record was listed in its own name. The fall-through
    # now carries no path condition, and a planned product
    # record's evidenced non-kill-path survival is listed
    # in its own name beside it: the own-name listing is
    # complete over the (arm x kind x path) space its
    # claim spans, the one stated exception standing —
    # a planned warmup/control kill-path survival mints
    # no listing, under the planned population's kind
    # discipline above.
    occurrence_flags = _planned_occurrence_flags(records)
    planned_by_id = {id(record): flag
                     for record, flag
                     in zip(records, occurrence_flags)}
    for record in records:
        if record["outcome"] == "infra-failure":
            losses.append(
                f"infrastructure failure in {record['arm']}/{record['path']}/"
                f"{record['context']} rep {record['rep']}: "
                f"{record.get('detail', 'no detail')}")
        if record["outcome"] == "exit-mismatch":
            if record["arm"] in PRODUCT_ARMS \
                    and record["arm"] != "guard" \
                    and record.get("kind") == "measured" \
                    and planned_by_id.get(id(record), False) \
                    and _evidenced_exit_mismatch(record):
                # A competitor's measured exit fault is a
                # measured product fault on profile dimension
                # (1)'s exit-code component — competitor-loss
                # evidence, like its measured output mismatches
                # and descendant survivals, not a campaign
                # standing loss. (An unevidenced label is a
                # record inconsistency, already listed; the
                # guard's own mismatches, and any arm's warmup
                # or control mismatches, stay campaign losses.)
                losses.append(_competitor_exit_loss_entry(record))
            else:
                losses.append(
                    f"exit mismatch in {record['arm']}/{record['path']}/"
                    f"{record['context']} rep {record['rep']}: "
                    f"{record.get('detail', 'no detail')}")
        if record["outcome"] == "descendant-survived" \
                and _evidenced_exit_mismatch(record):
            # Survivor-before-exit precedence: the slot is
            # classified by its survival, and its exit fault is
            # listed here under its own name — for the guard and
            # for warmup/control records as a campaign loss, for
            # a competitor's measured slot as competitor-loss
            # evidence beside the survival entry below.
            if record["arm"] in PRODUCT_ARMS \
                    and record["arm"] != "guard" \
                    and record.get("kind") == "measured" \
                    and planned_by_id.get(id(record), False):
                losses.append(_competitor_exit_loss_entry(record))
            else:
                losses.append(
                    f"exit mismatch in {record['arm']}/{record['path']}/"
                    f"{record['context']} rep {record['rep']}: "
                    f"exit code {record.get('exit_code')} != expected "
                    f"{expected_exit(record['arm'], record['path'])} "
                    f"for {record['arm']}/{record['path']} (the slot "
                    f"is classified descendant-survived: survivor "
                    f"precedence carries the graver fault, and the "
                    f"exit fault is listed separately, not as a "
                    f"record inconsistency)")
        if record["outcome"] == "output-mismatch" \
                and record["arm"] == "guard":
            losses.append(
                f"output mismatch in {record['arm']}/{record['path']}/"
                f"{record['context']} rep {record['rep']}: "
                f"{record.get('detail', 'no detail')}")
        elif record["outcome"] == "output-mismatch" \
                and record.get("kind") == "measured" \
                and record["arm"] in PRODUCT_ARMS \
                and planned_by_id.get(id(record), False) \
                and _evidenced_output_mismatch(record):
            losses.append(_competitor_output_loss_entry(record))
        elif record["outcome"] == "output-mismatch" \
                and record["arm"] in PRODUCT_ARMS \
                and record["arm"] != "guard" \
                and not planned_by_id.get(id(record), False) \
                and _evidenced_output_mismatch(record):
            # Round-41 F6, kind-completed by round-42 F2:
            # an UNPLANNED competitor record's evidenced
            # output fault is listed in its own name, as the
            # guard's is — named incompleteness, never
            # competitor evidence (the prefixed branch above
            # requires a planned occurrence). The branch
            # carries no kind condition: at warmup and
            # control grain as at measured grain, such a
            # record's fault stands in a fault listing.
            losses.append(
                f"output mismatch in {record['arm']}/{record['path']}/"
                f"{record['context']} rep {record['rep']}: "
                f"{record.get('detail', 'no detail')}")
        elif record["outcome"] == "output-mismatch" \
                and record["arm"] not in PRODUCT_ARMS \
                and not planned_by_id.get(id(record), False) \
                and _evidenced_output_mismatch(record):
            # Round-42 F2: an UNPLANNED record of an arm no
            # branch above reaches — a floor arm's control
            # record above all — lists its evidenced output
            # fault in its own name on the same terms: the
            # claim is every unplanned record's, at every
            # kind, and a duplicate control record's fault
            # stood in no listing before this fall-through.
            losses.append(
                f"output mismatch in {record['arm']}/{record['path']}/"
                f"{record['context']} rep {record['rep']}: "
                f"{record.get('detail', 'no detail')}")
        if record["outcome"] == "descendant-survived" \
                and record["arm"] == "guard":
            losses.append(
                f"guard kill path left a descendant alive in "
                f"{record['context']} rep {record['rep']}")
        elif record["outcome"] == "descendant-survived" \
                and record.get("path") == "kill" \
                and record.get("kind") == "measured" \
                and record["arm"] in PRODUCT_ARMS \
                and planned_by_id.get(id(record), False) \
                and _evidenced_survived(record):
            losses.append(_competitor_loss_entry(record))
        elif record["outcome"] == "descendant-survived" \
                and record["arm"] in PRODUCT_ARMS \
                and record["arm"] != "guard" \
                and planned_by_id.get(id(record), False) \
                and record.get("path") != "kill" \
                and _evidenced_survived(record):
            # Round-43 F2: a PLANNED product record's
            # evidenced survival on a non-kill path (the
            # fast path above all) is listed in its own
            # name, as the guard's is above — the planned
            # grain of the path-complete listing below.
            # (A planned kill-path survival is competitor
            # evidence at measured kind, in the branch
            # above, and mints no listing at warmup/control
            # kind, under the planned population's kind
            # discipline the evidence branches state.)
            losses.append(
                f"{record['arm']} kill path left a descendant "
                f"alive in {record['context']} rep "
                f"{record['rep']}")
        elif record["outcome"] == "descendant-survived" \
                and record["arm"] in PRODUCT_ARMS \
                and record["arm"] != "guard" \
                and not planned_by_id.get(id(record), False) \
                and _evidenced_survived(record):
            # Round-41 F6, kind-completed by round-42 F2,
            # path-completed by round-43 F2: an UNPLANNED
            # competitor record's evidenced survival is
            # listed in its own name, as the guard's is
            # above — named incompleteness, never
            # competitor evidence. The branch carries no
            # kind condition and no path condition: at
            # warmup and control grain as at measured
            # grain, on the fast path as on the kill path,
            # such a record's fault stands in a fault
            # listing. (Before round 43 the branch kept
            # the competitor-loss definition's path ==
            # "kill" condition, so a product arm's
            # evidenced fast-path survival stood in no
            # fault listing in 9 of 9 enumerated
            # (arm x kind) cells, planned or unplanned,
            # while a floor arm's identical record was
            # listed in its own name.)
            losses.append(
                f"{record['arm']} kill path left a descendant "
                f"alive in {record['context']} rep "
                f"{record['rep']}")
        elif record["outcome"] == "descendant-survived" \
                and record["arm"] not in PRODUCT_ARMS \
                and not planned_by_id.get(id(record), False) \
                and _evidenced_survived(record):
            # Round-42 F2: an UNPLANNED record of an arm no
            # branch above reaches lists its evidenced
            # survival in its own name on the same terms —
            # every unplanned record's evidenced fault, at
            # every kind, never nowhere.
            losses.append(
                f"{record['arm']} kill path left a descendant "
                f"alive in {record['context']} rep "
                f"{record['rep']}")
    for context in CONTEXTS:
        has_floor_records = any(
            r.get("kind") == "control" and r.get("context") == context
            for r in records)
        floor, agreed = floor_ms(records, context)
        if has_floor_records and floor is None:
            losses.append(
                f"no usable floor in {context}: the context "
                "carries floor records but no floor can be stated "
                "from them (at least one floor arm yields no ok "
                "control median); the floor's absence is not "
                "agreement, and a campaign whose headline is a "
                "distance above that floor cannot certify without "
                "one")
        elif has_floor_records and floor is not None and not agreed:
            losses.append(
                f"the two floor arms disagree in {context}; no floor "
                "can be stated for that context")
        guard_fast = _median(records, "guard", "fast", context)
        if floor is not None and agreed and guard_fast is not None \
                and guard_fast < floor:
            losses.append(
                f"guard median below the floor in {context}: "
                "measurement invalid, not a win")
    for entry in freeze_gate(records, expected_output, config):
        if entry["outcome"] != "PASS":
            losses.append(
                f"freeze gate [{entry['context']}] "
                f"{entry['dimension']}: {entry['outcome']} — "
                f"{entry['detail']}")
    return losses


def _pricing_items(pricing):
    """Resolve the pricing argument to per-item entries.

    Returns (items, priced_total, any_priced). Only the pricing-probe
    evidence object — a reference to recorded probe observations,
    carrying its kind tag and the guard fingerprint the probes
    measured — yields numbers, each traceable through its probe name
    and paired-observation count. Any other non-None pricing (a bare
    dict of numbers from the configuration) is operator-supplied:
    every item it names is reported as "operator-supplied, not
    measured" with no number, and items it does not name are simply
    not measured. An item's ms yields a number only when it is a
    finite, non-boolean number: a boolean is not a time, and NaN
    and the infinities are not medians of paired differences —
    each resolves to "not measured", as a non-number already does,
    so no decomposition remainder is ever computed from one.
    Total (round-30 F2): the finiteness tests are
    _is_finite_number's, so an int too large to convert to float
    (10**400, in the item's ms or among its paired differences)
    resolves the same way — "not measured", or through the stated
    counts when the differences are unusable — never an
    OverflowError out of this rollup.

    The sign counts the sign-inconsistency flag states are held
    the same way: the verdict is this rollup's own, so it is not
    certified from the entry's unchecked count fields. When the
    entry carries its paired_differences_ms (the differences
    themselves, as the production producer records them), the
    sign counts are DERIVED from those differences and govern —
    the resolved item carries the derived counts, the flag is
    stated from them, and stated counts that disagree with the
    derived counts (in value, or in type — a count of another
    type is not the differences' count) are themselves named as
    an inconsistency, stated from the derived counts alone so a
    value of another type never reaches the flag text. When the
    entry carries no usable differences, its stated counts are
    used only when every count field it carries is a
    non-boolean integer; an entry whose count fields are not
    integers resolves to "not measured", as a non-finite ms
    does — a verdict field is not carried by a value of another
    type.

    The price itself, and the paired-observation count the
    price's trace and the sign flag state, are held to the same
    differences. The producer (pricing.prices_from_observations)
    defines an item's ms as the median of its paired
    differences, so when the entry carries usable differences
    an ms that disagrees with that median is named as an
    inconsistency on the item — a price its own carried
    differences refute is not a clean measurement. The items
    container carrying the entries is held in the same idiom:
    an "items" that is not an object (a string, null, a list)
    carries no entries, so every item resolves to "not
    measured" — the rollup never raises on its own input's
    container flip. And
    paired_observations, wherever the entry states it, is
    admitted only as a non-boolean integer, equal to the
    differences' length when differences are carried: an entry
    stating it in another type, or at another value than the
    differences it carries, resolves to "not measured", so the
    unchecked value never renders into any flag or trace text
    and no remainder is computed from a price whose stated
    observation count its own differences refute."""
    items = {}
    priced_total = 0.0
    any_priced = False
    if isinstance(pricing, dict) and pricing.get("kind") == "pricing-probe-evidence" \
            and pricing.get("guard_fingerprint"):
        measured = pricing.get("items", {})
        if not isinstance(measured, dict):
            measured = {}
        for item in EXPENSIVE_CLASS:
            entry = measured.get(item)
            ms = entry.get("ms") if isinstance(entry, dict) else None
            # The paired differences the entry carries, when they
            # are usable as differences: a list (or tuple) whose
            # every element is a finite, non-boolean number. The
            # sign counts are derived from them when present; a
            # differences field of any other shape derives
            # nothing and the stated counts are held instead.
            differences = None
            if isinstance(entry, dict):
                carried = entry.get("paired_differences_ms")
                # Total (round-30 F2): the finiteness term goes
                # through _is_finite_number, so a difference of
                # 10**400 makes the carried differences unusable —
                # the entry resolves through its stated counts or
                # to "not measured" below — instead of raising
                # OverflowError out of math.isfinite.
                if isinstance(carried, (list, tuple)) and all(
                        _is_finite_number(d) for d in carried):
                    differences = list(carried)
            derived = None
            if differences is not None:
                derived = (
                    sum(1 for d in differences if d > 0),
                    sum(1 for d in differences if d < 0),
                    sum(1 for d in differences if d == 0))
            # Without usable differences, the stated counts are
            # admitted only as non-boolean integers: an entry
            # carrying a count field of another type resolves to
            # "not measured" below, as a non-finite ms does.
            counts_admitted = differences is not None or all(
                isinstance(entry.get(field), int)
                and not isinstance(entry.get(field), bool)
                for field in ("positive_count", "negative_count",
                              "zero_count")
                if isinstance(entry, dict) and field in entry)
            # The stated paired-observation count is admitted
            # only as a non-boolean integer, and — when the entry
            # carries its differences — only at the differences'
            # length: the count the price's trace and the sign
            # flag state is the count of the pairs the price
            # rests on, so a stated count of another type or
            # value resolves the entry to "not measured" below
            # and never renders unchecked into flag or trace
            # text. An entry stating no count admits nothing to
            # hold and is judged on its other fields.
            pairs_admitted = True
            if isinstance(entry, dict) \
                    and "paired_observations" in entry:
                stated_pairs = entry.get("paired_observations")
                if not isinstance(stated_pairs, int) \
                        or isinstance(stated_pairs, bool):
                    pairs_admitted = False
                elif differences is not None \
                        and stated_pairs != len(differences):
                    pairs_admitted = False
            if isinstance(entry, dict) \
                    and _is_finite_number(ms) \
                    and counts_admitted \
                    and pairs_admitted:
                resolved = {
                    "ms": entry["ms"], "tier": "measured (pricing probe)",
                    "trace": entry.get("trace", ""),
                }
                for field in ("min_ms", "max_ms", "q1_ms", "q3_ms",
                              "iqr_ms", "paired_differences_ms"):
                    if field in entry:
                        resolved[field] = entry[field]
                if derived is not None:
                    resolved["positive_count"] = derived[0]
                    resolved["negative_count"] = derived[1]
                    resolved["zero_count"] = derived[2]
                else:
                    for field in ("positive_count", "negative_count",
                                  "zero_count"):
                        if field in entry:
                            resolved[field] = entry[field]
                flags = []
                if entry["ms"] < 0:
                    flags.append(
                        f"negative price {entry['ms']:.3f} ms for "
                        f"{item}: a priced cost cannot be negative")
                if derived is not None:
                    positive, negative, zero = derived
                    pairs = len(differences)
                    if differences:
                        # The producer defines the price as the
                        # median of the paired differences it
                        # carries; an ms that is not that median
                        # is a price its own differences refute,
                        # named here — the honest entry, whose
                        # ms IS the median, earns no such flag
                        # and keeps exactly its previous
                        # behaviour.
                        median = statistics.median(differences)
                        if entry["ms"] != median:
                            flags.append(
                                f"ms stated for {item} does not "
                                f"agree with the median of its "
                                f"paired_differences_ms (stated "
                                f"{entry['ms']:.3f} ms; the "
                                f"differences' median is "
                                f"{median:.3f} ms, as the "
                                f"producer computes it); a price "
                                f"its own carried differences "
                                f"refute is not a clean "
                                f"measurement")
                    stated_disagree = any(
                        field in entry
                        and (not isinstance(entry.get(field), int)
                             or isinstance(entry.get(field), bool)
                             or entry.get(field) != derived[index])
                        for index, field in enumerate(
                            ("positive_count", "negative_count",
                             "zero_count")))
                    if stated_disagree:
                        flags.append(
                            f"sign counts stated for {item} do not "
                            f"agree with the sign counts derived "
                            f"from its paired_differences_ms "
                            f"({positive} positive, {negative} "
                            f"negative, {zero} zero of {pairs} "
                            f"pairs); the derived counts govern")
                else:
                    positive = entry.get("positive_count")
                    negative = entry.get("negative_count")
                    pairs = entry.get("paired_observations", "?")
                if positive and negative:
                    flags.append(
                        f"paired differences for {item} are "
                        f"sign-inconsistent ({positive} positive, "
                        f"{negative} negative of "
                        f"{pairs} pairs)")
                resolved["inconsistencies"] = flags
                items[item] = resolved
                priced_total += entry["ms"]
                any_priced = True
            else:
                items[item] = {"ms": None, "tier": "not measured"}
    elif pricing:
        for item in EXPENSIVE_CLASS:
            if isinstance(pricing, dict) and item in pricing:
                items[item] = {"ms": None,
                               "tier": "operator-supplied, not measured"}
            else:
                items[item] = {"ms": None, "tier": "not measured"}
    else:
        for item in EXPENSIVE_CLASS:
            items[item] = {"ms": None, "tier": "not measured"}
    return items, priced_total, any_priced


def decompose(records, pricing=None, context=None):
    """Distance above the floor, decomposed against the census.

    With a context named, returns that context's decomposition (or a
    status explaining why none exists there). With no context,
    returns {"contexts": {context: decomposition}} — never a pooled
    number. pricing is the pricing-probe evidence object (see
    _pricing_items): a reference to the campaign's recorded probe
    observations, never a bare dict of numbers. Unpriced items are
    reported as not measured; the remainder is stated as the
    unexplained distance, never spread across items by guesswork."""
    if context is None:
        return {"contexts": {ctx: decompose(records, pricing, ctx)
                             for ctx in CONTEXTS}}
    floor, agreed = floor_ms(records, context)
    guard_fast = _median(records, "guard", "fast", context)
    if floor is None or guard_fast is None:
        return {"status": f"not measurable in {context} from these records"}
    if not agreed:
        return {"status": "the two floor arms disagree; no floor can be "
                          "stated and no distance above a floor is reported"}
    distance = guard_fast - floor
    items, priced_total, any_priced = _pricing_items(pricing)
    inconsistencies = []
    for name, item in items.items():
        for flag in item.get("inconsistencies", []):
            inconsistencies.append(f"INCONSISTENCY: {flag}")
    if any_priced and priced_total > distance:
        inconsistencies.append(
            f"INCONSISTENCY: the priced items sum to "
            f"{priced_total:.3f} ms, exceeding the "
            f"{distance:.3f} ms distance above the floor; the "
            f"decomposition's parts do not sum and no unexplained "
            f"remainder is reported for this context")
    inconsistent = bool(inconsistencies)
    return {
        "context": context,
        "floor_ms": floor, "floor_agreed": agreed,
        "guard_fast_median_ms": guard_fast,
        "distance_above_floor_ms": distance,
        "census": CENSUS,
        "items": items,
        "priced_total_ms": priced_total if any_priced else None,
        "inconsistent": inconsistent,
        "inconsistencies": inconsistencies,
        "unexplained_ms": (distance - priced_total
                           if any_priced and not inconsistent else None),
    }


def pricing_inconsistency_losses(records, pricing=None):
    """Standing losses for pricing decompositions that do not hold
    together: per context, every INCONSISTENCY decompose() flags
    (a negative price, sign-inconsistent paired differences, or
    priced items summing past the distance). Contexts with no
    measurable decomposition contribute nothing."""
    losses = []
    if not pricing:
        return losses
    for context in CONTEXTS:
        decomp = decompose(records, pricing, context)
        for reason in decomp.get("inconsistencies", []):
            losses.append(
                f"pricing decomposition [{context}]: {reason}")
    return losses


def campaign_losses(records, plan_counts=None, identity_losses=None,
                    pricing=None, decisions=None, reduced=False,
                    expected_output=None, config=None):
    """Every campaign standing loss, competitor-loss evidence
    entries included under their prefix: exact slot accounting,
    per-record failures, floor validity, the freeze gate,
    competitor-identity losses when given, pricing
    INCONSISTENCY losses when pricing evidence is given, and
    evaluation-profile losses (every PROFILE_DIMENSIONS dimension
    the claim ledger leaves UNEVALUATED — see
    profile_dimension_losses()). decisions is the decisions
    mapping the run's configuration carries (see
    decisions_from_config). When reduced is true the run was
    made with --reduced, and REDUCED_SHAPE_LOSS is added for
    that reason alone: a reduced run of any size, the full plan
    size included, carries it, so its outcome is incomplete even
    when every record is otherwise perfect. This is
    the single completeness standard render() and the campaign
    driver's recorded campaign_status both use, so an
    UNEVALUATED profile dimension makes the campaign INCOMPLETE
    in the status logic, not only in the report text.
    expected_output, when given, is the expected-bytes mapping
    the record-consistency discipline holds a match verdict's
    evidence to (see _record_inconsistencies)."""
    losses = standing_losses(records, plan_counts, expected_output,
                             config)
    if identity_losses:
        losses = losses + list(identity_losses)
    losses = losses + pricing_inconsistency_losses(records, pricing)
    losses = losses + profile_dimension_losses(
        records, decisions, expected_output, config)
    if reduced:
        losses = losses + [REDUCED_SHAPE_LOSS]
    return losses


def render(records, plan_counts, fingerprints, pricing=None, platform="",
           arm_versions=None, identity_losses=None, decisions=None,
           reduced=False, expected_output=None, config=None):
    """The full report as text. Incomplete is a first-class outcome.
    identity_losses, when given, are competitor-identity mismatches:
    in a certifying (non-reduced) shape they are standing losses,
    because a campaign that measured the wrong binaries is not a
    result. arm_versions maps arm name to its probed version string
    (or None when unprobed); every arm's fingerprint and version are
    listed, not only the guard's. reduced carries the run's
    --reduced shape into the same completeness standard
    campaign_losses() applies (see REDUCED_SHAPE_LOSS), so a
    re-rendered reduced run never renders a complete STATUS."""
    losses = campaign_losses(records, plan_counts,
                             identity_losses=identity_losses,
                             pricing=pricing, decisions=decisions,
                             reduced=reduced,
                             expected_output=expected_output,
                             config=config)
    competitor = [loss for loss in losses
                  if loss.startswith(COMPETITOR_LOSS_PREFIX)]
    campaign_standing = [loss for loss in losses
                         if not loss.startswith(COMPETITOR_LOSS_PREFIX)]
    complete = not campaign_standing
    lines = []
    lines.append("PROCESS-TREE-GUARD BENCHMARK CAMPAIGN")
    lines.append(f"platform: {platform or 'not recorded'}")
    lines.append("")
    lines.append("Anchor arm (current gauntlet binary):")
    guard_fp = fingerprints.get("guard", {})
    lines.append(f"  {guard_fp.get('path', 'not recorded')} "
                 f"sha256={guard_fp.get('sha256', 'not recorded')} "
                 f"size={guard_fp.get('size', 'not recorded')}")
    lines.append("")
    lines.append("Arm identities (every arm's fingerprint and probed version):")
    for arm in PRODUCT_ARMS:
        fp = fingerprints.get(arm, {})
        version = (arm_versions or {}).get(arm)
        lines.append(f"  {arm}: {fp.get('path', 'not recorded')} "
                     f"sha256={fp.get('sha256', 'not recorded')} "
                     f"size={fp.get('size', 'not recorded')} "
                     f"version={version if version else 'not probed'}")
    lines.append("")
    lines.append(STALE_PAIRING_PROVENANCE)
    lines.append("")
    lines.append("Kill facts (stated exactly):")
    lines.append("  " + NUEWFRAME_KILL_FACTS)
    lines.append("")
    lines.append("Floor-based interpretation (the headline; two "
                 "context-labelled distances, no raw median race):")
    for context in CONTEXTS:
        decomp = decompose(records, pricing, context)
        lines.append(f"  [{context}]")
        if decomp.get("status"):
            lines.append(f"    {decomp['status']}")
        else:
            lines.append(f"    floor: {decomp['floor_ms']:.3f} ms "
                         f"(agreement of the minimal tools: {decomp['floor_agreed']})")
            lines.append(f"    guard fast-path median: {decomp['guard_fast_median_ms']:.3f} ms")
            lines.append(f"    distance above floor: {decomp['distance_above_floor_ms']:.3f} ms")
            lines.append(f"    census (counted, not timed): plain path "
                         f"{decomp['census']['plain_total']} named calls, hook path "
                         f"{decomp['census']['hook_total']}")
            for name, item in decomp["items"].items():
                if item["ms"] is not None:
                    price = f"{item['ms']:.3f} ms"
                    if item.get("trace"):
                        price += f" ({item['trace']})"
                else:
                    price = item["tier"]
                line = f"      - {name}: {price} [{item['tier']}]"
                for flag in item.get("inconsistencies", []):
                    line += f" — INCONSISTENCY: {flag}"
                lines.append(line)
            for reason in decomp.get("inconsistencies", []):
                if "parts do not sum" in reason:
                    lines.append(f"      - {reason}")
            if decomp["unexplained_ms"] is not None:
                lines.append(f"      - unexplained remainder: {decomp['unexplained_ms']:.3f} ms")
    lines.append("")
    lines.append("Per-arm medians by context (for inspection only):")
    for context in CONTEXTS:
        lines.append(f"  [{context}]")
        for path in ("fast", "kill"):
            parts = []
            for arm in PRODUCT_ARMS:
                med = _median(records, arm, path, context)
                text = f"{med:.3f} ms" if med is not None else "no successful slots"
                parts.append(f"{arm}: {text}")
            lines.append(f"    {path}: " + "; ".join(parts))
    lines.append("")
    lines.append("Freeze gate (SPEC freeze rule: win or tie on each "
                 "of the five gate dimensions, per context):")
    for entry in freeze_gate(records, expected_output, config):
        lines.append(f"  [{entry['context']}] {entry['dimension']}: "
                     f"{entry['outcome']} — {entry['detail']}")
    lines.append("")
    lines.append("Evaluation profile (SPEC_Benchmark section 3: win "
                 "or tie on every applicable dimension — the full "
                 "enumeration of all eight dimensions, with a state "
                 "per dimension):")
    for index, entry in enumerate(
            profile_dimension_states(records, decisions,
                                     expected_output, config), 1):
        lines.append(f"  ({index}) {entry['dimension']}: "
                     f"{entry['state']} — {entry['detail']}")
    lines.append("  While any profile dimension is UNEVALUATED the "
                 "campaign is INCOMPLETE (never a certified result), "
                 "even if the observed dimensions pass; a dimension "
                 "renders NOT-APPLICABLE only via a decision in the "
                 "run's configuration (campaign.json's "
                 "'profile_dimension_decisions', the configuration "
                 "whose SHA-256 run.json records), whose reason "
                 "prints beside it above.")
    lines.append("")
    lines.append("Claim ledger (SPEC_Benchmark section 11: every "
                 "profile dimension, its state, and the basis "
                 "inside this run package for that state):")
    for index, entry in enumerate(
            claim_ledger(records, decisions, expected_output,
                         config), 1):
        lines.append(f"  ({index}) {entry['dimension']}: "
                     f"{entry['state']} — basis: {entry['basis']}")
    lines.append("")
    if competitor:
        lines.append("Competitor losses (evidence: competitor kill "
                     "slots that left a descendant alive, competitor "
                     "fast slots whose output did not match the "
                     "expected bytes, and competitor measured slots "
                     "whose exit code did not match the expected "
                     "code — the guard's wins on profile dimension "
                     "(1); not campaign standing losses):")
        for loss in competitor:
            lines.append(f"  - {loss}")
        lines.append("")
    if complete:
        lines.append("STATUS: complete campaign; the freeze gate is met "
                     "on every gate dimension in every context and no "
                     "evaluation-profile dimension is UNEVALUATED.")
    else:
        lines.append("STATUS: INCOMPLETE CAMPAIGN — not a result, not a freeze.")
        lines.append("Standing losses:")
        for loss in campaign_standing:
            lines.append(f"  - {loss}")
    return "\n".join(lines) + "\n"
