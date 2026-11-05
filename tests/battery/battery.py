# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""The Windows battery for the five theory items the earlier phases
deferred. Each case is executable: it carries its recorded adoption
conditions as its pass criteria, runs every part that can run on the
present platform, and reports the rest as BLOCKED-WINDOWS with the
exact missing capability named. A case never passes on the strength
of its runnable parts alone: adoption requires every condition.

Two kinds of condition:

- Source audits, which run anywhere. Every audit is anchored to a
  function-scoped extraction: the needle must occur inside the
  named function's body, and the audit names the file, the function
  and the line of its site. A needle that merely occurs somewhere
  in the file evidences nothing.
- Behavioural conditions, which only a driver run can evidence.
  Each case has a driver in battery/drivers/ (case_a.py ...
  case_o.py) that executes the charged runs on Windows, decides
  "evidenced"/"refuted" per condition from the observed output,
  and preserves an evidence record (commands, exit codes, raw
  outputs) for every command it runs. run.ps1 invokes this battery
  as part of the run, with --results-dir, so driver verdicts and
  their evidence records land in results\\. A missing driver makes
  its case BLOCKED-ERROR, never PASS. The drivers also run in a
  stub tier (--mode stub via the case context) that exercises the
  decision logic against a stub probe on any platform; the stub
  tier evidences nothing about Windows.

Outcomes: PASS (all conditions evidenced), FAIL (a condition was
evidenced false), BLOCKED-WINDOWS (conditions remain that only a
Windows run can evidence), BLOCKED-ERROR (a driver or its evidence
is missing). On this Linux machine every case is expected to end
BLOCKED-WINDOWS with its audit parts green; a FAIL here is a real
defect in the tree or in the case.

Exit-code contract (consumed by run.ps1, which must not let a
battery decision abort the run before the test families): 0 when
the battery executed and no case is FAIL or BLOCKED-ERROR (PASS
and BLOCKED-WINDOWS verdicts); EXIT_EXECUTION_FAILURE (2) when any
case is BLOCKED-ERROR — the battery could not execute that case
(a missing driver, a driver error, withheld evidence), which
takes precedence over FAIL; EXIT_FAIL_DECISION (3) when the
battery executed, wrote its report, and at least one case verdict
is FAIL with none BLOCKED-ERROR — FAIL is an adoption decision
("do not adopt"), a legitimate outcome of the run, never an
execution failure. Any other exit (for example 1 from an
unhandled crash) is not a verdict at all.

Every case record — in the --json output and in battery.json —
names the tree it audited under "tree": the --tree value exactly
as passed (run.ps1 passes the resolved form of its own -Tree).

Usage: python battery.py [--tree PATH] [--json]
                         [--results-dir DIR]
"""

import argparse
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_TREE = os.path.normpath(os.path.join(HERE, "..", ".."))
DRIVERS_DIR = os.path.join(HERE, "drivers")

# Exit codes: see the exit-code contract in the module docstring.
EXIT_OK = 0
EXIT_EXECUTION_FAILURE = 2
EXIT_FAIL_DECISION = 3

sys.path.insert(0, HERE)
import env_fixtures  # noqa: E402


# ------------------------------------------------------- extraction
def _brace_end(src, open_idx):
    """Index just past the '}' matching the '{' at open_idx,
    skipping line/block comments, strings and char literals."""
    depth = 0
    i = open_idx
    n = len(src)
    while i < n:
        ch = src[i]
        if ch == "/" and i + 1 < n and src[i + 1] == "/":
            j = src.find("\n", i)
            i = n if j < 0 else j + 1
            continue
        if ch == "/" and i + 1 < n and src[i + 1] == "*":
            j = src.find("*/", i + 2)
            i = n if j < 0 else j + 2
            continue
        if ch == '"':
            i += 1
            while i < n and src[i] != '"':
                i += 2 if src[i] == "\\" else 1
            i += 1
            continue
        if ch == "'":
            m = re.match(r"'(\\.|[^\\'])'", src[i:])
            if m:
                i += m.end()
                continue
            i += 1
            continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return n


def _line_of(src, idx):
    return src.count("\n", 0, idx) + 1


def function_spans(src):
    """Every function definition with a body, as dicts with name,
    qualified name (Type::name inside an impl block), the line of
    the fn keyword, and the body's span and text."""
    impls = []
    for m in re.finditer(r"(?m)^[ \t]*impl\b([^{]*)\{", src):
        header = m.group(1)
        tokens = re.findall(r"[A-Za-z_]\w*", header)
        if "for" in tokens:
            type_name = tokens[tokens.index("for") + 1]
        elif tokens:
            type_name = tokens[-1]
        else:
            continue
        open_idx = m.end() - 1
        impls.append((open_idx, _brace_end(src, open_idx), type_name))
    spans = []
    for m in re.finditer(r"\bfn\s+([A-Za-z_]\w*)", src):
        j = m.end()
        while j < len(src) and src[j] not in "{;":
            j += 1
        if j >= len(src) or src[j] == ";":
            continue  # a declaration, not a definition
        end = _brace_end(src, j)
        qualified = m.group(1)
        for open_idx, impl_end, type_name in impls:
            if open_idx < m.start() < impl_end:
                qualified = f"{type_name}::{m.group(1)}"
                break
        spans.append({"name": m.group(1), "qualified": qualified,
                      "line": _line_of(src, m.start()),
                      "body_start": j, "body_end": end,
                      "body": src[j:end]})
    return spans


