<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# Interrupted Windows campaign: reproducible prefix

This package preserves all 1,881 completed observations from the fixed 1,940-slot campaign: 36 controls, 224 warmups and 1,621 measured observations. `missing.json` contains the exact 59 remaining saved-order keys. A missing completed observation does not establish whether its slot started.

The original run reached its 3,600-second process-tree deadline. Its state remains **incomplete and provisional**, with empty recorded fingerprint/version maps, null final standing-loss/freeze counts and no pricing observations. The unchanged canonical checker returns **REFUSED** for three identity findings over those unfinished maps. Their exact messages remain in `replay.json`; they do not establish that staged executables were physically absent.

`slots.jsonl` retains all 17 allowlisted fields and the three Job subfields, including failures and nulls. Values and order were compared against the retained originals using exact decimal parsing. `replay.py` verifies source/configuration/data identities, the complete prefix and missing suffix, and reproduces the descriptive counts, floor checks, elapsed summaries and resource summaries in the manifest. Both floor controls fail the specified five-percent agreement rule. Floor-distance claims remain null.

## Replay

Export [source commit a95f32b8](https://github.com/WikdSolvemProbler/process-tree-guard/tree/a95f32b8dea5de48e82de5a00c9c78653230586b) into a separate directory. Supply that directory as `--source-root`; the replay verifies its pinned analysis files and configuration. The source snapshot and this package belong to the same repository history.

From the repository root on Windows, with the guard and Python available:

```powershell
process-tree-guard --timeout 60 -- python tests/evidence/windows-campaign-prefix-20261119/replay.py --source-root C:/path/to/a95-source
```

For a data-only replay on a Unix host with GNU timeout:

```sh
timeout --signal=KILL --kill-after=2s 60s python3 tests/evidence/windows-campaign-prefix-20261119/replay.py --source-root /path/to/a95-source
```

The replay launches no product, build, probe or remote operation. Each invocation needs a finite process-tree deadline. The retained raw data and the complete 1,940-slot acceptance rules are unchanged.

## Interpretation and privacy

These are descriptions of an interrupted prefix. They establish no complete-campaign ranking, winner, equivalence, certificate or acceptance result. Each cell reports its actual and planned sample counts. Elapsed summaries use the original rules for eligible `ok` observations; failures remain in the rows and outcome counts. CPU and peak working set describe the measured arm process, not its whole tree.

All recorded final Job counts are zero, while 430 rows also record a harness termination flag. Those observations remain separate and do not certify unassisted guard cleanup.

The projection omits argv, raw process/HANDLE identities, output bytes, private paths, host details, fine timestamps and free-form diagnostics. Exact originals remain private with their hashes recorded in the manifest. Public replay verifies the projected arithmetic; it cannot independently establish the omitted executable, stream or process-custody facts. The comprehensive benchmark and public full-run qualification remain open.
