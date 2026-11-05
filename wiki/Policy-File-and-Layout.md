# Policy File and Layout
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page teaches the policy file and layout of process-tree-guard: the policy file and the two install layouts. The policy file is [POLICY.md](../POLICY.md). The program reads the policy file at run time. The policy file is product data, not documentation.

## Contents

- [Where a program looks for its files](#where-a-program-looks-for-its-files)
- [The two layouts](#the-two-layouts)
- [The BIN placeholder](#the-bin-placeholder)
- [Envelope rules by role](#envelope-rules-by-role)
- [The policy file as product data](#the-policy-file-as-product-data)

## Where a program looks for its files

A program finds its own files from its own location. The program uses `std::env::current_exe` for that location. The program never uses `PATH` for its own files. The program never uses the current folder for its own files. A planted file in `PATH` or in the current folder is never a file of the guard.

The command that the guard runs is a different case. The guard finds that command with the system search for a command. That search can use the current folder. Name one exact program with a full path when you need one exact program.

[↑ Back to Contents](#contents)

## The two layouts

In the install layout, the full program stands in `bin`. The roles stand in `libexec`. `POLICY.md` stands at the root, one folder above each program folder.

```text
<root>/bin/process-tree-guard.exe
<root>/bin/process-tree-guard-auto.exe
<root>/libexec/process-tree-guard-deadline.exe
<root>/libexec/process-tree-guard-deadline-auto.exe
<root>/libexec/process-tree-guard-gate.exe
<root>/libexec/process-tree-guard-contain.exe
<root>/POLICY.md
```

In the flat layout, every program stands in one folder. `POLICY.md` stands one folder above that folder. Both layouts work. Tests cover both layouts.

[↑ Back to Contents](#contents)

## The BIN placeholder

The policy file names its programs with the placeholder `{{BIN}}`. In the install layout, `{{BIN}}` is the `bin` folder. That folder holds `process-tree-guard.exe` and `process-tree-guard-auto.exe`. `{{BIN}}` is not `libexec`. The `libexec` folder holds the roles.

An install with the gate as its hook and no full program in `bin` uses a stated variant. That install sets `{{BIN}}` to the `libexec` folder. That install names the deadline role and its automatic copy in place of the two full names.

[↑ Back to Contents](#contents)

## Envelope rules by role

An envelope is a command line that already names the guard in the valid form. The full program accepts an envelope that names the full program in the same folder. The full program also accepts the earlier name `process-guard.exe` in the same folder.

The gate accepts an envelope that names a deadline program in the gate folder. The valid names are `process-tree-guard.exe`, `process-guard.exe`, and `process-tree-guard-deadline.exe`. When the gate folder is `libexec`, the gate also accepts those names in the `bin` folder beside it. The gate never accepts the gate itself. A role keeps its rules under any file name.

[↑ Back to Contents](#contents)

## The policy file as product data

Hook answers print the installed policy file for the context vendors. Edit `POLICY.md` only as product data. Do not treat the policy file as a documentation page. The style rules of the documentation set do not apply to the policy file. That split is a design rule, not an error.

[↑ Back to Contents](#contents)