def extract_function(src, qualified):
    """The span of the function named `qualified` ("name" or
    "Type::name"), or None when no such function definition
    exists."""
    spans = function_spans(src)
    for span in spans:
        if span["qualified"] == qualified:
            return span
    if "::" not in qualified:
        matches = [s for s in spans if s["name"] == qualified]
        if len(matches) == 1:
            return matches[0]
    return None


def needle_line(src, span, needle):
    """1-based line of the first occurrence of needle inside the
    function's body, or None."""
    idx = span["body"].find(needle)
    if idx < 0:
        return None
    return _line_of(src, span["body_start"] + idx)


# ------------------------------------------------------------- cases
DRIVER_STATUSES = ("evidenced", "refuted", "error")


def _driver_payload_problem(payload):
    """The first shape defect in a driver payload, or None: the
    payload must be a JSON object; its conditions (when present)
    an object whose every value is an object carrying a status
    from DRIVER_STATUSES and a string detail; its records (when
    present) a list. A driver that reports no conditions object
    at all is not malformed here — the per-condition lookup
    names the missing verdicts — but every shape it does hand
    back must be one the case machinery can consume."""
    if not isinstance(payload, dict):
        return (f"its JSON payload is a {type(payload).__name__}, "
                "not an object of conditions and records")
    conditions = payload.get("conditions")
    if conditions is not None:
        if not isinstance(conditions, dict):
            return ("its conditions are a "
                    f"{type(conditions).__name__}, not an object "
                    "mapping condition names to verdicts")
        for name, cond in conditions.items():
            if not isinstance(cond, dict):
                return (f"condition {name!r} is a "
                        f"{type(cond).__name__}, not an object "
                        "carrying a status and a detail")
            if cond.get("status") not in DRIVER_STATUSES:
                return (f"condition {name!r} reports status "
                        f"{cond.get('status')!r}, which is not a "
                        "driver status (evidenced, refuted or "
                        "error)")
            if not isinstance(cond.get("detail"), str):
                return (f"condition {name!r} carries detail "
                        f"{cond.get('detail')!r}, not the string "
                        "detail every driver verdict carries")
    records = payload.get("records")
    if records is not None and not isinstance(records, list):
        return (f"its records are a {type(records).__name__}, "
                "not a list of evidence records")
    return None


