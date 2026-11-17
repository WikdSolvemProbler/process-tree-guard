<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# reduced-run
<!-- ste-exceptions: reduced -->

## Contents

- [reduced-run](#reduced-run)

This folder holds the preserved reduced-run package. The package is a reduced campaign run, preserved as the standing record of its numbers. The public `run.json` and `report.txt` redact recorded home-directory names as `REDACTED`; numerical observations are unchanged. Exact unredacted originals are retained locally outside the public tree.

**Belongs here:** The five files of the package, and no other file.

**Does not belong here:** Changes to recorded measurements. Publication path redactions and the derived-report refresh documented below leave the numerical observations unchanged. A test recomputes the numbers from the recorded observations.

`run.json` is the manifest of the run. `report.txt` is the report of the run, in text form.

**Next:** The [suite manual](../../README.md) states what the reduced run proves and what it does not prove.

[↑ Back to Contents](#contents)

## Redacted publication copies

The retained JSONL files below are redacted publication copies, not new runs. Only the account component in literal Unix home paths was replaced. Pricing executable fingerprint paths use `REDACTED` to match the run manifest; other JSONL account paths retain `REDACTED_ACCOUNT`. Recorded executable paths are therefore publication placeholders. The exact original bytes are preserved privately; their hashes below identify those originals without publishing the account values.

Every parsed nonpath field, numerical measurement, result, failed attempt, record order and nonstring JSON lexeme is unchanged. Both files remain parseable JSONL. Original executable fingerprints remain measurements of the original artifacts; a redacted pathname does not identify a new executable or qualify a rerun.

| Publication copy | Original SHA-256 | Redacted SHA-256 | Account-path replacements | Affected records |
| --- | --- | --- | ---: | ---: |
| `observations.jsonl` | `659a6a745a7f798d3bfd837ac3717aea49b9ef8a7bfb2c19c683ad3902c310ab` | `003b9f23532eea9ebd8757ad3acc270c2e01e6655a441bdfacc555cd0382a32d` | 43 | 40 |
| `pricing-observations.jsonl` | `3336fa9896f56f95ca42bb2c712ed9dcfb2c4ff415811d30a79fd958cfd74310` | `f0be9da3184c4e9630cb640aa819c1d535ea4ea3729bea26e7af90f635c1fb60` | 57 | 18 |

The 18 pricing fingerprint placeholders were normalized after a private e16 source check: every original pricing fingerprint matched the original run fingerprint in path, digest, and size. This is a publication-path correction only. It leaves all numerical observations unchanged and does not supply the absent configuration anchor for the package's five other unverifiable checks. The private e16 Git pricing blob uses LF line endings and has SHA-256 `8c4d8c60f3549bf196306403407d5a5d2a8fa76c80f77b2c043f9184dd80c706`. Converting only its 18 line endings to CRLF reproduces exactly the earlier original hash `3336fa9896f56f95ca42bb2c712ed9dcfb2c4ff415811d30a79fd958cfd74310` in the table. Both byte forms parse to the same 18 records.

## Derived-report refresh, 2026-11-17

The report was rendered again from the unchanged manifest, 40 observations and 18 pricing records using the reporting correction in `f5c10f76`. After newline normalization, exactly eight content lines now name the arms whose latency medians are missing. The refreshed text uses LF line endings. Their UNEVALUATED verdicts, all numerical values, the incomplete status, 82 standing losses and zero freeze-gate FAIL entries are unchanged. No probe or workload was rerun to generate this report.

The [previous report at 946d404](https://github.com/WikdSolvemProbler/process-tree-guard/blob/946d404845a1c47c0b454e7896058f64a9ff1801/tests/evidence/reduced-run/report.txt) remains preserved in Git. The separately preserved Windows checkout used CRLF and has SHA-256 `b9e6ed475d8f8a7dbe9cbcc137030be096d8486f61af3a80dbf4fbb14a3eac4f`; normalizing only its line endings to LF yields SHA-256 `a95664b15928116d9c032be5beb06cdad6bd8641db7ef165a88de94b23789dcb`. The refreshed LF report has SHA-256 `fa14b15ad266134e1d4c6f7e055a11a28f11063d67c6b8f9e9b0a026f165740e`. The package still lacks its original configuration anchor and retains all five declared unverifiable checks; this refresh supplies no new provenance or competitive result.
