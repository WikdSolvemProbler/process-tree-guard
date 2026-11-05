<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# src/cli

## Contents

- [src/cli](#srccli)

This folder holds the parts of the command-line module that stand as their own files. The parent file `src/cli.rs` holds the rest of the module.

**Belongs here:** The automatic-mode form and the help text of the command line.

**Does not belong here:** Argument rules that the parent module owns.

`help.rs` holds every help text. The wiki Command Reference page is written from these texts.

`auto.rs` holds the automatic-mode command form.

**Next:** The [src README](../README.md) states the source layout.

[↑ Back to Contents](#contents)
