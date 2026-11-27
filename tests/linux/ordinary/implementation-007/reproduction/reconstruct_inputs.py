"""Reconstruct the two historical ordinary Linux fixture source bundles."""

import argparse
import base64
from hashlib import sha256
import json
from pathlib import Path


BASE_COMMIT = "b8bfa00cd24e2eaf1f8733d2d46b28b9bd9f23af"
SNAPSHOT_COMMIT = "932c12da9f770a6b609a6f9075ea1923fc2c270f"
SNAPSHOT_MANIFEST = "e4dd57ccc1e5dcc847d78072e521ca82decf5e43e3172e91321cc431084bca58"
MAX_SOURCE_BYTES = 128 * 1024
LINUX = "src/sys/linux/"
RUST_BASE = (
    "arming_failure_controller.rs", "cgroup_authority.rs", "child_reaper.rs", "client.rs", "delegation.rs",
    "deployment/process-tree-guard.socket", "deployment/process-tree-guard@.service", "entry.rs", "invocation.rs",
    "manager.rs", "mod.rs", "native_activation.rs", "native_arming.rs", "native_port.rs", "request.rs",
    "service_scope.rs", "settlement.rs", "supervisor.rs", "terminal_controller.rs",
)
SHARED_HEADERS = ("owner_watch.h", "waitid_bridge.h")
BASE_OVERLAYS = {
    "src/deadline.rs": "6979e99e7bab5e73b1a1be076244c99480cdc4851d3cc82a1b243584f193f42e",
    LINUX + "manager.rs": "19ed471ce091a63033b0f9df6a25f0e72def165bf5e2a32f68ab97fc2981a382",
    LINUX + "mod.rs": "e7e69cee9941baabc437488870b96c11fde845961518e181be21de34a2a548a8",
}
MANAGER_LINE = b'#[link(name="libsystemd.so.0",kind="dylib",modifiers="+verbatim")]\n'
MANAGER_FIXTURE_LINE = b'#[cfg_attr(not(ptg_ordinary_fixture),link(name="libsystemd.so.0",kind="dylib",modifiers="+verbatim"))]\n'
MOD_APPEND = (b'\n#[path = "caller_adapter.rs"]\nmod caller_adapter;\n'
              b'\n#[path = "ordinary_fixture.rs"]\npub(crate) mod ordinary_fixture;\n')
FIXTURES = {
    "ordinary_check.rs": (176, "3cb37644a4116b2f2d30139efa04af435e11b595dc9f51fca25d2a66ae394d96"),
    "ordinary_fixture.rs": (3287, "33618c61ebf883570c3bdfb835e5dc5f096c80bc4346945c1797b5c93622307a"),
}
BUNDLES = {
    "C-SOURCES.json": (34, 269981, "93e0221d36fde430b83d1f13c853f474f15f90160a70c7d3c636d48fd697f4fc"),
    "RUST-SOURCES.json": (23, 651406, "e1f5d05b9914c12384da5bfeb1dc4588ec9ac75656818ace782e444d711fe675"),
}


def digest(data):
    return sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def source(root, name):
    path = (root / name).resolve(strict=True)
    require(path.is_relative_to(root) and path.is_file(), f"source is outside the export or is not a file: {name}")
    with path.open("rb") as stream:
        data = stream.read(MAX_SOURCE_BYTES + 1)
    require(len(data) <= MAX_SOURCE_BYTES, f"source exceeds size limit: {name}")
    return data


def candidate_sources(snapshot):
    raw = source(snapshot, "SOURCE-MANIFEST.json")
    require(digest(raw) == SNAPSHOT_MANIFEST, "source manifest does not match the published snapshot")
    members = json.loads(raw)["code_members"]
    files = {}
    for row in members:
        name = row["name"]
        require(Path(name).name == name and name not in files, "invalid candidate source role")
        data = source(snapshot, name)
        require((len(data), digest(data)) == (row["bytes"], row["sha256"]), f"candidate bytes differ: {name}")
        files[name] = data
    require(len(files) == 33 and "caller_adapter.rs" in files, "candidate role count differs")
    return files


def c_sources(base, candidate):
    files = {"src/" + name: data for name, data in candidate.items() if name.endswith((".c", ".h"))}
    for header in SHARED_HEADERS:
        files["native_headers/" + header] = source(base, LINUX + header)
    return files


def rust_sources(base, candidate, fixture_directory):
    files = {name: source(base, name) for name in ("src/deadline.rs", *(LINUX + part for part in RUST_BASE))}
    for name, expected in BASE_OVERLAYS.items():
        require(digest(files[name]) == expected, f"expected raw base blob for {name}")
    files["src/deadline.rs"] = files["src/deadline.rs"].replace(b"\n", b"\r\n")
    manager = LINUX + "manager.rs"
    require(files[manager].count(MANAGER_LINE) == 1, "manager fixture overlay anchor differs")
    files[manager] = files[manager].replace(MANAGER_LINE, MANAGER_FIXTURE_LINE)
    files[LINUX + "mod.rs"] += MOD_APPEND
    for name, expected in FIXTURES.items():
        data = source(fixture_directory, name)
        require((len(data), digest(data)) == expected, f"fixture bytes differ: {name}")
        files[name if name == "ordinary_check.rs" else LINUX + name] = data
    files[LINUX + "caller_adapter.rs"] = candidate["caller_adapter.rs"]
    return files


def bundle(name, files):
    count, size, expected = BUNDLES[name]
    require(len(files) == count, f"source role count differs: {name}")
    records = {role: {"base64": base64.b64encode(data).decode("ascii"),
                      "bytes": len(data), "sha256": digest(data)} for role, data in files.items()}
    raw = (json.dumps(records, sort_keys=True, indent=2) + "\n").encode("ascii")
    require((len(raw), digest(raw)) == (size, expected), f"historical bundle identity differs: {name}")
    return raw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-tree", required=True, type=Path, help=f"raw exported Git tree at {BASE_COMMIT}")
    parser.add_argument("--snapshot", required=True, type=Path, help=f"candidate source directory from {SNAPSHOT_COMMIT}")
    parser.add_argument("--output-dir", required=True, type=Path, help="new directory under an existing parent")
    args = parser.parse_args()
    try:
        base = args.base_tree.resolve(strict=True)
        snapshot = args.snapshot.resolve(strict=True)
        fixtures = Path(__file__).resolve().parent
        candidate = candidate_sources(snapshot)
        outputs = {
            "C-SOURCES.json": bundle("C-SOURCES.json", c_sources(base, candidate)),
            "RUST-SOURCES.json": bundle("RUST-SOURCES.json", rust_sources(base, candidate, fixtures)),
        }
        args.output_dir.mkdir()
        for name, raw in outputs.items():
            with (args.output_dir / name).open("xb") as output:
                output.write(raw)
            print(f"{name}  {len(raw)} bytes  {digest(raw)}")
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