class Case:
    """conditions: list of (name, kind, payload). kind "audit":
    payload(ctx) -> (status, detail). kind "driver": payload is the
    charged-runs text, used verbatim in the blocked detail; the
    status comes only from this case's driver."""

    def __init__(self, ident, title, conditions,
                 driver_timeout_s=None):
        self.ident = ident
        self.title = title
        self.conditions = conditions
        # The kill deadline for this case's Windows driver run.
        # None means the default; a case whose driver runs
        # sub-operations under their own deadlines must name a
        # deadline that covers their sum (case (m) does).
        self.driver_timeout_s = driver_timeout_s

    def driver_path(self, ctx):
        return os.path.join(ctx.get("driver_dir", DRIVERS_DIR),
                            f"case_{self.ident}.py")

    def _run_driver(self, ctx):
        """Invoke this case's driver once, when the context calls
        for a run. Returns (state, payload): state is "blocked"
        (no run on this platform), "none" (evidence withheld),
        "missing" (driver file absent), "error" (driver ran but
        produced no usable verdicts), or "ran"."""
        if ctx.get("evidence") == "none":
            return "none", None
        mode = ctx.get("driver_mode")
        if mode is None:
            mode = "windows" if ctx["platform"] == "win32" else "blocked"
        if mode == "blocked":
            return "blocked", None
        path = self.driver_path(ctx)
        if not os.path.isfile(path):
            return "missing", path
        out_path = None
        if ctx.get("results_dir"):
            out_path = os.path.join(ctx["results_dir"],
                                    f"battery-case-{self.ident}.json")
        argv = [sys.executable, path, "--mode", mode,
                "--variant", ctx.get("driver_variant", "good"),
                "--tree", ctx["tree"]]
        if ctx.get("build_dir"):
            argv += ["--build-dir", ctx["build_dir"]]
        if out_path:
            argv += ["--out", out_path]
        timeout = (self.driver_timeout_s or 3600) \
            if mode == "windows" else 600
        try:
            proc = subprocess.run(argv, capture_output=True, text=True,
                                  timeout=timeout)
        except subprocess.TimeoutExpired:
            return "error", f"driver exceeded its {timeout} s deadline"
        try:
            payload = json.loads(proc.stdout)
        except (json.JSONDecodeError, ValueError):
            return "error", (f"driver produced no parseable verdicts "
                             f"(exit {proc.returncode}): "
                             f"{proc.stderr.strip()[-400:]}")
        # The driver's exit code is part of its contract (0 = every
        # condition evidenced, 1 = a condition refuted, 2 =
        # blocked/error): verdicts printed by a driver that exited
        # outside that contract — a crash after printing, say — are
        # not usable evidence, however clean the JSON looks.
        if proc.returncode not in (0, 1):
            return "error", (f"driver exited {proc.returncode}, "
                             "outside its exit-code contract "
                             "(0 = evidenced, 1 = refuted, 2 = "
                             "blocked/error); its verdicts are "
                             "not used")
        # The payload's shape is part of the same contract: the
        # case machinery indexes it as an object of per-condition
        # objects carrying a status from the driver's own set
        # (evidenced / refuted / error) and a string detail, with
        # records a list. A payload in any other shape — a JSON
        # list or string, conditions as a list, a condition that
        # is a bare string or lacks its detail, a status outside
        # the set — previously crashed the case run outright
        # (item assignment on a list payload, KeyError on a
        # missing detail) or, for an unknown status, was silently
        # mapped to BLOCKED-WINDOWS, a tier verdict the driver
        # never returned. Malformed verdicts are driver error:
        # the case is BLOCKED-ERROR, and the defect is named.
        shape_problem = _driver_payload_problem(payload)
        if shape_problem is not None:
            return "error", (f"driver produced malformed verdicts: "
                             f"{shape_problem}")
        payload["_evidence_file"] = out_path
        payload["_exit_code"] = proc.returncode
        return "ran", payload

    def _driver_condition(self, ctx, state, payload, name, what):
        driver_rel = f"battery/drivers/case_{self.ident}.py"
        if state == "blocked":
            return {"condition": name, "status": "windows",
                    "detail": f"{what}: requires Windows; the driver "
                              f"{driver_rel} executes these runs and "
                              f"is not run on {ctx['platform']}"}
        if state == "none":
            return {"condition": name, "status": "error",
                    "detail": f"{what}: no driver evidence available; "
                              f"a condition is evidenced only by a "
                              f"driver run of {driver_rel} with its "
                              f"evidence records preserved"}
        if state == "missing":
            return {"condition": name, "status": "error",
                    "detail": f"{what}: driver missing: {payload}; "
                              f"the case is BLOCKED-ERROR, never PASS"}
        if state == "error":
            return {"condition": name, "status": "error",
                    "detail": f"{what}: driver error: {payload}"}
        conditions = payload.get("conditions", {})
        cond = conditions.get(name) if isinstance(conditions, dict) \
            else None
        if cond is None:
            return {"condition": name, "status": "error",
                    "detail": f"{what}: the driver returned no "
                              f"verdict for this condition"}
        if not isinstance(cond, dict) \
                or cond.get("status") not in DRIVER_STATUSES \
                or not isinstance(cond.get("detail"), str):
            return {"condition": name, "status": "error",
                    "detail": f"{what}: the driver returned a "
                              f"malformed verdict for this "
                              f"condition: {cond!r}"}
        records = payload.get("records", [])
        result = {"condition": name, "status": cond["status"],
                  "detail": cond["detail"]}
        result["evidence"] = {
            "driver": driver_rel, "tier": payload.get("tier"),
            "records": len(records) if isinstance(records, list)
            else 0,
            "file": payload.get("_evidence_file")}
        return result

    def run(self, ctx):
        needs_driver = any(kind == "driver"
                           for _n, kind, _p in self.conditions)
        state = payload = None
        if needs_driver:
            state, payload = self._run_driver(ctx)
        results = []
        for name, kind, payload_item in self.conditions:
            if kind == "audit":
                status, detail = payload_item(ctx)
                results.append({"condition": name, "status": status,
                                "detail": detail})
            else:
                results.append(self._driver_condition(
                    ctx, state, payload, name, payload_item))
        if any(r["status"] == "error" for r in results):
            # An execution failure takes precedence over a FAIL
            # decision (the exit-code contract above): a case that
            # could not be fully executed produced no complete
            # decision, even when another condition was refuted.
            verdict = "BLOCKED-ERROR"
        elif any(r["status"] == "refuted" for r in results):
            verdict = "FAIL"
        elif all(r["status"] == "evidenced" for r in results):
            verdict = "PASS"
        else:
            verdict = "BLOCKED-WINDOWS"
        return {"case": self.ident, "title": self.title,
                "verdict": verdict, "conditions": results,
                "tree": ctx["tree"]}


