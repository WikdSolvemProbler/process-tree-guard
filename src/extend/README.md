<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/extend

## Contents

- [src/extend](#srcextend)

This folder holds the parts of the extend module that stand as their own files. The extend mechanism grants holds that extend a run past its first deadline, under a cap.

**Belongs here:** The client, the codec, the descriptor, and the server side of one hold request.

**Does not belong here:** The log form and the run state. Each has its own subfolder here.

`codec.rs` is a pure file. The criterion statement in `src/lib.rs` names the pure set.

**Next:** The wiki Extendable Runs page teaches the hold concept end to end.

[↑ Back to Contents](#contents)
