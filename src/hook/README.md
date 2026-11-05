<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/hook

## Contents

- [src/hook](#srchook)

This folder holds the core hook mechanism. A hook run reads one payload, decides, and prints one decision.

**Belongs here:** The envelope check, the payload scanner, the JSON reader, and the path rules.

**Does not belong here:** Vendor rules. One vendor is one adapter in `src/plugins/`.

`json.rs` and `scanner.rs` are pure files. The scanner reads a payload as a stream and never builds a second form of it.

**Next:** The [plugins README](../plugins/README.md) states the vendor side of a hook run.

[↑ Back to Contents](#contents)