def read_tree(ctx, rel):
    with open(os.path.join(ctx["tree"], rel), encoding="utf-8",
              errors="replace") as h:
        return h.read()


# ---------------------------------------------------------------- (a)
def case_a():
    """Job-handle signalled-when-empty teardown wait vs the current
    Sleep(10) poll. The battery decides adoption."""
    def poll_present(ctx):
        src = read_tree(ctx, "src/engine/wait.rs")
        span = extract_function(src, "wait_until_empty")
        line = needle_line(src, span, "Sleep(10)") if span else None
        term = extract_function(src, "terminate_and_wait")
        call_line = (needle_line(src, term, "wait_until_empty")
                     if term else None)
        if line is not None and call_line is not None:
            return ("evidenced",
                    "baseline located: src/engine/wait.rs, function "
                    f"wait_until_empty, line {line}: Sleep(10) sits "
                    "inside wait_until_empty, which terminate_and_wait "
                    f"calls at line {call_line}; terminate_and_wait "
                    "polls active_processes with Sleep(10) between "
                    "polls inside the 5,000 ms teardown budget")
        return ("refuted",
                "baseline poll not found where the case expects it: "
                "Sleep(10) inside the body of wait_until_empty in "
                "src/engine/wait.rs, called by terminate_and_wait; "
                "the comparison target has moved and the case is "
                "stale")

    return Case("a", "job-handle signalled-when-empty teardown wait "
                     "vs Sleep(10) poll — battery decides adoption",
                [("baseline poll located in source", "audit",
                  poll_present),
                 ("verdict equivalence + latency + budget", "driver",
                  "equivalence runs: for tree sizes 0/1/2/64/65 members "
                  "and a member that never dies, the signalled wait "
                  "and the poll must return the same (empty, "
                  "remainder) verdicts, both proofs racing the same "
                  "job (one job per trial, terminated once, both "
                  "waits armed on it), the signalled wait's latency "
                  "to proof may exceed the poll's by at most one "
                  "poll tick (10 ms, the Sleep(10) granularity) per "
                  "trial — a sub-tick difference is noise, not a "
                  "latency loss — and the single 5,000 ms budget "
                  "must bind the wait exactly as it binds the poll"),
                 ("no lost wakeup at the emptying race", "driver",
                  "race runs: a job that empties between the "
                  "non-empty check and the wait's start must still "
                  "wake the wait (no lost signal), across 1,000 "
                  "repetitions with randomised emptying delays")])


