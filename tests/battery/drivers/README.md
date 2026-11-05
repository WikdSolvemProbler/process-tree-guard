<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# drivers
<!-- ste-exceptions: drivers, probe -->

## Contents

- [drivers](#drivers)

This folder holds the driver of each battery case. A driver runs its case. For each condition of the case, the driver states if the condition stands or does not stand.

**Belongs here:** One driver file per case, the Windows probe of each case, and the shared driver code.

**Does not belong here:** The case set itself. The case set lives in `battery.py`, one folder up.

`driver_common.py` holds the code that all drivers share.

`stub_probe.py` is the probe that the drivers use when no Windows probe can run. This form proves the driver logic on any system. It proves nothing about Windows.

**Next:** The [battery folder](../README.md) states the battery as a whole.

[↑ Back to Contents](#contents)
