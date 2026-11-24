# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Peter Shiller
"""Sealed local entry point for the Windows laptop 182.001 A campaign."""

import argparse
import hashlib
import io
import json
import os
import re
import secrets
import sys
from contextlib import ExitStack
from importlib import import_module
from pathlib import Path

HERE = Path(__file__).resolve().parent
if __name__ == "__main__" and (
    not sys.flags.isolated or not sys.dont_write_bytecode
    or (HERE / "__pycache__").exists() or (HERE.parent / "__pycache__").exists()
):
    raise SystemExit("run the isolated campaign stage with Python -I -B and no bytecode cache")
sys.path.insert(0, str(HERE))

SOURCE_COMMIT = "caebd14329e454584fe5d263efe7f4213d77aa54"
INPUT_SCHEMA = "ptg-laptop-182-001-inputs-v1"
MANIFEST_SCHEMA = "ptg-laptop-182-001-a-manifest-v1"
RUN_ID = re.compile(r"laptop-182-001-a-[a-z0-9-]{1,50}\Z", re.ASCII)
SHA256 = re.compile(r"[0-9a-f]{64}\Z", re.ASCII)
PROTOCOL_DIR = HERE / "laptop-182-001"
DURING_SECONDS = 6600
PRIMARY_SECONDS = 6900
TOOL_SECONDS = 6920
VOLATILE_ENV = frozenset({"CODEX_TOOL_CALL_ID", "RUST_LOG"})
FORBIDDEN_ENV = frozenset({"PYTHONHOME", "PYTHONPATH", "PYTHONSTARTUP"})

SOURCE_FILES = (
    "arms.py", "harness.py", "laptop_analysis.py", "laptop_ancestry.py", "laptop_audit.py",
    "laptop_barrier.py", "laptop_launch.py", "laptop_schedule.py", "laptop_telemetry.py",
    "laptop_worker.py", "plan.py", "pricing.py", "probe.py", "report.py", "runner.py",
    "windows_payload_observer.py", "windows_slot_job.py", "../envcase.py",
    "../usability-evidence.json",
)
OBSERVER_PINS = {
    "audit_package.py": "3908c9db44e2696fd535c1a1c9b87a4867573a42a4d825d648da6981370669d2",
    "quiet_gate.py": "c8995c81f6d05c4f17f8b1a00551065665f97a7ef15e5407212f63c3d1135ac3",
    "inventory_gate.py": "4a7b6ce493753ff5b642ad7ca7a7dd04c9487847e0425a35688c8778246e3785",
    "image_policy.py": "f9ca5e411de733df0e44cec6b3604790ed21336213850ef7f35b8acd0d54ff9d",
}
PROTOCOL_PINS = {
    "PROTOCOL.md": "67f14e04a8750ab9ed8e7b1ec6c3b02f22dff8a7a51774444f263672044587fe",
    "ADDENDUM-A-A-001.md": "4743e6037cf51dba5beedbd9c875afb315519a78908b075969033e65b0d79468",
    "REVIEW-CRITIC-ADDENDUM-A-A-001.json": "a821745c3b4d7992ada8b604f93bfb98d0e9cdae0c3d0f7a7d9c6b65590fdb05",
    "REVIEW-DESIGNER-ADDENDUM-A-A-001.json": "7c6ab02b541c798310550739ea9ff8d07dc596a9c1c965900d9d2ecff50deb61",
    "AMD-QUIET-FIXTURE-001.md": "883b5f4019f9c23d2d1be7b7556a095c0f64a3c9389ec5ece45e1f8fb203394a",
    "REVIEW-CRITIC-QUIET-FIXTURE-001.json": "26e92f6850ad95b25dcb8c0e28f54e5adcff643ff208699c120ac3652c9f4817",
    "REVIEW-DESIGNER-QUIET-FIXTURE-001.json": "5e6121a3caa382fe04d1f94a0cc036ee04a229a3288a5e350396eb93f33ddad1",
    "REVIEW-CRITIC-FINAL.json": "3a59ab41a721b73575917f612ab8a35f5f43d4a2e7d2bc614d4bb052a0f6d898",
    "REVIEW-DESIGNER-FINAL.json": "3bfd5afd156f1c6a6006127b7773b7cf5d9b90e641b1ae577c526fd92148aa99",
}
HISTORICAL_FIXTURE_SHA256 = "d55d6b947c921b9d28e803a5057dac51e8df044fbdd7106b88880292c4c47a2f"
LITERAL_PINS = {
    "config": "16b551edb24a42dd2cc8db1ffb822e0a4359c4f522981a3bff35554bc1cfafb9",
    "fixture": "346ca22737701fb5e2a7188d0ad9aadca07a496984ca83c7e34747997324f8a3",
    "processkit": "a79542cfe449f6916921845c2d7e02ea425c496c1a12d6359f7ca6b5f75141bb",
    "nuewframe": "765a364afd529c10b58b7cfa1cd9f13cf955e7a053f1e4a2d467038f064d16e9",
    "sysprims": "0a3144a6ca7b2df7ee382bbbac85d89f5ed75ee2643e1036356a8eaa4d490867",
    "competitor_identities": "62835816c70c662db3161bbba6fd4a829c7c3a1fd2b4c6234275c5c134d1a4c1",
}
BUNDLE_PATHS = {
    "guard": "build/process-tree-guard.exe",
    "fixture": "build/testkit.exe",
    "nuewframe": "build/competitors/timeout.exe",
    "processkit": "build/competitors/processkit-cli.exe",
    "sysprims": "build/competitors/sysprims.exe",
    "competitor_identities": "build/competitor-identities.json",
}
REQUIRED_FILES = frozenset(BUNDLE_PATHS) | {
    "config", "python", "cmd", "powershell", "primary_guard", "tool_guard",
    "build_receipt", "source_archive", "toolchain_manifest",
}
NATIVE_CONTROLS = frozenset({
    "actual_candidate_protection", "independent_guard_cleanup", "clock_and_observer",
    "barrier_and_witness", "record_retention", "instrumentation_a_a", "raw_replay",
})


