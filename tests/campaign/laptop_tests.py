# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Offline controls for laptop 182.001 order, overlap and retention."""

import hashlib
import tempfile
import time
import unittest
from argparse import Namespace
from collections import Counter
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

from laptop_analysis import analyze
from laptop_ancestry import census_admissions, census_allowed, role_instances, validate_ancestry
from laptop_audit import bind_price_jobs, bind_streams, bind_worker_items, bind_worker_launch
import laptop_barrier
import laptop_launch
import laptop_worker
import pricing
from laptop_schedule import SLOT_GROUPS, TAIL_GROUP, build_schedule, canonical_bytes, schedule_bytes
from laptop_worker import WorkerExecution, append_record, group_plan
from plan import build_plan


def ancestry_fixture():
    primary = {"path": "C:/sealed/process-tree-guard.exe", "sha256": "d" * 64}
    tool = {"path": "C:/sealed/process-guard.exe", "sha256": "e" * 64}
    pins = {"primary_guard": primary, "tool_guard": tool}
    policy = {
        "enclosing_guards": pins, "veto_sha256": [primary["sha256"], tool["sha256"]],
        "pinned_basenames": ["process-guard.exe", "process-tree-guard.exe"],
    }
    chain = [
        {"pid": 19, "parent_pid": 14, "path": "C:/sealed/python.exe",
         "name": "python.exe", "creation_filetime": 190},
        {"pid": 14, "parent_pid": 16, "path": primary["path"],
         "name": "process-tree-guard.exe", "creation_filetime": 140},
        {"pid": 16, "parent_pid": 13, "path": "C:/sealed/powershell.exe",
         "name": "powershell.exe", "creation_filetime": 135},
        {"pid": 13, "parent_pid": 10, "path": tool["path"],
         "name": "process-guard.exe", "creation_filetime": 130},
    ]
    proof = {"chain": chain, "snapshot_upper_ticks": 195, "roles": [
        {**chain[1], "role": "primary_guard", "image_sha256": primary["sha256"]},
        {**chain[-1], "role": "tool_guard", "image_sha256": tool["sha256"]},
    ]}
    return pins, policy, proof


class _Gate:
    last_receipt_sha256 = "a" * 64

    def __init__(self):
        self.requests = []

    def checkpoint(self, ordinal, label, through):
        self.requests.append((ordinal, label, through))
        return time.perf_counter_ns()


