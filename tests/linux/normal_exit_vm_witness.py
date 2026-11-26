"""One natural completion in a disposable, offline guest."""

import errno
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import select
import stat
import subprocess
import sys
import time


INPUT = Path("/var/lib/ptg-x64-normal")
CGROUP = Path("/sys/fs/cgroup")
SOCKET = "process-tree-guard.socket"
READY = re.compile(rb"READY ([1-9][0-9]*) ([1-9][0-9]*)\n")
UNIT = re.compile(r"process-tree-guard@[0-9]+-[0-9]+-2000\.service")
LEAF = re.compile(r"run-[0-9a-f]{32}")
ASSETS = {
    "root": ("/usr/libexec/process-tree-guard-root", 0o755),
    "process-tree-guard.socket": ("/usr/lib/systemd/system/process-tree-guard.socket", 0o644),
    "process-tree-guard@.service": ("/usr/lib/systemd/system/process-tree-guard@.service", 0o644),
    "ref.rules": ("/etc/polkit-1/rules.d/50-process-tree-guard-ref.rules", 0o644),
    "delegation.policy": ("/etc/process-tree-guard/delegation.policy", 0o644),
}


def need(condition, message):
    if not condition:
        raise RuntimeError(message)


def emit(kind, *, diagnostic=False, **fields):
    prefix = "PTG-X64-CGROUP " if diagnostic else "PTG-X64-NORMAL "
    print(prefix + json.dumps({"kind": kind, **fields}, sort_keys=True, separators=(",", ":")), flush=True)


def read(file, maximum=4096):
    with Path(file).open("rb") as stream:
        data = stream.read(maximum + 1)
    need(len(data) <= maximum, "bounded observation exceeded")
    return data


def tick(pid):
    value = read(f"/proc/{pid}/stat")
    return int(value.rsplit(b")", 1)[1].split()[19])


def payload():
    need(os.getresuid() == (2000,) * 3, "payload user differs")
    print(f"READY {os.getpid()} {tick('self')}", flush=True)
    need(sys.stdin.buffer.read(1) == b"R", "original completion release differs")
    return 7


def command(args, seconds=3):
    result = subprocess.run(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=seconds, check=False)
    need(result.returncode == 0 and not result.stderr and len(result.stdout) <= 8192,
         f"native manager/guest query failed: {args[1]}")
    return result.stdout.decode("ascii")


def ctl(*args, deadline=None):
    if deadline is None:
        return command(["/usr/bin/systemctl", "--system", *args])
    remaining = deadline - time.monotonic()
    need(remaining > 0, "manager observation exceeded its absolute deadline")
    text = command(["/usr/bin/systemctl", "--system", *args], seconds=min(3, remaining))
    need(time.monotonic() <= deadline, "manager response arrived after its absolute deadline")
    return text


def show(unit, *names, diagnostic=False, deadline=None):
    text = ctl("show", "--all", "--no-pager", unit, "--property=" + ",".join(names), deadline=deadline)
    if diagnostic:
        emit("manager-properties", diagnostic=True, unit=unit, raw=text)
    items = [line.split("=", 1) for line in text.splitlines()]
    need(len(items) == len(names) and {x[0] for x in items} == set(names), "exact manager properties differ")
    return dict(items)


def tg_units():
    return ctl("list-units", "--all", "--type=service", "--no-legend", "--plain", "process-tree-guard@*.service")


def wait_manager_tick(deadline):
    delay = deadline - time.monotonic()
    need(delay > 0, "owned manager terminal state was not published in time")
    time.sleep(min(0.02, delay))


