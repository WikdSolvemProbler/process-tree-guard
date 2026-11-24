# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Independently join the settled laptop A clocks and the stock campaign."""

import hashlib
from collections import Counter
from pathlib import Path

import harness
from laptop_launch import file_hash, parse_json, require, worker_argv, worker_pipe_name
from laptop_schedule import MODE, SLOT_GROUPS, build_schedule, canonical_bytes, schedule_bytes, schedule_sha256
from pricing import GUARD_LAUNCHER_MODE, PROBE_TIMEOUT_DETAIL
from laptop_worker import capture_size, group_plan
from plan import PRODUCT_ARMS, check_plan, slot_key


CAMPAIGN_FILES = (
    "run.json", "observations.jsonl", "pricing-observations.jsonl", "report.txt",
    "claim-ledger.json",
)


def jsonl(path):
    raw = path.read_bytes()
    require(raw.endswith(b"\n"), f"missing terminal newline: {path.name}")
    rows = [parse_json(line) for line in raw.splitlines()]
    require(all(isinstance(row, dict) for row in rows), f"non-object row: {path.name}")
    return rows


def bind_streams(capture, observations):
    files = {}
    streams = capture / "streams"
    require(streams.is_dir() and streams.resolve() == streams, "stream custody directory differs")
    for index, row in enumerate(observations):
        require(not row.get("laptop_stream_error"), "stream custody failed during capture")
        artifacts = row.get("laptop_streams")
        if row["path"] != "fast":
            require(artifacts is None, "unplanned stream exposure")
            continue
        require(isinstance(artifacts, dict) and set(artifacts) == {"stdout", "stderr"},
                "planned capture lacks retained stream custody")
        for name in ("stdout", "stderr"):
            expected_name = f"{index:04d}-{name}.bin"
            expected = artifacts[name]
            path = streams / expected_name
            require(path.is_file() and path.resolve() == path, "retained stream is absent or redirected")
            size, digest = path.stat().st_size, file_hash(path)
            require(expected == {"name": expected_name, "size": size, "sha256": digest},
                    "retained stream fingerprint differs")
            observed = row.get(f"output_{name}_sha256")
            if observed is not None:
                require((observed, row.get(f"output_{name}_len")) == (digest, size),
                        "retained bytes differ from the original timed oracle")
            else:
                require(row.get("outcome") != "ok", "successful relay output was never observed")
            files[expected_name] = digest
    require({path.name for path in streams.iterdir()} == set(files),
            "retained stream census differs")
    return files


def bind_worker_items(capture, schedule, observations, pricing):
    durable = jsonl(capture / "laptop-pricing.jsonl")
    require(durable == [{"index": len(schedule) + index, "observation": row}
                        for index, row in enumerate(pricing)], "pricing durable prefix differs")
    worker = jsonl(capture / "laptop-worker.jsonl")
    settled = [row for row in worker if row.get("kind") == "item_settled"]
    require(len(settled) == len(observations) + len(pricing), "settlement population differs")
    for index, (ordered, observation, receipt) in enumerate(zip(schedule, observations, settled)):
        key = slot_key(ordered.slot)
        require(key == receipt["identity"]
                and [observation[column] for column in ("kind", "arm", "path", "context", "rep")]
                == key[:5] and observation.get("variant") == ordered.slot.variant
                and receipt["index"] == index, "durable slot identity differs")
        bounds = observation.get("laptop_timing_ns")
        require(isinstance(bounds, list) and len(bounds) == 2
                and all(type(value) is int for value in bounds)
                and bounds == [receipt.get("timing_start_ns"), receipt.get("timing_end_ns")]
                and 0 <= bounds[0] <= bounds[1]
                and observation["elapsed_ms"] == (bounds[1] - bounds[0]) / 1e6,
                "durable clock differs from the stock timing measurement")
    for index, (price, receipt) in enumerate(zip(pricing, settled[len(observations):])):
        require(receipt["index"] == len(observations) + index
                and receipt["identity"] == ["pricing", price["probe"], price["rep"], price["shape"]],
                "pricing item differs from the durable settled order")
    return settled[len(observations):]


