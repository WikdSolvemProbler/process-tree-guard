# Measurement
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This page teaches the measurement method of process-tree-guard in summary. The full record stands in the repository file [MEASUREMENT.md](../MEASUREMENT.md). Speed in this set is a distance above a floor, not a race claim.

## Contents

- [The method](#the-method)
- [The floor](#the-floor)
- [Contexts](#contexts)
- [Results](#results)
- [Tiers of the results](#tiers-of-the-results)

## The method

Count the work of a path. A path cost is the native calls of that path and the allocations that scale with input size. A count settles what a path costs, in counted operations. A count does not settle a wall-clock race. State each measured result at its tier.

[↑ Back to Contents](#contents)

## The floor

The floor is the cost of a spawn-and-die guard with no added work. Measure the guard as a distance above that floor. A path that adds no call above the floor is at the floor. A path that adds a call states that call by name and by count.

[↑ Back to Contents](#contents)

## Contexts

Each principal path stands in its own context. The hook allow path streams the payload and counts its scan work. The run path counts its creation, wait, and teardown work. The extendable path counts its pipe and log work. The full record gives one table per path. The summary tables stand in the dropdown of the Results section.

[↑ Back to Contents](#contents)

## Results

The results stand in summary here. The final numbers stand here after the final measurement step: The counted totals of the three principal paths stand here. The hook verdict path costs 18 native calls for the allow decision. The same path costs 21 native calls for the deny decision. The plain run path costs 45 native calls in its fixed core. The extendable run path costs 43 native calls at extension start. Each total is a count of named calls from the source. No total here is a wall-clock result. The campaign on Windows that gives a speed verdict is still owed.

Open the dropdown for the full results tables.

<details>
<summary>The full results tables stand in this dropdown.</summary>

The hook verdict path table stands first.

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

The plain run path table stands second.

| Step (source) | Calls | Count |
|---|---|---|
| Entry hardening (`diagnostics::cli_exit` → `cli_process::enter` → `harden_self` → `deny_acl`, for the process object and the thread object; the same row §2.1 counts for the hook route) | `GetCurrentProcess` 1, `GetCurrentThread` 1, `GetSecurityInfo` 2, `LocalFree` 2, `SetSecurityInfo` 2 | 8 |
| Automatic-name check (`lib::entry_full` / `lib::entry_deadline` → `auto_by_name`, the stack form; the contain and gate entries make no such check) | `GetModuleFileNameW` 1 | 1 |
| Job setup (`engine::run_job_core`) | `CreateJobObjectW` 1, `SetInformationJobObject` 1 | 2 |
| Standard handles (the handle loop in `run_job_core`; on a process-owning invocation the three handles are read back from their per-handle slots; each slot's one probe is paid at its first read here, and no `GetStdHandle` is paid a second time for any of them. The current-process pseudo-handle only the returning-library branch duplicates with is fetched on that branch's first duplication and kept for its later ones, so this route pays no fetch for it) | `GetStdHandle` 3, `GetConsoleMode` 3 (the one probe of each handle's slot, paid at this first read), `SetHandleInformation` 3 (one per distinct handle) | 9 |
| Owner, the parent only (`engine::watched_owner_handles` and `engine::parent_process_id_on` in `src/engine/owners.rs`; the one pseudo-handle fetch serves both the creation-time read of this process and the parent query through it) | `GetCurrentProcess` 1, `GetProcessTimes` 2, `NtQueryInformationProcess` 1, `OpenProcess` 1 | 5 |
| Console probe (`cli_process::Console::probe`) | `CreateFileW` 1 (the `CONIN$` device opens; 2 when `CONIN$` fails, whether `CONOUT$` then opens, so the probe succeeds with input missing, or neither console device opens) | 1 |
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

The extendable run path table stands third.

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
| Side (source) | Calls | Count |
|---|---|---|
| Server: accept (`extend::accept`, the overlapped connect completes pending) | `ResetEvent` 1 (the connect's event, reset before the connect is posted), `ConnectNamedPipe` 1, `GetLastError` 1, `WaitForMultipleObjects` 1, `GetSystemTimePreciseAsFileTime` 1, `GetOverlappedResult` 1 | 6 |
| Server: who the client is (`extend::handle_client`, `extend::arrive`, `Process::open`) | `GetNamedPipeClientProcessId` 1, `OpenProcess` 1, `GetProcessTimes` 1, `QueryFullProcessImageNameW` 1, `IsProcessInJob` 1, `GetTickCount64` 1 (the rate admission's clock) | 6 |
| Server: the request (`Io::read` in `src/extend/serve.rs`, finished by `finish_io`) | `ResetEvent` 1, `ReadFile` 1, `GetLastError` 1 (the pending finish reads the error state before it waits), `WaitForMultipleObjects` 1, `GetOverlappedResult` 1 | 5 |
| Server: the decision (`extend::client_gone`, `extend::evaluate`) | `PeekNamedPipe` 1, `WaitForSingleObject` 1 (the aliveness poll of `started_before`), `GetTickCount64` 1 (the grant's clock), `SetEvent` 1 (the wake) | 4 |
| Server: the reply (`Io::write`, finished by `finish_io`) and the disconnect (`extend::serve`) | `ResetEvent` 1, `WriteFile` 1, `GetLastError` 1 (the pending finish, as in the request), `WaitForMultipleObjects` 1, `GetOverlappedResult` 1, `DisconnectNamedPipe` 1 | 6 |
| **Server total per grant** | | **27** |
| Client: connect and exchange (`extend::connect`, `extend::exchange` in `src/extend/client.rs`, finished by `finish_io`) | `CreateFileW` 1, `SetNamedPipeHandleState` 1, `WriteFile` 1, `ReadFile` 1, `ResetEvent` 2 (the exchange event is thread-local: created once per thread and reset before each phase, not created and closed per exchange), `GetLastError` 2 (the pending finish of each phase, before its wait), `WaitForMultipleObjects` 2 (one per overlapped finish), `GetOverlappedResult` 2, `CloseHandle` 1 (the pipe) | 13 |

</details>

[↑ Back to Contents](#contents)

## Tiers of the results

A counted result is a source count, not a platform run. A Linux result proves harness logic only. A Windows result at the final version waits for a local MSVC run. No page in this set states a Windows execution number as proven before that run.

[↑ Back to Contents](#contents)