def require(condition, message):
    if not condition:
        raise ValueError(message)


def strict_object(entries):
    result = dict(entries)
    require(len(result) == len(entries), "duplicate JSON key")
    return result


def canonical_bytes(payload):
    text = json.dumps(payload, sort_keys=True, separators=(",", ":"), allow_nan=False)
    return (text + "\n").encode("utf-8")


def parse_json(raw):
    def invalid(token):
        raise ValueError(f"invalid JSON constant: {token}")

    return json.loads(raw, object_pairs_hook=strict_object, parse_constant=invalid)


def file_hash(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def file_identity(path, expected=None):
    path = Path(path)
    require(path.is_absolute() and path.resolve(strict=True) == path and path.is_file(),
            f"expected an absolute canonical file path: {path}")
    digest = file_hash(path)
    require(expected is None or SHA256.fullmatch(expected) and expected == digest,
            f"file digest differs: {path}")
    return {"path": str(path), "size": path.stat().st_size, "sha256": digest}


def validate_input_ref(ref):
    require(isinstance(ref, dict) and set(ref) == {"path", "sha256"},
            "input reference requires path and SHA-256")
    require(isinstance(ref["sha256"], str) and SHA256.fullmatch(ref["sha256"]),
            "input SHA-256 is absent or malformed")
    return file_identity(ref["path"], ref["sha256"])


def normalized_environment():
    result = {}
    for key, value in os.environ.items():
        upper = key.upper()
        require(upper not in FORBIDDEN_ENV, "forbidden Python environment setting")
        if upper not in VOLATILE_ENV:
            require(upper not in result, "duplicate folded environment key")
            result[upper] = value
    return hashlib.sha256(canonical_bytes(result)).hexdigest()


def directory(path, *, exists):
    path = Path(path)
    require(path.is_absolute() and path.resolve(strict=exists) == path
            and (not exists or path.is_dir()), f"invalid canonical directory: {path}")
    return path


def validate_build(files):
    receipt = parse_json(Path(files["build_receipt"]["path"]).read_bytes())
    expected = {
        "schema": "ptg-laptop-182-001-build-v1",
        "source_commit": SOURCE_COMMIT,
        "source_archive_sha256": files["source_archive"]["sha256"],
        "guard_sha256": files["guard"]["sha256"],
        "toolchain_manifest_sha256": files["toolchain_manifest"]["sha256"],
        "target": "x86_64-pc-windows-msvc",
        "profile": "release",
        "locked": True,
        "offline": True,
    }
    require(isinstance(receipt, dict) and all(receipt.get(k) == v for k, v in expected.items()),
            "candidate build receipt differs from the exact approved source or release recipe")


def qualification(files, measurement):
    if "native_qualification" not in files:
        return False
    receipt = parse_json(Path(files["native_qualification"]["path"]).read_bytes())
    expected = {
        "schema": "ptg-laptop-182-001-native-qualification-v1",
        "measurement_sha256": measurement,
        "controls": {control: "PASS" for control in NATIVE_CONTROLS},
    }
    require(isinstance(receipt, dict) and all(receipt.get(k) == v for k, v in expected.items()),
            "native control receipt is incomplete or bound to other measurement inputs")
    return True


def isolated_inputs(path):
    original = file_identity(path)
    source = parse_json(path.read_bytes())
    require(isinstance(source, dict) and set(source) == {"schema", "files", "observer_dir"}
            and source["schema"] == INPUT_SCHEMA, "unexpected input schema")
    refs = source["files"]
    require(isinstance(refs, dict) and REQUIRED_FILES <= set(refs)
            and set(refs) <= REQUIRED_FILES | {"native_qualification"}, "input role set differs")
    files = {role: validate_input_ref(ref) for role, ref in refs.items()}
    for role, expected in LITERAL_PINS.items():
        require(files[role]["sha256"] == expected, f"unexpected fixed artifact for {role}")
    config = Path(files["config"]["path"])
    require(config.name == "campaign-sysprims-fast-warn.json" and config.parent == HERE.parent,
            "fixed configuration must be beside the isolated campaign module directory")
    for role, relative in BUNDLE_PATHS.items():
        require(Path(files[role]["path"]) == config.parent / relative, f"wrong staged path for {role}")
    require(Path(files["python"]["path"]).samefile(sys.executable), "active Python differs from its pin")
    require(os.name == "nt" and sys.maxsize > 2 ** 32 and sys.version_info[:2] == (3, 12),
            "laptop A expects Windows x64 CPython 3.12")
    require(Path(files["cmd"]["path"]).samefile(os.environ.get("COMSPEC", "")),
            "COMSPEC differs from its pinned image")
    require(Path(files["primary_guard"]["path"]).name.lower() == "process-tree-guard.exe",
            "primary guard does not match the qualified ancestry basename")
    require(not Path(files["primary_guard"]["path"]).samefile(files["guard"]["path"]),
            "candidate and enclosing guard must have separate custody")
    require(not Path(files["tool_guard"]["path"]).samefile(files["primary_guard"]["path"]),
            "independent outer guard path differs from the primary")
    validate_build(files)
    observer = directory(source["observer_dir"], exists=True)
    observer_files = {name: file_identity(observer / name, digest)
                      for name, digest in OBSERVER_PINS.items()}
    require({row.name for row in observer.iterdir()} == set(OBSERVER_PINS),
            "observer stage contains additional files")
    return original, files, observer_files


def describe(input_path, capture, run_id):
    from laptop_schedule import SEED, build_schedule, schedule_sha256
    from laptop_worker import group_plan

    require(isinstance(run_id, str) and RUN_ID.fullmatch(run_id), "invalid fresh laptop A run ID")
    capture = directory(capture, exists=False)
    require(capture.parent.is_dir(), "capture parent directory must already exist")
    root, files, observer = isolated_inputs(input_path)
    source = {name: file_identity((HERE / name).resolve()) for name in SOURCE_FILES}
    protocol = {name: file_identity(PROTOCOL_DIR / name, digest)
                for name, digest in PROTOCOL_PINS.items()}
    rows = build_schedule()
    pinned = {
        "schema": MANIFEST_SCHEMA,
        "source_commit": SOURCE_COMMIT,
        "files": {role: ref for role, ref in files.items() if role != "native_qualification"},
        "sources": source,
        "observer_sources": observer,
        "protocol": protocol,
        "effective_environment_sha256": normalized_environment(),
        "seed": SEED,
        "schedule_sha256": schedule_sha256(rows),
        "group_count": len(group_plan(rows)),
        "deadlines_s": {"during": DURING_SECONDS, "primary": PRIMARY_SECONDS, "tool": TOOL_SECONDS},
    }
    measurement = hashlib.sha256(canonical_bytes(pinned)).hexdigest()
    images = [files[role] for role in ("guard", "fixture", "nuewframe", "processkit", "sysprims",
                                       "primary_guard", "tool_guard")]
    veto = {row["sha256"] for row in images}
    veto.add(HISTORICAL_FIXTURE_SHA256)
    destination = input_path.with_name("manifest.json")
    entry = str(Path(__file__).resolve())
    execute_argv = [
        files["primary_guard"]["path"], "--timeout", str(PRIMARY_SECONDS), "--",
        files["python"]["path"], "-I", "-B", "-X", "utf8", entry,
        "execute", "--manifest", str(destination), "--approved-sha", "<same manifest SHA-256>",
    ]
    return {
        **pinned,
        "inputs": {"description": root, "config": files["config"]["path"]},
        "manifest_path": str(destination),
        "working_directory": str(Path(files["config"]["path"]).parent.parent),
        "execute_argv_template": execute_argv,
        "measurement_sha256": measurement,
        "native_qualification": files.get("native_qualification"),
        "native_ready": qualification(files, measurement),
        "capture": str(capture),
        "run_id": run_id,
        "image_policy": {
            "veto_sha256": sorted(veto),
            "pinned_basenames": sorted({Path(row["path"]).name.casefold() for row in images}),
            "enclosing_guards": {
                role: {key: files[role][key] for key in ("path", "sha256")}
                for role in ("primary_guard", "tool_guard")
            },
            "scope": "selected process names and byte-verified paths sampled at settled boundaries",
        },
    }


def load_manifest(path, approved_sha):
    require(isinstance(approved_sha, str) and SHA256.fullmatch(approved_sha), "invalid manifest digest")
    raw = path.read_bytes()
    require(hashlib.sha256(raw).hexdigest() == approved_sha, "manifest digest differs")
    manifest = parse_json(raw)
    require(isinstance(manifest, dict) and manifest.get("schema") == MANIFEST_SCHEMA
            and raw == canonical_bytes(manifest), "manifest is not in frozen canonical form")
    return manifest


def code_refs(manifest):
    """Reject source path substitutions before importing any campaign code."""
    groups = (
        ("sources", HERE, SOURCE_FILES),
        ("protocol", PROTOCOL_DIR, PROTOCOL_PINS),
    )
    entries = []
    for field, directory, names in groups:
        sealed = manifest[field]
        require(set(sealed) == set(names), f"frozen {field} list differs")
        for name in names:
            ref = sealed[name]
            expected = (directory / name).resolve(strict=True)
            require(Path(ref["path"]) == expected, "campaign code resolves outside its sealed stage")
            require(file_identity(expected, ref["sha256"]) == ref, "campaign code differs from the seal")
            entries.append(ref)
    return entries


def verify_live_manifest(path, approved_sha):
    manifest = load_manifest(path, approved_sha)
    require(Path(manifest["manifest_path"]) == path, "live manifest path differs from the recipe")
    code_refs(manifest)
    current = describe(Path(manifest["inputs"]["description"]["path"]),
                       Path(manifest["capture"]), manifest["run_id"])
    require(manifest == current, "live execution inputs differ from the sealed manifest")
    return manifest


def execution_approval(path, digest):
    approval_path = path.with_name("execution-approvals.json")
    raw_ref = file_identity(approval_path)
    approval = parse_json(approval_path.read_bytes())
    require(approval.get("schema") == "ptg-laptop-182-001-execution-approval-v1"
            and approval.get("manifest_sha256") == digest
            and approval.get("authorized") is True, "execution admission is absent or misbound")
    reviews = approval.get("reviewers")
    require(isinstance(reviews, list) and len(reviews) == 2
            and all(isinstance(review, dict) for review in reviews), "need two independent manifest reviews")
    roles = {review.get("role") for review in reviews}
    require(roles == {"designer", "critic"} and all(
        review.get("model") == "gpt-6.2-sol" and review.get("decision") == "APPROVED"
        and review.get("manifest_sha256") == digest and review.get("open_objections") == []
        for review in reviews
    ), "manifest review does not establish both exact approvals")
    prereg = validate_input_ref(approval.get("public_preregistration"))
    return raw_ref, prereg


def iter_held_refs(manifest):
    for field in ("files", "sources", "observer_sources", "protocol"):
        yield from manifest[field].values()
    yield manifest["inputs"]["description"]
    if manifest["native_qualification"] is not None:
        yield manifest["native_qualification"]


def windows_read_hold(stack, ref):
    import _winapi
    import msvcrt

    path = Path(ref["path"])
    handle = _winapi.CreateFile(str(path), 0x80000000, 1, 0, 3, 0, 0)
    try:
        fd = msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY)
    except OSError:
        _winapi.CloseHandle(handle)
        raise
    stream = stack.enter_context(io.open(fd, "rb"))
    stat = os.fstat(stream.fileno())
    require(stat.st_ino > 0 and os.path.samestat(stat, path.stat())
            and stat.st_size == ref["size"], f"held file identity differs: {path}")
    require(hashlib.file_digest(stream, "sha256").hexdigest() == ref["sha256"],
            f"held file bytes differ: {path}")
    return path, stat


