<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# battery

## Contents

- [battery](#battery)

This folder holds the battery of the suite. The battery holds five cases. Each case states one design question and ends in PASS or FAIL on a Windows run.

**Belongs here:** The case runner and the environment fixture code of the battery.

**Does not belong here:** Campaign code. Campaign code lives in `campaign/`.

`battery.py` runs the five cases and records the verdict of each case. On a system that is not Windows, each case stands at BLOCKED-WINDOWS.

**Next:** The [driver files](drivers/README.md) hold the driver of each case.

[↑ Back to Contents](#contents)
