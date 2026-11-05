<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/plugins

## Contents

- [src/plugins](#srcplugins)

This folder holds one adapter for each hook vendor family. An adapter speaks one vendor protocol to the core hook mechanism.

**Belongs here:** One adapter file per vendor family, and the shared verdict form.

**Does not belong here:** The core hook mechanism. The mechanism lives in `src/hook/`.

`antigravity.rs` speaks the antigravity protocol.

`powershell_auto.rs` speaks the powershell-auto protocol.

`verdict.rs` holds the verdict form that every adapter returns.

**Next:** The [integrations README](../../integrations/README.md) states the wiring side of each vendor.

[↑ Back to Contents](#contents)
