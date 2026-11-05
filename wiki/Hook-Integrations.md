# Hook Integrations
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page guides hook integrations: an integrator wires a host to process-tree-guard. A vendor is one hook protocol. A host is a program that calls the guard through a hook. One vendor can serve two hosts. One host can use two vendors.

## Contents

- [The host and vendor table](#the-host-and-vendor-table)
- [What a check vendor allows](#what-a-check-vendor-allows)
- [Wire a host in four steps](#wire-a-host-in-four-steps)
- [Worked example: Claude Code](#worked-example-claude-code)
- [The payload cap](#the-payload-cap)
- [Fail closed](#fail-closed)
- [Gate and full role differences](#gate-and-full-role-differences)

## The host and vendor table

| Host | Vendor | Tool or event | Kind | Answer |
|---|---|---|---|---|
| Claude Code | `pretooluse` | Bash tool | Check | Allow gives an empty object. Deny gives exit code 2. |
| Codex | `pretooluse` | PreToolUse form | Check | The guard does not separate this host from Claude Code at the protocol level. |
| Claude Code | `powershell-auto` | PowerShell tool | Rewrite | The answer rewrites the call to the automatic wrapper, or denies. Exit code is always 0. |
| Cursor | `cursor` | Hook event | Check | Allow gives `{"permission":"allow"}`. Deny gives exit code 2. |
| Cursor | `cursor-context` | Context event | Context | The answer prints the installed policy file. |
| Antigravity | `antigravity` | `run_command` call | Rewrite | The answer rewrites the call to the automatic wrapper, or denies. |
| Antigravity | `antigravity-context` | Context event | Context | The answer prints the installed policy file. |

A kind predicts the code shape. Check reads a payload and answers allow or deny. Rewrite reads a payload and answers with a rewritten command or a denial. Context prints the installed policy and reads no payload.

Codex is a named place as a wiring example. No `integrations/codex/` folder exists at this version. The missing pieces are the wiring example, its tests, and a host fact from the host documentation. The `pretooluse` row and its tests serve Codex today.

[↑ Back to Contents](#contents)

## What a check vendor allows

A check vendor allows one form of command and denies every other command. The allowed form is the envelope:

```text
{{BIN}}/process-tree-guard.exe --timeout SECONDS --shell powershell --command-b64 BASE64
```

`{{BIN}}` is the folder of the full program. The command text is Base64 encoded UTF-8 text. A rewrite vendor needs no envelope. A rewrite vendor sends each command through the automatic copy. The automatic copy has a default deadline of 7200 seconds. The installed [POLICY.md](../POLICY.md) states these rules to the agent. [Policy File and Layout](Policy-File-and-Layout.md) states which envelope each role accepts.

[↑ Back to Contents](#contents)

## Wire a host in four steps

Step 1. Copy the example folder for your vendor from [integrations/](../integrations/README.md). The folders are [integrations/pretooluse/](../integrations/pretooluse/README.md), [integrations/cursor/](../integrations/cursor/README.md), [integrations/antigravity/](../integrations/antigravity/README.md), and [integrations/powershell-auto/](../integrations/powershell-auto/README.md).

Step 2. Place `hooks.json` in the host configuration home. Place `hooks-gate.json` in that home when you use the gate role.

Step 3. Name the full program in `hooks.json`. Name the gate in `hooks-gate.json`. Write the gate as `process-tree-guard-gate VENDOR`.

Step 4. Keep a finite deadline in the host wiring. The examples carry `"timeout": 10`. A wiring with no finite host deadline breaks the one promise of the guard.

[↑ Back to Contents](#contents)

## Worked example: Claude Code

This example wires the Bash tool of Claude Code to the full program. The file is [integrations/pretooluse/hooks.json](../integrations/pretooluse/hooks.json).

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "C:/Tools/process-tree-guard/bin/process-tree-guard.exe hook pretooluse",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

The gate form names `process-tree-guard-gate.exe pretooluse` in the `libexec` folder. The file is [integrations/pretooluse/hooks-gate.json](../integrations/pretooluse/hooks-gate.json). The PowerShell tool uses the `powershell-auto` vendor with the matcher `PowerShell`. The file is [integrations/powershell-auto/hooks.json](../integrations/powershell-auto/hooks.json).

A command in the envelope form passes the check. Any other Bash command gets a denial with exit code 2. The denial reason begins with `TIMEOUT_REQUIRED` and names the installed policy file. The agent can then send the command again in the envelope form.

[↑ Back to Contents](#contents)

## The payload cap

The guard reads a payload under a 16 MiB cap. The guard decides on the first 16 MiB. The guard never reads bytes past the cap. A payload whose deciding text is not complete inside the cap fails. A malformed payload fails closed.

[↑ Back to Contents](#contents)

## Fail closed

Fail closed means a doubt gives a denial, not an allow. A malformed payload fails. A payload with no valid envelope fails. A stream error in a hook denial still protects the host. The host reads a denial status as a block. [Security Model](Security-Model.md) states the denial rule in full.

[↑ Back to Contents](#contents)

## Gate and full role differences

The gate accepts a smaller envelope set than the full program. The gate rewrites to the automatic copy of the deadline role. The full program rewrites to the automatic copy of its own name. The gate never accepts an envelope that names the gate. The gate never runs a command. [Policy File and Layout](Policy-File-and-Layout.md) states the envelope rules for each role.

[↑ Back to Contents](#contents)
