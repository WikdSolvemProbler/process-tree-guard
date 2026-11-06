<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# Verification record — what was exercised here, and at what tier

Development machine: Linux (no Windows, no MSVC). Everything below
was run under finite deadlines. Raw outputs are in `evidence/`.

## Contents

- [Linux-proven](#linux-proven)
- [Claim ledger and the six profile-dimension decisions (round-8 P7 landing; round-26 collections landing)](#claim-ledger-and-the-six-profile-dimension-decisions-round-8-p7-landing-round-26-collections-landing)
- [Windows-theory (by construction, not yet executed anywhere)](#windows-theory-by-construction-not-yet-executed-anywhere)
- [Not verified anywhere yet](#not-verified-anywhere-yet)

[↑ Back to Contents](#contents)

## Linux-proven

1. **Test families** — `python3 run_tests.py`: 647 tests counted once,
   593 run and pass, 54 skipped (the guard-driving families;
   skip reason: binaries not built here), 0 failures, 0 errors.
   `evidence/test-run.txt` holds the summary.
   The guard-facing vs harness-facing split, stated explicitly
   (round-26 F5, answering sanity check p4-5's D2): the
   classification rule is by file — harness-facing is
   `tests/test_campaign_logic.py` (the campaign harness's
   self-consistency); guard-facing is the four files that drive
   or observe the guard (`tests/test_guard_families.py`,
   `tests/test_coverage_families.py`,
   `tests/test_output_observation.py`,
   `tests/test_scanner_differential.py`); "including battery"
   adds `tests/test_battery_cases.py`, whose cases are guard
   failure-tree branches judged through the suite's own decision
   procedures. At round 26 the executed census was 303 campaign-logic / 90 guard-facing / 159 including battery, of 462 counted once at round 26 — the balance the sanity checker measured, with the harness-facing file holding the majority; the completeness-accounting landing states the split it landed: 487 campaign-logic / 91 guard-facing / 160 including battery, of 647 counted once. The same census stated BY SUBJECT (round-31 F3's construction, rebuilt per-test by round-32 F3 beside the by-file statement): the classification rule by subject is the behaviour a test pins — a test is guard-facing by subject when the behaviour it pins is a verdict the campaign renders about an arm on a guard-facing dimension, an arm-behaviour family, or the battery; it is harness by subject when what it pins is the instrument's own discipline (package shape, census and documents, pricing provenance/stamps, admission, run namespaces, build/run scripts, record-shape naming as such). By file, the four guard files and the battery are guard-facing by subject entire; inside `tests/test_campaign_logic.py` the classification is PER TEST, by an explicit, reviewable enumeration in the census test itself (round-32 F3): every guard-facing test in that file is named there by Class.test_name as the per-test application of the rule above, and every other campaign-logic test is harness by subject — no class is counted wholesale, after round 32's review showed the wholesale proxy classifying at least four tests against the rule's own examples (the glossary test and this census test itself, which is harness by subject — documents / census; the two round-30 pricing tests, harness — pricing; and, in the other direction, Round21NeighbourTests' guard-kill-path-correctness case, guard-facing). The completeness-accounting landing states the by-subject split it landed: 312 guard-facing / 335 harness, of 647 counted once (91 guard files + 69 battery + 152 per-test-classified tests inside test_campaign_logic.py). The two statements measure different things and both are true under their stated rules: by file the harness-facing file holds the majority, by subject the guard-facing share is 312 of 647 — 48%, just under half the suite, stated at its measured figure (the earlier characterisation of that share is retired by the round-51 re-judgment, which removed 34 materialization / settlement / write-back names from the per-test enumeration under the rule above) — the rounds 26–31 dimension programme grew inside the campaign-logic file, so the by-file statement understates the guard-facing share by the 152 per-test-classified tests inside that file, and neither statement may be read for the other. Both are held by `Round26LandingTests.test_split_statement_in_verification_matches_the_census`, which recomputes both from the test sources. The trend across the round-26 landing: the guard-facing share grew by the round's one guard-driving addition (the .bat family), the harness-facing share by the round's sixteen campaign-logic tests (the collection machinery for dimensions (3), (4), (6) and (8) is harness logic, and its tests land where the logic lives; the sixteenth is the closing run's own finding — a scale child killed before it published its tree is an infrastructure failure, never an "ok" with an empty tree); across the round-27 landing the harness-facing share grew by the round's twelve campaign-logic tests (the five holds — the scale count, the computed usability state, the per-arm resource state, the malformed exit code, the aligned deep chain — are harness logic, and their tests land where the logic lives) and the guard-facing share did not move; across the round-28 landing the harness-facing share grew by the round's eight campaign-logic tests (the four tightened holds — the stated usability rule, the distinct-PID scale count, the zero-peak resource rule, the resource presence hold — are harness logic, and their tests land where the logic lives) and the guard-facing share did not move; across the round-29 landing the harness-facing share grew by the round's nine campaign-logic tests (the six fixes — the three-valued usability state, the integer-typed peak hold, the key-presence hold, the variant-aware distinct-PID count, the structural rule agreement, the total numeric holds — are harness logic, and their tests land where the logic lives) and the guard-facing share did not move; across the round-30 landing the harness-facing share grew by the round's seven campaign-logic tests (the two fixes — the one shared scale-count predicate in the runner and the checker, and totality completed at the pricing producers, the pricing-evidence rollup, the median aggregations and the slot accounting — are harness logic, and their tests land where the logic lives) and the guard-facing share did not move; across the round-31 landing the harness-facing share by file grew by the round's eight campaign-logic tests (the two verdict constructions — dimension (3)'s arm comparison and dimension (6)'s designated pinned expectation — are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject all eight are guard-facing dimension-programme tests, the divergence the by-subject statement above now records beside this one; across the round-32 landing the harness-facing share by file grew by the round's ten campaign-logic tests (the rebuilt (3) comparison, the slot-bound (6) pin, and the per-test census, glossary and (5)/(7)-statement holds are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject seven of the ten are guard-facing (the F1/F2 verdict regressions) and three are harness (the census, glossary and statement holds — documents and census by the rule's own examples); across the round-33 landing the harness-facing share by file grew by the round's eleven campaign-logic tests (the scale-path variant hold, the malformed argv content hold, the cardinality/floor/maximum rebuild of (3), the unpaired rendering, and the STAMP-edge and pin-reach statement holds are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject eight of the eleven are guard-facing (the F1/F2/F3/F4 verdict regressions) and three are harness (the presentation-agreement, STAMP-edge and pin-reach statement holds), and three round-26 plan/template construction tests were reclassified harness by subject (round-33 F5 — plan malformed cells, plan scaling cells, malformed argv presentation: construction as such, no verdict), moving three from the guard-facing side of the by-subject split with no test added or removed for it; across the round-34 landing the harness-facing share by file grew by the round's eight campaign-logic tests (the sorted-rank-dominance rebuild of (3) with its compared-unit rendering, the verdict-layer rep hold on both variant paths, and the argv exclusivity conjunct are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject all eight are guard-facing (the F1/F2/F3/F4 verdict regressions), and one round-33 test was renamed with its assertion corrected under the same fix (the peak-total regression, now the sorted-rank-dominance peak test), moving no count; across the round-35 landing the harness-facing share by file grew by the round's seven campaign-logic tests (the censoring vocabulary and imputation for (3), the verdict-layer rep hold over every record kind, the argv exactness limb, and the loss-magnitude entry are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject all seven are guard-facing (the F1/F2/F3/F4 verdict regressions), and one round-34 test was renamed with its assertion corrected under the same fix (the exactly-half PASS regression, now the exactly-half censored regression), moving no count; across the round-36 landing the harness-facing share by file grew by the round's five campaign-logic tests (the plan-driven unit enumeration and the per-field note accounting for (3), the plan-anchored argv length budget, and the accounted-only censored classification are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject all five are guard-facing (the F1/F2/F3/F4 verdict regressions), moving no other count; across the round-37 landing the harness-facing share by file grew by the round's five campaign-logic tests (the build_plan-anchored census for (3), the underived argv-budget naming for (6), and the degraded-list completion are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject four of the five are guard-facing (the F1/F2 verdict regressions) and one is harness (the F3 degraded-set regression, package naming discipline), and one round-18 package-layer test was renamed with its assertions corrected under the same fix (the output-evidence control, now the underived-budget naming at the package layer), moving no count; across the round-38 landing the harness-facing share by file grew by the round's seven campaign-logic tests (the plan-bound coverage floor for (3), the sentinel cap for (6), the derivation-read budget-gap fact, and the refresh-disclosure hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject six of the seven are guard-facing (the F1/F2/F3 verdict regressions) and one is harness (the F4 disclosure hold — documents and census), moving no other count; across the round-39 landing the harness-facing share by file grew by the round's five campaign-logic tests (the per-limb argv derivation for (6), the no-fallback UNPLANNED gate for (3), and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject four of the five are guard-facing (the F1/F2 verdict regressions) and one is harness (the restatement hold — documents and census), and one round-38 test's program segment was restated under F1 with its assertions corrected (a non-string program now derives the token-count length, where the round-38 form named it present-but-unusable), moving no other count; across the round-40 landing the harness-facing share by file grew by the round's five campaign-logic tests (the slot-derived gate keying and per-(cell, arm) plan-0 rule for (3), the planned-population exclusion for the evidence consumers, the construction-tokenization derivation for (6), and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject four of the five are guard-facing (the F1/F2/F3/F4 verdict regressions) and one is harness (the restatement hold — documents and census), and two earlier tests' segments were restated under the fixes with their assertions corrected (the round-38 wrong-type test's string-template and string-args segments, which now derive under F4; the round-39 planned-resolution test's cost-cell construction, whose records now carry the empty-command cell's own reps under F1), moving no other count; across the round-41 landing the harness-facing share by file grew by the round's seven campaign-logic tests (the multiplicity-exact planned population, the freeze gate's planned-population binding, arm identity against the record's second encoding, the one-shot-iterator materialization, the variant-absence naming, the own-name unplanned-fault listings, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject six of the seven are guard-facing (the F1/F2/F3/F4/F5/F6 verdict regressions) and one is harness (the restatement hold — documents and census), moving no other count; across the round-42 landing the harness-facing share by file grew by the round's five campaign-logic tests (the path-semantics arm identity, the kind-complete own-name listings, the mutation-refusing materialization, the confined write-back, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject four of the five are guard-facing (the F1/F2/F3/F4 verdict regressions) and one is harness (the restatement hold — documents and census), moving no other count; across the round-43 landing the harness-facing share by file grew by the round's five campaign-logic tests (the canonical-sequence arm identity, the path-complete own-name listings, the pricing-inclusive shared materialization, the one-shot-ness materialization, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject four of the five are guard-facing (the F1/F2/F3/F4 verdict regressions) and one is harness (the restatement hold — documents and census), moving no other count; across the round-44 landing the harness-facing share by file grew by the round's five campaign-logic tests (the sole-component-confined canonical strip, the admission-bound shared materialization, the any-Mapping slot cache, the proven-one-shot write-back, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject three of the five are guard-facing (the F1/F3/F4 verdict regressions) and two are harness (the F2 admission regression — admission by the rule's own examples — and the restatement hold — documents and census), moving no other count; across the round-45 landing the harness-facing share by file grew by the round's six campaign-logic tests (the materialization-discipline version-args choice, the widened factory signature, the restart-proof write-back limbs, the materialized-evidence classification, the single shared-read semantics, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject four of the six are guard-facing (the F2/F3/F4/F5 verdict regressions) and two are harness (the F1 version-args regression — the probe chain's own discipline, on the round-44 F2 precedent — and the restatement hold — documents and census), and one round-44 test's assertion was corrected under F5 with its construction unmoved (the F4 regression's runner-after-append expectation, which stated the round-44 split's local-read semantics round 45 abolished; the construction now states the single shared read's basis), moving no other count; across the round-46 landing the harness-facing share by file grew by the round's five campaign-logic tests (the materialization-access slot serve, the storage/mint identity distinction, the slot route's capture discipline, the capture-derived classification, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject four of the five are guard-facing (the F1/F2/F3/F4 verdict regressions) and one is harness (the restatement hold — documents and census), moving no other count; across the round-47 landing the harness-facing share by file grew by the round's six campaign-logic tests (the read-unit slot classification, the copying store's re-read replacement, the relation-flip serve, the eager mint's first read, the absorbed raising classification, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject five of the six are guard-facing (the F1/F2/F3 verdict regressions) and one is harness (the restatement hold — documents and census), moving no other count; across the round-48 landing the harness-facing share by file grew by the round's nine campaign-logic tests (the deferred slot verdict with its abolished confirmation access, the one-breadth fresh-traversal exclusion, the discarded-access-free derivation, the one-way presence judgement, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject eight of the nine are guard-facing (the F1/F2/F3/F4 verdict regressions) and one is harness (the restatement hold — documents and census), moving no other count; across the round-49 landing the harness-facing share by file grew by the round's five campaign-logic tests (the empty-value slot settlement, the minting settlement's re-opening and flag retirement, the capture-confined TypeError translation, the no-pending write-back gate, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject four of the five are guard-facing (the F1/F2/F3/F4 verdict regressions) and one is harness (the restatement hold — documents and census), moving no other count; across the round-50 landing the harness-facing share by file grew by the round's three campaign-logic tests (the settled-minting empty-persistence hold, the read-found write-back gate, and the VERIFICATION restatement hold are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject two of the three are guard-facing (the F1/F2 verdict regressions) and one is harness (the restatement hold — documents and census), moving no other count; across the round-51 landing the harness-facing share by file grew by the round's two campaign-logic tests (the fallthrough-carried marker regressions are campaign logic, and their tests land where the logic lives) and the guard-facing share by file did not move, while by subject both new tests are harness (the F1 regressions — materialization / settlement discipline by the rule's own terms, on the round-51 F3 re-judgment), and that re-judgment removed 34 pre-existing names from the per-test enumeration (the round 41–50 materialization / one-shot / write-back / settlement / candidate tests, listed name-by-name in the landing log), moving 34 from the guard-facing side of the by-subject split to the harness side with no test added or removed for it; across the completeness-accounting landing the harness-facing share by file grew by the landing's one campaign-logic test (the resource-note vocabulary/write-sites tie — the verdict layer's closed collection-failure vocabulary is now compared mechanically, by AST, against the runner's note-generation write sites) and the guard-facing share by file did not move, while by subject the new test is harness (the instrument's own discipline, by the rule's own examples), moving no other count; across the round-53 landing the harness-facing share by file grew by the landing's one campaign-logic test (the compile-all test — every Python file in the package compiled under py_compile as a suite test, the cure for finding F-Acct-2) and the guard-facing share by file did not move, while by subject the new test is harness (the instrument's own discipline, by the rule's own examples), and the landing's other fixes (the spawn-race relabel, the completed Part-3 enumeration, the D2 class re-judgments, the bidirectional tie, the glossary definitions) added no test, moving no other count; across the round-60 landing the harness-facing share by file grew by the landing's one campaign-logic test (the VERIFICATION item-6 pricing-figures hold — the item-6 parenthetical's medians are held against the preserved package's own recomputation, the cure for the false figure that stood there) and the guard-facing share by file did not move, while by subject the new test is harness (documents / census, by the rule's own examples), moving no other count; the split is held by `Round26LandingTests.test_split_statement_in_verification_matches_the_census`, which recomputes it from the test sources. Load disclosure
   (round-14): two earlier full runs of the 311-test suite on
   this shared box, under extreme load (load average ~19 on
   2 cores), failed the two stub-tier KillTreeTests kill slots
   with infra-failure — the stub kill chain is three Python
   interpreter startups deep (harness, arm, tree child) inside
   the arm's 2 s kill deadline, and under that load the tree
   child's marker publication lost the race; the same tests
   passed in isolation, as a module, and after the battery
   module, and the final run above passed in full.
   The shipped Windows kill child is the native testkit, whose
   startup does not carry this race at stub tier's depth; the
   stub-tier kill slots remain load-sensitive on a saturated
   Linux box, and a KillTree infra-failure there under extreme
   load is that sensitivity, not a guard or classification
   defect — re-run before treating it as one. Among the tests is
   the census-consistency test: it reads the counted-once totals
   stated in `README.md`, this file and `FAILURE-TREE.md`, in
   the canonical phrasing "N tests counted once", and fails if
   they disagree with `run_tests.py --count-only`; it also
   holds the recorded summary in `evidence/test-run.txt` to
   the same count, so the evidence file the first line of this
   item cites is part of the census set and is refreshed by
   every landing that moves the total. Among the
   tests are the counted-ops census tests in item 6: they
   recount the counted-ops census from the vendored enumeration
   of MEASUREMENT §2's named calls in `campaign/census.json`
   and hold `report.py`'s CENSUS and the census totals printed
   in `README.md` and `FAILURE-TREE.md` §1 to that recount.
   (Count revised when
   the environment-case landing added eight tests: the shared
   case-insensitive setting rule's variant-collapse and
   no-variant controls, the resolver pricing probe's shape under
   `Path`-spelled and dual-spelling parent environments and its
   recorded intended effective PATH, and the F10 child-environment
   observations; when
   the round-4 battery landings added eleven tests: the
   noise-honesty, measured-held-bytes, verdict-value-pairing,
   unfiltered-enumeration, ASCII-matcher, inheritance-record and
   confirmed-teardown controls in item 4; when
   the round-3 battery-probe landings added fourteen tests: the
   computed-from-observations controls in item 4; when
   the round-3 coverage landings added thirty-nine tests: the
   scanner differential's reference, corpus and negative-control
   tests and its two Windows-tier comparison tests, and the
   coverage families for the payload cap, the context and
   rewriting vendors, non-Unicode arguments and resolver order —
   see item 7; and when
   the Phase-4 round-2 campaign-core landings added fourteen
   anywhere-tier tests: the tolerance derivation, both floor arms
   as real spawns, the tree kill path and probe, per-context
   medians, and exact slot accounting with exit validation; when
   the battery landings added fourteen more: the function-scoped
   audit controls and the driver-tier controls in item 4; and
   when the harness-integrity landings added sixteen more: the
   pricing-probe and provenance controls, seeded order and its
   tamper checker, run namespacing, competitor identity, and the
   build script's mechanical checks in item 6; and when the
   sanity-check-p4-1 landings added nine more, after the
   round-5 landings had taken the runner's total to 227 while
   the documents still stated the earlier figure: the
   census-consistency test itself, the three competitor-pin
   tests and the build script's pin-verification test in
   item 6, and the four evaluation-profile tests in item 6;
   and when the round-6 landings added two more: the
   evaluation-profile decision-reason control and the build
   script's preserved-bundle default control in item 6; and
   when the round-7 landing added one more: the harness-fallback
   control that strips the probe-evidence claim keys from
   configuration pricing before it can reach a report, in
   item 6's pricing family; and when the round-7
   handle-ownership landing added one more: the F9
   pipe-ownership source test, which pins exactly-once close
   ownership for every pipe end in the raw-argv helper; and
   when the round-8 claim-ledger landing added six more: the
   shipped-decisions source control, the perfect-fixture
   completeness proof with the shipped decisions in force and
   the exactly-six-UNEVALUATED-losses control without them, the
   harness claim-ledger writing control, and the two
   ledger-verification controls (a ledger claiming an
   unrecorded decision, and a ledger state the records
   contradict, each named as a package defect) in item 6; and
   when the round-8 pricing landings added four more: the
   owner-verification one-level construction test (the assembled
   shapes insert exactly 0 and 1 levels, the configured launcher
   no longer wrapping the recording launcher) and the resolver
   probe's whole-field controls (the controlled-field shape, the
   spawn-seam capture proving the pinned environment and scratch
   working directory reach the child spawn, and the harness-cwd
   decoy control), in item 6's pricing family; and when the
   round-8 tree-consistency landing added four more: the build
   script's tree-identity build-record control and the run
   script's -Tree defaulting, --tree battery plumbing and
   pre-campaign mismatch-refusal controls in item 6; and
   when the round-9 reduced-shape fence landing added three
   more: the full-size reduced-outcome control, the small-N
   reduced-shape control and the reduced-package re-render
   control in item 6; and when the round-10 landings added six
   more: the driver exit-code control and the
   driver-verdicts-after-a-bad-exit control in item 4, the
   BOM staging-record control, the build script's
   version-probe preference control, and the run script's
   campaign-abort stage-record and test-stage preference
   controls in item 6; and when the round-10 scanner-hash
   landing added one more: the corpus payload-hash control
   in item 7, which recomputes the summary's payload
   SHA-256 from the regenerated corpus by the serialization
   stated beside the field in the summary; and when the
   counted-ops census landing added three more: the
   vendored-enumeration recount, the guard-tree re-derivation
   and the document census-total controls in item 6; and when
   the round-11 record-consistency landings added eight more:
   the unmodified-fixture consistency control, the four
   record-consistency variant controls (an exit code, a
   survivor flag, a missing elapsed time and a missing tree
   identity, each contradicting its record's ok classification
   and named as a standing loss with its gate dimension not
   passing), the package-checker record-consistency control
   with its clean-namespace control, and the two
   competitor-loss kind controls (warmup and control kinds
   minting no competitor loss; exactly one measured competitor
   loss) in item 6; and when
   the round-11 output-observation landing added ten more: the
   stub-tier output capture controls (a matching relay fast
   slot recorded with per-stream digests, lengths and match
   verdicts; the mute arm — the expected exit code with both
   streams suppressed — and the garble arm — stdout altered,
   stderr intact — each classified `output-mismatch`, never
   "ok"; kill slots carrying no output observation; the floor
   slots' observation, floor-direct judged against the same
   expected bytes and floor-spawn digested without a verdict)
   and the output-gate controls (the perfect fixture's
   dimension (1) claim resting on output correctness beside
   kill-path and exit-code correctness, a guard output
   mismatch FAILing that gate dimension and the claim, a
   fixture stripped of output observations leaving the
   dimension UNEVALUATED and the campaign incomplete, a
   competitor output mismatch listed as competitor-loss
   evidence that does not block a complete campaign, and the
   shipped configuration's output-workload control) in
   item 6; and when the round-12 record-consistency landing
   added five more: the negative-label controls in item 6 —
   an "ok" guard fast record whose own stream verdict is
   False, a descendant-survived label without survivor
   evidence (minting no competitor loss and no kill-latency
   exclusion, with both kill dimensions UNEVALUATED), an
   exit-mismatch label carrying the expected exit code, an
   output-mismatch label with no False verdict (minting
   nothing), and a match verdict recorded without the digest
   and length that evidence it; and when the round-12
   pricing-stamp landing added seven more: the pricing stamp
   controls in item 6 — the genuine helper records and the
   genuine stub-tier probe records each carrying the
   probe-shape stamps and pricing all three items, the
   stamp-erased variants pricing nothing (all three stamps
   erased) or withholding only the erased item, the stamp
   defects named by item and field for every stamped field,
   the stamp-erased namespace rendering the affected item
   not measured, and the preserved reduced-run package's
   stamps verified genuine; and when the round-12
   output-passthrough landing added five more: the
   anywhere-tier assertion controls in
   `tests/test_guard_families.py` for the exact-output
   assertion the F1 Windows test applies (the fixture's
   construction, exact captures passing, and suppressed
   stderr, reversed stdout lines, and truncated or altered
   bytes each failing it); and when the round-13 landings
   added nine more: the shipped-kill-shape completeness
   control and the survivor-precedence controls in item 6,
   the competitor exit-mismatch evidence control in item 6,
   the two resolver stamp-structure controls in item 6, and
   the three recorded-outcome controls in item 6; and when
   the round-14 landings added seven more: the
   corrupt-pricing-file, finalised-slots_run, forged-report-
   status and forged-identity-verdict controls in item 6, the
   resolver intended-path structure control in item 6, the
   newest-namespace resolution control in item 6, and the
   driver build-directories control in item 1's families; and
   when the round-14 P8 landing added one more: the
   vendored competitor-bundle control in item 6, pinning every
   binary vendored under `benchmark/competitors-001/bins` to
   its SHA-256 pin in `campaign.json`; and when the round-15
   landings added six more: the malformed-package controls in
   item 6 — the observation and pricing record-shape controls,
   the forged plan_counts / slots_planned control, and the
   forged ledger-basis control; and when the round-16
   landings added ten more: the malformed-package controls in
   item 6 — the manifest, ledger and identity shape controls
   and the plausible-elapsed and unknown-outcome controls;
   and when the round-17 landings added nine more: the
   pricing probe/shape and pairing controls, the cmd_report
   shape-refusal controls and the manifest float-count
   control in item 6, the malformed identity-container
   control in item 6, and the non-boolean output-match and
   float exit-code record-consistency controls in item 6;
   and when the round-18 adjudication landings added sixteen
   more: the torn-package report-path controls, the
   float-equal pricing-stamp controls, the package-layer
   output-evidence controls and their manifest-binding
   control, the rollup pricing-evidence admission controls
   and the configuration container controls in item 6; and
   when the round-19 adjudication landings added eleven
   more: the floor-control elapsed controls, the vanished-
   floor standing-loss control and the package-layer floor
   control in item 6, the out-of-range tree-identity PID
   controls (grandchild and present-child forms, with their
   package-layer naming) in item 6, the pricing sign-count
   derivation controls (derived counts governing, stated-
   count disagreement named, string counts never reaching
   the flag text, non-integer counts without differences
   resolving to "not measured") in item 6, and the
   pricing_probes container controls on the loader and
   admission surfaces in item 6; and when the round-21
   adjudication landings added seventeen more: the
   outcome×path pairing controls (a kill record labelled
   `output-mismatch` and a fast record labelled
   `descendant-survived`, each named and completing nothing),
   the survivor-under-every-kill-label controls (a competitor
   exit-mismatch kill record with survivor True named, and the
   guard's kill-path correctness FAILing over the same shape),
   the survived exit-code-presence control (survivor records
   with no observed exit code named and minting nothing, the
   genuine-code control minting exactly as before), the
   pricing context-stamp controls for the owner-verification
   and resolver pairs, the pricing items-container control
   (a non-object items resolving every item to "not measured",
   never a traceback) and the configuration field-level
   controls (the survivor_checker, seed, per-template-value
   and expected-output fields on the loader and admission
   surfaces, with their reduced-run named refusals) in
   item 6, the recorded test-run evidence census control in
   item 6, and the recorded battery-evidence freshness
   control in item 4; and when the round-22 adjudication
   landings added sixteen more: the output-mismatch
   exit-code-presence control, the per-path field-discipline
   controls (a fast record's survivor value, a kill record's
   output fields), the configuration element/token-level
   controls (template and child-args elements, survivor
   checker elements, version_args, guard_launcher and
   expected_exit values, with the two expected_exit tables'
   divergence named and pinned), the pricing stamp-vector
   controls (the resolver baseline's intended_path and
   foreign stamps on every probe), the scanner
   corpus-summary field controls in item 7, the VERIFICATION
   §4 prose-coordinate census control in item 4, and the
   recorded execute/skip split census control in item 6;
   and when the round-23 adjudication landings added
   seventeen more: the owner-timing gate repair (45 pooled,
   interleaved paired repetitions and a bound derived from
   the measured dispersion at four standard errors of a
   median difference, taken as the larger of the pair-level
   and set-level estimates — the set level paired in
   round 24, and the whole bound re-estimated in the
   round-25 sensitivity repair from the plain condition
   only, as item 6 states) with
   its in-bracket delay sensitivity control in item 6,
   rebuilt in round 24 on the landed construction itself
   with the mover at the gate's demonstrated 250 ms
   detection floor (stated, as the gate states it, for the
   sampler-verified quiet condition), the
   tree-identity field-pairing controls (a
   fast record's never-written fields held null, a kill
   record's tree_marker held present), the pricing
   classification and exact-field-set controls (an ok record's
   exit code, each record's item, the loaded-only fields on a
   baseline record), the configuration scalar path/input
   controls (children stdin, arm executable and child program
   held as strings on the loader and admission surfaces), the
   run package's beside-field controls (run_id, environment,
   config_templates and the report body held to the run's own
   configuration and anchor fingerprint), the scanner
   corpus-summary seed control in item 7, the §4 prose
   subject-and-duration census control in item 4, the
   documents' scanner-count and campaign-figure census
   control, the item-3 figure census control, the
   run-manifest provenance control and the FAILURE-TREE
   citation census control in item 6.) Count revised when
   the round-24 landings added ten more: the neighbour-holds
   completion (`Round24NeighbourTests` — the kill record's
   arm_pid held to PID form, the owner-loaded bracket values
   held as values, the staging-record path held on both
   config surfaces with its named refusal, and the run
   package's beside-fields completed with the unverifiable
   field named rather than passed); the census controls
   extended, not added — this document's own copies of the
   held quantities, item 3's beside-figures and FAILURE-
   TREE's prefixed in-tree citations join their held sets;
   and the owner-timing sensitivity control was rebuilt on
   the landed gate construction with its detection floor
   stated in item 6 (250 ms — the smallest in-bracket mover
   the landed gate caught in every repeated construction,
   stated for the sampler-verified quiet condition; a
   200 ms mover was not caught, and no sensitivity below
   the floor is claimed). Count revised when the round-25
   landings added eleven more: the neighbour-holds
   completion (`Round25NeighbourTests` — the package
   checker's naming duty completed generally over every
   verification the absent configuration anchor degrades,
   `arm_pid` and `argv` held at the breadth the runner
   writes them with the perfect fixture aligned to
   construction truth on both fields, the pricing records'
   `guard_fingerprint` companion values held in form and
   to the manifest, the environment hold made positional,
   and report.txt's verdict-bearing sections held to the
   run's own recomputed outcomes); the census controls
   extended, not added, apart from one — VERIFICATION item
   6's counted-ops census copy, the tree's 437 count in
   README and FAILURE-TREE, VERIFICATION's theory-section
   53 and 14,400 s copies join their held sets, FAILURE-
   TREE §6's bare in-tree fn citations join the citation
   census, and the floor-citation census control in item 6
   holds VERIFICATION's 250 ms citations to the gate's
   stated floor with the quiet-condition qualification
   carried where they stand. Count revised when the
   round-25 sensitivity repair added one more: the
   plain-condition bound control
   (`OwnerVerificationTimingTests.test_gate_bound_is_estimated_from_the_plain_condition_only`),
   pinning the repaired gate's bound to the plain
   condition's dispersion alone, after the floor was
   refuted at its stated quiet condition and the gate's
   bound construction was repaired as item 6 states.
2. **Campaign logic against stub arms** — the stub fast slot exits
   0 and its output is observed: with the relay fast child in
   force, the slot's record carries both streams' SHA-256
   digests, lengths and match verdicts against the expected
   bytes the configuration names, and the mute and garble stub
   arms (the expected exit code with the child's output
   suppressed, and with its stdout altered) are each classified
   `output-mismatch`, never "ok"; the stub kill slot runs a tree-shaped child (the child spawns
   a grandchild that records its PID) and a tree-killing stub arm
   exits 124 at its 2 s deadline with no survivor; a stub arm that
   kills only the direct child is classified
   `descendant-survived`; a kill slot with the survivor probe
   removed is classified `infra-failure`, never "ok"; a stub arm
   exiting 125 on a fast slot is classified `exit-mismatch` and
   excluded from medians; and a missing arm binary is classified
   `infra-failure`.
3. **Reduced harness run** — `harness.py run --reduced 40` against
   stub arms completed and wrote `observations.jsonl`,
   `pricing-observations.jsonl`, `claim-ledger.json`, `run.json`
   and `report.txt` into
   its own run namespace (`evidence/reduced-run/` holds that run
   package). Its report is labelled a reduced logic exercise and
   INCOMPLETE, as required. Correction of an earlier misdiagnosis
   in this file: the reduced run's original floor disagreement
   was structural, not "Python process startup dominates both
   arms" — one floor arm then ran no process at all (its elapsed
   time was two clock reads around a dictionary write,
   ~0.001–0.005 ms against the other arm's real spawn), so
   agreement was impossible by construction. Both floor arms are
   now real spawns of distinct minimal programs. In the current
   regenerated run (round-32 landing: regenerated under the
   round-32 holds — the preserved round-27 package's one guard
   malformed warmup, a nonexistent-program slot its round-26-era
   runner recorded "ok" at exit 1, recomputed under round-32
   F2's warmup-inclusive pin as a record inconsistency; in the
   regenerated run the runner itself classifies that slot
   exit-mismatch, and it stands in the enumeration below as the
   exit fault standing_losses assigns to a warmup slot) the NO_WINDOW
   floor pair agrees (medians 52.1 ms and 50.8 ms), and the DETACHED pair
   agrees (medians 51.2 ms and 51.9 ms); the report
   states no distance anywhere, because the reduced run contains
   no measured guard slots in either context. The standing losses
   name every missing cell group (the thirty-two measured groups and
   the thirty-two warmup groups the 40 slots did not reach), the
   guard malformed warmup's exit mismatch, every unevaluated freeze-gate
   dimension in both contexts (five dimensions, output correctness
   among them), the six UNEVALUATED evaluation-profile dimensions
   and the reduced shape itself; the report's Evaluation profile
   section enumerates all eight
   SPEC_Benchmark §3 dimensions — dimensions (1), (2), (3), (4),
   (6) and (8) UNEVALUATED in this run (the collected dimensions
   render UNEVALUATED on a run whose records do not carry their
   collection — for (8), while its named artifact records no
   prompt class exercised with an observed outcome, round-27
   F2), dimensions (5) and (7) NOT-APPLICABLE
   under the shipped configuration decisions, the entries printing
   beside them — and its Claim ledger section renders one claim
   per dimension on the same basis. The run's pricing probes
   executed all
   18 paired stub-tier observations without a failure and priced
   all three expensive-class items from them (stub-tier medians
   -1.1 ms, -12.0 ms and -0.0 ms), each price carrying its
   paired differences' dispersion and sign counts; stub-tier
   timings prove the plumbing only. Its `run.json` records seed
   20261029, the 40-slot execution order, the environment, the
   configuration's SHA-256, the profile-dimension decisions and
   the claim ledger beside that hash, and the campaign outcome
   (`campaign_status: incomplete`, 82 standing losses, no
   freeze-gate FAIL), and the harness's own package checker
   verifies the package in full apart from the verifications
   its absent configuration anchor degrades: its
   `config_path` names a scratch file deleted with the run's
   temporary directory and the package carries no
   configuration content of its own, so every verification
   that anchor would have grounded is unverifiable from
   this package — the recorded `config_templates` (config
   content), competitor identity, the expected-output
   pairing, the profile decisions, and the argv length
   budget (round-37 F3: the absent anchor degrades that
   budget too — the package footing derives no fast-vector
   length for any arm under the NO_VERIFIED_PLAN sentinel,
   so dimension (6)'s length limb does not fire for this
   package's records) — and
   `check_run_package` returns exactly those five problems,
   each naming its verification as unverifiable, rather
   than passing any of them silently or falling back to
   comparing the package against itself (round-25: the
   naming duty is general — round 24 named only
   `config_templates` while identity fell back to the
   manifest's own recorded list, the pairing fell back to
   digest form only, and decisions were sourced from
   `run.json` itself). That is a tier-labelled non-execution
   for this package (the anchored holds are stated at this
   breadth here and in FAILURE-TREE): the holds execute in
   full on a package recorded with its config content
   carried — a live configuration file whose SHA-256 matches
   the manifest's recorded hash — which is the revival path,
   exercised by the anchored-package control in item 6.
   Provenance of the stored renders (round-38 F4): the
   package's records stand exactly as that run wrote them —
   `observations.jsonl` and `pricing-observations.jsonl`
   are byte-identical to the run's own output — but the
   package's three stored renders (`report.txt`,
   `claim-ledger.json`, and the claim ledger embedded in
   `run.json`) were refreshed from the package's records
   under the round-37 checker. Round 37's plan-anchored
   dimension-(3) census moved the recomputation a stored
   render must agree with, and a render that disagrees with
   the recomputation from the package's own records is a
   package defect under this suite's own checker law, so
   the renders were recomputed from the records and
   rewritten: the outcome was recomputed before writing and
   unmoved (incomplete, 82 standing losses, 0 freeze-gate
   fails), and the records were byte-identical before and
   after. The stored renders are therefore round-37
   recomputations over this run's records, not the run's
   own writes — this item states that provenance so a
   reader of the suite alone can learn it here, not only
   from the gauntlet's records.
4. **Battery audit parts and driver tier** — `battery/battery.py`:
   all five cases end BLOCKED-WINDOWS (their behavioural
   conditions cannot be evidenced here), with every source-audit
   condition evidenced and none refuted.
   `evidence/battery-linux.json` holds the record. The audits are
   function-scoped: each names its file, function and line —
   `Sleep(10)` at `src/engine/wait.rs`, function
   `wait_until_empty`, line 487. The `vars_os()` call is at
   `src/engine/environment.rs`, function `environment_block`,
   line 72. The audit enumerates the four pseudo-handle fetch
   sites in `harden_self` (lines 212, 219), `watched_owner_handles`
   (line 481), and `cli_exit` (line 335), with their consumers,
   including `deny_acl` line 213, `parent_process_id_on` line 489,
   and `TerminateProcess` line 335.
   `Console::finish` at `src/cli_process.rs` line 173; and
   `CreateIoCompletionPort` inside `Extension::start` at
   `src/extend/server.rs` line 124. (Coordinates re-recorded in
   the round-21 landing from a fresh battery run against the
   current tree, each verified against the tree source text at
   that line; the recorded evidence is now held to a fresh run
   by the recorded battery-evidence freshness control in this
   item's family, so a tree move that shifts the audited lines
   fails the suite until the record is regenerated.)
   Mutation controls against a scratch copy of the tree run
   as suite tests: deleting the
   counted owner fetch while the other `GetCurrentProcess()`
   sites are kept, relocating `Sleep(10)` to an unrelated
   function, and renaming `Console::finish` each turn the
   corresponding audit "refuted"; the unmutated tree stays
   "evidenced". The drivers' decision logic is exercised in the
   stub tier: the good variant evidences every behavioural
   condition of every case; the broken variants refute exactly
   their charged conditions (case (a) with the measured
   latencies doubled to 10,000 ms; case (o) with the verdict
   taken from the packet; cases (l), (m), (n) likewise); a
   missing or renamed driver makes its case BLOCKED-ERROR,
   never PASS; and a stub run's evidence records (commands,
   exit codes, raw outputs) are preserved and asserted.
   Round-3 probe discipline — every decided field computed
   from observations, never a probe-written constant the
   decider trusts — is exercised by fourteen further stub-tier
   controls: case (a) decides from the recorded (empty,
   remainder) pairs and the measured latencies (a wait/poll
   remainder mismatch and a never-dies wait returning early
   with budget unspent each refute; a trial record without a
   remainder is BLOCKED-ERROR); case (o) counts the
   active-process-zero packets actually dequeued (a
   zero-delivery variant refutes), counts the teardown
   operations actually performed (a 4-operation variant
   refutes), computes the forged-packet verdict packet by
   packet against the job query (a packet claim flipping the
   verdict refutes), and blocks a forged record carrying a
   verdict-source string without the packet-influence
   observation; case (n) requires the pending cells' held
   output in bytes (a pending-ignored variant refutes) and
   computes the pin fallback's effective verdicts from the
   re-measured lazy verdicts (a fallback record restating the
   eager values is BLOCKED-ERROR); case (l) performs the
   set-entry operation in both serialisations (a stale
   channel entry kept beside the set one refutes; the set
   step absent is BLOCKED-ERROR); case (m) requires a
   non-empty enumerated consumer list per site (an
   unchecked-consumers variant refutes, and the check's
   failure side is unit-tested against an empty list) and
   its driver deadline in battery.py is 14,400 s — the sum
   of its per-site build and test sub-deadlines — with one
   shared scratch copy and one cargo target across sites.
   A driver dying mid-run is BLOCKED-ERROR.
   Round-4 probe discipline — the decider may not trust a
   probe-written label for the quantity it decides — is
   exercised by eleven further stub-tier controls: case (a)
   compares latencies with a tolerance derived from the poll
   tick (10 ms), so the noisy variant (+0.02 ms on the size-0
   trial, +5 ms on a dying trial) still evidences while a
   systematic 2× latency variant refutes, and every trial
   record must state that both proofs raced the same job;
   case (n) pairs each role pairing's exit code, decision
   output bytes and consulted verdict values together (a
   pairing whose verdict values differ refutes although its
   exit codes are equal) and requires every role cell's held
   byte count to equal the pending-pattern bytes measured for
   that cell (gate cells reporting label bytes with no pattern
   behind them refute); case (l) compares the two recorded
   enumerations themselves, unfiltered (a divergence in an
   entry no fixture names refutes although both blocks omit
   it), judges the set operation under the one ASCII-exact
   matcher (a model side skipping by Unicode casefold
   refutes), and unit-tests that matcher against the tree's
   eq_ignore_ascii_case semantics (ASCII variants match,
   non-ASCII case variants do not); case (o) requires the
   non-inheritance spawn record — inheritance enabled, the
   port's inherit flag, the inheritable control's outcome (a
   bare claim blocks; an inheritable port refutes) — counts
   the teardown from per-operation confirmations (a performed
   but unconfirmed disassociation counts 4 and refutes), and
   requires the fallback's two verdicts to carry distinct
   observation ids (two readings of one record block). The battery's
   no-evidence test withholds driver evidence in a simulated
   win32 context and asserts BLOCKED-ERROR on every platform —
   it no longer exempts win32. The fixture set parsed by the
   tests contains the 500-entry case, byte-equal to the
   deterministic generator in `battery/env_fixtures.py`;
   removing the generated case from a scratch copy of the set
   turns the fixture test red (demonstrated). Round-10 driver
   exit discipline: a driver's exit code now follows its stated
   contract for every condition status — a condition landing
   "error" exits 2, never 0 (which claims every condition
   evidenced) — and battery.py no longer uses verdicts from a
   driver that exited outside its contract (0/1), however clean
   the printed JSON: a scratch driver printing an
   all-evidenced payload and then exiting 3 leaves its case
   BLOCKED-ERROR. Both are exercised in the stub tier here.
5. **Syntax** — every Python file compiles under `py_compile`.
   Held by a suite test (round-53 F6, the cure for finding
   F-Acct-2): `CompletenessAccountingTests.test_every_python_file_in_the_package_compiles`
   enumerates every `.py` file in the package (31 files,
   including the battery drivers and the probe files the
   runner never imports on this tier) and compiles each
   with `py_compile` (doraise).
6. **Harness integrity (pricing, order, evidence, identity)** —
   exercised as suite tests against stubs and fixtures. Pricing:
   the three probes run as campaign steps and price each
   expensive-class item from their paired observations; a bare
   pricing dict (the hand-typed 999.0 demonstration) is reported
   "operator-supplied, not measured" with no number and no
   unexplained remainder; an incomplete probe pair set prices
   nothing; no probes at all yields "not measured" for every
   item. Pricing provenance (round-12 stamp landing): a price
   claims a difference between two constructed shapes, and the
   records it rests on must carry the stamps that construction
   stamps — the console pair's contexts (baseline DETACHED,
   loaded NO_WINDOW), the owner pair's inserted_levels
   (baseline 0, loaded 1), the resolver pair's probe_count
   (baseline 0, loaded RESOLVER_EMPTY_FOLDERS) and the pinned
   resolver field both resolver shapes are constructed under
   (working_directory, pathext, current_folder_rule, and the
   loaded shape's intended_path) — cross-checked by
   `pricing.pricing_stamp_defects`, whose expectations are
   derived from pricing.py's own construction (the
   RESOLVER_EMPTY_FOLDERS / RESOLVER_PATHEXT /
   RESOLVER_CURRENT_FOLDER_RULE constants, and the shape
   literals in `_shapes`, stated in that checker's comments).
   An item whose records fail any stamp does not price: it is
   absent from the evidence `prices_from_observations` builds
   (so a re-rendered report shows it "not measured", the same
   state an unpriced item renders), and
   `_verified_pricing_evidence` / `check_run_package` name the
   stamp defect by item and field. The suite's fabricated
   pricing records (`pricing_records()` in the tests) carry the
   genuine stamps, so the provenance controls exercise the real
   shape; erasing exactly the stamp differences the round-12
   protest erased (console baseline context, owner loaded
   inserted_levels, resolver loaded probe_count) now prices
   nothing and names nine stamp defects, where it previously
   returned identical medians with no problems. The preserved
   reduced-run package's records are genuine probe output:
   their stamps verify clean, the package still prices its
   three items (medians −1.1 / −12.0 / −0.0 ms as recorded
   in item 3) — verified explicitly against the package on disk
   for this landing; the package checker's problems for it
   are exactly the named unverifiable set its absent
   configuration anchor degrades (item 3). Owner-verification
   timing gate: the gate's demonstrated detection floor is
   250 ms, stated for the sampler-verified quiet condition —
   the smallest in-bracket mover the landed gate caught in
   every repeated construction (250 ms failed the gate in
   3 of 3 constructions in round 24; a 200 ms mover was not
   caught, and no sensitivity below the floor is claimed).
   The floor's round-24 statement was refuted at its stated
   condition in round 25: on a sampler-verified quiet
   closing run the floor mover's 251.94 ms difference stood
   against a 324.46 ms bound, because the bound's
   dispersion inputs were functions of the treated sample
   (the pair level pooled the treated condition's centred
   differences, and the set level was estimated from the
   per-pair differences of set prices — the effect's own
   per-epoch values, whose heterogeneity it read as
   noise). The gate is repaired in construction: the bound
   is now estimated from the plain (untreated) condition
   only, at both levels, with the median absolute deviation
   (breakdown point 50%) in place of the interquartile
   range (breakdown 25%) — no property of the treated
   sample can raise the bound, and a burst confined to a
   minority of the plain construction cannot either; a
   burst spanning a majority of the plain construction is
   a genuinely noisy construction, where the bound's rise
   states that the floor is not measurable in it. The
   floor is re-stated at 250 ms on the repaired
   construction, re-demonstrated in the round-25 landing:
   five consecutive isolated runs of the sleeping +
   sensitivity pair, each launched after a sustained-quiet
   window, passed both controls in all five (run sampler
   peaks 7.26–8.18), and the reconstruction of the refuting
   run returns the captured 324.46 ms bound from the old
   construction and 47.01 ms from the repaired one. The
   floor remains a quiet-condition figure, not an
   unconditional property of the gate. Order: two seeds give different warmup/measured orders
   with identical configured totals; harness runs at seeds 7 and 8
   record different orders, both passing the order checker; a
   run package with its recorded seed edited, or two observation
   lines swapped, is flagged by the checker. Evidence: a second
   harness invocation over a partial (reduced) first run's
   directory preserves the first run's files byte-for-byte
   (hash-compared) and lands in its own namespace, and a legacy
   partial `observations.jsonl` sitting directly in the out
   directory is likewise preserved. Identity: a stub binary
   probing as version 9.9.9 against the pinned 1.0.0 is named as
   a mismatch; a binary claiming the right version with different
   bytes than the staging record is caught by hash; a matching
   identity passes; the pin in `campaign.json` is the anchor — a
   binary matching the staging record but not the pin fails
   identity, a binary matching the pin is still checked against
   the record, and a scratch binary whose `--version` carries
   the right token but whose bytes differ from the real pin in
   `campaign.json` is rejected by the same hash verification
   `build.ps1` uses at staging (unit-tested directly, and the
   script side checked mechanically: the pin hash is verified
   before the competitor copy and before the record is written,
   the throw naming the arm, the expected pin, and the actual
   hash); the report lists every arm's fingerprint and
   version, and an identity mismatch turns an otherwise complete
   fixture INCOMPLETE. Build script: `build.ps1`'s text carries
   the 1,800 s deadline constant, the bounded wait, the
   process-tree kill on expiry, the exit-code check, the
   preserved-bundle default staging source with its operator
   override (the bundle beside the package, resolved before the
   staging block an explicitly named `-Competitors` folder feeds),
   the staging
   identity verification naming mismatches, and the hash record;
   the staged build record (`build\build-record.json`) carrying
   the built tree's identity — `tree_as_given` (the `-Tree` value
   as given) and `tree_resolved` (its resolved absolute form) —
   written on every build, outside the competitor staging block,
   so a guard-only build records its tree too —
   brace and parenthesis balance verified mechanically here; its
   execution is Windows-theory (below). Freeze gate: a fixture
   campaign whose nuewframe kill slots are all
   descendant-survived, with every other cell complete and the
   guard otherwise winning, renders with
   kill-path correctness naming nuewframe losing, its slots
   listed as competitor losses (evidence) rather than campaign
   standing losses, and nuewframe excluded from the kill-latency
   comparison; the negative controls hold — a rival whose kill
   records are absent stays UNEVALUATED and the campaign
   INCOMPLETE, a guard descendant-survived kill slot still FAILs
   kill-path correctness, and a rival whose measured kill slots
   are all infra-failure is not converted into a correctness loss
   or a campaign pass. Evaluation profile: every report prints
   the full enumeration of SPEC_Benchmark §3's eight dimensions
   with a state per dimension — (1) Correctness and output
   quality; (2) Latency, throughput, and startup time; (3) CPU,
   memory, storage, network, energy, and monetary cost;
   (4) Scaling, concurrency, and resource exhaustion;
   (5) Reliability, prolonged operation, recovery, and data
   integrity; (6) Security boundaries and resistance to malformed
   inputs; (7) Compatibility, portability, installation, and
   integration; (8) Usability and accessibility for interactive
   products — the gate dimensions mapping into (1) and (2) and
   carrying their PASS/FAIL, every other dimension defaulting to
   UNEVALUATED; a report over the reduced-run records lists all
   eight with states, a four-dimension-pass package with the
   rest UNEVALUATED still renders INCOMPLETE (in
   `campaign_losses` and the harness outcome, not only in text),
   a dimension with no decision in the decisions mapping the
   run's configuration carries cannot render NOT-APPLICABLE,
   and a decision there renders NOT-APPLICABLE with its reason
   printed beside the dimension. An entry whose reason is empty, blank or not
   a string decides nothing — the dimension stays UNEVALUATED
   and an otherwise complete fixture stays INCOMPLETE (the
   decision-reason control). The phrase "every applicable dimension"
   appears only together with that enumeration. Claim ledger
   (round-8 landing): the decisions are package data in
   `campaign.json` (`profile_dimension_decisions`), recorded
   verbatim in each run's `run.json` beside the configuration's
   SHA-256 — a suite control pins that `campaign/report.py`
   carries no decisions table and that the shipped mapping
   covers exactly the six dimensions the gate does not
   observe, each with a non-empty entry — a scope decision, or
   (since the round-26 landing) a COLLECTED instrument
   declaration for dimensions (3), (4), (6) and (8). The harness writes the claim ledger
   (one claim per §3 dimension: state, plus a basis pointing to
   the freeze gate over `observations.jsonl` or to the
   configuration decision and its reason) as `claim-ledger.json`
   in each run namespace, mirrored in `run.json`; the report
   renders it in full; `campaign_outcome` consumes it; and
   `check_run_package` verifies it against the records and the
   recorded configuration hash and decisions — controls forge a
   ledger claiming a decision the configuration does not carry
   and a ledger claiming a state the records contradict, and
   each is named as a package defect. The existence proof is an
   anywhere-tier fixture: the perfect fixture (1,940 records as configured)
   (guard wins or ties every gate dimension, floors agree, no
   survivors) with the shipped decisions in force returns
   `campaign_status: complete` with zero standing losses, and
   the same fixture with the decisions removed returns exactly
   the six UNEVALUATED profile losses — the decisions, not the
   code, do the deciding. Run script: `battery.py`'s exit-code
   contract is pinned by suite tests (0 = executed with no FAIL
   or BLOCKED-ERROR; 2 = BLOCKED-ERROR execution failure, taking
   precedence over FAIL; 3 = FAIL decision with the report
   written; never 1, the crash code), and `run.ps1`'s text is
   checked mechanically for the matching constants, the
   missing-`battery.json` execution check between the battery and
   test stages, the FAIL decision recorded without a throw before
   the families, the end-of-run aggregate naming every stage that
   did not pass, the campaign failure still aborting first, and
   brace / parenthesis / bracket balance; the same item's
   tree-consistency controls pin `run.ps1`'s `-Tree` declared
   with build.ps1's exact defaulting, the resolved tree passed
   to the battery as `--tree`, every battery case record naming
   the tree it audited (the `--tree` value, recorded by
   `battery.py` in `battery.json` and the `--json` output), and
   the build-record comparison before the campaign stage: a
   mismatch refuses, naming both trees, before any stage
   executes its payload, and with no build record present the
   stage record states the audited tree and that no build
   record was present to check against; its execution is
   Windows-theory (below). Round-10 script and record controls:
   the staging record is read as utf-8-sig, so the byte-order
   mark build.ps1's Set-Content writes under Windows
   PowerShell no longer silently disables the record check (a
   BOM-carrying record file parses to the same record here);
   run.ps1's campaign-abort path creates the results folder
   before writing the stage record, so an admission refusal —
   which writes no namespace — still records the campaign exit
   code; and the two places a script redirects a native
   command's stderr under its Stop preference (build.ps1's
   competitor version probe, run.ps1's test stage, whose
   runner writes its progress to stderr) run under a locally
   relaxed preference restored immediately after, checked
   mechanically here, executed on Windows only in theory.
   Stopped campaigns: a suite test stops
   a stub-tier run mid-loop (the slot runner raising after five
   of twelve slots) and verifies the provisional `run.json`
   already existed before the first slot with the full planned
   order, that its `slots_run` tracks the records written, that
   the package checks consistent-as-a-prefix, and that the
   report renders INCOMPLETE naming the stopped state and the
   missing cells; an observations-only namespace (no manifest)
   also renders INCOMPLETE via both the parent directory and the
   namespace itself, a namespace with neither manifest nor
   observations exits as an error with no report, non-prefix
   observations and a tampered provisional seed are each flagged
   by the order checker, and a fixture with every slot present
   still renders INCOMPLETE for the evaluation profile (below).
   Counted-ops census: the census figures every report's
   floor-based decomposition prints (plain path 45 named
   calls, hook path 18, run-core subtotal 36) are the vendored
   enumeration of MEASUREMENT §2's named calls in
   `campaign/census.json` — the plain path's and the hook
   path's named calls in §2's own groupings, in §2's order,
   with the subtotal's composition as §2.2 states it (job
   setup to process exit) — enforced by the counted-ops census
   tests in `CensusConsistencyTests`
   (`tests/test_campaign_logic.py`): one recounts the plain
   total, the hook total and the run-core subtotal from that
   enumeration and holds `report.py`'s CENSUS to the recount;
   one re-derives the enumeration from the guard tree's
   MEASUREMENT §2, by the parsing method stated in that test
   (sections sliced by heading, table rows split into Step /
   Calls / Count cells, a backticked name with its integer
   multiplicity as a named call, the hook payload row's
   per-window-fill `ReadFile` taking its Count-column figure),
   and holds the vendored data to the re-derivation — that
   test skips, with its reason stated, when the guard tree is
   not present at the suite's expected relative location
   (`..`, the folder that holds the suite), and the recount
   test never skips; and one
   holds the census totals printed in `README.md`'s
   decomposition passage and `FAILURE-TREE.md` §1 to the
   recount. No census citation in this package points only
   outside the package.
   Pricing uncertainty: the evidence object carries
   each item's paired differences with dispersion (min, max,
   quartiles, IQR) and sign counts beside the unchanged median;
   a fixture reproducing a −1.178 ms console price and priced
   items summing to 19.149 ms against a 10.000 ms distance
   renders with INCONSISTENCY flags on the item line and as a
   standing loss, states that the decomposition's parts do not
   sum, and prints no negative unexplained remainder, while a
   consistent fixture (9/9 same-sign, tight dispersion) renders
   with medians unchanged and no flag (its STATUS is INCOMPLETE
   only for the evaluation profile). Pre-flight
   admission: matching stub identities are admitted, a
   wrong-version competitor is named as an admission failure,
   and a stub-tier config with one competitor binary deleted
   makes a non-reduced run exit with the distinct admission code
   (4) naming the arm, having written no `run.json` and no
   observations, while the same config under `--reduced` still
   runs; the post-run identity record in `run.json` remains.
   Campaign outcome contract: a stub-tier run's `run.json`
   records `campaign_status: incomplete` with its standing-loss
   and freeze-gate FAIL counts beside the report's STATUS line
   while the harness exit code stays 0 (status, not exit code,
   carries the outcome — the recorded choice), a fixture with
   every slot and gate dimension passing has outcome incomplete
   with zero freeze-gate FAILs, its standing losses being the
   evaluation profile's UNEVALUATED dimensions (the profile is
   wired into the outcome's status logic, not only the report
   text), a
   freeze-gate-FAIL fixture's outcome is incomplete with the
   FAIL counted, and `run.ps1`'s text is checked mechanically
   for the shared `campaign_status` key, the
   `campaign status: ...` stage record, and the campaign's
   naming in the final aggregate before the battery stage.
   Reduced-shape fence: the perfect fixture at full plan size
   with the shipped decisions in force, driven through
   `campaign_outcome` with the reduced flag, records
   `campaign_status: incomplete` with exactly one standing
   loss — the loss naming the reduced shape (`--reduced`) as
   its reason — where the same records without the flag record
   complete with zero; a small-N reduced run's losses carry
   the same named reduced-shape loss beside its missing-slot
   losses; and a preserved full-size package whose manifest
   records the reduced shape re-renders through `cmd_report`
   as INCOMPLETE with that loss named, never a complete
   STATUS.
   Output observation (round-11 landing): profile dimension
   (1) is no longer certified on kill-path and exit-code
   evidence alone. The fast child is an output workload — the
   stub relay here, testkit `relay` in the shipped
   configuration: the runner feeds it the child spec's
   configured stdin text on a regular file, it copies that
   text plus one newline to stdout and writes its fixed
   `error-stream` line to stderr, and the expected bytes on
   both streams are configuration data in `campaign.json`,
   identical for all four arms. Every fast-path slot's stdout
   and stderr are captured to per-slot regular files (never
   the null device, never pipes) and the record carries, per
   stream, the emitted SHA-256, length and match verdict;
   kill-path slots carry no output observation, because the
   kill child's product is the tree it leaves, judged by the
   survivor probe, not a byte stream. Output correctness is a
   freeze-gate dimension mapped into dimension (1): the
   negative controls are exercised at stub tier — the mute
   arm returns the expected exit code 0 with both streams
   suppressed and is classified `output-mismatch`, the garble
   arm alters stdout and is likewise classified — and at
   fixture tier a guard output mismatch FAILs the gate
   dimension and the dimension (1) claim, a fixture stripped
   of output observations leaves the dimension UNEVALUATED
   (never observed-passing) and the campaign incomplete, and
   a competitor output mismatch is listed as competitor-loss
   evidence under the same prefix as competitor kill losses
   and does not block a complete campaign. The shipped
   configuration control pins the workload's shape: the fast
   child's expected stdout is its configured stdin text plus
   one newline, and its expected stderr is the relay's fixed
   line.
   Record consistency, negative direction (round-12 landing):
   the round-11 discipline cross-checked an "ok" record
   against its fields but trusted every negative label at
   face value and never cross-checked the output fields at
   all, so a fixture whose guard fast records were classified
   "ok" while their own `output_stdout_match` was False
   completed with output correctness PASS, and a fixture
   whose competitor kill records were relabelled
   "descendant-survived" while their survivor fields stayed
   False minted 180 competitor losses, excluded the arm from
   kill-latency (erasing an honest kill-latency FAIL), and
   completed. The discipline now runs in both directions in
   `campaign/report.py`: an "exit-mismatch" record whose
   exit code is the expected code (or unobserved), a
   "descendant-survived" record whose survivor field is not
   True, an "output-mismatch" record with no False stream
   verdict, an "ok" fast record with a False stream verdict,
   and a match verdict carried without its digest and length
   are each named record-inconsistency standing losses;
   survival and output evidence are counted only where the
   record's own fields earn them (`_evidenced_survived`, the
   competitor-loss builders and the output gate's
   inconsistency branch), so the fabricated fixtures above
   now render INCOMPLETE with both kill dimensions
   UNEVALUATED and no competitor losses minted, while the
   unmodified perfect fixture and the genuine
   descendant-survived fixture (survivor True) still complete
   exactly as before, and the preserved reduced-run package
   still passes `check_run_package` with no problems. The
   five controls are exercised at fixture tier in item 1.
   Round-13 landings, same item: (i) the tree identity an ok
   kill record must carry is the grandchild's — the shipped
   kill workload (testkit spawn running testkit mark) records
   only the grandchild's PID at the slot's marker, so the
   round-11 rule demanding child_pid as well made the shipped
   shape earn 792 inconsistency losses on a perfect fixture
   (child_pid null on every kill record) and FAIL kill-path
   correctness in both contexts; child_pid is now required
   only to be an integer when present, and a perfect fixture
   in exactly the shipped shape completes. (ii) Survivor-
   before-exit precedence is now one rule in both halves: a
   runner-produced descendant-survived record with a
   mismatching exit code (exercised at stub tier: direct-only
   kill, exit 7) is no longer named a record inconsistency,
   and its exit fault is listed separately — for a competitor's
   measured slot as competitor-loss evidence. (iii) A
   competitor's measured exit mismatch joins its measured
   output mismatches and descendant survivals as
   competitor-loss evidence on dimension (1): one such slot in
   a perfect fixture now completes, where it previously stood
   as a campaign loss with every gate dimension PASSing. (iv)
   The resolver pricing stamps are cross-checked as the
   construction states them: the loaded record's intended_path
   must be the laid-out field (four empty folders, then the
   child's folder) and every resolver record must share the
   pair's one working directory — a fabricated single-folder
   intended_path and a split working directory each previously
   priced the item at full value and now name stamp defects
   and price nothing. (v) check_run_package verifies a
   finalised package's recorded outcome (campaign_status and
   both counts, the fields run.ps1 acts on) against the outcome
   its own records recompute to: the preserved reduced-run
   package still checks clean, and the same package with its
   manifest rewritten to complete/0/0 is now named a defect.
   Round-14 landings, same item: (vi) the resolver stamp check
   holds intended_path to the laid-out structure — the four
   empty folders named empty-0..empty-3 under one scratch
   parent, the parent of the record's own resolver-cwd working
   directory, then the child's folder — so a fabricated field
   of five arbitrary folders of the right entry count now
   names a stamp defect and prices nothing, where it
   previously priced at full value; (vii) check_run_package
   recomputes competitor identity from the fingerprints and
   versions a manifest itself records, against the pins of the
   configuration whose SHA-256 it records (when that file is
   present and hashes to the recorded value): a package
   recording wrong competitor hashes and versions beside an
   empty recorded mismatch list — claiming complete — now
   names every recomputed mismatch, its recorded outcome fails
   recomputation against the recomputed identity losses, its
   finalised slots_run is held to its record count, its
   report.txt STATUS line is held to the recomputed outcome,
   and a corrupt pricing-observations.jsonl is named as a
   package defect instead of crashing the checker; a parent
   directory's run namespace is resolved by manifest
   modification time, not by name; and the guard families'
   driver no longer falls back to the guard tree's cargo
   target folder, so a stale binary there cannot be located
   or certified in place of the staged package.
   Round-15 landings, same item: (viii) the package checker
   validates record shape before any recomputation — an
   observation record that is not an object, lacks a
   slot-identity field, carries a non-integer rep or an
   elapsed_ms that is neither null nor a number, and an ok
   pricing-probe record whose rep or elapsed_ms cannot pair,
   are named package defects, where they previously raised
   KeyError / TypeError out of check_run_package and hid every
   later check (the parseable form of round 14's P1 crash);
   the pricing pairing itself now skips records that cannot
   pair, so such a record prices nothing instead of crashing
   the pairing arithmetic. (ix) The recorded plan and the
   ledger's bases are verified against the anchors the checker
   already computes: plan_counts must equal the configured
   plan's derived counts and slots_planned the recorded
   order's own length (a manifest recording a one-slot plan
   previously passed clean), and each claim-ledger entry's
   basis is held to the basis the records and the recorded
   decisions give — a basis forged identically in
   claim-ledger.json and in run.json previously passed clean,
   the states and reasons being the only ledger fields held.
   Round-16 landings, same item: (x) the shape validation now
   covers the manifest and the ledger as well as the records:
   a run.json that parses but is not an object, a recorded
   fingerprints value that is not per-arm objects carrying a
   string sha256, a claim whose dimension is not a string, a
   pricing record whose probe name is unhashable or whose
   guard fingerprint's sha256 is not a string, and a recorded
   identity list holding non-strings are each named package
   defects, where each previously raised AttributeError /
   TypeError out of check_run_package on its own axis (the
   same crash class as (viii), one level up: the manifest and
   ledger shapes the recomputations consume). (xi) An
   elapsed_ms must now be a time the runner's monotonic clock
   can produce — finite and non-negative: NaN, infinity and
   negative values (each parses as a JSON number) previously
   passed every check and entered medians and paired pricing
   differences unnamed, with NaN paired differences reported
   as sign-consistent zero-sign pairs and infinite dispersion;
   they are named shape defects, and in the pricing pairing
   itself such a record prices nothing. An observation outcome
   outside the five classifications the runner writes is
   likewise named: it previously fed no median and no loss
   list and vanished silently.
   Round-17 landings, same item: (xii) the shape validation
   now covers the pricing records' own containers — an ok
   pricing-probe record missing its probe or shape, or
   carrying either in a wrong type, is named by field (each
   previously raised KeyError out of the pairing, and out of
   check_run_package itself), the pairing tolerates such
   records (they price nothing), and the paired-count
   recomputation counts only integer reps (an unhashable rep
   previously raised TypeError out of it on the report path);
   the competitor-identity check names malformed containers
   (a non-object pinned entry or identities mapping, a
   non-mapping fingerprints argument, a non-string fingerprint
   path in the staging comparison) as unconfirmed identities
   instead of raising; cmd_report applies the checker's shape
   verdicts before rendering and refuses a malformed package
   with the defects named, in the same idiom as its non-object
   manifest refusal (the render previously raised on records
   the checker names); a recorded manifest count must be an
   integer — a float count (46.0 standing losses, 40.0 slots
   planned, 18.0 pricing observations) previously passed every
   equality comparison unnamed; and a record's verdict fields
   must carry their types — an output match verdict that is
   neither boolean nor null, and an exit code that equals the
   expected code in value but is not an integer (0.0),
   previously passed the record-consistency cross-check
   unnamed, the non-boolean verdict completing a perfect
   fixture's campaign through the output gate.
   Round-18 adjudication landings, same item: (xiii) the
   five type/value-confusion fixes the round-18 review
   specified are covered by controls whose fail-before was
   taken against the preserved pre-fix code at the round's
   preservation head: cmd_report guards its three package
   reads (observations, pricing observations, manifest) in
   the checker's (OSError, ValueError) idiom, refusing a
   torn file with the defect named — exercised at
   representative cuts of each file's first record (the
   manifest's whole body), where every cut previously
   raised a raw JSONDecodeError; the pricing stamp
   discipline holds an integer stamp to the integer type,
   so a float-equal inserted_levels (1.0 / 0.0) or
   probe_count (4.0 / 0.0) names a stamp defect and prices
   nothing; the outcome/report layer holds a True match
   verdict's digest and length to the expected bytes
   derived from the configuration the package's manifest
   names (by path, anchored to the SHA-256 the manifest
   records) and requires the digest's 64-hex form
   regardless — a "zzz" digest, a well-formed digest of
   other bytes, and lengths of 7 and of 10**30 are each
   named at the package layer, and the correct pair
   completes; the rollup's pricing-evidence admission
   accepts only a finite, non-boolean ms, a NaN or True
   price resolving to "not measured" with no remainder
   computed from it, where a finite price still prices;
   and the configuration loader validates its root and
   its arms / children containers as objects with a named
   ValueError, admission_failures naming a non-object arm
   entry as that arm's admission failure, where each shape
   previously raised AttributeError.
7. **Scanner differential, reference side and corpus** — the
   independent parsed-form reference in
   `tests/scanner_reference.py` is pinned to the documented
   verdicts on directed cases (good and bad envelope shapes, the
   gate/full role-dependent paths, both timeout rounding bounds,
   last-key-wins and the `cmd` fallback). The seeded generator
   produces a corpus of 357 cases (51 directed, 9 cursor-directed,
   29 timeout-token, 43 structure, 25 window-edge, 90 mutation,
   110 fuzz; largest payload 113,041 bytes, every case below the
   16 MiB cap), byte-for-byte deterministic across generations
   (payload SHA-256 in `evidence/scanner-corpus-summary.json`:
   SHA-256 over the corpus in generation order, each case
   contributing its payload length as an 8-byte big-endian
   unsigned integer followed by the payload bytes, with no
   category, index or separator bytes and no trailing bytes —
   the serialization stated beside the field in the summary
   and recomputed and asserted against the summary by the
   corpus test `test_corpus_payload_hash_matches_the_summary`).
   The reference's verdicts over the corpus are non-vacuous in
   every role/vendor combination (full/pretooluse 57 allow /
   300 deny; gate/pretooluse 60 / 297; cursor 8 / 349 under both
   roles). Negative controls: a case-insensitive-path variant of
   the reference and an always-allow guard side each diverge from
   the aligned reference over the corpus, so the comparison can
   fail. The guard side of the differential (1,453 comparisons
   per Windows run) is Windows-theory (below), as are the
   coverage families' guard-driving assertions; their anywhere
   parts — the payload-cap payload construction with its exact
   sizes, the install-layout and resolver-field layout code —
   run and pass here. The resolver field's environment
   construction is likewise pinned anywhere: the environment a
   resolver probe or an F10 test hands its child carries exactly
   one path-spelled entry — the laid-out field — even when the
   parent environment spells the variable `Path` or carries both
   spellings, and the pricing resolver probe's loaded
   observations record the effective PATH they intended. The
   pricing resolver probe's whole field is likewise pinned
   anywhere: both its shapes are constructed with a fresh,
   empty scratch working directory and a child environment
   pinning PATHEXT and setting
   NoDefaultCurrentDirectoryInExePath (parent variants of each
   controlled name collapsed), every resolver observation
   records the working directory, the PATHEXT and the
   current-folder rule in force, a spawn-seam capture proves
   the pinned values reach the child spawn, and a same-named
   decoy placed in the harness process's own working directory
   is never the hit because the child's working directory is
   the scratch folder.

[↑ Back to Contents](#contents)

## Claim ledger and the six profile-dimension decisions (round-8 P7 landing; round-26 collections landing)

Dimensions (1) and (2) are observed: the freeze gate over the
campaign's own records carries them, and their ledger basis is
that gate. For each of the six dimensions the campaign does not
observe, the landing had two honest routes — wire an observed
state from evidence the suite genuinely produces for *this
comparison*, or record a scope decision whose reason states why
the dimension does not bear on this comparison's claim or what
the comparison explicitly does not claim. Wiring was rejected
for all six, for one shared reason: the evidence that touches
(4), (6) and (7) — the tree's wide/deep-tree kills, the
malformed-payload hook families and the scanner differential,
the hook-vendor families — is non-comparative (the guard against
its own specification or its tree, never measured against the
three competitors in this campaign) and lives outside the run
package, so an "observed-passing" ledger claim for a comparative
dimension would have asserted a comparison that never ran. For
(6) that holds even though the command-line surface is shared
by all four arms: this campaign collects no malformed-input
observations on any arm. For (7) the hook-vendor surface that
evidence touches has no counterpart in the competitors. Each dimension therefore carried a
scope decision in `campaign.json`
(`profile_dimension_decisions`, reason verbatim there and in
every report and ledger), argued per dimension. The round-26
landing (below, and in the per-dimension entries for (3), (4),
(6) and (8)) superseded four of those decisions with
collection declarations after sanity check p4-5's D1 charges
were sustained in round 26's adjudication: the entries for
(5) and (7) stand as decided; the entries for (3), (4), (6)
and (8) now record what collects each dimension:

- **(3) CPU, memory, storage, network, energy, and monetary
  cost — collected (round-26 F3; the round-8 decision is
  superseded).** The runner now carries the instrument whose
  absence the old decision cited: every slot record carries the
  arm process's `cpu_ms` and `peak_working_set_bytes` from the
  platform's per-process accounting (POSIX: the wait4 rusage of
  the arm's reap, exact for that child; Windows: GetProcessTimes
  and GetProcessMemoryInfo on the arm's handle), or a
  `resource_note` naming why a field is absent — absent-with-
  reason, never silently zero. The dimension's state is
  computed from those records; a run whose measured records
  lack the fields (or carry only notes) renders it UNEVALUATED.
  Round-27 F3: the fields are also held per record (a present
  counter must be a finite, non-negative number — a negative
  or NaN value is an inconsistency, not a measurement; an
  absent one must carry its reason), and the state is computed
  per arm with a reachable FAIL: an arm whose measured records
  carry no numeric measurement of a counter fails the
  dimension, and the ledger states the per-arm numeric
  coverage a PASS observed. Round-29 F2/F3/F6: the peak hold
  binds type — a present peak must be integer-valued (both
  runner sources produce an integer), so a fractional or
  subnormal peak is an inconsistency and no coverage, as a
  peak of 0 already was; the presence hold names a record
  whose resource KEY is missing, with or without a retained
  `resource_note`, in every record state (the runner
  constructs every record with both keys and never deletes
  one; the note excuses a null value only — its
  collection-failure shape in the infra states and in the
  by-note platform counter failures on a completed slot);
  and the numeric holds are total — a huge, non-finite or
  wrong-typed resource value is named, never an exception
  in the checker. Round-31 F1 made per-arm coverage the
  precondition, not the verdict; round-32 F1 rebuilt the
  comparison it preconditions, after round 32's review
  executed the paired statistic's four defects (a verdict
  flipped by permuting repetition labels, silent per-cell
  absence certifying a PASS and a COMPLETE campaign, a
  median blind to total cost, and variants pooled inside a
  cell). The state now computes the arm-comparison verdict
  SPEC_Benchmark §3 evaluates, §7's freeze gate requires
  (win or tie on every applicable dimension) and §12
  separates from accounting, with NO pairing by rep index:
  the comparison unit is one (competitor, counter, path,
  context, variant) cell — the variant is part of the unit,
  so scale variants are never pooled, and on the scale
  path the variant is the one the record's slot encodes
  in the plan (round-33 F1), never its carried label.
  Round-33 F3 rebuilt the aggregates again, after round
  33's review executed the median+total pair's three
  defects (a total comparing unequal numeric populations
  — a false loss minted by count alone, on the peak
  counter a sum of maxima that measures the invocation
  count, not memory cost; a PASS on a unit numeric on 1
  of its 90 designated records; and a worst record at 55×
  the competitor's worst passing on both aggregates):
  the comparison was made only over EQUAL numeric
  populations, at a per-unit numeric-coverage floor — a
  unit whose two sides carry different numeric counts,
  or in which a side is numeric on only a minority of
  its planned records in the cell (its numeric count
  less than half its measured records there), is not
  compared at all — over a median/maximum/total aggregate
  set. Round-33 F4: the unpaired list is rendered by
  name in the profile detail and the ledger basis at
  both verdict consumers, the way cost losses are named
  — before that landing the names existed only in the
  comparison's return value, which both consumers
  discarded. Round-34 F1 replaced the aggregate set with
  the single criterion its regress pointed at, after
  round 34's review executed the triple's two remaining
  blindnesses (a designated arm whose mean per-invocation
  peak was higher passing on median and maximum — the
  round-33 landing's own regression had enshrined the
  shape, peaks [300, 500, 900] against [100, 500, 900]
  asserted as no loss; and a designated arm worse at 43
  of 90 sorted ranks, and at the 75th and 90th
  percentiles, passing median, total and maximum alike):
  the comparison is SORTED-RANK DOMINANCE — in each
  compared unit, per counter (cpu_ms and
  peak_working_set_bytes alike), the designated arm's
  sorted plausible numeric values must be at or below
  the competitor's at EVERY rank, over the same equal
  numeric populations at the same per-unit coverage
  floor. Equality at a rank is a tie at that rank; a
  designated value above the competitor's at any rank
  is a cost loss naming the counter, the failing ranks
  and the values at them, and the dimension FAILs
  (profile state FAIL, ledger state observed-failing, a
  standing loss through the profile FAIL loop), under
  the freeze gate's own win-or-tie discipline with no
  tolerance band. Total-cost and peak-mean sensitivity
  are implied by full-vector dominance — at equal
  cardinality a rankwise at-or-below vector has its
  total and its mean at or below the other's — so no
  aggregate stands beside the criterion, and no band of
  the distribution is left unjudged. A unit that is
  one-sided, cardinality-unequal or below the coverage
  floor is NAMED in the comparison's unpaired list and
  renders the dimension UNEVALUATED — the dimension
  cannot PASS silently on unpaired cells, and a campaign
  whose dimension is UNEVALUATED cannot render COMPLETE
  off them. Round-34 F4: every compared unit's per-unit
  numeric and planned counts are rendered in the PASS
  surfaces (the profile detail and the ledger basis),
  extending round-33 F4's render discipline from
  unpaired units to all units — a PASS at exactly half
  coverage (45/90 per unit) reads as one. Round-35 F1: the
  population a PASS certifies is now itself held — a null
  counter is accounted only by a resource_note from the
  runner's CLOSED collection-failure vocabulary (the exact
  notes runner.py's note-generation sites write; any other
  note, including one that declares the censoring in words,
  leaves the record UNACCOUNTED, named as a resource
  inconsistency, and the dimension UNEVALUATED over it),
  and a unit carrying any censored (null-valued) record on
  either side for a counter is rendered in the unpaired
  list as a CENSORED unit under the worst-case imputation
  (every censored value resolved against the designated
  arm's claim: a censored designated value unbounded above,
  a censored competitor value at the counter's minimum) —
  never compared, never a loss, and the dimension
  UNEVALUATED over it, however sparse the censoring: a
  PASS certifies only units whose every planned record was
  measured on that counter. The round-34 exactly-half PASS
  is thereby corrected (its regression renamed and
  re-asserted as the censored construction it is), and the
  reviewer's full P1 construction — half a cell censored
  on both counters for every arm — renders UNEVALUATED
  with the campaign INCOMPLETE. Round-35 F4: a dominance
  failure's standing-loss entry carries its MAGNITUDE —
  the failing (unit, counter) pairs by name with their
  failing-rank counts and the aggregate failing-rank
  total — so the reviewer's pair (3 paired losses / 3
  failing ranks against 144 paired losses / 2,520 failing
  ranks) no longer renders the identical entry, while
  standing_loss_count still counts entries (one per
  failing dimension) and freeze_gate_fail_count still
  counts gate FAIL entries only, each as its stated
  meaning requires. Round-36 F1: the comparison's unit
  enumeration is PLAN-DRIVEN — every planned unit
  (competitor × counter × path × context × variant, from
  the plan's planned populations) exists in the census
  whether or not any plausible value survives; a unit
  with zero plausible values on a side renders in the
  unpaired list with its planned counts, never absent,
  never PASS-eligible — the round-36 reviewer's cell
  censored on every record of every arm (360 records)
  produced no entry anywhere before this landing and the
  dimension PASSed over it; it now renders UNEVALUATED
  with the campaign INCOMPLETE, its six units named.
  Round-37 F1: the planned population is build_plan()'s
  measured slots — the same source the (4)/(6) limbs
  trust — not the judged records: round 36 built the
  planned map from the records themselves, so a planned
  cell whose records were DELETED entirely vanished the
  same way; the round-37 reviewer's deletions (all 360
  measured fast/DETACHED records; all 720 measured kill
  records) PASSed (3) over the absent cells. A planned
  unit with no records now renders in the unpaired list
  with its planned counts from the plan and zero present,
  the dimension UNEVALUATED and the ledger not
  observed-passing over it; the honest fixture's compared
  census still reads 144 with its verdict unmoved. The
  regressions stand in `Round37LandingTests`.
  Round-39 F2: no carried fallback at plan count 0 —
  a cell the plan does not contain is not a comparison
  population. Rounds 36–38 fell back to the carried
  count for such a cell, so one fabricated record per
  arm in an invented variant (wide-999) compared on
  all 6 of its units and its PASS surfaces certified
  "1 planned record(s) (1/1)" for a variant whose
  planned population is 0, the dimension PASSing over
  it. Now such a cell's records render UNPLANNED in
  the unpaired list, with the plan count stated as 0,
  never compared and never rendered with a fabricated
  planned count, and the dimension renders
  UNEVALUATED over them at the state, profile, ledger
  and render surfaces; genuine variant resolution is
  unmoved (a wide-8 deletion leaves the other scale
  variants compared; a malformed plan cell is
  floor-bound at its own planned counts; the honest
  fixture still compares 144 and PASSes). The
  regressions stand in `Round39LandingTests`.
  Round-40 F1: the gate keys by the slot-derived
  variant on every path — a record's variant is the
  slot it answers, never its carried label, on the
  malformed and fast paths exactly as on the scale
  path since round 33. Round 39's gate read the
  carried label on every non-scale path, so a
  malformed case-relabel (Empty-Command) minted 6
  UNPLANNED units and 6 planned-shortfall units for
  one keying defect while (6) PASSed by slot, and a
  fast record's carried string — a field no fast slot
  encodes — manufactured an UNPLANNED cell while its
  planned cell compared the remainder. Now the
  relabelled records render as their planned cell's
  records (the label named once, in the variant
  channel), no UNPLANNED cell and no planned-shortfall
  pair is minted from a label, and the honest fixture
  holds 144 compared, (3) PASS. Round-40 F2: the
  plan-0 rule binds per (cell, arm), the grain the
  counts use — a zero-planned arm's records are
  UNPLANNED even inside a planned cell, and no
  compared unit renders a count over a zero
  denominator (no "(4/0)" in a compared unit); the
  shipped symmetric plan is unmoved. Round-40 F3:
  unplanned records are excluded from every evidence
  consumer — a record matching no plan slot (the
  slot-accounting identity, variant-blind) is not
  evidence anywhere: it supplies no resource
  coverage, moves no decompose or pricing median,
  flips no kill state and mints no competitor-loss
  entry; it exists in the verdict only as named
  incompleteness. One wide-999 record no longer
  softens (3)'s earned FAIL through the coverage
  limb, ninety fabricated fast-999 records move no
  median and erase no pricing standing loss, and one
  kill-999 record flips no kill state. The
  regressions stand in `Round40LandingTests`.
  Round-41 F1: the planned population is
  multiplicity-exact — the plan's Counter, slot
  accounting's own semantics — in every evidence
  consumer: a record beyond its slot's planned count
  is UNPLANNED, rendering as unplanned naming and
  never as evidence. Round 40's membership filter
  admitted copies at full multiplicity: 100 copies of
  one planned slot moved the guard fast median
  62.0 -> 100000.0, one duplicate copy softened this
  dimension's earned FAIL to UNEVALUATED through the
  coverage limb, and 90 copies erased a pricing
  INCONSISTENCY. Round-41 F2: the planned-population
  discipline binds the freeze gate's label readers —
  guard_all and guard_fast are drawn from the filtered
  population, and the gate's inconsistency readers
  read only planned records' items — so one fabricated
  guard rep-999 record per label (exit-mismatch,
  output-mismatch, infra-failure) moves no gate
  dimension and mints no freeze-gate standing loss;
  the inconsistency hold itself still names every
  record, planned or not. Round-41 F6: every
  unplanned record's evidenced fault is listed in
  its own name in the standing losses — exit faults,
  output mismatches and descendant survivals alike,
  each under its evidenced predicate — never as
  competitor evidence and never nowhere; round 40
  listed only the exit faults, so an unplanned
  competitor output mismatch or descendant survival
  stood in no fault listing at all. The regressions
  stand in `Round41LandingTests`. Round-42 F2: the
  own-name listing is kind-complete — every unplanned
  record's evidenced fault is listed in its own name
  at warmup and control grain as at measured grain:
  an unplanned warmup record's evidenced output
  mismatch and descendant survival, and an unplanned
  control record's evidenced output fault, stand in
  the standing losses under their evidenced
  predicates, never as competitor evidence and never
  nowhere; round 41's fall-throughs carried the
  measured-kind gate of the evidence branches they
  mirror, so a duplicate warmup record carrying a
  fault rendered a standing-loss list byte-identical
  to a faultless duplicate's, and no branch reached a
  floor arm's output fault at all. The planned
  population's kind discipline is untouched. The
  regressions stand in `Round42LandingTests`.
  Round-43 F2: the own-name listing is path-complete —
  every unplanned record's evidenced fault is listed
  in its own name at every path as at every kind, and
  a planned product record's evidenced non-kill-path
  survival is listed in its own name beside it: a
  product arm's evidenced fast-path descendant
  survival stands in the standing losses under its
  evidenced predicate at planned and unplanned grains
  alike, never as competitor evidence and never
  nowhere; round 42's competitor survival fall-through
  kept the competitor-loss definition's path == "kill"
  condition, so that shape stood in no fault listing
  in 9 of 9 enumerated (arm x kind) cells while a
  floor arm's identical record was listed in its own
  name. The one stated exception stands: a planned
  warmup/control kill-path survival mints no listing,
  under the planned population's kind discipline.
  The regressions stand in `Round43LandingTests`.
  Round-36 F2: the note accounting is PER FIELD — the
  full pairing, read off the runner's write sites, is:
  a peak_working_set_bytes null is accounted by
  "GetProcessMemoryInfo failed on this arm" beside a
  numeric cpu_ms (the ONE single-null shape the runner
  writes), or by any of the both-counters notes with
  cpu_ms also null; a cpu_ms null is accounted ONLY
  with peak_working_set_bytes also null, under one of
  the both-counters notes — "GetProcessTimes failed on
  this arm", "resource counters unavailable: " followed
  by the error text, "arm not spawned yet; no process
  accounted", "arm never spawned; no process to
  account", "arm not reaped; resource counters
  unavailable", or "platform exposes no per-child
  resource accounting the runner can read (no wait4)".
  A null in any other note/field shape is UNACCOUNTED,
  named in the inconsistency channel, on warmup and
  control records as on measured ones. Round-36 F4: the
  censored classification counts accounted nulls only —
  a null counts toward a unit's censored count only
  when its key is present and the null is accounted FOR
  THAT FIELD under the pairing above; an unaccounted
  null and a missing key render as what they are, and
  no surface (profile detail, ledger basis, standing
  loss, render) describes a unit as censored on records
  whose nulls are not accounted. The regressions stand in `Round31LandingTests`
  (the designated arm mutated to cpu ×3 + 1 and peak ×3
  on every measured record FAILs the dimension, where
  coverage alone passed it; the honest tie fixture and a
  cheaper designated arm PASS) and in
  `Round32LandingTests` (the round-32 reviewer's four
  probes: rep-permutation invariance, per-cell absence,
  total-cost sensitivity, variant resolution) and in
  `Round33LandingTests` (the round-33 reviewer's
  cardinality pair, minority-sample, tail and
  render-absence probes) and in `Round34LandingTests`
  (the round-34 reviewer's peak-mean, quantile and
  exactly-half constructions, with the round-33 peak
  regression corrected to mint the loss it was blind to) and in
  `Round35LandingTests` (the round-35 reviewer's
  censoring, note-vocabulary, sparse-censoring and
  loss-magnitude constructions) and in
  `Round36LandingTests` (the round-36 reviewer's
  fully-censored-cell, per-field-pairing, plan-budget
  and censored-classification constructions).
  The network, monetary and persisted-storage sub-items remain
  unperformed by every arm, and no energy instrument is
  carried; no claim is made for those sub-items.
- **(4) Scaling, concurrency, and resource exhaustion —
  collected (round-26 F4; the round-8 decision is
  superseded).** The plan now carries tree-scale variation as
  plan parameters: scaling slots at widths 8 and 72 (bracketing
  the 64-member teardown window the failure tree names) and
  depths 4 and 40 (the in-tree deep chain's size), driven for
  all four arms in both contexts through the campaign's
  existing kill machinery — the scale child builds the named
  tree, the arm's timeout code is expected, and the shipped
  survivor probe judges every published tree PID. The level
  conventions, stated exactly (round-27 F5): the in-tree deep
  chain is 40 levels starting at level 1 (the tree's battery
  B35 runs testkit "deep 1 40" and asserts 40 levels), and the
  campaign's deep chain is aligned to that convention — its
  child starts at level 1, so deep-D builds levels 1 through
  D, D processes in all; a wide-N tree is one root publishing
  N descendant PIDs. The published count is held to the
  variant (round-27 F1): a scale record must publish exactly
  N (wide-N) or D (deep-D) descendant PIDs, in the runner's
  classification and in the checker's ok-record rule alike;
  under-publication is an infrastructure failure, never "ok".
  Round-29 F4: the distinct-PID count is variant-aware in
  both places — for wide variants the published set excludes
  the arm root and the child root the runner records
  separately; for deep variants the published list carries
  the level-1 root itself (child_pid is None in the runner's
  deep records), the count is the list's distinct
  cardinality, and a root also named in child_pid neither
  adds nor subtracts.
  Round-30 F1: the count is ONE predicate in both places —
  `plan.scale_count_matches`, called by the runner's
  classification and by the checker's count hold alike. It
  binds BOTH terms, per variant: the published list's LENGTH
  equals the variant's raw published count (wide-N: N — the
  ids file carries the N descendants, the root recorded
  separately in child_pid; deep-D: D — the per-level files,
  level 1 included) AND its distinct-after-exclusion
  cardinality equals the variant count (wide excludes
  arm_pid and child_pid; deep excludes arm_pid alone). An
  excluded root published beside the full tree — wide-8
  carrying [root] + 8 genuine kids (9 PIDs), deep-4 carrying
  the arm PID as an extra level (5 PIDs) — is therefore an
  infrastructure failure in the runner and an inconsistency
  in the checker, by the same numbers; before round 30 the
  runner bound only the distinct term and classified both
  constructions "ok" on records its own checker condemned.
  Round-30 F2 (totality, completed at the recomputation
  level): the pricing producers name extreme values in the
  existing defect vocabulary instead of raising (a 10**400
  elapsed is unpairable in `prices_from_observations`; a
  10**400 outer bracket is a named `outer_elapsed_ms` stamp
  defect in `pricing_stamp_defects`); `_pricing_items`
  resolves an extreme ms as "not measured" and derives
  nothing from extreme paired differences; every aggregation
  in the outcome / render / freeze-gate / decompose chain
  admits only plausible elapsed values (finite, non-negative,
  non-boolean) into its medians — extreme values are excluded
  by that stated rule and named by the harness's shape
  discipline on the record itself, so an even-count cell of
  two 10**400 values renders a state, never an OverflowError;
  and `slot_accounting_losses` names a record whose rep is
  unhashable instead of crashing on it.
  Round-33 F1: the variant a scale record is held to is
  the one its SLOT encodes in the plan, on every consumer
  — `_scale_variant_of`, this dimension's count hold and
  dimension (3)'s cell key all read the slot-derived
  variant, and a carried variant label that disagrees
  with the slot's encoded shape is named as a variant
  record inconsistency on the scale path exactly as on
  the malformed path. This reverses the round-32
  landing's narrowing of the variant hold to the
  malformed path, which rested on the suite's scale test
  constructions carrying a dummy rep; those constructions
  are repaired as construction-truth (each scale Slot now
  carries the rep its variant encodes). The round-33
  reviewer's relabel probe — every measured wide-72
  record relabelled "wide-8", publishing 8 of its 72
  PIDs, rendering (4) PASS, (3) PASS and a COMPLETE
  campaign — is the regression, in `Round33LandingTests`.
  The derivation is recorded in `campaign/plan.py` and in
  `campaign.json`'s entry. Round-34 F2: rep
  well-formedness is held in the verdict layer itself —
  a scale record whose rep is not a non-boolean integer
  within its kind's band for the path (a float twin of a
  genuine rep, which every identity Counter reads as the
  genuine slot while the derivation reads nothing, a
  boolean, a negative or out-of-band rep) is named as a
  rep record inconsistency by the record-consistency
  discipline itself, exactly as on the malformed path;
  the derivation deriving nothing is itself the finding,
  never a silent skip of a hold, and a negative rep
  derives no variant (the plan checks the band before
  indexing, so negative indexing never resolves a rep to
  another variant). The round-34 reviewer's float-form
  relabel probe — every measured wide-72 record carrying
  its float-twin rep, the "wide-8" label and 8 PIDs,
  rendering (4) PASS, (3) PASS and a COMPLETE campaign —
  is the regression, in `Round34LandingTests`. Round-35 F2:
  the rep hold is now ONE verdict-layer hold over every
  record kind (fast, kill, malformed, scale and control
  alike) — round 34's variant-path scope rested on the
  premise that slot accounting names the other paths'
  reps, which the round-35 review executed as false for
  the float/bool twins (a float rep on a fast record and
  a bool rep on a kill record were named by nothing in
  the verdict layer and fed every median); the twins are
  the regressions, in `Round35LandingTests`.
  Concurrency of invocations and
  resource exhaustion beyond tree scale remain unmeasured for
  any arm, and no claim is made for them.
- **(5) Reliability, prolonged operation, recovery, and data
  integrity — decided.** Slots are independent short
  invocations; the campaign has no soak period, no
  crash-recovery scenario and no dataset whose integrity could
  be compared. Per-invocation reliability is already what
  dimensions (1) and (2) observe through slot outcomes.
  Leftover-descendant behaviour is observed, but under
  dimension (1) as kill-path correctness, not here: an arm
  that leaves descendants running after it exits —
  nuewframe/timeout, which kills no descendants, is the case
  in this campaign's kill facts — is recorded there as a
  competitor loss. That observation is a per-invocation
  kill-path observation, not a prolonged-operation or
  recovery measurement, and it supplies none of the soak
  period, recovery scenario or comparable dataset this
  dimension would need; prolonged operation, recovery and
  data integrity are claimed for no arm. Distinguishing
  statement (round-32 F5): this dimension is
  NOT-APPLICABLE BY ADJUDICATION -- round 26's executed
  dimension adjudication decided it away on the scope
  grounds above, re-confirmed in round 31 — and that
  status is a scope decision, not a verdict the campaign
  computes: it is distinct from dimensions (3) and (6),
  which are COLLECTED dimensions whose states are computed
  from the run's records (their verdict semantics were
  fixed in round 31 and rebuilt in round 32 — an
  arm-comparison verdict for (3), designated pinned
  expectations for (6)); no such computation stands behind
  this entry, and none is claimed.
- **(6) Security boundaries and resistance to malformed inputs
  — collected (round-26 F1; the round-8 decision is
  superseded).** The campaign now collects the shared shapes:
  the plan's malformed-input slots present the six shapes the
  old decision named — a nonexistent program, a malformed
  timeout, an out-of-range timeout, an empty command, an
  over-long argument, a non-Unicode argument — identically to
  all four arms through the arms' own templates
  (`runner.malformed_argv`; the reviewer's executed proof that
  the template machinery produces them is in round 26's
  verdict), with each arm's exit code recorded per slot as the
  observation and the dimension's state computed from those
  records. Round-27 F4: each malformed record is held to that
  code — present, and an integer; no code is pinned for the
  path, so a missing or untyped code is an inconsistency, the
  record carrying no observation at all. Round-31 F2: the
  designated arm's code is an expectation, not only an
  observation, for the shapes its own contract pins —
  `plan.malformed_expected_exit` pins expected_exit = 125
  for the designated arm only, for the nonexistent-program,
  malformed-timeout, out-of-range-timeout and empty-command
  shapes (the guard's documented exit-code contract: 125 is
  the guard error for invalid arguments and for executable
  not found) and for the non-unicode-argument shape (the
  guard's argument rule refuses any argument that is not
  valid Unicode with the same guard error, pinned at 125 by
  this suite's F9 family in every position). The pinning
  activates the existing exit-code rules for the designated
  arm — the runner classifies a designated slot answering a
  pinned shape with another code "exit-mismatch", the
  checker holds measured designated records to the pinned
  code, and the dimension's PASS requires the designated
  arm to meet its pinned expectations on its measured
  malformed records: a designated exit 0 on a pinned shape
  renders the dimension FAIL with a standing loss, where
  before the pin it rendered PASS identically to 125.
  Competitors keep expected None for every shape — their
  codes are observations of any value and move nothing —
  and the over-long-argument shape remains unpinned for all
  arms, the designated arm included: the guard's contract
  states no argument-length limit, so that shape's recorded
  code remains the observation itself. The reviewer's
  regressions stand in `Round31LandingTests`. Round-32 F2:
  the pin binds the shape as encoded by the PLAN — the
  slot identity (path, rep, kind) whose rep numbering is
  variant-major — never the record's self-carried variant
  label: a carried label that disagrees with the slot's
  encoded shape is named as a variant record inconsistency,
  and the checker derives the pinned shape from the slot
  alone, so relabelling a violating record to the unpinned
  shape (or to a garbage label) moves nothing. The pin's
  reach, stated exactly (round-33 F2 rider, correcting
  the round-32 landing's shared-formulation phrasing):
  the checker calls `plan.malformed_pin_violation`
  directly — in the dimension's violations computation
  and in the record-inconsistency hold — with the
  plan-derived variant; the runner never calls that
  function — its classification reaches the same
  expectation table through `arms.expected_exit`, which
  delegates to `plan.malformed_expected_exit` with the
  slot's variant. One expectation table, two readers,
  over warmup records and measured records alike: the
  runner faults a designated warmup slot answering a
  pinned shape with another code as "exit-mismatch", and
  the checker holds warmup records to the pin on the same
  numbers — the round-32 reviewer's relabel and warmup
  probes are the regressions, in `Round32LandingTests`.
  Round-33 F2: a malformed record's argv CONTENT is
  held against its slot's encoded shape as well — the
  record carries two encodings of the presented shape
  (the rep, bound by round-32 F2, and the argv, the
  executed vector closest to the execution itself), and
  before this landing the argv was held for form only.
  The content predicate (`report.malformed_argv_presents`)
  is derived from the arms' own presentation templates —
  the shape-distinguishing content `runner.malformed_argv`
  writes for each shape: the nonexistent program token,
  the malformed and out-of-range timeout texts, the
  empty child token, the 4096-character argument's
  length property, the non-Unicode argument's encoding
  property — and a record whose argv presents another
  shape is named as an argv record inconsistency in its
  own right, whatever its exit code answers: the
  round-33 reviewer's transplant probe (designated
  nonexistent-program records carrying the genuine
  over-long-argument argv, rendering (6) PASS and a
  COMPLETE campaign) is the regression, in
  `Round33LandingTests`, beside a predicate-agreement
  test binding the predicate to `malformed_argv`'s
  genuine output for every shape and arm. Round-34 F2:
  rep well-formedness is held in the verdict layer
  itself — a malformed record whose rep is not a
  non-boolean integer within its kind's band for the
  path (a float twin of a genuine rep, a boolean, a
  negative or out-of-band rep) is named as a rep record
  inconsistency by the record-consistency discipline
  itself, on this path exactly as on the scale path;
  the round-34 reviewer's float-form pin evasion
  (designated nonexistent-program records carrying
  float-twin reps, the unpinned shape's label and exit
  0, rendering (6) PASS with zero violations and a
  COMPLETE campaign) is the regression, in
  `Round34LandingTests`. Round-34 F3: the argv hold is
  two conjuncts — the vector must present its slot's
  encoded shape AND no other shape: a vector presenting
  another shape's distinguishing marks beside its own
  is named as an argv record inconsistency for every
  shape it is not; the round-34 reviewer's hybrid probe
  (a genuine nonexistent-program argv extended with the
  other five shapes' marks, presenting all six shapes
  at once and named by nothing) is the regression, in
  `Round34LandingTests`, and the genuine-argv pairwise
  matrix stays clean (0 confusions of 144 under the
  shipped configuration, as under the stub). Round-35 F2: rep
  well-formedness is one verdict-layer hold over every
  record kind (see dimension (4)'s statement) — the
  float rep on a fast record and the bool rep on a kill
  record are its regressions, in `Round35LandingTests`.
  Round-35 F3: the argv hold binds EXACTNESS in the same
  ONE-vector item — a vector carrying content no
  construction wrote for its slot (an unmarked junk tail
  appended to a genuine vector, presenting its own shape
  and no other) is named beside substitution and hybrid:
  the exactness derivation holds the vector's length
  against the arm's own fast vector (the timeout shapes
  substitute in place, the nonexistent and empty shapes
  drop the child arguments, the two mark shapes append
  exactly one token) and its closing token against the
  shape's construction mark; the round-35 reviewer's
  junk-tail probe (genuine argv plus three unmarked
  tokens, named by nothing in any layer) is the
  regression, in `Round35LandingTests`, and the genuine
  pairwise matrix stays clean (0 confusions of 144).
  Round-36 F3: the length budget the exactness
  derivation holds is anchored to the PLAN — the
  expected fast-vector length per arm derives from the
  run's own argv templates expanded through the slot
  construction (the shipped configuration's templates:
  guard 7, processkit 6, nuewframe 5, sysprims 6 tokens
  including the executable token), never from a
  statistic of the record set under judgment: the
  round-36 reviewer's uniform extension (the same three
  junk tokens on every designated fast record) moved
  the round-35 modal budget 4 → 7 and its junk-tailed
  timeout victims were named 0/8; under the plan anchor
  they are named 8/8 again, the genuine pairwise matrix
  stays clean (0 of 144), and both perfect fixtures
  stay clean. Which plan binds follows the
  expected-output discipline: the run's own
  configuration where a caller supplies it (a package
  recomputation derives it from the anchored
  configuration the manifest names and hashes to), the
  shipped configuration for a bare record set, and no
  budget at all where no verified plan exists for the
  records (a preserved package whose configuration
  anchor is absent — the length limb does not fire, and
  the absent anchor stands named unverifiable), and a
  configuration states a length only where it states
  the construction itself — the arm's own fast template
  and the fast child spec's arguments; a configuration
  carrying neither (a manifest anchor written for
  another derivation, the expected bytes) states no
  length, and the checker invents none from defaults
  another plan carries. The
  regression is in `Round36LandingTests`. Round-37 F2/F3:
  an arm with no derivable budget is NAMED, never silent
  — round 37's reviewer executed the dodge the round-36
  refinement left unnamed: a junk-tailed package under a
  minimal anchored configuration (arms {}, expected
  bytes only) certified COMPLETE at the package layer
  with zero problems, its 8 timeout victims named nowhere.
  Now every malformed record judged under a configuration
  that derives no budget for its arm names that arm's
  budget as underived, with the plan fact stated (which
  part of the construction the configuration does not
  state), so the verdict layer and the package layer both
  block COMPLETE for the affected shapes; and where no
  verified plan exists at all, the package checker's
  degraded-verification list names the argv length budget
  among the verifications the absent anchor degrades
  (item 3's set is five, not four). The regressions stand
  in `Round37LandingTests`. Round-38 F2/F3: the sentinel
  itself may not certify — round 38's reviewer executed a
  junk-tailed set under NO_VERIFIED_PLAN rendering
  COMPLETE at the verdict layer, and a junk package under
  an absent anchor printing a complete STATUS in the
  report consumer while its checker named five problems.
  Now a set judged under the sentinel renders dimension
  (6) UNEVALUATED, degraded-named in the profile detail
  and the ledger basis, whenever it would otherwise PASS,
  so `campaign_outcome` cannot render COMPLETE and the
  report consumer's STATUS agrees with the package
  checker's problems on the same package; a dimension
  already UNEVALUATED under the sentinel renders exactly
  as before (the reduced package's outcome, its standing
  losses and its degraded set do not move). And the
  budget-gap fact now reads the ONE derivation's own
  failure: the component that actually fails is the
  component the naming states, never a stated component
  reported absent. Round-39 F1 restates that derivation
  to its per-limb truth: a length derives from the
  count of the expanded vector; the program and args
  limbs are token counts, type-independent, so a
  non-string program or a non-string args element
  derives the same length a string token derives and
  is never named underived for its element type; the
  template limb alone can fail to expand — a
  non-string template token, which the expansion's own
  string tests refuse, is named present-but-unusable —
  and a component that is absent (no template, no
  child spec, no args, no program), or stated in a
  form the construction cannot read as a whole (a
  child spec that is not a mapping), is named as the
  absence or the unusable whole it is. The round-38
  form's contrary breadth on the program/args limbs
  (a non-string program or args element deriving no
  length, minting 192 budget-gap items over the
  genuine fixture for a budget of 7/5/6/6 by counting)
  is withdrawn. The regressions stand in
  `Round38LandingTests` and in `Round39LandingTests`.
  Round-40 F4 restates the derivation to the
  construction's full tokenization breadth: the
  derivation mirrors the construction's tokenization
  for every config shape the construction accepts —
  an args value stated as a string tokenizes per
  character and as a dict by its keys, under the
  list() coercion the runner applies on every path;
  a template stated as a string tokenizes per
  character under the iteration the runner's resolve
  applies; and sequence tokens appended verbatim by
  the expansion (a list or tuple template token)
  derive as the one token the construction appends —
  for placeholder-free sequences only (round-41
  restatement of this statement's breadth): a sequence
  token carrying a timeout placeholder raises on BOTH
  sides — the derivation names it a gap at that token
  and the construction raises in the same expansion —
  so the two sides agree in refusal as in derivation.
  A length derives wherever the construction builds
  a vector; the gap fact names only shapes where the
  construction itself cannot build one — a
  non-iterable template or args value, a template
  token the expansion's own string tests refuse. The
  round-39 form's whole-value sentence (a template
  or an args value that is not a list named an
  unusable whole) is withdrawn for those forms: the
  reviewer's four shape constructions each derive
  the built length (12/11/10/11 for string args,
  7/6/5/6 for dict args, 10 for a string template, 6
  for sequence tokens) and mint no underived items,
  and genuine configs derive exactly as before
  (guard 7, processkit 6, nuewframe 5, sysprims 6).
  The regressions stand in `Round40LandingTests`.
  Round-41 F3: arm identity is held against the
  record's second encoding — the executed vector's
  executable token must not be another product arm's
  configured executable — so a pairwise arm-label swap
  (guard<->sysprims fast, processkit<->sysprims
  malformed), which round 40 rendered with slot
  accounting exact, no inconsistencies, standing
  losses 0 and every gate dimension PASS over two
  wrong binaries, is named in the argv channel and
  cannot render standing losses 0; a token that is no
  arm's configured executable states nothing about
  which arm ran and is not named. Round-41 F5:
  variant absence is named — a malformed or scale
  record whose variant is missing, null, empty or
  untyped renders a variant item naming the absence,
  because the runner stamps the slot's encoded shape
  on every such record it writes; round 40 named only
  a disagreeing label, so variant=None on every
  malformed record yielded no variant item at all and
  this dimension and dimension (3) PASSed over 192
  erased labels — now the absence renders variant
  items and standing losses, and no verdict passes
  over an unnamed absence. Round-41 F4: the
  derivation never consumes a one-shot iterator — a
  configuration value stated as a generator is
  materialized once at the read the derivation and
  the runner share, and the materialized list is
  written back into the configuration object — so
  repeated derivations on the same object agree
  (generator args derive 7 then 7, a generator
  template 5 then 5) and the runner's child_for sees
  the full vector after any derivation; round 40's
  list() coercion read a generator to exhaustion and
  left the runner building an empty vector from the
  same object the derivation had just budgeted from.
  The regressions stand in `Round41LandingTests`.
  Round-42 F1: the arm-identity comparison is held
  under the platform's path semantics — the executed
  vector's executable token is canonicalized (case
  folded, separators unified, current-directory and
  root prefixes dropped, an absolute spelling of the
  same binary agreeing with the configured relative
  spelling), so a pairwise arm-label swap whose kept
  tokens are respelled (uppercase, forward slashes, a
  ./ prefix, an absolute path) is named in the argv
  channel and cannot render standing losses 0, the
  exact-token control still renders 2 items / 2
  losses, a respelling of the filed arm's own
  executable agrees, and a token denoting no arm's
  executable under that identity states nothing about
  which arm ran and is not named. Round-42 F3:
  materialization never requires a mutable mapping —
  a configuration carried in mutation-refusing
  mappings (a MappingProxyType, a frozen dict) derives
  and runs as the as-found construction accepted it,
  repeated derivations agreeing (no 5-then-1) and
  nothing crashing, the materialized list carried by
  an identity cache on the suite's side where a
  write-back is refused; a shape that cannot be
  materialized identically on both sides names its
  gap on both sides. Round-42 F4: the write-back is
  confined to the values that need it — only a
  one-shot iterator is materialized once; a value
  already materialized or re-iterable (a string, a
  dict, a tuple) is read locally and never rewritten
  in place, so a derivation cannot repair or alter
  the configuration a second consumer judges, and
  the shipped configuration remains byte-identical
  after a full report run. The regressions stand in
  `Round42LandingTests`. Round-43 F1: executable
  identity is decided on canonical component
  sequences — the canonicalization extends to the
  deterministic Win32 forms of the landed class
  (trailing dots and spaces stripped from every
  component, an omitted final extension completed),
  and identification requires equality of the
  canonical sequences after dropping only the
  disclosed prefix forms (an absolute spelling, the
  device prefixes): a proper suffix of the configured
  sequence (a basename only, a partial tail) never
  identifies the campaign's binary, and a genuinely
  different directory never agrees. The 8.3 short-name
  class stands beside parent-relative (..) components
  as a named limit: resolving either needs state the
  record set does not carry. Round-43 F3: every
  consumer of the child spec's args reads through the
  shared materialization, pricing included — the
  pricing probes' two raw list() reads of the fast
  child spec are routed through
  arms.materialized_child_args, so pricing-first,
  derivation-first and runner-first orders all agree
  on the same configuration object. Round-43 F4:
  materialization is keyed on one-shot-ness of the
  value, not iterator type identity — any value that
  can be consumed only once (a one-shot iterator, a
  one-shot iterable, a payload-carrying subclass, a
  factory mapping's minted values) is materialized
  once at the shared read whatever its type, while a
  provably re-iterable value is still never rewritten;
  the round-42 confinement's dichotomy is restated to
  the flow's unit, and the one-shot iterable class's
  as-found agreement (a round-42 regression) is
  restored. The regressions stand in
  `Round43LandingTests`. Round-44 F1: the trailing
  strip is confined to components carrying other
  content — a component made solely of dots and spaces
  canonicalizes as itself when it is exactly "." or
  "..", and otherwise stands in the canonical sequence
  verbatim, so no padded traversal spelling (".. ",
  ".. .", an all-dots component) can satisfy the
  identity hold: the `..` disclosure above is restated
  at this breadth — parent-relative components, the
  plain ".." and every padded spelling of it, stand
  unresolved in the canonical sequence. Round-44 F2:
  the shared-read discipline binds admission_failures —
  the pre-flight read of any spec value (a child
  entry's args, an arm's template, an identity's
  version_args, the pricing probes' guard_launcher,
  the survivor_checker) goes through the shared
  materialization (arms.materialized_list), as do the
  pre-flight's own version probe chain (arm_versions
  and arms.probe_version), the runner's survivor probe
  and the pricing launcher reads, so admission-first
  order agrees with every other order. Round-44 F3:
  the spec-slot cache holds a factory mapping's minted
  values at the spec-slot unit for any Mapping type,
  not only dict-carried specs — a non-dict Mapping
  factory spec derives and runs with its minted values
  held and repeated derivations agree, so the
  round-43 statement above ("a factory mapping's
  minted values ... whatever its type") is now true
  at its stated breadth; a minting mapping has no
  storage view, so the slot holds its first
  materialization for that spec object, as
  materialized_child_args states. Round-44 F4: the
  two uses of the one-shot test are separated — the
  cache may keep the conservative third disjunct of
  is_one_shot_value ("not provably re-iterable"), but
  write-back (the derivation's two limbs, the runner's
  construction, template resolution) fires only for a
  value proven one-shot (a value whose traversal
  cannot restart by construction — a built-in one-way
  iterator, or an iterable handing that same one-way
  iterator back; the breadth as repaired by round-45
  F3 below); a merely unprovable value is never
  rewritten in place by any consumer, and a genuinely
  re-iterable custom value survives all three
  consumers unrewritten, its own object and live
  backing intact — what every consumer builds from
  it is the one shared read's first materialization
  (round-45 F5 below). The
  regressions stand in `Round44LandingTests`.
  Round-45 F1: the stated-vs-default version_args
  choice is made under the materialization
  discipline — arm_versions chooses by the entry's
  stating the field, never by a truthiness or length
  judgement over the stated token-list value, and
  probe_version receives the materialized tokens, so
  a payload-carrying version_args stating a
  non-default payload is probed as stated in both
  directions: a confirmable identity confirms, and a
  stated-args refutation is carried as a probed-
  version mismatch. Round-45 F2: the factory
  signature is widened from iterators-in-fact to
  one-shot values — the spec-slot cache serves a
  factory mapping whose accesses mint fresh one-shot
  values over one shared source (one-shot iterables
  handing back one shared iterator included) under
  the flow's own one-shot test, is_one_shot_value,
  applied to the stored and the fresh fingerprint
  alike; the iterator-minting carrier is unmoved.
  (Round-46 F1/F2 below restate this serving at its
  landed breadth: the signature's kind test is
  superseded by the materialization-access serve and
  the identity-across-accesses distinction.) Round-45 F3: write-back fires
  only where a second traversal cannot yield the
  full sequence again by construction — the proven
  limbs are confined to built-in one-way iterators
  and to iterables handing that same iterator back,
  so a restarting iterator (an iterator in fact
  whose __iter__ resets its traversal over a live
  backing) is never rewritten by the derivation, the
  runner or template resolution; this breadth is
  stated directly here, not by defining proof as its
  own limbs. Round-45 F4: classification never
  leaves a value worse than one honest read leaves
  it, at the breadth round-46 F4 below states
  exactly (the one class named there pays one
  further iter() call after capture) — every
  consumer classifies on materialized
  evidence: the read path materializes with the one
  iter() call an honest read makes and derives the
  proven classification from that capture (at most
  one further iter() call, made after the tokens are
  captured, whose spending cannot reach them), so a
  handshake value, whose iter() spends one element,
  renders through derivation and construction
  exactly the tokens one honest single read renders
  — a stated four tokens render three, never the
  empty vector the stacked speculative tests left.
  Round-45 F5: the unproven class has one semantics
  — the runner, the derivation and pricing read
  through a single shared read, so for any single
  configuration object the derivation's budget basis
  and the runner's construction basis are the same
  tokens: a per-call (parity) value builds the
  vector the derivation priced, in both reader
  orders, and an owner's intervening mutation moves
  neither side. The "repeated derivations agree"
  statements in this item are scoped to that
  shared-read class: they hold for every value the
  shared read serves (one-shot and unproven values
  alike — the first materialization, held
  identically for every consumer) and, for provably
  re-iterable values, at any moment the owner's
  state is unchanged, every consumer reading that
  state live alike. The regressions stand in
  `Round45LandingTests`. Round-46 F1: the spec-slot
  cache's serve decision is keyed on the
  materialization access — the access that drains —
  not on the fingerprint access's kind: the slot
  serves its first materialization on a later read
  exactly when that read is itself a minting read
  and the stored entry was itself taken on a
  minting read, so for any factory whose accesses
  mint over one shared source in any mix of kinds
  (a plain list on one access, a one-shot iterable
  on the other included), repeated reads agree
  whenever an access mints a draining one-shot
  value, and no slot entry is stored under a
  fingerprint the serve test cannot recognise (the
  serve test recognises no fingerprint's kind at
  all). Round-46 F2: the slot distinguishes storage
  from minting by an observable in the flow's own
  terms — identity across accesses: a storing
  mapping returns the identical object on
  successive gets, a factory mints a fresh one —
  so a storing Mapping's replacement is re-read
  exactly as the dict-carried one is, and a minting
  factory's accesses over one shared source are
  served the slot's first materialization for that
  spec object. Round-46 F3: the slot route stands
  under the F4 discipline — zero speculative
  iter() calls ahead of capture on any consumer
  route: the slot's storage/mint observation is two
  identity comparisons over the accesses the flow
  already makes, and it classifies without calling
  iter(), so no consumer classifies a value ahead
  of reading it on any route, the slot route
  included. Round-46 F4: the proven classification
  derives from the capture itself with zero
  additional iter() calls on the value, except for
  the one class the capture itself identifies and
  one capture cannot decide — a value that is not
  an iterator in fact whose capture traversal is a
  built-in one-way iterator, whose same-iterator
  limb is a property of a second call by definition
  (a re-iterable twin yielding through a fresh
  traversal of the same type is indistinguishable
  from it in one capture): for that class alone the
  classification makes one further iter() call,
  after the tokens are captured so its spending
  cannot reach them, and that call can spend
  whatever one iter() call on such a value spends —
  on a pop-front reservoir value, one further
  reservoir element, an element no captured token
  contained; on a handshake value, one further
  handshake step against the source its capture
  already drained. Every other value's
  classification is read off the capture alone:
  an iterator in fact is judged on the capture's
  own traversal, and an iterable whose capture
  traversal is not one-way is proven False with no
  further call, the limb being unable to hold for
  it whatever a second call returned. This
  statement replaces the round-45 sentence's broad
  form at exactly this breadth. The regressions
  stand in `Round46LandingTests`. Round-47 F1: the
  spec-slot classification stands at the flow's own
  unit, the READ: a read of a non-dict Mapping spec
  makes one access — the materialization access
  itself, which is the read's fingerprint — and
  storage and minting are distinguished from that
  read's capture composed with identity within and
  across reads. The serve condition: a read whose
  value is the stored entry's very value serves the
  entry; a fresh capture differing from the entry's
  tokens serves the entry exactly when it is a mint
  at the read's unit — its capture traversal is the
  stored capture's very traversal (a fresh wrapper
  over the one shared source), or, where that
  identity does not decide, its capture is proven
  one-shot over a new traversal or a plain list
  standing empty against a non-empty entry and one
  confirmation access taken after the capture
  returns a further object rather than the captured
  object itself; a fresh capture meeting neither
  mint limb is storing — the mapping that returns
  the captured object itself on the confirmation
  access states a replacement, re-read as a
  dict-carried replacement is, and a capture that
  is not proven one-shot and not a drained plain
  list (a copying store's copy, each copy yielding
  the stored tokens in full) is storing on its face
  and stands as the read's tokens. The agreement
  claim states the same breadth in the same terms:
  for any factory whose reads mint over one shared
  source in any mix of those kinds — a plain list,
  a proven one-shot value, or a wrapper handing
  back the shared traversal — repeated reads agree,
  and every storing mapping's stated replacement is
  re-read, the identity-returning store's and the
  dict-carried store's as before. A per-read minter
  — a factory that caches its mint across one
  read's accesses and mints fresh for the next
  read — is minting under the traversal identity,
  however its within-read identity stands; a
  copying store is storing under its capture's
  face. Round-47 F2: the read makes no access whose
  mint it discards ahead of capture — one access
  per read, and the derivation judges a spec's
  stating a field on its key set, not on an access
  to the value — so an eager mint, one that spends
  at mint time by draining the shared source to
  build its value, is the very value its read
  captures: a first read renders the configuration's
  vector in either order of kinds, never a drained
  remainder. The round-46 accounting of a discarded
  mint as one that spends nothing is withdrawn: no
  mint is discarded ahead of a capture at all, and
  the one access an ambiguous read makes after its
  capture — the F1 confirmation access — spends
  its mint, if it mints, only over a source that
  read's capture already holds. Round-47 F3: the
  named class's cost is restated at its raising
  face — the one post-capture iter() call can
  raise as well as spend, and its failure is
  absorbed as unproven for any exception, not only
  TypeError: a member whose second iter() raises
  RuntimeError renders its captured tokens through
  the derivation, the construction and template
  resolution alike, exactly as the TypeError member
  does; where the call raises, the classification
  is forgone and the read stands, and the call can
  no longer abort any consumer. The regressions
  stand in `Round47LandingTests`. Round-48 F1: the
  confirmation access is abolished — no
  classification verdict rests on any access beyond
  the reads' own captures, at any depth, so no
  decision access can return the captured object by
  memo, return a wrapper around it, raise into a
  verdict, or spend over a source no capture held.
  Where neither identity decides (a proven one-shot
  capture over a new traversal, or a plain-list
  capture standing empty against a non-empty entry)
  the verdict is DEFERRED: the ambiguous read serves
  an interim stated exactly — its own capture's
  tokens when the capture is non-empty, the stored
  entry's tokens when it is empty — the candidate
  capture is held pending beside the retained entry,
  and the following read's own capture settles
  storing-vs-minting from captures already taken: a
  fresh value reading the candidate's very traversal
  settles storing and the candidate becomes the
  entry; a capture reading the entry's very
  traversal, stating the entry's tokens, or
  reproducing the candidate's empty tokens with a
  fresh value settles minting and the entry stands,
  held as settled against later ambiguous captures
  on the same following-read evidence; a fresh value
  reproducing the candidate's non-empty tokens
  settles storing — a re-stated vector, since mints
  over one draining source yield successive
  remainders, never the same non-empty tokens twice
  — and the reproducing capture becomes the entry; a
  return of the candidate's very object settles
  nothing and the interim stands. A raising access
  at any decision point classifies UNDECIDABLE:
  there is no decision access left to absorb one, a
  raise at a read's own capture access propagates
  exactly as one honest read propagates it, no
  verdict is taken, and nothing is silently a
  minting verdict — the round-47 F3 absorption
  stands untouched at its own site, the
  classification call inside the capture, which is
  not a decision site. The cost accounting states
  the same breadth: every read spends exactly its
  own one materialization access, over exactly the
  source that read's capture holds, and the decision
  spends no access at all. Round-48 F2: the breadth
  is stated at one breadth here and in arms.py's
  slot docstring alike — the kinds are a plain list
  minted over the one shared source, a proven
  one-shot value minted over it, and a wrapper
  handing back the one shared traversal, beside
  every storing mapping — and one carrier stands
  outside that breadth in both statements alike: a
  fresh-traversal mint, a factory whose every
  iter() call recreates its traversal over the
  shared source so no traversal object is ever
  handed back twice, is held by neither statement;
  its captures are not proven (each classification
  call hands back a fresh traversal, and a raising
  second call is absorbed as unproven exactly as
  the named class's is), so its reads stand on the
  storing face — the first capture in full, then
  the drained remainder the source is left with.
  Round-48 F3/F4: the derivation makes no discarded
  access at all — presence is judged over the same
  materialized evidence the construction builds
  from, never over the spec's key set: no iteration
  of the spec's keys and no contains access over
  the spec is spent (the round-47 key-set judgement
  and its raising-iteration contains fallback are
  withdrawn, so a mapping whose key iteration
  raises is judged on the shared read's evidence
  alone, and an eager mint is never drained ahead
  of the capture the derivation prices); the args
  judgement is the shared read's own stated element
  and the program judgement is the construction's
  own program access, whose value the expansion
  uses, so a value the shared read serves is stated
  and no stated field is named unstated because its
  key set was one-shot, unlisted or unreadable; and
  the derivation's TypeError handler names the type
  of the value the read failed on, captured at the
  failing read and carried by the error itself —
  no second args access exists in the handler, so
  a minting spec's second mint is never named for
  the first. The regressions stand in
  `Round48LandingTests`. Round-49 F1: an empty stated
  value is a value — a pending empty candidate's very
  object persisting across following reads settles
  storing and the empty replacement becomes the entry
  (the dict route serves a stated [] on the assignment
  read itself; the non-dict route serves it once the
  object's persistence settles it), and a fresh value
  reproducing the candidate's empty tokens settles
  minting only on a proven one-shot capture, an
  unproven fresh empty value standing as the new
  candidate instead, never settled minting against
  itself. Round-49 F2: settlements bind the entry
  they judged — a minting settlement admits an
  ambiguous capture as a candidate again on the same
  deferred terms, so the re-opening is reachable in
  both directions, and a storing-face entry change
  retires the settled flag with the entry it judged.
  Round-49 F3: the UnusableValueTypeError translation
  is confined to the capture step (iter(value)); a
  TypeError raised during traversal propagates as
  itself, so raise propagation is identical to one
  honest read for every exception type. Round-49 F4:
  a deferred verdict is settled by reads, never by
  writes — the proven-gated write-backs (the
  derivation's and the runner's alike) are gated also
  on no candidate pending on the slot, so the interim
  candidate is never planted into the caller's spec
  and the materialized-only and consumer-first
  sequences over the same factory reach the same
  settlement. The regressions stand in
  `Round49LandingTests`. Round-50 F1: one rule for
  the memo observable — no property of the returned
  object separates a storing empty value's
  persistence from a memoizing mint's return of its
  last mint, so the empty persistence settlement
  stands only against an entry no minting settlement
  stands on: a candidacy that displaced a standing
  minting settlement finds an empty candidate's
  object returns neutral, exactly as a non-empty
  candidate's are, and a settled minting is never
  flipped on object persistence alone (an empty
  replacement stated over a settled minting
  displaces only on a fresh capture reading the
  candidate's very traversal); the pure storing
  empty assignment, stated over an entry no minting
  settlement stands on, still displaces on its
  object's persistence on both routes. Round-50 F2:
  the gate is judged on the state the consumer's
  read found, not the state it left — both
  proven-gated write-backs consult no-pending before
  their own read as well as after it, so a read that
  settles a candidate plants nothing in its own
  call, and the consumer-first and materialized-only
  sequences over the same factory reach the same
  settlement and serve the owner's replacement
  identically. The regressions stand in
  `Round50LandingTests`. On this Linux machine the non-Unicode shape names
  its presentation limit honestly (the platform's exec
  encoding refuses a lone surrogate; the record is an
  infrastructure failure naming that, and the shape presents
  as written on Windows). The guard-only part of the surface
  (hook envelope, vendor payload — no counterpart in any
  competitor) remains not comparable and stands on the
  non-comparative evidence (this suite's test families —
  including the missing-program, timeout and non-Unicode
  shapes in F1, F4 and F9 — its malformed-payload families
  and 357-case scanner differential, and the tree's own
  tests), not on this freeze gate.
- **(7) Compatibility, portability, installation, and
  integration — decided.** The campaign runs one staged build
  per arm on one frozen platform (the Windows laptop); it
  establishes compatibility there only and claims no
  portability across systems, versions or machines. Integration
  is non-comparative on its own ground: the competitors are
  standalone timeout commands with no hook-vendor surface,
  and the guard's vendor integrations are verified
  non-comparatively by this suite's vendor families.
  Distinguishing statement (round-32 F5): this dimension
  is NOT-APPLICABLE BY ADJUDICATION -- round 26's executed
  dimension adjudication decided it away on the scope
  grounds above, re-confirmed in round 31 -- and that
  status is a scope decision, not a verdict the campaign
  computes: it is distinct from dimensions (3) and (6),
  which are COLLECTED dimensions whose states are computed
  from the run's records (their verdict semantics were
  fixed in round 31 and rebuilt in round 32 -- an
  arm-comparison verdict for (3), designated pinned
  expectations for (6)); no such computation stands behind
  this entry, and none is claimed.
- **(8) Usability and accessibility for interactive products --
  collected (round-26 F2; the round-8 decision is
  superseded).** SPEC_Benchmark §3 frames this dimension at
  the product level ("for interactive products"), and the
  designated product does have an interactive surface -- its
  human-facing approval prompts (the Windows Hello request,
  the credential dialog and the console password prompt, in
  the guard's auth code). The dimension is now collected by
  the instrument that already touched that surface, formalized:
  the manual check in the tree's `CONTRIBUTING.md` ("Manual
  check of the prompts") is recorded in this package as
  `usability-evidence.json` -- one record per prompt class,
  each carrying the boolean exercised flag, what the check
  exercises for that class (its steps), what the operator
  observed, and the outcome per prompt class. Its ledger
  state is computed from that artifact by the campaign code
  (round-27 F2; round-28 F1: an entry qualifies only when it
  names one of the artifact's declared prompt classes,
  carries exercised as the boolean true, records a non-null
  observed outcome, and carries an outcome in the declared
  vocabulary -- pass, fail, not-run -- other than "not-run";
  round-29 F1, reversing the round-28 landing's choice on
  the record): Dimension (8) is three-valued: a qualifying entry (a declared prompt class, exercised as the boolean true, an observed outcome recorded) with outcome 'fail' renders the dimension FAIL -- profile state FAIL, ledger state observed-failing, and a standing loss, and a fail dominates any pass; qualifying entries with outcome 'pass' and no fail render COLLECTED; and no qualifying entry renders UNEVALUATED. The outcome vocabulary stays pass, fail, not-run -- what changes is fail's effect: a fail is collected evidence recorded as the loss it is, never as the dimension satisfied.
  "Collected" is by that named manual instrument, neither
  observed-passing (no campaign slot measures the surface)
  nor NOT-APPLICABLE; while every outcome stands at
  "not-run" the dimension renders UNEVALUATED in every
  ledger, the honest not-yet-collected state. The rule is
  held structurally (round-29 F5): the vocabulary, the
  prompt classes and the collected_rule are compared as
  parsed structures across the campaign code, the
  artifact's schema and campaign.json -- not substrings --
  so rule-reversing text in any of the three fails the
  agreement test. Tier, stated honestly: manual/Windows-theory -- a manual
  check is a manual check; until the ready-to-fly Windows run
  is performed by a person present, every record's outcome
  stands at "not-run" in the artifact, recorded as such.

State of the profile after the round-26 landing: dimensions
(1) and (2) are observed by the freeze gate; (3), (4) and (6)
are collected from the campaign's own records by the
instruments their entries name, and render UNEVALUATED on any
run whose records do not carry the collection — the entries
name the instruments, they do not mint the evidence; (8) is
collected by its named manual instrument at manual tier; (5)
and (7) remain decided away on the SPEC-text grounds their
entries state (round 26's adjudication confirmed both by
name). No dimension is observed on evidence it does not have,
and none that is applicable and presentable is decided away:
a complete campaign is reachable exactly when the guard wins
or ties every gate dimension, the collected dimensions'
records stand complete and clean, and the two documented
scope decisions are the accepted scope — the report and
ledger print every entry beside its dimension, so the claim
is never read as broader than it is. Round-27 carries the
same rule to (8): its state is computed from its artifact's
records, so as the artifact stands (every outcome "not-run")
(8) renders UNEVALUATED, and a complete campaign is
reachable only once the manual check has been performed and
its observed outcomes recorded in the artifact.

[↑ Back to Contents](#contents)

## Windows-theory (by construction, not yet executed anywhere)

- `build.ps1` and `run.ps1` execution (their brace/paren balance
  was checked mechanically here; PowerShell parsing and the MSVC
  build are Windows runs). That includes run.ps1's stage
  handling: a battery FAIL decision (exit 3) recorded while the
  run continues to the test families, a battery execution
  failure (exit 2, an unexpected exit, or no `battery.json`)
  aborting before them, per-stage exit codes recorded, and the
  aggregate failure named only after every stage has run — the
  distinguishing logic is pinned by the suite tests in item 6,
  its PowerShell execution is theory here. That includes
  run.ps1's campaign-outcome handling: reading `campaign_status`
  from the newest `run.json`, recording `campaign status: ...`
  in `stage-exit-codes.txt`, and naming an incomplete campaign
  in the aggregate — the contract between the harness's recorded
  status and the script's stage handling is pinned by suite
  tests (item 6), and the harness side (status recorded in a
  stub-tier run's `run.json` beside its STATUS line, exit code
  unchanged) is exercised there; the PowerShell execution is
  theory here. That includes build.ps1's new
  behaviours: the 1,800 s deadline terminating a hung cargo
  build's process tree and failing the script, the staging
  identity verification failing the build on a wrong-version
  competitor or on a binary whose SHA-256 differs from its pin
  in `campaign.json`, and the default competitor staging source
  (the preserved bundle beside the package, an operator-named
  `-Competitors` folder overriding it) — all implemented and
  mechanically checked here, executed nowhere yet. A normal build's exit code and staging
  are unchanged by construction: the bounded wait returns the
  process's own exit code and the staging steps are the same
  copies, plus the identity record. That also includes the
  tree-consistency acceptance, which stays exercise-not-assert
  in this tier: on Windows, build with `build.ps1 -Tree` naming
  a copy of the tree at a non-default path, then run `run.ps1`;
  the battery's evidence in `results\battery\` must name that
  same tree as the source it audited — or `run.ps1` must
  refuse, naming the mismatch, before the campaign stage. The
  build record's write, the run's `-Tree` resolution and
  refusal, and the battery's `--tree` plumbing are implemented
  and mechanically checked here (item 6); the Windows run that
  exercises them end to end has not happened.
- The pricing probes' Windows execution against the built guard
  (NO_WINDOW console attachment, the recording launcher as the
  owner probe's single intermediate level, and the resolver's
  PATH walk under its controlled field — scratch working
  directory, pinned PATHEXT, current-folder probe disabled — on
  the target machine): the probe shapes, pairing, provenance
  and failure rules are Linux-proven at stub tier in item 6;
  the Windows timings are theory until the campaign runs. The
  round-8 acceptances stay in this tier, exercised not
  asserted: a Windows run recording the guard's ancestor-chain
  length under both owner-probe shapes (loaded − baseline
  exactly one level, the timing bracket still excluding
  launcher startup), and the resolver probe run from a working
  directory seeded with a same-named child and with a PATHEXT
  lacking `.EXE`.
- Competitor identity on the real as-found binaries: the pinned
  versions are the contract's (ProcessKit CLI v0.3.4,
  nuewframe/timeout v1.0.0, sysprims v0.2.4), the version
  arguments (`--version`) are the stated probe shape, and the
  SHA-256 pins in `campaign.json` are the anchor — each taken
  from the preserved bundle's own run-manifest
  (`benchmark/competitors-001/run-manifest.json`) for the binary
  that actually ran (processkit-cli.exe, timeout.exe and
  sysprims.exe respectively). A supplied binary that does not
  match its pin is not the as-found competitor and is refused
  at staging; the hashes staged are also recorded into
  `build\competitor-identities.json` as an added consistency
  record. The check itself is Linux-proven at
  stub tier in item 6; and the binaries vendored in-package at
  `benchmark/competitors-001/bins` are matched against the pins
  here by the vendored-bundle control in item 6 (hash only —
  their version probe at staging remains a Windows-run event).
- The full campaign (reference plan as configured: 1,940 slots — 36 control + 224 warmup + 1,680 measured; the size is set by the measurement argument, not by quota) and every number it will produce.
- The 54 guard-driving tests (F1–F10, the context and rewriting
  vendor families, and the scanner differential's guard side:
  1,453 verdict comparisons over the 357-case corpus) against
  the built package.
- The battery drivers' Windows execution: each case ships a
  driver in `battery/drivers/` (variant builds and the guard's
  own tests for case (m), on one shared scratch copy and cargo
  target with the driver's 14,400 s deadline covering its
  sub-deadlines; API-level probes for cases (a), (l), (n),
  (o) — one job per trial, terminated once, with both proofs
  armed on that same job and a never-dies member measured
  before it is terminated in (a); a Rust helper compiled with
  the tree's own environment module included, its
  environment_block executed in a process holding the fixture
  through a CreateProcessW block, the same process's vars_os
  and unfiltered GetEnvironmentStringsW enumerations recorded,
  and list-level judging of the duplicate-case fixture in (l);
  the four role programs across the matrix in (n) — decision
  output bytes and consulted verdict values recorded per cell,
  pending output really produced on each role's own handles
  with its pattern bytes measured, the gate on a guarded
  payload through a named vendor, and the contain role inside
  a real enclosing extendable run; counted zero-packet
  dequeues, a real teardown sequence counted from
  per-operation confirmations, three-point
  association-failure injection with the two verdicts observed
  independently per point, an inheritance-enabled
  non-inheritance trial with an inheritable control handle,
  and forged packets posted by a separate child process in
  (o)),
  invoked by `run.ps1` so verdicts and evidence records land
  in `results\battery\`. The decision logic is Linux-proven
  in the stub tier (item 4); the Windows runs themselves,
  and every verdict they will produce, are theory until a
  Windows run.
  The 500-entry environment fixture is not in this tier: it
  ships in `fixtures/env-fixtures.json` and is regenerated
  deterministically by `battery/env_fixtures.py`.
- Competitor argument templates in `campaign.json` / `arms.py`:
  confirmed against the preserved bundle binaries — the bundle's
  campaign ran exactly these binaries, the ones pinned by
  SHA-256 in `campaign.json`. A binary that does not match its
  pin is not the as-found competitor and is refused at staging;
  there is no operator-confirmation path to certifying with a
  substitute. Their execution against the real binaries is a
  Windows-run event.
- The campaign's survivor probe on Windows: `campaign.json` wires
  the shipped probe (`campaign/probe.py`) on by default and the
  kill child is the testkit spawn/mark tree; the probe's Windows
  liveness check (OpenProcess plus GetExitCodeProcess) and the
  tree's marker behaviour under the real arms are by construction
  here and execute on the Windows run. The stub-tier equivalents
  (tree child, direct-only versus tree-killing arms, probe
  removal) are Linux-proven in item 2.
- The campaign's output observation against the real arms: that
  the guard and each as-found competitor pass the relay child's
  stdin through and return its stdout and stderr unchanged, so
  their fast slots match the expected bytes, is a Windows-run
  event. The capture mechanism (per-slot files, digests,
  verdicts), the classification and the gate wiring are
  Linux-proven at stub tier in items 2 and 6; a competitor that
  does not pass the streams through will be recorded as
  output-mismatch and listed as competitor-loss evidence there,
  and a guard that does not will fail output correctness.

[↑ Back to Contents](#contents)

## Not verified anywhere yet

- Any speed claim about the current tree. The census is counted;
  the campaign is owed; this package is the instrument, not a
  result.
- The completeness accounting for the phase (the second sense of
  completeness named in `README.md`, distinct from a run's
  campaign slot accounting) is EXECUTED, as the phase's closing
  step after the round-52 silence: it stands in
  `COMPLETENESS.md`, by its four parts — (1) failure-tree
  branches (88 rows mapped: 63 to a suite test or family, 18
  to an in-tree test, 4 to a battery case, 3 to a
  tier-labelled non-execution — the spawn-race row counted
  at its mechanism level since the round-53 landing, its
  consequences mapped in-tree in its row — plus the degraded set of
  exactly 5, each mapped); (2) feature surfaces (the campaign
  machinery end to end, the battery's case set, the
  eight-dimension programme with its ledger state, the
  sensitivity gate, the ready-to-fly surface, and the
  guard-facing surface stated whole — by file and by
  subject — as sanity check p4-10's point D1 charged);
  (3) doc claims (34 mapped, as completed at the round-53
  landing; the 2 findings flagged UNKEPT at the accounting's
  first landing, F-Acct-1 and F-Acct-2, cured there — the
  materialization definitions landed in the README Glossary
  with the census lock amended openly, and the compile-all
  test landed as a suite test); (4) battery conditions (17 conditions mapped;
  the 12 behavioural conditions are non-executed BY TIER,
  with the revival path — the ready-to-fly Windows run —
  stated once, exactly, at the head of that part). Parts
  (1), (2) and (4) are complete at the breadths the
  accounting states; part (3) is complete at the breadth
  the round-53 landing restated it, after the round-53
  review of the accounting sustained four protests and
  charged their fixes (F1–F6, landed); round 54 reviews
  that landing, and the pre-closure sanity check follows.
  The accounting also carries
  the two standing observations from the round-39
  adjudication: the resource-note vocabulary/write-sites
  tie, closed there by a new test, and the old 1-record
  sub-probe's n=1 edge, mapped there to a named
  non-execution with its revival path.
- Programmatic consumption of configurations (the D2
  disposition of sanity check p4-10, carried inside the
  accounting): config objects may reach the materialization
  functions programmatically, without `load_config`, from
  the suite's own consumers — the shapes those consumers
  can actually carry. `admission_failures` judges a
  configuration consumed without `load_config` and reads
  token lists through the shared materialization because a
  payload-carrying list subclass can be a first reader;
  the derivation and the runner's construction expressly
  read mutation-refusing spec mappings (a MappingProxyType,
  a frozen dict) that `json.load` cannot produce; and
  `campaign_outcome`, `check_run_package`, the derivation
  and the pricing consumers all take config objects as
  parameters. Beyond the classes named in the sentences
  above, five further classes are CLAIMED at the same
  breadth, each tied to a named consumer that actually
  carries it (round-53 F3, re-judged under the executed
  consumer criterion — carried by a named consumer,
  executed, agreeing twice): minting mappings, copying
  mappings and memoizing mappings, and handshake and
  restarting iterators — each is carried by both
  `report._plan_fast_argv_derivation` and
  `runner.child_for`, as the suite's own Round44/Round45/
  Round47 tests route them and as the round-53 review's
  and landing's executed probes confirm; they are kept
  and named class by class in `COMPLETENESS.md`. (At the
  accounting's first landing those five classes stood as
  defensive depth on the stated basis that no consumer
  carries them; that basis was false at the consumer
  level the claimed classes are judged by, and the
  round-53 review's executed probe withdrew it.) The
  fresh-traversal mint stands
  outside both statements, as item 6 and `arms.py` state
  at one breadth.

[↑ Back to Contents](#contents)