# ---------------------------------------------------------------- (l)
def case_l():
    """Environment block via GetEnvironmentStringsW on extendable
    starts, including a vars_os differential."""
    def current_form(ctx):
        src = read_tree(ctx, "src/engine/environment.rs")
        span = extract_function(src, "environment_block")
        line = needle_line(src, span, "vars_os()") if span else None
        if (line is not None
                and "GetEnvironmentStringsW" not in span["body"]):
            return ("evidenced",
                    "baseline located: src/engine/environment.rs, "
                    f"function environment_block, line {line}: "
                    "environment_block builds from vars_os; the "
                    "GetEnvironmentStringsW form is absent from "
                    "environment_block")
        return ("refuted",
                "baseline moved: vars_os inside environment_block in "
                "src/engine/environment.rs, with no "
                "GetEnvironmentStringsW form there, is the premise; "
                "the case's premise is stale")

    names = ", ".join(env_fixtures.fixture_names())
    differential_what = (
        "vars_os differential: for each fixture environment in the "
        f"fixture set ({names} — the named shapes in "
        "fixtures/env-fixtures.json plus the 500-entry case "
        "generated deterministically by battery/env_fixtures.py), "
        "the block built from GetEnvironmentStringsW and the block "
        "built from vars_os must be byte-identical after the "
        "documented sort, or every difference must be a named, "
        "lossless, adoption-blocking-or-accepted entry in the case "
        "report; adoption requires losslessness, preserved "
        "skip-all matching, =C: parsing, and stable-sort/one-entry "
        "semantics. The vars_os side is produced by executing the "
        "tree's actual builder (a Rust helper compiled with the "
        "tree's src/engine/environment.rs included, its "
        "environment_block run in a process whose environment is "
        "the fixture), the GetEnvironmentStringsW side is read "
        "unfiltered (every entry the enumeration yields, not only "
        "names a fixture names), and the two enumerations "
        "themselves are compared, so a divergence in a "
        "non-fixture entry is decidable; the one case matcher is "
        "the tree's eq_ignore_ascii_case semantics, implemented "
        "identically on both sides. Each fixture is judged in "
        "two forms: as-is, and with the channel entry set — the "
        "set operation actually performed on both sides, every "
        "existing case-variant of the channel name skipped and "
        "replaced; a fixture a real Windows environment cannot "
        "represent (duplicate case-folded names) is judged at "
        "the list level, never installed where it would collapse")
    return Case("l", "environment block via GetEnvironmentStringsW on "
                     "extendable starts — vars_os differential",
                [("vars_os baseline located in source", "audit",
                  current_form),
                 ("byte-level differential over the fixture set",
                  "driver", differential_what)])


# ---------------------------------------------------------------- (m)
# The four counted fetch sites (census): hardening's process fetch,
# hardening's thread fetch, the owner walk's self fetch in
# watched_owner_handles, and the exit path's process fetch. Each
# site is anchored to its enclosing function.
PSEUDO_SITES = [
    ("hardening process fetch", "src/cli_process/harden.rs",
     "harden_self", "GetCurrentProcess()"),
    ("hardening thread fetch", "src/cli_process/harden.rs",
     "harden_self", "GetCurrentThread()"),
    ("owner facts self fetch", "src/engine/owners.rs",
     "watched_owner_handles", "GetCurrentProcess()"),
    ("exit path process fetch", "src/diagnostics.rs",
     "cli_exit", "GetCurrentProcess()"),
]


def _strip_line(text):
    """A source line with string contents and any // comment
    removed, so identifier matching never fires inside them."""
    out = []
    i = 0
    while i < len(text):
        if text[i] == '"':
            i += 1
            while i < len(text) and text[i] != '"':
                i += 2 if text[i] == "\\" else 1
            i += 1
            out.append('""')
            continue
        if text[i] == "/" and i + 1 < len(text) and text[i + 1] == "/":
            break
        out.append(text[i])
        i += 1
    return "".join(out)


