<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

# integrations/muse-code

## Contents

- [integrations/muse-code](#integrationsmuse-code)
- [What the host sends and accepts](#what-the-host-sends-and-accepts)
- [Where to place the files](#where-to-place-the-files)
- [Evidence tier](#evidence-tier)

This folder holds the wiring example for Meta Muse Code. The host uses the `pretooluse` vendor. The vendor string is exact: `pretooluse`.

**Belongs here:** The two hook files for this host.

**Does not belong here:** Examples for any other host. No adapter code is needed. The `pretooluse` adapter in `src/plugins/` serves this host unchanged.

Name the full program for the full role. Name the gate program for the gate role. Give the wiring a finite deadline.

Each file holds `schema_version` and one `PreToolUse` group. The group matches the shell tools of the host. The matcher uses the exact-list form of the host.

**Next:** The wiki [Hook Integrations](../../wiki/Hook-Integrations.md) page states the steps for each host.

[↑ Back to Contents](#contents)

## What the host sends and accepts

The facts below come from the public developer documentation of Meta Muse Code, version 1.4. The payload facts also come from a public report of a captured payload. A check vendor needs only the first four items.

- The host writes one JSON object on standard input. The object has `tool_name`, `tool_input` and `tool_use_id`. It also has `hook_event_name`, `session_id`, `cwd`, `transcript_path`, `model`, `permission_mode` and `turn_id`. The `shell` call carries `tool_input.command` as a string.
- The `exec_command` call is reported to carry the command as `tool_input.cmd`. The `pretooluse` vendor reads `command` first and `cmd` second, so it accepts both.
- Exit code 2 with text on standard error blocks the call. The host sends the text to the model as the reason.
- The host reads standard output only on exit code 0. A denial of the `pretooluse` vendor uses exit code 2 and writes the reason on standard error. The host therefore honours it.
- An allow of the `pretooluse` vendor is an empty JSON object and exit code 0. The host does not read a decision from it. The host then applies its own approval flow.
- The host can also answer `allow`, `ask` or `deny` in a `hookSpecificOutput` object, and can rewrite the input with `updatedInput`. An `allow` needs `updatedInput`, and the rewritten call still goes through approval. This folder does not use either form.
- Any exit code other than 0 and 2 is a failure, and the call continues. The guard uses exit code 2 for every denial.
- The host skips a hook for a payload over 256 KiB. The command then runs with no check. Keep commands under that size.
- The host runs a hook with a cleared environment. The guard builds every path from its own location and not from the environment, so this does not change a verdict.
- The host reads hooks when a session starts. Restart the host after a change.

[↑ Back to Contents](#contents)

## Where to place the files

Merge the `hooks` member of `hooks.json` into `~/.config/muse/settings.json`, or into `$XDG_CONFIG_HOME/muse/settings.json` when that variable is set. The settings file must hold `"schema_version": 1`. A file without it is rejected.

A project can use `.muse/hooks.json` instead. The host runs a project file only after you trust the folder. The host ignores top-level members of a project file other than `hooks`.

The installer writes `hooks/muse-code.json` with the paths of this machine when it finds `%USERPROFILE%\.config\muse` or `%XDG_CONFIG_HOME%\muse`. It does not edit the settings file.

The hook text in `command` and in `commandWindows` is the same. The host uses `commandWindows` on Windows.

A denial begins with `TIMEOUT_REQUIRED` and names the installed policy file. The agent can then send the command again in the envelope form.

[↑ Back to Contents](#contents)

## Evidence tier

The integration follows the documented protocol. It has not yet been run against Meta Muse Code. Two things stand verified here: the unit and battery tests of the `pretooluse` vendor, and one unit test that reads a payload with the field set above.

These facts rest on the documentation or on a report, and not on a run. The first is the Windows behavior of the host. The second is the name and payload of the `exec_command` and `powershell` tools. The third is a block on exit code 2 when the denial also writes JSON on standard output.

[↑ Back to Contents](#contents)
