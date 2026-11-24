"""Independent admission of a complete campaign package under sealed bindings."""

from collections import Counter
import hashlib
import json
from pathlib import Path

EXPECTED_COUNTS = {"control": 36, "warmup": 224, "measured": 1680}
SLOTS = 1940
FILES = ("run.json", "observations.jsonl", "pricing-observations.jsonl",
         "claim-ledger.json", "report.txt")


def read_jsonl(raw):
    return [json.loads(line) for line in raw.splitlines()]


def sha256_file(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def package_path(root, name):
    item = root / name
    if not item.is_file() or item.resolve() != item:
        raise ValueError(f"campaign file absent or redirected: {name}")
    return item


def observed_key(row):
    key = [row[field] for field in ("kind", "arm", "path", "context", "rep")]
    if row.get("variant") is not None:
        key.append(row["variant"])
    return key


def require_integer(data, field, expected):
    value = data.get(field)
    if type(value) is not int or value != expected:
        raise ValueError(f"campaign {field} does not equal {expected}")


def processkit_event_evidence(root, observations, *, recorded_root=None):
    actual_root = root / "processkit-events"
    if not actual_root.is_dir() or actual_root.resolve() != actual_root:
        raise ValueError("ProcessKit event directory absent or redirected")
    recorded_root = actual_root if recorded_root is None else recorded_root
    dirs = set()
    hashes = {}
    missing = []
    for row in observations:
        if row["arm"] != "processkit":
            continue
        argv = row.get("argv")
        if not isinstance(argv, list) or argv.count("--jsonl") != 1:
            raise ValueError("ProcessKit slot does not bind one event path")
        index = argv.index("--jsonl") + 1
        if index >= len(argv) or not isinstance(argv[index], str):
            raise ValueError("ProcessKit event path is missing from argv")
        path = Path(argv[index])
        slot = path.parent.name
        if (path.name != "events.jsonl" or not slot.startswith("slot-")
                or path.parent.parent != recorded_root or slot in dirs):
            raise ValueError("ProcessKit event path is duplicated or outside the bound tree")
        dirs.add(slot)
        directory = actual_root / slot
        if not directory.is_dir() or directory.resolve() != directory:
            raise ValueError("ProcessKit slot event directory is absent or redirected")
        names = {item.name for item in directory.iterdir()}
        if not names:
            if (row["path"], row.get("variant"), row.get("exit_code")) != (
                    "malformed", "malformed-timeout", 100):
                raise ValueError("ProcessKit slot is missing required event evidence")
            missing.append(observed_key(row))
            continue
        if names != {"events.jsonl"}:
            raise ValueError("ProcessKit slot event directory contains unexpected entries")
        file = package_path(directory, "events.jsonl")
        raw = file.read_bytes()
        events = read_jsonl(raw)
        if (not events or any(type(event.get("schema_version")) is not int
                              or event["schema_version"] != 1 for event in events)
                or events[-1].get("event") != "runner_exit"
                or events[-1].get("code") != row["exit_code"]):
            raise ValueError("ProcessKit event stream does not match its observed exit")
        first = events[0]
        if first.get("event") == "run_started":
            prefix = f"run-{row['arm_pid']}-"
            if not isinstance(first.get("run_id"), str) or not first["run_id"].startswith(prefix):
                raise ValueError("ProcessKit event stream does not match its arm PID")
        elif first.get("event") != "spawn_failed" or (row["path"] != "malformed"
                                                      and row["outcome"] == "ok"):
            raise ValueError("ProcessKit event stream lacks the expected opening event")
        hashes[file.relative_to(root).as_posix()] = hashlib.sha256(raw).hexdigest()
    if {path.name for path in actual_root.iterdir()} != dirs:
        raise ValueError("ProcessKit event directory census differs from observations")
    return hashes, missing


def audit_complete(harness, root, *, run_id, config, config_sha, seed, fingerprints,
                   recorded_event_root=None):
    root = root.absolute()
    if root.name != run_id or root.resolve() != root or not root.is_dir():
        raise ValueError("campaign namespace differs from sealed run ID")
    files = {name: package_path(root, name) for name in FILES}
    digests = {name: sha256_file(path) for name, path in files.items()}
    meta = json.loads(files["run.json"].read_bytes())
    if (meta.get("run_id") != run_id or Path(meta.get("config_path", "")) != config
            or meta.get("config_sha256") != config_sha
            or meta.get("reduced") is not False or meta.get("provisional") is not False
            or meta.get("slot_isolation") != harness.WINDOWS_SLOT_ISOLATION):
        raise ValueError("campaign metadata differs from sealed identity or final shape")
    for name, expected in (("seed", seed), ("slots_run", SLOTS), ("slots_planned", SLOTS)):
        require_integer(meta, name, expected)
    counts = meta.get("plan_counts")
    if counts != EXPECTED_COUNTS or any(type(value) is not int for value in counts.values()):
        raise ValueError("campaign metadata plan counts differ")
    observed_fingerprints = meta.get("fingerprints", {})
    if set(observed_fingerprints) != set(fingerprints):
        raise ValueError("campaign executable inventory differs")
    for arm, expected in fingerprints.items():
        actual = observed_fingerprints[arm]
        if (Path(actual.get("path", "")) != expected["path"]
                or actual.get("sha256") != expected["sha256"]
                or actual.get("size") != expected["size"]):
            raise ValueError(f"campaign executable identity differs: {arm}")
    if meta.get("competitor_identity_mismatches") != []:
        raise ValueError("campaign recorded competitor identity mismatches")

    order = [harness.slot_key(slot) for slot in harness.build_plan(seed)]
    if len(order) != SLOTS or meta.get("order") != order:
        raise ValueError("campaign seeded order differs")
    observations = read_jsonl(files["observations.jsonl"].read_bytes())
    if len(observations) != SLOTS or [observed_key(row) for row in observations] != order:
        raise ValueError("campaign observations are not the full seeded sequence")
    if Counter(row["kind"] for row in observations) != EXPECTED_COUNTS:
        raise ValueError("campaign observation census differs")
    if not all(harness._slot_job_drained(row.get("slot_job")) for row in observations):
        raise ValueError("campaign lacks one or more exact slot Job drain receipts")

    package_problems = harness.check_run_package(root)
    if package_problems:
        raise ValueError(f"canonical package verification failed: {package_problems[:12]!r}")
    if any(sha256_file(files[name]) != digest for name, digest in digests.items()):
        raise ValueError("campaign package changed during verification")
    events, absent = processkit_event_evidence(root, observations, recorded_root=recorded_event_root)
    digests.update(events)
    return {
        "complete": True, "run_id": run_id, "slots": len(observations),
        "forced_slot_drains": sum(row["slot_job"]["terminated"] for row in observations),
        "campaign_status": meta["campaign_status"],
        "standing_loss_count": meta["standing_loss_count"],
        "freeze_gate_fail_count": meta["freeze_gate_fail_count"],
        "processkit_event_files": len(events),
        "processkit_parser_refusals_without_event": absent,
        "file_sha256": digests,
    }