def bind_worker_launch(launch, telemetry, manifest, manifest_sha):
    starts = [row for row in telemetry if row.get("kind") == "started"]
    workers = [row for row in telemetry if row.get("kind") == "worker_dispatched"]
    require(len(starts) == len(workers) == 1, "capture has ambiguous worker launch evidence")
    origin, worker = starts[0], workers[0]
    require(launch.get("manifest_sha256") == origin.get("manifest_sha256") == manifest_sha,
            "launch and capture manifest bindings differ")
    nonce = origin.get("barrier_nonce")
    expected = worker_argv(Path(manifest["manifest_path"]), manifest_sha,
                           manifest, worker_pipe_name(nonce), nonce)
    require(launch.get("worker_argv") == origin.get("worker_argv") == expected
            and worker.get("worker_recipe_sha256") == hashlib.sha256(canonical_bytes(expected)).hexdigest(),
            "recorded worker invocation differs from the frozen recipe")
    python = manifest["files"]["python"]
    require(Path(worker["image_path"]) == Path(python["path"])
            and worker.get("image_sha256") == python["sha256"]
            and worker.get("job_member") is True
            and type(worker.get("pid")) is int and worker["pid"] > 0
            and type(worker.get("creation_filetime")) is int and worker["creation_filetime"] > 0,
            "held native worker image, identity or Job proof differs")
    from laptop_ancestry import validate_ancestry

    proof = launch.get("guard_ancestry")
    require(proof == origin.get("guard_ancestry"), "launch and observer ancestry receipts differ")
    observer_pid = launch.get("supervisor_pid")
    validate_ancestry(proof, manifest["image_policy"], observer_pid)
    primary, tool = proof["roles"]
    for item in (primary, tool):
        expected_guard = manifest["files"][item["role"]]
        require(Path(item["path"]) == Path(expected_guard["path"])
                and item["image_sha256"] == expected_guard["sha256"],
                "enclosing guard role differs from the frozen invocation")
    require(observer_pid == origin.get("observer_pid")
            and (primary["pid"], primary["creation_filetime"])
            == (origin.get("guard_pid"), origin.get("guard_created_ticks"))
            and proof["chain"][0]["creation_filetime"] <= worker["creation_filetime"]
            and worker["pid"] not in {node["pid"] for node in proof["chain"]},
            "worker and observer receipt ancestry differs")
    return {"pid": worker["pid"], "creation_filetime": worker["creation_filetime"]}, nonce


def bind_price_jobs(path, pricing, settlements, manifest, manifest_sha, worker_identity, nonce):
    raw = path.read_bytes()
    require(raw.endswith(b"\n"), "pricing membership evidence has a torn record")
    lines = raw.splitlines(keepends=True)
    require(len(lines) == len(pricing) == len(settlements), "pricing Job proof population differs")
    helper = [
        manifest["files"]["python"]["path"], "-I", "-B",
        manifest["sources"]["pricing.py"]["path"], GUARD_LAUNCHER_MODE,
    ]
    for offset, (line, row, settlement) in enumerate(zip(lines, pricing, settlements)):
        membership = parse_json(line)
        identity = {"accepted": True, "before": worker_identity, "after": worker_identity,
                    "assigned": 1, "listed": 1, "pids": [worker_identity["pid"]]}
        actual = membership.get("witness", {})
        require(isinstance(actual, dict) and actual.get("accepted") is True
                and all(type(actual.get(key)) is int for key in ("assigned", "listed"))
                and membership.get("kind") == "pricing-settlement"
                and membership.get("nonce") == nonce
                and membership.get("manifest_sha256") == manifest_sha
                and type(membership.get("group")) is int
                and membership["group"] == settlement["group"] == SLOT_GROUPS + offset // 2
                and type(membership.get("index")) is int
                and membership["index"] == settlement["index"] and actual == identity
                and hashlib.sha256(line).hexdigest() == settlement.get("pricing_membership_sha256"),
                "pricing phase Job proof does not bind the sole held worker")
        asked, authorized = membership["requested_ns"], membership["authorized_ns"]
        require(type(asked) is int and type(authorized) is int
                and settlement["timing_end_ns"] <= asked <= authorized <= settlement["settled_ns"]
                <= asked + 1_000_000_000 and row.get("detail") != PROBE_TIMEOUT_DETAIL,
                "pricing proof crossed the timed endpoint or deadline")
        if row.get("probe") == "owner-verification" and row.get("shape") == "loaded":
            require(row["argv"][:len(helper)] == helper, "pricing helper differs from its sealed Python invocation")


