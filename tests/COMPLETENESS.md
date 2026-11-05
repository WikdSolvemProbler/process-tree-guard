<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# Completeness accounting — Phase 4 (the phase sense of completeness)

This file is the completeness accounting Peter charged on
2026-10-29 ("I just want every single fucking thing tested"):
every failure-tree branch, every surviving feature surface,
every documentation claim, and every standing battery
condition, each mapped to a named test or to an explicit
tier-labelled non-execution with its revival path written
down — nothing silently absent. It is the closing step of
Phase 4, executed after the round-52 byte-exact silence. It
is a different sense of "completeness" from the campaign
sense (exact slot accounting for one run), which `README.md`
defines separately; the word carries both senses in this
package, and this file discharges the phase sense.

## Contents

- [Part 1: Failure-tree branches](#part-1--failure-tree-branches)
  - [1. Fast path (plain run; census: 45 named calls)](#1-fast-path-plain-run-census-45-named-calls)
  - [2. Kill path (deadline and teardown)](#2-kill-path-deadline-and-teardown)
  - [3. Hook surfaces (gate and full role; vendors: pretooluse, cursor](#3-hook-surfaces-gate-and-full-role-vendors-pretooluse-cursor)
  - [4. CLI and error contract](#4-cli-and-error-contract)
  - [5. Extendable runs](#5-extendable-runs)
  - [6. Automatic mode and roles](#6-automatic-mode-and-roles)
  - [7. Measurement system itself](#7-measurement-system-itself)
  - [The degraded set (exactly 5): the preserved reduced-run package's unverifiable verifications](#the-degraded-set-exactly-5--the-preserved-reduced-run-packages-unverifiable-verifications)
- [Part 2: Feature surfaces](#part-2--feature-surfaces)
  - [2.1 The campaign machinery, end to end](#21-the-campaign-machinery-end-to-end)
  - [2.2 The battery's case set and its BLOCKED-WINDOWS tier](#22-the-batterys-case-set-and-its-blocked-windows-tier)
  - [2.3 The dimension programme: eight dimensions](#23-the-dimension-programme--eight-dimensions)
  - [2.4 The sensitivity gate and its 250 ms floor](#24-the-sensitivity-gate-and-its-250-ms-floor)
  - [2.5 The ready-to-fly surface](#25-the-ready-to-fly-surface)
  - [2.6 The guard-facing surface whole (sanity p4-10 point D1)](#26-the-guard-facing-surface-whole-sanity-p4-10-point-d1)
- [Part 3: Documentation claims](#part-3--documentation-claims)
- [Part 4: Battery conditions](#part-4--battery-conditions)
  - [Case (a): teardown wait vs Sleep(10) poll (3 conditions)](#case-a--teardown-wait-vs-sleep10-poll-3-conditions)
  - [Case (l): environment block differential (2 conditions)](#case-l--environment-block-differential-2-conditions)
  - [Case (m): pseudo-handle sites (2 conditions)](#case-m--pseudo-handle-sites-2-conditions)
  - [Case (n): verdict surface across Console::finish (2 conditions)](#case-n--verdict-surface-across-consolefinish-2-conditions)
  - [Case (o): IOCP teardown proof (8 conditions)](#case-o--iocp-teardown-proof-8-conditions)
  - [Standing battery conditions](#standing-battery-conditions)
- [D2 disposition: unreachable input classes (sanity p4-10)](#d2-disposition--unreachable-input-classes-sanity-p4-10)
- [Standing observations carried from round 39](#standing-observations-carried-from-round-39)
- [What this accounting does not do](#what-this-accounting-does-not-do)
- [E10 extension: Phase 5 completeness re-run over the reorganized tree](#e10-extension--phase-5-completeness-re-run-over-the-reorganized-tree)
  - [Landing inventory (§9.2)](#landing-inventory-92)
  - [Landing edits (path updates, sourced from the manifest's keys)](#landing-edits-path-updates-sourced-from-the-manifests-keys)
  - [R1: Path translation](#r1--path-translation)
  - [R2: Re-inventory](#r2--re-inventory)
  - [R3: Surfaces re-mapped](#r3--surfaces-re-mapped)
  - [R4: Claims re-mapped, and extended](#r4--claims-re-mapped-and-extended)
  - [R5: Battery and degraded set](#r5--battery-and-degraded-set)
  - [R6: Failure-tree integrity](#r6--failure-tree-integrity)
  - [§9.3 ADDITION: the named open item, carried](#93-addition--the-named-open-item-carried)
  - [Named non-executions at this step (tier-labelled)](#named-non-executions-at-this-step-tier-labelled)

Construction method (executed at this landing, recorded in
the landing log): `FAILURE-TREE.md` was parsed row by row
(88 table rows in sections 1–7, plus its section-5 scope
decision and the degraded set its section-7 package row
carries); every test in the suite was inventoried by AST
(647 tests counted once as this accounting lands, restated at the round-53 landing and at the round-60 landing); the
by-subject enumeration `GUARD_SUBJECT_TESTS` was extracted
from the census test and every one of its 152 names verified
to exist; the battery's conditions were read from
`evidence/battery-linux.json` (17 conditions over cases
(a), (l), (m), (n), (o)); and `FAILURE-TREE.md`'s own
test-name citations were verified by the suite's own census
test (`CensusConsistencyTests.test_failure_tree_citations_resolve_to_suite_tests`,
executed targeted at this landing, OK). Where a mapping in
`FAILURE-TREE.md` is wrong, this file says so as a finding;
`FAILURE-TREE.md` itself is not rewritten by this step.

Tier vocabulary used below: **anywhere** (runs on Linux
here), **Windows tier** (executes only on the ready-to-fly
Windows run against the built package; on this machine the
test skips or the case reports BLOCKED-WINDOWS),
**Windows/MSVC tier**, **manual/Windows-theory** (a person
present on an interactive Windows desktop), **in-tree**
(the guard tree's own Rust tests, Windows-tier execution
unless stated). A non-execution is a complete disposition
when its tier and revival path are named.

Counts as this accounting lands (recomputed from the test
sources and the preserved packages at this landing; the
landing log carries the recompute):

- Census: 647 tests counted once — by file 487
  campaign-logic / 91 guard-facing / 160 including battery;
  by subject 312 guard-facing / 335 harness; execute split
  593 run / 54 skipped on this machine.
- Part 1: 88 failure-tree rows mapped — 63 to a suite test
  or family, 18 to an in-tree test, 4 to a battery case,
  3 to a tier-labelled non-execution (the spawn-race row
  counted here, at its mechanism level, at the round-53
  landing — its consequences are mapped in-tree in its
  row, but the row is not counted as an in-tree test
  mapping); plus the degraded
  set, exactly 5 entries, each mapped below.
- Part 2: the campaign machinery (10 surfaces), the
  battery's case set, the eight-dimension programme, the
  sensitivity gate, the ready-to-fly surface, and the
  guard-facing surface whole (91 by file + 69 battery +
  152 by subject inside the campaign-logic file = 312 by
  subject) — every surface mapped.
- Part 3: 35 load-bearing documentation claims mapped —
  32 kept by named tests, 1 dispositioned as a
  tier-labelled non-execution (the cap-over-default scope
  decision, whose seams are mapped in Part 1), 2 kept by a
  named non-test mechanism, 0 flagged UNKEPT (the two
  findings flagged at the accounting's first landing,
  F-Acct-1 and F-Acct-2, were cured at the round-53
  landing and their rows below record the cures).
- Part 4: 17 battery conditions mapped (5 source-audit
  conditions evidenced on Linux; 12 behavioural conditions
  non-executed BY TIER, revival stated once at the head of
  the part), plus the battery's 3 standing-condition groups
  (case set, BLOCKED-WINDOWS derivations, driver-payload
  shape validation).
- Standing footings, unmoved: the preserved reduced-run
  package reads 82 standing losses at the package footing,
  82 at the explicit-sentinel footing, 83 at the direct
  no-plan footing (the symmetric difference is the one
  disclosed vector); its claim ledger reads UNEVALUATED
  ×6 / NOT-APPLICABLE ×2; its degraded set is exactly the
  5 entries mapped in Part 1.

[↑ Back to Contents](#contents)

## Part 1 — Failure-tree branches

Every row of `FAILURE-TREE.md`, in its own order. The
mapping names the family or test the row itself names;
existence of every suite test cited in that file is held by
the citation census test named above. The file's two
closing rows of section 7 — "Guard binary missing and
families fail obscurely" and "Trace lines or the timing
self-measurement record…" — are the last two lines of the
section-7 table below, mapped like every other row. One
row is annotated where its own text splits the disposition:
the spawn-race row (section 2) maps its *consequences*
in-tree, and its creation mechanism itself is, by the row's
own statement, "not behaviourally distinguishable by a test
in either tree" — a named non-execution at the mechanism
level, kept as the row states it. The row's label states
that split in the row itself, and the partition above
counts the row as the non-execution, not as an in-tree
test mapping (round-53 F1, correcting the accounting's
first landing, whose row label and partition counted it
on the in-tree-test side).

### 1. Fast path (plain run; census: 45 named calls)

- **Child exit code lost or rewritten** -> test_exit_codes_preserved — MAPPED (suite test).
- **Child output dropped or reordered** -> test_child_output_passes_through — MAPPED (suite test).
- **An arm drops or garbles the child's output on the campaign fast path while returning the expected exit code** -> tests/test_output_observation.py — MAPPED (suite test).
- **Program not found reported as the wrong code** -> test_missing_program_is_guard_error_125 — MAPPED (suite test).
- **Shell form decodes or routes wrongly** -> test_shell_form_runs_the_decoded_command — MAPPED (suite test).
- **Resolver probe order (PATH/PATHEXT, current folder rule)** -> test_coverage_families.py — MAPPED (suite test).
- **Owner exits mid-run → tree ends, 125** -> tree/tests/guard.rs; g25_immediate_owner_exit_kills_child; g44_library_run_ends_the_tree_with_125_when_the_parent_exits_during_the_call — MAPPED (in-tree test, Windows-tier execution).
- **Console finish mis-restores the console** -> Console::finish — NON-EXECUTION (Windows tier; battery case (p) not yet created, revival in FAILURE-TREE cell).
- **Guard's own startup cost regresses** -> campaign fast path, floor-based report — MAPPED (suite test).

### 2. Kill path (deadline and teardown)

- **Deadline missed or late** -> test_sleeper_killed_at_the_deadline_with_124 — MAPPED (suite test).
- **Descendant survives the kill** -> test_descendant_tree_dies_with_the_child — MAPPED (suite test).
- **Descendant left running when the command finishes normally, not at a deadline** -> tree/tests/guard.rs; g19_g20_descendant_killed_on_timeout_and_normal_exit — MAPPED (in-tree test, Windows-tier execution).
- **Child asks to break away from the job and is allowed out** -> tree/tests/guard.rs; g26_child_cannot_break_away_from_job — MAPPED (in-tree test, Windows-tier execution).
- **Child runs before it is contained (the spawn race: created without the job, or created running and assigned to it afterwards)** -> split disposition (round-53 F1): its *consequences* -> tree/tests/guard.rs; g26_child_cannot_break_away_from_job; g19_g20_descendant_killed_on_timeout_and_normal_exit; G28 for a slow-to-exit descendant, as FAILURE-TREE's own cell states — MAPPED (in-tree test, Windows-tier execution); its *creation mechanism itself* -> PROC_THREAD_ATTRIBUTE_JOB_LIST (the constant at tree/src/sys.rs); tree/src/engine/attributes.rs — NON-EXECUTION at the mechanism level (source-audited in-tree; by the row's own statement in FAILURE-TREE.md, "not behaviourally distinguishable by a test in either tree"). Counted in the Part-1 partition as the non-execution.
- **Partial output at kill lost** -> test_partial_output_survives_the_kill — MAPPED (suite test).
- **Teardown of a wide tree (>64 members) stalls or leaks** -> tree/tests/battery.rs; b33_to_b35_wide_and_deep_trees_are_killed — MAPPED (in-tree test, Windows-tier execution).
- **Member that never dies exhausts the teardown budget wrongly** -> battery case (o) burden item 3 — MAPPED (battery case, Windows tier).
- **Poll-based teardown proof is slow or wrong** -> battery case (a) — MAPPED (battery case, Windows tier).
- **IOCP proof faked by a forged packet** -> battery case (o) forged-packet condition — MAPPED (battery case, Windows tier).
- **Competitor kill claims misstated** -> test_nuewframe_facts_stated_exactly — MAPPED (suite test).

### 3. Hook surfaces (gate and full role; vendors: pretooluse, cursor,

- **Guarded command denied, or unguarded allowed (pretooluse)** -> F3 allow/deny tests — MAPPED (suite test).
- **Malformed payload allowed** -> test_pretooluse_denies_malformed_payload_with_2 — MAPPED (suite test).
- **Cursor answer shape wrong** -> test_cursor_allow_shape — MAPPED (suite test).
- **Gate role and full role diverge** -> test_gate_role_matches_full_role_on_allow_and_deny — MAPPED (suite test).
- **Gate role's own argument contract wrong (no vendor not reading pretooluse; the gate running a command)** -> tree/tests/battery.rs; b125_gate_equals_the_full_command_hook_for_every_vendor; b126_gate_denial_text_exit_code_and_never_running_a_command; permissionDecision — MAPPED (in-tree test, Windows-tier execution).
- **Gate role's argument contract: a leading `hook` read as the vendor name, or a further argument after the vendor, not refused (the tree README's Programs section: both are refused with the usage text on standard error and exit code 125, without reading standard input; implemented in `tree/src/lib.rs`, `entry_gate` / `entry_full`, through `hook::usage_error`)** -> tree/tests/battery.rs; b131_gate_prints_its_usage_for_help_and_does_not_read_a_payload; hook; pretooluse --help; pretooluse; b125_gate_equals_the_full_command_hook_for_every_vendor — MAPPED (in-tree test, Windows-tier execution).
- **Unknown vendor silently accepted** -> test_unknown_vendor_is_a_usage_error_never_silent — MAPPED (suite test).
- **Antigravity rewrite wrong (envelope, Base64 round-trip, length cap)** -> F8AntigravityTests; test_coverage_families.py — MAPPED (suite test).
- **PowerShell-auto rewrite wrong** -> F8PowershellAutoTests; test_coverage_families.py — MAPPED (suite test).
- **Context vendors print the wrong policy** -> F8ContextVendorTests; test_coverage_families.py — MAPPED (suite test).
- **Streaming scanner diverges from the parsed verdict** -> test_scanner_differential.py; scanner_reference.py; evidence/scanner-corpus-summary.json; CorpusTests.test_corpus_summary_fields_match_the_regenerated_corpus; scanner_reference.CORPUS_SEED — MAPPED (suite test).
- **Payload cap (16 MiB) decided wrongly** -> F7PayloadCapTests; test_coverage_families.py — MAPPED (suite test).

### 4. CLI and error contract

- **Any argument error exits 2 (would block a host hook event)** -> F4 (all tests assert 125) — MAPPED (suite test).
- **Timeout bounds (0, negative, >86400, non-numeric, non-finite)** -> test_bad_timeouts_are_125 — MAPPED (suite test).
- **Extension options accepted without `--extendable`** -> test_extension_options_need_extendable — MAPPED (suite test).
- **Help broken on any role** -> test_help_is_zero — MAPPED (suite test).
- **A `.cmd` or `.bat` program is run instead of refused** -> tree/tests/guard.rs; g27_cmd_files_rejected; .cmd; .cmd/.bat; test_bat_program_is_refused_like_cmd; tests/test_guard_families.py — MAPPED (in-tree test, Windows-tier execution).
- **A built program's Windows version resource is wrong or missing** -> tree/tests/guard.rs; g39_built_programs_carry_their_version_resource; CI; PROCESS_TREE_GUARD_REQUIRE_RESOURCES — MAPPED (in-tree test, Windows-tier execution).
- **Non-Unicode argument handling** -> F9NonUnicodeArgvTests; test_coverage_families.py — MAPPED (suite test).

### 5. Extendable runs

- **Granted hold does not extend the run** -> test_child_extension_outlives_the_initial_deadline — MAPPED (suite test).
- **Run with no extension ends late** -> test_run_without_extension_ends_at_its_deadline — MAPPED (suite test).
- **`extend` outside a run starts something** -> test_extend_outside_a_run_is_refused — MAPPED (suite test).
- **Contain role runs without an enclosing run** -> test_contain_outside_a_run_is_refused — MAPPED (suite test).
- **Contain role inside an enclosing run misbehaves (does not run the command as a logged request to that run, or a timeout over the run's cap is granted)** -> tree/tests/guard.rs; g58_the_contain_role_is_a_request_to_the_run_that_encloses_it — MAPPED (in-tree test, Windows-tier execution).
- **Environment block built wrongly for the child** -> battery case (l) differential — MAPPED (battery case, Windows tier).
- **Rate limit, hold reaping, log completeness under load** -> tree/tests/battery.rs; b133_requests_are_rate_limited_per_process; b137_a_flood_from_outside_the_job_cannot_push_a_grant_out_of_the_log; b138_a_process_in_the_job_that_asks_in_a_loop_cannot_flood_standard_error_or_the_file; b145_the_documents_state_the_log_the_hold_limit_and_the_reply_that_is_not_taken; g51_the_run_ends_at_the_latest_deadline_of_the_children_that_are_alive — MAPPED (in-tree test, Windows-tier execution).
- **Cap-over-default approval flow (Hello / credential / console)** -> in-tree seam and parity coverage (see the scope decision below this section) — NON-EXECUTION (scope decision, manual/Windows-theory).
- **Channel spoofing by a process outside the job** -> tree/tests/battery.rs; b132_a_client_outside_the_job_is_refused_and_the_deadline_does_not_move; b134_malformed_and_forged_requests_are_refused_and_move_nothing; tree/tests/guard.rs; g71_a_reply_out_of_range_is_refused_by_the_client_and_never_crashes_it; g81_every_connection_from_a_client_outside_the_job_is_counted_in_the_log — MAPPED (in-tree test, Windows-tier execution).

### 6. Automatic mode and roles

- **Auto runs the wrong form (program vs shell line)** -> test_auto_runs_a_program_form_command — MAPPED (suite test).
- **Auto deadline taken from the wrong source (the environment variables `PROCESS_TREE_GUARD_AUTO_TIMEOUT`, `PROCESS_GUARD_AUTO_TIMEOUT`, `HARNESS_AUTO_TIMEOUT` in that precedence — a set first variable, even empty, shadowing the rest — against `--timeout` and the 7,200 s default)** -> tests/auto.rs; auto_timeout_setting; src/cli.rs — MAPPED (in-tree test, Windows-tier execution).
- **Auto accepts extension options** -> test_auto_is_never_extendable — MAPPED (suite test).
- **Deadline role grows a hook mode** -> test_deadline_role_has_no_hook_mode — MAPPED (suite test).
- **Compatibility alias behaves differently from the program (the 1.x name `process-guard.exe`, kept through 2.x)** -> tree/tests/guard.rs; g29_to_g33_both_names_behave_identically; tree/tests/battery.rs — MAPPED (in-tree test, Windows-tier execution).
- **`-auto` file-name dispatch wrong** -> tree/tests/auto.rs; u14_to_u16_both_automatic_names; process-tree-guard-auto.exe; process-guard-auto.exe — MAPPED (in-tree test, Windows-tier execution).
- **Library entry points diverge from CLI** -> g43_library_run_returns_the_command_code_and_124_at_the_deadline; g49_library_run_hook_answers_on_standard_output_and_returns_the_hook_code; tree/tests/guard.rs — MAPPED (in-tree test, Windows-tier execution).
- **Library diagnostics records lost, reordered, replaced wrongly, or past their caps (README: owned `take_diagnostics()` records, next outer call replaces untaken records, nested calls append, queue caps of 1 MiB / 256 records with `PROCESS_GUARD_DIAGNOSTICS_TRUNCATED`)** -> tree/src/diagnostics.rs; returning_capture_preserves_nested_record_order_and_owned_output; next_outer_invocation_replaces_previous_optional_output; optional_queue_caps_are_explicit_and_preserve_utf8; take_diagnostics; tree/src/cli.rs — MAPPED (in-tree test, Windows-tier execution).
- **Gate accepts a program from a layout the docs refuse, or refuses one they accept (README "Where the programs go": envelope programs in their own folder or a `bin` folder beside `libexec`, never the gate itself, policy file one folder above the program)** -> tree/tests/battery.rs; b123_gate_allows_envelopes_of_the_programs_that_enforce_a_deadline; b124_gate_denies_the_gate_itself_other_paths_and_altered_envelopes; b127_gate_context_is_the_policy_one_folder_above; b128_the_hook_role_keeps_the_gates_rules_under_another_file_name; b129_the_full_command_keeps_the_rules_of_its_name_under_the_gates_file_name — MAPPED (in-tree test, Windows-tier execution).

### 7. Measurement system itself

- **Configured plan drifts from its own decomposition (reference plan as configured: 36 + 224 + 1680 = 1940; 1,620 is the reference decomposition of the original plan shape — the round-26 measurement-argument resize re-pinned the configuration to the totals the malformed and scaling cells yield)** -> test_plan_totals_match_configured_decomposition — MAPPED (suite test).
- **Floor arms disagree and a distance is still reported** -> test_floor_refused_when_the_minimal_tools_disagree — MAPPED (suite test).
- **The floor median rests on ok control records whose elapsed time was never recorded, or a context's floor vanishes entirely (floor records carried, no usable floor) and the campaign still certifies** -> RecordConsistencyTests; test_campaign_logic.py — MAPPED (suite test).
- **A kill record's tree identity is an integer but no PID any descendant carried (grandchild_pid of −1, 0 or 10^30; a present child_pid of −5), so the survivor classification rests on an identity outside the platform PID range** -> RecordConsistencyTests; test_campaign_logic.py; report.MAX_PID; _evidenced_survived; test_survived_records_without_pid_form_identity_mint_nothing_and_are_named; test_package_checker_flags_survived_records_without_pid_identity — MAPPED (suite test).
- **A competitor exit fault is minted from an exit_code no instrument recorded (exit_code "boom", 7.5, True or [7] on a measured exit-mismatch record, or under a descendant-survived label)** -> RecordConsistencyTests; test_campaign_logic.py; _evidenced_exit_mismatch; _record_inconsistencies; test_non_integer_exit_code_on_a_competitor_record_mints_nothing_and_is_named; test_non_integer_exit_code_under_a_survival_label_mints_no_exit_loss — MAPPED (suite test).
- **Stale anchor pairing cited without provenance** -> test_stale_pairing_appears_only_with_provenance — MAPPED (suite test).
- **Decomposition prices a cheap item, or invents a price** -> test_decomposition_prices_only_the_expensive_class — MAPPED (suite test).
- **A pricing price rests on records whose probe-shape stamps do not carry the construction the price claims (console context pair, owner inserted levels, resolver probe count and pinned field)** -> PricingStampTests; test_campaign_logic.py; Round21NeighbourTests.test_owner_and_resolver_context_flips_are_stamp_defects_and_price_nothing; intended_path; inserted_levels; inserted_levels — MAPPED (suite test).
- **The rollup's pricing evidence admits a boolean or non-finite price (a NaN price decomposing to a NaN unexplained remainder with no inconsistency named; a True price carried as a measured 1.0 ms)** -> PricingUncertaintyTests; test_campaign_logic.py; items; report._pricing_items; decompose; Round21NeighbourTests.test_pricing_items_container_of_another_type_resolves_not_measured — MAPPED (suite test).
- **The pricing sign-inconsistency verdict is certified from unchecked count fields (paired differences that disagree in sign priced clean on stated counts of 0 / 0; string counts flowing into the flag text)** -> PricingUncertaintyTests; test_campaign_logic.py; _pricing_items; paired_differences_ms; ms; prices_from_observations — MAPPED (suite test).
- **Counted-ops census cannot be recounted from the package, or `report.py`'s CENSUS drifts from the enumeration §2 states** -> CensusConsistencyTests; test_campaign_logic.py; campaign/census.json; README.md — MAPPED (suite test).
- **Recorded evidence goes stale against what the suite and the tree now produce (`evidence/test-run.txt` recording a 344-test run while the documents state the current total; `evidence/battery-linux.json` and VERIFICATION §4 stating source coordinates the tree no longer carries)** -> CensusConsistencyTests.test_recorded_test_run_evidence_matches_the_runner_count; test_campaign_logic.py; RecordedBatteryEvidenceTests.test_recorded_battery_evidence_matches_a_fresh_run; test_battery_cases.py; run_tests.py --count-only; evidence/test-run.txt — MAPPED (suite test).
- **A classification rests on observations its path never makes, the survivor field is trusted only when its label already agrees with it, or a survived classification rests on no observed exit code (a kill record labelled `output-mismatch`; a fast record labelled `descendant-survived`; an `exit-mismatch` kill record with survivor True over which kill-path correctness PASSes; `descendant-survived` records with exit_code null minting competitor losses)** -> Round21NeighbourTests; test_campaign_logic.py; _record_inconsistencies; _kill_state; descendant-survived; _evidenced_survived — MAPPED (suite test).
- **Missing binary measured as a time** -> test_missing_binary_is_infra_failure_not_a_fake_time — MAPPED (suite test).
- **Kill path cannot see a surviving descendant (single-process child, unscoped or absent checker)** -> KillTreeTests — MAPPED (suite test).
- **One floor arm runs no process, so floor agreement is impossible by construction** -> FloorSpawnTests — MAPPED (suite test).
- **Floor tolerance untethered from the observed minimal-tool agreement** -> test_floor_tolerance_matches_the_bundle_agreement — MAPPED (suite test).
- **Headline medians pool the two console contexts** -> PerContextTests — MAPPED (suite test).
- **"Complete" counted records instead of accounting for slots; exit codes never validated; freeze gate never stated** -> SlotAccountingTests; test_complete_campaign_has_no_standing_losses — MAPPED (suite test).
- **Slot machinery misclassifies outcomes** -> campaign stub-run tests — MAPPED (suite test).
- **Campaign certifies output quality it never observes (profile dimension (1) passing on kill-path and exit-code evidence alone, with every stream discarded)** -> OutputCaptureTests; OutputGateTests; tests/test_output_observation.py; RecordConsistencyTests; test_campaign_logic.py — MAPPED (suite test).
- **A torn package file crashes the report path with a traceback instead of a named refusal (the stopped campaign's mid-write manifest or appended record files — the exact package `cmd_report` exists to render)** -> MalformedPackageTests; test_campaign_logic.py; observations.jsonl; pricing-observations.jsonl; run.json; run.json — MAPPED (suite test).
- **The configuration loader crashes on container type flips (a non-object root, `arms` or `children` as a list, one arm entry as a string) instead of the named-refusal discipline the package files carry** -> AdmissionTests; test_campaign_logic.py; load_config; admission_failures; pricing_probes; load_config — MAPPED (suite test).
- **A stopped campaign reported as a result** -> test_report_headline_is_distance_above_floor_not_a_race — MAPPED (suite test).
- **Campaign can never be complete: unobserved profile dimensions decidable only by editing the measuring instrument, and no claim ledger in the run package** -> ProfileDimensionTests; ClaimLedgerTests; CampaignOutcomeContractTests; campaign.json; run.json; claim-ledger.json — MAPPED (suite test).
- **A measurement gate is unstable at the load band the suite calls quiet, so its verdict flips on sampling noise rather than the quantity it guards (the owner-price gate: two medians of 9 paired differences against an IQR bound — a sampler-verified quiet run failed it by 0.29 ms while a greener run at a higher load peak passed)** -> OwnerVerificationTimingTests; test_campaign_logic.py; test_sleeping_launcher_does_not_move_the_owner_price; test_launcher_delay_inside_the_bracket_moves_the_owner_price; test_launcher_delay_inside_the_bracket_moves_the_owner_price; CensusConsistencyTests.test_verification_floor_citations_carry_the_quiet_qualification — MAPPED (suite test).
- **The competitor pins' provenance chain is severed at its second link (the preserved bundle's own run-manifest — the pins' stated source — recording a hash or size no test compares to the pins or the binaries)** -> VendoredCompetitorBundleTests.test_run_manifest_hashes_and_sizes_match_the_campaign_pins; test_campaign_logic.py — MAPPED (suite test).
- **The run package's beside-fields contradict the run they package (run.json's run_id, environment or config_templates, or report.txt's body citing a fingerprint the manifest does not anchor) and no consumer reads them against the run** -> Round23NeighbourTests; Round24NeighbourTests; test_campaign_logic.py; check_run_package; test_package_checker_holds_run_id_environment_and_report_body; test_package_checker_holds_config_templates_to_the_runs_configuration — MAPPED (suite test).
- **Build and run name different trees, so the battery silently audits sources the campaign never measured (or crashes on an absent default tree)** -> BuildScriptTests; RunScriptTests; build.ps1; build\build-record.json; run.ps1; -Tree — MAPPED (suite test).
- **Guard binary missing and families fail obscurely** -> test_guard_families.py — MAPPED (suite test).
- **Trace lines or the timing self-measurement record emitted without their enabling variables (the trace names, then the 32-hex timing nonce), or either one holding the tree past its deadline or changing an exit code** -> tests/guard.rs; src/timing.rs — MAPPED (in-tree test, Windows-tier execution).
### The degraded set (exactly 5) — the preserved reduced-run package's unverifiable verifications

`harness.check_run_package` over `evidence/reduced-run/`
returns exactly these five problems, re-executed at this
landing. Each is a tier-labelled non-execution *for that
package*: its configuration anchor is absent (the recorded
`config_path` names a scratch file deleted with the run's
temporary directory), so the verification is named
unverifiable rather than passed or fallen back to the
package's own claim. The revival path for all five is the
same: a package recorded with its config content carried —
a live configuration file whose SHA-256 matches the
manifest's recorded hash — under which the hold executes in
full; that path is exercised by the anchored-package
controls named beside each entry.

- **config_templates** (the recorded templates cannot be
  checked against the run's configuration) ->
  `Round25NeighbourTests.test_package_checker_names_the_full_degraded_set`,
  `Round24NeighbourTests.test_package_checker_names_unverifiable_config_templates`,
  `Round24NeighbourTests.test_anchored_package_templates_verify_and_mismatch_names`
  — NON-EXECUTION for the preserved package (anchor tier),
  revival as stated above.
- **competitor identity** (the identity verdict rests on no
  anchored recomputation) ->
  `Round25NeighbourTests.test_package_checker_names_the_full_degraded_set`,
  `Round25NeighbourTests.test_fabrications_under_the_absent_anchor_are_named_unverifiable`
  — NON-EXECUTION for the preserved package (anchor tier),
  revival as stated above.
- **expected-output pairing** (a match verdict's expected
  bytes cannot be derived without the anchor) -> the same
  two Round25NeighbourTests names — NON-EXECUTION for the
  preserved package (anchor tier), revival as stated above;
  the anchored form is exercised by the package-layer
  output-evidence controls in `RecordConsistencyTests`.
- **profile decisions** (the decisions the ledger renders
  cannot be verified against an anchored configuration) ->
  the same two Round25NeighbourTests names —
  NON-EXECUTION for the preserved package (anchor tier),
  revival as stated above.
- **argv length budget** (no fast-vector length derives for
  any arm under the NO_VERIFIED_PLAN sentinel, round-37 F3)
  -> the same two Round25NeighbourTests names, and
  `Round37LandingTests.test_minimal_anchor_package_names_the_underived_budget`
  — NON-EXECUTION for the preserved package (anchor tier),
  revival as stated above.

[↑ Back to Contents](#contents)

## Part 2 — Feature surfaces

### 2.1 The campaign machinery, end to end

- **Plan** (`campaign/plan.py`: slot construction, seeded
  order basis, malformed/scale cells, per-cell totals) ->
  `PlanTests`, `TemplateTests`,
  `Round26LandingTests.test_plan_carries_malformed_cells_for_every_arm_shape_context`,
  `Round26LandingTests.test_plan_carries_scaling_cells_bracketing_the_window_boundary`,
  and the plan-total hold
  `test_plan_totals_match_configured_decomposition` —
  MAPPED (anywhere).
- **Load** (`campaign/harness.py` `load_config`: the
  named-ValueError refusal discipline at container, entry,
  field, element and scalar levels) -> `AdmissionTests`,
  `Round22NeighbourTests`, `Round23NeighbourTests`,
  `Round24NeighbourTests` (the load/admission shape holds
  named in Part 1, section 7's loader row) — MAPPED
  (anywhere).
- **Admission** (`harness.admission_failures`: pre-flight
  identity, presence and shape naming that never raises on a
  config shape) -> `AdmissionTests`,
  `CompetitorIdentityTests`, and the admission halves of
  the Round22–Round24 neighbour holds — MAPPED (anywhere).
- **Runner** (`campaign/runner.py`: one slot = one outer
  lifetime, output capture, resource instrumentation,
  scale/kill classification) -> `StubRunTests`,
  `KillTreeTests`, `FloorSpawnTests`, `OutputCaptureTests`
  (in `tests/test_output_observation.py`),
  `HarnessDeadlineKillTests`,
  `Round26LandingTests`' runner tests — MAPPED (anywhere;
  the real arms' behaviour is Windows tier).
- **Report** (`campaign/report.py`: floor rule, per-context
  medians, freeze gate, slot accounting, record
  consistency, pricing rollup, claim ledger, render) ->
  `FloorAndReportTests`, `PerContextTests`,
  `SlotAccountingTests`, `RecordConsistencyTests`,
  `MalformedPackageTests`, `PricingUncertaintyTests`,
  `RecordedOutcomeTests` and the Round21–Round51 landing
  classes — MAPPED (anywhere).
- **Pricing** (`campaign/pricing.py`: the three
  expensive-class probes, stamps, uncertainty) ->
  `PricingProbeTests`, `PricingStampTests`,
  `PricingProvenanceTests`, `PricingUncertaintyTests`,
  `OwnerVerificationTimingTests` — MAPPED (anywhere;
  Windows execution against the built guard is theory,
  revival the Windows run).
- **Outcome** (`harness.campaign_outcome`: completeness
  verdict consuming the claim ledger) ->
  `CampaignOutcomeContractTests`, `StoppedCampaignTests`,
  `ClaimLedgerTests` — MAPPED (anywhere).
- **Rollup** (the pricing-evidence rollup into the report's
  decomposition: `report._pricing_items`,
  `prices_from_observations`) -> `PricingUncertaintyTests`,
  `Round21NeighbourTests.test_pricing_items_container_of_another_type_resolves_not_measured`,
  `Round30LandingTests` (totality at the pricing producers
  and the rollup) — MAPPED (anywhere).
- **Survivor probe** (`campaign/probe.py`, wired on by
  default) -> `KillTreeTests.test_missing_probe_is_infra_failure_never_ok`,
  `KillTreeTests.test_probe_answers_for_the_named_pids_only`
  — MAPPED (anywhere; its Windows liveness check is Windows
  tier, revival the Windows run).
- **Census** (`campaign/census.json`, the vendored
  counted-ops enumeration) -> `CensusConsistencyTests`
  (recount, tree re-derivation, document holds) — MAPPED
  (anywhere; the tree re-derivation half skips, with its
  reason stated, when the tree is absent beside the suite).

### 2.2 The battery's case set and its BLOCKED-WINDOWS tier

The five cases (a), (l), (m), (n), (o) are mapped condition
by condition in Part 4. The case set as a surface ->
`BatteryShapeTests` (five cases present; no case passes
without Windows evidence; no case fails on its audit
parts), `DriverTierTests.test_every_case_has_a_driver_file`
— MAPPED (anywhere for the audit/stub tier; behavioural
execution Windows tier, Part 4's revival statement).

### 2.3 The dimension programme — eight dimensions

Ledger state, stated at both breadths the suite carries:
in the shipped configuration, dimensions (1) and (2) are
the freeze-gate dimensions (observed-passing / failing by
the gate over a run's records), (3), (4), (6) and (8) carry
collection declarations naming their instruments, and (5)
and (7) carry scope decisions (NOT-APPLICABLE by the
executed adjudication of round 26, recorded as data in
`campaign.json`); in the preserved reduced-run package's
rendered ledger the same eight read UNEVALUATED ×6 /
NOT-APPLICABLE ×2, recomputed at this landing.

- **(1) Correctness and output quality** ->
  `FreezeGateKillTests`, `OutputGateTests`,
  `SlotAccountingTests` — MAPPED (anywhere; real-arm
  verdicts Windows tier).
- **(2) Latency, throughput, startup** ->
  `FloorAndReportTests`, `PerContextTests`,
  `FloorSpawnTests` — MAPPED (anywhere; real-arm numbers
  Windows tier).
- **(3) CPU / memory / cost (collected: the runner's
  per-invocation resource instrumentation)** -> the
  Round26–Round41 cost-comparison classes
  (`Round31LandingTests`–`Round41LandingTests` as enumerated
  in §2.6), `Round35LandingTests` (censoring vocabulary),
  `Round36LandingTests` (per-field note accounting) and the
  new `CompletenessAccountingTests.test_resource_note_vocabulary_matches_the_runner_write_sites`
  (§5 below) — MAPPED (anywhere).
- **(4) Scaling (collected: the plan's scaling slots)** ->
  `Round26LandingTests.test_runner_scale_slot_kills_the_wide_tree`,
  `Round26LandingTests.test_runner_scale_slot_kills_the_deep_tree`,
  `Round27LandingTests`' scale-count holds — MAPPED
  (anywhere at stub tier; real-arm scaling Windows tier).
- **(5) Reliability (NOT-APPLICABLE by scope decision)** ->
  `ProfileDimensionTests.test_not_applicable_renders_only_via_decisions_entry_with_reason`,
  `ProfileDimensionTests.test_dimension_without_decisions_entry_cannot_render_not_applicable`
  — MAPPED (anywhere): the decision's rendering is tested;
  the dimension itself is a named non-execution by scope
  decision, revival a configuration decision recorded in
  `campaign.json` reversing the scope statement.
- **(6) Security / malformed inputs (collected: the plan's
  malformed slots)** -> `Round26LandingTests.test_runner_malformed_slot_records_the_arms_answer`,
  `Round31LandingTests`' designated-pin tests,
  `Round32LandingTests`' slot-bound pin tests,
  `Round37LandingTests`–`Round40LandingTests`' argv-budget
  derivation tests — MAPPED (anywhere at stub tier; real-arm
  answers Windows tier).
- **(7) Compatibility (NOT-APPLICABLE by scope decision)**
  -> the same two `ProfileDimensionTests` names as (5) —
  MAPPED (anywhere) as a decision rendering; the dimension
  itself is a named non-execution by scope decision,
  revival as for (5).
- **(8) Usability (collected: the formalized manual check
  in `usability-evidence.json`)** ->
  `Round26LandingTests.test_usability_evidence_artifact_carries_the_formalized_check`,
  `Round27LandingTests.test_usability_state_is_computed_from_the_artifact`,
  `Round28LandingTests`' stated-rule holds,
  `Round29LandingTests`' three-valued state tests —
  MAPPED (anywhere) for the state computation; the manual
  check itself is NON-EXECUTION at manual/Windows-theory
  tier (every record stands at outcome "not-run"), revival
  a person present on an interactive Windows desktop
  executing the manual check in the tree's
  `CONTRIBUTING.md`, as `FAILURE-TREE.md` §5 records.

### 2.4 The sensitivity gate and its 250 ms floor

- **Owner-verification timing gate** (the bound estimated
  from the plain condition only, at both levels, median
  absolute deviation; the floor stated at 250 ms for the
  sampler-verified quiet condition, and no sensitivity
  below the floor claimed) -> `OwnerVerificationTimingTests`
  (5 tests: the sleeping-launcher stability control, the
  in-bracket sensitivity control at the floor, the
  plain-condition bound control, and its companions), held
  in the documents by
  `CensusConsistencyTests.test_verification_floor_citations_carry_the_quiet_qualification`
  and `Round25NeighbourTests.test_floor_citations_in_verification_are_qualified_and_held`
  — MAPPED (anywhere). The quiet qualification is part of
  the claim: a floor mover inside a load burst is read as
  noise by construction, as the gate's own statement
  records.

### 2.5 The ready-to-fly surface

- **`build.ps1`** (one command to build: MSVC location,
  release build of the four role programs and the test
  helper for `x86_64-pc-windows-msvc`, staging, identity
  pin verification, build record; the cargo build under a
  finite deadline of 1,800 s; preflight of the
  prerequisites it uses — cargo and the MSVC target —
  failing, naming the missing prerequisite and its remedy,
  before any stage runs) -> `BuildScriptTests`
  (7 tests, including
  `test_cargo_invocation_has_a_finite_deadline`, which holds
  the script text `$BuildDeadlineSeconds = 1800`) — MAPPED
  (anywhere) as script-text/mechanical checks; the script's
  execution is NON-EXECUTION at Windows tier, revival the
  ready-to-fly Windows run (one command: `.\build.ps1`).
- **`run.ps1`** (one command to run: campaign, then
  battery, then every test family; stage exit-code
  contract; build-record tree check; preflight of the
  prerequisites it uses — Python before stage 1 — failing,
  naming the missing prerequisite and its remedy, before
  any stage runs) -> `RunScriptTests`
  (10 tests), `BatteryExitContractTests` (4 tests) —
  MAPPED (anywhere) as script-text/mechanical checks;
  execution NON-EXECUTION at Windows tier, revival the
  ready-to-fly Windows run (one command: `.\run.ps1`).

### 2.6 The guard-facing surface whole (sanity p4-10 point D1)

This mapping is D1's cure artifact: the guard-facing
surface is stated whole, and the lists below show what is
covered and what is tier-labelled. By file: 91 tests in
the four guard-facing files plus 69 in the battery file =
160 including battery. By subject: those 160 plus the 152
per-test-classified tests inside
`tests/test_campaign_logic.py` (the re-judged
`GUARD_SUBJECT_TESTS` enumeration of round 51, every name
verified to exist at this landing) = 312 guard-facing of
647 counted once — 48%, just under half, as
`VERIFICATION.md` states it. The guard-driving subset
(54 tests) executes only on Windows against the built
package and skips elsewhere with the missing-binary
reason; that tier label applies to the F1–F10 families,
the context/rewriting vendor families and the scanner
differential's guard side, and their revival is the
ready-to-fly Windows run. The battery file's 69 are
anywhere-tier audit/stub tests whose behavioural subject
is Windows tier (Part 4). The 152 by-subject tests run
anywhere against stub arms and fixture records; the
verdicts they pin about real arms are Windows tier.

Guard-facing by file — the four guard files and the battery file, every test named:

`tests/test_guard_families.py` (36 tests):

- PartialOutputConstructionTests: `test_partial_output_payload_construction`, `test_partial_output_fixture_writes_flushes_then_outlives`.
- OutputPassthroughAssertionTests: `test_output_fixture_construction`, `test_exact_captures_pass_the_output_assertion`, `test_suppressed_stderr_fails_the_output_assertion`, `test_reversed_stdout_lines_fail_the_output_assertion`, `test_truncated_or_altered_bytes_fail_the_output_assertion`.
- NegativeControlTests: `test_missing_binary_raises_binary_missing`, `test_driver_deadline_is_finite`, `test_build_dirs_never_fall_back_to_the_tree_target_folder`.
- F1FastPathTests: `test_exit_codes_preserved`, `test_child_output_passes_through`, `test_missing_program_is_guard_error_125`, `test_shell_form_runs_the_decoded_command`.
- F2KillPathTests: `test_sleeper_killed_at_the_deadline_with_124`, `test_descendant_tree_dies_with_the_child`, `test_partial_output_survives_the_kill`.
- F3HookTests: `test_pretooluse_allows_a_guarded_command`, `test_pretooluse_denies_an_unguarded_command_with_2`, `test_pretooluse_denies_malformed_payload_with_2`, `test_cursor_allow_shape`, `test_unknown_vendor_is_a_usage_error_never_silent`, `test_gate_role_matches_full_role_on_allow_and_deny`.
- F4CliContractTests: `test_no_command_is_125_never_2`, `test_bad_timeouts_are_125`, `test_unknown_option_is_125`, `test_extension_options_need_extendable`, `test_help_is_zero`, `test_bat_program_is_refused_like_cmd`.
- F5ExtendableTests: `test_child_extension_outlives_the_initial_deadline`, `test_run_without_extension_ends_at_its_deadline`, `test_extend_outside_a_run_is_refused`, `test_contain_outside_a_run_is_refused`.
- F6AutoAndRoleTests: `test_auto_runs_a_program_form_command`, `test_auto_is_never_extendable`, `test_deadline_role_has_no_hook_mode`.

`tests/test_coverage_families.py` (31 tests):

- CapConstructionTests: `test_cap_payload_construction_sizes`.
- F7PayloadCapTests: `test_payload_exactly_at_the_cap_is_allowed`, `test_payload_past_the_cap_decides_on_its_first_cap_bytes`, `test_unterminated_payload_past_the_cap_is_denied`, `test_watched_string_beyond_the_cap_is_denied`.
- F8ContextVendorTests: `test_cursor_context_prints_the_installed_policy`, `test_antigravity_context_prints_the_installed_policy`, `test_context_vendors_fall_back_when_no_policy_is_installed`, `test_context_shape_on_the_staged_package`.
- F8AntigravityTests: `test_antigravity_rewrites_a_plain_run_command`, `test_antigravity_passes_an_already_wrapped_command`, `test_antigravity_allows_other_tools_untouched`, `test_antigravity_denies_a_malformed_payload`, `test_antigravity_denies_when_the_wrapper_is_missing`.
- F8PowershellAutoTests: `test_powershell_auto_wraps_a_plain_command_as_ask`, `test_powershell_auto_allows_under_bypass_permissions`, `test_powershell_auto_passes_an_already_wrapped_command`, `test_powershell_auto_denies_an_unclassifiable_payload`.
- F9PipeOwnershipTests: `test_each_pipe_handle_is_closed_in_exactly_one_site`.
- F9NonUnicodeArgvTests: `test_non_unicode_program_argument_is_a_guard_error`, `test_non_unicode_first_argument_is_a_guard_error`, `test_unicode_argument_reaches_the_program_search`.
- F10ResolverOrderTests: `test_path_folders_are_searched_in_order`, `test_current_folder_is_searched_before_path`, `test_no_default_current_directory_removes_the_current_folder`, `test_pathext_membership_gates_the_search`, `test_pathext_extensions_are_tried_in_order`, `test_name_with_a_pathext_suffix_is_probed_as_named`.
- F10ResolverEnvCaseTests: `test_child_env_replaces_a_Path_spelled_parent_entry`, `test_child_env_collapses_Path_and_PATH_parent_variants`, `test_child_env_without_a_parent_variant_is_unchanged`.

`tests/test_output_observation.py` (10 tests):

- OutputCaptureTests: `test_fast_slot_records_matching_output_observation`, `test_mute_arm_is_output_mismatch_with_the_expected_exit_code`, `test_garble_arm_alters_stdout_and_is_output_mismatch`, `test_kill_slot_carries_no_output_observation`, `test_floor_slots_carry_the_observation_their_child_defines`.
- OutputGateTests: `test_complete_fixture_dimension_1_rests_on_output`, `test_guard_output_mismatch_fails_dimension_1`, `test_missing_output_observation_is_unevaluated_never_passing`, `test_competitor_output_mismatch_is_competitor_loss_evidence`, `test_shipped_config_names_the_output_workload`.

`tests/test_scanner_differential.py` (14 tests):

- ReferenceDirectedTests: `test_documented_good_envelopes_are_allowed`, `test_documented_bad_envelopes_are_denied`, `test_role_dependent_paths`, `test_timeout_token_boundaries`, `test_last_key_wins_and_cmd_is_only_a_fallback`.
- CorpusTests: `test_corpus_is_deterministic_and_below_the_cap`, `test_corpus_payload_hash_matches_the_summary`, `test_corpus_summary_fields_match_the_regenerated_corpus`, `test_corpus_covers_the_named_categories`, `test_corpus_verdicts_are_not_vacuous`.
- NegativeControlTests: `test_misaligned_reference_is_caught`, `test_always_allow_guard_side_is_caught`.
- ScannerDifferentialTests: `test_scanner_matches_the_parsed_verdict_over_the_corpus`, `test_scanner_matches_on_window_edge_cases`.

`tests/test_battery_cases.py` (69 tests):

- BatteryShapeTests: `test_five_cases_present`, `test_no_case_passes_without_windows_evidence`, `test_no_case_fails_on_its_audit_parts`.
- CaseATests: `test_baseline_poll_located`.
- CaseLTests: `test_vars_os_baseline_located`, `test_env_fixtures_cover_the_differential_shapes`, `test_reference_sort_model_is_stable_and_case_folded`.
- CaseMTests: `test_per_site_audit_evidenced`, `test_audit_names_the_invalid_handle_collision`.
- CaseNTests: `test_verdict_surface_located`, `test_fallback_is_part_of_the_pass_criteria`.
- CaseOTests: `test_six_item_burden_plus_forged_packet`, `test_burden_prices_the_port_close`.
- FunctionScopedAuditTests: `test_audits_name_file_function_and_line`, `test_case_m_audit_enumerates_handle_consumers`, `test_unmutated_scratch_tree_audits_evidenced`, `test_counted_owner_fetch_deleted_refutes_case_m`, `test_sleep_relocated_to_unrelated_function_refutes_case_a`, `test_console_finish_renamed_refutes_case_n`.
- DriverTierTests: `test_stub_good_variant_evidences_every_condition`, `test_stub_broken_case_a_fails_the_budget_condition`, `test_stub_broken_case_o_fails_the_forged_packet_condition`, `test_stub_broken_variants_fail_l_m_n`, `test_missing_driver_reports_blocked_error_never_pass`, `test_stub_run_preserves_evidence_records`, `test_every_case_has_a_driver_file`, `test_run_ps1_invokes_the_battery_entry_point`, `test_driver_exit_code_is_two_for_error_conditions`, `test_malformed_driver_payloads_block_the_case`, `test_driver_verdicts_printed_before_a_bad_exit_are_not_used`.
- Round3ProbeTests: `test_stub_broken_case_a_remainder_mismatch_refutes`, `test_stub_broken_case_a_early_wait_return_refutes`, `test_case_a_trial_without_remainder_is_blocked_error`, `test_stub_broken_case_o_zero_packets_refute`, `test_stub_broken_case_o_four_op_teardown_refutes`, `test_case_o_forged_claim_without_observation_blocked`, `test_stub_broken_case_n_pending_ignored_refutes`, `test_case_n_pinned_copy_is_blocked_error`, `test_stub_broken_case_l_stale_channel_entry_refutes`, `test_case_l_missing_set_step_is_blocked_error`, `test_stub_broken_case_m_unchecked_consumers_refute`, `test_case_m_consumers_checked_requires_enumeration`, `test_case_m_driver_timeout_covers_sub_deadlines`, `test_driver_error_mid_run_is_blocked_error`.
- Round4ProbeTests: `test_stub_noisy_case_a_still_evidences`, `test_stub_broken_case_a_systematic_2x_latency_refutes`, `test_stub_broken_case_n_gate_label_bytes_refute`, `test_stub_broken_case_n_role_verdict_values_refute`, `test_stub_broken_case_l_nonfixture_divergence_refutes`, `test_stub_broken_case_l_matcher_divergence_refutes`, `test_case_l_ascii_matcher_is_the_tree_semantics`, `test_stub_broken_case_o_inheritable_port_refutes`, `test_case_o_noninheritance_without_spawn_record_blocked`, `test_stub_broken_case_o_unconfirmed_teardown_refutes`, `test_case_o_fallback_shared_observation_blocked`.
- Round5ProbeTests: `test_stub_broken_case_m_timeout_is_blocked_error`, `test_stub_broken_case_m_spawn_failure_is_blocked_error`, `test_case_m_decide_distinguishes_execution_from_failure`, `test_stub_truncated_case_a_is_blocked_error`, `test_stub_case_a_without_never_dies_is_blocked_error`, `test_stub_truncated_case_l_is_blocked_error`, `test_stub_truncated_case_m_is_blocked_error`, `test_stub_truncated_case_n_is_blocked_error`, `test_stub_case_o_zero_phases_truncated_blocked`, `test_stub_case_o_zero_trial_not_empty_refutes`, `test_stub_case_o_cycles_below_charged_blocked`, `test_stub_case_o_forged_truncated_blocked`.
- ProbeWinLHandleTests: `test_every_standard_handle_comes_from_get_osfhandle`.
- RecordedBatteryEvidenceTests: `test_recorded_battery_evidence_matches_a_fresh_run`.
Guard-facing by subject inside `tests/test_campaign_logic.py` — the 152 re-judged names, by class:

- BatteryExitContractTests (4): `test_exit_code_constants_are_distinct_and_not_the_crash_code`, `test_exit_codes_distinguish_decision_from_execution`, `test_fail_decision_still_writes_the_battery_report`, `test_real_battery_blocked_windows_exits_zero`.
- CampaignOutcomeContractTests (3): `test_complete_and_freeze_fail_outcomes`, `test_perfect_fixture_with_shipped_decisions_is_complete`, `test_perfect_fixture_without_decisions_is_exactly_six_losses`.
- CompetitorLossKindTests (3): `test_one_measured_competitor_exit_mismatch_is_evidence`, `test_one_measured_survival_is_exactly_one_competitor_loss`, `test_warmup_and_control_kind_survivals_mint_no_competitor_loss`.
- FloorAndReportTests (9): `test_complete_campaign_has_no_standing_losses`, `test_floor_is_the_agreement_of_the_minimal_tools`, `test_floor_refused_when_the_minimal_tools_disagree`, `test_floor_tolerance_matches_the_bundle_agreement`, `test_guard_descendant_survival_is_a_standing_loss`, `test_no_distance_reported_when_the_floor_arms_disagree`, `test_nuewframe_facts_stated_exactly`, `test_report_headline_is_distance_above_floor_not_a_race`, `test_warmup_records_do_not_move_the_headline_medians`.
- FloorSpawnTests (3): `test_both_floor_arms_are_real_process_spawns`, `test_floor_agreement_when_the_floor_medians_are_close`, `test_sleeper_floor_arm_breaks_agreement_and_suppresses_distance`.
- FreezeGateKillTests (5): `test_guard_descendant_survived_kill_slot_still_fails`, `test_nuewframe_all_descendant_survived_is_losing_not_a_campaign_loss`, `test_rival_all_infra_failure_is_not_a_correctness_loss`, `test_rival_partial_survival_loses_correctness_keeps_latency`, `test_rival_with_no_kill_records_stays_unevaluated`.
- HarnessDeadlineKillTests (1): `test_hanging_arm_stays_infra_failure_even_with_a_survivor`.
- KillTreeTests (5): `test_direct_only_arm_leaves_descendant_survived`, `test_missing_probe_is_infra_failure_never_ok`, `test_probe_answers_for_the_named_pids_only`, `test_survivor_with_wrong_exit_is_classified_by_survival`, `test_tree_killing_arm_leaves_no_survivor`.
- PerContextTests (2): `test_absent_context_produces_no_number`, `test_context_medians_are_never_pooled`.
- ProfileDimensionTests (2): `test_four_dimension_pass_with_rest_unevaluated_renders_incomplete`, `test_reduced_run_report_lists_all_eight_dimensions_with_states`.
- Round21NeighbourTests (1): `test_guard_kill_path_correctness_does_not_pass_over_survivor_true_under_exit_mismatch`.
- Round26LandingTests (10): `test_collected_dimensions_render_from_the_runs_records`, `test_collected_dimensions_without_records_are_unevaluated`, `test_resource_dimension_needs_fields_or_reasons_on_records`, `test_runner_malformed_slot_records_the_arms_answer`, `test_runner_non_unicode_shape_names_its_presentation_limit_here`, `test_runner_records_resource_fields_on_a_stub_slot`, `test_runner_scale_slot_kills_the_deep_tree`, `test_runner_scale_slot_kills_the_wide_tree`, `test_scale_child_killed_before_publication_is_infra_failure`, `test_usability_evidence_artifact_carries_the_formalized_check`.
- Round27LandingTests (12): `test_combined_malformed_and_resource_mutation_is_incomplete`, `test_deep_chain_starts_at_level_1_like_the_in_tree_chain`, `test_malformed_records_hold_their_exit_code`, `test_resource_dimension_fails_when_an_arm_is_unmeasured`, `test_resource_pass_states_per_arm_coverage_in_the_ledger`, `test_resource_values_are_held_per_record`, `test_runner_deep40_exact_count_passes`, `test_runner_deep40_truncated_build_is_infra_failure`, `test_runner_wide72_exact_count_passes`, `test_runner_wide72_partial_publication_is_infra_failure`, `test_scale_ok_record_must_publish_the_variants_exact_count`, `test_usability_state_is_computed_from_the_artifact`.
- Round28LandingTests (7): `test_resource_presence_hold_names_keys_deleted_records_of_every_kind`, `test_runner_scale_classification_counts_distinct_pids_excluding_the_root`, `test_scale_count_hold_binds_distinct_pids_at_wide72`, `test_scale_count_hold_rejects_root_substitutions`, `test_usability_collected_requires_a_vocabulary_outcome_and_a_recorded_observation`, `test_usability_collected_requires_the_boolean_exercised_and_a_declared_class`, `test_zero_peak_is_an_inconsistency_and_counts_as_no_measurement`.
- Round29LandingTests (8): `test_deep_record_naming_its_root_in_child_pid_passes_the_count_hold`, `test_fractional_peak_is_named_and_counts_toward_no_coverage`, `test_numeric_holds_are_total_over_extreme_values`, `test_resource_presence_hold_partial_deletion_matrix`, `test_runner_deep_classification_counts_the_published_list_itself`, `test_usability_fail_only_renders_fail_with_a_standing_loss`, `test_usability_pass_only_renders_collected_and_mixed_renders_fail`, `test_variant_aware_count_hold_still_rejects_substitutions_and_repetitions`.
- Round30LandingTests (4): `test_deep_arm_pid_overpublication_fails_in_runner_and_checker`, `test_honest_scale_constructions_stay_ok_and_clean`, `test_one_predicate_agrees_over_the_count_matrix`, `test_wide_root_overpublication_fails_in_runner_and_checker`.
- Round31LandingTests (8): `test_malformed_expectation_table_is_designated_and_per_shape`, `test_resource_3x_fixture_fails_the_dimension`, `test_resource_comparison_is_order_invariant_across_rep_labels`, `test_resource_comparison_tie_is_exactly_zero_on_each_aggregate`, `test_resource_honest_and_cheaper_fixtures_pass`, `test_security_competitor_codes_and_unpinned_shape_move_nothing`, `test_security_designated_exit_0_fails_the_dimension`, `test_security_exit_mismatch_label_is_held_to_the_pin`.
- Round32LandingTests (7): `test_cost_comparison_is_order_invariant_under_rep_relabelling`, `test_cost_comparison_is_variant_resolved_on_the_scale_path`, `test_cost_total_loss_cannot_pass_on_a_favourable_median`, `test_cost_unpaired_cell_is_named_and_the_dimension_is_unevaluated`, `test_pin_applies_to_warmups_as_the_runner_faults_them`, `test_pin_binds_the_slots_encoded_shape_not_the_carried_label`, `test_variant_label_disagreeing_with_its_slot_is_named`.
- Round33LandingTests (8): `test_cost_comparison_never_compares_unequal_populations`, `test_cost_maximum_sees_the_tail`, `test_cost_unit_numeric_on_a_minority_cannot_pass`, `test_malformed_argv_transplant_is_named`, `test_scale_relabelled_wide72_records_are_named_and_do_not_pass`, `test_scale_variant_consumers_read_the_slot_derivation`, `test_sorted_rank_dominance_judges_the_peak_counter`, `test_unpaired_units_are_rendered_in_profile_ledger_and_report`.
- Round34LandingTests (8): `test_exactly_half_censored_renders_unpaired_counts`, `test_float_rep_malformed_pin_evasion_is_named_in_the_verdict_layer`, `test_float_rep_scale_relabel_is_named_in_the_verdict_layer`, `test_honest_pass_renders_compared_unit_counts`, `test_hybrid_argv_presenting_all_six_shapes_is_named`, `test_negative_and_bool_reps_derive_nothing_and_are_named`, `test_peak_mean_dominance_loss`, `test_sorted_rank_dominance_names_the_failing_ranks`.
- Round35LandingTests (7): `test_bool_rep_on_a_kill_record_is_named_in_the_verdict_layer`, `test_censored_half_cell_renders_unpaired_not_a_pass`, `test_dominance_loss_entry_carries_its_magnitude`, `test_float_rep_on_a_fast_record_is_named_in_the_verdict_layer`, `test_junk_tail_argv_is_named_by_the_one_vector_hold`, `test_null_counter_with_a_non_vocabulary_note_is_unaccounted`, `test_sparse_symmetric_censoring_flips_its_unit`.
- Round36LandingTests (5): `test_cpu_only_fully_censored_cell_renders_unevaluated`, `test_fully_censored_cell_is_enumerated_and_unevaluated`, `test_unaccounted_and_missing_key_nulls_are_not_censored`, `test_uniform_fast_extension_cannot_move_the_argv_budget`, `test_unwritable_note_shapes_are_named_in_every_population`.
- Round37LandingTests (4): `test_deleted_cell_is_enumerated_and_unevaluated`, `test_deleted_kill_path_is_enumerated_and_unevaluated`, `test_minimal_anchor_package_names_the_underived_budget`, `test_minimal_config_names_the_underived_argv_budget`.
- Round38LandingTests (6): `test_absent_anchor_package_surfaces_agree`, `test_exactly_half_deletion_stays_compared`, `test_partial_deletion_renders_unpaired_against_the_plan`, `test_program_missing_gap_names_the_program`, `test_sentinel_junk_fixture_cannot_certify`, `test_wrong_type_components_named_unusable_not_absent`.
- Round39LandingTests (4): `test_derived_length_holds_the_exactness_limb_and_template_token_still_gaps`, `test_nonstring_program_and_args_elements_derive_the_token_count_length`, `test_planned_variant_resolution_unmoved_by_the_unplanned_gate`, `test_unplanned_cell_renders_unplanned_at_plan_count_zero_and_never_compares`.
- Round40LandingTests (4): `test_derivation_mirrors_the_constructions_tokenization`, `test_gate_keys_by_the_slot_derived_variant_on_every_path`, `test_plan_zero_rule_binds_per_cell_arm`, `test_unplanned_records_are_excluded_from_every_evidence_consumer`.
- Round41LandingTests (5): `test_arm_identity_is_held_against_the_records_second_encoding`, `test_every_unplanned_records_fault_is_listed_in_its_own_name`, `test_the_freeze_gates_label_readers_judge_the_planned_population`, `test_the_planned_population_is_multiplicity_exact`, `test_variant_absence_is_named`.
- Round42LandingTests (2): `test_arm_identity_is_held_under_the_platforms_path_semantics`, `test_every_unplanned_records_fault_is_listed_in_its_own_name_at_every_kind`.
- Round43LandingTests (2): `test_executable_identity_is_decided_on_canonical_component_sequences`, `test_the_own_name_listing_is_complete_over_arm_kind_and_path`.
- Round44LandingTests (1): `test_padded_dot_components_never_manufacture_an_identity`.
- SlotAccountingTests (2): `test_exit_mismatch_excluded_from_medians_and_listed`, `test_stub_arm_exiting_125_on_fast_is_exit_mismatch`.

[↑ Back to Contents](#contents)

## Part 3 — Documentation claims

Every load-bearing claim in `README.md`, `VERIFICATION.md`
and `FAILURE-TREE.md`. A claim is KEPT BY TEST where a named
test holds it, KEPT BY MECHANISM where a named non-test
mechanism keeps it, NON-EXECUTION where the claim itself is
tier-labelled, and UNKEPT where nothing keeps it — an
unkept claim is a finding, quoted, not restated.

Enumeration method (stated so the enumeration's own
completeness is checkable; re-executed at the round-53
landing, F2): the sources are enumerated by structure —
every `README.md` section (`##` heading) and every
numbered `VERIFICATION.md` item (1–7) plus its closing
sections, in order — and within each, every sentence
carrying a number, a tier, a verdict rule, or a
ready-to-fly instruction is a candidate claim. Each
candidate is assigned to exactly one part of this
accounting: a claim whose subject is a failure-tree
branch is enumerated in Part 1, a feature surface in
Part 2, a battery condition in Part 4, and a claim
assigned to none of those is enumerated here. (README's
Layout section is a descriptive inventory of the files
present, carrying no number, tier, verdict rule or
instruction, and is excluded on that stated ground.)
The round-53 sweep of that enumeration found one claim
family assigned nowhere — the scripts' preflight of
their prerequisites, now the row below — and judged the
remaining near-misses assigned as follows: the
provisional-manifest / reportable-namespace sentences
are the standing-losses row's own subject (its named
class, `StoppedCampaignTests`, is the provisional
manifest's test class); the anchor-arm sentence is the
arm-identity row's subject; the pricing dispersion /
uncertainty sentences are the decomposition row's
subject (held there by `PricingUncertaintyTests`); the
campaign's pre-flight admission is a Part-2 surface
(§2.1, Admission) and a Part-1 row; the stage exit-code
contract is the run.ps1 row's subject and Part 4's
standing conditions. The denominator below is the
enumeration's count under this method.

- **Census, counted once (647 as this accounting lands,
  restated at the round-53 landing and at the round-60
  landing;
  593 execute / 54 skip; by-file and by-subject splits)**, 
  stated in all three documents and in
  `evidence/test-run.txt` -> KEPT BY TEST:
  `CensusConsistencyTests.test_documented_counted_once_totals_match_the_runner`,
  `CensusConsistencyTests.test_recorded_test_run_evidence_matches_the_runner_count`,
  `CensusConsistencyTests.test_recorded_split_matches_the_derived_split`,
  `Round26LandingTests.test_split_statement_in_verification_matches_the_census`.
- **Campaign size: 1,940 configured (36 control + 224
  warmup + 1,680 measured); 1,620 the reference
  decomposition of the original plan shape** (README,
  VERIFICATION, FAILURE-TREE §7) -> KEPT BY TEST:
  `test_plan_totals_match_configured_decomposition`,
  `CensusConsistencyTests.test_documents_scanner_and_campaign_figures_match_their_sources`.
- **Counted-ops census: plain path 45 named calls, hook
  path 18, run-core subtotal 36** (README, FAILURE-TREE §1,
  VERIFICATION item 6) -> KEPT BY TEST:
  `CensusConsistencyTests.test_counted_ops_census_recomputes_from_the_vendored_enumeration`,
  `CensusConsistencyTests.test_documents_printing_the_census_state_the_recomputed_totals`.
- **The guard tree's own tests: 437 counted once** (README,
  FAILURE-TREE) -> KEPT BY TEST: the tree recount inside
  `CensusConsistencyTests.test_documents_printing_the_census_state_the_recomputed_totals`
  (skips, reason stated, when the tree is absent).
- **Glossary: 19 bullets / 20 terms, the count the glossary
  itself states** (README) -> KEPT BY TEST:
  `Round32LandingTests.test_glossary_census_states_and_holds_its_term_count`,
  `Round26LandingTests.test_glossary_definitions_land_in_the_readme_and_are_held`.
  **FINDING F-Acct-1 — CURED at the round-53 landing
  (cure (a), as charged):** at the accounting's first
  landing the glossary defined exactly its then-10 terms
  and the materialization vocabulary the rounds 41–51 work
  runs on (MATERIALIZATION, PROVEN, STATED, CANDIDATE,
  SETTLEMENT, STORING / MINTING as a classification of
  mappings, DEFERRED, CAPTURE, TRAVERSAL) was defined at
  no single findable point — its only definitions were
  `campaign/arms.py`'s comment block, organized as
  round-by-round restatements (sanity p4-10's D3). At the
  round-53 landing the ten definitions land in the
  glossary, from first principles, in the glossary's own
  style; the census lock is amended OPENLY in the same
  motion (9 bullets / 10 terms -> 19 bullets / 20 terms,
  the amendment named in the glossary's own statement and
  in both lock tests); and arms.py's round-organized
  restatements are replaced by the mechanism stated once,
  pointing at the glossary as the definitions' home.
- **Floor rule: two minimal arms, agreement within 5%,
  re-derived from the bundle's 50.9 / 52.5 ms pair; headline
  is distance above floor per context, never pooled**
  (README) -> KEPT BY TEST: `FloorAndReportTests`
  (including `test_floor_tolerance_matches_the_bundle_agreement`),
  `PerContextTests`.
- **Stale anchor pairing "66.5 vs 50.9" only with
  provenance** (README) -> KEPT BY TEST:
  `test_stale_pairing_appears_only_with_provenance`.
- **Decomposition prices only the expensive class, only
  from the campaign's own pricing probes, stamps
  cross-checked, hand-typed pricing reported
  "operator-supplied, not measured"** (README,
  VERIFICATION item 6) -> KEPT BY TEST: `PricingProbeTests`,
  `PricingStampTests`, `PricingProvenanceTests`,
  `PricingUncertaintyTests`.
- **Execution order is seeded; the checker re-derives it**
  (README) -> KEPT BY TEST: `SeededOrderTests` and the
  package-order holds in `harness.check_run_package`'s
  tests (`MalformedPackageTests`, `RecordedOutcomeTests`).
- **Every arm's identity is listed; a pin mismatch is a
  standing loss, the pin is the anchor, the staging record
  only an added consistency record** (README, VERIFICATION)
  -> KEPT BY TEST: `CompetitorIdentityTests`,
  `VendoredCompetitorBundleTests`.
- **Competitor identities and versions: ProcessKit CLI
  v0.3.4, nuewframe/timeout v1.0.0, sysprims v0.2.4, pins
  from the preserved bundle's own run-manifest** (README,
  VERIFICATION) -> KEPT BY TEST:
  `VendoredCompetitorBundleTests.test_run_manifest_hashes_and_sizes_match_the_campaign_pins`,
  `VendoredCompetitorBundleTests.test_vendored_competitor_binaries_match_the_campaign_pins`
  (hash side, anywhere); the version probe at staging is
  NON-EXECUTION at Windows tier, revival the build run.
- **Kill claims about nuewframe/timeout stated exactly**
  (README) -> KEPT BY TEST:
  `FloorAndReportTests.test_nuewframe_facts_stated_exactly`.
- **The kill child is a tree; a kill slot with no probe,
  no recorded identity or an unanswered probe is an
  infrastructure failure, never "ok"** (README) -> KEPT BY
  TEST: `KillTreeTests`.
- **Exit codes checked against the per-arm table;
  mismatches excluded from every median;
  survivor-before-exit precedence** (README) -> KEPT BY
  TEST: `SlotAccountingTests`, `CompetitorLossKindTests`,
  the Round13 survivor-precedence controls in
  `RecordConsistencyTests`' family.
- **Output is observed on the fast path; a guard output
  mismatch fails output correctness; a guard fast record
  with no observation leaves dimension (1) UNEVALUATED**
  (README) -> KEPT BY TEST: `OutputCaptureTests`,
  `OutputGateTests`.
- **Completeness in the campaign sense is exact slot
  accounting plus the freeze gate plus the eight-dimension
  profile; while any dimension is UNEVALUATED the campaign
  is INCOMPLETE** (README) -> KEPT BY TEST:
  `SlotAccountingTests`, `ProfileDimensionTests`,
  `ClaimLedgerTests`, `CampaignOutcomeContractTests`.
- **Completeness in the phase sense is this accounting**
  (README, as restated at this landing) -> KEPT BY
  MECHANISM: this file, executed as the phase's closing
  step and recorded in the landing log with its per-part
  counts; its review is round 53, named in
  `VERIFICATION.md`'s closing list as the step that follows.
- **Kill-path correctness is comparative; an all-survived
  arm has no kill latency to lose to** (README) -> KEPT BY
  TEST: `FreezeGateKillTests`.
- **Standing-losses rule: a stopped campaign is an
  incomplete campaign with its standing losses listed,
  never a result** (README) -> KEPT BY TEST:
  `StoppedCampaignTests`,
  `FloorAndReportTests.test_report_headline_is_distance_above_floor_not_a_race`.
- **The cap-over-default scope decision** (README's "one
  explicit scope decision"; FAILURE-TREE §5) ->
  NON-EXECUTION at manual/Windows-theory tier, as the
  documents themselves state: the interactive remainder
  (a real Windows Hello request, a real password at the
  credential dialog or console prompt) is exercised only
  by the manual check in the tree's `CONTRIBUTING.md`,
  formalized for this package as `usability-evidence.json`;
  revival is that manual check on Peter's local Windows
  run. The non-interactive seams are in-tree tests, mapped
  in Part 1 §5.
- **The reduced-run figures: INCOMPLETE, 82 standing
  losses, floor medians 52.1 / 50.8 ms (NO_WINDOW) and
  51.2 / 51.9 ms (DETACHED), pricing medians −1.1 / −12.0 /
  −0.0 ms, seed 20261029, 40 slots, 18 paired observations,
  no freeze-gate FAIL** (VERIFICATION item 3) -> KEPT BY
  TEST: `CensusConsistencyTests.test_verification_item3_figures_match_the_preserved_reduced_package`
  (with its beside-figures extension).
- **The preserved package still prices its three items
  at the item-3 medians, restated in item 6's
  harness-integrity parenthetical (medians −1.1 / −12.0 /
  −0.0 ms, "as recorded in item 3")** (VERIFICATION
  item 6) -> KEPT BY TEST (round-60 F9):
  `CensusConsistencyTests.test_verification_item6_pricing_figures_match_the_preserved_reduced_package`
  recomputes the medians from the package's own records
  and holds the item-6 sentence against them. **FINDING
  — CURED at the round-60 landing:** at the round-60
  audit this sentence carried a superseded generation's
  figures (−187.0 / 165.8 / −241.0 ms), was mapped
  nowhere in this accounting, and was held by no test;
  the sentence is corrected to the figures the package
  produces and stands mapped here, so a regeneration
  cannot strand it again.
- **The degraded set is exactly 5 and each is named
  unverifiable, never silently passed** (VERIFICATION
  item 3) -> KEPT BY TEST:
  `Round25NeighbourTests.test_package_checker_names_the_full_degraded_set`;
  re-executed at this landing (`check_run_package` returns
  exactly 5, Part 1).
- **The reduced package's stored renders are round-37
  recomputations over the run's records, not the run's own
  writes** (VERIFICATION item 3, round-38 F4 disclosure) ->
  KEPT BY TEST:
  `Round38LandingTests.test_verification_item3_discloses_the_render_refresh`.
- **Battery: all five cases BLOCKED-WINDOWS on Linux, every
  source-audit condition evidenced and none refuted; the
  coordinates item 4 states** (VERIFICATION item 4) ->
  KEPT BY TEST:
  `RecordedBatteryEvidenceTests.test_recorded_battery_evidence_matches_a_fresh_run`,
  `CensusConsistencyTests.test_verification_section4_prose_coordinates_match_the_fresh_battery_record`,
  `CensusConsistencyTests.test_verification_section4_prose_subjects_and_duration_match_the_fresh_record`.
- **Syntax: every Python file compiles under `py_compile`**
  (VERIFICATION item 5) -> KEPT BY TEST (round-53 F6):
  `CompletenessAccountingTests.test_every_python_file_in_the_package_compiles`
  enumerates every `.py` file in the package (31 files at
  the round-53 landing, including `battery/drivers/*.py`
  and the probe files the runner never imports on this
  tier) and compiles each with `py_compile` (doraise).
  **FINDING F-Acct-2 — CURED at the round-53 landing:**
  at the accounting's first landing no test held this
  claim; it stood kept only by an executed check at that
  landing. The compile-all test is that finding's charged
  cure.
- **The sensitivity floor is 250 ms, stated for the
  sampler-verified quiet condition** (VERIFICATION items 1
  and 6) -> KEPT BY TEST: `OwnerVerificationTimingTests`
  (§2.4) and the floor-citation census tests named there.
  The quiet qualification travels with every citation the
  census holds; a citation without it would fail those
  tests.
- **"312 of 644 — 48%, just under half" (the by-subject
  guard-facing share)**, as restated at the accounting's
  first landing to 312 of 645 and at the round-53 landing to
  312 of 646 and at the round-60 landing to
  **312 of 647** (VERIFICATION item 1) -> KEPT BY TEST:
  `Round26LandingTests.test_split_statement_in_verification_matches_the_census`
  (§2.6 states the surface whole). The retired "roughly
  half" phrasing appears nowhere in the three documents,
  verified by search at this landing.
- **No Windows or MSVC number is claimed anywhere in the
  package; evidence tiers are stated at their true
  breadth** (README "What runs where", VERIFICATION) ->
  KEPT BY MECHANISM: the tier statements themselves, the
  skip-reason negative controls in `test_guard_families.py`
  (`NegativeControlTests`), and this accounting's
  tier labels; no test can prove a negative about prose,
  and the pre-closure sanity check is briefed on exactly
  this surface.
- **FAILURE-TREE's own coverage statement: "Implemented
  means runnable test code exists in `tests/`"** ->
  KEPT BY TEST: the citation census
  (`CensusConsistencyTests.test_failure_tree_citations_resolve_to_suite_tests`)
  resolves every cited suite test by AST; Part 1 of this
  accounting re-verified the row states against the
  inventory. No wrong FAILURE-TREE mapping was found at
  citation level; two rows' family cells name a family
  generically (section 7's "campaign stub-run tests" =
  `StubRunTests`; section 1's "campaign fast path,
  floor-based report" = `FloorAndReportTests`), which Part
  1 records at the specific names.
- **build.ps1 ready-to-fly claims: one command to build,
  the 1,800 s deadline, staging pin verification,
  build-record written on every build** (README) -> KEPT
  BY TEST at script-text breadth: `BuildScriptTests`
  (§2.5); the execution claim is NON-EXECUTION at Windows
  tier, revival the Windows build run.
- **run.ps1 ready-to-fly claims: one command to run, a
  re-run only adds a namespace, campaign INCOMPLETE cannot
  exit 0, battery FAIL is a decision and BLOCKED-ERROR an
  abort** (README) -> KEPT BY TEST at script-text breadth:
  `RunScriptTests`, `BatteryExitContractTests`,
  `RunNamespaceTests`; execution NON-EXECUTION at Windows
  tier, revival the Windows run.
- **The scripts' preflight of their prerequisites:
  `build.ps1` and `run.ps1` preflight the ones they use
  and fail naming the missing prerequisite and its remedy
  before any stage runs** (README, "The one build
  command", Prerequisites) -> KEPT BY TEST at
  script-text breadth:
  `BuildScriptTests.test_build_preflights_cargo_and_the_msvc_target`,
  `RunScriptTests.test_run_preflights_python_before_stage_1`;
  execution NON-EXECUTION at Windows tier, revival the
  Windows run. (Round-53 F2: this family was enumerated
  nowhere in this accounting's first landing — the word
  "preflight" occurred zero times in this file — though
  the named tests held it; it is mapped here and in
  §2.5's surface statements.)
- **The skip machinery's honesty: the 54 guard-driving
  tests skip with the missing-binary reason, and the
  negative controls prove the reason is the missing
  binary** (README, VERIFICATION item 1) -> KEPT BY TEST:
  `NegativeControlTests.test_missing_binary_raises_binary_missing`,
  `CensusConsistencyTests.test_recorded_split_matches_the_derived_split`
  (the gated count re-derived from the sources' decorators).
- **Scanner: a seeded corpus of 357 cases, 1,453
  comparisons per Windows run, corpus summary held at
  every field** (README, FAILURE-TREE §3, VERIFICATION
  item 7) -> KEPT BY TEST: `CorpusTests` (including
  `test_corpus_summary_fields_match_the_regenerated_corpus`),
  `CensusConsistencyTests.test_documents_scanner_and_campaign_figures_match_their_sources`;
  the guard side of the comparisons is NON-EXECUTION at
  Windows tier, revival the Windows run.

[↑ Back to Contents](#contents)

## Part 4 — Battery conditions

The battery is Windows-tier. Its Linux state is
non-execution BY TIER, stated once, exactly, here: **every
behavioural condition below is non-executed on this
machine because the battery's drivers require a Windows
machine with the built guard; the revival path for every
one of them is the ready-to-fly Windows run (`.\run.ps1`,
which invokes `battery\battery.py` against the built tree
and records verdicts and per-condition evidence in
`results\battery\`), and until that run each case stands
BLOCKED-WINDOWS — a tier-labelled non-execution, not a
pass and not a failure.** The source-audit conditions are
evidenced on Linux and mapped to the audit tests; the
drivers' decision logic is exercised on Linux in the stub
tier and mapped to the stub-tier tests that refute broken
variants of exactly these conditions.

### Case (a) — teardown wait vs Sleep(10) poll (3 conditions)

- **baseline poll located in source** (evidenced:
  `Sleep(10)` at `src/engine/wait.rs`, `wait_until_empty`,
  line 481) -> `CaseATests.test_baseline_poll_located`,
  `FunctionScopedAuditTests.test_audits_name_file_function_and_line`,
  `FunctionScopedAuditTests.test_sleep_relocated_to_unrelated_function_refutes_case_a`
  — MAPPED (anywhere).
- **verdict equivalence + latency + budget** (windows) ->
  stub tier: `DriverTierTests.test_stub_good_variant_evidences_every_condition`,
  `DriverTierTests.test_stub_broken_case_a_fails_the_budget_condition`,
  `Round3ProbeTests.test_stub_broken_case_a_remainder_mismatch_refutes`,
  `Round3ProbeTests.test_stub_broken_case_a_early_wait_return_refutes`,
  `Round4ProbeTests.test_stub_noisy_case_a_still_evidences`,
  `Round4ProbeTests.test_stub_broken_case_a_systematic_2x_latency_refutes`
  — NON-EXECUTION at Windows tier (revival above).
- **no lost wakeup at the emptying race** (windows) ->
  stub tier: the same `DriverTierTests` good/broken pair —
  NON-EXECUTION at Windows tier (revival above).

### Case (l) — environment block differential (2 conditions)

- **vars_os baseline located in source** (evidenced:
  `vars_os()` at `src/engine/environment.rs`,
  `environment_block`, line 72) ->
  `CaseLTests.test_vars_os_baseline_located`,
  `CaseLTests.test_env_fixtures_cover_the_differential_shapes`,
  `CaseLTests.test_reference_sort_model_is_stable_and_case_folded`
  — MAPPED (anywhere).
- **byte-level differential over the fixture set**
  (windows) -> stub tier:
  `DriverTierTests.test_stub_broken_variants_fail_l_m_n`,
  `Round3ProbeTests.test_stub_broken_case_l_stale_channel_entry_refutes`,
  `Round3ProbeTests.test_case_l_missing_set_step_is_blocked_error`,
  `Round4ProbeTests.test_stub_broken_case_l_nonfixture_divergence_refutes`,
  `Round4ProbeTests.test_stub_broken_case_l_matcher_divergence_refutes`,
  `Round4ProbeTests.test_case_l_ascii_matcher_is_the_tree_semantics`,
  `ProbeWinLHandleTests.test_every_standard_handle_comes_from_get_osfhandle`
  — NON-EXECUTION at Windows tier (revival above).

### Case (m) — pseudo-handle sites (2 conditions)

- **per-site comparison audit (source)** (evidenced: the
  four fetch sites and their consumers) ->
  `CaseMTests.test_per_site_audit_evidenced`,
  `CaseMTests.test_audit_names_the_invalid_handle_collision`,
  `FunctionScopedAuditTests.test_case_m_audit_enumerates_handle_consumers`,
  `FunctionScopedAuditTests.test_counted_owner_fetch_deleted_refutes_case_m`
  — MAPPED (anywhere).
- **per-site A/B behaviour on Windows** (windows; driver
  deadline 14,400 s, held to the battery's own
  `driver_timeout_s` by the §4 prose census) -> stub tier:
  `Round3ProbeTests.test_stub_broken_case_m_unchecked_consumers_refute`,
  `Round3ProbeTests.test_case_m_consumers_checked_requires_enumeration`,
  `Round3ProbeTests.test_case_m_driver_timeout_covers_sub_deadlines`,
  `Round5ProbeTests.test_stub_broken_case_m_timeout_is_blocked_error`,
  `Round5ProbeTests.test_stub_broken_case_m_spawn_failure_is_blocked_error`,
  `Round5ProbeTests.test_case_m_decide_distinguishes_execution_from_failure`
  — NON-EXECUTION at Windows tier (revival above).

### Case (n) — verdict surface across Console::finish (2 conditions)

- **verdict surface located in source** (evidenced:
  `Console::finish` at `src/cli_process.rs` line 173) ->
  `CaseNTests.test_verdict_surface_located`,
  `CaseNTests.test_fallback_is_part_of_the_pass_criteria`,
  `FunctionScopedAuditTests.test_console_finish_renamed_refutes_case_n`
  — MAPPED (anywhere).
- **verdict equality across the finish cycle** (windows)
  -> stub tier:
  `Round3ProbeTests.test_stub_broken_case_n_pending_ignored_refutes`,
  `Round3ProbeTests.test_case_n_pinned_copy_is_blocked_error`,
  `Round4ProbeTests.test_stub_broken_case_n_gate_label_bytes_refute`,
  `Round4ProbeTests.test_stub_broken_case_n_role_verdict_values_refute`
  — NON-EXECUTION at Windows tier (revival above). This
  condition is a neighbour of, not a test of, the
  console-restoration branch: battery case (p), mapped as
  a non-execution in Part 1 §1, is that branch's revival.

### Case (o) — IOCP teardown proof (8 conditions)

- **extendable precedent located in source** (evidenced:
  `CreateIoCompletionPort` inside `Extension::start` at
  `src/extend/server.rs` line 124) ->
  `CaseOTests.test_six_item_burden_plus_forged_packet` —
  MAPPED (anywhere).
- **zero-packet delivery, including the
  already-empty-at-association case** (windows) -> stub
  tier: `Round3ProbeTests.test_stub_broken_case_o_zero_packets_refute`,
  `Round5ProbeTests.test_stub_case_o_zero_trial_not_empty_refutes`
  — NON-EXECUTION at Windows tier (revival above).
- **packet-stream composition (honest dependence census)**
  (windows) -> stub tier: the `DriverTierTests` good
  variant and `Round5ProbeTests.test_stub_case_o_zero_phases_truncated_blocked`
  — NON-EXECUTION at Windows tier (revival above).
- **the never-dies budget case** (windows) -> stub tier:
  the good variant's never-dies record in
  `DriverTierTests.test_stub_good_variant_evidences_every_condition`
  (the stub probe's case-(o) payload carries the
  never-dies budget record the decider judges) —
  NON-EXECUTION at Windows tier (revival above).
- **fallback invariance** (windows) -> stub tier:
  `Round4ProbeTests.test_case_o_fallback_shared_observation_blocked`
  — NON-EXECUTION at Windows tier (revival above).
- **port close / disassociation effects** (windows) ->
  stub tier: `CaseOTests.test_burden_prices_the_port_close`,
  `Round3ProbeTests.test_stub_broken_case_o_four_op_teardown_refutes`,
  `Round4ProbeTests.test_stub_broken_case_o_unconfirmed_teardown_refutes`
  — NON-EXECUTION at Windows tier (revival above).
- **non-inheritance** (windows) -> stub tier:
  `Round4ProbeTests.test_stub_broken_case_o_inheritable_port_refutes`,
  `Round4ProbeTests.test_case_o_noninheritance_without_spawn_record_blocked`
  — NON-EXECUTION at Windows tier (revival above).
- **forged-packet case (binding conditions)** (windows)
  -> stub tier:
  `DriverTierTests.test_stub_broken_case_o_fails_the_forged_packet_condition`,
  `Round3ProbeTests.test_case_o_forged_claim_without_observation_blocked`,
  `Round5ProbeTests.test_stub_case_o_forged_truncated_blocked`
  — NON-EXECUTION at Windows tier (revival above).

### Standing battery conditions

- **The case set itself** (five cases, each with its
  recorded adoption conditions as pass criteria) ->
  `BatteryShapeTests.test_five_cases_present`,
  `DriverTierTests.test_every_case_has_a_driver_file` —
  MAPPED (anywhere).
- **The BLOCKED-WINDOWS derivations** (a case reports
  BLOCKED-WINDOWS when its behavioural conditions cannot
  be evidenced here; BLOCKED-ERROR — never PASS — when a
  driver is missing, dies mid-run, or returns a bad exit;
  the exit-code contract 0 / 2 / 3) ->
  `BatteryShapeTests.test_no_case_passes_without_windows_evidence`,
  `BatteryShapeTests.test_no_case_fails_on_its_audit_parts`,
  `DriverTierTests.test_missing_driver_reports_blocked_error_never_pass`,
  `DriverTierTests.test_driver_exit_code_is_two_for_error_conditions`,
  `Round3ProbeTests.test_driver_error_mid_run_is_blocked_error`,
  `BatteryExitContractTests`,
  `RecordedBatteryEvidenceTests.test_recorded_battery_evidence_matches_a_fresh_run`
  (the recorded record in `evidence/battery-linux.json`
  must equal a fresh run) — MAPPED (anywhere).
- **Driver-payload shape validation** (a malformed,
  truncated or verdict-carrying driver payload blocks its
  case; verdicts printed before a bad exit are not used;
  a stub run's evidence records are preserved) ->
  `DriverTierTests.test_malformed_driver_payloads_block_the_case`,
  `DriverTierTests.test_driver_verdicts_printed_before_a_bad_exit_are_not_used`,
  `DriverTierTests.test_stub_run_preserves_evidence_records`,
  `Round5ProbeTests`' truncated-payload tests
  (`test_stub_truncated_case_a_is_blocked_error`,
  `test_stub_truncated_case_l_is_blocked_error`,
  `test_stub_truncated_case_m_is_blocked_error`,
  `test_stub_truncated_case_n_is_blocked_error`,
  `test_stub_case_o_zero_phases_truncated_blocked`,
  `test_stub_case_o_forged_truncated_blocked`) — MAPPED
  (anywhere, stub tier).

[↑ Back to Contents](#contents)

## D2 disposition — unreachable input classes (sanity p4-10)

The round-51 review's evidence stands in the record and
was re-read at this landing: `harness.py`'s
`admission_failures` judges "a configuration consumed
without load_config" (harness.py:661) and reads token
lists through the shared materialization because a
payload-carrying list subclass can be a first reader
(harness.py:674); the derivation in `report.py` is
expressly built for mutation-refusing mappings
(MappingProxyType / frozen, named at report.py:1855 and
report.py:1991, and at arms.py:1382) that `json.load`
cannot produce; and `campaign_outcome`,
`check_run_package`, the derivation and the pricing
consumers all take config objects as parameters.
Disposition (a) is therefore taken, at exactly the breadth
that evidence supports, and the claim is written into
`VERIFICATION.md`'s closing section at this landing in
these terms: **config objects may reach the
materialization functions programmatically, without
load_config, from the suite's own consumers — the shapes
those consumers can actually carry.** Class by class:

- **Plain JSON shapes** (dict / list / str / number, via
  `load_config` -> `json.load`) — CLAIMED: the home case;
  every consumer downstream of the loader carries them.
- **Programmatically built config objects** (dicts and
  child specs constructed in code — the suite's own
  `stub_config`, the harness's constructions, packages
  recomputed by `check_run_package` from a manifest's
  anchored configuration) — CLAIMED: the consumers take
  config objects as parameters, so a spec value stated in
  code reaches the shared read without passing the loader.
- **Mutation-refusing mappings** (MappingProxyType, frozen
  dicts) **as spec mappings** — CLAIMED, tied to a
  consumer: the runner's construction reads its spec
  through `.get` and the derivation mirrors it; both
  expressly absorb a refused write-back and serve the
  identity cache. No claim is made that a whole
  configuration arrives in this shape from any shipped
  path.
- **Payload-carrying list subclasses** (a list whose
  `__iter__` yields from a one-shot payload) **as token
  lists** — CLAIMED, tied to a consumer:
  `admission_failures` is a first reader in a run's own
  order and reads through the shared materialization for
  exactly this shape (harness.py:674); the runner's and
  the derivation's reads of the same values follow it.
- **One-shot iterables and generators as spec values** —
  CLAIMED, tied to a consumer: the runner's construction
  (`runner._materialized_args`, round-41 F4) materializes
  a spec value stated as a one-shot iterator in a
  programmatically built spec, and the derivation reads
  the same object.
- **Minting mappings** (a factory minting a distinct
  object per access) — CLAIMED (round-53 F3, re-judged
  under the executed consumer criterion): a MintingMap
  spec placed as `children["fast"]` is carried by BOTH
  named consumers — `report._plan_fast_argv_derivation`
  -> (7, None) and `runner.child_for` -> the honest
  vector, twice agreeing (the round-53 reviewer's
  executed probe, re-executed at this landing) — and the
  suite's own `Round44LandingTests` routes exactly this
  class through exactly these consumers. The first
  landing's stated basis ("no shipped consumer constructs
  or carries one") applied a stricter test to this class
  than to the CLAIMED classes above and is withdrawn.
- **Copying mappings / stores** — CLAIMED (round-53 F3,
  re-judged under the same executed criterion, by this
  landing's own execution): a CopyStoreMap spec (its
  `__getitem__` returns a copy of the stored value) is
  carried by both named consumers, agreeing twice, and
  its stated replacement is re-read by both (derivation
  (8, None); `child_for` the replacement vector) — the
  suite's own `Round47LandingTests.test_a_copying_stores_replacement_is_reread`
  routes exactly this class through exactly these
  consumers (probe and output in the landing log).
- **Memoizing mappings** — CLAIMED (round-53 F3,
  re-judged under the same executed criterion, by this
  landing's own execution): a MemoMap spec (its
  `__getitem__` caches its mint and returns the memo
  across a read's accesses) is carried by both named
  consumers, agreeing twice (derivation (7, None);
  `child_for` the honest vector) — the suite's own
  `Round47LandingTests.test_the_slot_classifies_a_per_read_minter_as_minting`
  routes exactly this class through exactly these
  consumers, and the round-50 memo-observable rule
  governs its settlement face (probe and output in the
  landing log).
- **Handshake iterators** (an `__iter__` that spends an
  element) — CLAIMED (round-53 F3): a HandshakeIterable
  as the fast spec's args is carried by both named
  consumers — derivation -> (8, None); `child_for` ->
  the honest single-read vector, exactly what one honest
  read renders (the round-53 reviewer's executed probe,
  re-executed at this landing) — and the suite's own
  `Round45LandingTests` routes exactly this class
  through exactly these consumers.
- **Restarting iterators** (a custom iterator whose
  `__iter__` resets the traversal) — CLAIMED (round-53
  F3): a RestartingIterator as the args is carried by
  both named consumers — derivation -> (7, None);
  `child_for` -> the honest vector, twice agreeing
  (executed as for handshake above) — and the suite's
  own `Round45LandingTests` routes exactly this class
  through exactly these consumers. The proven test
  (round-45 F3) still excludes them from write-back by
  construction; that exclusion is about write-back, not
  about consumer carriage.
- **Fresh-traversal mints** — NAMED EXCLUSION, not
  defensive depth: a factory whose every `iter()` call
  recreates its traversal over the shared source is held
  by neither the arms.py statement nor VERIFICATION's
  (round-48 F2, one breadth in both); its captures are
  not proven and its reads stand on the storing face, as
  both statements say.

[↑ Back to Contents](#contents)

## Standing observations carried from round 39

- **The vocabulary <-> write-sites mechanical tie** —
  CLOSED BY A NEW TEST at the accounting's first landing:
  `CompletenessAccountingTests.test_resource_note_vocabulary_matches_the_runner_write_sites`
  (§2.3, dimension (3)). Before that landing the tie
  between `report.py`'s closed collection-failure
  vocabulary (`RESOURCE_NOTE_VOCABULARY`, which its comment
  says is read off `runner.py`'s write sites) and the
  runner's actual note-generation sites was kept by
  comment and by the literal strings inside the
  Round35/Round36 tests; no test compared the two sets.
  The test compares them mechanically by AST, in both
  directions, including the parameterized prefix family
  and the peak-only pairing. Round-53 F4 restates the
  converse at its true breadth: the accounting's first
  form tied the forward direction but its converse was a
  four-substring wording filter (a new note phrased
  without those substrings passed it). The converse now
  enumerates the runner's note-producing positions by
  AST structure — (a) every value written to a record's
  `resource_note` over the write routes the round-55 F3
  completeness argument enumerates: subscript stores
  (assignment, augmented, annotated, and — round-56
  F1 — a store through a subscript in every further
  binding position the language offers: for-loop
  targets, comprehension targets, and with-item
  targets, the binding forms that can carry a
  subscript store; a subscript write
  whose key is not a string literal is resolved through
  the module's own assignments, or by its constant
  blocks for an f-string key, and fails closed as
  unenumerable where its key cannot be resolved, as
  does an `update` argument the enumeration cannot see
  into — round-54 F6), dict literals at any position
  (the record dict literal's entry, and merge/rebind
  literals such as `{**record, k: v}` under literal or
  resolved keys), `dict(...)` constructions, the method
  writes `record.update(...)` (keyword and dict forms),
  `record.setdefault("resource_note", ...)`,
  `record.__setitem__(...)` and `record.__ior__(...)`
  (round-57 F3: the method spelling of the `|=` merge,
  bound and unbound, its argument inspected under the
  update discipline) (the value-less removals
  `.pop` / `.__delitem__` / `.clear` / `.popitem` are
  enumerated and produce no literal), the functional
  spellings `operator.setitem`, `operator.ior` and
  `operator.__ior__` (round-58 F2: the functional
  spellings of `__ior__`, enumerated by imported name
  and by module attribute in the runner and in reached
  helpers alike, under the update argument discipline;
  round-59 F3: the family is judged by denotation —
  resolution follows the binding's origin in the
  `operator` module, so a member reached by re-export
  through a package module is the operator function
  it denotes; round-60 F6: the reach to the module
  itself is judged the same way — a module expression
  resolving through a function-local alias of a
  module, and a star re-export's names judged by
  their origin in the `operator` module, are the
  operator function they denote)
  and the unbound
  `dict.__setitem__` / `dict.update` / `dict.setdefault`
  / `dict.__ior__`
  forms, getattr-computed writes resolved by their
  literal method name (an unresolvable method name
  fails closed), and writes through helper functions
  in ANY module of the package that the record is
  passed to — resolved through the calling module's
  imports (by imported name, by module attribute, by
  function-local import), transitively, with
  module-attribute resolution itself transitive
  (round-58 F3: an attribute chain through package
  modules, at any depth the package's own import graph
  reaches, resolves to its definition — for classes,
  walked at `__init__`, and functions alike — or fails
  closed; round-59 F4: a from-import re-export of a
  definition through an intermediate package module
  resolves to that definition, in a chain of any
  depth, for functions and classes alike, or fails
  closed where a record-flow value passes through a
  re-export link the enumeration cannot resolve;
  round-60 F7: the alias census behind that
  resolution covers the annotated alias beside the
  plain one, and the fail-closed half fires on a
  record-flow module-attribute call whose
  intermediate binding the enumeration cannot
  resolve, whatever the unresolved link's source —
  a re-export chain through a module outside the
  package tree fails closed by the same rule;
  round-60 F4: the fail-closed for an unresolved
  wrapper is not binding-scoped — an undecidable
  call receiving a package-produced value in any
  argument position whose value can reach the record
  (a binding, a merge, or a parameter a reached
  helper flows into the record) fails closed or is
  followed, at a delegation as at a binding, and the
  fail-closed search is total (round-60 F5: it
  inspects Call nodes only and the resolver answers
  None for any other node shape — a position is
  named on every argument shape, never raised on);
  round-60 F7: a builtins eval/exec re-exported
  through a package module is refused at its
  denotation, by module attribute or by imported
  name; round-61 F2, as an amendment, completed by
  round-62 F1–F4: resolution is
  over denotation as well as node shape — a
  definition with a non-empty decorator_list is
  judged at its decorated denotation, never at its
  undecorated body — functions, methods and classes
  alike: its decorators are applied
  bottom-up, each resolved to a package definition
  and judged by its own returns — a decorator whose
  every return is its argument unchanged (an
  identity decorator, determinable from its body
  only under the binding census below: the
  parameter's reaching binding at every return is
  the parameter itself, a rebind of the parameter
  defeating identity unless the rebind is provably
  the argument itself under the same census)
  leaves the denotation at the body it decorates; a
  decorator whose returns install one callable (a
  wrapper function it defines and returns, or a
  lambda it returns — for a class, a replacement
  class it defines and returns) makes that callable
  the denotation, and the installed callable's
  returns (for a class, its methods) are the
  denotation's subject (a wrapper is
  transparent, and leaves the denotation at the body
  likewise, only when every one of its returns is
  its captured function unchanged or a call of it
  whose arguments pass through in fact — each
  argument its own corresponding parameter unchanged
  or an expression provably denoting that parameter
  under the standing identity rules, with no
  substitution, no extra or dropped argument and no
  keyword re-binding; a constructed argument — a
  call, a dict, any non-identity value — breaks
  transparency, and an argument flow the judgment
  cannot resolve fails closed); the identity and
  transparency limbs read ONE binding census
  (round-62 F3, completed by round-63 F1-F3): the
  binding forms assignment, augmented assignment,
  annotated assignment, walrus, for-targets,
  with-item targets and delete, plus subscript /
  attribute stores — read against the parameter by
  the identity limb and against the captured
  function by the transparency limb; the census
  scan is dominance-aware, under the same
  control-flow discipline as the reaching rule
  above (round-63 F1: bindings are judged per
  standing path through the scope's statement
  structure — a restore certifies only on paths
  where it stands between the swap and the return,
  a swap on any reaching path defeats, and a path
  structure the scan cannot decide fails closed);
  a store is judged by the value its target's flow
  reaches (round-63 F2: through names, container
  elements, subscript extractions and call results
  under the standing resolve rules — a store whose
  reached value is provably disjoint from the
  judged value, a different proven source or an
  element replacement on a distinct container,
  defeats neither limb, and a target flow the
  census cannot resolve is never read as
  never-reaching and fails closed at either limb);
  comprehension targets are censused at their true
  grain (round-63 F3: a name target is the
  comprehension's own scope and is exempt; an
  attribute or subscript target is a store into
  the enclosing scope); a decorator
  that cannot be resolved to
  a package definition, a decorator expressed as a
  call, or a decorator whose installed callable
  cannot be determined from its returns fails closed
  at any record-flow call of the decorated name,
  exactly as an unresolvable callee does), with writes on
  a reached helper's record-carrying parameters and
  their aliases enumerated by the same routes, every
  dict literal in a reached helper's body enumerated
  likewise, and a followed helper's returned dict
  enumerated at its returns — a return binding
  operates for the record-producing module's own
  record too (round-56 F2: a helper's returned dict
  bound to the record name in `runner.py` itself,
  by a plain or an augmented rebind, a helper of
  any arity in any module, is enumerated at its
  returns; round-57 F1/F2: the binding operates in
  every form that can bind the record name from a
  return flow — tuple and starred unpacks, annotated
  assignments, loop-target and with-item name
  bindings, in the record-producing module and
  inside reached helpers alike — and over compound
  merge and rebind values, a helper call anywhere
  inside the value being followed at its returns
  (round-58 F1: the follow is over the whole value
  expression — the gate on the value's root node type
  is removed at both follow sites, so subscript-rooted
  and conditional-expression-rooted values, and every
  other expression form in value position, are walked
  end to end exactly as a BinOp-rooted value is; the
  one exception is a literal/container root bound as
  a whole — a dict literal is enumerated by its own
  route, and a list/tuple/set's elements reach the
  record name only through a followed extraction or
  a paired unpack; round-59 F1: the follow is over
  the record name's value flow within its scope — a
  helper's return bound to any local and then to the
  record name, by assignment chain or by extraction
  from a reached container, is followed at the
  helper's returns under the reaching rule
  (round-60 F1, restated control-flow sound and
  symmetric in both directions: a binding that
  dominates its use — before it in a statement
  sequence they share, with no conditional container
  between that sequence and the binding that the use
  stands outside of — replaces every earlier
  candidate whole, so a rebound intermediate
  replaces the earlier binding whole; a binding that
  does not dominate its use replaces nothing, and
  every candidate the rule leaves standing is
  followed, in the dead-branch case and its mirror
  alike; round-61 F1, as an amendment: the candidate
  set is reaching over iterations, not textual
  position — a binding inside a loop body is a
  candidate at every use inside that same loop's
  body (and at a while loop's test), and at every
  use inside any loop that encloses both the binding
  and the use (a back-edge through a nested loop
  included), whatever its textual position relative
  to the use; such a loop-carried candidate does not
  dominate its use by that route and is followed
  beside the standing candidates under the same
  symmetric rule, in the delegation pass's alternates
  as at the binding sites — a loop-carried flow is
  never excluded by position before dominance is
  judged; the bindings are collected over every
  binding form — assignment, annotated assignment,
  walrus, for-targets, comprehension targets and
  with-item targets alike, round-60 F3); round-60
  F2: a tracked value passed into a resolved helper
  is followed at the parameter the helper returns
  from, one step and two, and deeper to a stated
  depth cap of 6 (round-61 F3, as an amendment: the
  number 6 is stated here, in the tie test's
  R-comments and route comments, and in the cap's
  own unenumerable message, beside the code literal
  that enforces it), past which the flow fails
  closed; round-59 F2: a lambda-literal callee at a
  binding is walked at its body exactly as a
  factory-returned lambda is, and an unresolved
  callee receiving a package-produced value at a
  binding fails closed — an unresolvable callee is
  never a transparent wrapper for a package value)
  and a package module global named as a merge or
  rebind source being walked at its own module's
  globals), and a call-of-call callee the record is
  passed to is resolved to its returned callable's
  definition; a package class the record is passed
  to is resolved by either spelling — imported name
  or module attribute — and walked at its `__init__`
  (round-57 F4); a method on an instance of a package
  class at a record-flow position — a record-flow
  value passed to it, or a record-flow name bound
  from it — is walked at that method's decorated
  denotation by the same route (round-62 F1); a
  method on an instance returned by a package
  factory — a call whose callee resolves to a
  package function whose every return is a
  construction of one package class — is walked by
  the same route at that class's method (round-63
  F5); self-calls inside a walked method dispatch
  at the class being constructed — the
  construction or receiver site's class, a subclass
  override before the inherited definition over the
  package method resolution order, an inherited
  self-called method walked from a subclass
  `__init__` (round-63 F4); a record-flow value
  passed to a callable the enumeration cannot resolve
  (a call-of-call whose returned callable it cannot
  resolve included) fails closed, (b)
  every return value of `_windows_resource_fields`, and
  (c) every string constant assigned to a NOTE-named
  variable — resolves names through the module's own
  assignments, and fails on any expression shape at a
  note position it cannot enumerate; no wording filter
  remains. NAMED EXCLUSIONS, outside the enumerated
  routes: a write performed by a method on an instance
  (`x.method(record)`) whose class the enumeration
  does not resolve to a package class — a method on
  an instance produced by a call whose return class
  the enumeration cannot determine (a factory call
  whose returns are not all constructions of one
  package class) included; a write
  performed inside a non-package callable the record
  is handed to by attribute call; the record reached
  other than as a call argument or a followed return
  value (through a module global, a closure, or a
  default argument); and code generated at runtime
  outside the package's source (an eval/exec reach
  standing in the package's own source is not
  outside it: in any spelling — plain name, builtins
  attribute, getattr-computed by literal name, or
  subscript-computed into a mapping known to be the
  builtins namespace (the module itself, its
  `__dict__`, or `vars(builtins)`), resolved by its
  literal key with an unresolvable key on such a
  namespace failing closed —
  it fails closed, round-57 F5 / round-58 F4; round-59
  F5: the refusal is keyed on the namespace value,
  however reached — a local name bound to a namespace
  form, a copy of one inline or through a bound
  local, or the `__builtins__` entry of the module's
  own globals is the same namespace as the inline
  spellings, its flow tracked under the same reaching
  rule as the record's; round-60 F8: the namespace
  value is tracked over the same routes as the record
  value at full breadth — an extraction from a
  reached container, a crossing into a reached
  helper as an argument (the parameter is the
  namespace inside it) and out of one as a return,
  and a copy of globals() as of the namespace — one
  flow discipline for both tracked values; a mapping not known by one of
  these routes to be the builtins namespace is
  outside this refusal). The tie holds over
  the enumerated routes of (a)+(b)+(c), and no wider
  breadth than that is claimed. Classification: harness by
  subject (the instrument's own discipline; it renders no
  verdict about an arm or the guard).
- **The old 1-record sub-probe's n=1 edge** — NAMED
  NON-EXECUTION. The six round-31/32/33 predicate
  constructions were re-based at round 38 from small
  populations (1–3 records per arm) to plan-shaped
  populations (90 per arm), and no later landing re-probed
  the edge they vacated: a cost-comparison unit *compared*
  on exactly one record per side (n=1). That edge cannot
  arise under the shipped plan — the measured
  per-(arm, cell) populations of `plan.build_plan()` are
  exactly 3 or 90, recomputed at this landing — because a
  carried population of 1 in a planned cell is a minority
  of its plan and renders unpaired, which is tested:
  `Round39LandingTests.test_planned_variant_resolution_unmoved_by_the_unplanned_gate`
  (1 numeric of 3 planned renders unpaired as a minority;
  2 of 3 compares). The n=1 *compared* unit is therefore
  a non-execution at this tier; its revival path is a plan
  carrying a cell at 1 rep per arm (a configuration
  change), under which the sorted-rank dominance,
  median and total predicates would be probed at n=1 by
  the same construction the re-based tests use at n=90.

[↑ Back to Contents](#contents)

## What this accounting does not do

It does not close Phase 4. The campaign itself — the full
1,940-slot run on Windows, the battery's behavioural
verdicts, and every Windows-tier number — remains owed,
exactly as `VERIFICATION.md`'s closing list states. This
accounting was reviewed as round 53, whose four sustained
protests and two findings were cured at the round-53
landing this file now records; round 54 reviews that
landing, and the mandatory
pre-closure sanity check runs against the accounted
artifact before closure.

---

[↑ Back to Contents](#contents)

## E10 extension — Phase 5 completeness re-run over the reorganized tree

Executed at Phase 5 execution step E10 (design §9.1 R1–R6,
§9.2, §9.3), on the reorganized tree, from the suite's landed
home at `suite/` (workspace copy: `tree/suite/`; the workspace
source copy at `phase4-suite/` stays on disk — this landing is
a copy into `tree/suite/`, not a move). Tier of every result
in this extension: Linux run-tier, produced on this machine.
Windows-tier executions stay owed, named below. Charged
remote head at this step: `5ad55c6ce2052b7d99ad800117c8f8a40bcce08c`
(branch `gauntlet/process-tree-guard`), verified by readback
before work began.

### Landing inventory (§9.2)

- Files in: **53** (the §9.2 complete inventory, counted on
  disk at `phase4-suite/`, excluding `__pycache__` bytecode).
- Files out: **53** at the same relative paths under `suite/`,
  contents unchanged except the landing edits named below —
  plus **10 new files** written at this step under §5.4
  (standard-form READMEs in the suite subfolders:
  `battery/`, `battery/drivers/`, `benchmark/`,
  `benchmark/competitors-001/`,
  `benchmark/competitors-001/bins/`, `campaign/`,
  `evidence/`, `evidence/reduced-run/`, `fixtures/`,
  `tests/`). Landed total under `suite/`: **63 files**.
- The five empty fixture directories (`campaign/empty-0` …
  `empty-3`, `campaign/resolver-cwd`) are re-created empty;
  they carry no README — their emptiness is the fixture
  (§5.4's named exception).
- `campaign.json` lands byte-identical (verified by direct
  comparison). `usability-evidence.json` lands unchanged.
- The suite's own `README.md` lands path-updated only
  (assumption A5): the layout and `-Tree` default statements
  now state the shipped geometry; it is the suite's
  operating manual, in = out. It is **not** PTG-STE-1 prose:
  `ste_check` over it at the landed home returns 597 gating
  failures (M1 vocabulary). That state is a named finding
  for E11/E12's presentability verification, where the
  standard-form suite README draft stands as the held
  fallback (A5). The ten new subfolder READMEs pass
  `ste_check` clean (0 gating failures; two carry a stated
  file-local exception list for their folder's own name —
  `drivers`/`probe`, `reduced`).
- Manifest writing (§10 E10): the working manifest
  `tools/layout.json` now states `suite.home` = `suite` and
  `suite.campaign_config` = `suite/campaign.json` (shipped
  values, written in this landing; no other key changed;
  the file stands at 122 lines, as the dependency graph's
  proportion paragraph states).

### Landing edits (path updates, sourced from the manifest's keys)

The shipped geometry is: the suite at the repo root's
`suite/`, the guard tree at the suite's parent folder
(`tree.root`). Every suite-relative tree join was re-written
to that geometry:

- `battery/battery.py`: `DEFAULT_TREE` — the audit row 15
  edit (§3.6), from `../../tree` (workspace geometry) to
  `../..`, verified by execution: the fresh battery record
  at the landed home equals the recorded record (below, R5).
- `battery/drivers/driver_common.py`: `DEFAULT_TREE` —
  `../tree` → `..` (the suite's parent).
- `battery/drivers/probe_win_l.py`: its no-argument default —
  `../../tree` → `../../..` (Windows-tier probe; the spelling
  now resolves to the guard root from the drivers folder).
- `tests/test_battery_cases.py`: `TREE` — `../../tree` →
  `../..`.
- `tests/test_campaign_logic.py`: all 8 tree joins —
  `../../tree` → `../..`, `../../tree/MEASUREMENT.md` →
  `../../MEASUREMENT.md`, `../../tree/tests` → `../../tests`,
  `../../tree/src` → `../../src` (two sites); and the
  `RunScriptTests` pin of the two scripts' shared `-Tree`
  default re-spelled to the shipped geometry, with its
  comment (the pin's subject — build and run defaults are
  the same spelling — is unchanged).
- `build.ps1` / `run.ps1`: the `-Tree` default —
  `..\tree` → `..` in both, and `run.ps1`'s header sentence
  that states the default re-worded to the same geometry.
  `run.ps1` passes the resolved tree to `battery.py` as
  `--tree` and the suite's own `campaign.json` as `--config`
  (verified in the landed text; held by `RunScriptTests`).
- `README.md` / `VERIFICATION.md` (the manual pair): the
  layout bullet, the unnamed-`-Tree` statement, and the
  tree-presence statements now state the shipped geometry
  (`..`, the folder that holds the suite). No figure,
  count, or verdict statement in either file was touched.
- **T4 repair (charged at E10):** `run.ps1`'s header stated
  the 1,620-slot shape (36 control + 144 warmup + 1,440
  measured). Corrected in the open to the configured
  campaign as the record states it: **1,940 slots (36
  control + 224 warmup + 1,680 measured); 1,620 stands as
  the original shape's reference decomposition.** Reason:
  the stale header was a factual defect in shipped product
  text — the suite README, `campaign.json`'s configured
  plan, and the Phase 4 record (round 26) all state 1,940;
  the header alone still stated the pre-resize shape. The
  repair is held by execution: the plan-total hold
  (`test_plan_totals_match_configured_decomposition`) and
  the campaign-figures census pass at the landed home.

### R1 — Path translation

Every distinct path citation in Parts 1–4 of this accounting
was translated through §3.5's mapping table (evidence:
`hidden_files/e10-land/rerun-evidence.log`):

- 16 distinct path spellings (15 unique after normalization):
  12 resolve at the same path on the reorganized tree.
- **Moved:** `tree/src/sys.rs` → `tree/src/sys/windows.rs`.
  Its one occurrence in this accounting is the Part 1 §2
  spawn-race row ("the constant at `tree/src/sys.rs`"):
  read that citation as `tree/src/sys/windows.rs`
  (`PROC_THREAD_ATTRIBUTE_JOB_LIST` stands there now; the
  row's second citation, `tree/src/engine/attributes.rs`,
  is unmoved). The row's disposition is unchanged.
- **Split:** `tree/src/lib.rs` → `tree/src/lib.rs` +
  `tree/src/dispatch.rs`; `tree/src/cli.rs` →
  `tree/src/cli.rs` + `tree/src/cli/auto.rs` +
  `tree/src/cli/help.rs`. Citations to either file are
  re-pointed to all resulting homes; the two citations
  whose cited item has one exact new home are dispositioned
  under R6 below.
- **Land nowhere: 0.** No citation in this accounting stops
  covering the tree it names.

### R2 — Re-inventory

- Counted-once total at the reorganized state: **647** —
  reconciles with E0's 647 exactly, no cause to state.
  Both roads agree: `run_tests.py --count-only` prints 647
  from the landed home, and an AST inventory of the landed
  test sources counts 647 (per file: 487
  `test_campaign_logic.py` / 69 `test_battery_cases.py` /
  36 `test_guard_families.py` / 31
  `test_coverage_families.py` / 14
  `test_scanner_differential.py` / 10
  `test_output_observation.py` — the E0 splits, unmoved).
  By file: 487 campaign-logic / 91 guard-facing / 160
  including battery. By subject: 312 guard-facing / 335
  harness (held by `Round26LandingTests`' split test at
  the landed home).
- `GUARD_SUBJECT_TESTS` (152 names) re-verified name by
  name: the census test that holds the enumeration asserts
  every name exists in the landed sources, and passes at
  the landed home. Test function names are unchanged by
  the reorganization (§3.5), as designed.
- Full suite run at the landed home (Linux run-tier,
  serial, quiet box): **647 tests in 557.348s, OK (skipped=54) — 647 counted once, 593 executed / 54 skipped, 0 failures, 0 errors, EXIT:0, wrapper ELAPSED 558, load sampler 1-min peak 3.23 over 38 samples (a sampler-verified quiet box); log hidden_files/e10-land/run2.log, done-marker run2.done**. The first run at the
  landed state (647 in 515.506s, 1 failure, 54 skipped)
  caught a genuine landing defect, recorded here so the
  record is complete: §5.4 mandates a README in
  `benchmark/competitors-001/bins/`, and
  `VendoredCompetitorBundleTests` pinned that folder's
  exact contents to the three binaries. The cure, applied
  at this step and commented at the assertion: the pin
  stays exact and now names the mandated contents (the
  three pinned binaries plus `README.md`); any other
  extra file fails as before. No binary, pin, or hash
  changed. The friction is flagged for E12 as an
  execution-discovered design conflict between §5.4's
  folder list and the suite's pin, resolved toward both
  texts.

### R3 — Surfaces re-mapped

Part 2's surfaces re-stated against the reorganized tree:

- **Ready-to-fly:** `build.ps1` builds the four role
  programs and the test helper. The helper's source moved
  at E4 (D-C4): `tree/Cargo.toml` names its path as
  `tests/support/testkit.rs`, and that file stands on disk
  at the landed state — the build's staging and the
  suite's build-record checks were re-verified against
  the new path at E4 and stand here (`BuildScriptTests`
  green at the landed home; the script's execution stays
  Windows-tier, owed below).
- **Campaign machinery, battery case set, eight
  dimensions, sensitivity gate, guard-facing whole:** each
  surface's keeper tests pass at the landed home in the
  targeted re-run set (census, completeness-accounting,
  split, battery-evidence, build/run script, and degraded
  set classes — 100 tests, 0 failures, 0 errors) and in
  the full run. The battery's default tree and the test
  families' tree joins now resolve to the guard root
  (the suite's parent), verified by the fresh battery
  record equalling the recorded record (R5).

### R4 — Claims re-mapped, and extended

- Part 3's 35 documentation claims: every claim's citations
  resolve under R1's translation; the claims whose keeper
  is a suite test are held by that test at the landed home
  (the census family holds the documents' figures —
  campaign size, census totals, tree count 437, glossary
  count, item-3 and item-6 figures, §4 coordinates —
  against the reorganized tree and passes).
- The pure-set claim enters in §4.3's reduced pin-match
  form: the landed set is **12 files** — the eleven of E0
  plus `src/cli/help.rs`. Provenance, stated beside the
  entry as §9.1 requires: `src/cli/help.rs` was **computed
  by the live census at E3** at its designed content
  (D-C2's split birth); it did not stand at E0. The set as
  landed is stated identically in three places, verified
  at this re-run: `src/lib.rs`'s own doc lines, the
  manifest's `pins.pure_set`, and the files on disk (all
  12 exist). Keeper: the shipped gate's G3/G4 pin-match
  holds (the gate is kept outside this repository under
  the OD-PY1 ruling; it does not ship in this tree). Named non-execution: **L-T1** — full census
  re-derivation is not shipped and derivation-class drift
  is invisible to the shipped instruments; revival is a
  §4.3 revival run of the working instruments.
- **Extended Part 3 — claims the new documentation set
  makes about the tree**, each with its named keeper:
  - *The suite stands at `suite/`, at 647 tests counted
    once* (wiki `Testing-and-Benchmark.md`, filled at this
    step from the landed state) — KEPT BY EXECUTION: this
    re-run (R2's two roads and the full run).
  - *The counted-ops census is 45 plain / 18 hook / 36
    run-core* (wiki `Measurement.md` summary; repo
    `MEASUREMENT.md` full record) — KEPT BY TEST:
    `CensusConsistencyTests`' counted-ops tests at the
    landed home, re-deriving from the tree's
    `MEASUREMENT.md` at its reorganized path.
  - *The tools proportion: the working instruments total
    40,462 lines against the product source's 24,966
    lines, about 162 per cent, reported not gated*
    (`docs/dependency-graph.md`'s paragraph, §4.4's
    statement in its landed form) — KEPT BY RE-EXECUTION
    at this step: every listed tool's line count was
    re-counted from disk and matches the paragraph
    exactly (the manifest stands at 122 lines after this
    step's `suite.*` writing); the source side re-counts
    at 62 files / 24,966 lines. The instruments do not
    ship in this tree (OD-PY1 ruling); they are kept
    outside this repository, as the paragraph states.
  - *Tier statements in the wiki set and READMEs (Windows
    the one proven backend; Linux and macOS named places
    at their tiers)* — KEPT BY MECHANISM: the tier labels
    themselves and this accounting's non-executions;
    execution keepers are the owed Windows-tier runs
    named below.
  - *Folder README statements of what belongs where*
    (§5.4 set, including this step's ten suite READMEs) —
    KEPT BY MECHANISM: the landed tree itself, read
    against each README at E12's file accounting; the ten
    new READMEs pass `ste_check` at this step.

### R5 — Battery and degraded set

- Part 4's 17 conditions re-stated: their suite-side
  citations are path-stable under the landing; the
  tree-side audit citations resolve under R1. The fresh
  battery run at the landed home (Linux tier) **equals
  the recorded record** `evidence/battery-linux.json`
  exactly — verdicts BLOCKED-WINDOWS ×5, every audit
  coordinate at its reorganized-tree line — held by
  `RecordedBatteryEvidenceTests` at the landed home.
- **The degraded set (exactly 5) re-executed** against
  the preserved package at the reorganized state:
  `harness.check_run_package` over
  `evidence/reduced-run/` returns exactly the same five
  unverifiable verifications — config_templates,
  competitor identity, expected-output pairing, profile
  decisions, argv length budget. Count and membership
  stand, no cause to state. Held by
  `Round25NeighbourTests.test_package_checker_names_the_full_degraded_set`
  at the landed home.
- Standing footings recomputed: the preserved package
  reads **82 / 82 / 83** standing losses (package footing
  / explicit-sentinel footing / direct no-plan footing),
  the one disclosed vector standing as the symmetric
  difference — held by
  `CensusConsistencyTests.test_verification_item3_figures_match_the_preserved_reduced_package`
  at the landed home; its claim ledger reads UNEVALUATED
  ×6 / NOT-APPLICABLE ×2 (Part 2 §2.3's statement,
  recomputed by the dimension tests at the landed home).

### R6 — Failure-tree integrity

`FAILURE-TREE.md` is **not rewritten** by this re-run.
Two of its mappings are stale *because of Phase 5's
moves*; both are findings here, with the translation that
cures them, and the cure is applied to this accounting,
not silently to the tree file:

- §3 gate row: "implemented in `tree/src/lib.rs`,
  `entry_gate` / `entry_full`, through `hook::usage_error`"
  — translation: `entry_gate` and `entry_full` stand in
  `tree/src/dispatch.rs` (the D-C1 split), and
  `usage_error` stands in `tree/src/plugins.rs` (the D-C3
  split). The row's in-tree test citations
  (`tree/tests/battery.rs`) are unmoved.
- §6 auto row and §7 diagnostics row: the
  `auto_timeout_setting` unit tests and the
  `take_diagnostics` assertions cited to `src/cli.rs` —
  translation: both stand in `src/cli/auto.rs` (the D-C2
  split; `auto_timeout_setting` at `src/cli/auto.rs`,
  its tests in the same file). `tests/auto.rs` is
  unmoved.
- No other `FAILURE-TREE.md` mapping names a moved path:
  the file contains no citation to `src/sys.rs`,
  `src/bin/testkit.rs`, `tests/antigravity.rs`,
  `src/hook/antigravity.rs`, or `src/hook/powershell.rs`
  (verified by search at this re-run).
- The reorganization does **not** touch any
  `resource_note` write site: the runner is suite-side
  and moved whole (§9.3's stated expectation, verified —
  no finding arises in either direction).

### §9.3 ADDITION — the named open item, carried

> "resource_note vocabulary tie — converse completeness
> unproven; escape classes: call-spelled writes, cross-module
> helper write, dict-merge rebind under variable key,
> indirection hiding the executing callable (helper
> pass-throughs, alias chains, inheritance, `__code__`
> replacement). Suite green at terminal state; guard and
> measurements unaffected."

This item is an **addition** to this accounting: it did not
stand in `COMPLETENESS.md` before this extension (its
on-record homes were the STATUS.md / CONTRACT.md ruling
record and the prepared issue text at
`hidden_files/issue-resource-note-tie.md`). It now stands
here as a named open item appended to the accounting's
standing footings, and in the wiki's
`Testing-and-Benchmark.md` under its own name — its two
permanent homes (§9.3).

- **State: OPEN, carried — not closed.** Phase 4 closed
  by Peter's ruling, not by silence, with this item
  standing. Phase 5 does not close it, does not downgrade
  it, and its own closure does not imply the suite's
  completeness is unqualified.
- **The property:** no code path in the suite writes a
  `resource_note` value outside the approved vocabulary
  (`RESOURCE_NOTE_VOCABULARY`). The forward direction is
  held by
  `CompletenessAccountingTests.test_resource_note_vocabulary_matches_the_runner_write_sites`
  (green at the landed home in this re-run); the converse
  — that the checker's enumeration covers every write
  position — is the unproven half. The covered set and
  the enumeration's breadth stand in this file's
  "Standing observations" section, at their true breadth,
  with their named exclusions.
- **Escape classes** (the item's own terms, above):
  call-spelled writes; a cross-module helper write; a
  dict-merge rebind under a variable key; indirection
  hiding the executing callable — helper pass-throughs,
  alias chains, inheritance, `__code__` replacement.
- **Closure routes (either suffices):** (1) a completeness
  argument over the language's mutation and indirection
  forms that survives adversarial review with no executed
  escape — proof completed; or (2) a restructure of the
  runner so the property holds by construction — the
  choke-point form, every note write through one point
  whose inputs are the vocabulary.
- **Scope:** this item is about the suite's own
  bookkeeping. It does not change the guard, the method
  of measurement, or any measured result.
- **The GitHub issue stays parked** on Peter's word
  (issue text at `hidden_files/issue-resource-note-tie.md`;
  filing not attempted at this step, not re-raised).

### Named non-executions at this step (tier-labelled)

- **Windows run-tier, owed:** in-tree `cargo test`
  execution; `build.ps1` and `run.ps1` execution; the
  battery's behavioural verdicts; the full 1,940-slot
  campaign. Revival for all: the release test run on a Windows
  machine with the MSVC toolchain.
- **Measurement wiki tokens:** `Measurement.md`'s
  `final-measurement-results` and `full-measurement-tables`
  tokens are **not** filled at E10 — this step produces no
  measurement result (a suite test run is not a campaign
  measurement, and no number from it is dressed as one).
  They remain for **E12**, to be filled from the final
  measurement record or carried as named non-executions
  with the Windows-tier revival above.
- **L-T1** (above, R4): the pure-set claim's full
  re-derivation — named loss, revival a §4.3 revival run.

[↑ Back to Contents](#contents)
