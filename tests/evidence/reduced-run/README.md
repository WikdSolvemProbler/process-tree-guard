<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# reduced-run
<!-- ste-exceptions: reduced -->

## Contents

- [reduced-run](#reduced-run)

This folder holds the preserved reduced-run package. The package is a reduced campaign run, preserved as the standing record of its numbers. The public `run.json` and `report.txt` redact recorded home-directory names as `REDACTED`; numerical observations are unchanged. Exact unredacted originals are retained locally outside the public tree.

**Belongs here:** The five files of the package, and no other file.

**Does not belong here:** Changes to recorded measurements. The only publication edit replaces private home-directory names. A test recomputes the numbers from the recorded observations.

`run.json` is the manifest of the run. `report.txt` is the report of the run, in text form.

**Next:** The [suite manual](../../README.md) states what the reduced run proves and what it does not prove.

[↑ Back to Contents](#contents)

## Redacted publication copies

The retained JSONL files below are redacted publication copies, not new runs. Only the account component in literal Unix home paths was replaced with `REDACTED_ACCOUNT`. Recorded executable paths are therefore publication placeholders. The exact original bytes are preserved privately; their hashes below identify those originals without publishing the account values.

Every parsed nonpath field, numerical measurement, result, failed attempt, record order and nonstring JSON lexeme is unchanged. Both files remain parseable JSONL. Original executable fingerprints remain measurements of the original artifacts; a redacted pathname does not identify a new executable or qualify a rerun.

| Publication copy | Original SHA-256 | Redacted SHA-256 | Account-path replacements | Affected records |
| --- | --- | --- | ---: | ---: |
| `observations.jsonl` | `659a6a745a7f798d3bfd837ac3717aea49b9ef8a7bfb2c19c683ad3902c310ab` | `003b9f23532eea9ebd8757ad3acc270c2e01e6655a441bdfacc555cd0382a32d` | 43 | 40 |
| `pricing-observations.jsonl` | `3336fa9896f56f95ca42bb2c712ed9dcfb2c4ff415811d30a79fd958cfd74310` | `fc3e0d5e87a2caf0991fdafb4e6b6a028bfda41de4bea0682af5a249ebd87e1e` | 57 | 18 |
