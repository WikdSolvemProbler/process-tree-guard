<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/sys

## Contents

- [src/sys](#srcsys)

This folder is the platform axis of the guard. It holds one backend folder for each operating system name.

**Belongs here:** The selection module and one folder per backend or named place.

**Does not belong here:** Platform declarations outside a backend. A declaration outside its backend is a defect.

`mod.rs` selects the backend and refuses a target with no proven backend. It declares nothing.

**Next:** The [windows folder](windows/README.md) states the one proven backend.

[↑ Back to Contents](#contents)
