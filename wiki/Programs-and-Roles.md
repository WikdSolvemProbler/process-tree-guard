# Programs and Roles
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page teaches the programs and roles of process-tree-guard: the four programs and the role of each program. A role is one of the four program forms: full, deadline, gate, and contain.

## Contents

- [The four programs](#the-four-programs)
- [Choose a program](#choose-a-program)
- [The gate takes a vendor directly](#the-gate-takes-a-vendor-directly)
- [The automatic copy and the -auto rule](#the-automatic-copy-and-the--auto-rule)
- [Library entry points](#library-entry-points)

## The four programs

| Program | Role | Job |
|---|---|---|
| `process-tree-guard` | Full | Every form runs in this program. |
| `process-tree-guard-deadline` | Deadline | The deadline form, the `extend` subcommand, and automatic mode. |
| `process-tree-guard-gate` | Gate | The hook form. The gate answers a payload and never runs a command. |
| `process-tree-guard-contain` | Contain | A run inside an extendable run. The contain role asks that run for its deadline. |

Install the full program when you need one program. Install a role when a host needs only that role.

`ptg` is the short name of the full program. A copy of `process-tree-guard.exe` named `ptg.exe` has the full role and behaves exactly as `process-tree-guard.exe`. The role belongs to the program itself and the file name only selects automatic mode, so the copy needs no other change.

[↑ Back to Contents](#contents)

## Choose a program

Choose the full program for direct use. Choose the deadline role when a host needs only a deadline. Choose the gate when a host needs only the hook answer. Choose the contain role for a command inside an extendable run.

The deadline role has no hook form. The deadline role reads `hook` as a program name. The deadline role refuses that name with exit code 125.

[↑ Back to Contents](#contents)

## The gate takes a vendor directly

Write the gate as `process-tree-guard-gate VENDOR`. Do not write `process-tree-guard-gate hook VENDOR`. Only the full program has the `hook` subcommand. The gate reads a leading `hook` as a vendor name. That name is not a vendor. The gate refuses it with exit code 125.

[↑ Back to Contents](#contents)

## The automatic copy and the -auto rule

Copy a program to a name that ends in `-auto`. That copy selects automatic mode in the full program and in the deadline role. The gate has no automatic mode of its own. The gate rewrites to the automatic copy of the deadline role. The contain role has no automatic mode under any name.

The earlier names `process-guard` and `process-guard-auto` are accepted as aliases. The short names `ptg` and `ptg-auto` are the names of the full program in the same way: `ptg-auto.exe` behaves exactly as `process-tree-guard-auto.exe`, and a hook installed as `ptg.exe` rewrites to the `ptg-auto.exe` beside it. A role keeps its rules under any file name. The gate keeps the gate rules when a user copies it to another name.

[↑ Back to Contents](#contents)

## Library entry points

The library is `process_tree_guard`. The library is not on crates.io. Use the library as a path dependency or a git dependency.

| Entry point | Job |
|---|---|
| `run` | Run a program and its arguments under a deadline. |
| `run_shell` | Run Base64 encoded command text in a shell. |
| `run_auto` | Run automatic mode. |
| `run_hook` | Run a hook form of the full program. |
| `extend` | Ask the enclosing extendable run for a hold. |
| `main_full`, `main_deadline`, `main_gate`, `main_contain` | The entry point of each program. Each returns the exit code. |

`GuardError` is the error type. `MAX_SECONDS` is 86400. `DEFAULT_MAX_EXTENSION` is 21600. `CHANNEL_VARIABLE` is `PROCESS_TREE_GUARD_CHANNEL`.

[↑ Back to Contents](#contents)
