# Measurement
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

How speed is argued for this tool, what each principal path costs counted
from the source, what has been proved equivalent to what, and what has not
been measured at all. The census in section 2 was counted from the source
named in each row. Every number in it is a count of operations in the code
as it stands; it can be recounted from the same functions.

## Contents

- [1. The method](#1-the-method)
- [2. The census](#2-the-census)
  - [2.1 The hook verdict path](#21-the-hook-verdict-path)
  - [2.2 The plain run path](#22-the-plain-run-path)
  - [2.3 The extendable run path](#23-the-extendable-run-path)
- [3. The differential-proof inventory](#3-the-differential-proof-inventory)
- [4. Evidence tiers, and what is not measured](#4-evidence-tiers-and-what-is-not-measured)
- [5. The claims discipline](#5-the-claims-discipline)
- [6. Verification of this document](#6-verification-of-this-document)

## 1. The method

On this class of tool, speed is argued by counting the external operations
a path performs: the native calls it makes, the heap allocations that scale
with its input or its environment, the threads it creates and the processes
it spawns. Wall-clock measurement of the Windows product cannot run on the
Linux machine this tree is developed on — there is no Windows and no MSVC
target here — so a count is the evidence that can be produced and checked
on this machine, and the campaign in section 4 is the evidence that cannot.

The yardstick is the reference design's counted win: a spawn-and-die guard
at about 14 external operations, with zero standing surface and one spawn
per call. A path of this guard is held against that shape: a small constant
number of operations on the ordinary route, nothing standing between
calls, and no spawn but the command's own.

What a count is, and what it is not:

- A count is a source-verifiable fact. The calls, allocations, threads and
  spawns of a path are in its code, and a reader can recount them.
- A count is not a time. What one native call, one allocation or one spawn
  costs on Windows is a price, and no price is measured by counting. Per-
  operation prices on Windows, and MSVC startup behaviour, are theory
  until the benchmark campaign of section 4 measures the built product.
- A count therefore settles "what does each path cost, counted". It does
  not settle "who is faster". Only a measured campaign settles that, and
  no speed verdict in this tree rests on a count.

The census counts named native calls — the `extern` calls declared in
`src/sys.rs` — one by one, with the file and function each was counted
from. Standard-library operations that reach the operating system (a file
probe, an environment read, a file write through `std::fs`) are listed
separately as standard operations, not folded into the native tally:
folding them in would price a standard call as if its cost were known to
equal one declared call, which is exactly the assumption the count refuses
to make. Allocations are counted only where they scale — with the number
of arguments, the length of `PATH`, the size of the environment, the
membership of the job, or the size of a payload or a line. A fixed small
allocation that every call of a path pays alike is named where it occurs
but is not the census's subject.

[↑ Back to Contents](#contents)

## 2. The census

Three principal paths, each on its ordinary route. "Ordinary" is defined
per path, and every place where the count depends on an input says so and
gives the dependence instead of a constant.

### 2.1 The hook verdict path

The route: `diagnostics::cli_exit` calls `cli_process::enter`, the entry
dispatches to `hook::main` for a verdict vendor (`pretooluse` or
`cursor`), `hook::scan_verdict` streams the payload from standard input,
and one decision line is written with `cli_process::write_stdout_line`.

Ordinary route, as counted: the allow decision; the payload arrives on a
pipe or file (standard input is not a console, so it is read through its
raw handle); standard output is not a console; the payload is `L` bytes
with `0 < L <= 16384`; the decision line fits the writer's 512-byte stack
line. Named native calls:

| Step (source) | Calls | Count |
|---|---|---|
| Hardening (`cli_process::enter` → `harden_self` → `deny_acl`, for the process object and the thread object) | `GetCurrentProcess` 1, `GetCurrentThread` 1, `GetSecurityInfo` 2, `LocalFree` 2, `SetSecurityInfo` 2 | 8 |
| Console probes of the standard handles (`cli_process::probed_handle`, once per handle per invocation: each handle is probed at its own first use and kept in its own slot) | `GetStdHandle` 2, `GetConsoleMode` 2 (standard input and standard output; standard error is not used on this route) | 4 |
| The executable's own spelling (`hook::stack_self_path`) | `GetModuleFileNameW` 1 | 1 |
| Standard input handle (`cli_process::standard_input`; the handle and the console verdict are the ones its slot cached, so neither a second fetch nor a second probe) | — | 0 |
| Payload reads (`hook::Scanner` over the raw standard input) | `ReadFile` per window fill | 2 at this size (see below) |
| The decision line (`cli_process::write_stdout_line`, byte form; its handle is the cached one above) | `WriteFile` 1 | 1 |
| Process exit (`diagnostics::cli_exit`) | `GetCurrentProcess` 1, `TerminateProcess` 1 (the guard itself) | 2 |
| **Total, allow** | | **18** |

Threads created: 0. Processes spawned: 0.

The deny decision writes a second line (the reason, on standard error)
and builds its text first: `WriteFile` 1 more, and the standard-error
handle's own probe (`GetStdHandle` 1, `GetConsoleMode` 1) is paid
there, for a total of 21, and two more allocations (the formatted
reason and the answer buffer; the policy path the reason names is
spelled from the executable's two folder splits inside that format,
not built as a string of its own first).

Standard operations, not in the tally: one or two standard
environment reads, paid once per process and cached for the process —
the trace name, read new-first as `PROCESS_TREE_GUARD_TRACE`,
falling back to `PROCESS_GUARD_TRACE`, by `trace_enabled_live`
behind the entry's `trace("start")` call and held in its `OnceLock`
slot thereafter: one probe when the new name is set, two when it is
not — so this is a once-per-process cached read, not a per-call
count, and no per-call total above moves for it.

Dependences on this path:

- Payload reads. The scanner holds a 16 KiB window (`hook::Scanner`) and
  fills it with one `ReadFile` per fill. A payload of `L > 0` bytes takes
  the fills that carry it plus one final fill that reports the end of the
  input: `ceil(L / 16384) + 1` reads when every read returns all it was
  asked for. A pipe that returns short reads adds a fill per short return.
  The payload cap is 16 MiB (`hook::MAX_INPUT`); the count above is the
  one-window case.
- The scan itself allocates nothing on the heap: the window is stack
  storage, and the envelope checker (`hook::EnvelopeCheck`, with its
  `TimeoutCheck` and `PayloadCheck`) keeps only counters and cursors, in
  stack storage, borrowed from the executable's spelling. No value tree
  of the payload is built.
- Hardening's access-list storage (`cli_process::AclOut`) is stack
  storage when the built list fits 1,024 bytes and one heap allocation
  otherwise; which of the two occurs depends on the size of the access
  list the process already has.
- Arguments: the argument iterator materialises one `OsString` per
  argument it yields, and this route consumes the vendor argument (and,
  for the full command, the `hook` token before it). Nothing else on the
  route scales with the argument count.
- A decision line longer than 512 bytes takes the allocating form of the
  writer (one formatted string, and on a console one `Vec<u16>`), and a
  write the stream accepts only in part is looped, one `WriteFile` or
  `WriteConsoleW` per part. A console standard input is read through the
  standard console path instead of `ReadFile`; that route is not the one
  counted here.

### 2.2 The plain run path

The route: `cli::run_args` → `engine::run_job_core` with no extension
settings. The executable is resolved, a job is created, the child is
created suspended inside the job by the creation attribute list, resumed,
waited on with its owner, and the job is proved empty at teardown.

Ordinary route, as counted: the command is named without a path, so
`engine::which` searches for it; the child is a distinct process for each
of the three standard handles, all present; a console is attached; the
child exits by itself during the first plain wait pass, leaving no other
process in the job; the process that launched the guard is a system
boundary program at its verified path, so the console finish's walk
breaks at the first level it examines; the timing instrumentation is off (neither
the trace — `PROCESS_TREE_GUARD_TRACE`, falling back to
`PROCESS_GUARD_TRACE` — with a timing nonce nor the trace alone changes any
count below except as noted in section 4). Named native calls:

| Step (source) | Calls | Count |
|---|---|---|
| Entry hardening (`diagnostics::cli_exit` → `cli_process::enter` → `harden_self` → `deny_acl`, for the process object and the thread object; the same row §2.1 counts for the hook route) | `GetCurrentProcess` 1, `GetCurrentThread` 1, `GetSecurityInfo` 2, `LocalFree` 2, `SetSecurityInfo` 2 | 8 |
| Automatic-name check (`lib::entry_full` / `lib::entry_deadline` → `auto_by_name`, the stack form; the contain and gate entries make no such check) | `GetModuleFileNameW` 1 | 1 |
| Job setup (`engine::run_job_core`) | `CreateJobObjectW` 1, `SetInformationJobObject` 1 | 2 |
| Standard handles (the handle loop in `run_job_core`; on a process-owning invocation the three handles are read back from their per-handle slots — each slot's one probe is paid at its first read here, and no `GetStdHandle` is paid a second time for any of them. The current-process pseudo-handle only the returning-library branch duplicates with is fetched on that branch's first duplication and kept for its later ones, so this route pays no fetch for it) | `GetStdHandle` 3, `GetConsoleMode` 3 (the one probe of each handle's slot, paid at this first read), `SetHandleInformation` 3 (one per distinct handle) | 9 |
| Owner, the parent only (`engine::watched_owner_handles` and `engine::parent_process_id_on` in `src/engine/owners.rs`; the one pseudo-handle fetch serves both the creation-time read of this process and the parent query through it) | `GetCurrentProcess` 1, `GetProcessTimes` 2, `NtQueryInformationProcess` 1, `OpenProcess` 1 | 5 |
| Console probe (`cli_process::Console::probe`) | `CreateFileW` 1 (the `CONIN$` device opens; 2 when `CONIN$` fails — whether `CONOUT$` then opens, so the probe succeeds with input missing, or neither console device opens) | 1 |
| Creation attributes (`engine::JobAttributeList::new`, stack storage) | `InitializeProcThreadAttributeList` 1, `UpdateProcThreadAttribute` 2 (the job list and the handle list) | 3 |
| Deadline anchor (`engine::fixed_deadline`) | `GetTickCount64` 1 | 1 |
| Spawn (`run_job_core`) | `CreateProcessW` 1 | 1 |
| Pre-resume check (`engine::poll_before_resume` and the deadline read in `fixed_pre_resume_with`, `src/engine/wait.rs`) | `WaitForMultipleObjects` 1, `GetTickCount64` 1 | 2 |
| Resume (`run_job_core`) | `ResumeThread` 1 | 1 |
| Wait, first plain pass, the child exits in it (`engine::wait_with_deadline_observed`) | `GetTickCount64` 1, `WaitForMultipleObjects` 1 | 2 |
| Exit code (`run_job_core`) | `GetExitCodeProcess` 1 | 1 |
| Teardown, the job already empty (`engine::terminate_and_wait` → `active_processes`) | `QueryInformationJobObject` 1 | 1 |
| Console finish, fixed part (`cli_process::Console::finish`) | `GetSystemWindowsDirectoryW` 1; for an attached console `FlushConsoleInputBuffer` 1, `GetConsoleProcessList` 1, `FreeConsole` 1 | 4 |
| Console finish, first walk level (`cli_process::Console::finish` → `boundary`, on the first owner; the walk examines at least this one level on every run that reaches the finish, and the ordinary route's first owner is a boundary, so it breaks there) | `QueryFullProcessImageNameW` 1 | 1 |
| Process exit (`diagnostics::cli_exit`) | `GetCurrentProcess` 1, `TerminateProcess` 1 (the guard itself) | 2 |
| **Total, fixed core** | | **45** |

The subtotal from job setup to process exit is 36; the total above is
the entry-to-exit count, on the same boundary §2.1 uses for the hook
route (which counts its entry hardening and its process exit). A
contain-role run, whose entry makes no automatic-name check, totals
44 on this boundary for its run core — but a contain run cannot start
at all without its mandatory enclosing-run request, which the core
above does not include: before the job is set up it pays the client
exchange of §2.3 (13 named calls when the request is granted on the
first attempt), plus the one-time `CreateEventW` of its thread's
first exchange, for an entry-to-exit total of 58 on an ordinary
successful contain run. A plain run started inside an extendable run
pays the same request the same way; the ordinary route counted above
is a top-level run, which pays none of it (its channel probe finds no
channel and makes no pipe call).

Threads created by the guard: 0. Processes spawned: 1 (the command).

Resolution is not in the native tally because `engine::which`
(`src/engine/resolve.rs`) makes no `sys.rs` call. What it costs:

- Filesystem probes: one `std::path::Path::is_file` probe per candidate
  until the first hit. The candidates per folder are 1 when the name
  already carries an extension from `PATHEXT`, and otherwise one per
  `PATHEXT` entry (`E` entries; four in the default list the code falls
  back to). The folders are the current folder (unless
  `NoDefaultCurrentDirectoryInExePath` is set) and then the `D` folders
  of `PATH`, in order, so the probe count `P` satisfies
  `1 <= P <= (D + 1) * E`, set by the hit's position. On a process-owning
  invocation both parsed lists are read once and cached for the process.
- Allocations: the `PATHEXT` text (one string and one vector of its entry
  ranges), the search folders (one `PathBuf` per `PATH` entry, `D` of
  them), one candidate `PathBuf` reused for every probe, and one result
  string for the hit.

Standard operations, not in the tally: five standard environment
reads, or six when the trace name is read at its fallback spelling,
each paid once per process and cached for the process — the trace
name, read new-first as `PROCESS_TREE_GUARD_TRACE`, falling back to
`PROCESS_GUARD_TRACE` (read by `trace_enabled_live` behind the
entry's `trace("start")` call and held in its `OnceLock` slot),
`PROCESS_TREE_GUARD_CHANNEL` (the `register_start` channel probe,
whose channel probe the prose above mentions, read through the
channel name's `OnceLock` cache in `extend::client`), and the
resolver's three, `PATHEXT`, `PATH` and
`NoDefaultCurrentDirectoryInExePath` (each read once and held in the
resolver's `OnceLock` caches on a process-owning invocation —
`PATHEXT` in its parsed list, `PATH` and
`NoDefaultCurrentDirectoryInExePath` together in its search
directories — while a returning library call reads them live per
call, as the resolution prose above states for the lists themselves)
— so these are once-per-process cached reads, not per-call counts,
and no per-call total above moves for them.

Dependences on this path:

- Wait passes. The tally holds the pass the child exits in. Each further
  plain pass costs `GetTickCount64` 1 and `WaitForMultipleObjects` 1.
  After three plain passes the wait arms a suspend-inclusive timer: one
  `CreateWaitableTimerExW` (and its `CloseHandle` when the wait ends),
  and each timer pass costs `GetSystemTimeAsFileTime` 1, `GetTickCount64`
  1, `SetWaitableTimer` 1 and `WaitForMultipleObjects` 1
  (`wait_with_deadline_observed`). A plain pass waits at most 250 ms of
  the time left; a run whose child exits at once never creates the timer.
- Teardown with the job not empty. When `active_processes` does not
  report zero, `terminate_and_wait` adds: one `QueryInformationJobObject`
  for the member list (repeated only while the list outgrows the scratch
  buffer), one `OpenProcess` per member (`M` members), `TerminateJobObject`
  1, one `WaitForMultipleObjects` per window of at most 64 members, one
  `CloseHandle` per member, and the empty-wait's polls — one
  `QueryInformationJobObject` per poll with `Sleep` 10 ms between polls —
  all inside the one 5,000 ms teardown budget. A child not already known
  to have exited is then waited on once (`WaitForSingleObject`) with what
  is left of that budget.
- The console finish walk. After its fixed part, `Console::finish` walks
  the owners from the first owner up, at most 64 levels. The finish
  builds no expected-path strings: its boundary test is the engine's
  in-place segment comparison (`engine::ends_the_walk`) over the one
  Windows folder string the finish queried. The first level
  examined is counted in the tally above; each further level
  examined costs `QueryFullProcessImageNameW` 1 (its image path). Each
  level that is not a system boundary adds `GetProcessId` 1,
  `NtQueryInformationProcess` 1 and `OpenProcess` 1, and, when attaching
  to that level's console succeeds, `AttachConsole` 1 with the flush it
  drives (`CreateFileW` 1, `FlushConsoleInputBuffer` 1, `CloseHandle` 1,
  `GetConsoleProcessList` 1, `FreeConsole` 1). Creation times are read
  once per retained handle — `levels + 1` `GetProcessTimes` over the
  whole walk, because each parent's time is carried to the next level —
  and each superseded parent handle is closed once, the last one at the
  end. The walk's cost therefore scales with the number of ancestors
  above the process that launched the guard, up to its 64-level bound.
- The owner fallback. The tally takes `NtQueryInformationProcess`
  succeeding. When it does not, `parent_process_id_by_snapshot` enumerates
  the system's processes instead: `GetCurrentProcessId` 1 (the
  identifier it scans the snapshot for), `CreateToolhelp32Snapshot` 1,
  `Process32FirstW` 1, `Process32NextW` once per process on the system,
  and `CloseHandle` 1 — a cost that scales with the system's process
  count, on a route the ordinary run does not take.
- Allocations that scale: one string per command-line argument (the
  argument vector and the child vector); the resolution allocations
  above; the wide application and command lines, which are stack storage
  up to 260 and 1,024 UTF-16 units respectively and spill to one heap
  vector each past those lengths; in the console finish, one image
  string per level walked, and the flushed-process list, which grows
  with the console's process list — the folder units the finish
  compares against are stack-held on both routes into it (queried
  into its own buffer, or encoded from the handed-in string into
  stack storage, spilling to a heap vector only for a handed-in
  string past 512 units, which no route in this tree builds), so
  the finish allocates no folder storage. The handle lists of the run are inline up to
  16 entries and spill to one vector past that. An extendable run's
  environment block is the plain path's one scaling allocation the plain
  route does not pay: the plain route builds none.

### 2.3 The extendable run path

The route: `cli::run_args` → `engine::run_job_with` →
`engine::run_job_core`, whose extension starter is
`Extension::start` (`src/extend/server.rs`). The run otherwise follows
the plain path, with three differences counted where they occur: no
fixed deadline anchor is read before creation (the deadline is the
holds'), the child's environment carries the channel name, and the wait
is the extendable wait. This section counts what the extension itself
adds: its start, the supervisor's passes, and one grant.

Ordinary route, as counted: the cap is at or below the default, so no
approval is asked; an `--extension-log` file is given and opens as a
regular file; the run is not enclosed in another run.

Extension start (inside `run_job_core`, after the job is set up):

| Step (source) | Calls | Count |
|---|---|---|
| Opening the log (`extend::open_log` → `extend::check_log_path`, `Log::start` in `src/extend/log.rs`) | `GetFileType` 1 (on the check's open; the handle that open made is handed through to the audit file, not opened or typed again) | 1 |
| The channel name (`extend::channel_name` → `sys::secure_random_16`, first use) | `LoadLibraryExW` 1, `GetProcAddress` 1, `GetModuleHandleExW` 1, and the generator call itself (`SystemFunction036`) 1 | 4 |
| The deadline base (`Extension::start` → `tick`: the tick the run's own deadline is counted from) | `GetTickCount64` 1 | 1 |
| The wake and stop events (`Extension::start`) | `CreateEventW` 2 | 2 |
| The pipe instances (`extend::create_pipes`, including `current_user_sid`) | `GetCurrentProcess` 1, `OpenProcessToken` 1, `GetTokenInformation` 1, `ConvertSidToStringSidW` 1, `LocalFree` 1, `CloseHandle` 1 (the token), `ConvertStringSecurityDescriptorToSecurityDescriptorW` 2, `CreateNamedPipeW` 4, `SetKernelObjectSecurity` 4, `LocalFree` 2 | 18 |
| The completion port (`Extension::start`) | `CreateIoCompletionPort` 1, `SetInformationJobObject` 1 | 2 |
| Hardening the two threads started here (`cli_process::harden_worker` → `harden_thread`, once per thread) | `GetSecurityInfo` 2, `LocalFree` 2, `SetSecurityInfo` 2 | 6 |
| The gates the two threads wait on before they run (`extend::spawn_gated`, one per thread: the starter creates the event and sets it at the release; the thread waits on it and closes it) | `CreateEventW` 2, `SetEvent` 2, `WaitForSingleObject` 2, `CloseHandle` 2 | 8 |
| The first serve thread's own operation event (`extend::serve`, created as the thread starts and closed as it ends) | `CreateEventW` 1 | 1 |
| **Total, extension start** | | **43** |

Threads created at the start: 2 (one serve thread for the first pipe
instance, and the job monitor thread). Each of those two threads is
started under a name built as one `String` — `"extension-server"` for
the serve thread and `"extension-monitor"` for the monitor thread
(the starter's `name.into()` in `Extension::start`): two fixed
allocations at the start, one per thread, neither scaling with any
input — and none for the thread bodies themselves, which the
starter takes as the closures they are rather than boxed trait
objects. The log's sink names are built the same way when the log is
made: one String per sink (`Sink::start_with`'s `name.into()` — two
sinks with an audit file, one without), fixed allocations of the same
kind, though no sink thread is started there. Two more fixed
allocations of the passage are named here so the passage's account
is complete: the channel name is one String, built once by
`channel_name` and held in the Extension for the run; and each sink
holds its writer as one boxed closure (`Sink`'s `write` field), one
box per sink of the same counts as the sink names, with the file
sink's idle hook and the audit file's alert boxed the same way. The log's own threads are not
among them: a log sink's thread starts on its first queued line, not at
the start (see the grant below). Standard operations at the start, not
in the tally: the log file is opened once through `std::fs` (the path
check), the handle that open made is handed through to the audit file
and held for the run's appends, and its metadata is read once to seed
the tracked size. Without `--extension-log`,
`standard_error_records` is asked instead of the file checks:
`GetStdHandle` 1, `GetConsoleMode` 1, and `GetFileType` 1 when standard
error is not a console. The first two are standard error's one slot
probe, paid here because this ask comes first: the handle loop of
§2.2 reads that slot back, so an extendable run without a log file
pays the pair once in all, at this ask — the pair §2.2 counts at the
handle loop is the same pair, not a second one.

The child's environment block (`engine::environment_block`) allocates in
proportion to the environment it copies: the entries are held as
`vars_os` yielded them (no per-entry encoding is built to hold them),
one upper-case string for each variable whose name is not already
upper-case ASCII text its entry holds (a name that already is borrows
its entry's text, building nothing), one sort-key row and one sort
index per variable, and the block itself, reserved to a byte-length
upper bound on its unit size and written with each entry encoded once,
straight into it. The sort borrows each key's held text in place at
every comparison instead of re-forming its lossy text per comparison.
It scales with the number of variables
and their total length, and makes no native call.

Supervisor passes. The supervisor waits in `Extension::remaining` plus
one wait per pass (`engine::wait_extendable` → `wait_until` →
`wait_with_deadline_observed`, with the extension's wake event in the
wait set):

- One plain pass costs `GetTickCount64` 1 (in `remaining`, under the
  state lock) and `WaitForMultipleObjects` 1: 2 named calls.
- A pass that carries the wake event waits in slices of at most
  1,000 ms (`wait_slice_wake`, the `BACKSTOP_WAKE_MS` of `src/extend.rs`),
  so an idle run makes about one pass per second, and a grant or a job
  exit ends the pass at once through the wake event instead.
- From the fourth pass the timer is armed, as on the plain path: one
  `CreateWaitableTimerExW` for the wait (closed when it ends), and 4
  named calls per pass (`GetTickCount64`, `GetSystemTimeAsFileTime`,
  `SetWaitableTimer`, `WaitForMultipleObjects`).
- `remaining` with `H` holds standing adds, per pass, the job's process
  list query (`QueryInformationJobObject` 1, into a thread-held buffer
  that is reused and grows only to the largest list its thread has seen)
  and one aliveness poll per hold the list still names
  (`WaitForSingleObject` with a zero timeout, in `State::release`:
  a hold the list no longer names is dropped on the in-place
  membership search alone, with no poll). With no holds it adds
  neither.
- The monitor thread (`extend::watch_job`) blocks in
  `GetQueuedCompletionStatus` with a 10,000 ms timeout and sets the wake
  event (`SetEvent` 1) for each job-exit packet it receives. It is the
  run's second standing thread, beside the supervisor itself.

One grant, steady state — not the run's first connection and not its
first log line, whose one-time costs follow the table. The asking side
is a process in the job asking for its own deadline (`Target::Caller`),
and the grant stands.

| Side (source) | Calls | Count |
|---|---|---|
| Server: accept (`extend::accept`, the overlapped connect completes pending) | `ResetEvent` 1 (the connect's event, reset before the connect is posted), `ConnectNamedPipe` 1, `GetLastError` 1, `WaitForMultipleObjects` 1, `GetSystemTimePreciseAsFileTime` 1, `GetOverlappedResult` 1 | 6 |
| Server: who the client is (`extend::handle_client`, `extend::arrive`, `Process::open`) | `GetNamedPipeClientProcessId` 1, `OpenProcess` 1, `GetProcessTimes` 1, `QueryFullProcessImageNameW` 1, `IsProcessInJob` 1, `GetTickCount64` 1 (the rate admission's clock) | 6 |
| Server: the request (`Io::read` in `src/extend/serve.rs`, finished by `finish_io`) | `ResetEvent` 1, `ReadFile` 1, `GetLastError` 1 (the pending finish reads the error state before it waits), `WaitForMultipleObjects` 1, `GetOverlappedResult` 1 | 5 |
| Server: the decision (`extend::client_gone`, `extend::evaluate`) | `PeekNamedPipe` 1, `WaitForSingleObject` 1 (the aliveness poll of `started_before`), `GetTickCount64` 1 (the grant's clock), `SetEvent` 1 (the wake) | 4 |
| Server: the reply (`Io::write`, finished by `finish_io`) and the disconnect (`extend::serve`) | `ResetEvent` 1, `WriteFile` 1, `GetLastError` 1 (the pending finish, as in the request), `WaitForMultipleObjects` 1, `GetOverlappedResult` 1, `DisconnectNamedPipe` 1 | 6 |
| **Server total per grant** | | **27** |
| Client: connect and exchange (`extend::connect`, `extend::exchange` in `src/extend/client.rs`, finished by `finish_io`) | `CreateFileW` 1, `SetNamedPipeHandleState` 1, `WriteFile` 1, `ReadFile` 1, `ResetEvent` 2 (the exchange event is thread-local: created once per thread and reset before each phase, not created and closed per exchange), `GetLastError` 2 (the pending finish of each phase, before its wait), `WaitForMultipleObjects` 2 (one per overlapped finish), `GetOverlappedResult` 2, `CloseHandle` 1 (the pipe) | 13 |

Standard operations, not in the tally: one wall-clock query through
the standard clock (`SystemTime::now`) per grant, kept refusal or release
on the server side, for the log line's time and, on a grant, the
reply's deadline.

Dependence on the holds already standing: the server total above is
exact when no hold stands when the request arrives (the run's first
grant). The client's process handle is not closed on that grant:
`State::grant` moves the opened `Process` into the hold it creates,
and the handle is closed only when the hold is later reaped, dropped
or cleared at shutdown. `arrive` and `State::grant` each settle the deadline first
(`State::ended` → `State::expire` → `State::release`), and with `H`
holds standing each of those two passes pays the release's job
process-list query (`QueryInformationJobObject` 1) and one aliveness
poll per hold whose time has not run out and which the list still
names (`WaitForSingleObject` with a zero timeout, up to `H`): a grant taken while holds stand therefore
pays 2 more named calls plus up to `2H`
polls on the server side, beyond the 27 above — unless the two
settles share a tick. `State::release` keeps a stamp, the tick of
its last completed reap, and a settle called with that same tick
skips the job-list query and the aliveness polls and stands on that
reap's answers (the deadline is still folded on every settle, as
before): per grant or renewal whose `arrive` and grant settles
share a tick, one `QueryInformationJobObject` and up to `H`
aliveness polls are saved, so such a grant pays 1 more named call
plus up to `H` polls beyond the 27 above. A renewal — a grant
whose (process, token) already stands — pays one more besides: the
newly opened client `Process` is not moved into a hold (the standing
hold keeps its own handle), so it is dropped as `State::grant`
returns, for one `CloseHandle`. (The supervisor's
passes pay the same release once per pass, as counted under
"Supervisor passes" above; a refusal decided before `State::grant`
is reached pays only `arrive`'s pass.)

A call that completes synchronously instead of pending skips its wait,
its error-state read and its reap: one `WaitForMultipleObjects`, one
`GetLastError` and one `GetOverlappedResult` fewer on whichever side
completed at once (`finish_io` returns the count the initiating call
itself wrote into its caller's out-variable).
A busy channel makes the client's connect retry (`WaitNamedPipeW` once
per retry, inside its 5,000 ms bound in `CONNECT_MS`).

One-time costs around the grant:

- The first connection a serve thread takes starts the serve thread of
  the next pipe instance (`extend::spawn_next`): `CreateEventW` 1 (its
  gate), one thread creation, its hardening (`GetSecurityInfo` 1,
  `LocalFree` 1, `SetSecurityInfo` 1), `SetEvent` 1 (the gate), and in
  the new thread `WaitForSingleObject` 1 and `CloseHandle` 1 (the gate
  again), plus the new thread's own operation event (`CreateEventW` 1,
  as in the extension start above) — 8 named calls and 1 thread, and
  one String for the new thread's name, built as at the start. The fleet grows by one thread per
  concurrent connection up to the pipe's instance count, 4 (`INSTANCES`
  in `src/extend.rs`), and no further.
- The first line queued for the log starts that sink's thread
  (`Sink::ensure_started` in `src/extend/log.rs`): one thread creation
  and its hardening (3 named calls) and the gate the thread waits on
  before it takes a line (`CreateEventW` 1 and `SetEvent` 1 in the
  starter, `WaitForSingleObject` 1 and `CloseHandle` 1 in the thread)
  per sink — two sinks with an audit file (standard error and the
  file), one without.
- The client's first exchange on its thread creates that thread's
  exchange event (`extend::exchange_event` in `src/extend/client.rs`):
  `CreateEventW` 1, closed when the thread ends. The client total of
  13 above is a thread that has exchanged before; a one-shot asking
  process (the `extend` program, a nested guard's start) makes its
  only exchange on a fresh thread and pays this call on it, as the
  contain total in §2.2 already counts for its route.
- The grant's log line itself: the serve thread builds one line string
  and queues that one build for every sink (shared storage; a second
  sink queues a reference, not a copy). The writing is the sink
  threads' work: the standard-error sink writes through the counted
  line writer (`WriteFile` or `WriteConsoleW` 1 in steady state — the
  sink thread resolved its handle and console verdict once, on its
  first line, through one `GetStdHandle` and one `GetConsoleMode`,
  because the cached verdict of `cli_process::enter` is a property of
  the entering thread), and the file sink
  makes one file write through `std::fs` per line. With no audit file a
  grant stands only once its line has been written to standard error
  (`Log::record`, waited for at most 500 ms); with a file, the line is
  queued and not waited for.

Allocations that scale on this path, beyond the plain path's: the
environment block above; the hold table, which grows with the number of
live holds (at most 256, `MAX_HOLDS`); the rate table, one entry with
one image string and one vector of request times per distinct asking
process (at most 1,024 entries, `MAX_RATE_KEYS`); per connection, the
client's image strings (the message itself is held inline, in the
64 bytes the read already made, and copies into no vector); and the
log lines as described. The four pipe instance handles and each
serve thread's list of unserved instances are not allocations: both
are fixed stack storage bounded by `INSTANCES`.

Shutdown of the extension (`Extension::shutdown`, after the tree is
gone) adds `SetEvent` 1, `CancelIoEx` 4 (one per pipe instance),
`PostQueuedCompletionStatus` 1, `GetTickCount64` 1 (the tick the
final notices are sampled at) and `CloseHandle` 7 (the four instances,
the stop event, the port and the wake event): 14 named calls in
`shutdown` itself, and the joins of the threads it started. Accepts
are charged by one attribution rule: each accept is charged to the
connection it serves, and the final dangling accept of each serve
thread — the one no client ever completes — is charged to shutdown.
On the never-asked run this census quantifies, that dangling accept
is guaranteed to have been posted pending, so its issue is charged
here: `ResetEvent` 1, `ConnectNamedPipe` 1, `GetLastError` 1 and
`WaitForMultipleObjects` 1 (infinite), 4 named calls; and its
completion during the joins is charged here too: `GetOverlappedResult`
1 and the close of its own operation event (`CloseHandle` 1 per serve
thread, one on such a run), 2 named calls — for 20 named calls in all
on such a run (14 + issue 4 + completion 2), 21 when that serve
thread's own cancellation also fires. `shutdown` runs once: it is called
explicitly at the end of the run and again by the extension's
`Drop`, and a completion flag set by the first call makes the
second return at once, paying none of the above again.

[↑ Back to Contents](#contents)

## 3. The differential-proof inventory

Places in this tree replace a whole-value computation with a streaming
or segment form that must give the same answers. What the tree records
about each is stated here as it stands in the tests and comments,
together with the differential evidence executed during development and
recorded in the development record outside the tree; where neither the
tree nor that record has a comparison count, none is claimed. An
executed count's provenance is stated the same way for each item below:
the harness was executed during development; the harness itself is not
preserved as an in-tree test; durable in-tree differential tests are the
test suite's deliverable.

**The streaming verdict scanner against the parsed verdict** —
`hook::scan_verdict` against the whole-string form. The reference is
kept in the tree: `parse_json` builds the value tree and
`hook::valid_envelope` checks it with `envelope_for`, and the test
helper `dom_verdict` (`src/hook.rs`, tests) computes the parsed verdict
for comparison. Evidence: in-tree unit tests.

- `streaming_verdict_matches_the_parsed_verdict` asserts scan equals
  parsed verdict for every payload in its list — envelope spellings in
  `command` and in `cmd`, repeated keys with the last value winning in
  both positions, non-string values, nested unwatched values, malformed
  numbers, bad and good surrogate escapes, and escaped key spellings —
  each under four executable spellings (the full command's two names,
  the gate, and the empty path) and both cursor settings, and asserts
  that two invalid-UTF-8 inputs are denied.
- `streaming_verdict_keeps_the_payload_cap` covers the 16 MiB cap: a
  valid payload padded with spaces past the cap is allowed as its first
  16 MiB decide, an unterminated payload past the cap is denied, and a
  watched string standing after a 100,000-byte unwatched string decides
  as the parsed verdict decides.
- `streaming_verdict_timeout_tokens_short_and_long` holds 12 listed
  timeout tokens (among them `86400`, `86401`, `.5`, `5.`, `1e3`, and
  tokens of 100 to 400 digits settled from counters) to the same answer
  from the scan and from the parsed verdict.

Executed differential, beyond the in-tree tests above: about 1.3 million
comparisons (mutated, generated and targeted cases, including half-ULP
and subnormal timeout edges, depth, cap and window-crossing cases)
against the old parsed-verdict form, with zero divergences. The harness
was executed during development; the harness itself is not preserved as
an in-tree test; durable in-tree differential tests are the test suite's
deliverable.

**The bulk-run scanner change** — inside `Scanner::string`, a sink that
discards its characters (`Skip`, and a `KeyProbe` with no names) may
consume a maximal plain run of a string whole when the run is valid
UTF-8 as it stands; a run the whole-run validation refuses — an invalid
byte in it, or a scalar split by the window edge — falls back to the
per-byte path without consuming anything. The collecting sink of the
Antigravity scan (`Capture`, which holds the one `CommandLine` a
rewrite needs) takes the same validated run as one slice of text: a
valid run starts and ends at a scalar boundary, so the collected text
is byte-for-byte the text the per-character delivery produced.
Evidence: in-tree, and
indirect. There is no test function of its own — that is a stated
gap, not an omission — and the in-tree coverage is indirect. It is
covered by
`streaming_verdict_with_multibyte_paths_and_window_edges`, whose stated
subject includes unwatched strings bulk-consuming plain runs per window
fill: multibyte scalars straddling the 16 KiB fill edges, escapes at the
edges, and long plain runs (16,380, 40,000 and 16,383 bytes in its
filler) are all held to the parsed verdict, in watched strings and in
keys as well, and six listed invalid byte sequences are denied.

Executed differential: 72,514 comparisons (seeds, mutations, generated
structures, escape-heavy strings, window-boundary and chunked reads, cap
behaviour, and full Antigravity decisions) against the pre-change
scanner, with zero divergences. The harness was executed during
development; the harness itself is not preserved as an in-tree test;
durable in-tree differential tests are the test suite's deliverable.

**The dead-sink bulk path** — a sink that can no longer change its
answer (`EnvelopeCheck` once dead, `KeyProbe` once every named slot is
dead, `WatchKey` once every slot is dead under both spellings,
`MatchSink` once not alive) reports `takes_bulk` on that state, so the
rest of its string is validated whole like a discarding sink's instead
of being decoded character by character into an answer that cannot
change. Each such sink's readers read only its live state (`matched`,
`exact`, `case_insensitive`, `finish` all consult the flags the bulk
path leaves untouched), and `push_char` on a dead sink changes nothing a
reader can see. Executed differential: 240,000 comparisons (generated
and mutated verdict payloads under both vendors, both roles and seven
chunk sizes, Antigravity scans and decisions, and PowerShell facts)
against the pre-change scanner, with zero divergences; 71,246 of the
verdict payloads were also held against the parsed verdict, with zero
mismatches in both forms; and a negative control (the envelope check
claiming the bulk path while still live) produced 11,925 parsed-verdict
mismatches, so the harness was not vacuous. The harness was executed
during development; the harness itself is not preserved as an in-tree
test; durable in-tree differential tests are the test suite's
deliverable.

**The payload run form** — inside `Scanner::string`, a sink at the
payload stage of a live envelope check (`EnvelopeCheck` with `stage ==
Payload`, no payload whitespace seen, and not dead) takes a plain run of
the current window for run processing when the run validates as UTF-8 as
it stands, on the same terms as the bulk and capture forms above; a run
that validation refuses falls back to the per-byte path without
consuming anything, so an invalid byte is rejected there exactly as
before. `PayloadCheck`'s run form processes only the run's
whitespace-free prefix — split at the first decoded whitespace character
by `char::is_whitespace`, the full Unicode set — in quartets taken
straight from the run's bytes, carrying its group, grouped, padding,
body and total state across window edges; `total` counts the prefix's
delivered characters only, whitespace is never delivered to it, and
every other prefix character goes through `push_char` itself, so
non-alphabet intrusions, padding placement and the after-padding rule
are that form's by construction. At the first whitespace character the
run form stops unconsumed, and the rest of the string goes through the
per-character path unchanged. The split is found inside the processing
loop itself, not in a pre-pass over the run: a quartet step holds no
whitespace byte, so it can never cross the split, and every other
character is tested with `char::is_whitespace` at its own position
before it is delivered — the prefix delivered, the state carried and the
length reported are the pre-pass form's, with the pre-pass's full decode
of the run removed. Executed differential for this fused form:
13,670,768 comparisons (every Unicode scalar embedded in quartets and
standing alone, generated and mutated payload runs under eight chunk
sizes, long runs crossing window edges with padding and whitespace
inserted, and invalid-UTF-8 runs) against both the pre-pass form and the
pure per-character form, with zero divergences in both; and a negative
control — the fused form testing only the space character in place of
`char::is_whitespace` — diverged 100,241 times over the same corpus, so
the harness was not vacuous. The harness was executed during
development; the harness itself is not preserved as an in-tree test. The
run is validated once, in the scanner: the validated text is handed to
`PayloadCheck` as text, as the collecting form's validated text is
handed to its sink, and `PayloadCheck`'s run form repeats no validation
of its own — a second whole-run validation of every payload run stood in
it before, recovering the text the scanner's validation had already
produced and discarded. Executed differential for this single-validation
form: 22,979,548 comparisons (every Unicode scalar standing alone and
embedded in quartets and padding contexts, generated and mutated payload
runs chunked at character boundaries, long runs crossing window edges
under six chunk sizes, and invalid-UTF-8 runs, which both forms refuse
without consuming or changing state) against both the double-validation
form and the pure per-character form, with zero divergences; and a
negative control — a run form that delivers whitespace to the checker
instead of stopping before it — diverged 94,226 times in 100,000
comparisons, so the harness was not vacuous. The harness was executed
during development; the harness itself is not preserved as an in-tree
test. Executed differential: 1,050,000 comparisons (generated and
mutated payload-bearing inputs under both verdict vendors and nine chunk
sizes, with window-edge variation, padding at edges and split across
edges, every Unicode whitespace class at prefix-end and inside tails,
whitespace-then-non-whitespace tails, multi-byte non-whitespace
intrusions in the prefix, and invalid-UTF-8 runs on the fallback path)
against the pre-change per-character form, with zero divergences and an
identical result hash; 346,653 of the inputs were also held against the
parsed verdict (`parse_json` + `valid_envelope`), with zero mismatches
in both forms; and a negative control — the run form splitting its
prefix at the first space byte alone, so multi-byte whitespace inside a
plain run is counted as payload — diverged 10,422 times over the same
corpus, so the harness was not vacuous. The harness was executed during
development; the harness itself is not preserved as an in-tree test;
durable in-tree differential tests are the test suite's deliverable.

**The Antigravity streaming decision** — `hook::antigravity_scan`
streams the payload and `hook::antigravity_decision` decides from the
scan alone; the payload is never materialised and the only string held
is the one `CommandLine` a rewrite needs. Evidence: in-tree unit and
integration tests of behaviour, not a scan-against-parsed differential
in the tree; the executed scan-against-parsed differential is stated
below with its provenance. The unit test
`antigravity_decision_rewrites_allows_and_denies` feeds its listed
payloads through the scan (the test helper runs `antigravity_scan` with
the payload's own length as its budget) and asserts the decision for
each: a plain command is rewritten and the original is recoverable from
the rewrite's Base64 exactly; the wrapped form fed back passes
unchanged; other tools are allowed untouched; the listed malformed and
ambiguous payloads (repeated keys, keys that match only in another
letter case) are denied; and the length boundary is the one computed
from `MAX_REWRITTEN_LINE` in the source. The integration checks of
`tests/antigravity.rs` (the A series of `tests/PARITY.md`, 42
Antigravity checks in its table) run the built programs end to end. No
test in the tree compares the streaming decision against a parsed
reference decision — that is a stated gap, not an omission. Executed
differential, outside the tree: two independent complete runs against
the pre-change form, of 442,884 comparisons and of 40,149,018
comparisons (282,492 verdict cases and 39,866,526 envelope cases in the
second run), with zero divergences in both. The harnesses were executed
during development; the harnesses themselves are not preserved as
in-tree tests; durable in-tree differential tests are the test suite's
deliverable.

**The PowerShell span parse** — `hook::powershell_payload_facts` walks
the payload with the streaming scanner and records two facts: the byte
span of the last `tool_input` value, and whether the last
`permission_mode` value is `bypassPermissions`. Only that span is then
parsed into a value tree (`parse_json` on `input.get(span)`), because
the rewrite refills its command slot. The function's comment states the
equivalence it is built to: every other value is skipped by shape, a
repeated watched key replaces the earlier one as the last value won in
the whole-payload tree, and the rejections are the ones the
whole-payload parse gave. Evidence in the tree is end-to-end behaviour,
not a unit differential: the automatic-mode checks of `tests/auto.rs`
(U09, that the PowerShell hook rewrites and the rewrite runs; U18, the
same under both program names; U21, that the gate rewrites to the
deadline wrapper beside it) exercise the built programs, and no test in
`src/hook.rs` compares the span walk against a whole-payload parse of
the same input — that is a stated gap, not an omission. Executed
differential, outside the tree: 203,424 comparisons against the
pre-change form, with zero divergences, and a negative control (a
deliberately broken probe name) producing 1,503 divergences, so the
harness was not vacuous. The harness was executed during development;
the harness itself is not preserved as an in-tree test; durable in-tree
differential tests are the test suite's deliverable.

**The streaming PowerShell capture** — `powershell_auto` streams with a
capturing reader. Executed differential: 46,933 comparisons, with zero
divergences, and a negative control producing 6 divergences in 289
comparisons, so the harness was not vacuous. The harness was executed
during development; the harness itself is not preserved as an in-tree
test; durable in-tree differential tests are the test suite's
deliverable.

**The `Scanner::arm_capture` seeding** — `Scanner::arm_capture` seeds
the PowerShell capture from the window, deleting the whole-payload side
copy. Executed differential: 96,807 comparisons, with zero divergences,
and a negative control producing 1,004 divergences in 2,571 comparisons
on the deliberately broken variant. The harness was executed during
development; the harness itself is not preserved as an in-tree test;
durable in-tree differential tests are the test suite's deliverable.

**The envelope segment walks** — the rewriting vendors' envelope paths
are matched as segment walks (`PathSet` carrying the borrowed envelope
paths, with a segment-prefix `envelope_for`). Executed differential:
1,665,284 comparisons, with zero divergences, and two negative controls
producing 864 and 28,936 divergences. The harness was executed during
development; the harness itself is not preserved as an in-tree test;
durable in-tree differential tests are the test suite's deliverable.

**The base64 validator** — `command_text_is_valid` in `base64.rs`, an
independent byte-at-a-time UTF-8 state machine that materialises no
decoded text, in place of the decode check for the already-enveloped
checks. Equivalence assertions were added inside the existing base64
test. Executed differential: 81,411 inputs, with zero divergences
against both the live decode check and a frozen pre-edit reference, and
four negative controls (dropping the NUL, whitespace, UTF-8 or shape
checks) all diverging. The harness was executed during development; the
harness itself is not preserved as an in-tree test; durable in-tree
differential tests are the test suite's deliverable.

**The wrapper segment form** — wrapper comparisons in borrowed segment
form (`WrapperSpec` in segment form, with `first_wrapper` the only built
string). Executed differential: 1,159,174 comparisons, with zero
divergences, and a negative control producing 42 divergences. The
harness was executed during development; the harness itself is not
preserved as an in-tree test; durable in-tree differential tests are the
test suite's deliverable.

**The fused command acceptance** — the nonempty-and-no-NUL test of a
command text (`nonempty_without_nul` in `base64.rs`) settles both halves
in one character pass — a non-whitespace character seen (the trim strips
exactly the `char::is_whitespace` characters) and no NUL — where
`decode_command`'s fused form was the only site and `auto_slice` and the
two Antigravity command tests still spelled it as two walks
(`trim().is_empty()` beside `contains('\0')`). All four sites now call
the one helper. The separator test of `engine::which` is fused the same
way: one byte pass for `/` or `\`, which are ASCII and never occur
inside a multi-byte character. Executed differential: 6,448,264
comparisons (every Unicode scalar alone, flanked by a solid character
and by whitespace, and doubled; 2,000,000 generated strings over an
alphabet holding every whitespace scalar, NUL, both separators and
multi-byte solids; and long whitespace, NUL and separator runs) against
both pre-change two-scan forms, with zero divergences; and a negative
control — the fused acceptance testing only the ASCII whitespace
characters in place of `char::is_whitespace` — diverged 150,248 times
over the same corpus, so the harness was not vacuous. The harness was
executed during development; the harness itself is not preserved as an
in-tree test; durable in-tree differential tests are the test suite's
deliverable.

**The reap's test order** — `State::release` tests a hold's membership
in the job's process list (the in-place binary search over the list the
reap's one query already fetched) before its aliveness poll, where the
poll came first: a hold the list no longer names is dropped on the
search alone and pays no `WaitForSingleObject`. The retained set is
unchanged — the tests are a conjunction of queries the tree already
short-circuits (the poll is skipped for a hold whose time has run out,
and in `started_before`) — so their order decides only which calls are
made. The list-unreadable fallback keeps its order (poll, then the
per-hold job query). Executed differential: 32,288 comparisons
(exhaustive three-hold states over expiry, aliveness, listing and job
membership, in both list modes and with no list, plus 20,000 generated
tables of up to 40 holds) against the pre-change order, with identical
retained sets in all of them, 209,770 polls before and 152,928 after;
and a negative control — the membership test alone, with the poll
dropped — diverged 18,614 times, so the harness was not vacuous. The
harness was executed during development; the harness itself is not
preserved as an in-tree test; durable in-tree differential tests are the
test suite's deliverable.

**The capped padding count** — `base64_decode` and
`command_text_is_valid` count a value's trailing `=` padding only up to
3, where the count ran over the whole run: the only facts read from it
are 0, 1, 2 and more-than-2 (the refusal, and the body slice and tail
handling on a count of at most 2), so a capped count decides every input
exactly as the full count did, and a trailing run of any length is
refused after three steps instead of after a walk of the whole run.
Executed differential: 187,277 comparisons (exhaustive strings up to
length 8 over an alphabet holding `=`, an alphabet byte and an invalid
byte; valid encodings of generated byte strings and their mutations;
generated soup over the full alphabet; and trailing `=` runs of up to
20,000 under nine heads) against both pre-change forms, with zero
divergences; and a negative control — the count capped at 1 — diverged
10,107 times, so the harness was not vacuous. The harness was executed
during development; the harness itself is not preserved as an in-tree
test; durable in-tree differential tests are the test suite's
deliverable.

**The envelope check settling from its sub-checkers** — the dead-sink
bulk path above settled the stages the envelope check walks itself, but
its two sub-checkers could be settled false with the check itself still
live: a timeout token carrying a character that is neither a digit nor
its one point (the sticky `bad` its `finish` refuses on), and a payload
carrying a non-alphabet byte after padding, a third `=`, a decoded byte
the UTF-8 decoder refused, or a decoded NUL (the sticky states
`PayloadCheck::finish` refuses on, and a padding count that only grows).
The check now sets its own dead flag from `settled_false` at every feed
site — the timeout and payload arms and the payload run form — so the
rest of such a string is validated whole like a settled stage's, instead
of being fed through the sub-checker to the string's end (a payload may
run to the input cap). The sub-checkers' later state is read by no one
once the check is dead: `finish` is dead-gated, and the run and bulk
predicates read the flag. Executed differential: the extracted scanner
and envelope sections, old form against new form driven through the
scanner's own string dispatch, 200,090 comparisons (100,045 cases under
both roles: directed envelopes — settling bytes at every stage,
window-boundary placements of the settling byte, scanner hazards after
the settling point, payloads past 100,000 characters — and generated
envelope-shaped and raw-byte fuzz), with zero divergences in scan
outcome and verdict; and a negative control — settling at the first `=`
— diverged 8 times, so the harness was not vacuous. The harness was
executed during development; the harness itself is not preserved as an
in-tree test.

**The command validator returning at the settling quartet** —
`command_text_is_valid` fed every decoded byte of a value to its check
to the end of the body, where the sibling acceptance in the same file
(`nonempty_without_nul`) returns at the NUL that settles it and the
alphabet test in the same loop already returned at once. A decoded byte
the UTF-8 decoder refused, or a NUL byte, settles the answer false for
the whole value: both flags are set once and never cleared, the result
is their negation among its conjuncts, and the check has no other
effect. The walk now returns false at the quartet that sets either.
Executed differential: 422,052 comparisons (exhaustive short forms over
an alphabet holding alphabet bytes, `=`, an invalid byte and a
non-alphabet byte; encodings of byte strings with the settling bytes at
every offset up to 200 and long tails behind them; encodings of random
bytes; a 1,000,000-byte value settled at its first quartet), with zero
divergences; and a negative control — also settling on the positive
`solid` flag — diverged 195 times, so the harness was not vacuous. The
harness was executed during development; the harness itself is not
preserved as an in-tree test.

**The timeout check settling from its counter states** —
`TimeoutCheck::settled_false` read only its sticky `bad` flag, where the
check's other states settle the answer false by the arithmetic of its
own `finish`: a whole part past the cap by a whole unit (six significant
digits, or a five-digit value above 86400) under every continuation, the
86400 boundary with its fraction already past the rounding bound
(`fraction_mid` 1, a final state), and a zero whole part whose fraction
is already below the smallest subnormal's half (`fraction_sub` -1,
likewise final). The check now reports those states settled too, so the
rest of such a token takes the bulk path like a bad token's, instead of
being fed digit by digit to the string's end (a timeout token may run to
the input cap). Executed differential: 106,014 settled states reached by
directed and generated tokens were each continued exhaustively (to depth
4 over digits, point, a bad character and the deciding space) and at
random (to length 40), 103,999,734 continuations in all, against the
pre-change form, with zero continuations in which the pre-change form
could still succeed; and a negative control — settling also at a whole
value of exactly 86400 — was caught succeeding, so the harness was not
vacuous. The harness was executed during development; the harness itself
is not preserved as an in-tree test.

**The envelope tail's one-pass token and capped padding** —
`envelope_remainder`, the whole-string envelope tail the segment forms
share, settled its timeout token with digit walks over both token parts
followed by a full `f64` parse of the token, and counted the payload's
trailing `=` padding with `trim_end_matches`, a walk of the whole
trailing run the capped counts of `base64_decode` and
`command_text_is_valid` had already refused to make. The tail now feeds
the token once through `TimeoutCheck` — the streaming checker's counter
form, whose `finish` states the whole-string token rules and parses only
a short token's kept prefix — and counts the padding capped at 3, as the
base64 forms do. Executed differential: 320,421 token comparisons
(exhaustive short tokens over digit, point, exponent, sign,
bad-character and multi-byte alphabets; directed 86400-boundary and
subnormal-boundary constructions; 300,000 generated long tokens) between
the pre-change digit-and-parse logic and `TimeoutCheck::finish`, with
zero divergences, and a negative control (a form that also accepts a
trailing point) diverging; and 79,524 padding comparisons (exhaustive
strings to length 9 over an alphabet holding `=`, trailing `=` runs to
20,000) between the `trim_end_matches` count and the capped count, with
zero divergences, and a negative control (a count capped at 1) diverging
1,094 times. The harnesses were executed during development; the
harnesses themselves are not preserved as in-tree tests.

**The environment key test in one pass** — `already_upper_ascii`, the
test that decides whether an environment entry's name needs an owned
upper-case sort key, ran `is_ascii` over the name and then a second scan
for a lower-case byte. The fused form refuses on the first byte that is
at or past 0x80 or is ASCII lower-case, one pass deciding both halves by
De Morgan's law on the per-byte tests. Executed differential: 1,412,064
comparisons (every Unicode scalar standing alone, and generated names
over ASCII, multi-byte and mixed alphabets) against the pre-change
two-scan form, with zero divergences; and a negative control — the fused
form without its 0x80 half — diverged 1,111,936 times, so the harness
was not vacuous. The harness was executed during development; the
harness itself is not preserved as an in-tree test.

**The timeout run form** — inside `Scanner::string`, a sink at the
timeout stage of a live envelope check (`EnvelopeCheck` with `stage ==
Timeout` and not dead) takes a plain run of the current window for run
processing when the run validates as UTF-8 as it stands, on the same
terms as the payload run form above; a run that validation refuses falls
back to the per-byte path without consuming anything, so an invalid byte
is rejected there exactly as before. `EnvelopeCheck`'s run form feeds
only the run's ASCII digit-and-point prefix through the same
`TimeoutCheck::push_char` the per-character path feeds: digits and the
point are ASCII, so each byte of the prefix is the character it names
and a multi-byte character's lead byte (at or past 0x80) ends the prefix
on that character's boundary, leaving the character itself to the
per-character path. After each feed the run form tests `settled_false`,
and on the feed that settles the check it sets the check's dead flag and
stops, exactly as the per-character feed does, so the bulk path takes
the rest of the string; the terminating space and every other character
are never consumed by the run. Only the delivery overhead changes: every
consumed character is delivered through the identical `push_char`, in
order. Executed differential: the extracted scanner and envelope
sections, old form against new form driven through the scanner's own
string dispatch, 1,062,038 comparisons (generated and mutated verdict
payloads under both roles and seven chunk sizes, timeout tokens crossing
the 16 KiB window edges, tokens settled mid-run at each settled state —
six significant digits, a whole part past the cap, the 86400 fraction
bound and the subnormal fraction bound — spaces and bad characters at
run boundaries, multi-byte characters at prefix ends, the Unicode
whitespace set inside and around the token, and invalid-UTF-8 runs on
the fallback path), with zero divergences in scan outcome, verdict and
final offset; 44,038 of the comparisons were also held against expected
verdicts computed from the envelope's component rules, with zero
mismatches in both forms; and a negative control — a run form that also
consumes the terminating space — diverged 22,875 times, so the harness
was not vacuous. The harness was executed during development; the
harness itself is not preserved as an in-tree test; durable in-tree
differential tests are the test suite's deliverable.

**The scanner's structural whitespace skip** — `Scanner::ws` skipped the
grammar's four whitespace bytes (space, tab, line feed, carriage return)
one `peek` at a time, between every pair of tokens of the payload,
however long the run between them (a payload may pad to the 16 MiB cap).
The skip now consumes each window's maximal prefix of those bytes whole
and refills only when a whole window was whitespace. The set is exactly
the per-byte form's — the JSON grammar's whitespace, all ASCII, so a
byte test is exact — and `available` peeks (and fills) on the same
boundaries the per-byte loop peeked on, so the consumed prefix, the
fills, the budget spent and the end-of-input and failure states left
behind are the per-byte form's. Executed differential: the extracted
scanner sections, old form against new form, 120,000 comparisons
(whitespace runs to 40,000 bytes crossing the 16 KiB window edges under
eight read chunk sizes, mixed token bytes, empty inputs, budgets of 0,
1, 10 and the window edges, and failing readers), comparing the offset,
the end-of-input and failure flags, the budget and fetched counts and
the next byte after the skip, with zero divergences; and a negative
control — a skip form that also consumes the vertical tab — diverged
23,682 times in 24,000 comparisons, so the harness was not vacuous. The
harness was executed during development; the harness itself is not
preserved as an in-tree test; durable in-tree differential tests are the
test suite's deliverable.

**The envelope whitespace run form** — inside `Scanner::string`, a sink
at one of the envelope check's two whitespace stages of a live check
takes a plain run of the current window for run processing when the run
validates as UTF-8 as it stands, on the same terms as the payload and
timeout run forms above; a run that validation refuses falls back to the
per-byte path without consuming anything, so an invalid byte is rejected
there exactly as before. The two stages are the leading stage
(`EnvelopeCheck` with `stage == Lead` and not dead), where a whitespace
character returns from `push_char` at once, and the payload stage once
its trailing whitespace has begun (`stage == Payload` with `payload_ws`
standing and not dead), where a whitespace character leaves `payload_ws`
standing and moves nothing else. No consumed character changes state in
either stage, so `EnvelopeCheck`'s run form records nothing per
character: it reports only the maximal decoded-whitespace prefix of the
run, each character of the prefix tested with `char::is_whitespace` at
its own position (the full Unicode set, never a space-byte test), and
the first non-whitespace character and every byte after it are left
unconsumed for the per-character path, which settles them exactly as
before (the leading stage dispatches on it; the payload stage kills the
check on it). Executed differential: the extracted scanner and envelope
sections, old form against new form driven through the scanner's own
string dispatch, 356,164 comparisons (generated and mutated verdict
payloads under both roles and seven chunk sizes, every Unicode
whitespace scalar opening a command string and closing a payload, raw
and escaped, long whitespace runs crossing the 16 KiB window edges,
whitespace-then-content tails, escapes at run boundaries, and
invalid-UTF-8 runs on the fallback path), with zero divergences in scan
outcome, verdict and final offset; and a negative control — a run form
splitting the run at the space byte alone and consuming everything
before it as whitespace — diverged 13,562 times over the same corpus, so
the harness was not vacuous. The harness was executed during
development; the harness itself is not preserved as an in-tree test;
durable in-tree differential tests are the test suite's deliverable.

**The number digit run form** — `Scanner::number`, the skip path for
every unwatched number value, walked its ten-state shape machine one
`peek` per byte. In three of its shapes a digit is a self-loop —
`Whole`, `Fraction` and `ExponentDigits` step to themselves on a digit —
and each of the three is reached only by consuming a digit (from `Start`
or `Sign`, from `Dot` or `WholeDot`, and from `Exponent` or
`ExponentSign` respectively), so `any` already stands in all three.
Inside `number`, when the current shape is one of the three, the maximal
ASCII digit prefix of the current window is now consumed whole via
`available()`, refilling only when a whole window was digits, and `any`
is set as the per-byte form sets it. Digits here are ASCII bytes that
genuinely are the characters. Every other byte keeps the per-byte path
and its transitions exactly, including the first byte that is not a
digit, which the per-byte path then classifies as before. Only the
delivery overhead changes. Executed differential: the extracted scanner
sections, old form against new form driven through `Scanner::number`
itself, 171,486 comparisons (generated number tokens, valid and
malformed shapes, digit runs to 40,000 bytes crossing the 16 KiB window
edges under several chunk sizes and budgets, digits at every shape
boundary, a point or an exponent letter at a run's end, and failing
readers), comparing acceptance and the offset left behind (and the
end-of-input and failure flags, the budget and the fetched count), with
zero divergences; and a negative control — a run form that also consumes
digits in the `Exponent` shape, skipping its sign/digit transition
accounting — diverged 11,226 times over the same corpus, so the harness
was not vacuous. The harness was executed during development; the
harness itself is not preserved as an in-tree test; durable in-tree
differential tests are the test suite's deliverable.

**The base64 quartet forms** — `base64_decode` and
`command_text_is_valid` in `base64.rs` walked their bodies one byte at a
time, shifting each value into an accumulator and testing the position
modulo four on every byte to find each quartet's end, where the
streaming payload checker (`PayloadCheck::push_payload_run` in
`hook/envelope.rs`) already decoded a quartet as one group. Both walks now take
whole quartets through `as_chunks::<4>()`: the four values shift into one
group in order and its three bytes are emitted at once; the partial
group the quartets leave (three bytes when the padding is one, two when
it is two, none when there is none — the only shapes a length in whole
quartets allows) is accumulated and shifted exactly as the per-byte form
accumulated and shifted it, and the validator's settled-answer test runs
after each whole quartet's emitted bytes, at the same points as the
per-byte form's, and never inside a quartet. A byte outside the alphabet
fails the whole value in either form, and neither walk keeps any other
state a quartet's internal order could decide. The encoders take the
same step for a full group: `append_group` and `base64_encode` emit its
four alphabet characters without testing the position against the group
length per character, a test that stands only on a partial group's tail.
Executed differential: the pre-change forms against the changed forms,
1,319,605 comparisons (exhaustive strings to length 7 over an alphabet
holding alphabet bytes, `=` and an invalid byte; valid encodings of
generated byte strings and their mutations; encodings of whitespace, NUL
and invalid-UTF-8 payloads; trailing `=` runs to 20,000 under eight
heads; one 100,003-byte value and its mutation), with zero divergences
in both the decode and the validator, and 40,000 encoder comparisons
(generated byte strings and generated UTF-16LE texts), with zero
divergences; and a negative control — a quartet form assembling its four
values in reverse order — diverged 116,854 times in 1,080,800
comparisons, so the harness was not vacuous. The harness was executed
during development; the harness itself is not preserved as an in-tree
test; durable in-tree differential tests are the test suite's
deliverable.

**The string command-line renderer's counted backslash run** —
`command_line` in `engine/resolve.rs` held each argument's pending
backslash run in an auxiliary string built per argument and built a
`repeat` temporary for the doubled run at every embedded quote, where
the wide renderer (`append_wide_arg`, same file) counts the run and
writes it straight into the result. The string renderer now counts it
the same way: each held character was one backslash, so the held length
was the count, and every write the counted form makes is the write the
string form made — 2n+1 backslashes before an embedded quote, the run
itself before any other character, and the run doubled ahead of the
closing quote when the argument is quoted. Its one production caller
renders the command text an exception prompt shows (`list2cmdline` in
`cli::plan`); no counted route changes. Executed differential: the
pre-change form against the changed form, 243,867 comparisons
(exhaustive single arguments to length 6 and exhaustive pairs to length
3 per side over the deciding alphabet — a plain byte, backslash, quote,
space, tab — and 200,000 generated argument lists of up to five
arguments built from backslash runs, quotes, spaces, tabs, plain runs
and multi-byte characters), with zero divergences; and a negative
control — a counted form writing 2n backslashes before an embedded quote
instead of 2n+1 — diverged 156,610 times in the same 243,867
comparisons, so the harness was not vacuous. The harness was executed
during development; the harness itself is not preserved as an in-tree
test; durable in-tree differential tests are the test suite's
deliverable.

**The whole-string envelope tail settling at its token** —
`envelope_remainder`, the whole-string envelope tail the segment forms
share, fed its timeout token through `TimeoutCheck` to the token's end,
where every streaming feed of the same checker stops at the character
that settles it: the tail now returns false at the feed on which
`settled_false` first stands. The states that predicate names are final
under every continuation — that is the predicate's own statement, the
streaming checker's dead gate already rests on it, and its counter
states were held to it in the timeout-settling entry above — so a token
settled partway (a whole part past the cap, a bad character, the 86400
fraction bound, the subnormal fraction bound) refuses at that character
however the token continues, and the rest of a token that may run to the
input cap is not fed. The shell and payload steps after the token are
reached exactly as before for every token that never settles. Executed
differential: the extracted `TimeoutCheck` and `envelope_remainder`
sections, old form against new form, 489,512 comparisons (exhaustive
tokens to length 6 over digit, point, exponent, sign, bad-character and
multi-byte alphabets; directed 86400-boundary fractions at every place
of the rounding bound with each last digit stepped either way;
subnormal-boundary fractions at places 300 to 345; digit runs to
100,000; and generated tokens to 5,000 characters, each under seven
shell and payload tails), with zero divergences; and a negative control
— a form that also settles at a whole value of exactly 86400 — diverged
25 times, so the harness was not vacuous. The harness was executed
during development; the harness itself is not preserved as an in-tree
test; durable in-tree differential tests are the test suite's
deliverable.

**The number walk's dead-shape run** — `Scanner::number`, the skip path
for every unwatched number value, consumed a token that had already
failed its shape machine one `peek` at a time to the token's end: in the
dead shape every token byte is a self-loop (the shape is absorbing, and
the function's answer is already settled `None`), so the per-byte walk
decided nothing per byte. The maximal token-byte prefix of the current
window is now consumed whole in that shape, via `available()`, on the
same terms as the digit runs of the digit-stable shapes: refilling only
when a whole window was token bytes, and leaving the first byte that is
not a token byte unconsumed. The consumed prefix, the fills, the budget
spent and the end-of-input and failure states left behind are the
per-byte form's, and acceptance is unchanged — a dead token is refused
at the same end, having been consumed to the same offset. Executed
differential: the extracted scanner sections, old form against new form
driven through `Scanner::number` itself, 132,751 comparisons (exhaustive
tokens to length 5 over a shape-deciding alphabet; directed digit and
dead runs to 100,000 bytes crossing the 16 KiB window edges under three
read chunk sizes and failing readers; and generated token soup to 20,000
bytes), comparing acceptance, the offset left behind, the end-of-input
and failure flags, the budget and the fetched count, with zero
divergences; and a negative control — a dead run that also consumes the
byte `x` — diverged 503 times, so the harness was not vacuous. The
harness was executed during development; the harness itself is not
preserved as an in-tree test; durable in-tree differential tests are the
test suite's deliverable.

**The payload run form stopping at its settling step** —
`PayloadCheck::push_payload_run`, the quartet run form of the streaming
payload checker, fed every quartet and character of a window's run to
the end of the run, where the per-character feed of the same checker
stops at the character that settles it, the timeout run form stops at
its settling feed, and the base64 validator returns at its settling
quartet: a step that sets one of the `settled_false` states (a
non-alphabet byte, a byte after padding, a third `=`, a decoded byte the
UTF-8 decoder refused, or a decoded NUL) decided the rest of the run for
no reader — the envelope check sets its dead flag from the same
predicate when the run returns, its `finish` is dead-gated, and the
scanner's bulk path takes the rest of the string from there. The run
form now tests `settled_false` at the head of its loop and ends at the
step that settled it; the unconsumed rest of the run goes through the
bulk path exactly as the rest of the string after a per-character
settling feed does. Executed differential: the extracted scanner and
envelope sections, old form against new form driven through the
scanner's own string dispatch, 422,337 comparisons (directed envelopes —
settling quartets and characters at offsets from the first quartet to
offset 40,000, window-edge placements under seven read chunk sizes,
escapes inside the payload, whitespace tails and multi-byte intrusions —
and generated payload-shaped soup and long generated payloads crossing
the 16 KiB window edges), comparing the scan outcome, the verdict, the
final offset and the fetched and failure states, with zero divergences;
and a negative control — a run form that also settles at the positive
`solid` flag — diverged 163 times, so the harness was not vacuous. The
harness was executed during development; the harness itself is not
preserved as an in-tree test; durable in-tree differential tests are the
test suite's deliverable.

**The whole-string parser's whitespace and number-token runs** —
`Parser::ws` and the number token of `Parser::value` in `hook/json.rs`, the
whole-string parser the PowerShell span parse and the reference verdicts
run on a captured value (which may pad to the payload cap), walked their
runs one byte at a time: the whitespace skip advanced `pos` per byte
between every pair of tokens, and the number token advanced it per token
byte. The streaming scanner's forms of both walks were settled long
before (the structural whitespace skip, and the number digit and
dead-shape runs), but the whole-string parser kept the per-byte forms.
Both runs are now consumed whole with a slice scan for the first byte
outside the run's set: the whitespace set is the grammar's four ASCII
bytes, and the token set is the parser's own byte set, so the position
each walk leaves — and therefore every slice and parse that follows — is
the per-byte form's. Executed differential: the extracted parser
sections, old form against new form, 120,019 comparisons (directed
tokens and whitespace placements, generated objects, arrays, strings and
number soup, whitespace runs to 40,000 bytes, digit runs to 100,000
bytes, mutations of a seed payload, and `\x0b`/`\x0c` bytes between
tokens), comparing the parsed value in full, with zero divergences; and
a negative control — the new whitespace form with the vertical tab added
to its set — diverged 5,980 times over the same corpus, so the harness
was not vacuous. The harness was executed during development; the
harness itself is not preserved as an in-tree test; durable in-tree
differential tests are the test suite's deliverable.

**The command acceptance returning at its first solid character** —
`nonempty_without_nul` in `base64.rs` walked the whole text character by
character after its answer's emptiness half had already settled: once a
non-whitespace character stands, no later character can unset it, and
the only fact the rest of the text decides is whether a NUL stands in it
(a NUL before that point already returned, as it still does). The walk
now returns at that character with that one remaining test made as a
byte scan of the rest: a NUL is the single byte 0x00, and no multi-byte
character contains a 0x00 byte (its bytes are all at least 0x80), so the
byte scan decides exactly what the per-character walk's remaining NUL
tests decided, where the walk re-decoded every character of a command
that may run to the payload cap and re-tested its whitespace for a
settled answer. Executed differential: 8,672,414 comparisons (every
Unicode scalar standing alone, doubled, flanked by a solid character and
by whitespace, and followed by a NUL; 2,000,000 generated strings over
an alphabet holding every whitespace scalar, NUL, solids and multi-byte
characters; and whitespace, solid and NUL-tailed runs to 1,000,000
characters) against the pre-change form, with zero divergences; and a
negative control — a form that returns true at the first solid character
without the remaining NUL scan — diverged 2,569,598 times, so the
harness was not vacuous. The harness was executed during development;
the harness itself is not preserved as an in-tree test; durable in-tree
differential tests are the test suite's deliverable.

**The scanner's escape delivery** — inside `Scanner::string`, a `\u`
escape's decoded character was encoded back to UTF-8 and fed byte by
byte through the string's `Utf8Decoder`, which re-derived the same
character for the sink: up to four decoder steps and an encode per
escape, on a string that may consist of escapes to the input cap. The
character is now delivered itself when the decoder stands at a scalar
boundary — a character's own encoding fed there produces exactly that
character and leaves the decoder at a boundary — and the string is
rejected at once when the decoder stands mid-scalar, which is the
encode-feed's own outcome there: the encoding's first byte is an ASCII
or lead byte, never the continuation byte a mid-scalar decoder waits
for, so its first feed already failed. The whole-string parser's string
form never re-derived its escapes (it encodes the character straight
into its output); the streaming form now matches it. Executed
differential: the extracted scanner sections, old form against new form
driven through `Scanner::string` with a collecting sink, 960,320
comparisons (directed escapes — surrogate pairs and halves, bad and
`+`-signed hex, NUL and control escapes — escapes at and straddling the
16 KiB window edges, escape runs of 9,000 escapes crossing several
windows, an incomplete scalar standing immediately before an escape,
invalid raw bytes, unterminated strings, failing readers, and generated
escape-and-raw soup under eight read chunk sizes), comparing the scan
outcome, the collected text, the final offset and the fetched and
failure states, with zero divergences; and a negative control —
delivering the character with no completeness test — diverged 89,712
times, so the harness was not vacuous. The harness was executed during
development; the harness itself is not preserved as an in-tree test;
durable in-tree differential tests are the test suite's deliverable.

**The timeout check's bulk step** — both remaining feeds of
`TimeoutCheck` in `hook/envelope.rs` delivered digit by digit even in states
where a digit changes nothing but two counters: `envelope_remainder`'s
token loop and `EnvelopeCheck::push_timeout_run`. `TimeoutCheck` now
carries a bulk step (`push_run`) both feeds offer the rest of their run
to first, taken only on a live, unsettled check past the kept prefix
(`bytes` above 32, where a per-character step writes no kept byte and
`finish` reads `bytes` only against that bound), and only in three
states: in the fraction phase with both rounding comparisons already
decided (`fraction_mid` and `fraction_sub` both nonzero — a digit step
there consults neither comparison), the maximal ASCII digit run is
consumed whole; in the whole phase with no significant digit yet
(`whole_significant` zero), the maximal zero run is consumed whole; and
in the fraction phase at or past place 341 — past the midpoint bound's
37 places and the subnormal bound's last place, 340, so both bounds a
zero step consults are zero and a zero digit equals them — the maximal
zero run is consumed whole. Each step adds the run's length to `bytes`
and to the phase's one digit count and nothing else, and no state above
can settle or be settled mid-run, so both feeds' existing stop
conditions stand untouched. The runs are scanned as bytes: the digits
the per-character feed steps on are the characters `'0'..='9'`, which
are ASCII, so a digit byte is the character it names and a multi-byte
character's bytes, all at or past 0x80, end a run on that character's
own boundary. Executed differential: the extracted `TimeoutCheck` and
both feed sites, old form against new form, 232,409 comparisons
(exhaustive short tokens over digit, point, bad-character and multi-byte
alphabets, and over a digit/point/space alphabet; directed tokens
matching the `TIMEOUT_MID` and `ZERO_MID` prefixes with the last digit
stepped either way at every place and zero tails of 100,000 digits;
tokens entering and leaving each bulk state mid-run; generated tokens;
and the streaming form under chunk sizes 2, 3, 7, 4096, 16383 and 16384
with runs crossing the 16 KiB window edge), comparing the check's full
counter state, its settled state and its `finish` verdict at every step,
with zero divergences; and two negative controls — the digit-run state
taken without its mid/sub precondition, which diverged 348 times, and
the past-the-bounds zero state entered at place 300 instead of 341,
which diverged 33 times — so the harness was not vacuous. The harness
was executed during development; the harness itself is not preserved as
an in-tree test; durable in-tree differential tests are the test suite's
deliverable.

[↑ Back to Contents](#contents)

## 4. Evidence tiers, and what is not measured

**Windows-proven** (the release test run for 0.5.0, Wednesday, 04NOV2026, on Windows 11, x86-64, with the Microsoft Visual C++ (MSVC) toolchain; the log `windows-cargo-test.log` is attached to the v0.5.0 release):

- `cargo test --release --no-fail-fast`: 437 tests, 433 passed, 4 failed, 0 ignored. The four open tests (G62, G72, G80, G83) suspend the guard through a new handle to it, which the guard denies by design; they need redesign, not a product change.
- The release build (`tests/build.ps1`) exits 0 and stages the four programs and the test helper.

**Linux-proven** (produced and checkable on the development machine):

- This census. It is counted from the source and can be recounted from
  the functions it names.
- The cross-target type and borrow check: `cargo check --target
  x86_64-pc-windows-gnu --all-targets` passes on this tree (with no
  warnings). That check also compiles the test code of
  section 3; it does not execute it.
- The same check at the same tier for the two architectures the
  releases are not built for: `cargo check --target
  aarch64-pc-windows-msvc --all-targets` passes, and `cargo check
  --target i686-pc-windows-msvc --all-targets` passes. These are
  cross-target type-and-borrow evidence that the declarations are
  width-clean — not execution, and not a speed fact: nothing ran on
  either architecture, and neither is a release target. Both
  checks are kept permanent by the tree's own continuous integration `check` job
  (`.github/workflows/ci.yml`), outside this record. The Windows test suite executes
  where Windows runs, not here: tests cannot execute on this machine,
  and no result in this document comes from running one.

**Theory** (argued, not measured):

- The price of any single operation on Windows: what a native call, a
  heap allocation, a thread creation or a process spawn costs there, and
  therefore what any tally in section 2 costs in time.
- MSVC startup behaviour. The supported target is MSVC (the version
  resource `build.rs` writes is linked only there; the GNU target is
  unsupported, as `README.md` states). How the MSVC-built programs start
  — runtime prologue, static data, import surface — is reasoned about
  from the target's known shape and is not measured here, because there
  is no MSVC and no Windows on this machine.

**Owed: the benchmark campaign on Windows.** For 0.5.0 the campaign run-20261104T230944Z-s20261029 recorded 922 of 1,940 slots and was stopped; its status is incomplete and it is not a speed result. The frozen capture
(design 003): 1,620 slots — 36 controls, 144 warmups and 1,440
measured — in six arms, with the `NO_WINDOW` and `DETACHED` console
contexts kept separate, the endpoint being the whole outer lifetime of
the program under test plus its included controller work and cleanup.
The guard is compared against the three external competitors as found —
ProcessKit CLI v0.3.4, nuewframe/timeout v1.0.0 and sysprims v0.2.4 —
which receive no repairs and no tuning, while the guard is repaired
generally until it wins or ties. One capture attempt, no retries,
exclusions or replacements, and an incomplete capture produces no speed
summary. That campaign, and only that campaign, is a speed verdict.

**The self-measurement record is not a measurement of anything here.**
`src/timing.rs` emits, only when both the trace name — read
new-first as `PROCESS_TREE_GUARD_TRACE`, falling back to
`PROCESS_GUARD_TRACE` — and the timing nonce — read new-first as
`PROCESS_TREE_GUARD_TIMING_NONCE`, falling back to
`PROCESS_GUARD_TIMING_NONCE` (exactly 32 lowercase hexadecimal
characters) — are set, one `PROCESS_GUARD_NATIVE_TIMING_V1` record per
fixed run, after cleanup. The nonce is read only when the trace is
on, once per fixed run, and pays one extra failed `var_os` probe per
traced fixed run when the new nonce name is unset. Its `assurance` field reads
`SOURCE_PINNED_SELF_MEASUREMENT`. That label names the record's
provenance — where its numbers come from: the running program's own
clock reads, taken at the points its source pins. The label describes
the record; it is not itself a result, and no speed verdict rests on
it. A run without both variables emits no record and makes none of the
record's clock, frequency or identity queries.

[↑ Back to Contents](#contents)

## 5. The claims discipline

No new 1,620-slot performance result exists for this tree: the
capture has not started, and there is no new speed verdict, no current
speed win and no completed external campaign. Nothing in this
document should be read as claiming one. The census of section 2 answers "what does each path cost,
counted"; the campaign of section 4 answers "who is faster, measured".
The first answer is complete on its own terms and is not the second
answer in cheaper dress: a path can be counted to the last call and its
speed still be unmeasured, which is the state of every path in this tree
today. The wall-clock campaign — the 1,620-slot capture against the
three competitors as found — remains owed and is the test suite's
deliverable, and the census does not settle "is it slower".

[↑ Back to Contents](#contents)

## 6. Verification of this document

- Every census number above was derived by reading the function it is
  attributed to, in the tree as it stands.
- `cargo check --target x86_64-pc-windows-gnu --all-targets` passes
  on this tree (the document adds no code).
- The `#[test]` count of the tree is 437; this document adds no test
  and no code.
- The test suite was not executed for this document: its tests target
  Windows and cannot execute on this machine.

[↑ Back to Contents](#contents)