def hold_guard_ancestry(stack, manifest):
    """Bind both existing enclosing guard instances, independently of the trial jobs."""
    from inventory_gate import entries, identity, is_alive, open_exact
    from laptop_ancestry import HeldGuardAncestry, ROLES
    from laptop_barrier import native

    _, kernel, _ = native()
    image_policy = manifest["image_policy"]
    pins = image_policy["enclosing_guards"]
    expected = list(ROLES)
    snapshot, upper = entries(capture_upper=True)
    observer_pid = current = os.getpid()
    seen = set()
    clock = upper
    chain, roles, handles = [], [], {}
    while current in snapshot and current not in seen and expected:
        seen.add(current)
        entry = snapshot[current]
        handle = stack.enter_context(open_exact(current))
        image, created = identity(handle)
        require(is_alive(handle) and image.name.casefold() == entry.name
                and int(kernel.GetProcessId(handle)) == current and 0 < created <= clock,
                "unverified enclosing process ancestry")
        clock = created
        node = {"pid": current, "parent_pid": entry.parent_pid, "name": entry.name,
                "path": str(image), "creation_filetime": created}
        chain.append(node)
        role = expected[0]
        pin = pins[role]
        sealed = Path(pin["path"])
        if image.name.casefold() == sealed.name.casefold():
            require(image == sealed and image.samefile(sealed),
                    "enclosing guard image differs from the seal")
            roles.append({**node, "role": role, "image_sha256": pin["sha256"]})
            handles[role] = handle
            expected.pop(0)
        current = entry.parent_pid
    require(not expected, "pinned primary and finite tool guard ancestry are required")
    proof = {"chain": chain, "roles": roles, "snapshot_upper_ticks": upper}
    return HeldGuardAncestry(proof, handles, image_policy, observer_pid)


