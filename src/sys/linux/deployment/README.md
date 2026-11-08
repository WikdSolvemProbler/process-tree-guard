# Staged Linux product entry

These are product source entry points, not installed services or a test framework.
The normal cross-platform selector remains fail-closed for Linux.

- `../root_main.rs` builds the protected root-service executable installed as
  `/usr/libexec/process-tree-guard-root`. Its only accepted role is `--service`.
- `../client_main.rs` builds the ordinary standalone client. It captures the
  original parent/all-ancestor capabilities and absolute BOOTTIME cutoff before
  activation, then sets NNP only on its own process.
- The in-process library client requires existing NNP=1 and never changes the
  caller's process policy. It retains the original requester in addition to the
  manual parent or complete auto ancestor set.
- `process-tree-guard.socket` and `process-tree-guard@.service` are exact checked
  runtime configuration sources. They have no installation hooks. PID1 owns the
  activation listener; each accepted connection creates one finite Type=exec
  service. The supervisor/watchdog subgroup and payload leaves remain inside the
  same held managed unit.

The service adopts the kernel-transferred requester pidfd before deciding any
caller identity, validates real procfs/nsfs and protected installation authority,
queries authenticated PID1 effective configuration, arms its sticky self timer,
and binds the original cutoff before it creates any watchdog or payload child.
It creates one private root listener for the same live requester and full request.
No root-manager bus descriptor is transferred to the ordinary client. The client
uses its own public read-authorized manager connection/reference.

Completion requires a matching settled root frame, exact service pidfd terminal,
original unit empty or narrowly proved retired, and retained manager invocation,
ExecMainPID, ExecMainCode/Status and Result. ENODEV alone, a missing pathname,
manager ControlGroup becoming empty, nlink and a replacement cgroup never imply
completion. The retired branch requires the previously live original held event's
raw ENODEV plus exact same-parent/name ENOENT under revalidated original mount and
protected ancestry; it proves no live tasks/online child group in that original
retired domain, not child-reaping or all CSS references released.

Normal Cargo wiring is intentionally pending: declare the Linux root/client
binary targets, package the C/headers and these unit sources, compile six native
C objects into one archive, and link libsystemd.so.0 plus librt on Linux. None of
those changes or the public selector is enabled by these source files. Windows
build/selector files are unchanged. The existing bounded native build route can
compile the two actual main sources and complete module graph first.

No installer, daemon, sudoers rule or permanent listener is created here. Native
entry/bootstrap/unit and full lifecycle qualification remains pending. Temporary
qualification must independently bound the socket, template, services and cleanup;
it cannot introduce an unreviewed socket drop-in because the current source checks
exact configuration provenance. The Linux 5.14 compatibility floor remains open
until actual entry qualification on supported kernels. Generic/native-timer errors
remain125; source work does not establish complete native timeout-cause124 parity.

Prior failed attempts retain their original outcomes. Public evidence requires
privacy review; raw host/account/SSH transport artifacts stay local.

Every synchronous post-bind refusal attempts exact private socket unlink and retains primary plus cleanup errors. PID1 owns the per-instance RuntimeDirectory and removes its remaining socket after stop, including abrupt self-timer death; the service verifies the effective directory/preserve/mode settings and holds its protected directory identity. This cleanup behavior still requires actual native entry qualification.

Post-presentation library failures currently return incomplete owning125 quarantine. A live host can leave the service/payload until the original cutoff. This does not satisfy public teardown-on-error parity and blocks enablement until native bounded cancellation/join is implemented and qualified.
