# Decision Records
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page holds the decision records of process-tree-guard. Each entry states one decision in three parts: wrong, fix, and why. This page is the only form in which the argument history of the product survives.

## Contents

- [The form](#the-form)
- [The criterion](#the-criterion)
- [Index by area](#index-by-area)
  - [DR-SPEED-01: The hook reads the payload with a streaming scanner](#dr-speed-01-the-hook-reads-the-payload-with-a-streaming-scanner)
  - [DR-SPEED-02: A hook denial writes one line and spawns no thread](#dr-speed-02-a-hook-denial-writes-one-line-and-spawns-no-thread)
  - [DR-SPEED-03: Work is paid only on the path that uses it](#dr-speed-03-work-is-paid-only-on-the-path-that-uses-it)
  - [DR-SPEED-04: A role is an explicit kind, not a function pointer](#dr-speed-04-a-role-is-an-explicit-kind-not-a-function-pointer)
  - [DR-SPEED-05: The plain run path carries no approval machinery](#dr-speed-05-the-plain-run-path-carries-no-approval-machinery)
  - [DR-SPEED-06: Speed is a counted census, not a feeling](#dr-speed-06-speed-is-a-counted-census-not-a-feeling)
  - [DR-SPEED-07: Fetched security descriptors are freed, not leaked by design](#dr-speed-07-fetched-security-descriptors-are-freed-not-leaked-by-design)
  - [DR-SECURITY-01: Every worker thread is hardened before its payload runs](#dr-security-01-every-worker-thread-is-hardened-before-its-payload-runs)
  - [DR-SECURITY-02: An unclassifiable PowerShell payload is denied](#dr-security-02-an-unclassifiable-powershell-payload-is-denied)
  - [DR-SECURITY-03: The access-list hardening keeps its full price](#dr-security-03-the-access-list-hardening-keeps-its-full-price)
  - [DR-SECURITY-04: The audit file is written through a held handle](#dr-security-04-the-audit-file-is-written-through-a-held-handle)
  - [DR-SECURITY-05: The verdict cache is not widened](#dr-security-05-the-verdict-cache-is-not-widened)
  - [DR-SECURITY-06: A teardown verdict comes only from an authoritative query](#dr-security-06-a-teardown-verdict-comes-only-from-an-authoritative-query)
  - [DR-SECURITY-07: The same-user boundary is stated, not implied](#dr-security-07-the-same-user-boundary-is-stated-not-implied)
  - [DR-SECURITY-08: The vendor set is closed and a vendor outside the set is a usage error](#dr-security-08-the-vendor-set-is-closed-and-a-vendor-outside-the-set-is-a-usage-error)
  - [DR-DESIGN-01: Containment is born into the job, and the guard refuses other systems](#dr-design-01-containment-is-born-into-the-job-and-the-guard-refuses-other-systems)
  - [DR-DESIGN-02: Four role programs, thin entry points, one shared core](#dr-design-02-four-role-programs-thin-entry-points-one-shared-core)
  - [DR-DESIGN-03: Decomposition moves code mechanically and re-points tests](#dr-design-03-decomposition-moves-code-mechanically-and-re-points-tests)
  - [DR-DESIGN-04: The guard has zero dependencies](#dr-design-04-the-guard-has-zero-dependencies)
  - [DR-DESIGN-05: Argument errors exit 125, a kept deadline exits 124, a hook denial exits 2](#dr-design-05-argument-errors-exit-125-a-kept-deadline-exits-124-a-hook-denial-exits-2)
  - [DR-DESIGN-06: The policy file is product data, not documentation](#dr-design-06-the-policy-file-is-product-data-not-documentation)
  - [DR-DESIGN-07: Every file carries its copyright header](#dr-design-07-every-file-carries-its-copyright-header)
  - [DR-SEAM-01: All platform declarations live in exactly one module](#dr-seam-01-all-platform-declarations-live-in-exactly-one-module)
  - [DR-SEAM-02: Reach is judged by a pinned computation and a pinned pure set](#dr-seam-02-reach-is-judged-by-a-pinned-computation-and-a-pinned-pure-set)
  - [DR-SEAM-03: The one pinned exception stands inside the definition](#dr-seam-03-the-one-pinned-exception-stands-inside-the-definition)
  - [DR-SEAM-04: No file receives platform names by a glob](#dr-seam-04-no-file-receives-platform-names-by-a-glob)
  - [DR-SEAM-05: The name chase fails closed at fixed bounds](#dr-seam-05-the-name-chase-fails-closed-at-fixed-bounds)
  - [DR-SEAM-06: Seam terms are defined from first principles](#dr-seam-06-seam-terms-are-defined-from-first-principles)
  - [DR-SEAM-07: There is no architecture axis in the shape](#dr-seam-07-there-is-no-architecture-axis-in-the-shape)
  - [DR-SEAM-08: The shape binds the denotation, not the literal path](#dr-seam-08-the-shape-binds-the-denotation-not-the-literal-path)
  - [DR-SEAM-09: A constant name halts the chase as a constant](#dr-seam-09-a-constant-name-halts-the-chase-as-a-constant)
  - [DR-SUITE-01: Every test is counted once, and the evidence states the count](#dr-suite-01-every-test-is-counted-once-and-the-evidence-states-the-count)
  - [DR-SUITE-02: Speed is reported as distance above the harness floor](#dr-suite-02-speed-is-reported-as-distance-above-the-harness-floor)
  - [DR-SUITE-03: A sensitivity bound is estimated from the plain condition only](#dr-suite-03-a-sensitivity-bound-is-estimated-from-the-plain-condition-only)
  - [DR-SUITE-04: Claims about a competitor are stated exactly](#dr-suite-04-claims-about-a-competitor-are-stated-exactly)
  - [DR-SUITE-05: One shared formulation serves the runner and the checker](#dr-suite-05-one-shared-formulation-serves-the-runner-and-the-checker)
  - [DR-SUITE-06: An extreme value is named, never an exception](#dr-suite-06-an-extreme-value-is-named-never-an-exception)
  - [DR-SUITE-07: The plan anchors the census; unplanned records are not evidence](#dr-suite-07-the-plan-anchors-the-census-unplanned-records-are-not-evidence)
  - [DR-SUITE-08: A failed usability exercise does not mint a collected dimension](#dr-suite-08-a-failed-usability-exercise-does-not-mint-a-collected-dimension)
  - [DR-SUITE-09: One vocabulary property is carried open, and its converse is not claimed](#dr-suite-09-one-vocabulary-property-is-carried-open-and-its-converse-is-not-claimed)
  - [DR-SUITE-10: Two campaign dimensions are not applicable](#dr-suite-10-two-campaign-dimensions-are-not-applicable)
  - [DR-ORG-01: The platform axis is operating-system first, at most two levels deep](#dr-org-01-the-platform-axis-is-operating-system-first-at-most-two-levels-deep)
  - [DR-ORG-02: Silicon and architecture are not levels of the axis](#dr-org-02-silicon-and-architecture-are-not-levels-of-the-axis)
  - [DR-ORG-03: Linux and macOS are named places, not backends](#dr-org-03-linux-and-macos-are-named-places-not-backends)
  - [DR-ORG-04: The `sys` root module holds selection and refusal only](#dr-org-04-the-sys-root-module-holds-selection-and-refusal-only)
  - [DR-ORG-05: Integrations are a top-level axis, never under `sys/`](#dr-org-05-integrations-are-a-top-level-axis-never-under-sys)
  - [DR-ORG-06: Adapters are families, not one file per vendor string](#dr-org-06-adapters-are-families-not-one-file-per-vendor-string)
  - [DR-ORG-07: One vendor has a named place and no folder](#dr-org-07-one-vendor-has-a-named-place-and-no-folder)
  - [DR-ORG-08: Role dispatch and the help texts leave the large files](#dr-org-08-role-dispatch-and-the-help-texts-leave-the-large-files)
  - [DR-ORG-09: One selector chooses among the usage texts](#dr-org-09-one-selector-chooses-among-the-usage-texts)
  - [DR-ORG-10: The pure set is computed, never curated](#dr-org-10-the-pure-set-is-computed-never-curated)
  - [DR-ORG-11: The test helper is not a shipped program](#dr-org-11-the-test-helper-is-not-a-shipped-program)
  - [DR-ORG-12: Wiring examples live under `integrations/`](#dr-org-12-wiring-examples-live-under-integrations)
  - [DR-ORG-13: Every judged home is named once, in one manifest](#dr-org-13-every-judged-home-is-named-once-in-one-manifest)
  - [DR-ORG-14: The shipped seam claim is pin-match, and the loss is named](#dr-org-14-the-shipped-seam-claim-is-pin-match-and-the-loss-is-named)
  - [DR-ORG-15: A keeper is a named, executed, blocking check](#dr-org-15-a-keeper-is-a-named-executed-blocking-check)
  - [DR-ORG-16: The working instruments do not ship; the suite does](#dr-org-16-the-working-instruments-do-not-ship-the-suite-does)
  - [DR-ORG-17: The word list is a local list, named as a local list](#dr-org-17-the-word-list-is-a-local-list-named-as-a-local-list)
  - [DR-ORG-18: The vocabulary choices stand as one collective decision](#dr-org-18-the-vocabulary-choices-stand-as-one-collective-decision)
  - [DR-ORG-19: Documentation lives in three homes, and the wiki is staged as one folder](#dr-org-19-documentation-lives-in-three-homes-and-the-wiki-is-staged-as-one-folder)
  - [DR-ORG-20: Working documents are demoted into history, never destroyed](#dr-org-20-working-documents-are-demoted-into-history-never-destroyed)
  - [DR-ORG-21: Design silence is the only gate on execution; the seam closes against the shipped shape](#dr-org-21-design-silence-is-the-only-gate-on-execution-the-seam-closes-against-the-shipped-shape)

## The form

One entry states a decision in one STE sentence as its title. The **Wrong** part states the defect as a fact about the artefact. The **Fix** part states the act of the final tree, with the file or page that holds it. The **Why** part states the principle or the measurement that makes the fix right. The **Held by** part names the gate, the test, or the page that keeps the decision. An entry with no mechanical hold states "held by this record". No entry names a person, a round, or a work process.

[↑ Back to Contents](#contents)

## The criterion

A decision earns an entry when three conditions hold. C1: the final tree differs from the default that a competent reader predicts from the problem alone. C2: the reason does not stand in the final code and documents alone. C3: a silent reversal breaks a claim of the product. A decision that fails one condition earns no entry. The record counts stand here after the conversion step: The conversion counts stand here. Source documents: 270. Ledger rows: 282. Entries on this page: 62: 7 speed, 8 security, 7 design, 9 seam, 10 suite, and 21 organization. Duplicate feeds: 213 rows. Rows that earn no entry: 56. Undecided rows: 0.

[↑ Back to Contents](#contents)

## Index by area

Areas are SPEED, SECURITY, DESIGN, SEAM, SUITE, and ORG. Each area stands in one dropdown under this index. The index stays open. The entries sit one click away.

<details>
<summary>SPEED entries state each speed decision in the standard form.</summary>

### DR-SPEED-01: The hook reads the payload with a streaming scanner

**Wrong:** The hook built the full payload in memory. It
built a full data tree from it before it gave a verdict. The
cost grew with the payload, not with the decision.

**Fix:** A streaming scanner in `src/hook/scanner.rs` reads
the payload in one pass and gives the same verdict. The
scanner builds no data tree.

**Why:** A verdict needs a small set of facts from the
payload. It does not need the payload as a structure. The
scanner was checked against the parsed form on a large
mutated corpus with zero divergences.

**Held by:** The scanner differential tests in the suite
and the speed gate.


### DR-SPEED-02: A hook denial writes one line and spawns no thread

**Wrong:** Each hook denial spawned a writer thread for the
diagnostics record. A denial paid a thread start to write
one line.

**Fix:** The denial writes to standard error at once,
through the raw line writer in `src/cli_process/`. A small
diagnostics backlog also writes at once. Only a large
backlog uses the threaded path.

**Why:** A thread is standing machinery for a one-line act.
The synchronous write gives the same bytes with no start
cost and no queue.

**Held by:** The speed gate's census of the denial path.


### DR-SPEED-03: Work is paid only on the path that uses it

**Wrong:** Every run paid for hardening, for a waitable
timer, for an attribute list, and for program searches. It
paid before it knew its own form. Help calls and context
calls paid for machinery they never use.

**Fix:** Hardening is skipped for help calls and context
vendors. The timer, the attribute list, and the program
search are built lazily, at first use, in `src/engine/`
and `src/cli_process/`.

**Why:** A cost that no consumer reads is pure loss on the
fast path. Deferred construction keeps each mechanism and
removes its price from the paths that do not use it.

**Held by:** The counted-ops census in `MEASUREMENT.md`
and the speed gate.


### DR-SPEED-04: A role is an explicit kind, not a function pointer

**Wrong:** The shared code found the role by a comparison
of function pointers. Every role program therefore linked
every role's machinery, and the gate program carried the
full guard's weight.

**Fix:** Each program's entry passes an explicit entry kind.
The kind is stored and compared in `src/lib.rs` and
`src/cli_process/`. Shedding is measured per role program,
not asserted.

**Why:** An address taken is a link kept. The gate program
fell by about one half in size when the comparison no
longer forced the link. A shedding claim without a size
measurement is not a claim.

**Held by:** The speed gate's gate-image purity checks and
the recorded role sizes.


### DR-SPEED-05: The plain run path carries no approval machinery

**Wrong:** The plain run path and the extendable path were
one path. The contain role therefore linked the approval
code it can never call. The linker could not prove the
shared path dead for it.

**Fix:** The engine core takes a decisions pair in
`src/engine/`. The plain path passes a pair that asks
nothing and names no approval code. The extendable path
passes a pair that asks.

**Why:** A capability belongs on the path that uses it. The
contain program shed about one fifth of its size, and all
approval text left its image. Behaviour did not change on
either path.

**Held by:** Marker scans of the contain image and the
speed gate.


### DR-SPEED-06: Speed is a counted census, not a feeling

**Wrong:** Speed claims rested on single timings of
unpinned artefacts. A count of operations and a time were
used as if they were the same fact.

**Fix:** The guard carries a counted-ops census of its
named calls, in `MEASUREMENT.md`. Each removed operation
class becomes a permanent gate check, so the class cannot
return silently.

**Why:** A count is exact and repeats on any machine. A
time is local and noisy. The census is the claim; the
measurement method for time stands in DR-SUITE-02.

**Held by:** The speed gate and the census tests in the
suite.


### DR-SPEED-07: Fetched security descriptors are freed, not leaked by design

**Wrong:** A repair proposed that the hardening path not
free the two security descriptors it fetches. The repair let
process exit reclaim them. The premise counted only the
one-shot programs.

**Fix:** The hardening code frees what it fetches, on every
path, in `src/sys/windows.rs`. The counted cost of the two
frees stands.

**Why:** The extension server is a resident process, not a
one-shot. In a resident, an unfreed fetch per run is a
leak with no bound. A counted saving that holds only for
short-lived processes does not hold for this product.

**Held by:** This record. The resident server's hardening
sites are the evidence.

</details>

<details>
<summary>SECURITY entries state each security decision in the standard form.</summary>

### DR-SECURITY-01: Every worker thread is hardened before its payload runs

**Wrong:** Four worker threads ran with no hardening. One
carried user credentials. A thread that runs unhardened is
surface the guard's own promise does not cover.

**Fix:** Each thread's payload sits behind a successful
hardening call, in `src/auth/` and `src/diagnostics.rs`.
On a hardening failure, the prompt is unavailable, the
operation reports not started, or the optional output is
dropped.

**Why:** The guard cannot claim a hardened process while
its own threads run outside the hardening. The failure
forms are closed: each one gives less, never more.

**Held by:** The security gate's per-thread checks, with a
negative control.


### DR-SECURITY-02: An unclassifiable PowerShell payload is denied

**Wrong:** The PowerShell adapter gave no decision, and so
allowed the run, on a payload it could not classify. It
also read the first `command` value, where the rest of the
tree reads the last.

**Fix:** An unclassifiable payload is denied, in
`src/plugins/powershell_auto.rs`. The extraction follows
the tree's one last-wins rule.

**Why:** A pass with no decision is a run with no
enforcement. Two rules for one field let a repeated key
choose the weaker rule. One rule, and a closed failure,
remove both holes.

**Held by:** The suite's PowerShell family tests and the
security gate.


### DR-SECURITY-03: The access-list hardening keeps its full price

**Wrong:** The hardening price looked reducible: two reads
and two writes of security data before each dispatch.

**Fix:** The price stands, in `src/sys/windows.rs`. The
read is load-bearing: the deny list is built so that it
preserves the allow entries that already exist.

**Why:** The two objects are distinct. A write without the
read would destroy access entries the guard has no right to
remove. A cheaper hardening that breaks the host's own
access rules is not cheaper; it is wrong.

**Held by:** The security gate and the hardening tests.


### DR-SECURITY-04: The audit file is written through a held handle

**Wrong:** Each audit write opened the file and read its
size from the directory. A foreign append between writes
could move the size the guard believed.

**Fix:** The audit log in `src/extend/log/` writes through
one held handle and tracks the size in memory. A partial
write that is retried can record a line twice.

**Why:** No grant decision reads the tracked size against
the directory entry. A foreign append can neither
manufacture nor erase a record. Twice is a safe failure;
zero is not. The wart is stated, not hidden.

**Held by:** The audit log tests and the security gate.


### DR-SECURITY-05: The verdict cache is not widened

**Wrong:** A wider verdict cache was available as a speed
measure. The cache sits on the audit channel's boundary.

**Fix:** The cache keeps its current breadth. The split
between what is cached and what is re-derived stands as
drawn in `src/hook/`.

**Why:** A wider cache would let a cached verdict stand
where the audit channel must see the act itself. A silent
pass on the audit channel is a fail-open, and no counted
saving pays for it.

**Held by:** This record and the security gate's boundary
checks.


### DR-SECURITY-06: A teardown verdict comes only from an authoritative query

**Wrong:** A proof through a completion packet was proposed
for the teardown of a non-empty job. A forged or foreign
completion packet is indistinguishable from a true one.

**Fix:** The teardown verdict comes from an authoritative
query of the job at the decision instant, in
`src/engine/`. A completion packet, if one is ever used,
may wake the guard only. No packet field may influence
the verdict.

**Why:** A signal that an in-job process can fabricate
cannot prove the job is empty. The kill-on-close property
is the backstop, and the query is the proof. Any future
adoption must also pass a forged-packet battery case.

**Held by:** Battery case (o) in the suite and this
record.


### DR-SECURITY-07: The same-user boundary is stated, not implied

**Wrong:** The guard's limits against a hostile process of
the same user were discoverable only by reading the code.
A reader could take the promise at a strength the guard
does not claim.

**Fix:** `SECURITY.md` and the limits section of the root
`README.md` state the boundary in plain terms, beside the
promise itself.

**Why:** Authority honesty is part of the mechanism. A guard
that states its own limits cannot be trusted past them by
accident. It cannot be blamed for a claim it never made.

**Held by:** The documentation claims accounting in the
suite's completeness record.


### DR-SECURITY-08: The vendor set is closed and a vendor outside the set is a usage error

**Wrong:** Silent acceptance of a string outside the set
would run a hook with no protocol and no enforcement shape.

**Fix:** The vendor set is closed at six named strings, in
`src/plugins/`. A vendor outside the set is refused as a
usage error with exit code 125, before any payload is read.
A malformed payload under a known vendor is denied.

**Why:** A hook event is a host's trust decision. A name the
guard does not know is not a variant of a name it knows. It
is an error. Errors at this border are loud and early.

**Held by:** The suite's hook family tests and the
in-tree battery checks for the gate role.

</details>

<details>
<summary>DESIGN entries state each design decision in the standard form.</summary>

### DR-DESIGN-01: Containment is born into the job, and the guard refuses other systems

**Wrong:** A portable shape over per-system backends was the
predicted default. On other systems, a process group is a
naming scheme, not a containment. A process can leave it by
a new session or a double fork.

**Fix:** A run's processes are created inside the job
object, suspended, with the job in the creation attribute
list, in `src/engine/attributes.rs`. The crate refuses to
build for a target with no proven backend. The refusal
reason stands in the crate documentation in `src/lib.rs`.

**Why:** Creation-time assignment has no assignment window
and no breakaway. Kill on close then covers every descendant
at any depth. A move to another system would carry the
product's name on a weaker promise, and the name is the
claim.

**Held by:** The refusal in `src/sys/mod.rs`, the in-tree
breakaway and descendant tests, and this record.


### DR-DESIGN-02: Four role programs, thin entry points, one shared core

**Wrong:** The base tree's large modules mixed every role's
machinery in shared files of thousands of lines. No reader
could hold one role's shape in one view.

**Fix:** The product ships four role programs, full,
deadline, gate, and contain, each with a thin entry point.
Shared machinery lives in the library core. Capability
survives as programs, not as modes of one program.

**Why:** A role is a distinct promise to a distinct user.
Separate programs keep each role's image, surface, and
failure forms separate. The measured shedding in DR-SPEED-04
and DR-SPEED-05 is only possible in this shape.

**Held by:** The design gate and the dependency graph.


### DR-DESIGN-03: Decomposition moves code mechanically and re-points tests

**Wrong:** The large modules could not be split by pure
movement alone. Tests named the single file as the home of
the code under test.

**Fix:** The split proceeds in `src/auth/`, `src/engine/`,
and `src/extend/`. Where a check's intent is structural,
the check is re-pointed at the new files as one defined
concatenation. Its assertions stay byte-identical.

**Why:** The intent of those checks is the structure, not
the unity of one file. A split that edits assertions to pass
proves nothing. A split that cannot move because a check
names a path proves the wrong thing.

**Held by:** The design gate's re-pointed checks and the
line-multiset accounting of each split.


### DR-DESIGN-04: The guard has zero dependencies

**Wrong:** A dependency would put code the reader cannot
see in this repository on the guarded path.

**Fix:** The crate depends on nothing outside the standard
library. The rule is stated in `CONTRIBUTING.md` with its
reason.

**Why:** Every line that runs is in this repository, where
it can be read, counted, and argued about. Zero is also a
supply-chain property: there is nothing to substitute.

**Held by:** The design gate's dependency check and
`Cargo.toml` itself.


### DR-DESIGN-05: Argument errors exit 125, a kept deadline exits 124, a hook denial exits 2

**Wrong:** A generic error code of 2 for argument errors
would be read by a host as a hook decision. It could block a
host event for the wrong reason.

**Fix:** The exit contract stands in `src/cli/`. The
contract gives 124 for a run that ends at its deadline. It
gives 125 for a guard error or a usage error. It gives 2 for
a hook denial under the pretooluse protocol. It gives the
child's own code for a run that ends itself.

**Why:** Each audience reads one code. A host, a shell, and
a person must never need the same number to mean two
things. The contract is small enough to state in full,
and it is stated in full.

**Held by:** The suite's contract family tests and the
in-tree battery.


### DR-DESIGN-06: The policy file is product data, not documentation

**Wrong:** `POLICY.md` reads as a document, so it was
treated as one. The program in fact reads and prints this
file at run time. The context vendors print its text into
host sessions.

**Fix:** `POLICY.md` is ruled runtime product data with a
documentation view. The view lives on the wiki page for
the policy file and layout. A vocabulary check over
`POLICY.md` itself reports; it does not gate, and it never
rewrites the file on its own.

**Why:** The file's wording is product surface: hosts show
it to users. A documentation process that silently edits
product surface changes the product without an argument.

**Held by:** The reporting-mode check and the completeness
accounting, which dispositions each finding by name.


### DR-DESIGN-07: Every file carries its copyright header

**Wrong:** A tree that may be folded, file by file, into
another repository cannot rely on one licence file. One file
at its root cannot travel with an extracted file.

**Fix:** Every file in the tree carries its copyright and
licence header. The header is a landing requirement, not a
courtesy.

**Why:** Incorporation is a stated use of this product. A
file that carries its own provenance stays honest when it is
copied alone. A reader never has to guess which terms cover
the file in hand.

**Held by:** Gate hold G1 of the design gate, from the
first organization step onward.

</details>

<details>
<summary>SEAM entries state each seam decision in the standard form.</summary>

### DR-SEAM-01: All platform declarations live in exactly one module

**Wrong:** Platform declarations spread across the tree
make every file a possible platform file. No reader can
say which files can reach the platform.

**Fix:** Exactly one module holds one operating system's
declarations: today `src/sys/windows.rs`. Its one child,
`src/sys/windows/rng.rs`, holds logic, not declarations.

**Why:** One home makes reach a question with an answer.
Every other seam decision on this page is a consequence
of this one's shape.

**Held by:** Gate hold G10 and the census.


### DR-SEAM-02: Reach is judged by a pinned computation and a pinned pure set

**Wrong:** A judgement of platform reach by reading alone
cannot be checked, and two readers reach two answers.

**Fix:** A pinned computation, the census, judges each
production file's reach through the named channels. The
files it proves reach-free form the pure set, pinned by
name in the dependency graph.

**Why:** A claim that a file cannot reach the platform is a
claim a machine must hold. Its breadth is one a reader can
inspect. Pinning the set by name makes any change to it a
visible event, never a drift.

**Held by:** Gate holds G3 and G4, in pin-match form; see
DR-ORG-14 for the breadth of that hold.


### DR-SEAM-03: The one pinned exception stands inside the definition

**Wrong:** One pure file names a standard-library platform
path once, for its encoding work. Treated as an exception
outside the rule, it makes the pure set look approximate.

**Fix:** The exception is part of the definition of the pure
set. The set is the files at zero, plus
`src/engine/environment.rs` at its one named occurrence.

**Why:** A definition that names its one exception is
exact. A definition that hides it teaches the reader that
other exceptions may also be hidden.

**Held by:** The pin in gate hold G3 and the census.


### DR-SEAM-04: No file receives platform names by a glob

**Wrong:** A glob import or a glob re-export can supply a
platform name to a file. The file's own text does not name
it. The census would then judge a text that is not the whole
truth.

**Fix:** Every production file imports the platform names
it uses explicitly, in its own text, from the one
declaration module. A visibility-carrying glob re-export
of the backend is banned in every spelling. A private glob
import inside the backend's own files is the tree's form
and is allowed.

**Why:** The seam claim is a claim about texts. A supply
channel that leaves no trace in the text voids the claim
at exactly the file it feeds.

**Held by:** Gate hold G10, clause (a), in its re-export
form.


### DR-SEAM-05: The name chase fails closed at fixed bounds

**Wrong:** The computation that follows a name through
aliases and re-exports can be made to run without bound. It
can also cease in silence and call the file pure.

**Fix:** The chase is bounded at 8 hops and 16 variants.
Exhaustion inside a chase whose resolution fails records a
named refusal, whatever the item's spelling. An item proven
a non-wrapper by a completed resolution is the exception.

**Why:** A bounded computation that refuses is honest. An
unbounded one is a hang. A chase that ceases in silence
gives a false proof. A rename must never be able to suppress
a refusal: the spelling is not the fact.

**Held by:** The design gate's refusal records and the
census's stated bounds.


### DR-SEAM-06: Seam terms are defined from first principles

**Wrong:** The seam vocabulary, reach, mint, census,
pure, carriage, ring, was defined by the computation that
uses the words. A reader had to trust the program to know
what its own claims mean.

**Fix:** Each term is defined from the one first principle.
That principle stands in the wiki page for the seam
criterion and in the glossary. A body of code depends on the
platform exactly where it uses a platform mechanism. The
computation is then one test of the definitions, not their
source.

**Why:** A criterion whose words mean what its program does
is circular. Definitions that stand alone let a hostile
reader judge the program against the words. That direction
is the only one that proves anything.

**Held by:** Lint hold M5, which holds the glossary and
the dictionary equal, and this record.


### DR-SEAM-07: There is no architecture axis in the shape

**Wrong:** A folder axis for processor architecture was
proposed. The tree carries no architecture-specific code,
so the axis would organize an empty set.

**Fix:** No architecture level exists in `src/sys/`. The
declarations are width-clean. The 64-bit ARM and 32-bit
targets stand at the type-and-borrow tier only; neither
is a release target.

**Why:** An axis earns its place by the variance it
organizes. Where no code varies, a folder is a claim that it
does. The tier label keeps the type check from being read as
proof of execution.

**Held by:** Gate hold G10, clause (b), by entry-set
comparison of the `sys/` tree.


### DR-SEAM-08: The shape binds the denotation, not the literal path

**Wrong:** The seam shape was at risk. A reader could read
it as a claim about one literal path for the declaration
module. A move of the module would then look like a change
of shape.

**Fix:** The shape binds what the module denotes, the one
home of one system's declarations, not its spelling. The
module moved to `src/sys/windows.rs` with its content
verbatim, and the census was re-pinned in the same
landing.

**Why:** A criterion about paths breaks on the first
honest rename. A criterion about denotation survives
every move that preserves the fact it states.

**Held by:** The census facts, exact before and after the
move, and gate hold G10.


### DR-SEAM-09: A constant name halts the chase as a constant

**Wrong:** The name chase read a constant through the same
paths as a function. A plain constant named through a
re-export could be recorded as a wrapper use. A file with no
reach gained a false reach fact.

**Fix:** Both instruments that run the chase carry a
constant end. It is a distinct kind, placed after the
wrapper and platform-derived ends. It is file-granular and
inert. Its exact text stands in the instruments that ship as
working tools. Its form was proven on the unmoved tree
before any move used it.

**Why:** Kind order is meaning here. An end placed beside
the function end manufactures reach. Placed after the true
wrapper ends, it names a constant as a constant. It changes
no other verdict.

**Held by:** The completed-run proof duty: no instrument
result counts from a run that did not complete.

</details>

<details>
<summary>SUITE entries state each suite decision in the standard form.</summary>

### DR-SUITE-01: Every test is counted once, and the evidence states the count

**Wrong:** A test that runs under two harnesses can be
counted twice. A total built that way certifies a suite
that does not exist.

**Fix:** The suite reports one counted-once total, derived
by a census of its own test files, in
`suite/COMPLETENESS.md`. The evidence file that states the
total is refreshed only from a green run, in its canonical
form.

**Why:** The total is the suite's first claim about
itself. If the count can inflate, every claim under it is
unpriced. The refresh discipline keeps the evidence and
the count from ever describing different suites.

**Held by:** The census-evidence pair of tests inside the
suite itself.


### DR-SUITE-02: Speed is reported as distance above the harness floor

**Wrong:** A raw median race against one competitor was the
headline number. Two independent minimal tools agree at one
value on a given machine. That value is a machine and
harness floor, not a competitor's trick. The old anchor
binary also predated the converged tree, so the timing and
the census described different artefacts.

**Fix:** The campaign reports the guard's distance above
the floor, decomposed against the counted-ops census. The
anchor is the current binary. Any citation of the old
race carries the fact that its anchor predates the
converged tree.

**Why:** A race against a floor measures the floor. A
distance above it, priced operation by operation, is the
only number that can convict or clear the guard itself.

**Held by:** The campaign report rules and their tests in
the suite.


### DR-SUITE-03: A sensitivity bound is estimated from the plain condition only

**Wrong:** The sensitivity gate's bound was estimated from
a mixed set of conditions. The bound then certified the
noise of the loaded conditions as if it were the plain
condition's spread.

**Fix:** The bound is estimated from the plain condition
only, in the campaign harness. The spread estimator is
the median absolute deviation, not a quartile range.

**Why:** A bound estimated from a mixed condition certifies
noise. The plain condition is the only one whose spread
the gate may honestly promise to detect a change against.

**Held by:** The sensitivity gate's tests in the suite.


### DR-SUITE-04: Claims about a competitor are stated exactly

**Wrong:** Kill-path claims about one competitor are easy to
state loosely. The loose forms are these. It kills a tree.
It acts at its deadline.

**Fix:** The suite states the facts exactly and tests the
statement. On Windows the tool ends only the direct child.
Its first timed action waits five seconds behind a graceful
step. That step is not implemented on Windows. In the
preserved trials it killed nothing inside a two-second
deadline.

**Why:** A comparison is evidence only at the precision of
its weakest claim. An exact false claim about a competitor
would be a manufactured win. The measurement rules bar that
win at the same strength as a false claim about the guard.

**Held by:** The suite's exact-statement test for this
competitor's facts.


### DR-SUITE-05: One shared formulation serves the runner and the checker

**Wrong:** The runner and the checker each computed the
scale count in their own form. On one live shape they
disagreed: a cell could be sound to the runner and an
inconsistency to the checker.

**Fix:** One formulation in the campaign's plan module is
called by both. A matrix of cells shows the verdicts in
agreement.

**Why:** Two computations of one fact will diverge at the
edge neither author tested. A checker that can disagree
with the runner cannot certify the runner.

**Held by:** The agreement matrix tests in the suite.


### DR-SUITE-06: An extreme value is named, never an exception

**Wrong:** Values far outside the measured range, a number
with hundreds of digits, made producers and medians raise
errors. They gave no verdict instead. A single crafted
record could end the accounting itself.

**Fix:** Every numeric hold in the campaign is total. An
implausible value is named as a defect of its record. An
extreme time resolves as not measured. Only plausible values
enter any median.

**Why:** The accounting must survive the data it exists to
judge. A crash is a verdict the record never states, and
an unstated verdict is a hole in the completeness claim.

**Held by:** The paired-extremes regression tests in the
suite.


### DR-SUITE-07: The plan anchors the census; unplanned records are not evidence

**Wrong:** A deleted cell could pass. The checks bound the
records that were carried, not the cells that were planned.
A record with no planned cell could also move medians,
coverage, and kill state.

**Fix:** The census is anchored to the campaign's own plan.
A planned cell with no records renders unevaluated, with its
units named. A record that matches no planned cell is
excluded from every evidence consumer. It is listed under
its own name as a fault.

**Why:** Evidence is a relation between a plan and a record.
A record outside the plan is not weak evidence; it is not
evidence. Letting it move a verdict lets fabrication certify
a campaign.

**Held by:** The plan-anchored census tests. The sentinel
rule also holds it: a missing plan anchor caps a dimension
at unevaluated.


### DR-SUITE-08: A failed usability exercise does not mint a collected dimension

**Wrong:** The usability dimension counted an exercised
prompt as collected whatever its outcome. A fail therefore
minted the same state as a pass.

**Fix:** The dimension is three-valued, in the suite's
evidence rules. A fail renders the dimension failed. The
campaign is incomplete with a standing loss. A pass with no
fail renders it collected. No exercise renders it
unevaluated.

**Why:** Collection and verdict are different facts, and
the sibling dimensions keep them apart. A dimension that
cannot fail cannot be lost, and a campaign that cannot
lose a dimension no longer measures it.

**Held by:** The usability state tests, which hold the
rule equal across code, schema, and campaign data.


### DR-SUITE-09: One vocabulary property is carried open, and its converse is not claimed

**Wrong:** The suite's bookkeeping writes a resource note
from a closed vocabulary. A static check was asked to prove
that no code path can write a note outside the vocabulary.
The forward direction is proven over the enumerated routes.
The converse is not proven. Write forms exist that the
enumeration does not cover. One form is call-spelled writes.
One form is a cross-module helper write. One form is a
dict-merge rebind under a variable key. One form is
indirection that hides the executing callable.

**Fix:** The property stands as a named open item in
`suite/COMPLETENESS.md` and on the wiki's testing page, in
this exact form. It closes only by a completeness argument
that survives hostile review. It closes by a restructure
that makes the property hold by construction.

**Why:** The item concerns the suite's own bookkeeping. It
does not change the guard, the method of measurement, or
any measured result. A claim of completeness that the
enumeration cannot support would be the one defect this
suite exists to prevent.

**Held by:** This record and the named open item in the
completeness accounting.


### DR-SUITE-10: Two campaign dimensions are not applicable

**Wrong:** The campaign's eight dimensions were carried as
if all eight applied to every arm. Two of them name surfaces
no arm has. No arm offers a soak, a dataset, or a recovery
surface for one. The specification's own terms exclude the
other.

**Fix:** Both dimensions stand as not applicable, with the
reason stated beside each, in the completeness
accounting. Either reopens only on new evidence, named as
such.

**Why:** A dimension that cannot be collected and cannot be
failed is not rigour. It is a permanent unevaluated state.
That state hides the dimensions that do apply. A stated
non-applicability is a claim, and it is priced like one.

**Held by:** The dimension ledger in the completeness
accounting.

</details>

<details>
<summary>ORG entries state each organization decision in the standard form.</summary>

### DR-ORG-01: The platform axis is operating-system first, at most two levels deep

**Wrong:** A silicon-first axis was proposed for `sys/`.
Silicon organizes nothing in this tree. No code varies by
processor family. The first level would sort by a property
that decides nothing.

**Fix:** `src/sys/` is organized by operating system, at
most two levels deep. Today exactly one level is
populated: `windows.rs`, with its logic child under it.

**Why:** An axis must sort by the variance that exists.
The operating system is where every platform fact in this
product varies; the processor is where none does.

**Held by:** Gate hold G10, clause (b).


### DR-ORG-02: Silicon and architecture are not levels of the axis

**Wrong:** Folder levels for silicon and for architecture
would stand as permanent claims about the tree's shape. No
file's existence is explained by them.

**Fix:** Neither level exists. See DR-SEAM-07 for the
architecture half of this decision and its tier.

**Why:** A level that organizes an empty set is not neutral.
It teaches the next reader to expect code the product
refuses to grow. It must be argued out again at every
review.

**Held by:** Gate hold G10, clause (b), by full entry-set
comparison.


### DR-ORG-03: Linux and macOS are named places, not backends

**Wrong:** The universality of the product's identity
could be read as a claim that other backends exist. A
folder with a declaration module in it would state that
claim in code.

**Fix:** `src/sys/linux/` and `src/sys/macos/` each hold
exactly one file. The file is a README that names the place.
It states its evidence tier as theory. It states its revival
condition.

**Why:** A named place keeps the identity honest and the
door findable. A placeholder backend would build a guard
that cannot keep the promise of DR-DESIGN-01. That outcome
is the one the refusal exists to prevent.

**Held by:** Gate hold G10, clause (b): a second file in
either folder fails the comparison.


### DR-ORG-04: The `sys` root module holds selection and refusal only

**Wrong:** A root module that also forwards declarations
becomes a second home for platform facts. The seam's
one-home rule dies by convenience.

**Fix:** `src/sys/mod.rs` holds exactly the selection of
the one backend and the refusal for every other target.
It holds no declaration, no type, and no forwarding use.

**Why:** The root's job is to say which world the crate is
in. It refuses the worlds it cannot serve. Anything more
makes it a facade, and a facade is a second seam.

**Held by:** Gate hold G10, clause (c), by item-set
comparison against the enumerated form.


### DR-ORG-05: Integrations are a top-level axis, never under `sys/`

**Wrong:** Integrations were sketched under the platform
axis. An integration is a protocol a host speaks, not a
property of an operating system. The same host protocol can
arrive on any system.

**Fix:** Adapters live at `src/plugins/`, a top-level axis
beside the core and `src/sys/`. No host-kind level,
no folder for a class of host, stands between the axis
and the family files.

**Why:** Two axes that vary independently must not nest.
Nesting the protocol axis under the platform axis would
state that a protocol belongs to a system. Every future host
would have to argue its way out of the wrong tree.

**Held by:** The dependency graph and this record.


### DR-ORG-06: Adapters are families, not one file per vendor string

**Wrong:** One file per vendor string would split the
context vendors from the decision vendors. Those vendors
share their protocol's core. One file per vendor string
would duplicate the scan that all of them run.

**Fix:** `src/plugins/` holds one file per family. A family
is one scan of the payload's command against the envelope
rule, and one verdict computation for its vendors.
`src/plugins/verdict.rs` holds the shared scan-and-verdict
form. The context form is a named exception inside its
family, not a second family.

**Why:** The unit of an integration is the protocol's
decision, not its spelling. Files drawn at the string would
multiply the one place a verdict rule must live. The rules
would drift apart in silence.

**Held by:** The six-vendor differential checks hold this
decision. Gate hold G7's link and structure checks over the
tree's documentation of the axis also hold it.


### DR-ORG-07: One vendor has a named place and no folder

**Wrong:** A wiring example for one host was at risk of
shipping as a folder. The folder would present that host as
a supported integration with an adapter behind it. No
adapter for it exists.

**Fix:** No `integrations/codex/` folder ships. The host
keeps a named place in the integration documentation, with
its revival condition. The condition is an adapter that wins
its argument, wired and tested like the others.

**Why:** A folder is a claim of support. The honest forms
are an adapter or a named absence. A dressed example is
neither. A reader who wires from it discovers the truth at
run time.

**Held by:** This record and the folder inventory check at
the documentation steps.


### DR-ORG-08: Role dispatch and the help texts leave the large files

**Wrong:** `src/lib.rs` mixed the crate's criterion text
with live role dispatch. `src/cli.rs` mixed parsing with the
help texts and the automatic form. Each file answered two
questions, so neither could be read as one thing.

**Fix:** Dispatch lives in `src/dispatch.rs`. The automatic
form lives in `src/cli/auto.rs`. The usage constants and the
help function live in `src/cli/help.rs`. That file holds
text and the selection of text. It holds no parsing, no
validation, and no decision.

**Why:** A file that does one thing can be reviewed in one
sitting and changed for one reason. The splits also made
the help file provably reach-free; see DR-ORG-10.

**Held by:** The design gate's disposition checks and the
dependency graph.


### DR-ORG-09: One selector chooses among the usage texts

**Wrong:** Three call sites each carried the same verbatim
conditional that chooses a usage text by role. A change to
the choice had three homes. A name chase could not read the
constant names through any spelling the sites used.

**Fix:** `src/cli/help.rs` holds one item, `usage_for`,
whose body is that conditional, verbatim. All three sites
take the form of a help call on the selector's result.

**Why:** The warrant is the triplication itself. One
decision, three homes, is a defect under the consolidation
principle. That principle holds whatever any instrument can
or cannot read. The selector sits over the texts it chooses
among, which is where a reader looks for it.

**Held by:** The design gate. The closed API diff at the
step that landed it also holds it. That diff showed exactly
one added symbol.


### DR-ORG-10: The pure set is computed, never curated

**Wrong:** A split file was born whose designed content
reaches nothing on any channel. The pre-registered count of
the pure set therefore read one low. The count could have
been defended by suppressing the computation's report. It
could have been defended by giving the new file a reach it
does not need.

**Fix:** The pure set's membership is recomputed at every
step by the live census. The pin in `tools/layout.json` is
edited only after the computation names the delta. The split
file, `src/cli/help.rs`, entered the set as its twelfth
member, named in advance as the step's only delta.

**Why:** The set is a measure, not a list this product keeps
by hand. A count defended against its own computation is a
quota. A quota is the one form of number this tree's rules
refuse. A member's loss, by contrast, is never a step's to
name into existence: it needs its own seam argument.

**Held by:** Standing gate (iv) at each execution step. The
anti-curation order also holds it: the computation runs
before the pin is edited.


### DR-ORG-11: The test helper is not a shipped program

**Wrong:** The test helper lived in `src/bin/`, among the
four role programs. Its home stated that it is a fifth
product. The production census had to carry rules about a
file that is not production.

**Fix:** The helper lives at `tests/support/testkit.rs`,
content verbatim. `src/bin/` holds the role programs,
and nothing else.

**Why:** A folder's membership is a claim about what the
product is. Test machinery under the tests states the truth.
The move cost no content change at all. That is the proof
that the old home was the only defect.

**Held by:** The design gate's cap entry for the file at its
new home. The manifest key that names it also holds it.


### DR-ORG-12: Wiring examples live under `integrations/`

**Wrong:** Host wiring files lived under `examples/`. An
example is read as a model to copy and adapt. These files
are the supported wiring for named hosts. One host's absence
from the folder is itself information.

**Fix:** The folder is `integrations/` at the tree root, one
subfolder per wired host, contents byte-identical at the
move. The manifest key that names the folder was retired.
Its replacement was born in the same landing. Every reader's
key list followed in the same landing.

**Why:** The name of a folder is the first instruction it
gives. A key retirement whose readers do not follow is a
broken source of truth. The readers fail by name in that
case. That failure is the designed proof.

**Held by:** Gate holds D9 and D36, which read the wiring
through the manifest at its new root.


### DR-ORG-13: Every judged home is named once, in one manifest

**Wrong:** The design gate, the graph generator, and the
execution plan each carried tree paths as literals.

**Fix:** One manifest, `tools/layout.json`, names every home
that a key is earned for. A key is earned when the plan must
name the home to move it. A key is earned when the home keys
a path-set table an instrument reads. A key is earned when
two readers must agree on the home. Each reader keeps
exactly one literal of its own: the manifest's name, beside
itself. Homes that earn no key stand as a named class
exclusion, with their reason and their revival.

**Why:** A home stated in three places is three facts that
can drift. A home stated once is one fact. It can be wrong
once, loudly, for every reader at the same time. The
equivalence proof on the unmoved tree shows a second fact.
The change of form changed no verdict.

**Held by:** The readers' fail-closed duties hold this
decision. The gate's key-set equality checks hold it. The
closure-stage verification of every value against the landed
tree holds it.


### DR-ORG-14: The shipped seam claim is pin-match, and the loss is named

**Wrong:** The seam claim was stated at the breadth of its
full derivation. That breadth is this: no file outside the
pure set can reach the platform. A derivation-class change
inside a file is not seen by the shipped check. A new alias,
an initialiser, or a re-export is such a change.

**Fix:** The shipped claim is reduced to exactly what the
shipped instruments check. The reduction is stated as a
named loss with its revival. That statement stands in the
wiki page for the seam criterion. The revival is a run of
the full working computation. Any change to an initialiser,
an alias, or a re-export in a pure file's supply chain
requires that run.

**Why:** A claim at a breadth its keeper does not hold is a
false claim. The wider fact may be true. The narrow claim is
kept every time the gate runs. The wide fact stands as
history of the derivation, not as a standing promise.

**Held by:** Gate holds G3 and G4 for the narrow claim. The
revival duty is stated in [The Seam Criterion](The-Seam-Criterion.md).
No instrument can state it.


### DR-ORG-15: A keeper is a named, executed, blocking check

**Wrong:** The word "keeper" carried the design's
completeness claims with no definition. An unnamed
intention to review could be spent as if it were a check.

**Fix:** A keeper is a check that meets three conditions. It
is named in the place that makes the claim. It is executed
at a named step or by a standing instrument. Its failure
blocks the claim it keeps. A check that runs but whose
failure changes nothing is a report, not a keeper. A claim
may not be its own keeper.

**Why:** Completeness is the product's central predicate, in
the suite's accounting and in this record alike. A predicate
built on an undefined word can be satisfied by anything.
That is the same as being satisfied by nothing.

**Held by:** Lint hold M5's form is applied to the word
itself. Every use of the word in the documentation set must
read under this definition.


### DR-ORG-16: The working instruments do not ship; the suite does

**Wrong:** The plan shipped the design gate and the word
list checker as product, in one of two argued forms. Both
forms made the product carry machinery whose only reader is
the work itself. One form weakened a named hold to make the
rebuild fit.

**Fix:** No instrument ships in this product. The gate, the
generator, the word list checker, and the manifest are
kept outside this repository. The named branch is
`ptg-gauntlet-instruments`. A README states what
each holds and what it does not hold. The test and benchmark
suite is final product and ships at `tests/`, as designed.
The shipped graph at `wiki/Dependency-Graph.md` is generated
by the working generator before the instruments are disposed
of.

**Why:** Product surface must win its argument as product.
An instrument's results can be product while the instrument
is not. Preservation in a named home loses nothing that a
shipment would keep.

**Held by:** The final tree's file accounting, which
contains no `tools/` folder, and this record.


### DR-ORG-17: The word list is a local list, named as a local list

**Wrong:** The documentation set is written under a
controlled vocabulary discipline. Its source standard is a
licensed text this project does not hold. Silence about the
substitution would pass a local list off as the standard.

**Fix:** The list is named PTG-STE-1: the project's local
word list and rule subset, version 1. The statement that it
is not ASD-STE100 stands in three places. Those places are
the dictionary's own front matter, the glossary, and the
completeness accounting. Any document that names the
discipline without the qualification in the same document is
non-conformant by this rule.

**Why:** A conformance claim is a claim like any other. It
is kept at its true breadth or not at all. The approach,
one word for one meaning, short sentences, one instruction
per sentence, is followed in full. The provenance is stated
in full. Neither half may borrow the other's strength.

**Held by:** The STE lint, holds M1 to M5, over the
documentation set.


### DR-ORG-18: The vocabulary choices stand as one collective decision

**Wrong:** Each term of the product had several live
synonyms in the old texts. The guard and its deadline each
had other names in those texts. A reader could not tell
whether two words named one thing or two.

**Fix:** Each meaning has one approved word, held in
`wiki/dictionary.tsv` and shown in the glossary. A
hold is one granted instance; an extension is the
mechanism. An owner is a role in the rule; a parent is a
fact about birth. A vendor is a protocol; a host is a
program. The full table is the dictionary itself.

**Why:** One word for one meaning lets a sentence be checked
by a machine. It lets a sentence be trusted by a reader at
the same time. The choices earn one entry together. Singly,
each is a preference. Together, they are the discipline
every other page depends on.

**Held by:** Lint holds M1 and M3, and hold M5 for the
glossary's equality with the dictionary.


### DR-ORG-19: Documentation lives in three homes, and the wiki is staged as one folder

**Wrong:** The old root document was at once a door, a
manual, and a reference. Three jobs in one file made each
job worse and made every claim hard to find twice.

**Fix:** The set lives in three homes. The repo root holds
the door: a short README. The wiki holds
the teaching and reference set. It is staged as one
self-contained folder of flat pages. The filenames of the
pages are their titles. Repo documents hold what the
platform or the version tie puts in the repo. Every section
of every page ends with a link back to that page's contents.

**Why:** A reader arrives with one job. A home per job means
the door can be short. The teaching can be full. The staged
folder can move into the wiki in one mechanical step. No
link is rewritten in that step.

**Held by:** Gate hold G7 and lint hold M4, which check
contents, back-links, and link existence together.


### DR-ORG-20: Working documents are demoted into history, never destroyed

**Wrong:** A final tree that carries its own working papers
presents process as product. Deleting them would remove the
only place a converted decision can be checked against its
source.

**Fix:** The documentation is completed first. This record
is converted second. The working documents are removed last.
Each removal is its own preserved step. The version record
states that removed documents remain in the repository's
history at the parent commit.

**Why:** The order is the proof. History shows the
conversion before the removal, so a reader of history can
verify that nothing was deleted unconverted. Removal is
demotion, not destruction.

**Held by:** The execution order itself holds this
conversion. The sanity interface for this conversion also
holds it. A fresh reader can compare entries on this page
against picked source documents.


### DR-ORG-21: Design silence is the only gate on execution; the seam closes against the shipped shape

**Wrong:** A sequence stood in the record. In that sequence,
the seam axis had to close before any organization step
could run. The governing design states no such gate. The
sequence would have judged the seam's shape before the shape
that ships existed.

**Fix:** Execution runs after the design's own silence,
with no step waiting on a seam verdict. The seam's
closing argument and the pre-closure sanity check are
held at the final step, against the reorganized tree.

**Why:** The shape that must survive hostile judgement is
the shape that ships. Judging an earlier shape proves a fact
about a tree no reader will ever meet. It leaves the shipped
tree unjudged at exactly the point that matters.

**Held by:** The closure conditions at the final execution
step, and this record.

</details>

[↑ Back to Contents](#contents)
