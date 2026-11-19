# Historical Windows repaired-reference replay

This package reproduces the numerical analysis of the 6 November 2026 observed-load Windows x64 MSVC comparison. The consolidated guard was built from `8bf828a`. The reference was the Windows-repaired `9c6f0df` build derived from `681ad230`. The arms use their original `main`, `wake`, and `bare` labels. The protocol, full source commit IDs, and executable hashes are in [expected.json](expected.json).

## Run the replay

Use CPython 3.12 with a finite process-tree deadline. With the Windows guard on PATH, run the input check and full calculation from the repository root.

```powershell
process-tree-guard --timeout 30 -- python -I -B tests/evidence/windows-reference-20261106/replay.py --check-inputs
process-tree-guard --timeout 360 -- python -I -B tests/evidence/windows-reference-20261106/replay.py
```

The full replay regenerates the seeded schedule, all 16 contrasts with their diagnostics and intervals, and all four descriptive paired summaries. Pass `--out report.json` to save the regenerated numerical report to a new file.

## What the records cover

[slots.jsonl](slots.jsonl) keeps the original index, status, and elapsed nanoseconds of each of the 2,967 slots. The frozen generator reconstructs the 15 controls, 72 warmups, 2,880 measured trials, 40 chronological macroblocks and all six launch orders. The original verifier checked the private native, telemetry, and qualification receipts. The public replay checks the projected rows and their numerical analysis. An earlier incomplete attempt produced zero slots and has a separate archived receipt hash.

The primary estimates are differences between nearest-rank marginal quantiles. The adjusted outcomes subtract each triplet's bare duration before those quantiles are taken. The descriptive paired-difference quantiles are reported separately. The original seeded bootstrap uses 100,000 resamples for each block length, with length 4 primary and lengths 2 and 8 for sensitivity. Its nominal inference assumes stationary weak dependence across macroblocks.

The consolidated guard's manual-startup raw p90 was **7.9079 ms slower**, with a primary interval of **[2.6032, 17.0082] ms**. The other 15 contrasts were unresolved. This is historical evidence from one observed-load run. The quiet-host campaign, current-version speed acceptance, and later diagnostic qualifications have separate records. [Benchmark history](../../benchmark/README.md#windows-comparison-with-repaired-wake-6-november-2026)

## Verification

The archived Windows verifier reproduced its original analysis byte for byte. The public replay independently reproduced every numerical record on Windows x64, Linux ARM64 and macOS ARM64. The regenerated report SHA-256 was `bc0f1914b7095d27ee123835919756780c7961192e601fe2ad5f14b101e02a4b` on all three platforms. Both remote scratch directories were removed. Local input checks rejected a changed duration and reordered rows.