def observe_manager_terminal(original, unit_events, joined_unit, deadline):
    unit = original["Id"]
    original_pid = original["MainPID"]
    fields = ("Id", "InvocationID", "ControlGroup", "MainPID", "ControlPID", "ExecMainPID",
              "ExecMainCode", "ExecMainStatus", "Result", "ActiveState", "SubState", "NRestarts")
    while True:
        terminal = show(unit, *fields, diagnostic=True, deadline=deadline)
        group = terminal["ControlGroup"]
        need(terminal["Id"] == unit and terminal["InvocationID"] == original["InvocationID"]
             and group in {original["ControlGroup"], ""}
             and terminal["ExecMainPID"] == original_pid and terminal["NRestarts"] == "0",
             "manager record is not the original invocation and execution")
        if terminal["ActiveState"] == "failed":
            need(terminal["SubState"] == "failed" and terminal["MainPID"] == terminal["ControlPID"] == "0"
                 and terminal["ExecMainCode"] == "1" and terminal["ExecMainStatus"] == "7"
                 and terminal["Result"] == "exit-code", "owned manager terminal status differs")
            break
        need((terminal["ActiveState"] == "active" and terminal["SubState"] == "running"
              or terminal["ActiveState"] == "deactivating" and terminal["SubState"] in
              {"stop", "stop-sigterm", "stop-sigkill", "stop-post", "final-sigterm", "final-sigkill"})
             and terminal["MainPID"] in {original_pid, "0"} and terminal["ControlPID"] == "0"
             and terminal["Result"] in {"success", "exit-code"},
             "unexpected manager state before terminal publication")
        wait_manager_tick(deadline)

    # A blank ControlGroup can follow retirement after the original join.
    # Observe the same retained directory and events descriptors again.
    manager_unit_group = joined_unit
    if not terminal["ControlGroup"]:
        while True:
            need(time.monotonic() < deadline, "owned cgroup retirement observation expired")
            now_state = terminal_group(unit_events, deadline=deadline)
            need(time.monotonic() <= deadline, "owned cgroup retirement observation arrived late")
            if now_state in {"retired_zero", "retired_enodev"}:
                need(joined_unit["terminal"] in {now_state, "held_zero"},
                     "unexpected original cgroup retirement transition")
                manager_unit_group = {**group_receipt(unit_events), "terminal": now_state}
                break
            need(joined_unit["terminal"] == now_state == "held_zero",
                 "original cgroup became visible after retirement")
            wait_manager_tick(deadline)
    emit("manager-unit-group", diagnostic=True, group=manager_unit_group)

    inventory = ctl("list-units", "--all", "--type=service", "--no-legend", "--plain",
                    "--full", "--no-pager", "process-tree-guard@*.service", deadline=deadline)
    emit("manager-inventory", diagnostic=True, raw=inventory)
    rows = [line.split(None, 4) for line in inventory.splitlines()]
    need(len(rows) == 1 and rows[0][:4] == [unit, "loaded", "failed", "failed"],
         "manager inventory differs from the single owned terminal service")
    return terminal, manager_unit_group


def watch_process(pid):
    fd = os.pidfd_open(pid)
    pulse = select.poll()
    pulse.register(fd, select.POLLIN)
    return fd, pulse


def poll_mask(pulse):
    mask = 0
    for _, events in pulse.poll(0):
        mask |= events
    return mask


def signaled(pulse):
    return bool(poll_mask(pulse) & select.POLLIN)


def actor(pid, epoch, pulse):
    return {"pid": pid, "epoch_ticks": epoch, "poll_mask": poll_mask(pulse)}


def group_receipt(group):
    path, directory, _fd, pair = group
    observed = os.fstat(directory)
    need((observed.st_dev, observed.st_ino) == pair, "receipt cgroup descriptor changed")
    return {"path": str(path), "device": observed.st_dev, "inode": observed.st_ino}


def membership_bytes(pid, role=None):
    raw = read(f"/proc/{pid}/cgroup")
    if role is not None:
        # The read is bounded at 4096 bytes. The JSON string is escaped and
        # reversible even if the raw bytes cannot be parsed as a cgroup path.
        emit("membership", diagnostic=True, actor=role, pid=pid,
             bytes=len(raw), raw_latin1=raw.decode("latin-1"))
    return raw


def path_for(pid, role=None):
    place = membership_bytes(pid, role).decode("ascii")
    need(place.startswith("0::/system.slice/") and place.endswith("\n") and place.count("\n") == 1,
         "outside system manager cgroup v2")
    return place[3:-1]


def group_components(group):
    terms = group[1:].split("/")
    need(group.startswith("/") and all(
        0 < len(term) <= 255 and term not in {".", ".."} and term.isascii() and term.isprintable()
        for term in terms), "untrusted cgroup path segment")
    return terms


def held_events(relative):
    terms = relative.split("/")
    need(terms[0] == "" and all(t and t not in {".", ".."} for t in terms[1:]), "untrusted cgroup path segment")
    path = CGROUP / relative.removeprefix("/")
    directory = os.open(path, os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_RDONLY)
    fd = os.open("cgroup.events", os.O_NOFOLLOW | os.O_CLOEXEC | os.O_RDONLY, dir_fd=directory)
    bound = os.fstat(directory)
    need(bound.st_uid == bound.st_gid == 0 and b"populated 1" in os.pread(fd, 256, 0).splitlines(),
         "original cgroup ownership or live population differs")
    return path, directory, fd, (bound.st_dev, bound.st_ino)


