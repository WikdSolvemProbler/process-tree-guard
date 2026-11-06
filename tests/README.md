<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# Suite — tests and benchmark for process-tree-guard

This package is the test and benchmark suite for process-tree-guard.
The suite holds a campaign harness.
The harness executes the benchmark campaign of the reference plan (as configured: 1,940 slots — 36 control + 224 warmup + 1,680 measured).
The measurement argument sets the size of the campaign.
A quota does not set the size.
The reference decomposition of the original plan shape is 1,620 slots.
The malformed-input and scaling cells of the measurement argument add the rest.
The configured total is 1,940.
The suite also holds a Windows battery.
The battery covers the five theory items that the earlier phases deferred.
Test families in the suite map to the failure tree of the guard (`FAILURE-TREE.md`).
Python is permitted in this package only.
The guard itself is Rust.
This package never changes the guard.

## Contents

- [Glossary](#glossary)
- [The one build command (Windows laptop)](#the-one-build-command-windows-laptop)
- [The one run command (Windows laptop)](#the-one-run-command-windows-laptop)
- [What runs where — evidence tiers](#what-runs-where--evidence-tiers)
- [How results are reported](#how-results-are-reported)
- [One explicit scope decision](#one-explicit-scope-decision)
- [The suite's counted-once total](#the-suites-counted-once-total)
- [Layout](#layout)

[↑ Back to Contents](#contents)

## Glossary

These terms are the package's own terms.
Each term stands at the operational meaning the suite uses.
The glossary carries 19 bullets / 20 terms.
ANCHOR and ANCHORED share one bullet and count as two terms.
A glossary census test holds that count.
The test recomputes the count from this section.

- **HOLD** — a hold is a check that names a violation instead of passing it silently. Where this package says a figure or a field is held (the census holds a total. A record field is held to a form), a check compares it against its source and fails naming the disagreement. Rather than letting a wrong value pass unremarked.
- **MINT** — to mint is to create a competitor loss (or a refusal) from a record. A loss exists only where a record's own fields earn it under the report's rules. And a record that cannot earn it mints nothing.
- **NEIGHBOUR** — a neighbour is the field, label, container, element, or artifact adjacent to one already held. The next place the same class of disagreement could stand, once the held one is correct.
- **STAMP** — a stamp is. one of the values the pricing construction stamps on every pricing record, in the breadth `pricing_stamp_defects` actually cross-checks. The probe-shape metadata (the console pair's contexts — baseline DETACHED, loaded NO_WINDOW. the owner pair's contexts — NO_WINDOW for baseline and loaded alike — and its inserted_levels, baseline 0, loaded 1. The resolver pair's contexts — NO_WINDOW for baseline and loaded alike — and its probe_count, baseline 0. Loaded the empty-folder count, together with the pinned resolver fields both resolver shapes are constructed under: pathext. current_folder_rule, the shared working_directory, and intended_path on the loaded shape. Laid out as the construction lays it out); None in every field a probe does not construct for its shape . So a non-null value in such a field is a stamp its construction never writes. the item stamp (the item a record carries is the item its probe prices); an "ok" record's exit_code. Held to the fast path's expected code; the loaded-only fields guard_pid and outer_elapsed_ms. Which the owner-verification loaded shape alone writes — None on every other probe and shape. And on that one shape held to the construction (guard_pid in PID form, outer_elapsed_ms the finite. Non-negative bracket covering the record's own elapsed_ms) on an "ok" record. And on a record of another outcome held wherever it carries a value (a launcher that never ran writes neither. field, and a record of another outcome carrying neither is not faulted for that absence). the guard fingerprint's companion values guard_fingerprint.path and guard_fingerprint.size. held as a pair — a fingerprint carrying either companion is held on both (the path an absolute path string. The size a non-negative integer); and the exact constructed field set itself. So a field beyond that set is a field no run of its probe and shape writes.. Integer stamps are held to the integer type as well as the value. A value of another type that merely compares equal to the stamp (1.0 for 1. Above all) is not the stamp the construction writes. `pricing_stamp_defects`. cross-checks exactly those stamps — no other record field is a stamp . And an item whose records fail any of them does not price.
- **ANCHOR / ANCHORED** — an anchor is the recorded reference a verification is checked against instead of the package's own claim. The SHA-256 pin in `campaign.json` for a competitor binary. The configuration content hash a run's manifest records for its configuration. A package is anchored when a live configuration file. Is present and hashes to the SHA-256 its manifest records. So the checker verifies the package's beside-fields against that anchored configuration; a package without a verifiable anchor is unanchored. And every verification the absent anchor degrades is named unverifiable by the checker — never silently passed. And never fallen back to the package's own recorded claim.
- **PERFECT FIXTURE** — a perfect fixture is one of the suite's two fabricated complete-campaign record sets. Each a known-good standard the tests judge intact and mutated record sets against. `complete_records()` in `tests/test_campaign_logic.py` (one correct record per plan slot, built from the plan itself. In which the guard wins or ties every median, both floor arms agree in both contexts. Every exit code matches its expected table, every fast slot's output matches the expected bytes. And no kill slot leaves a survivor) and. `complete_fixture()` in `tests/test_output_observation.py` (the same complete-campaign shape built on that family's own fixture records. Resting dimension (1) on the output observation its fast records carry).
- **TOTALITY** — totality is the rule that the holds and recomputation consumers never raise on the values they judge: a huge. non-finite, wrong-typed or unhashable value in a record is named in the existing defect vocabulary (an inconsistency. A stamp defect, a standing loss), never an exception in the checker — extreme values are named, not exceptions.
- **NEXT-RING** — a next-ring copy is. a document copy of a held quantity one ring further out than the set previously held. A restatement of a figure outside the section or document the census already holds to its source. The census extends. To each next-ring copy in turn, so a figure cannot drift in a copy nobody reads.
- **CENSUS** — the census is the recomputation discipline that holds this package's documents and evidence to their sources: stated totals, splits. Coordinates and figures are recomputed from the test sources, the plan. The vendored enumerations and the preserved packages by the census tests. And a statement that disagrees with its recomputation fails naming the disagreement (see HOLD).
- **MATERIALIZATION** — a materialization is the one read that turns a stated value into the concrete list of tokens it yields. Held under the value's identity so every later read of that same value, by any consumer, in any order. Serves that same list. A value needs no held materialization only when its re-readability is PROVEN (see PROVEN). A provably re-iterable value is read live on each use. A value whose re-readability cannot be shown is. Held as one-shot, however it would in fact behave . A custom iterable that does in fact hand back a fresh traversal over its live backing on every call. But is not provably re-iterable, stands on the held side of the line: it is materialized once. And later reads serve the held list. Not the backing's later state. Materialization is what makes a value that can be consumed only once readable by every. Consumer at all.
- **PROVEN** — a classification is proven when the evidence in hand. establishes it by construction, not by the absence of a counterexample. A one-shot classification is proven only where a second traversal cannot yield the full sequence again by construction . The capture's traversal is a built-in one-way iterator, which cannot restart. Or the value hands that same iterator back on the one further call the named undecidable class is permitted. Every. Other classification is unproven, however plausible, and no consumer may key a write-back on an unproven one.
- **STATED** — a value is stated when it is the value a mapping presents at the access a read makes. The face the owner has put in place at that moment, as opposed to a value a reader reconstructs, caches. Or infers. What a read captures is the stated value at that read. When the owner puts a different value in place, the new value is stated, and it . Not the value it replaced — is what a following read judges.
- **CANDIDATE** — a candidate is. a capture held pending beside a slot's retained entry when a read's evidence decides neither storing nor minting for it. It is not the entry, no settlement stands on it. And while it pends the read that took it serves only the interim the mechanism states . The candidate's own tokens when they are non-empty. The entry's tokens when they are empty. A pending candidate's fates are three. First, SETTLEMENT (see SETTLEMENT). On either of the two paths that settle: on the general path. A following read's capture settles the candidate (that capture may instead replace it with a new candidate. The verdict deferring again on the same deferred terms); on the one captureless path. an EMPTY candidate is settled STORING by its very object's persistence across following reads . Reads that return the candidate's object and take no capture at all . But only over an entry no minting settlement stands on. Over a candidacy admitted over a standing minting settlement (the marked candidacy). an empty candidate's object returns are NEUTRAL and settle nothing, exactly as a non-empty candidate's are (see SETTLEMENT). Second. RETIREMENT UNSETTLED: a following read's storing-face capture. stands as the entry at once and retires the pending candidate unsettled with it . The candidate neither becomes the entry (the storing-face capture does), nor does the entry stand. Nor is a new candidate admitted, and its deferred verdict never arrives. Third, EXPIRY . a candidate has no expiry of its own: it does not lapse with time or with any count of reads. And evidence that decides neither side (a non-empty candidate's object returns, a marked candidacy's empty object returns. an empty candidate's first object return over an unmarked candidacy — the return that arms the persistence evidence. It decides neither side and leaves the candidate pending. A further such return settling STORING on the persistence path as the first fate states. A following read whose capture raises, on which no verdict is taken. a following read that returns the entry's very object, where that object is not provably re-iterable — served by identity. Taking no capture and rendering no verdict. (a provably re-iterable entry's very object is not so served: its return takes the capture path. And the candidate is judged there . where the capture the return takes states the entry's own tokens, settling MINTING with the entry standing. As executed under an unmarked candidacy and a marked one alike. where the capture the return takes states different tokens — the returned object no longer equalling the entry as it stood. As where the owner has mutated the standing object in place . The capture is judged on its face on the general path. So a storing-face capture there stands as the entry and retires the candidate unsettled, the entry not standing). a following read whose stated value is absent — the mapping presents no value at that read. And the read returns the mechanism's missing sentinel before any candidate handling. Taking no capture and rendering no verdict) leaves it pending, until a following read settles it, replaces it. Or retires it as above.
- **SETTLEMENT** — a settlement is. the decision that a pending candidate is storing (the candidate becomes the entry) or minting (the entry stands). Taken with no further access beyond the reads that stand. It is taken on one of two paths. The general path, from captures already made and the following read's own capture; and the empty candidate's persistence path. Where the evidence is the candidate object's persistence across following reads that return the object and take no capture . That path settles STORING only. And only over an entry no minting settlement stands on (a candidacy that displaced a standing minting settlement finds an. Empty candidate's object returns neutral, exactly as a non-empty candidate's are). A settlement binds the entry it judged. It governs that entry until a new candidate is admitted on the same deferred terms. Which re-opens the question in either direction. And a storing-face change of entry retires the settlement with the entry it judged. The same storing-face capture. When a candidate stands pending in the settlement's place. stands as the entry at once and retires the pending candidate unsettled . The candidate neither becomes the entry nor is succeeded by a new candidate. And its deferred verdict is mooted (see CANDIDATE). A settlement is taken by reads alone, never by a write.
- **STORING** — a mapping is storing when its accesses return the stored object itself. Or a copy that yields the stored tokens in full (a copy of a copy stands in the same shape. Its capture likewise yields the stored tokens in full). Or a fresh instance, on each access, of a wrapper type that reads the stored value . Delegating to the stored object itself, to a copy of it, or through a further wrapper . So the wrapper's capture is the stored value's own reading, its tokens and its traversal. The value awaiting capture is the stored value. So the mapping's stated value stands behind every access alike. A storing mapping's changed stated value is the owner's replacement. And it is re-read exactly as a replacement stated in a plain dictionary is. Where a storing candidate's settlement is in question, it is taken on SETTLEMENT's two paths. The empty candidate's captureless persistence path included. At the breadth SETTLEMENT states for that path (it settles only over an entry no minting settlement stands on).
- **MINTING** — a mapping is minting when each access PRODUCES the value it returns over one shared source. Rather than retrieving a stored entry: the ordinary factory's accesses create a distinct value on each access. And the memoizing factory — which produces its mint once and returns that same memo object across a read's accesses. Until it produces anew — is minting by the same production. The returned object's identity notwithstanding (the memo is the mint's own last product. Not an entry the owner stated and left standing). What no minting access does is return a stored entry. For there is no stored entry. Only the source each produced value is made from. Because a later mint over a draining source can yield only. What the source has left. The first materialization taken over that source is the truth every consumer serves for that slot.
- **DEFERRED** — a verdict is deferred when the evidence of the captures taken so far decides neither side and the mechanism. Takes no further access to force one: the read serves its stated interim. The candidate is held pending beside the retained entry, and the verdict waits for a following read . For its own capture on the general path, or, for an empty candidate. For the captureless evidence of the candidate object's persistence across following reads, at the breadth SETTLEMENT states for that path. It settles only over an entry no minting settlement stands on. a candidacy that displaced a standing minting settlement finding its empty candidate's object returns neutral (see SETTLEMENT). A deferred verdict. Is not a guess at either side; a classification that cannot be decided and cannot be deferred is undecidable. And a raise at a read's own capture then propagates as one honest read propagates it.
- **CAPTURE** — a capture is. the single traversal a read performs over a value. Together with the tokens it yields and the traversal object that produced them. Every classification is derived from captures . Their tokens, their traversal's identity against other captures' traversals, and whether that traversal can restart — with the one named exception. The class a single capture identifies as undecidable (a value that is not an iterator in fact whose capture traversal. Is a built-in one-way iterator) pays exactly one further `iter()` call after its tokens are captured. Because its proof is a property of a second call by definition.
- **TRAVERSAL** — a traversal is one passage over a value by an iterator: it has an identity (the iterator object itself). A direction (it only ever advances — what it has yielded it does not yield again). And a reach (the tokens it yields before it is spent). Two captures that read the same traversal are two. Views of one source, however different the values that handed it back. A factory whose every `iter()` call recreates its traversal over a shared source makes a fresh traversal on every call. And no traversal object is ever handed back twice.

[↑ Back to Contents](#contents)

## The one build command (Windows laptop)

**Prerequisites** — these items must stand in place before either command runs.
`build.ps1` and `run.ps1` check the items that they use before any stage runs.
A missing item fails the script at once.
The failure names the missing item and its remedy.

- Python 3 on PATH as `python`.
  `run.ps1` and the suite call bare `python`.
  Install Python 3 and put `python` on PATH.
- A Rust toolchain with the `x86_64-pc-windows-msvc` target.
  Install the toolchain with rustup.
  Then run `rustup target add x86_64-pc-windows-msvc`.
- Visual Studio Build Tools with the C++ workload.
  `build.ps1` finds the MSVC toolset through vswhere.
- The required layout.
  The suite folder stands inside the guard tree.
  The guard tree is the folder that holds this suite.
  The default `-Tree` value is `..`, that folder.
  A user can name `-Tree` on both command lines instead.
  Build and run must name the same tree.
  `build.ps1` records its tree in `build\build-record.json`.
  `run.ps1` checks that record before any stage runs.
  A different resolved tree fails the run at once.
  The failure names both trees.
  With no build record, `run.ps1` proceeds with its own `-Tree`.
  The run records the audited tree in `results\stage-exit-codes.txt`.
  The record also states that no build record stood to check against.
  A copy of the suite folder alone has no guard sources to build.
- The preserved competitor bundle, for the competitor arms.
  The bundle stands in this package at `benchmark\competitors-001\`.
  Its `bins\` folder holds the three as-found binaries flat.
  The binaries are `processkit-cli.exe`, `timeout.exe`, and `sysprims.exe`.
  The bundle's `run-manifest.json` stands beside the binaries.
  `campaign.json` pins each binary by its SHA-256 checksum.
  The build never fetches a competitor binary.
  The build never patches a competitor binary.
  `build.ps1` also checks two locations beside this folder.
  One is `..\tree\benchmark\competitors-001\bins` in a guard-tree checkout.
  One is `..\benchmark\competitors-001\bins` below a repo root.
  The default build finds the vendored bundle with no named folder.
  `-Competitors` names a different bundle folder.
  That override always wins.
  With no bundle and no override, the build stages the guard alone.
  A campaign run then refuses at pre-flight admission.
  The refusal names the missing competitor binaries.
  The run records no spawn failures for binaries that never staged.

```powershell
.\build.ps1
```

With no arguments, the build stages the competitors from the preserved bundle.
In this package, that bundle is the vendored bundle in this folder.
A different bundle stands under `-Competitors`:
`.\build.ps1 -Competitors <folder holding processkit-cli.exe, timeout.exe, sysprims.exe>`.

The build finds MSVC first.
The build then builds the four role programs of the guard.
The build also builds the test helper of the guard.
The target is `x86_64-pc-windows-msvc` in release.
The build stages the programs into `build\`.
The cargo build runs under a finite deadline of 1,800 s.
A build that passes the deadline loses its whole process tree.
The script then fails and names the deadline.
Competitor binaries are package inputs, supplied as found.
Before staging, the build verifies each competitor binary.
The verification reads the identity that `campaign.json` pins.
The identity is the file name, the probed version, and the SHA-256 pin.
The probed versions are ProcessKit CLI v0.3.4, nuewframe/timeout v1.0.0, and sysprims v0.2.4.
The SHA-256 pin is the hash in the preserved bundle's own run-manifest.
That manifest records the binary that actually ran.
The pin in `campaign.json` is the anchor.
A hash mismatch fails the build at staging.
The failure names the arm, the expected pin, and the actual hash.
A version mismatch fails the build at staging.
The failure names the arm and both versions.
A same-named wrong binary is caught at staging.
No wrong binary is measured as that competitor.
The build records the staged hashes and probed versions.
The record is `build\competitor-identities.json`.
Later campaign runs check against that record as an added consistency record.
A binary that matches the record but not the pin fails identity.
The pin is the anchor, not the record.
Every build also writes the staged build record.
The record is `build\build-record.json`.
The record holds `tree_as_given` and `tree_resolved`.
The build writes the record with or without staged competitors.
`run.ps1` checks the record before the run.

[↑ Back to Contents](#contents)

## The one run command (Windows laptop)

```powershell
.\run.ps1
```

For a tree at a non-default path, name the tree that the build built:
`.\run.ps1 -Tree <folder the build built>`.
The run executes the full campaign first.
The campaign is the reference plan as configured: 1,940 slots — 36 control + 224 warmup + 1,680 measured.
The measurement argument sets the size, not a quota.
The campaign makes one attempt.
The campaign allows no retries, no exclusions, and no replacements.
The run then executes the battery.
The run then executes every test family.
The campaign writes into its own namespace, `results\<run-id>\`.
The run id stands in the namespace's `run.json`.
The namespace holds `report.txt`, `observations.jsonl`, and `pricing-observations.jsonl`.
The namespace also holds `claim-ledger.json` and `run.json`.
`results\tests.txt` sits beside the namespaces.
A re-run only adds a new namespace.
A re-run never overwrites a prior run's evidence.
A re-run never deletes a prior run's evidence.
The rule holds over a complete, a partial, or an ended prior campaign alike.
The namespace is reportable from the moment it exists.
`run.json` is written as a provisional manifest before the first slot executes.
The manifest records the seed, the full planned order, and the environment.
The manifest also records the configuration hash.
The manifest is updated as slots complete.
A campaign that ends before its last slot is reported from its namespace.
The report of that campaign is an incomplete campaign.
Its standing losses stand listed in that report.
Such a campaign is never reported as a result.
Pre-flight admission runs before the first slot of a certifying run.
Admission verifies that every product binary is present.
Admission verifies that every configured child program is present.
Admission verifies that the survivor probe's program is present.
Admission verifies that every competitor identity matches its pin in `campaign.json`.
The staging record is checked as well, as an added consistency record.
A binary that matches the record but not the pin fails identity.
A mismatch aborts the campaign before any slot executes.
The abort names the arm.
The abort carries a distinct non-zero exit.
Before the campaign stage, `run.ps1` compares its resolved `-Tree` with the build record.
A mismatch refuses the run before any stage executes its payload.
The refusal names both trees.
With no build record, the run proceeds.
The stage record then states the audited tree.
The stage record also states that no build record stood to check against.
The battery step runs each case's driver.
The driver is `battery\drivers\case_<id>.py`.
The step calls the driver through `battery\battery.py`.
The call names `--tree <the run's resolved -Tree>` and `--results-dir results\battery`.
Every case ends in PASS or FAIL.
Each case's evidence records stand in `results\battery\`.
The evidence records hold commands, exit codes, and raw outputs.
Every case record in `results\battery\battery.json` names the tree it audited.
The competitor argument templates and native exit codes are in `campaign.json`.
The Windows staging checks use the pinned bundle binaries. The run records the templates and each executed argument vector.
ProcessKit 0.3.4 requires `run --timeout`, a separate JSONL event log per slot, and `--inherit-stdin` for the relay workload. Its timeout status is 106.
nuewframe 1.0.0 requires a unit-bearing duration and `--`. SysPrims 0.2.4 treats bare durations as seconds, so its template uses an explicit `ms` suffix. Its default diagnostics remain in the measured output.

A full Windows run starts each arm in a private Job. The harness records the arm's exit and checks for survivors before draining that Job outside the timing interval. Each row records the cleanup result. A Job that cannot be proven empty stops the run before the next slot. Required ProcessKit event logs are retained in the run namespace.
A binary that does not match its pin is not the as-found competitor.
Staging refuses such a binary.
No operator-confirmation path certifies a run with a substitute binary.

The stages are kept apart by kind.
A campaign failure is an infrastructure abort.
An abort ends the run at once.
An abort claims no results complete.
A campaign **INCOMPLETE outcome** is a failed stage, not an abort.
An incomplete outcome means the freeze gate is unmet.
Standing losses stand in an incomplete outcome.
The harness exit code does not carry the campaign's verdict.
The verdict is carried by status.
The harness exits 0 for a completed execution with an incomplete outcome.
`run.json` records `campaign_status`.
`run.json` also records the standing-loss count and the freeze-gate FAIL count.
`report.txt` carries the same verdict in its STATUS line.
`run.ps1` reads `campaign_status` from the run's `run.json`.
`run.ps1` records the status in `results\stage-exit-codes.txt` as `campaign status: ...`.
`run.ps1` names the campaign in its final aggregate.
The aggregate names the campaign exactly as it names a battery FAIL.
The one run command cannot exit 0 over a guard loss.
The battery and the test families still run after an incomplete campaign.
A battery **FAIL is a decision**.
A FAIL means a case was refuted.
A refuted case's item is not adopted.
The FAIL stands recorded in `results\battery\`.
The run continues to the test families after a battery FAIL.
`run.ps1` names the battery stage in its aggregate failure.
The aggregate failure is reported after every stage has run.
A battery **execution failure** aborts loudly before the families.
An execution failure is a case BLOCKED-ERROR, a crash, or no `battery.json` written.
The battery then produced no decisions to continue on.
The distinction stands on the exit-code contract of `battery.py`.
Exit 0 means executed, with no FAIL and no BLOCKED-ERROR.
Exit 2 means BLOCKED-ERROR, an execution failure.
Exit 3 means a FAIL decision, with the report written.
Any other exit is not a verdict.
Every stage's exit code stands recorded in `results\stage-exit-codes.txt`.

[↑ Back to Contents](#contents)

## What runs where — evidence tiers

- **Linux-proven**: the harness logic is exercised here.
  The logic is plan accounting, templates, and slot execution and classification.
  The logic also holds the floor rule and the report rules.
  Stub arms and fixture data exercise that logic here.
  The battery's function-scoped source-audit parts run here.
  The battery drivers' decision logic is exercised here in the drivers' stub tier.
  The stub tier holds the negative controls.
  In the controls, broken variants fail their charged conditions.
  A missing driver blocks its case.
  Withheld observations block their cases.
  The round-3 variants each refute their condition.
  Those variants are zero packets delivered and a 4-operation teardown.
  Those variants are also a packet claim that turns the verdict.
  Those variants are also a remainder mismatch and pending output never produced.
  Those variants are also a stale set entry and unchecked consumers.
  The round-4 variants each stand where they are charged.
  Latency noise below one tick must NOT refute its condition.
  A systematic 2× latency must refute its condition.
  The round-4 variants also hold label-assigned gate bytes with no measured pattern.
  The round-4 variants also hold a role pairing with consulted verdict values that differ.
  The round-4 variants also hold an enumeration divergence in an entry that no fixture names.
  The round-4 variants also hold a set operation that skips under the wrong case matcher.
  The round-4 variants also hold a teardown call performed but unconfirmed.
  The round-4 variants also hold a fallback decided from two readings of one record.
  The round-4 variants also hold a port reported as inherited by an inheritance-enabled trial.
  Withheld spawn, route, and enumeration records block their round-4 cases.
  Every battery decider computes its fields from the raw observations.
  No decider reads a constant that a probe states about itself.
  The Python test families that do not drive the guard run here.
  Among them is the scanner differential's reference side.
  That side runs over its full seeded corpus, with its negative controls.
  `VERIFICATION.md` records the evidence.
- **Windows-theory**: every item that needs the built Windows product stands at this tier.
  The items are the full campaign's numbers.
  The items are also the guard-driving test families.
  The items are also the battery drivers' Windows execution.
  The items are also the two `.ps1` scripts' execution.
  These items are complete and correct by construction.
  A Windows run proves them.
  No number in this package is a Windows measurement.

[↑ Back to Contents](#contents)

## How results are reported

- The headline is the guard's **distance above the machine/harness floor, per console context**.
  The two distances are context-labelled, NO_WINDOW and DETACHED.
  The two distances are never pooled.
  Each context's floor is the agreement of the two minimal floor arms in that context.
  Both arms are real process spawns.
  Each arm spawns a distinct minimal exit-at-once program.
  Each arm has the same outer-lifetime shape as the other.
  One arm spawns the fast child directly.
  One arm spawns the platform's exit-at-once form directly.
  The floor stands at about 51 ms on the preserved bundle's machine.
  Agreement means the two medians stand within 5% of each other.
  That bound is re-derived from the bundle's own 50.9 ms / 52.5 ms pair.
  The pair shows a 1.6 ms, 3.0% spread.
  The 5% bound holds that spread with headroom.
  The bound stands in place of the earlier 10%.
  Each distance is decomposed against the counted-ops census.
  The census states plain path 45 named calls, hook path 18.
  That census is the vendored enumeration of MEASUREMENT §2's named calls.
  The enumeration stands in `campaign/census.json`.
  The counted-ops census tests in `tests/test_campaign_logic.py` enforce the census.
  Those tests are the `CensusConsistencyTests`.
  They recount the totals from that enumeration.
  With the guard tree at the suite's expected location, they re-derive the enumeration from the tree.
  A raw median race against one competitor is never the headline.
- The **anchor arm is the current gauntlet binary**.
  Every report records its path, SHA-256, and size.
  The stale pairing of 66.5 against 50.9 may appear only with its provenance.
  The 66.5 anchor predates the gauntlet.
- A decomposition prices per item only the expensive class.
  The class is console probe + console finish, owner verification, and resolver probe count.
  A price comes only from the campaign's own pricing probes.
  The probes are campaign steps in `campaign/pricing.py`.
  Each probe is a paired guard fast-path shape.
  The pair differs only in the named item.
  For the console item, the pair is DETACHED against NO_WINDOW.
  For owner verification, the pair is one intermediate launcher level against direct launch.
  The recording launcher is that one level.
  The harness spawns the launcher directly, as the parent of the guard.
  The launcher brackets the lifetime of the guard child.
  No second launcher wraps the launcher.
  The pair differs by exactly one inserted ancestor level.
  For the resolver probe count, the pair is a bare child name against the absolute path.
  The bare name stands behind empty PATH folders.
  Both resolver shapes spawn in a fresh, empty scratch working directory.
  Neither shape spawns in the folder of the harness.
  The child environment's PATHEXT is pinned for both shapes.
  The current-folder probe is disabled for both shapes.
  Every resolver record states its working directory.
  Every resolver record also states the PATHEXT and the current-folder rule in force.
  The probes run interleaved after the slots.
  Every raw run is written to the run's `pricing-observations.jsonl`.
  A price reaches the report only as a reference to those records.
  The reference names the probe name, the paired-observation count, and the guard fingerprint.
  The records must also carry the probe-shape stamps that the price claims.
  The console pair's stamps are its contexts, baseline DETACHED and loaded NO_WINDOW.
  The owner pair's stamps are its inserted-level counts, baseline 0 and loaded 1.
  The resolver pair's stamps are its probe counts, baseline 0 and loaded 4.
  The resolver stamps also hold the pinned resolver fields.
  Those fields are the working directory, PATHEXT, and the current-folder rule.
  The loaded shape's intended PATH is also a pinned resolver field.
  `pricing.pricing_stamp_defects` cross-checks exactly those stamps.
  An item whose records fail any stamp is "not measured".
  `harness.check_run_package` names a stamp defect by item and field.
  Pricing typed into `campaign.json` by hand is "operator-supplied, not measured".
  An item with incomplete probe pairs is "not measured".
  An incomplete item is never a number.
  Every price carries its uncertainty.
  The item's trace reports the paired differences' dispersion beside the median.
  The dispersion is the minimum, the maximum, the quartiles, and the interquartile range.
  The trace also reports the sign counts.
  A negative price is flagged **INCONSISTENCY**.
  Paired differences that disagree in sign are flagged **INCONSISTENCY**.
  Priced items that sum past the distance are flagged **INCONSISTENCY**.
  The flag stands on the item's line and as a standing loss.
  No unexplained remainder is printed for that context.
  A negative remainder is never printed as a measurement.
- **Execution order is seeded.**
  The warmup and measured blocks are shuffled under the recorded seed of the campaign.
  The seed is the configuration's `seed`, or `--seed`.
  `run.json` preserves the seed and the full execution order.
  `run.json` also preserves the machine, system, build, and CPU environment.
  `run.json` also preserves the configuration file's SHA-256.
  `harness.check_run_package` re-derives the order from the seed.
  The checker flags any mismatch against `run.json` or `observations.jsonl`.
  The configured per-cell totals of the run do not change.
- **Every arm's identity is listed.**
  The report names each arm's fingerprint and probed version.
  The fingerprint is the path, the SHA-256, and the size.
  The report names the guard's identity and each competitor's identity alike.
  In a certifying run, a competitor can disagree with its pinned identity.
  The disagreement is a version disagreement or a hash disagreement.
  The pin stands in `campaign.json`.
  The staging record is also checked, as an added consistency record.
  A binary that matches the record but not the pin fails identity.
  Such a disagreement is a standing loss.
  The campaign then measured the wrong binaries.
  Such a campaign is not a result.
- Kill claims about nuewframe/timeout are stated exactly.
  On Windows, that program does not kill descendants at all.
  It calls `TerminateProcess` on the direct child only.
  Its first kill action is a 5 s wait.
  The wait is the `kill_after` default.
  A graceful step stands before the wait.
  That step is unimplemented on Windows.
  In the bundle's kill trials, the program killed nothing.
  The child outlived the 2 s deadline.
  The child then died naturally inside the grace window.
- The **kill child is a tree**.
  The child spawns a grandchild.
  The grandchild records its PID at the slot's own marker file.
  The grandchild then sleeps.
  Every kill slot's record carries the identity of the tree.
  The identity is the marker and the arm PID.
  The identity also holds the child PID where the workload records it.
  The identity also holds the grandchild PID.
  The survivor probe is `campaign/probe.py`.
  `campaign.json` wires the probe on by default.
  The probe is called with the recorded tree PIDs for each slot.
  The PIDs are the child PID where recorded, and the grandchild PID.
  The call stands before the slot's elapsed time closes.
  A kill slot can have no probe, no recorded identity, or no answer from the probe.
  Each such slot is an infrastructure failure.
  No such slot is "ok" with survival unchecked.
- Every slot's exit code is checked against a per-arm expected-exit table.
  On the fast path, the expected code is the child's code, propagated.
  On the kill path, the expected code is the arm's documented deadline code.
  A mismatch is classified `exit-mismatch`.
  A mismatch is listed.
  A mismatch is excluded from every median.
  A competitor's measured exit mismatch is competitor-loss evidence.
  The evidence stands on the exit-code component of profile dimension (1).
  The evidence class is the class of its measured output mismatches.
  The class is also the class of its descendant survivals.
  The guard's own exit mismatches are campaign standing losses.
  On the kill path, a returned arm can leave a survivor and mismatch its exit code.
  Such an arm is classified `descendant-survived`.
  The survivor class stands before the exit class.
  The exit fault is listed separately under its own name.
  The exit fault is never a record inconsistency.
- **Output is observed on the fast path.**
  That observation is the output component of profile dimension (1).
  The fast child is an output workload, the testkit `relay`.
  The runner feeds the child the configured stdin text of the child spec.
  The text stands on a regular file.
  The child copies that text plus one newline to stdout.
  The child writes its fixed `error-stream` line to stderr.
  Both streams carry known bytes.
  The bytes are configuration data in `campaign.json`.
  The data is `expected_stdout` and `expected_stderr`.
  The data is identical for all four arms.
  Every fast-path slot runs with both streams redirected to per-slot regular files.
  The slots are the product arms and both floor arms, of every kind.
  No fast-path slot uses the null device for a stream.
  No fast-path slot uses a pipe for a stream.
  After the arm exits, the record carries the emitted bytes of each stream.
  The record carries the SHA-256 and the length of those bytes.
  The record also carries the match verdict where the child spec names expected bytes.
  The floor-spawn arm's own minimal program names no expected bytes.
  Its streams are digested and measured, with no verdict recorded.
  A fast slot can match on exit code and mismatch on output.
  Such a slot is classified `output-mismatch`.
  Such a slot is listed and excluded from every median.
  A guard output mismatch fails the gate dimension **output correctness**.
  A guard output mismatch stands as a campaign loss.
  Its class is the class of an exit mismatch.
  A competitor's measured output mismatches are listed as competitor losses.
  Those losses are evidence of the guard's win on dimension (1).
  Those losses are not campaign standing losses.
  A guard measured fast record can carry no output observation.
  Output correctness then stands UNEVALUATED.
  Dimension (1) reads observed-passing only under three passes.
  The three passes are kill-path correctness, exit-code correctness, and output correctness.
  Kill-path slots carry no output observation.
  Their output fields are null.
  Their streams stay on the null device.
  The kill child's product is the tree that the child leaves behind.
  The survivor probe judges that tree, not a byte stream.
  No arm is compared on bytes that the arm put out while being killed.
- **Completeness, in the campaign sense, is exact slot accounting.**
  The accounting runs against the configured plan of the run.
  The plan is fixed for that run once the run starts.
  Every slot stands present exactly once.
  Presence is by kind, arm, path, context, and rep.
  The freeze gate is stated per dimension per context.
  Its dimensions are fast-path latency and kill-path latency.
  Its dimensions are also kill-path correctness, exit-code correctness, and output correctness.
  The gate asks a win or a tie on each dimension.
  The full evaluation profile of SPEC_Benchmark §3 stands in every report.
  The profile is an enumeration of all eight dimensions.
  Each dimension carries a state.
  The profile asks a win or a tie on every applicable dimension.
  The eight dimensions are these.
  (1) Correctness and output quality.
  (2) Latency, throughput, and startup time.
  (3) CPU, memory, storage, network, energy, and monetary cost.
  (4) Scaling, concurrency, and resource exhaustion.
  (5) Reliability, prolonged operation, recovery, and data integrity.
  (6) Security boundaries and resistance to malformed inputs.
  (7) Compatibility, portability, installation, and integration.
  (8) Usability and accessibility for interactive products.
  The five gate dimensions map into dimensions (1) and (2).
  Kill-path correctness, exit-code correctness, and output correctness map into (1).
  Fast-path latency and kill-path latency map into (2).
  The gate dimensions carry their PASS or FAIL in those dimensions.
  Every other dimension defaults to UNEVALUATED.
  A dimension renders NOT-APPLICABLE only through a recorded decision.
  The decision stands as package data in `campaign.json`, in `profile_dimension_decisions`.
  Each run's `run.json` records the SHA-256 of that configuration.
  The decisions mapping stands recorded verbatim in `run.json` beside that hash.
  The reason of a decision prints beside its dimension.
  The shipped configuration carries one entry for each of dimensions (3) to (8).
  Two entries are scope decisions, for dimensions (5) and (7).
  Each scope decision states what this comparison does not claim.
  The comparison is the guard against the three named competitors.
  The comparison runs on the campaign as configured for the run.
  Four entries are collection declarations, `COLLECTED: <instrument>`.
  The declarations cover dimensions (3), (4), (6), and (8).
  Each declaration names the instrument that collects its dimension.
  For (3), the instrument is the runner's resource instrumentation for each call.
  For (4), the instrument is the plan's scaling slots.
  For (6), the instrument is the plan's malformed-input slots.
  For (8), the instrument is the formalized manual usability check in `usability-evidence.json`.
  The state of a collected dimension is computed from its instrument's evidence.
  The evidence is the records of the run for (3), (4), and (6).
  The evidence is the records of the manual artefact for (8).
  The rule is three-valued.
  A qualifying entry with outcome "fail" renders FAIL, observed-failing.
  A fail carries a standing loss.
  A fail dominates any pass.
  Qualifying entries with "pass" and no fail render "collected".
  A dimension is UNEVALUATED while no entry qualifies.
  A qualifying entry has a declared prompt class.
  A qualifying entry has the boolean exercised=true.
  A qualifying entry has a recorded observed outcome.
  That outcome stands in the declared vocabulary.
  That outcome is not "not-run".
  A dimension is UNEVALUATED while every outcome stands at "not-run".
  A collected dimension renders UNEVALUATED on a run without its collection records.
  `VERIFICATION.md` records the per-dimension argument.
  No source edit decides a dimension.
  `campaign/report.py` carries no decisions table.
  While any profile dimension is UNEVALUATED, the campaign is INCOMPLETE.
  An incomplete campaign is never a certified result.
  That rule holds when the observed dimensions pass.
  Every report also renders the **claim ledger** of SPEC_Benchmark §11.
  The ledger holds one claim per profile dimension, with its state.
  A state is observed-passing, observed-failing, UNEVALUATED, NOT-APPLICABLE, or collected.
  Each claim also carries its basis.
  The basis is the freeze gate over `observations.jsonl` for an observed dimension.
  The basis is the named collecting instrument for a collected dimension.
  The basis is the configuration decision and its reason for a decided dimension.
  The harness writes the same ledger into each run namespace as `claim-ledger.json`.
  The ledger is mirrored in `run.json`.
  The campaign outcome consumes the ledger.
  A complete outcome requires every dimension in a closing state.
  The closing states are observed-passing, collected, and decided.
  `harness.check_run_package` verifies the ledger against the records of the run.
  The checker also reads the recorded configuration hash.
  A ledger can claim a decision that the configuration does not carry.
  A ledger can also state a state that the records contradict.
  Each such ledger is a named package defect.
- **Completeness, in the phase sense,** is the completeness accounting.
  The word carries both senses in this package.
  The two senses are different senses.
  The accounting maps every failure-tree branch to a named test or a non-execution.
  The accounting maps every feature surface in the same way.
  The accounting maps every document claim in the same way.
  The accounting maps every standing battery condition in the same way.
  Each non-execution is tier-labelled and carries a revival path.
  That accounting stands in `COMPLETENESS.md`, by its four parts.
  The accounting was executed as the closing step of its phase.
  `VERIFICATION.md` records the accounting as executed in its closing list.
  The breadth is the breadth that the accounting states.
  The parts are complete.
  Part (3)'s two named findings stand for the review of that part.
- **Kill-path correctness is comparative.**
  The comparison evaluates every product arm.
  An arm can lose its kill dimension in a context.
  The loss stands when all its measured kill slots in that context are `descendant-survived`.
  A losing arm is excluded from the kill-latency comparison of that context.
  An arm that does not kill has no kill latency.
  Its missing median cannot veto the campaign.
  A rival can stand unevaluated in a context.
  That position holds with no kill records for the rival.
  That position also holds with only infra-failure or exit-mismatch kill records.
  An unevaluated rival is never a converted loss or a converted pass.
  Each competitor's descendant-survived kill slots are listed as competitor losses.
  Those losses are evidence of the guard's win on that dimension.
  Those losses are not campaign standing losses.
  The guard's own descendant-survived slot fails the dimension.
  The guard's own slot also fails the campaign.
  The same evidence rule holds on the output component of dimension (1).
  A competitor's measured fast slots classified `output-mismatch` are competitor losses.
  The guard's own output-mismatch slot fails output correctness.
  The guard's own slot also fails the campaign.
- **Standing-losses rule**: a campaign can end before every slot has run.
  A campaign can also fail the freeze gate in another way.
  Each such campaign is reported as an incomplete campaign.
  Its standing losses stand listed.
  Such a campaign is never reported as a result.

[↑ Back to Contents](#contents)

## One explicit scope decision

The cap-over-default approval flow has no test family in this suite.
The flow is a `--max-extension` over the 21600-second default cap.
Windows Hello or the account password approves that flow.
The absence of a family is a decision.
`FAILURE-TREE.md` §5 records the decision in full.
The non-interactive seams of the flow stand under automated test in the guard tree.
Those tests are the `src/auth/tests.rs` seam tests and the parity checks G93–G103.
The interactive remainder stands outside those tests.
The remainder is a real Windows Hello request.
The remainder is also a real password typed at a prompt.
Only the manual check in the tree's `CONTRIBUTING.md` exercises the remainder.
A person runs that check before a release that changes the approval code.
The person is present at that run.
This suite claims the remainder at no tier above manual/Windows-theory.

[↑ Back to Contents](#contents)

## The suite's counted-once total

647 tests counted once, each by `run_tests.py`.
The split is (593 execute on any platform; 54 drive the guard).
The 54 execute on Windows against the built package.
Elsewhere they skip.
The negative controls prove the skip reason is the missing binary.
The census-consistency test stands among them.
That test reads the counted-once totals in three documents.
The documents are `README.md`, `VERIFICATION.md`, and `FAILURE-TREE.md`.
The canonical phrasing is "N tests counted once".
The test fails on any disagreement with `run_tests.py --count-only`.
The census set also includes the recorded evidence summary `evidence/test-run.txt`.
The same discipline holds that summary's recorded total to the runner's count.
The counted-ops census tests also stand among them.
Those tests recount the plain, hook, and run-core totals.
The recount reads the vendored enumeration in `campaign/census.json`.
The tests re-derive that enumeration from the guard tree's MEASUREMENT §2.
That half runs with the tree at the suite's expected location.
That half skips, with its reason stated, when the tree is absent.
The tests check the census totals in the README and `FAILURE-TREE.md` against the recount.
The streaming-scanner differential also stands among them.
The differential holds a seeded corpus of 357 generated envelope payloads.
That size is the size of the corpus.
The size is reported separately from the test count.
Every case is compared, guard against an independent parsed-form reference.
The comparison count is in 1,453 comparisons per Windows run.
The guard's own in-tree Rust tests (437 counted once) belong to the tree.
This suite does not recount those tests.
`FAILURE-TREE.md` states which failure branches have families.
`FAILURE-TREE.md` also states which branches remain unimplemented.
The battery's five cases are decision procedures with adoption conditions.
The cases are counted as cases, not as tests.

[↑ Back to Contents](#contents)

## Layout

- `campaign/` — the plan, the arms, the runner, the report, the harness, pricing, the probe, the stubs, and `census.json`.
  The plan holds the slots, fixed per run once a run starts, in seeded order.
  The arms hold the as-found templates, the expected-exit table, version probing, and the competitor-identity check.
  The runner holds one slot as one outer lifetime.
  The report holds the rules of the reporting section.
  The harness is the driver.
  Pricing holds the `CO-7` pricing probes.
  The probe is the survivor probe.
  The stubs are the logic-exercise programs.
  `census.json` is the vendored counted-ops enumeration of MEASUREMENT §2's named calls.
- `battery/` — `battery.py`, `env_fixtures.py`, and `drivers/`.
  `battery.py` holds cases (a), (l), (m), (n), and (o), with function-scoped source audits.
  `env_fixtures.py` is the fixture loader and the deterministic 500-entry generator.
  `drivers/` holds one Windows driver per case, its Windows probes, and the stub-tier probe.
  The stub-tier probe exercises the decision logic.
- `tests/` — the test families and their shared driver.
  The folder also holds the coverage families.
  Those families cover the payload cap, the context and rewriting vendors, non-Unicode arguments, and resolver order.
  The folder also holds the scanner differential.
  The differential holds its independent parsed-form reference and its seeded corpus generator, `scanner_reference.py`.
- `fixtures/env-fixtures.json` — the environment differential set.
  The set includes the generated 500-entry case verbatim.
- `evidence/` — the Linux-tier exercise outputs, recorded at build time.
  The outputs are a reduced stub run and the battery's Linux verdicts.
- `envcase.py` — the shared case-insensitive environment-setting rule.
  Every site that overrides a named child-environment variable uses that rule.
  An override replaces any case variant of the name.
  An override never sits beside a case variant.
- `usability-evidence.json` — the formalized manual usability check for the interactive surface of the product.
  That check is the collecting instrument of dimension (8).
  The file holds one record per prompt class, with the recorded-outcome fields.
  The file is tier-labelled manual.
- `COMPLETENESS.md` — the completeness accounting in the phase sense.
  Every failure-tree branch, feature surface, document claim, and standing battery condition stands in the accounting.
  Each item maps to a named test or to a tier-labelled non-execution with a revival path.
- `build.ps1`, `run.ps1`, `campaign.json`, `run_tests.py`.

[↑ Back to Contents](#contents)
