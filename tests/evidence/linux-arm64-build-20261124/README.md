# Native ARM64 build evidence

Commit 9bd332de9b333aa0167afc4da1298d76edf9bc10 was built on ARM64 GNU/Linux with native Cargo and Rust 1.88.0. The source archive contains 136 Git members. Nine captured commands exited zero. All 12 serial client component tests and all 14 ordinary fixture I/O tests passed.

The working scope used a 180-second runtime limit and a five-second stop limit. Startup was admitted against the original 200-second Linux timeout. The retained worker handle reached terminal state and the original scope and cgroup retired. The receipt observed cgroup retirement through the held events descriptor and the absent original name.

## Build recipe

The native compiler and archive tool came from the guest. Cargo used a fresh private home and target directory, offline and locked, with the native C compiler as linker.

```sh
cargo build --release --locked --offline --target aarch64-unknown-linux-gnu --features linux-separate-service --bin process-tree-guard-root --bin process-tree-guard-client
cargo test --release --locked --offline --target aarch64-unknown-linux-gnu --features linux-separate-service --bin process-tree-guard-client -- --test-threads=1
```

The existing Python fixture I/O suite ran with isolated Python and the ordinary-user check from the CI recipe. Both Cargo invocations emitted the two per-binary edition deprecation warnings preserved in the raw logs. The SSH stderr stream contained 72 bytes. The local carrier stderr was empty. Their identities appear separately in the summary.

## Evidence scope

The two new ELFs match the historical bytes. The historical compiler identity remains unverified. This packet covers the ordinary build, component tests and original experiment scope retirement. Privileged bootstrap, full Linux backend behavior and comparative performance remain outside its evidence.

Attempt 002 failed compiler discovery. Attempt 003 was rejected at transient service admission. Both live managers later reported systemd 255.4-1ubuntu8.18, so the exact property rejection mechanism remains unresolved. Attempt 004 stayed unrun after timing review. Original artifacts and identity-bearing receipts remain private. The manifest binds the exact published bytes.
