# SPDX-License-Identifier: Apache-2.0
"""Read and independently replay a preserved native pricing Job witness."""

import argparse
from contextlib import ExitStack
import hashlib
import os
from pathlib import Path
import sys

NS = 1_000_000_000
PROBES = ("console-probe-finish", "owner-verification", "resolver-probe-count")
LABELS = tuple(f"pricing:{probe}:0" for probe in PROBES)
GROUPS = tuple((i, label, 2 * i - 1) for i, label in enumerate(LABELS)) + ((3, "tail", 5),)
EMPTY = {"active_before": 0, "terminated": False, "active_after": 0}
REFUSAL = "pricing phase Job does not contain only the exact worker"


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def identity(row):
    return {key: row[key] for key in ("pid", "creation_filetime")}


def fingerprint(manifest):
    ref = manifest["files"]["guard"]
    return {key: ref[key] for key in ("path", "size", "sha256")}


def one(rows, kind):
    values = [row for row in rows if row["kind"] == kind]
    require(len(values) == 1, f"missing or repeated {kind}")
    return values[0]


def bind_gate_worker(gate, expected):
    require(gate.get("identity") == gate.get("worker_before") == gate.get("worker_after") == expected
            and gate.get("client_pid") == expected["pid"],
            "retained barrier worker identity changed around the census")


def bind_census_rounds(witness, gate, *, observer_pid, worker, policy, ancestry):
    from inventory_gate import COMPILERS, WORKLOADS
    from laptop_census import validate_census

    require(witness["phase"] == "during" and witness["index"] == gate["group"]
            and witness["label"] == gate["label"] and witness["identity"] == worker
            and witness["requested_ns"] == gate["requested_ns"] and not witness["errors"]
            and gate["census"]["begin_ns"] == witness["begin_ns"]
            and witness["finished_ns"] <= gate["census"]["end_ns"],
            "native pricing census does not bind the original during gate")
    rounds = witness["census_rounds"]
    derived = validate_census(
        rounds, phase=witness["phase"], requested_ns=gate["requested_ns"],
        observer_pid=observer_pid, worker_identity=worker, image_policy=policy,
        compiler_names=COMPILERS, workload_names=WORKLOADS, ancestry=ancestry,
        held_before=witness["held_guards_before"], held_after=witness["held_guards_after"],
    )
    require(derived["decision"] == "admit" and derived == witness["census_decision"]
            and witness["rows"] == rounds[-1]["rows"]
            and witness["ancestry_admissions"] == derived["ancestry_admissions"][-1]
            and witness["begin_ns"] <= rounds[0]["begin_ns"]
            and rounds[-1]["finished_ns"] <= witness["finished_ns"],
            "native pricing census replacement evidence or final projection differs")


