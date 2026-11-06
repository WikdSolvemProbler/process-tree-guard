<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# benchmark

## Contents

- [benchmark](#benchmark)

This folder holds the preserved competitor set of the suite. The campaign runs against these competitor programs as found.

**Belongs here:** The preserved set, and the results record of the benchmark.

**Does not belong here:** Run output of a campaign. A run writes its output in its own run folder, not here.

`results-record.json` states the outcomes, the SHA record, and the competitor programs of the benchmark.

## Windows capture, 6 November 2026

The campaign recorded all 1,940 planned slots in 56 minutes 53 seconds. The seeded order, 1,680 measured rows, and 1,940 successful slot Job drain receipts verified. The harness reported an incomplete certification with 44 standing losses. The startup comparison was unevaluated under its full eligibility rules, and resource and scaling gates failed. Manual usability remained unevaluated. Host CPU before launch was 22.7–27.8%, so this capture does not establish a quiet-host speed record.

The guard's measured fast-path medians were 65.0 ms in `NO_WINDOW` and 74.3 ms in `DETACHED`. ProcessKit measured 86.9 ms and 546.4 ms, and nuewframe measured 69.9 ms and 507.3 ms. These are whole-slot medians under the observed host conditions. SysPrims' default stderr did not match the relay workload. The guard left no observed survivors in 204 measured kill and scaling slots. Nuewframe left survivors in all 204 of its corresponding slots. The harness recorded these observations before draining the slot Jobs.

The archive contains the original report, all observations and ProcessKit event logs, the audit and host telemetry, the exact configuration, and both earlier stopped attempts. The capture process exited 1 after finalizing the package because its redirected Windows console could not encode a report character. A later display-only fix replayed the report under that code page without modifying the package. The archive index binds each file by SHA-256. [Download the evidence](../evidence/windows-campaign-20261106.zip)

Archive SHA-256 `ef653c4fb2921adc9e466f767225ba257b26e0b9adecf354b8000f30ccf751bd`.

**Next:** The [competitors-001 folder](competitors-001/README.md) holds the set itself.

[↑ Back to Contents](#contents)
