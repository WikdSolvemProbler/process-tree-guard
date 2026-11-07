# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Offline audit of the pinned Linux reaper archive; no subprocesses or extraction."""
import hashlib
import json
from pathlib import Path
import re
import zipfile

ROOT = Path(__file__).resolve().parents[2]
ARCHIVE = ROOT / "tests/evidence/linux-direct-child-reaper-static-link.zip"
EXPECTED_SHA = "f546645adef6e0b470837f5a68a124cb073f5a92c7ec8b9626fac4eeb6d7ea34"


def audit():
    raw_archive = ARCHIVE.read_bytes()
    assert hashlib.sha256(raw_archive).hexdigest() == EXPECTED_SHA
    with zipfile.ZipFile(ARCHIVE) as archive:
        assert archive.testzip() is None
        names = archive.namelist()
        index = json.loads(archive.read("INDEX.json"))
        assert len(names) == len(set(names)) == 221
        assert set(names) == set(index) | {"INDEX.json"}
        for name, pin in index.items():
            raw = archive.read(name)
            assert len(raw) == pin["bytes"]
            assert hashlib.sha256(raw).hexdigest() == pin["sha256"], name

        def member(ending):
            found = [name for name in names if name.endswith(ending)]
            assert len(found) == 1, (ending, found)
            return archive.read(found[0])

        result = json.loads(archive.read("RESULTS.json"))
        assert result["status"] == "ORDINARY_SEQUENTIAL_DIRECT_CHILD_REAPER_COMPONENT_PASSED"
        assert result["attempts"] == 1 and result["ssh_exit"] == result["outer_exit"] == 0
        assert result["fixture_children"] == 3 and result["selected_fixture_tests"] == 1
        assert result["baseline_C_objects_byte_identical_to011"] is True
        assert result["hardening_flags_unchanged"] is True
        assert result["scratch_removed"] is True and result["original_scope_gone"] is True
        assert len(result["recorded_pid_absence"]) == 26
        assert all(row["proc_entry_present"] is False for row in result["recorded_pid_absence"])
        assert result["root_actions"] == result["payload_execs"] == result["cgroup_component_writes"] == 0
        binary = member("/download/reaper-tests")
        assert binary[:6] == b"\x7fELF\x02\x01"  # ELF64, little endian.
        assert hashlib.sha256(binary).hexdigest() == result["native_binary"]["sha256"]
        link_map = member("/download/reaper-link.map")
        assert hashlib.sha256(link_map).hexdigest() == result["link_map_sha256"]
        assert b"ld-linux-aarch64.so.1" in link_map and b"__stack_chk_guard@@GLIBC_2.17" in link_map
        assert b"libptg_reaper_fixture.a(waitid-bridge.o)" in link_map
        assert hashlib.sha256(member("/download/waitid-bridge.o")).hexdigest() == "fc70194391d0fc41a0c769719ce42d79154cdb82588fa669f458e71f72674a13"
        assert hashlib.sha256(member("/download/child-fixture.o")).hexdigest() == "5a96576738116536248b448532fa55c6a2926ed11768f7ce3e71c73e08ae3808"

        abi = json.loads(member("/download/abi-report.json"))
        stdout = member("/download/sequential-reaper.stdout.log").decode("utf-8")
        case_modes = re.findall(r"^CASE_PASSED mode=([123]) pid=([0-9]+).* repeat=ECHILD$", stdout, re.M)
        assert [(int(mode), int(pid)) for mode, pid in case_modes] == [(row["mode"], row["pid"]) for row in result["case_receipts"]]
        observations = re.findall(r"RAW_WAIT label=(\S+) expected_pid=(\d+) target_pidfd=(\d+) before_ns=(\d+) frame=WaitFrame \{ ([^}]+) \} native_hex=([0-9a-f]+) abi=\[", stdout)
        assert observations
        positive, repeats, pending = [], [], 0
        for label, pid, pidfd, before, fields, native_hex in observations:
            frame = {key: int(value) for key, value in (field.split(": ") for field in fields.split(", "))}
            native = bytes.fromhex(native_hex)
            assert len(native) == frame["native_size"] == abi["siginfo_size"] == 128
            assert frame["called"] == 1 and int(pid) > 0 and int(pidfd) >= 0 and int(before) > 0
            if frame["result"] == 0:
                # Decode using the installed-header reporter's offsets, independently
                # of the C normalization and without assuming Rust siginfo padding.
                for field in ("signo", "errno", "code", "pid", "uid", "status"):
                    offset = abi["siginfo_" + field + "_offset"]
                    decoded = int.from_bytes(native[offset:offset + 4], "little", signed=field != "uid")
                    target = "signal_errno" if field == "errno" else field
                    assert decoded == frame[target]
                if frame["pid"] > 0:
                    assert frame["pid"] == int(pid) and frame["uid"] == 501
                    assert frame["signo"] == abi["SIGCHLD"] and frame["signal_errno"] == 0
                    positive.append((frame["pid"], frame["code"], frame["status"]))
                else:
                    pending += 1
            if label == "repeat_after_reaped":
                assert frame["result"] == -1 and frame["call_errno"] == abi["ECHILD"]
                repeats.append(int(pid))
        pids = [row["pid"] for row in result["case_receipts"]]
        assert positive == [(pids[0], abi["CLD_EXITED"], 23), (pids[1], abi["CLD_EXITED"], 29), (pids[2], abi["CLD_KILLED"], 15)]
        assert repeats == pids and pending >= 2
        return {"schema": 1, "archive_sha256": EXPECTED_SHA, "verified_members": len(names),
                "raw_positive_waits": positive, "repeat_echild_pids": repeats,
                "raw_pending_observations": pending, "native_status_decode": "installed C header offsets, ELF64 little endian",
                "exact_C_objects_match_failed_attempt": True, "stack_protector_reference_and_static_link_map": "verified",
                "preserved_native_artifacts": result["artifacts_preserved"], "recorded_absent_pids": 26,
                "exact_scratch_removed": result["scratch_retained_original"],
                "scope": "Component ABI, raw child-reaping/linkage and recorded cleanup evidence only; no full Linux backend or performance qualification"}


if __name__ == "__main__":
    print(json.dumps(audit(), indent=2))