def _consumer_indices(body_lines, needle):
    """Body-line indices (0-based) that consume the handle one
    fetch of needle produces: for a `let` binding, the later lines
    that use the bound name or a name the value is moved into —
    stopping the bound name's own tracking where a later `let`
    rebinds it to an unrelated value (the value moved into its
    alias is still tracked); for an inline fetch, the fetch line
    itself, whose call takes the value."""
    for i, line in enumerate(body_lines):
        if needle not in line:
            continue
        binding = re.search(r"\blet\s+(?:mut\s+)?([A-Za-z_]\w*)", line)
        if not binding:
            return [i]
        var = binding.group(1)
        aliases = set()
        indices = []
        var_live = True
        for j in range(i + 1, len(body_lines)):
            text = _strip_line(body_lines[j])
            rebind = re.search(
                rf"\blet\s+(?:mut\s+)?{re.escape(var)}\s*=", text)
            if rebind and var_live:
                rhs = text[rebind.end():]
                if not re.search(rf"\b{re.escape(var)}\b", rhs):
                    var_live = False  # a different value takes the name
            uses = (var_live
                    and re.search(rf"\b{re.escape(var)}\b", text)) or \
                any(re.search(rf"\b{re.escape(a)}\b", text)
                    for a in aliases)
            if uses:
                indices.append(j)
                # An alias is a name the value itself is moved
                # into (possibly wrapped: Some(handle), &handle) —
                # not a value merely derived from it by a call.
                assign = re.search(
                    r"(?:\blet\s+(?:mut\s+)?)?([A-Za-z_]\w*)\s*=(?![=])",
                    text)
                if assign and assign.group(1) != var:
                    rhs = text[assign.end():].strip().rstrip(";")
                    if re.fullmatch(
                            rf"(?:Some|Ok|&|&mut)?\s*\(?\s*"
                            rf"{re.escape(var)}\s*\)?", rhs):
                        aliases.add(assign.group(1))
        return indices
    return []


def handle_consumers(span, needle):
    """The consumers of a fetched handle inside its enclosing
    function, named by the call each consuming line feeds. Each
    consumer is reported with its body line, re-anchored to a file
    line by the caller."""
    body_lines = span["body"].splitlines()
    consumers = []
    for j in _consumer_indices(body_lines, needle):
        line = body_lines[j]
        if needle in line:
            needle_name = needle.rstrip("()")
            calls = [c for c in re.findall(r"([A-Za-z_]\w*)\s*\(", line)
                     if c != needle_name]
        else:
            binding = re.search(r"\blet\s+(?:mut\s+)?([A-Za-z_]\w*)",
                                body_lines[_first_needle_line(body_lines, needle)])
            var = binding.group(1) if binding else ""
            calls = [c for c in re.findall(r"([A-Za-z_]\w*)\s*\(", line)
                     if c != var and c not in ("Some", "Ok")]
        for call in calls:
            entry = f"{call} (body line {j + 1})"
            if entry not in consumers:
                consumers.append(entry)
        if not calls:
            entry = f"use (body line {j + 1})"
            if entry not in consumers:
                consumers.append(entry)
    return consumers


def _first_needle_line(body_lines, needle):
    for i, line in enumerate(body_lines):
        if needle in line:
            return i
    return 0


def pseudo_site_report(tree):
    """Per-site audit data: for each counted site, whether the
    needle occurs inside its named function's body, the file, the
    function, the needle's line, and the enumerated handle
    consumers (with file lines)."""
    cache = {}
    report = []
    for name, rel, func, needle in PSEUDO_SITES:
        if rel not in cache:
            path = os.path.join(tree, rel)
            with open(path, encoding="utf-8", errors="replace") as h:
                cache[rel] = h.read()
        src = cache[rel]
        span = extract_function(src, func)
        line = needle_line(src, span, needle) if span else None
        consumers = []
        consumer_lines = []
        if line is not None:
            # Re-anchor consumer body lines to file lines: body
            # line 1 is the line of the body's '{'.
            brace_line = _line_of(src, span["body_start"])
            raw = handle_consumers(span, needle)
            for entry in raw:
                m = re.search(r"\(body line (\d+)\)", entry)
                consumers.append(entry.replace(
                    m.group(0), f"(line {brace_line + int(m.group(1)) - 1})"))
            body_lines = span["body"].splitlines()
            consumer_lines = [body_lines[j] for j in
                              _consumer_indices(body_lines, needle)]
        report.append({"name": name, "file": rel, "function": func,
                       "needle": needle, "line": line,
                       "consumers": consumers,
                       "consumer_lines": consumer_lines})
    return report


