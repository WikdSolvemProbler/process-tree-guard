<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# November 17 Windows campaign numerical replay

The public rows retain all 1,940 recorded slots in seed order. That includes 36 controls, 224 warmups, and 1,680 measured slots. Every retained numeric token, categorical value, and null was checked against the private originals. The private verification also matched every published calculation to the original observations with the hash-verified configuration. [Original report correction](../windows-campaign-report-correction-20261117-summary.json)

## Reproduce the calculations

Download and extract the [source ZIP for commit `aa634674cf09e595667694655375a4692532e199`](https://github.com/WikdSolvemProbler/process-tree-guard/archive/aa634674cf09e595667694655375a4692532e199.zip) into a separate directory. That snapshot includes the replay package and every analysis file pinned in `replay.json`. From the extracted snapshot root, give the Python 3.12 command a 30-second whole-process deadline. Use `python3` where that is the installed name.

```sh
python -B -X utf8 tests/evidence/windows-campaign-20261117/replay.py
```

The command checks the data and analysis digests, enforces the exact seeded order and row schema, and prints the recomputed JSON result. The hashes in the manifest use LF line endings. [Observed slots](slots.jsonl) · [Digests and expected result](replay.json)

The LF-normalized JSON output matched on Windows x64, macOS ARM64, and Linux ARM64. The common output SHA-256 was `13eaa51021681a16d2159752d4fd8e74a62b5e33153e647b200afdd062eec292`.

## Results reproduced

| Context | Guard fast median | Nuewframe fast median | Fast comparison |
| --- | ---: | ---: | --- |
| `NO_WINDOW` | 93.74325 ms | 86.7151 ms | FAIL |
| `DETACHED` | 95.2911 ms | 279.9394 ms | UNEVALUATED |

SysPrims had no eligible median in either fast context. The kill-latency arithmetic passed in both contexts with Nuewframe excluded because every measured kill slot was recorded with a survivor. Both floor controls disagreed beyond the 5% tolerance. The resource comparison evaluated 144 units, with 31 losses and 78 failing ranks. The replay pins the comparator used for these counts.

## Evidence scope

The rows retain slot identity, elapsed and resource values, exit codes, and recorded stream, survivor, and Job observations. All 1,940 slots recorded zero active processes at the final Job observation. The recorded harness termination flag was set on 443 slots.

The public scope is arithmetic and counts of saved labels. Independent validation of executable identity, process identity, output bytes, and Job custody requires the private originals. Those originals contain arguments, paths, process identifiers, and run-environment details. The original manifest records an incomplete campaign with 42 standing losses and zero freeze-gate failures. The corrected comparison above is tied to its analysis revision. The quiet-host campaign and speed acceptance remain open. [Archive availability](../README.md#withdrawn-private-evidence)