def audit_capture(path, manifest, manifest_sha, *, full=False):
    """Accept complete replayed custody even when the product verdict is incomplete."""
    from audit_package import processkit_event_evidence
    from laptop_telemetry import replay
    from laptop_analysis import BOOTSTRAP_REPS, analyze

    path = Path(path).resolve(strict=True)
    capture_size(path)
    schedule = build_schedule()
    require((path / "schedule.json").read_bytes() == schedule_bytes(schedule)
            and manifest["schedule_sha256"] == schedule_sha256(schedule),
            "capture and manifest schedule differ from regenerated sequence")
    launch = parse_json((path / "launch.json").read_bytes())
    worker_identity, nonce = bind_worker_launch(
        launch, jsonl(path / "telemetry.jsonl"), manifest, manifest_sha)
    root = path / "campaign" / manifest["run_id"]
    require(root.is_dir() and root.resolve() == root, "sealed campaign namespace is absent")
    meta = parse_json((root / "run.json").read_bytes())
    require(meta.get("run_id") == manifest["run_id"]
            and meta.get("laptop_manifest_sha256") == manifest_sha
            and meta.get("plan_mode") == MODE and meta.get("schedule_sha256") == manifest["schedule_sha256"]
            and meta.get("provisional") is False and meta.get("reduced") is False
            and meta.get("slot_isolation") == harness.WINDOWS_SLOT_ISOLATION
            and meta.get("config_path") == manifest["files"]["config"]["path"]
            and meta.get("config_sha256") == manifest["files"]["config"]["sha256"],
            "stock metadata differs from the sealed execution")
    expected_order = [slot_key(row.slot) for row in schedule]
    require(meta.get("order") == expected_order and meta.get("slots_planned") == len(schedule)
            and meta.get("slots_run") == len(schedule), "campaign population is incomplete")
    for arm in PRODUCT_ARMS:
        require(meta["fingerprints"].get(arm) == manifest["files"][arm],
                f"campaign executed an unbound artifact: {arm}")
    require(meta.get("competitor_identity_mismatches") == [], "rival identity check failed")
    digests = {name: file_hash(root / name) for name in CAMPAIGN_FILES}
    observations = jsonl(root / "observations.jsonl")
    pricing = jsonl(root / "pricing-observations.jsonl")
    require(Counter(row.get("kind") for row in observations) == check_plan([row.slot for row in schedule])
            and len(pricing) == 54, "measured, warmup, control, or pricing population differs")

    problems = harness.check_run_package(root)
    require(not problems, f"stock package replay failed: {problems[:12]}")
    settlements = bind_worker_items(path, schedule, observations, pricing)
    bind_price_jobs(path / "pricing-membership.jsonl", pricing, settlements,
                    manifest, manifest_sha, worker_identity, nonce)
    stream_hashes = bind_streams(path, observations)
    events, absent = processkit_event_evidence(
        root, observations,
        recorded_root=Path(manifest["capture"]) / "campaign" / manifest["run_id"] / "processkit-events",
    )
    telemetry = replay(path, groups=group_plan(schedule), manifest_sha=manifest_sha,
                       image_policy=manifest["image_policy"])
    require({row["index"] for row in telemetry["cheap_observer_overlap"]} == set(
        range(len(schedule) + len(pricing))
    ), "one or more settled timing receipts lacks overlap accounting")
    require(all(file_hash(root / name) == digest for name, digest in digests.items()),
            "stock package changed during independent replay")
    analysis = analyze(observations, telemetry["observer_spans_ns"],
                       repetitions=BOOTSTRAP_REPS if full else 0)
    return {
        "schema": "ptg-laptop-182-001-a-audit-v1", "complete": True,
        "manifest_sha256": manifest_sha, "run_id": manifest["run_id"],
        "slots": len(schedule), "pricing_rows": len(pricing),
        "campaign_status": meta["campaign_status"],
        "standing_loss_count": meta["standing_loss_count"],
        "freeze_gate_fail_count": meta["freeze_gate_fail_count"],
        "forced_slot_drains": sum(row["slot_job"]["terminated"] for row in observations),
        "processkit_event_files": len(events),
        "processkit_parser_refusals_without_event": absent,
        "stock_files_sha256": digests, "processkit_events_sha256": events,
        "retained_streams_sha256": stream_hashes, "telemetry": telemetry, "analysis": analysis,
    }