def audit_case_m(ctx):
    report = pseudo_site_report(ctx["tree"])
    missing = [s for s in report if s["line"] is None]
    if missing:
        return ("refuted", "; ".join(
            f"{s['name']}: {s['needle']} not found inside "
            f"{s['function']} in {s['file']}" for s in missing))
    sites = "; ".join(
        f"{s['name']}: {s['file']}, function {s['function']}, "
        f"line {s['line']}; handle consumers: "
        + (", ".join(s["consumers"]) if s["consumers"] else "none")
        for s in report)
    return ("evidenced",
            "per-site comparison audit: all four counted fetch "
            "sites located inside their enclosing functions — "
            + sites
            + ". Each site's consumer set is enumerated above and "
              "must be re-read at adoption time: (HANDLE)-1 "
              "collides numerically with INVALID_HANDLE_VALUE, so "
              "every consumer of each replaced handle must be shown "
              "not to test the value against INVALID_HANDLE_VALUE")


def case_m():
    return Case("m", "pseudo-handle constants at the four counted "
                     "fetch sites — battery decides adoption; "
                     "per-site audit is part of the case",
                [("per-site comparison audit (source)", "audit",
                  audit_case_m),
                 ("per-site A/B behaviour on Windows", "driver",
                  "per-site A/B: build with the constant substituted "
                  "at one site at a time and run the guard's own "
                  "Windows test files (guard, auto, antigravity, "
                  "battery parity checks) plus the hook verdict "
                  "path; adoption per site requires the full set "
                  "green with that site substituted and no consumer "
                  "treating the constant as an invalid handle")],
                # The driver may legitimately consume, per site,
                # one build and one test run of 1,800 s each
                # (driver_common.BUILD_TIMEOUT_S): 4 sites x
                # (1,800 + 1,800) s. The kill deadline covers
                # that sum; the driver's shared-target design
                # (one scratch copy, one cargo target across
                # sites) is what makes the sum a bound rather
                # than the expectation.
                driver_timeout_s=4 * (1800 + 1800))


# ---------------------------------------------------------------- (n)
def case_n():
    def split_present(ctx):
        src = read_tree(ctx, "src/cli_process.rs")
        span = extract_function(src, "Console::finish")
        free_line = (needle_line(src, span, "FreeConsole")
                     if span else None)
        attach_line = (needle_line(src, span, "AttachConsole")
                       if span else None)
        if free_line is not None and attach_line is not None:
            return ("evidenced",
                    "verdict consumers located: src/cli_process.rs, "
                    f"function Console::finish, line {span['line']}: "
                    f"Console::finish performs the FreeConsole "
                    f"(line {free_line}) / AttachConsole "
                    f"(line {attach_line}) cycle; the cli_process "
                    "handle slots and the Console::finish cycle are "
                    "the before/after comparison surface")
        return ("refuted",
                "console verdict surface not found: Console::finish "
                "in src/cli_process.rs containing its "
                "FreeConsole/AttachConsole cycle is the premise; "
                "case is stale")

    return Case("n", "lazy console verdicts — adoption gated on "
                     "before/after verdict equality across the "
                     "Console::finish cycle",
                [("verdict surface located in source", "audit",
                  split_present),
                 ("verdict equality across the finish cycle",
                  "driver",
                  "before/after verdict equality: for every "
                  "combination of standard handles (console / pipe "
                  "/ file / closed) x every role program x runs "
                  "with and without post-finish output pending, the "
                  "set of console verdicts consulted and their "
                  "values under lazy probing must equal those under "
                  "eager probing, across the whole Console::finish "
                  "FreeConsole/AttachConsole cycle. Role cells "
                  "evidence this with verdict-shaped records — "
                  "each role's full decision output bytes plus "
                  "exit code plus the consulted verdict values, "
                  "paired across the pending dimension, never exit "
                  "codes alone; pending output is really produced "
                  "on the role's own handles and every held byte "
                  "count is measured (pending-pattern bytes "
                  "counted in the drained stream or in the "
                  "producer's write record), never assigned from "
                  "the pending label; the gate runs a guarded "
                  "payload through a named vendor whose verdict "
                  "can vary, and the contain role runs inside a "
                  "real enclosing extendable run (a contain cell "
                  "that cannot run in that shape is BLOCKED-ERROR, "
                  "never a pairing). Adoption is gated on full "
                  "equality; on any inequality the fallback is to "
                  "pin, before Console::finish, only the verdicts "
                  "with pending post-finish output — the battery "
                  "must then evidence that the fallback preserves "
                  "equality on the failing cells")])


