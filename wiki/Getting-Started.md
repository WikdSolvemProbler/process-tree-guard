# Getting Started
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page guides a new user who is getting started with process-tree-guard to a first guarded run. The instructions use the Windows backend. Windows is the proven tier for these instructions.

## Contents

- [Get the programs](#get-the-programs)
- [Verify the checksum](#verify-the-checksum)
- [Place the programs](#place-the-programs)
- [Run a first command](#run-a-first-command)
- [Watch the deadline end a tree](#watch-the-deadline-end-a-tree)
- [Run shell text](#run-shell-text)
- [Use automatic mode](#use-automatic-mode)
- [Read exit codes 124 and 125](#read-exit-codes-124-and-125)
- [Next steps](#next-steps)

## Get the programs

Get a release for x86-64 Windows. A release holds four programs and one `SHA256SUMS` file. The four programs are `process-tree-guard.exe`, `process-tree-guard-deadline.exe`, `process-tree-guard-gate.exe`, and `process-tree-guard-contain.exe`. The release also holds `process-guard.exe`. That file is a copy of `process-tree-guard.exe` under the earlier name. A release also holds `LICENSE`, `NOTICE`, `README.md`, and `POLICY.md`.

Or build the programs from source. [Building From Source](Building-From-Source.md) gives the source build.

[↑ Back to Contents](#contents)

## Verify the checksum

Verify each program before you use it. Run this command in PowerShell:

```powershell
(Get-FileHash .\process-tree-guard.exe -Algorithm SHA256).Hash
```

Compare the result with the line for that file in `SHA256SUMS`. Do not use a program whose hash differs.

[↑ Back to Contents](#contents)

## Place the programs

Place the programs in one of the two layouts. [Policy File and Layout](Policy-File-and-Layout.md) states both layouts. Copy the full program to a name that ends in `-auto`. That copy is the automatic copy. A hook rewrite names that copy.

[↑ Back to Contents](#contents)

## Run a first command

Run one command under a deadline of 60 seconds:

```
process-tree-guard --timeout 60 -- my-command arg1 arg2
```

The guard passes standard input, standard output, and standard error through unchanged. The guard passes the exit code of the command through unchanged. The guard ends the whole tree at the deadline.

[↑ Back to Contents](#contents)

## Watch the deadline end a tree

This example starts a background `ping` and a foreground `ping`. Each `ping` runs for about ten minutes. The deadline is 5 seconds.

```powershell
process-tree-guard --timeout 5 -- cmd /c "start /b ping -n 600 127.0.0.1 >nul & ping -n 600 127.0.0.1 >nul"
$LASTEXITCODE
```

The expected result follows. After 5 seconds, the guard writes the notice `PROCESS_TIMEOUT` to standard error. Both `ping` processes end. `$LASTEXITCODE` is `124`. A timeout that ends only `cmd` leaves the background `ping` alive. The job object ends both.

[↑ Back to Contents](#contents)

## Run shell text

Give shell text as Base64 encoded 8-bit Unicode Transformation Format (UTF-8) text. Name the shell with `--shell`. The shells are `powershell`, `bash`, and `cmd`.

```powershell
$text = 'npm ci; npm test'
$b64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($text))
process-tree-guard --timeout 600 --shell powershell --command-b64 $b64
```

The Base64 form keeps quotes and special characters out of the reach of the calling shell. A hook host that can only allow or deny asks for exactly this form. [Hook Integrations](Hook-Integrations.md) states that rule.

[↑ Back to Contents](#contents)

## Use automatic mode

The automatic copy runs a command with a default deadline. The default is 7200 seconds. The variable `PROCESS_TREE_GUARD_AUTO_TIMEOUT` sets a different default.

```powershell
process-tree-guard-auto -- cargo build --release
process-tree-guard auto --timeout 900 -- cargo test
```

The first line uses the `-auto` file name. The second line uses the `auto` subcommand with an explicit deadline. A rewrite hook sends each command of an agent through the automatic copy.

[↑ Back to Contents](#contents)

## Read exit codes 124 and 125

Exit code 124 means the deadline passed. The guard ended the tree at the deadline. Exit code 125 means a guard error ended the run. A run also ends with 125 when the owner exits first. An owner is a process whose exit ends a run. [Command Reference](Command-Reference.md) lists every exit code.

[↑ Back to Contents](#contents)

## Next steps

Read [Programs and Roles](Programs-and-Roles.md) to choose a program. Read [Extendable Runs](Extendable-Runs.md) when work outlives one deadline. Read [Guarantees and Limits](Guarantees-and-Limits.md) before you trust the guard.

[↑ Back to Contents](#contents)