def terminal_group(group, deadline=None):
    path, directory, fd, identity = group
    observed = os.fstat(directory)
    need((observed.st_dev, observed.st_ino) == identity, "original held cgroup identity differs")

    def original_name_exists():
        try:
            named = path.lstat()
        except FileNotFoundError:
            return False
        need((named.st_dev, named.st_ino) == identity, "same-name cgroup was replaced")
        return True

    need(deadline is None or time.monotonic() < deadline, "original cgroup observation expired")
    named = original_name_exists()
    try:
        current = os.pread(fd, 256, 0)
    except OSError as failure:
        if failure.errno != errno.ENODEV:
            raise
        cutoff = time.monotonic() + 0.05
        if deadline is not None:
            cutoff = min(deadline, cutoff)
        while True:
            need(time.monotonic() < cutoff, "original cgroup ENODEV retirement observation expired")
            still_named = original_name_exists()
            remaining = cutoff - time.monotonic()
            need(remaining > 0, "original cgroup ENODEV retirement observation arrived late")
            if not still_named:
                return "retired_enodev"
            time.sleep(min(0.005, remaining))
    need(deadline is None or time.monotonic() <= deadline, "original cgroup observation arrived late")
    fields = current.splitlines()
    need(b"populated 0" in fields and b"populated 1" not in fields,
         "original cgroup is not positively empty")
    return "held_zero" if named else "retired_zero"


def read_ready(stream):
    until = time.monotonic() + 5
    fd = stream.fileno()
    poller = select.poll()
    poller.register(fd, select.POLLIN)
    ready = bytearray()
    while time.monotonic() < until and len(ready) <= 128 and not ready.endswith(b"\n"):
        if not poller.poll(max(1, int((until - time.monotonic()) * 1000))):
            break
        octet = os.read(fd, 1)
        if not octet:
            break
        ready += octet
    result = READY.fullmatch(ready)
    need(result is not None, "original payload readiness not observed")
    return int(result[1]), int(result[2])


def put_assets():
    pins = json.loads(read(INPUT / "SHA256.json", 16384))
    need(set(pins) == set(ASSETS) | {"client", "guest-witness.py"}, "fixture input manifest differs")
    for name, expected in pins.items():
        file = INPUT / name
        info = file.lstat()
        need(stat.S_ISREG(info.st_mode) and info.st_uid == info.st_gid == 0 and not info.st_mode & 0o022
             and hashlib.sha256(read(file, 2 << 20)).hexdigest() == expected, "immutable guest seed file differs")
    need(not tg_units(), "guest has preexisting guard service instances")
    before = show(SOCKET, "LoadState", "ActiveState")
    need(before == {"LoadState": "not-found", "ActiveState": "inactive"}, "guest already has guard socket")
    need(not os.path.lexists("/run/process-tree-guard") and not os.path.lexists("/etc/process-tree-guard"),
         "guest has preexisting guard data roots")
    for dest, _ in ASSETS.values():
        need(not os.path.lexists(dest), "guest has a protected deployment collision")
    os.mkdir("/etc/process-tree-guard", 0o755)
    for name, (dest, mode) in ASSETS.items():
        fd = os.open(dest, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, mode)
        with os.fdopen(fd, "wb") as stream:
            stream.write(read(INPUT / name, 2 << 20))
    ctl("daemon-reload")
    ctl("start", SOCKET)
    started = show(SOCKET, "InvocationID", "ActiveState", "NAccepted")
    need(re.fullmatch("[a-f0-9]{32}", started["InvocationID"]) and int(started["InvocationID"], 16) != 0
         and started["ActiveState"] == "active" and started["NAccepted"] == "0", "initial owned socket is not empty")
    return started["InvocationID"]


