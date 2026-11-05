# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Shared plumbing for the battery's per-case Windows drivers.

A driver executes its case's charged runs and returns, per
behavioural condition, "evidenced" or "refuted" decided from the
observed output alone. Every command a driver runs is preserved as
an evidence record (the command line, its exit code, and its raw
stdout/stderr) in the JSON the driver prints and, when --out is
given, in the evidence file it writes under the run's results
folder.

Modes:
- windows: the real charged runs. Only valid on win32; on any other
  platform the driver reports its conditions as blocked, never as
  evidenced.
- stub: the decision logic exercised against a stub probe program
  that emits observations in the same schema the Windows probe
  emits. The stub tier proves the decision logic and its negative
  controls on any platform; it evidences nothing about Windows.

Exit code: 0 when every condition is evidenced, 1 when any is
refuted, 2 when any is blocked/error.
"""

import argparse
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BATTERY_DIR = os.path.dirname(HERE)
SUITE_DIR = os.path.dirname(BATTERY_DIR)
DEFAULT_TREE = os.path.normpath(os.path.join(SUITE_DIR, ".."))
DEFAULT_BUILD = os.path.join(SUITE_DIR, "build")

COMMAND_TIMEOUT_S = 300
BUILD_TIMEOUT_S = 1800


def run_command(argv, timeout_s=COMMAND_TIMEOUT_S, cwd=None, env=None):
    """Run one command under a finite deadline; return its evidence
    record."""
    try:
        proc = subprocess.run(argv, capture_output=True, text=True,
                              timeout=timeout_s, cwd=cwd, env=env)
        return {"command": argv, "exit_code": proc.returncode,
                "stdout": proc.stdout, "stderr": proc.stderr}
    except subprocess.TimeoutExpired as exc:
        return {"command": argv, "exit_code": None,
                "stdout": exc.stdout.decode(errors="replace")
                if isinstance(exc.stdout, bytes) else (exc.stdout or ""),
                "stderr": f"deadline of {timeout_s} s expired"}
    except OSError as exc:
        return {"command": argv, "exit_code": None, "stdout": "",
                "stderr": f"spawn failed: {exc}"}


def run_probe(script, extra_args, timeout_s=COMMAND_TIMEOUT_S):
    """Run a probe program and parse the observation JSON on its
    stdout. Returns (observations or None, evidence record)."""
    record = run_command([sys.executable,
                          os.path.join(HERE, script)] + extra_args,
                         timeout_s=timeout_s)
    observations = None
    if record["exit_code"] == 0 and record["stdout"].strip():
        try:
            observations = json.loads(record["stdout"])
        except json.JSONDecodeError:
            observations = None
    return observations, record


def main(case_ident, decide, collect_windows, collect_stub):
    parser = argparse.ArgumentParser(
        description=f"battery case ({case_ident}) driver")
    parser.add_argument("--mode", choices=("windows", "stub"),
                        default="windows")
    parser.add_argument("--variant", default="good",
                        help="stub variant: 'good', 'broken', or a "
                             "named broken variant (broken-zero, "
                             "broken-pending, ...) refuting one "
                             "charged condition")
    parser.add_argument("--tree", default=DEFAULT_TREE)
    parser.add_argument("--build-dir", default=DEFAULT_BUILD)
    parser.add_argument("--out", default=None)
    args = parser.parse_args()

    records = []
    if args.mode == "windows" and sys.platform != "win32":
        payload = {"case": case_ident, "tier": "windows",
                   "blocked": "driver requires Windows; "
                              f"not run on {sys.platform}",
                   "conditions": {}, "records": records}
    else:
        collect = collect_windows if args.mode == "windows" else collect_stub
        observations, probe_records = collect(args)
        records.extend(probe_records)
        if observations is None:
            payload = {"case": case_ident, "tier": args.mode,
                       "blocked": "probe produced no observations",
                       "conditions": {}, "records": records}
        else:
            conditions = {}
            for name, (status, detail) in decide(observations).items():
                conditions[name] = {"status": status, "detail": detail}
            payload = {"case": case_ident, "tier": args.mode,
                       "variant": args.variant,
                       "observations": observations,
                       "conditions": conditions, "records": records}
    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)),
                    exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2)
    print(json.dumps(payload, indent=2))
    statuses = [c["status"] for c in payload["conditions"].values()]
    if "blocked" in payload or not statuses:
        return 2
    # Any status that is neither "evidenced" nor "refuted" — an
    # "error" above all — is an execution outcome, not a verdict:
    # the exit code must say 2 for it (the contract above), never
    # fall through to 0, which claims every condition evidenced.
    if any(s not in ("evidenced", "refuted") for s in statuses):
        return 2
    if any(s == "refuted" for s in statuses):
        return 1
    return 0


def stub_collect(case_ident):
    """The stub-tier collector: run the stub probe, which emits the
    case's observations (good or deliberately broken variant)."""
    def collect(args):
        observations, record = run_probe(
            "stub_probe.py", [case_ident, args.variant])
        return observations, [record]
    return collect