def write_exclusive(path, value):
    with path.open("xb") as stream:
        stream.write(canonical_bytes(value))
        stream.flush()
        os.fsync(stream.fileno())


def worker_pipe_name(nonce):
    require(isinstance(nonce, str) and SHA256.fullmatch(nonce), "invalid worker pipe nonce")
    return r"\\.\pipe\ptg-laptop-182-001-" + nonce


def worker_argv(path, approved_sha, manifest, pipe_name, nonce):
    return [
        manifest["files"]["python"]["path"], "-I", "-B", "-X", "utf8", str(Path(__file__).resolve()),
        "worker", "--manifest", str(path), "--approved-sha", approved_sha,
        "--pipe", pipe_name, "--nonce", nonce,
    ]


def bind_observers(manifest):
    observer_dir = Path(manifest["observer_sources"]["quiet_gate.py"]["path"]).parent
    require({entry.name for entry in observer_dir.iterdir()} == set(OBSERVER_PINS),
            "observer stage contains unbound inputs or a bytecode cache")
    sys.path.insert(0, str(observer_dir))
    for name in ("image_policy", "inventory_gate", "quiet_gate", "audit_package"):
        ref = manifest["observer_sources"][name + ".py"]
        source = Path(ref["path"])
        require(file_identity(source, ref["sha256"]) == ref, "observer source differs")
        module = import_module(name)
        require(Path(module.__file__).resolve() == source, "observer import does not match its pin")