def fixture():
    fixture_started = time.monotonic()
    os.umask(0o022)
    need(os.getresuid() == (0,) * 3 and os.getresgid() == (0,) * 3, "guest initial root required")
    need(os.uname().machine == "x86_64" and os.uname().release == "6.8.0-147-generic"
         and read("/proc/1/comm") == b"systemd\n" and read("/proc/sys/fs/suid_dumpable") == b"2\n",
         "unqualified guest platform")
    version = command(["/usr/bin/systemctl", "--version"]).splitlines()[0]
    need(version.startswith("systemd 255 "), "guest manager version differs")
    caller = pwd.getpwnam("ptg-lab")
    need(caller.pw_uid == 2000 and caller.pw_gid != 0 and caller.pw_dir == "/home/ptg-lab", "guest fixture account differs")
    instance = put_assets()
    emit("owned-socket-ready", kernel=os.uname().release, manager=255, socket_invocation=instance)
    argv = [str(INPUT / "client"), "--separate-service-context", "--timeout", "12", "--", "/usr/bin/python3",
            "-I", "-B", str(INPUT / "guest-witness.py"), "--payload"]
    proc = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=caller.pw_dir,
                            env={"PATH": "/usr/bin:/bin", "HOME": caller.pw_dir, "LANG": "C"},
                            user=caller.pw_uid, group=caller.pw_gid, extra_groups=[])
    descriptors = []
    try:
        client_epoch = tick(proc.pid)
        client_fd, client_wait = watch_process(proc.pid)
        descriptors.append(client_fd)
        need(tick(proc.pid) == client_epoch and not signaled(client_wait), "client pidfd hold changed")
        payload_pid, creation = read_ready(proc.stdout)
        payload_fd, payload_wait = watch_process(payload_pid)
        descriptors.append(payload_fd)
        need(payload_pid != proc.pid and tick(payload_pid) == creation and not signaled(payload_wait)
             and not signaled(client_wait), "original client or payload identity did not hold")
        try:
            membership_bytes(proc.pid, "client")
        except (OSError, RuntimeError) as error:
            emit("membership-unavailable", diagnostic=True, actor="client", pid=proc.pid, error=str(error)[:240])
        payload_group = path_for(payload_pid, "payload")
        terms = group_components(payload_group)
        need(len(terms) >= 2 and UNIT.fullmatch(terms[-2]) is not None and LEAF.fullmatch(terms[-1]) is not None,
             "payload does not name an instance and one dedicated leaf")
        unit, leaf = terms[-2:]
        props = show(unit, "Id", "InvocationID", "ActiveState", "MainPID", "ControlGroup", diagnostic=True)
        group = props["ControlGroup"]
        unit_terms = group_components(group)
        need(unit_terms[-1] == unit and payload_group == group + "/" + leaf,
             "payload is not a direct dedicated leaf of the manager's instance")
        native_pid = int(props["MainPID"])
        need(native_pid > 1 and native_pid not in {proc.pid, payload_pid}
             and props["Id"] == unit and props["ActiveState"] == "active"
             and re.fullmatch("[a-f0-9]{32}", props["InvocationID"]) and int(props["InvocationID"], 16) != 0,
             "manager service identity differs")
        service_epoch_before = tick(native_pid)
        service_fd, service_wait = watch_process(native_pid)
        descriptors.append(service_fd)
        service_epoch = tick(native_pid)
        need(service_epoch_before == service_epoch and not signaled(service_wait), "service pidfd did not join its original epoch")
        root_group = path_for(native_pid, "manager-mainpid")
        deployed = Path(ASSETS["root"][0])
        deployed_fd = os.open(deployed, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        descriptors.append(deployed_fd)
        original_elf = os.fstat(deployed_fd)
        with os.fdopen(os.dup(deployed_fd), "rb") as source_root:
            deployed_sha = hashlib.file_digest(source_root, "sha256").hexdigest()
        need(stat.S_ISREG(original_elf.st_mode) and original_elf.st_uid == original_elf.st_gid == 0
             and not original_elf.st_mode & 0o022 and deployed_sha == json.loads(read(INPUT / "SHA256.json"))["root"],
             "deployed root file is not the exact originally pinned ELF")
        process_exe = Path(f"/proc/{native_pid}/exe").stat()
        elf_identity = original_elf.st_dev, original_elf.st_ino
        need((process_exe.st_dev, process_exe.st_ino) == elf_identity, "original service executable identity differs")
        need(not signaled(service_wait) and not signaled(payload_wait) and root_group == group + "/supervisor"
             and tick(payload_pid) == creation and tick(native_pid) == service_epoch, "service/payload generation or placement changed")
        unit_events = held_events(group)
        descriptors.extend(unit_events[1:3])
        payload_events = held_events(payload_group)
        descriptors.extend(payload_events[1:3])
        uid = re.compile(rb"^Uid:\s+2000\s+2000\s+2000\s+2000$", re.MULTILINE)
        need(uid.search(read(f"/proc/{proc.pid}/status", 16384)) and uid.search(read(f"/proc/{payload_pid}/status", 16384)),
             "ordinary caller and payload credential proof differs")
        confirmed = show(unit, "Id", "InvocationID", "ActiveState", "MainPID", "ControlGroup")
        call_exe = Path(f"/proc/{native_pid}/exe").stat()
        need(confirmed == props and tick(native_pid) == service_epoch and tick(payload_pid) == creation
             and tick(proc.pid) == client_epoch and path_for(native_pid) == group + "/supervisor"
             and path_for(payload_pid) == payload_group and (call_exe.st_dev, call_exe.st_ino) == elf_identity
             and all(poll_mask(pulse) == 0 for pulse in (client_wait, payload_wait, service_wait)),
             "original manager, process generations, executable or placement changed before release")
        actors = {"client": (proc.pid, client_epoch, client_wait), "payload": (payload_pid, creation, payload_wait),
                  "root_service": (native_pid, service_epoch, service_wait)}
        groups = {"unit": group_receipt(unit_events), "payload": group_receipt(payload_events)}
        root_exe = {"device": elf_identity[0], "inode": elf_identity[1], "sha256": deployed_sha}
        emit("held-originals", manager=props, root_exe=root_exe, service_epoch_before=service_epoch_before,
             actors={name: actor(*values) for name, values in actors.items()}, groups=groups)
        proc.stdin.write(b"R")
        proc.stdin.flush()
        need(proc.wait(timeout=13) == 7 and signaled(client_wait), "original client did not forward native exit 7")
        need(signaled(payload_wait) and signaled(service_wait), "original payload/service not terminal at first client join")
        unit_state, payload_state = terminal_group(unit_events), terminal_group(payload_events)
        need(proc.stdout.read(1) == b"" and proc.stderr.read(1) == b"", "unexpected client output after original join")
        joined_actors = {name: actor(*values) for name, values in actors.items()}
        joined_groups = {"unit": {**group_receipt(unit_events), "terminal": unit_state},
                         "payload": {**group_receipt(payload_events), "terminal": payload_state}}
        emit("original-join", diagnostic=True, client_exit=7, actors=joined_actors,
             groups=joined_groups, empty_client_io=True, assistance_used=False)
        manager_deadline = min(fixture_started + 84, time.monotonic() + 3)
        terminal_manager, manager_unit_group = observe_manager_terminal(
            props, unit_events, joined_groups["unit"], manager_deadline)
        socket = show(SOCKET, "InvocationID", "ActiveState", "NAccepted",
                      diagnostic=True, deadline=manager_deadline)
        need(socket == {"InvocationID": instance, "ActiveState": "active", "NAccepted": "1"}, "expected exactly one owned activation")
        emit("pre-stop-joined", client_exit=7, three_original_pidfds_terminal=True,
             actors=joined_actors, groups=joined_groups,
             manager_invocation=props["InvocationID"], root_exe=root_exe,
             terminal_manager=terminal_manager, manager_unit_group=manager_unit_group, terminal_socket=socket,
             unit_cgroup=unit_state, payload_cgroup=payload_state, activation_count=1, assistance_used=False)
        ctl("stop", SOCKET)
        need(show(SOCKET, "ActiveState") == {"ActiveState": "inactive"}
             and not os.path.lexists("/run/process-tree-guard/activate.sock"), "owned socket did not stop")
        for dest, _ in ASSETS.values():
            Path(dest).unlink()
        Path("/etc/process-tree-guard").rmdir()
        ctl("daemon-reload")
        need(show(SOCKET, "LoadState", "ActiveState") == {"LoadState": "not-found", "ActiveState": "inactive"},
             "owned deployment retirement differs")
        need(not any(os.path.lexists(dest) for dest, _ in ASSETS.values())
             and not os.path.lexists("/etc/process-tree-guard"), "owned deployment assets remain")
        # No reset-failed: manager metadata for normal exit 7 may remain.
        emit("pass", deployment_retired=True, pre_stop_assistance_used=False)
    finally:
        for fd in reversed(descriptors):
            os.close(fd)


if sys.argv[1:] == ["--payload"]:
    raise SystemExit(payload())
if sys.argv[1:] != ["--fixture"]:
    raise SystemExit(125)
try:
    fixture()
except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
    emit("failure", error=str(error)[:240], forced_cleanup_is_never_pass=True)
    raise SystemExit(1)
