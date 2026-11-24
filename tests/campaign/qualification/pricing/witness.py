# SPDX-License-Identifier: Apache-2.0
"""Exercise and independently replay the frozen native pricing Job witness."""

import argparse
from contextlib import ExitStack
import hashlib
import os
from pathlib import Path
import secrets
import subprocess
import sys
import time

NS = 1_000_000_000
CAP_S = 300
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


def run_worker(args, manifest):
    from envcase import suppress_windows_error_dialogs
    from harness import load_config
    from laptop_barrier import BarrierClient, BarrierError
    from laptop_schedule import build_schedule
    from laptop_worker import WorkerExecution
    import pricing

    suppress_windows_error_dialogs()
    config = load_config(manifest["inputs"]["config"])
    require(Path(config["arms"]["guard"]["executable"]).samefile(manifest["files"]["guard"]["path"]),
            "pricing configuration points to a different guard")
    with (args.output / "worker.jsonl").open("x", encoding="utf-8") as worker, (
        args.output / "pricing.jsonl"
    ).open("x", encoding="utf-8") as prices, BarrierClient(args.pipe, args.nonce, args.manifest_sha) as gate:
        witness = WorkerExecution(args.output, build_schedule(), gate, worker, prices, args.manifest_sha)
        witness.groups = GROUPS
        try:
            if args.negative:
                witness.checkpoint(0, LABELS[0])
                try:
                    gate.pricing_settlement(0, 0)
                except (BarrierError, OSError) as error:
                    witness.emit("pricing_refused", group=0, index=0, client_error=type(error).__name__)
                else:
                    raise ValueError("pricing request unexpectedly received a successful acknowledgment")
                require(witness.last_index == -1 and witness.active is None,
                        "failed pricing attempt advanced its worker cursor")
            else:
                require(tuple(pricing.PROBES) == PROBES and pricing.PROBE_REPS == 9,
                        "frozen stock pricing recipe changed")
                original = pricing.PROBE_REPS
                try:
                    # Select one replicate in this private worker only. All shape
                    # construction, calls, timing and recorder hooks are stock.
                    pricing.PROBE_REPS = 1
                    records = pricing.run_pricing_probes(config, fingerprint(manifest), laptop=witness)
                finally:
                    pricing.PROBE_REPS = original
                require(len(records) == 6 and witness.last_index == 5,
                        "stock qualification did not retain all six pricing rows")
                witness.checkpoint(3, "tail")
            witness.emit("worker_terminal", success=True, settled_through=witness.last_index)
        except Exception as error:
            witness.emit("worker_terminal", success=False, settled_through=witness.last_index,
                         error=f"{type(error).__name__}: {error}")
            raise


def receipt_matches(worker_proc, job, worker_ident, extra, extra_ident):
    from laptop_barrier import process_in_job

    require(process_in_job(worker_proc._handle, job._handle) == worker_ident,
            "original worker phase Job identity changed")
    if extra is not None:
        require(process_in_job(extra._handle, job._handle) == extra_ident,
                "owned extra member lost its retained identity")
        try:
            extra.wait(0)
        except subprocess.TimeoutExpired:
            return
        raise ValueError("owned extra member terminated before membership proof")