def execute(path, approved_sha):
    manifest = load_manifest(path, approved_sha)
    require(manifest["native_ready"], "exact native qualification has not passed")
    code_refs(manifest)
    approval, prereg = execution_approval(path, approved_sha)
    capture_path = Path(manifest["capture"])
    require(not capture_path.exists(), "capture destination was already consumed")
    for key in list(os.environ):
        if key.upper() == "RUST_LOG":
            del os.environ[key]
    hold_refs = [
        *iter_held_refs(manifest), file_identity(path, approved_sha), approval, prereg,
    ]
    with ExitStack() as stack:
        held = [windows_read_hold(stack, ref) for ref in hold_refs]
        verify_live_manifest(path, approved_sha)
        bind_observers(manifest)
        from laptop_telemetry import capture
        from laptop_audit import audit_capture
        from laptop_schedule import build_schedule, schedule_bytes
        from laptop_worker import group_plan

        ancestry = hold_guard_ancestry(stack, manifest)
        rows = build_schedule()
        nonce = secrets.token_hex(32)
        pipe = worker_pipe_name(nonce)
        argv = worker_argv(path, approved_sha, manifest, pipe, nonce)
        capture_path.mkdir()
        (capture_path / "schedule.json").write_bytes(schedule_bytes(rows))
        write_exclusive(capture_path / "launch.json", {
            "manifest_sha256": approved_sha,
            "approval_sha256": approval["sha256"],
            "preregistration_sha256": prereg["sha256"],
            "worker_argv": argv,
            "guard_ancestry": ancestry.proof,
            "supervisor_pid": os.getpid(),
            "held": [{"path": str(file), "device": stat.st_dev, "file": stat.st_ino}
                     for file, stat in held],
        })
        terminal = {"accepted": False, "manifest_sha256": approved_sha}
        try:
            terminal["observer"] = capture(
                manifest, capture_path, argv, pipe_name=pipe, nonce=nonce, groups=group_plan(rows),
                manifest_sha=approved_sha, guard_ancestry=ancestry,
                max_during_seconds=DURING_SECONDS,
            )
            require(terminal["observer"].get("accepted") is True,
                    "native observer refused or stopped the campaign")
            for (file, stat), ref in zip(held, hold_refs):
                require(os.path.samestat(stat, file.stat()) and file_identity(file, ref["sha256"]) == ref,
                        f"held runtime input changed: {file}")
            terminal["audit"] = audit_capture(capture_path, manifest, approved_sha)
            terminal["accepted"] = True
        except Exception as error:
            terminal["error"] = f"{type(error).__name__}: {error}"
            raise
        finally:
            write_exclusive(capture_path / "terminal.json", terminal)
    return 0