def replay(args, manifest, launch):
    from image_policy import ALLOWED, classify
    from inventory_gate import COMPILERS
    from laptop_ancestry import census_admissions, census_allowed, validate_ancestry
    from laptop_schedule import canonical_bytes
    from laptop_barrier import GROUP_NS, RESERVE_NS
    from laptop_telemetry import (
        INSTRUMENTATION_WORKER_FLAGS, check_pricing_receiver, load_rows, read_pricing_membership,
    )
    from pricing import GUARD_LAUNCHER_MODE, PROBE_TIMEOUT_DETAIL, pricing_stamp_defects
    from windows_slot_job import CLEANUP_EXIT_CODE

    output = args.output
    recipe = launch.parse_json((output / "recipe.json").read_bytes())
    require(recipe["manifest_sha256"] == args.manifest_sha
            and recipe["source_sha256"] == args.capture_source_sha
            and recipe["groups"] == [list(item) for item in GROUPS]
            and recipe["negative"] is args.negative, "raw pricing witness recipe differs")
    nonce = recipe["nonce"]
    expected_argv = [
        manifest["files"]["python"]["path"], "-I", "-B", "-X", "utf8", str(args.capture_source),
        "worker", "--stage", str(args.stage), "--manifest", str(args.manifest),
        "--manifest-sha", args.manifest_sha, "--source-sha", args.capture_source_sha,
        "--output", str(output), "--pipe", launch.worker_pipe_name(nonce), "--nonce", nonce,
    ]
    if args.negative:
        expected_argv.append("--negative")
    require(recipe["worker_argv"] == expected_argv,
            "recorded worker invocation differs from the preserved capture source")
    worker = [row for row, _ in load_rows(output / "worker.jsonl")]
    supervisor = [row for row, _ in load_rows(output / "supervisor.jsonl")]
    gates = load_rows(output / "barrier.jsonl")
    inventories = {digest: row for row, digest in load_rows(output / "inventory.jsonl")}
    release = [row for row in worker if row["kind"] == "group_release"]
    settled = [row for row in worker if row["kind"] == "item_settled"]
    state = one(worker, "worker_terminal")
    drain = one(supervisor, "drain")
    require(one(supervisor, "terminal")["success"] is True and state["success"] is True
            and drain["worker_code"] == 0, "incomplete pricing qualification")
    started = one(supervisor, "started")
    require(started["guard_ancestry"] == recipe["guard_ancestry"]
            and started["observer_pid"] == recipe["supervisor_pid"]
            and started["manifest_sha256"] == args.manifest_sha
            and started["source_sha256"] == args.capture_source_sha,
            "started guard ancestry does not bind recipe")
    proof = recipe["guard_ancestry"]
    policy = manifest["image_policy"]
    validate_ancestry(proof, policy, recipe["supervisor_pid"])
    primary = proof["roles"][0]
    dispatch = one(supervisor, "worker_dispatched")
    expected_id = identity(dispatch)
    membership = read_pricing_membership(
        output / "pricing-membership.jsonl", expected_id, args.manifest_sha, recipe["nonce"],
        accepted=not args.negative,
    )

    def deadline_for(member, group):
        return min(member["requested_ns"] + NS, gates[group][0]["requested_ns"] + GROUP_NS,
                   recipe["deadline_ns"] - RESERVE_NS)
    python = manifest["files"]["python"]
    require(dispatch["creation_flags"] == recipe["worker_creation_flags"] == INSTRUMENTATION_WORKER_FLAGS
            and dispatch["job_member"] is True and dispatch["image_sha256"] == python["sha256"]
            and Path(dispatch["image_path"]).samefile(python["path"])
            and dispatch["worker_recipe_sha256"] == hashlib.sha256(
                canonical_bytes(recipe["worker_argv"])).hexdigest(),
            "worker original-handle and sealed recipe do not match")
    require(len(gates) == len(release) == len(inventories) == (1 if args.negative else 4),
            "barrier and census proof population differ")
    require(len(drain["barrier_ack_settled_ns"]) == len(gates), "missed exact native barrier ACK")
    require(args.negative or len(settled) == 6, "positive worker settlement population differs")
    for group, ((gate, digest), client, ack) in enumerate(zip(
        gates, release, drain["barrier_ack_settled_ns"], strict=True,
    )):
        witness = inventories[gate["census"]["sha256"]]
        bind_gate_worker(gate, expected_id)
        bind_census_rounds(witness, gate, observer_pid=recipe["supervisor_pid"],
                           worker=expected_id, policy=policy, ancestry=proof)
        admitted = census_admissions(witness["rows"], proof, witness["held_guards_before"],
                                     witness["held_guards_after"])
        require(admitted == witness["ancestry_admissions"]
                and census_allowed(witness["rows"], admitted, ALLOWED),
                "native census ancestry admission differs from independently derived roles")
        require(witness["guard_pid"] == primary["pid"]
                and witness["guard_created_ticks"] == primary["creation_filetime"],
                "census observer primary guard differs from held ancestry")
        for process in witness["rows"]:
            process_state = process["state"]
            if process["pid"] != primary["pid"]:
                require(process_state == classify(
                    process["name"], process["image_sha256"], enclosing=False,
                    member=process_state == "campaign_member", compiler_names=COMPILERS,
                    veto_sha256=set(policy["veto_sha256"]),
                ), "raw pricing census classification differs")
        previous_settlement = settled[2 * group - 1]["settled_ns"] if group else dispatch["dispatch_ns"]
        require((gate["group"], gate["label"], gate["settled_through"]) == GROUPS[group]
                and client["group"] == group and client["barrier_sha256"] == digest
                and expected_id == gate["identity"] == witness["identity"]
                and gate["client_pid"] == expected_id["pid"] and gate["nonce"] == recipe["nonce"]
                and gate["manifest_sha256"] == args.manifest_sha
                and previous_settlement <= client["requested_ns"] <= gate["requested_ns"]
                <= gate["admitted_ns"] <= witness["begin_ns"] <= witness["finished_ns"]
                <= gate["census"]["end_ns"] <= gate["authorized_ns"] <= ack
                <= gate["requested_ns"] + NS and gate["authorized_ns"] <= client["release_ns"]
                <= gate["requested_ns"] + NS, "native group permit did not bind worker/census")
    if args.negative:
        require(len(membership) == 1 and settled == [] and state["settled_through"] == -1
                and (output / "pricing.jsonl").stat().st_size == 0
                and drain["pricing_ack_settled_ns"] == []
                and one(worker, "pricing_refused")["index"] == 0
                and one(supervisor, "membership_refusal")["message"] == REFUSAL,
                "negative phase Job witness advanced a pricing slot")
        row, _, attempts = membership[0]
        extra = one(supervisor, "extra_member")
        expected_pids = {expected_id["pid"], extra["identity"]["pid"]}
        require(row["group"] == row["index"] == 0 and row["outcome"] == "persistent-extra"
                and row["authorized_ns"] is None and row["deadline_ns"] == deadline_for(row, 0)
                and release[0]["release_ns"] <= row["requested_ns"] <= row["decision_ns"]
                <= row["deadline_ns"] and attempts
                and all(query["outcome"] == "extra-members"
                        and set(query["witness"]["pids"]) == expected_pids for query in attempts)
                and extra["creation_flags"] == INSTRUMENTATION_WORKER_FLAGS
                and extra["fixture_sha256"] == manifest["files"]["fixture"]["sha256"]
                and drain["job"] == {"active_before": 1, "terminated": True, "active_after": 0}
                and drain["extra_code"] == CLEANUP_EXIT_CODE,
                "native negative witness did not prove the exact persistent owned extra and cleanup")
        result = {"accepted": 0, "rejected": 1, "pricing_rows": 0,
                  "refusal_attempt_count": len(attempts),
                  "request_to_refusal_ns": row["decision_ns"] - row["requested_ns"]}
    else:
        pricing = load_rows(output / "pricing.jsonl")
        accepted = drain["pricing_ack_settled_ns"]
        require(len(pricing) == len(settled) == len(membership) == len(accepted) == 6
                and state["settled_through"] == 5 and drain["job"] == EMPTY,
                "positive native witness does not cover all six rows")
        expected = {"accepted": True, "before": expected_id, "after": expected_id,
                    "assigned": 1, "listed": 1, "pids": [expected_id["pid"]]}
        rows = []
        begins = [row for row in worker if row["kind"] == "item_begin"]
        require(len(begins) == 6, "price begin population differs")
        settlement_report = {}
        for index, ((raw, _), (member, digest, attempts), item, ack) in enumerate(zip(
            pricing, membership, settled, accepted, strict=True,
        )):
            row = raw["observation"]
            group, position = divmod(index, 2)
            begin = begins[index]
            lower = settled[index - 1]["settled_ns"] if position else release[group]["release_ns"]
            probe, shape = PROBES[group], ("loaded", "baseline")[position]
            require(row["kind"] == "pricing-probe" and row["outcome"] == "ok" and row["exit_code"] == 0
                    and row.get("detail") != PROBE_TIMEOUT_DETAIL
                    and (row["probe"], row["rep"], row["shape"]) == (probe, 0, shape)
                    and row["guard_fingerprint"] == fingerprint(manifest)
                    and raw["index"] == begin["index"] == item["index"] == member["index"] == index
                    and member["group"] == begin["group"] == item["group"] == group
                    and begin["identity"] == item["identity"] == ["pricing", probe, 0, shape]
                    and member["kind"] == "pricing-settlement" and member["nonce"] == recipe["nonce"]
                    and member["manifest_sha256"] == args.manifest_sha and member["witness"] == expected
                    and item["pricing_membership_sha256"] == digest
                    and lower <= begin["begun_ns"] <= item["timing_start_ns"] <= item["timing_end_ns"]
                    and member["deadline_ns"] == deadline_for(member, group)
                    and member["authorized_ns"] <= ack <= member["deadline_ns"],
                    "pricing row does not bind one exact post-return phase Job witness")
            check_pricing_receiver(member, item)
            if (probe, shape) == ("owner-verification", "loaded"):
                helper = [python["path"], "-I", "-B", manifest["sources"]["pricing.py"]["path"],
                          GUARD_LAUNCHER_MODE]
                require(row["argv"][:len(helper)] == helper, "recording launcher substitution")
            rows.append(row)
            settlement_report[f"{probe}:{shape}"] = {
                "attempt_counts": [len(attempts)],
                "post_return_settlement_ns": [item["settled_ns"] - item["timing_end_ns"]],
            }
        require(pricing_stamp_defects(rows, platform="win32") == {}, "stock pricing stamp replay failed")
        result = {"accepted": 6, "rejected": 0, "pricing_rows": 6, "stock_stamp_replay": True,
                  "pricing_settlement": settlement_report}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("replay",))
    parser.add_argument("--stage", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--manifest-sha", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--capture-source", required=True, type=Path)
    parser.add_argument("--capture-source-sha", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--negative", action="store_true")
    parser.add_argument("--receipt", required=True, type=Path)
    args = parser.parse_args()
    require(sys.platform == "win32" and sys.flags.isolated and sys.flags.dont_write_bytecode,
            "native witness replay requires Windows and Python -I -B")
    args.stage = args.stage.resolve(strict=True)
    args.manifest = args.manifest.resolve(strict=True)
    args.output = args.output.resolve(strict=True)
    args.receipt = args.receipt.resolve()
    args.capture_source = args.capture_source.resolve(strict=True)
    require(args.manifest.parent == args.stage and not args.output.is_relative_to(args.stage),
            "frozen stage and private capture must be separate")
    require(not args.receipt.is_relative_to(args.output) and not args.receipt.is_relative_to(args.stage),
            "replay receipt must be outside the held stage and preserved capture")
    tests = args.stage / "tests"
    require(not any((directory / "__pycache__").exists() for directory in (tests, tests / "campaign")),
            "staged bytecode cache prohibited")
    sys.path.insert(0, str(tests / "campaign"))
    import laptop_launch as launch

    source = Path(__file__).resolve()
    require(not source.samefile(args.capture_source) and args.source_sha != args.capture_source_sha,
            "capture and replay sources must have distinct identities")
    refs = {
        "capture_source": launch.file_identity(args.capture_source, args.capture_source_sha),
        "replay_source": launch.file_identity(source, args.source_sha),
    }
    manifest = launch.load_manifest(args.manifest, args.manifest_sha)
    launch.code_refs(manifest)
    require(Path(sys.executable).samefile(manifest["files"]["python"]["path"]),
            "pricing witness Python differs from frozen executable")
    launch.bind_observers(manifest)
    with ExitStack() as stack:
        held = {role: launch.windows_read_hold(stack, ref) for role, ref in refs.items()}
        result = replay(args, manifest, launch)
        custody = {}
        for role, ref in refs.items():
            original, before = held[role]
            path, after = launch.windows_read_hold(stack, ref)
            require(path == original and os.path.samestat(before, after),
                    "held replay or capture source identity changed")
            custody[role] = {**ref, "device": before.st_dev, "file": before.st_ino}
        launch.write_exclusive(args.receipt, {
            "schema": "native-pricing-job-witness-v1",
            "manifest_sha256": args.manifest_sha, "source_sha256": args.capture_source_sha,
            "replay_source_sha256": args.source_sha, **custody, **result,
        })


if __name__ == "__main__":
    main()
