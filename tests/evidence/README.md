<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# evidence

## Contents

- [evidence](#evidence)

This folder holds the recorded evidence of the suite. A test holds each record here to a new run or a new count.

**Belongs here:** The recorded battery record, the summary of the scanner set, the recorded test run, and the preserved `reduced-run` package.

**Does not belong here:** New run results. A new run writes its results in its own run folder.

## Windows correctness reruns

The full Windows x64 suite at b774403 recorded 643 passes and four skips. Both earlier full runs are included. They each reported one infrastructure failure after a Python scale fixture reached its deadline before publishing the complete process list. The final run verified the staged binaries before and after testing. [Raw logs and 647 test certificates](windows-correctness-20261106.zip)

Archive SHA-256 b6254fc68a59ac7d53aa6d4a906e1cc3cf34c72c4ef36cad176bc7358998cc55.

## Linux portable correctness

The exact 47d8f47 archive completed the portable suite on the Tailscale Linux ARM64 guest with 593 passes and 54 skips. The first run reached its guest deadline and recorded a sleeping-launcher test failure before the final traceback. The isolated unchanged test and the full rerun passed. The archive preserves both attempts and independent guest cleanup checks. [Linux correctness evidence](linux-correctness-20261106.zip)

**Next:** The [`reduced-run` folder](reduced-run/README.md) holds the preserved package.

[↑ Back to Contents](#contents)
