# Experimental ordinary Linux source snapshot

These are the exact 33 native C, header and Rust adapter inputs preserved from
implementation-007. One offline x86-64 Linux Manual deadline case passed with
this code. The payload and its detached descendant occupied the same original
cgroup leaf; the caller returned 124, the original process handles signaled,
and the authenticated cleanup receipt and independent observations passed.

The watchdog retains the original reaper wait until the caller authenticates
retirement and acknowledges it inside the existing cutoff. It then consumes
the same native wait record. The first stop and original cutoff remain intact;
missing or invalid acknowledgment cannot produce success.

This directory is an inert source archive, excluded from the Cargo package.
It does not enable a Linux backend, install a helper, change Windows execution,
or qualify a general returning-library context. The observed caller was
cooperative, enrolled and prepared as a single-threaded subreaper. The helper
and watchdog were per invocation; no product daemon was used.

SOURCE-MANIFEST.json identifies every code member and the canonical header
and Rust dependency pins. The snapshot-local Git attributes preserve exact
bytes; committed blobs are verified against the manifest.

The [reconstruction package](reproduction/README.md) includes the exact Rust
fixtures and a script that reproduces both historical source bundles from
the pinned Git trees. This remains a partial reproduction archive: complete
build and guest recipes and public raw execution evidence are separate.
The numerical result and evidence hashes are in
tests/evidence/linux-x64-ordinary-library-deadline-20261127-summary.json.
Earlier failed attempts retain their original outcomes. Installation,
owner-loss, fault, wider escape and platform qualification, default support,
and comparative performance remain open.
