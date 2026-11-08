# Staged Linux product entry

These are product source entry points, not installed services or a test framework.
The normal cross-platform selector remains fail-closed for Linux.

- `../root_main.rs` builds the protected root-service executable installed as
  `/usr/libexec/process-tree-guard-root`. Its only accepted role is `--service`.
- `../client_main.rs` builds the ordinary standalone client. It captures the
  original parent/all-ancestor capabilities and absolute BOOTTIME cutoff before
  activation, then sets NNP only on its own process. It requires the explicit
  first flag `--separate-service-context`; no default context acknowledgment exists.
- `run_library` refuses125 before capture/settings/activation because transparent
  caller confinement/resource inheritance is not implemented. The distinct
  `run_library_delegated(command,seconds,mode,SeparateService::Acknowledged)`
  requires existing NNP=1 and never changes the caller's process policy. It keeps
  the original requester plus manual parent or complete auto ancestor set.
  Acknowledgment discloses a separate service context; it grants no authority.
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

The prior847 checkpoint returned incomplete owning125 quarantine without a post-presentation cancellation/join. Cancellation v2 below implements the missing path; unconfirmed outcomes still retain quarantine and runtime qualification remains required.

Cancellation v2 is now present in source: the same accepted private connection is directly watched by the native watchdog and terminal controller, with a first-cancellation cause and bounded client shutdown/join. The prior quarantine gap remains historical; current behavior compiles and its ordinary tests pass; native lifecycle qualification remains required. Only a valid native frame plus exact service/domain/manager join can complete a failed call; otherwise the125 outcome retains quarantine.

The previous d6 source passed11 ordinary build/link steps and10 tests, including read-only caller-thread umask capture and three parent-only gate mappings. Child umask/watchdog/terminal/root-client lifecycles remain unexecuted. Tested78 adds deny-default administrative delegation and PTGEX6 acknowledgment; its11 build/link steps and12 ordinary tests passed. Parser/default-refusal/IPC checks did not open a real /etc policy or exercise privileged enforcement. Nothing installs or activates a socket/service.

The fixed policy `/etc/process-tree-guard/delegation.policy` is at most4096 bytes:
`PTG-DELEGATION-1\ncontext separate-service\n` followed by0..64 strictly increasing
`uid N\n` lines, canonical decimal1..4294967294. Empty grants deny; comments, CR,
unknown text, duplicates and wildcards refuse. The service retains the protected
root-owned nofollow directory/file chain and exact bytes. The actual authenticated
requester UID must be granted before RuntimeMax extension, private listener or
provisioning; the same held snapshot is revalidated before provision, payload
creation and permit. Snapshot changes refuse admission/release, not an atomic
promise to revoke an already executing run. Both requests bind byte7=1 in the
40-byte PTGEX6 header; mask remains at38.

No caller seccomp/LSM/Landlock/rlimit/cgroup-ancestry replay is provided. The
separate-service grant is an explicit administrative capability and does not
complete transparent-context requirements. Default dispatch remains disabled.

The mode2 CORE-window source compiles and its12 ordinary tests pass; its actual native protection remains unqualified: it accepts host2 only through the reviewed family/protected-current-ELF/hard-limit gate and protected credential choreography. No sysctl or caller/parent limit is changed. The existing private runtime001 capsule remains frozen0-only and is incompatible with the observed host2 until root explicitly reviews/replaces its inputs after qualification. No installation is authorized by this source work.
