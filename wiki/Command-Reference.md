# Command Reference
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page is the complete command reference for process-tree-guard. Every command form, option, subcommand, and exit code stands in this page. The page text comes from the help texts in the source.

## Contents

- [Forms at a glance](#forms-at-a-glance)
- [Options](#options)
- [Subcommands](#subcommands)
- [Exit codes](#exit-codes)
- [Diagnostics](#diagnostics)
- [Full option tables by role](#full-option-tables-by-role)

## Forms at a glance

| Form | Program |
|---|---|
| `--timeout SECONDS [--] COMMAND [ARGS...]` | Full, deadline, contain |
| `--timeout SECONDS --shell {powershell,bash,cmd} --command-b64 BASE64` | Full, deadline, contain |
| `--extendable [--max-extension SECONDS] [--extension-log PATH] [--authenticate {desktop,console}] --timeout ...` | Full, deadline |
| `extend SECONDS` | Full, deadline |
| `hook [VENDOR]` | Full only |
| `auto [--timeout SECONDS] [--shell ...] [--command-b64 BASE64 \| [--] COMMAND...]` | Full, deadline |
| `[VENDOR]` | Gate only. The default vendor is `pretooluse`. |

`SECONDS` is a finite number. `SECONDS` is greater than zero. `SECONDS` is at most 86400.

`ptg` is the short name of the full program: every form above that runs in the full program runs the same way under `ptg.exe`, and `ptg-auto.exe` is the automatic copy of `ptg.exe`. The examples on this page use `process-tree-guard`.

[↑ Back to Contents](#contents)

## Options

| Option | Meaning |
|---|---|
| `--timeout SECONDS` | Set the deadline for the run. This option is required for a direct run. |
| `--shell {powershell,bash,cmd}` | Run the command text in the named shell. Use this option with `--command-b64`. |
| `--command-b64 BASE64` | Give the command text as Base64 encoded UTF-8 text. |
| `--extendable` | Start an extendable run. [Extendable Runs](Extendable-Runs.md) states the rules. |
| `--max-extension SECONDS` | Set the cap for one hold. The default cap is 21600 seconds. The largest cap is 86400 seconds. This option needs `--extendable`. |
| `--extension-log PATH` | Name the audit file for an extendable run. This option needs `--extendable`. |
| `--authenticate {desktop,console}` | Choose the approval method for a cap over the default. This option needs `--extendable`. |
| `-h`, `--help` | Print the usage text and exit with code 0. The gate prints its usage without a payload read. |

The extension options without `--extendable` fail with a guard error. A cap over 21600 needs user approval before the run starts. [Extendable Runs](Extendable-Runs.md) states the approval rules.

[↑ Back to Contents](#contents)

## Subcommands

`extend SECONDS` asks the enclosing extendable run for a hold. The hold lasts `SECONDS` from now. The cap of the run bounds the hold.

`hook [VENDOR]` runs a hook form of the full program. The default vendor is `pretooluse`. [Hook Integrations](Hook-Integrations.md) lists every vendor.

`auto` runs automatic mode. A file name that ends in `-auto` also selects automatic mode.

[↑ Back to Contents](#contents)

## Exit codes

| Code | Meaning |
|---|---|
| The command code | The command ended. The guard passes the code through. |
| 0 | Success. A hook allow answer also uses code 0 for the rewrite vendors. |
| 2 | A hook denial. The check family denies with code 2. The guard uses code 2 only in hook forms, because code 2 blocks a host hook event. |
| 124 | The deadline passed. The guard ended the whole tree. |
| 125 | A guard error ended the run, or the owner exited first, or an approval failed. |

A parse error gives exit code 125. A parse error never gives code 2 outside a hook form.

[↑ Back to Contents](#contents)

## Diagnostics

The notices `PROCESS_TIMEOUT` and `PROCESS_OWNER_EXITED` report a deadline end and an owner end. A program run writes each notice to standard error after the tree ends. A library call returns the notices as owned records. Call `take_diagnostics` after a library call to get the records. The record queue holds at most 1 MiB or 256 records. The queue adds a truncation marker at that limit.

[↑ Back to Contents](#contents)

## Full option tables by role

Open one dropdown per role for the full table of that role. The summary table above stays open.

<details>
<summary>The full program accepts every form in this table.</summary>

| Form | Notes |
|---|---|
| `--timeout` run | Direct form and shell form. |
| `--extendable` options | `--max-extension`, `--extension-log`, `--authenticate`. |
| `extend SECONDS` | Asks the enclosing run for a hold. |
| `hook [VENDOR]` | Vendors: `pretooluse`, `cursor`, `cursor-context`, `antigravity`, `antigravity-context`, `powershell-auto`. |
| `auto` | Subcommand form or `-auto` file name form. |

</details>

<details>
<summary>The deadline role accepts the deadline forms in this table.</summary>

| Form | Notes |
|---|---|
| `--timeout` run | Direct form and shell form. |
| `--extendable` options | `--max-extension`, `--extension-log`, `--authenticate`. |
| `extend SECONDS` | Asks the enclosing run for a hold. |
| `auto` | Subcommand form or `-auto` file name form. |
| `hook` | Not a form of this role. The role reads `hook` as a program name and refuses it with 125. |

</details>

<details>
<summary>The gate accepts only a vendor in this table.</summary>

| Form | Notes |
|---|---|
| `[VENDOR]` | The default vendor is `pretooluse`. |
| `-h`, `--help` as first argument | Prints usage. Exits with code 0. Reads no payload. |
| Any other argument | Refuses with usage text on standard error and exit code 125. Reads no payload. |

</details>

<details>
<summary>The contain role accepts only the request form in this table.</summary>

| Form | Notes |
|---|---|
| `--timeout SECONDS` run | Runs only inside an extendable run. The value of that flag is a request to that run. |
| No enclosing run | Exits with code 125. Runs nothing. |
| Extension options, `auto`, `hook` | Not forms of this role. The role refuses them. |

</details>

[↑ Back to Contents](#contents)