def command(args):
    if args.command == "describe":
        payload = describe(args.inputs, args.capture, args.run_id)
        sys.stdout.buffer.write(canonical_bytes(payload))
        return 0
    if args.command == "validate":
        manifest = verify_live_manifest(args.manifest, args.approved_sha)
        print(json.dumps({"manifest_sha256": args.approved_sha, "native_ready": manifest["native_ready"]}))
        return 0
    if args.command == "execute":
        return execute(args.manifest, args.approved_sha)
    manifest = load_manifest(args.manifest, args.approved_sha)
    capture_path = Path(manifest["capture"])
    if args.command == "worker":
        manifest = verify_live_manifest(args.manifest, args.approved_sha)
        require(manifest["native_ready"], "worker requires the exact native qualification")
        approval, _ = execution_approval(args.manifest, args.approved_sha)
        launch = parse_json((capture_path / "launch.json").read_bytes())
        require(launch.get("manifest_sha256") == args.approved_sha
                and launch.get("supervisor_pid") == os.getppid()
                and launch.get("approval_sha256") == approval["sha256"]
                and launch.get("worker_argv") == worker_argv(
                    args.manifest, args.approved_sha, manifest, args.pipe, args.nonce
                ), "worker invocation differs from the live admission")
        from laptop_schedule import build_schedule
        from laptop_worker import run_worker

        return run_worker(manifest, args.approved_sha, capture_path, build_schedule(), args.pipe, args.nonce)

    with ExitStack() as stack:
        refs = [*code_refs(manifest), *manifest["observer_sources"].values(),
                manifest["files"]["config"]]
        for ref in refs:
            windows_read_hold(stack, ref)
        bind_observers(manifest)
        from laptop_audit import audit_capture

        report = audit_capture(args.capture, manifest, args.approved_sha, full=args.full)
    sys.stdout.buffer.write(canonical_bytes(report))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest="command", required=True)
    description = subs.add_parser("describe", help="seal staged inputs and emit a prospective manifest")
    description.add_argument("--inputs", type=Path, required=True)
    description.add_argument("--capture", type=Path, required=True)
    description.add_argument("--run-id", required=True)
    for name in ("validate", "execute", "audit", "worker"):
        sub = subs.add_parser(name)
        sub.add_argument("--manifest", type=Path, required=True)
        sub.add_argument("--approved-sha", required=True)
        if name == "audit":
            sub.add_argument("--capture", type=Path, required=True)
            sub.add_argument("--full", action="store_true",
                             help="recompute all fixed bootstrap draws after custody replay")
        if name == "worker":
            sub.add_argument("--pipe", required=True)
            sub.add_argument("--nonce", required=True)
    args = parser.parse_args()
    if args.command != "describe":
        args.manifest = args.manifest.resolve(strict=True)
    try:
        return command(args)
    except (OSError, ValueError) as error:
        print(f"laptop A: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