def supervise(args, manifest, launch):
    from envcase import suppress_windows_error_dialogs
    from laptop_barrier import BarrierError, BarrierServer, append_row, process_in_job
    from laptop_telemetry import INSTRUMENTATION_WORKER_FLAGS, Observer, held_worker_launch
    from windows_slot_job import CLEANUP_EXIT_CODE, SlotJob

    suppress_windows_error_dialogs()
    output = args.output
    output.mkdir()
    deadline = time.perf_counter_ns() + CAP_S * NS
    nonce = secrets.token_hex(32)
    pipe = launch.worker_pipe_name(nonce)
    source = Path(__file__).resolve()
    argv = [manifest["files"]["python"]["path"], "-I", "-B", "-X", "utf8", str(source),
            "worker", "--stage", str(args.stage), "--manifest", str(args.manifest),
            "--manifest-sha", args.manifest_sha, "--source-sha", args.source_sha,
            "--output", str(output), "--pipe", pipe, "--nonce", nonce]
    if args.negative:
        argv.append("--negative")
    original_refs = [*launch.iter_held_refs(manifest), launch.file_identity(args.manifest, args.manifest_sha),
                     launch.file_identity(source, args.source_sha)]
    with ExitStack() as stack:
        held = [launch.windows_read_hold(stack, ref) for ref in original_refs]
        ancestry = launch.hold_guard_ancestry(stack, manifest)
        journal = stack.enter_context((output / "supervisor.jsonl").open("xb"))
        listings = stack.enter_context((output / "inventory.jsonl").open("xb"))
        stdout = stack.enter_context((output / "worker.stdout").open("xb"))
        stderr = stack.enter_context((output / "worker.stderr").open("xb"))
        launch.write_exclusive(output / "recipe.json", {
            "manifest_sha256": args.manifest_sha, "source_sha256": args.source_sha,
            "negative": args.negative, "guard_ancestry": ancestry.proof, "worker_argv": argv,
            "worker_creation_flags": INSTRUMENTATION_WORKER_FLAGS,
            "supervisor_pid": os.getpid(), "nonce": nonce, "groups": [list(g) for g in GROUPS],
            "deadline_ns": deadline, "qualification_scope": "one repetition per stock pricing probe",
        })

        def emit(kind, **fields):
            return append_row(journal, {"kind": kind, **fields})

        emit("started", guard_ancestry=ancestry.proof, observer_pid=os.getpid(),
             manifest_sha256=args.manifest_sha, source_sha256=args.source_sha)
        observer = Observer(output, journal, listings, ancestry, manifest["image_policy"])
        server = stack.enter_context(BarrierServer(
            pipe, nonce, args.manifest_sha, GROUPS, output / "barrier.jsonl",
            lambda job, worker_id, group, label, requested: observer.census(
                "during", group, job, requested, identity=worker_id, label=label)))
        job = SlotJob()
        stack.callback(job.close)
        extra = None
        extra_id = None
        try:
            if args.negative:
                fixture = manifest["files"]["fixture"]["path"]
                extra = job.launch([fixture, "sleep", "30"], INSTRUMENTATION_WORKER_FLAGS,
                                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL, cwd=manifest["working_directory"])
                stack.callback(extra.close)
                extra_id = process_in_job(extra._handle, job._handle)
                emit("extra_member", identity=extra_id, creation_flags=INSTRUMENTATION_WORKER_FLAGS,
                     fixture_sha256=manifest["files"]["fixture"]["sha256"])
            dispatch = time.perf_counter_ns()
            worker = job.launch(argv, INSTRUMENTATION_WORKER_FLAGS,
                                stdin=subprocess.DEVNULL,
                                stdout=stdout, stderr=stderr, cwd=manifest["working_directory"])
            stack.callback(worker.close)
            proof = held_worker_launch(job, worker, argv, manifest["files"]["python"])
            emit("worker_dispatched", dispatch_ns=dispatch,
                 creation_flags=INSTRUMENTATION_WORKER_FLAGS, **proof)
            worker_id = identity(proof)
            receipt_matches(worker, job, worker_id, extra, extra_id)
            finished = server.bind_worker(job, worker, dispatch, deadline)
            require(finished.wait(max(0, deadline - time.perf_counter_ns()) / NS),
                    "phase Job witness did not terminate on its absolute deadline")
            if args.negative:
                require(isinstance(server.error, BarrierError) and str(server.error) == REFUSAL
                        and len(server.authorizations) == 1 and len(server.ack_settled_ns) == 1
                        and server.pricing_authorizations == [] and server.pricing_ack_settled_ns == [],
                        "negative phase Job did not end on the exact refusal")
                emit("membership_refusal", message=REFUSAL, accepted_pricing_count=0)
            else:
                server.check()
                require(len(server.authorizations) == 4 and len(server.ack_settled_ns) == 4
                        and len(server.pricing_authorizations) == len(server.pricing_ack_settled_ns) == 6,
                        "phase Job witness population is incomplete")
            worker_code = worker.wait(4)
            require(worker_code == 0, "private pricing worker did not exit normally")
            if extra is not None:
                require(process_in_job(extra._handle, job._handle) == extra_id,
                        "original extra ownership witness did not survive to cleanup")
            drain = job.drain()
            extra_code = extra.wait(1) if extra is not None else None
            emit("drain", worker_code=worker_code, extra_code=extra_code, job=drain,
                 barrier_ack_settled_ns=server.ack_settled_ns,
                 pricing_ack_settled_ns=server.pricing_ack_settled_ns)
            if args.negative:
                require(drain == {"active_before": 1, "terminated": True, "active_after": 0}
                        and extra_code == CLEANUP_EXIT_CODE, "owned extra did not drain via retained Job")
            else:
                require(drain == EMPTY and extra_code is None, "positive phase Job did not drain naturally")
            for ref, (path, observed) in zip(original_refs, held, strict=True):
                current, ended = launch.windows_read_hold(stack, ref)
                require(current == path and os.path.samestat(observed, ended),
                        "qualification input identity changed at its terminal check")
            require(time.perf_counter_ns() < deadline, "native witness cap expired")
            emit("terminal", success=True)
        except Exception as error:
            emit("failure", reason=f"{type(error).__name__}: {error}")
            try:
                emit("abort_drain", job=job.drain())
            except Exception as cleanup:
                emit("abort_drain_error", reason=f"{type(cleanup).__name__}: {cleanup}")
            emit("terminal", success=False)
            raise


