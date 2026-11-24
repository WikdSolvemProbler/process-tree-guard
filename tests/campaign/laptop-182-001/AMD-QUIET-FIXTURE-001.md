# Prospective laptop fixture and instrumentation amendment

Apply this amendment before starting a new native A/A qualification or Phase A campaign. It supplements the original protocol (SHA-256 `67f14e04a8750ab9ed8e7b1ec6c3b02f22dff8a7a51774444f263672044587fe`) and A/A clarification (SHA-256 `4743e6037cf51dba5beedbd9c875afb315519a78908b075969033e65b0d79468`). Both original documents retain their published bytes.

## Workload fixture

Use one fresh Windows x86_64 fixture executable for every revised A/A and Phase A slot or control that calls testkit, including direct relay, wide and deep trees, and the stock pricing probes. The fixture is `testkit.exe` from source commit `caebd14329e454584fe5d263efe7f4213d77aa54`, 630,272 bytes, SHA-256 `346ca22737701fb5e2a7188d0ad9aadca07a496984ca83c7e34747997324f8a3`. The extracted raw-LF `tests/support/testkit.rs` has SHA-256 `1f25f0d13febbcf85cf5a84a9903315bc23cb2545e576d0ea4e52b6ea920a026`. The exact source archive has SHA-256 `bb1364c8d4b56b636d58db8c2b4b1d3ad327eec4f53487b31d880a64cc693d8d`.

Build from the Git archive with Rust 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), MSVC 14.51.36231, Windows SDK 10.0.28000.0 and `RUSTFLAGS=-D warnings`. Use a fresh target directory and the committed release profile.

```text
cargo build --offline --locked --release --target x86_64-pc-windows-msvc --features testkit --bin testkit -v
```

The retained original build process exited 0. Build recipe SHA-256 `b42193e8a155c43b4bd29025cddd6b69920305495c8559f7d1069b4a79908073`, toolchain index SHA-256 `6a04de07e6c23d2a767e8c7e15812403ce5da47c416ceffaf49dfaf93d38c0f9`, build log SHA-256 `0c49753ec45c12dc1fea1e1275edebe9936f3c0b37a746829bc441b05214e2ec`, and build result SHA-256 `81ef143d3131804a82ff1db3ac1f528dae6c02617038fd3d46a0a2155c89adb7` bind that build. Post-build verification matched all 1,044 source paths and four tool images.

At startup every fixture invocation applies the current source's Windows error-dialog suppression and verifies the mode. The wide child starts its sleep descendants and the deep child starts subsequent levels through the current source's `CREATE_NO_WINDOW` helper with null streams. That flag can create a console host with no visible window, so the change can affect elapsed time, resource cost and total Job membership. Retain the exact fixture PID counts and all existing survivor and whole-Job cleanup checks.

Make one byte-preserving substitution in the laptop-only copy of `tests/campaign-sysprims-fast-warn.json` (base SHA-256 `dbd53f039277755f40d9c3156e85c6e92a67a018778109f11e5effd71d1e2ce5`). In `children.kill.args[1]` replace the string `"0"` with `"134217728"`. The unchanged `spawn` command forwards this decimal value as `CREATE_NO_WINDOW` for the marked child. The exact revised config has SHA-256 `16b551edb24a42dd2cc8db1ffb822e0a4359c4f522981a3bff35554bc1cfafb9`, 35,415 bytes. Retain the same executable path binding and argument structure, inherited marked-child streams, marker publication, two-second arm deadline and 30-second hold. Add the new fixture hash to the inventory veto set and keep the historical fixture digest `d55d6b947c921b9d28e803a5057dac51e8df044fbdd7106b88880292c4c47a2f` and all other existing vetoes.

## Persistent instrumentation workers

Start each persistent Python worker for the A/A qualification, pricing witness, and campaign with `DETACHED_PROCESS` in place of its previous `CREATE_NO_WINDOW` flag. Keep the same Python image, arguments, working directory, environment, explicit NUL input, captured output/error streams and original creation-time Job assignment. Keep each actual trial, product arm and pricing guard launch's existing `NO_WINDOW` or `DETACHED` context and all stock measured start and end points.

Two direct `testkit sleep 30` controls also use `DETACHED_PROCESS`: the qualification's defensive Job drain control and the pricing witness's deliberate extra-member negative control. Each must present one exact fixture process in its assigned Job, with its existing NUL streams, arguments, containment and finite deadlines. The native Job census and expected refusal or drain remain mandatory. Guarded trial and pricing launch contexts and the marked child's `CREATE_NO_WINDOW` flag keep their respective specified values.

In the retained setup-only comparison, the previous instrumentation flag created a console host in the worker's Job before any console query. The detached worker had no console clients, was the sole Job member before and after the queries, and drained naturally. Each new launch still needs its original-handle evidence. Every pricing settlement still requires the exact sole-worker proof with complete membership, and every trial still requires its existing cleanup proof and deadlines. The setup diagnostic supplies only evidence about persistent worker launch on this host.

## Cohort and evidence

The fixture executable, its startup behavior and internal child creation contexts define a new workload cohort. Keep all earlier staged inputs, diagnostics, failures and receipts under their original identities. Report the new cohort separately and use it uniformly across rivals and contexts. This amendment is confined to qualification and Phase A. Phase B's matched-reference requirements retain their original fixture and matching rules unless separately amended.

The original sample counts, seeds, ordering rules, A/A assignment and observer timing, launch contexts of trial arms, stream and exit oracles, deadlines, marker checks, pricing comparisons, and adjudication thresholds apply. Before any revised fixture executions, both independent Sol reviewers approve the same amendment bytes and bind the new fixture, config, exact implementation, and launch recipe. Retain fresh native checks for process identity, expected markers, console visibility and cleanup under the amended launch before the campaign is allowed to score results.
