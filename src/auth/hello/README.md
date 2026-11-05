<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/auth/hello

## Contents

- [src/auth/hello](#srcauthhello)

This folder holds the operation part of the Windows Hello approval. The parent file `src/auth/hello.rs` holds the rest of the Hello module.

**Belongs here:** The one operation that asks for consent and reads the answer.

**Does not belong here:** Approval policy. The auth group owns the policy around this operation.

`text.rs` sits beside this folder in the auth group. It is a pure file and holds the Hello texts.

**Next:** The [auth README](../README.md) states the approval layout.

[↑ Back to Contents](#contents)
