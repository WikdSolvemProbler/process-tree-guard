# Windows flat-index component screen

**Outcome: rejected.** The flat sorted index failed the prespecified private cost screen in 10 of 12 cases. It was not installed in the product. No full guard build, startup comparison, or performance ranking followed. The protocol was frozen privately before the run; this was not an externally registered study.

The source baseline is process-tree-guard commit 33f76dbe96082656b5ae4b36527a247228f8d932. Both modules preserve the rate policy; the candidate changes only the internal index representation. The insertion-ordered rate vector, 1,024-key cap, outside-client share, refusal and refund rules, liveness callback order, and expiry behavior remain checked by the probe. The baseline module is the current source with CRLF line endings normalized to LF. The patch recreates the candidate from it.

This is a standalone component experiment. It calls the actual limiter methods under synthetic logical time and liveness. Its only native-process shim panics if reached. It does not exercise operating-system process containment or cleanup. Both implementations share one executable; the baseline's random-state initialization can already be warm. This experiment cannot measure DLL loading, process startup, real IPC contention, or the full guard.

## Files and numerical replay

- [Frozen protocol](MICRO-PROBE.md), [patch](FLAT-INDEX.patch), and [complete probe source](probe/extend.rs).
- [All 792 raw rows](raw.csv): 24 starter rows and 768 measured rows, with every ordered observation retained.
- [Independent replay](replay_cost.py) and [all exact results](replay.json), including 192 signed block contrasts.
- [Synthetic verifier contract checks](check_replay_contract.py), which are explicitly not empirical timing evidence.
- [Summary](summary.json) and [file hashes](manifest.json).

Python 3.12 standard library is sufficient to reproduce the numerical analysis; no Windows machine or administrator access is needed. Run from this package directory and choose an output name that does not already exist. For example, with an existing trusted finite-tree launcher on Windows:

~~~text
process-tree-guard --timeout 15 -- python -I -B -X utf8 replay_cost.py raw.csv replay-output.json
process-tree-guard --timeout 15 -- python -I -B -X utf8 check_replay_contract.py replay_cost.py synthetic-check-output
~~~

On Linux, the numerical replay can instead use an ordinary GNU timeout process group:

~~~text
timeout --signal=KILL --kill-after=2s 15s python3 -I -B -X utf8 replay_cost.py raw.csv replay-output.json
~~~

Replay must report 792 rows, cost_screen_pass false, and these failed cases: hit/refund at 32 and 1,024 keys, ascending fill, descending fill, backed-off miss, one-dead churn, mostly-retaining expiry, alternating retention, full expiry, and shutdown. Only hit/refund at one key and the first full miss passed this particular screen.

A successful process exit means the driver or replay completed. **It does not mean the cost gate passed.** No population equivalence or noninferiority claim follows from these descriptive values.

The supplied-file manifest authenticates this archive. A replay on another operating system can use different JSON line endings while reproducing the same numbers; regenerated JSON is not required to be byte-identical across operating systems.

## Regenerating the bounded experiment

The recorded experiment used Rust 1.98.1 (48a229ceaefd4985c50990b14116b6d856af0985), MSVC tools 14.51.36231, SDK 10.0.28000.0, Rust 2021, Windows x64 MSVC, opt-level 3, fat LTO, one codegen unit, panic abort, stripped symbols, and warnings as errors. There are no third-party probe dependencies.

Use a Developer Command Prompt with those exact compiler and SDK selections and a trusted finite whole-process-tree launcher. These command examples are for cmd.exe; preserve the native child's output bytes and disable optional launcher tracing. Do not overwrite an existing output or silently retry an incomplete run.

~~~text
process-tree-guard --timeout 30 -- rustc --edition=2021 --crate-name=ptg_flat_index_probe --target=x86_64-pc-windows-msvc -C opt-level=3 -C lto=fat -C codegen-units=1 -C panic=abort -C strip=symbols -D warnings probe/main.rs -o limiter-probe.exe
process-tree-guard --timeout 120 -- limiter-probe.exe > replica.csv 2> replica.stderr.txt
process-tree-guard --timeout 15 -- python -I -B -X utf8 replay_cost.py replica.csv replica-analysis.json
~~~

The private recorded compiler recipe also explicitly bound the selected linker. Bind and retain the actual tool versions, source hashes, artifact identity, command, original process outcome, and output when regenerating. An empty stderr and native success are prerequisites for replay. The program enforces a 100-second internal budget. The complete compile and run were each executed once, inside independent 30- and 120-second process-tree deadlines.

The fixed schedule has two starter visits, followed by 16 ABBA/BAAB blocks. The decision rule requires both the paired median and empirical p90 candidate-minus-baseline elapsed time per operation to be at most zero in every case. The exact rational values and all individual rows are retained. No tails, blocks, or slow cases were excluded, and no threshold changed after the run.

A new machine, compiler, or run may produce different timings or a different screen outcome. Preserve that new observation as its own dataset. Do not substitute it for this one.

## Review, history, and limits

Two independent Sol reviewers approved the same source, corrected protocol/carrier, and scoped actual results. They separately recomputed all raw rows and exact contrasts. Their agreement is not independent external reproduction.

Before execution, review found an incorrect final patch-hunk count, missing off-clock liveness-order coverage, and a replay parser that accepted a premature terminal record. Those were corrected before the single compile/run. Three synthetic parser fixtures checked an analytically known result and rejection of premature or missing terminal records. Earlier drafts and failed metadata reads remain preserved privately; they were not used to replace or rerun the experiment. A publication-text generation error occurred before any document write; the data were unchanged.

All behavior assertions completed, but the cost screen failed. In particular, the candidate's measured median one-dead churn cost increased by 2246675/128 ns per operation, and its median descending-fill cost increased by 4160425/16384 ns per admission. These are internal elapsed-time contrasts, not estimates of full guard overhead.

The supplied active workstation was used without a requested idle window. This package does not include a hardware fingerprint, resource-load trace, or confidential host/compiler-path records. The compiled image digest is retained in the summary; the binary is not redistributed. These restrictions limit exact environment reconstruction. The portable source and raw numerical replay do not complete the broader academic benchmark package.

The canonical product remains unchanged. The Windows speed requirement, comprehensive campaign, and Linux/macOS backend qualification remain open.

