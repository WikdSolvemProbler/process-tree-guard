# SPDX-License-Identifier: Apache-2.0
"""Offline final-sample census decision controls."""

from copy import deepcopy
import unittest

from laptop_ancestry import role_instances
from laptop_census import validate_census


def fixture():
    observer = dict(pid=10, parent_pid=20, name="observer.exe", path="C:/sealed/observer.exe",
                    creation_filetime=40)
    primary = dict(pid=20, parent_pid=30, name="primary.exe", path="C:/sealed/primary.exe",
                   creation_filetime=30)
    tool = dict(pid=30, parent_pid=0, name="tool.exe", path="C:/sealed/tool.exe", creation_filetime=20)
    tool_sha, primary_sha, ordinary_sha = "a" * 64, "b" * 64, "c" * 64
    roles = [dict(primary, role="primary_guard", image_sha256=primary_sha),
             dict(tool, role="tool_guard", image_sha256=tool_sha)]
    proof = {"chain": [observer, primary, tool], "roles": roles, "snapshot_upper_ticks": 50}
    policy = {"pinned_basenames": ["primary.exe", "tool.exe", "python.exe"],
              "veto_sha256": [primary_sha, tool_sha],
              "enclosing_guards": {r["role"]: {"path": r["path"], "sha256": r["image_sha256"]}
                                   for r in roles}}
    held = role_instances(proof)

    def entry(pid, name, parent=0):
        return {"pid": pid, "parent_pid": parent, "name": name}

    def round_(begin, upper, *, unresolved=False):
        root, shell = entry(20, "primary.exe", 30), entry(30, "tool.exe")
        worker = entry(40, "python.exe", 10)
        unknown = entry(50, "python.exe", 2)
        helper = entry(60, "quiet.exe", 7)
        complete = [entry(0, "system idle process"), entry(10, "observer.exe", 20), root, shell, worker]
        result = [dict(root, created_ticks=30, state="pinned_enclosing_guard"),
                  dict(shell, created_ticks=20, image_path=tool["path"], image_sha256=tool_sha,
                       alive_after=True, job_member=False, state="pinned_workload_outside_job"),
                  dict(worker, created_ticks=45, image_path="C:/sealed/python.exe", image_sha256=None,
                       alive_after=True, job_member=True, state="campaign_member")]
        if unresolved:
            complete.append(unknown)
            result.append(dict(unknown, state="membership_unverified", error="OSError", winerror=87,
                               reason="native open failed", operation="open_process", handle_acquired=False))
        else:
            complete.append(helper)
            result.append(dict(helper, created_ticks=50, image_path="C:/sealed/quiet.exe",
                               image_sha256=ordinary_sha, alive_after=True, job_member=False,
                               state="unclassified_background"))
        return {"begin_ns": begin, "snapshot_begin_ns": begin + 1, "snapshot_end_ns": begin + 2,
                "finished_ns": begin + 3, "snapshot_upper_ticks": upper, "end_error": 18,
                "entry_count": len(complete), "entries": complete,
                "rows": [dict(row, snapshot_upper_ticks=upper) for row in result]}

    arguments = dict(phase="during", requested_ns=100, observer_pid=10,
                     worker_identity={"pid": 40, "creation_filetime": 45}, image_policy=policy,
                     compiler_names={"cargo.exe"}, workload_names={"python.exe", "quiet.exe"},
                     ancestry=proof, held_before=held, held_after=held)
    return arguments, round_(101, 100, unresolved=True), round_(110, 110)


class CensusTests(unittest.TestCase):
    def test_pinned_basename_is_not_a_positive_veto_and_replacement_decides(self):
        arguments, first, second = fixture()
        initial = validate_census([first], **arguments)
        self.assertEqual((initial["decision"], initial["replacement_pid"]), ("replacement_required", 50))
        final = validate_census([first, second], **arguments)
        self.assertEqual((final["decision"], final["replacement_pid"]), ("admit", 50))
        self.assertEqual(len(final["ancestry_admissions"]), 2)
        self.assertEqual(first["rows"][-1]["state"], "membership_unverified")
        self.assertIsNone(validate_census([second], **arguments)["replacement_pid"])

    def test_noneligible_first_round_is_terminal(self):
        for error in ({"operation": "initial_identity"}, {"operation": "close_handle"},
                      {"handle_acquired": True}, {"winerror": 5}, {"winerror": True}):
            with self.subTest(error=error):
                arguments, first, second = fixture()
                first["rows"][-1].update(error)
                with self.assertRaises(ValueError):
                    validate_census([first, second], **arguments)
        arguments, first, second = fixture()
        first["entries"][-1]["name"] = first["rows"][-1]["name"] = "cargo.exe"
        with self.assertRaises(ValueError):
            validate_census([first, second], **arguments)
        arguments, first, second = fixture()
        another = dict(first["rows"][-1], pid=51)
        first["entries"].append({key: another[key] for key in ("pid", "parent_pid", "name")})
        first["rows"].append(another)
        first["entry_count"] += 1
        with self.assertRaisesRegex(ValueError, "first census contains a veto"):
            validate_census([first, second], **arguments)
        arguments, first, second = fixture()
        veto = dict(second["rows"][-1], image_sha256="a" * 64,
                    state="pinned_workload_outside_job", snapshot_upper_ticks=100)
        first["entries"].append({key: veto[key] for key in ("pid", "parent_pid", "name")})
        first["rows"].append(veto)
        first["entry_count"] += 1
        with self.assertRaises(ValueError):
            validate_census([first, second], **arguments)

    def test_incomplete_replacement_or_reused_pid_stops(self):
        arguments, first, second = fixture()
        for change in ("missing relevant row", "reuse under an irrelevant image", "duplicate",
                       "wrong EOF", "late", "mismatched guard", "mismatched worker", "forged member",
                       "new positive veto"):
            with self.subTest(change=change):
                modified = deepcopy(second)
                if change == "missing relevant row":
                    modified["rows"].pop()
                elif change == "reuse under an irrelevant image":
                    modified["entries"].append({"pid": 50, "parent_pid": 0, "name": "new.exe"})
                    modified["entry_count"] += 1
                elif change == "duplicate":
                    modified["entries"].append(modified["entries"][0])
                    modified["entry_count"] += 1
                elif change == "wrong EOF":
                    modified["end_error"] = 5
                elif change == "late":
                    modified["finished_ns"] = 1_000_000_101
                elif change == "mismatched guard":
                    modified["rows"][0]["created_ticks"] -= 1
                elif change == "mismatched worker":
                    modified["rows"][2]["created_ticks"] -= 1
                elif change == "new positive veto":
                    modified["rows"][-1].update(image_sha256="a" * 64,
                                               state="pinned_workload_outside_job")
                else:
                    modified["rows"][-1]["state"] = "campaign_member"
                with self.assertRaises(ValueError):
                    validate_census([first, modified], **arguments)

    def test_no_replacement_in_other_phases_or_after_clear_census(self):
        arguments, first, second = fixture()
        for rounds in ([first, second, second], [second, second]):
            with self.assertRaises(ValueError):
                validate_census(rounds, **arguments)
        for phase in ("pre", "post"):
            with self.assertRaises(ValueError):
                validate_census([first, second], **dict(arguments, phase=phase, worker_identity=None))


if __name__ == "__main__":
    unittest.main()