# ---------------------------------------------------------------- (o)
def case_o():
    def extendable_port_present(ctx):
        src = read_tree(ctx, "src/extend/server.rs")
        span = extract_function(src, "Extension::start")
        line = (needle_line(src, span, "CreateIoCompletionPort")
                if span else None)
        if line is not None:
            return ("evidenced",
                    "the extendable path's existing completion-port "
                    "association is the in-tree precedent the case "
                    "compares against: src/extend/server.rs, "
                    f"function Extension::start, line {span['line']}, "
                    f"CreateIoCompletionPort at line {line}; the "
                    "plain teardown branch carries no association "
                    "today")
        return ("refuted",
                "extendable completion-port precedent not found: "
                "CreateIoCompletionPort inside Extension::start in "
                "src/extend/server.rs is the premise")

    burden = [
        ("zero-packet delivery, including the already-empty-at-association case",
         "a job that empties before association, at association, and after "
         "association must each deliver the proof exactly once"),
        ("packet-stream composition (honest dependence census)",
         "exit packets queue ahead of the zero packet; the dequeue count "
         "for trees of 1/64/65/300 members is recorded as the dependence, "
         "not claimed as a fixed count"),
        ("the never-dies budget case",
         "a member that never dies consumes the single 5,000 ms budget "
         "through recomputed remainders and returns the same "
         "(empty=false, remainder) contract as the poll"),
        ("fallback invariance",
         "association failure at each injectable point falls back to the "
         "current poll proof with identical verdicts, the two verdicts "
         "being independent observations — the fallback behaviour with "
         "the port absent, and a separate poll run over a separately "
         "created identical job state — never two readings of one "
         "record"),
        ("port close / disassociation effects",
         "the port's CloseHandle is paid (minimum honest count is 5, not "
         "4): every call in the teardown sequence has its return value "
         "checked (disassociation additionally confirmed by an "
         "association-state query), the count is the number of "
         "confirmed operations in the observed sequence so a failed "
         "call changes the count and the verdict, no handle leaks "
         "across 10,000 returning library calls, and disassociation "
         "leaves no consumer of the port"),
        ("non-inheritance",
         "the port handle is never inherited by the child or any "
         "descendant: the checking child is spawned with handle "
         "inheritance enabled and the port handle offered through "
         "the inheritance path, the port itself created "
         "non-inheritable — the record states the creation "
         "attributes used, and a deliberately inheritable control "
         "handle offered through the same spawn must be reported "
         "inherited, proving the trial can see inheritance"),
        ("forged-packet case (binding conditions)",
         "a packet forged or posted by a same-user foreign process must "
         "not fake the proof: the packet is wake-only, the verdict comes "
         "only from an authoritative job query at the decision instant, "
         "and no reply field may influence the verdict — the case posts "
         "forged packets of every shape the port accepts and requires "
         "the verdict to track the job query, never the packet"),
    ]
    conditions = [("extendable precedent located in source", "audit",
                   extendable_port_present)]
    for name, what in burden:
        conditions.append((name, "driver", what))
    return Case("o", "IOCP teardown proof — six-item burden plus the "
                     "forged-packet case, under the binding conditions",
                conditions)


CASES = [case_a(), case_l(), case_m(), case_n(), case_o()]


def main(argv=None):
    parser = argparse.ArgumentParser(description="theory-item battery")
    parser.add_argument("--tree", default=DEFAULT_TREE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--results-dir", default=None,
                        help="folder for the battery report and the "
                             "per-case driver evidence records")
    args = parser.parse_args(argv)
    ctx = {"tree": args.tree, "platform": sys.platform}
    if args.results_dir:
        ctx["results_dir"] = args.results_dir
    reports = [case.run(ctx) for case in CASES]
    if args.results_dir:
        os.makedirs(args.results_dir, exist_ok=True)
        with open(os.path.join(args.results_dir, "battery.json"),
                  "w", encoding="utf-8") as handle:
            json.dump(reports, handle, indent=2)
    if args.json:
        print(json.dumps(reports, indent=2))
    else:
        for rep in reports:
            print(f"case ({rep['case']}) {rep['title']}")
            print(f"  verdict: {rep['verdict']}")
            for cond in rep["conditions"]:
                print(f"  [{cond['status']}] {cond['condition']}: "
                      f"{cond['detail']}")
            print()
    if any(r["verdict"] == "BLOCKED-ERROR" for r in reports):
        return EXIT_EXECUTION_FAILURE
    if any(r["verdict"] == "FAIL" for r in reports):
        return EXIT_FAIL_DECISION
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main())