def one(rows, kind):
    values = [row for row in rows if row["kind"] == kind]
    require(len(values) == 1, f"missing or repeated {kind}")
    return values[0]


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
    require(recipe["manifest_sha256"] == args.manifest_sha and recipe["source_sha256"] == args.source_sha
            and recipe["groups"] == [list(item) for item in GROUPS]
            and recipe["negative"] is args.negative, "raw pricing witness recipe differs")
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
            and started["source_sha256"] == args.source_sha,
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
        admitted = census_admissions(witness["rows"], proof, witness["held_guards_before"],
                                     witness["held_guards_after"])
        require(admitted == witness["ancestry_admissions"]
                and census_allowed(witness["rows"], admitted, ALLOWED),
                "native census ancestry admission differs from independently derived roles")
        require(witness["guard_pid"] == primary["pid"]
                and witness["guard_created_ticks"] == primary["creation_filetime"],
                "census observer primary guard differs from held ancestry")
        for process in witness["rows"]:
            state = process["state"]
            if process["pid"] != primary["pid"]:
                require(state == classify(
                    process["name"], process["image_sha256"], enclosing=False,
                    member=state == "campaign_member", compiler_names=COMPILERS,
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
    launch.write_exclusive(args.receipt, {"schema": "native-pricing-job-witness-v1",
                                         "manifest_sha256": args.manifest_sha,
                                         "source_sha256": args.source_sha, **result})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("capture", "worker", "replay"))
    parser.add_argument("--stage", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--manifest-sha", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--negative", action="store_true")
    parser.add_argument("--pipe")
    parser.add_argument("--nonce")
    parser.add_argument("--receipt", type=Path)
    args = parser.parse_args()
    require(sys.platform == "win32" and sys.flags.isolated and sys.flags.dont_write_bytecode,
            "native witness requires Windows and Python -I -B")
    args.stage = args.stage.resolve(strict=True)
    args.manifest = args.manifest.resolve(strict=True)
    args.output = args.output.resolve()
    require(args.manifest.parent == args.stage and not args.output.is_relative_to(args.stage),
            "frozen stage and private capture must be separate")
    tests = args.stage / "tests"
    require(not any((directory / "__pycache__").exists() for directory in (tests, tests / "campaign")),
            "staged bytecode cache prohibited")
    sys.path.insert(0, str(tests / "campaign"))
    import laptop_launch as launch

    source = Path(__file__).resolve()
    launch.file_identity(source, args.source_sha)
    manifest = launch.load_manifest(args.manifest, args.manifest_sha)
    launch.code_refs(manifest)
    require(Path(sys.executable).samefile(manifest["files"]["python"]["path"]),
            "pricing witness Python differs from frozen executable")
    launch.bind_observers(manifest)
    if args.action != "replay":
        current = launch.describe(Path(manifest["inputs"]["description"]["path"]),
                                  Path(manifest["capture"]), manifest["run_id"])
        require(current == manifest, "live qualification inputs differ from frozen pending manifest")
    for key in list(os.environ):
        if key.upper() == "RUST_LOG":
            del os.environ[key]
    if args.action == "capture":
        require(args.pipe is args.nonce is args.receipt is None, "unexpected capture transport")
        supervise(args, manifest, launch)
    elif args.action == "worker":
        require(args.nonce and args.pipe == launch.worker_pipe_name(args.nonce)
                and args.receipt is None and Path.cwd().samefile(manifest["working_directory"]),
                "invalid worker pipe or exact working directory")
        run_worker(args, manifest)
    else:
        require(args.pipe is args.nonce is None and args.receipt is not None, "invalid replay recipe")
        replay(args, manifest, launch)


if __name__ == "__main__":
    main()