class LaptopContractTests(unittest.TestCase):
    def test_describe_only_manifest_cannot_enter_worker_or_load_shadow_observer(self):
        with tempfile.TemporaryDirectory(prefix="ptg-laptop-offline-") as tmp:
            capture = Path(tmp) / "absent"
            manifest = {"capture": str(capture), "native_ready": False}
            args = Namespace(command="worker", manifest=Path(tmp) / "manifest.json",
                             approved_sha="a" * 64, pipe="unused", nonce="b" * 64)
            with (patch.object(laptop_launch, "load_manifest", return_value=manifest),
                  patch.object(laptop_launch, "verify_live_manifest", return_value=manifest),
                  patch.object(laptop_worker, "run_worker") as worker):
                with self.assertRaisesRegex(ValueError, "native qualification"):
                    laptop_launch.command(args)
                worker.assert_not_called()
                self.assertFalse(capture.exists())
            observer = Path(tmp) / "observer"
            observer.mkdir()
            (observer / "__pycache__").mkdir()
            manifest["observer_sources"] = {"quiet_gate.py": {"path": str(observer / "quiet_gate.py")}}
            with self.assertRaisesRegex(ValueError, "unbound inputs or a bytecode cache"):
                laptop_launch.bind_observers(manifest)

    def test_describe_vetoes_the_current_and_historical_fixture_images(self):
        historical = "d55d6b947c921b9d28e803a5057dac51e8df044fbdd7106b88880292c4c47a2f"
        current = "346ca22737701fb5e2a7188d0ad9aadca07a496984ca83c7e34747997324f8a3"
        with tempfile.TemporaryDirectory(prefix="ptg-laptop-offline-") as tmp:
            root = Path(tmp)
            names = (*laptop_launch.BUNDLE_PATHS, "primary_guard", "tool_guard", "config", "python")
            files = {
                name: {"path": str(root / name), "sha256": f"{idx:064x}"}
                for idx, name in enumerate(names, 1)
            }
            files["fixture"] = {"path": str(root / "testkit.exe"), "sha256": current}
            with (patch.object(laptop_launch, "isolated_inputs", return_value=({}, files, {})),
                  patch.object(laptop_launch, "normalized_environment", return_value="a" * 64)):
                manifest = laptop_launch.describe(root / "inputs.json", root / "capture",
                                                  "laptop-182-001-a-offline")
        self.assertEqual(manifest["files"]["fixture"]["sha256"], current)
        self.assertEqual(laptop_launch.LITERAL_PINS["fixture"], current)
        self.assertIn("testkit.exe", manifest["image_policy"]["pinned_basenames"])
        self.assertTrue({current, historical} <= set(manifest["image_policy"]["veto_sha256"]))

    def test_pricing_keeps_stock_rows_and_joins_post_timing_job_witness(self):
        original = {"outcome": "exit-mismatch"}

        def measured(_config, **kwargs):
            self.assertTrue(kwargs["defer_timeout_cleanup"])
            self.assertEqual(kwargs["guard_launcher"][1:3], ["-I", "-B"])
            self.assertEqual(kwargs["env"], {"Path": "one;two"})
            self.assertEqual(kwargs["cwd"], "held-cwd")
            return original

        with patch.object(pricing, "_time_guard_run", side_effect=measured):
            record = pricing._laptop_price({}, {
                "guard_record_path": "held-receipt", "env": {"Path": "one;two"}, "cwd": "held-cwd",
            })
        self.assertIs(record, original)
        self.assertEqual(record, {"outcome": "exit-mismatch"})

        manifest_sha, nonce = "a" * 64, "b" * 64
        worker = {"pid": 70, "creation_filetime": 100}
        manifest = {"files": {"python": {"path": "python"}},
                    "sources": {"pricing.py": {"path": "pricing"}}}
        membership = {
            "kind": "pricing-settlement", "nonce": nonce, "manifest_sha256": manifest_sha,
            "group": SLOT_GROUPS, "index": 1940, "requested_ns": 11, "authorized_ns": 12,
            "witness": {"accepted": True, "before": worker, "after": worker,
                        "assigned": 1, "listed": 1, "pids": [70]},
        }
        settlement = {"group": SLOT_GROUPS, "index": 1940, "timing_end_ns": 10, "settled_ns": 13}
        with tempfile.TemporaryDirectory(prefix="ptg-laptop-offline-") as tmp:
            receipt_path = Path(tmp) / "pricing-membership.jsonl"
            def save_witness():
                raw = canonical_bytes(membership)
                receipt_path.write_bytes(raw)
                settlement["pricing_membership_sha256"] = hashlib.sha256(raw).hexdigest()

            save_witness()
            bind_price_jobs(receipt_path, [record], [settlement], manifest, manifest_sha, worker, nonce)
            membership["witness"]["pids"] = [70, 71]
            save_witness()
            with self.assertRaisesRegex(ValueError, "sole held worker"):
                bind_price_jobs(receipt_path, [record], [settlement], manifest, manifest_sha, worker, nonce)

    def test_pipe_client_uses_integer_nulls_for_winapi_create_file(self):
        win = SimpleNamespace(GENERIC_READ=0x80000000, GENERIC_WRITE=0x40000000,
                              OPEN_EXISTING=3, FILE_FLAG_OVERLAPPED=0x40000000,
                              PIPE_READMODE_MESSAGE=2, CreateFile=Mock(return_value=51),
                              SetNamedPipeHandleState=Mock(), CloseHandle=Mock())

        def parent_pid(handle, result):
            self.assertEqual(handle, 51)
            result._obj.value = 82
            return True

        module = (win, SimpleNamespace(GetNamedPipeServerProcessId=parent_pid), None)
        nonce = "b" * 64
        name = laptop_launch.worker_pipe_name(nonce)
        with (patch.object(laptop_barrier, "native", return_value=module),
              patch.object(laptop_barrier.os, "getppid", return_value=82)):
            with laptop_barrier.BarrierClient(name, nonce, "a" * 64):
                win.CreateFile.assert_called_once_with(
                    name, 0xC0000000, 0, 0, win.OPEN_EXISTING, win.FILE_FLAG_OVERLAPPED, 0)
                call = win.CreateFile.call_args.args
                self.assertIs(type(call[3]), int)
                self.assertIs(type(call[6]), int)
                win.SetNamedPipeHandleState.assert_called_once_with(51, win.PIPE_READMODE_MESSAGE, None, None)
        win.CloseHandle.assert_called_once_with(51)

    def test_pricing_witness_requires_the_exact_live_worker_alone(self):
        job = SimpleNamespace(_handle=100)
        worker = SimpleNamespace(_handle=200)
        identity = {"pid": 70, "creation_filetime": 150}

        def native_query(handle, info_class, result, _size, _returned):
            self.assertEqual((handle, info_class), (100, 3))
            data = result._obj
            data.assigned = data.listed = len(ids)
            for index, pid in enumerate(ids):
                data.pids[index] = pid
            return True

        with (patch("windows_slot_job.QueryInformationJobObject", side_effect=native_query),
              patch.object(laptop_barrier, "process_in_job", return_value=identity) as exact):
            ids = [70]
            receipt = laptop_barrier.describe_pricing_membership(job, worker, identity)
            self.assertEqual(receipt["pids"], [70])
            self.assertTrue(receipt["accepted"])
            self.assertEqual(exact.call_count, 2)
            ids = [70, 80]
            receipt = laptop_barrier.describe_pricing_membership(job, worker, identity)
            self.assertEqual(receipt["pids"], [70, 80])
            self.assertFalse(receipt["accepted"])

    def test_price_timeout_keeps_failed_row_without_requesting_membership_or_settlement(self):
        rows = build_schedule()
        with tempfile.TemporaryDirectory(prefix="ptg-laptop-offline-") as tmp:
            capture = Path(tmp)
            gate = _Gate()  # No pricing_settlement method. Timeout must never request one.
            with (capture / "laptop-worker.jsonl").open("x", encoding="utf-8") as journal, (
                capture / "laptop-pricing.jsonl"
            ).open("x", encoding="utf-8") as prices:
                worker = WorkerExecution(capture, rows, gate, journal, prices, "a" * 64)
                worker.last_index = len(rows) - 1
                worker.next_group = SLOT_GROUPS + 1
                worker.before_pricing_slot("console-probe-finish", 0, "loaded")
                worker.after_pricing_slot("console-probe-finish", 0, "loaded")
                with self.assertRaisesRegex(ValueError, "exhausted the finite deadline"):
                    worker.retain_pricing_record({
                        "probe": "console-probe-finish", "rep": 0, "shape": "loaded",
                        "outcome": "infra-failure", "detail": pricing.PROBE_TIMEOUT_DETAIL,
                    })
                self.assertEqual(worker.last_index, len(rows) - 1)
                self.assertIsNotNone(worker.active)
            self.assertIn(pricing.PROBE_TIMEOUT_DETAIL, (capture / "laptop-pricing.jsonl").read_text())
            self.assertNotIn("item_settled", (capture / "laptop-worker.jsonl").read_text())

    def test_outer_guard_admission_preserves_the_raw_veto_for_other_instances(self):
        pins, policy, proof = ancestry_fixture()
        validate_ancestry(proof, policy, 19)
        held = role_instances(proof)
        primary, tool = [
            {"pid": node["pid"], "parent_pid": node["parent_pid"], "name": node["name"],
             "created_ticks": node["creation_filetime"], "snapshot_upper_ticks": 250}
            for node in proof["roles"]
        ]
        primary["state"] = "pinned_enclosing_guard"
        tool.update(state="pinned_workload_outside_job", image_sha256=pins["tool_guard"]["sha256"],
                    image_path=pins["tool_guard"]["path"], alive_after=True)
        rows = [primary, tool]
        allowed = {"pinned_enclosing_guard", "campaign_member", "ordinary"}
        proof_admissions = census_admissions(rows, proof, held, held)
        self.assertEqual(proof_admissions[0]["pid"], tool["pid"])
        self.assertTrue(census_allowed(rows, proof_admissions, allowed))
        self.assertEqual(tool["state"], "pinned_workload_outside_job")
        clone = dict(tool, pid=30)
        self.assertFalse(census_allowed([*rows, clone],
                                        census_admissions([*rows, clone], proof, held, held), allowed))
        with self.assertRaisesRegex(ValueError, "absent from the census"):
            census_admissions([primary, clone], proof, held, held)
        for changed in (dict(tool, parent_pid=999), dict(tool, created_ticks=120)):
            with self.assertRaisesRegex(ValueError, "not the exact held guard"):
                census_admissions([primary, changed], proof, held, held)
        with self.assertRaisesRegex(ValueError, "sole ancestry admission"):
            census_admissions([primary, dict(tool, state="membership_unverified")], proof, held, held)
        with self.assertRaisesRegex(ValueError, "held guards changed"):
            census_admissions(rows, proof, held, [held[0], dict(held[1], creation_filetime=120)])
        proof["chain"][2] = dict(proof["chain"][2], parent_pid=60)
        with self.assertRaisesRegex(ValueError, "broken native parent"):
            validate_ancestry(proof, policy, 19)

    def test_audit_binds_held_worker_image_and_invocation(self):
        digest, nonce, executable = "a" * 64, "b" * 64, "C:/sealed/python.exe"
        pins, policy, proof = ancestry_fixture()
        manifest = {"manifest_path": "C:/sealed/manifest.json", "image_policy": policy, "files": {
            "python": {"path": executable, "sha256": "c" * 64}, **pins,
        }}
        argv = laptop_launch.worker_argv(Path(manifest["manifest_path"]), digest, manifest,
                                        laptop_launch.worker_pipe_name(nonce), nonce)
        launch = {"manifest_sha256": digest, "worker_argv": argv, "supervisor_pid": 19,
                  "guard_ancestry": proof}
        started = {"kind": "started", "manifest_sha256": digest, "barrier_nonce": nonce,
                   "worker_argv": list(argv), "observer_pid": 19, "guard_ancestry": proof,
                   "guard_pid": 14, "guard_created_ticks": 140}
        dispatch = {"kind": "worker_dispatched", "pid": 20, "creation_filetime": 200,
                    "image_path": executable, "image_sha256": "c" * 64, "job_member": True,
                    "worker_recipe_sha256": hashlib.sha256(canonical_bytes(argv)).hexdigest()}
        bind_worker_launch(launch, [started, dispatch], manifest, digest)
        with self.assertRaisesRegex(ValueError, "held native worker image"):
            bind_worker_launch(launch, [started, dict(dispatch, image_sha256="x" * 64)], manifest, digest)
        with self.assertRaisesRegex(ValueError, "worker invocation differs"):
            bind_worker_launch(launch, [dict(started, worker_argv=argv[1:]), dispatch], manifest, digest)

    def test_exact_population_controls_and_balance(self):
        rows = build_schedule()
        self.assertEqual(Counter(row.slot for row in rows), Counter(build_plan()))
        self.assertEqual([row.slot for row in rows[:36]], build_plan()[:36])
        self.assertEqual(len(rows), 1940)
        self.assertEqual(len({row.group for row in rows}), SLOT_GROUPS)
        self.assertEqual(Counter(row.macroblock for row in rows[260:]), {i: 56 for i in range(30)})
        for context in ("DETACHED", "NO_WINDOW"):
            for path in ("fast", "kill"):
                cell = [row for row in rows if (row.slot.kind, row.slot.context, row.slot.path)
                        == ("measured", context, path)]
                self.assertEqual(len(cell), 360)
                position = Counter((row.slot.arm, row.position) for row in cell)
                self.assertEqual(len(position), 16)
                self.assertEqual(set(position.values()), {22, 23})
                carryover = Counter((previous.slot.arm, current.slot.arm)
                                    for previous, current in zip(cell, cell[1:])
                                    if previous.group == current.group)
                self.assertEqual(len(carryover), 12)
                self.assertEqual(set(carryover.values()), {22, 23})
        groups = group_plan(rows)
        self.assertEqual(groups[-1], (TAIL_GROUP, "tail", 1993))
        with self.assertRaises(ValueError):
            build_schedule(0)
        published_schedule = schedule_bytes(rows)
        self.assertEqual(published_schedule, schedule_bytes(build_schedule()))
        self.assertEqual(hashlib.sha256(published_schedule).hexdigest(),
                         "da7eb8ac14bbf6b5add7cf4f544586923c2f225f2087c3602c294c5ff5da221e")

    def test_stream_custody_and_settled_clock_reject_changes(self):
        rows = build_schedule()
        gate = _Gate()
        first = rows[0]
        with tempfile.TemporaryDirectory(prefix="ptg-laptop-offline-") as tmp:
            capture = Path(tmp)
            with (capture / "laptop-worker.jsonl").open("x", encoding="utf-8") as journal, (
                capture / "laptop-pricing.jsonl"
            ).open("x", encoding="utf-8") as pricing:
                worker = WorkerExecution(capture, rows, gate, journal, pricing, "b" * 64)
                with self.assertRaises(ValueError):
                    worker.before_slot(1, rows[1].slot)
                worker.before_slot(0, first.slot)
                start = time.perf_counter_ns()
                end = time.perf_counter_ns()
                fixture = capture / "owned-fixture.bin"
                fixture.write_bytes(b"relay")
                slot = first.slot
                row = {"kind": slot.kind, "arm": slot.arm, "path": slot.path,
                       "context": slot.context, "rep": slot.rep, "variant": slot.variant,
                       "outcome": "ok", "laptop_timing_ns": [start, end],
                       "elapsed_ms": (end - start) / 1e6,
                       "output_stdout_sha256": hashlib.sha256(b"relay").hexdigest(),
                       "output_stdout_len": 5,
                       "output_stderr_sha256": hashlib.sha256(b"relay").hexdigest(),
                       "output_stderr_len": 5}
                worker.retain_streams(row, {"stdout": fixture, "stderr": fixture})
                worker.after_slot(0, row)
            self.assertEqual(gate.requests, group_plan(rows)[:1])
            self.assertEqual(len(bind_streams(capture, [row])), 2)
            retained = capture / "streams" / "0000-stdout.bin"
            retained.write_bytes(b"tamper")
            with self.assertRaisesRegex(ValueError, "fingerprint differs"):
                bind_streams(capture, [row])
            retained.write_bytes(b"relay")
            with (capture / "laptop-pricing.jsonl").open("a", encoding="utf-8") as pricing:
                price = {"probe": "synthetic", "rep": 0, "shape": "stub"}
                append_record(pricing, {"index": 1, "observation": price})
            with self.assertRaisesRegex(ValueError, "pricing durable prefix differs"):
                bind_worker_items(capture, rows[:1], [row], [])

    def test_statistical_replay_uses_matched_oracles_and_actual_overlap(self):
        observed = []
        for row in build_schedule():
            slot = row.slot
            index = row.ordinal * 100_000_000
            duration = 1_000_000 if slot.arm == "guard" else 20_000_000
            observed.append({"kind": slot.kind, "arm": slot.arm, "path": slot.path,
                             "context": slot.context, "rep": slot.rep, "variant": slot.variant,
                             "outcome": "ok", "laptop_timing_ns": [index, index + duration]})
        results = analyze(observed, [], repetitions=5)["contexts"]
        for result in results:
            for contrast in result["contrasts"]:
                self.assertEqual(contrast["complete_oracle_pairs"], 90)
                for endpoint in contrast["endpoints"]:
                    self.assertEqual(endpoint["direction"], "faster")
                    self.assertTrue(endpoint["practically_meaningful"])
                    self.assertEqual(endpoint["point_ms"], -19)
        target = next(row for row in observed if row["kind"] == "measured" and row["path"] == "fast")
        left, right = target["laptop_timing_ns"]
        touch = [[left - 1, left], [right, right + 1], [left + 1, left + 1]]
        self.assertEqual(results, analyze(observed, touch, repetitions=5)["contexts"])
        overlap = [[0, 1940 * 100_000_000]]
        results = analyze(observed, overlap, repetitions=5)["contexts"]
        self.assertTrue(all(endpoint["direction"] == "unresolved" for result in results
                            for contrast in result["contrasts"] for endpoint in contrast["endpoints"]))
        victim = next(index for index, row in enumerate(observed)
                      if (row["kind"], row["arm"], row["path"], row["context"])
                      == ("measured", "sysprims", "fast", "DETACHED"))
        observed[victim]["outcome"] = "output-mismatch"
        result = analyze(observed, [], repetitions=5)["contexts"][0]
        rival = next(contrast for contrast in result["contrasts"] if contrast["rival"] == "sysprims")
        self.assertEqual(rival["complete_oracle_pairs"], 89)
        self.assertTrue(all(not endpoint["intervals_ns_x2"] for endpoint in rival["endpoints"]))


if __name__ == "__main__":
    unittest.main()

