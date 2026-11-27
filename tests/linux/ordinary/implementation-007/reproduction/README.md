# Ordinary Linux fixture source inputs

These files reconstruct the two source bundles used to build the x86-64 ordinary Linux deadline fixture. They preserve the exact historical bytes from the one observed case.

- Supply a raw exported repository tree from Git commit `b8bfa00cd24e2eaf1f8733d2d46b28b9bd9f23af` as `--base-tree`. A Git archive preserves the tracked LF bytes. A checkout that already converted the deadline source to CRLF differs from the required raw base.
- Supply the exported `tests/linux/ordinary/implementation-007` directory from commit `932c12da9f770a6b609a6f9075ea1923fc2c270f` as `--snapshot`.
- Keep `ordinary_check.rs` and `ordinary_fixture.rs` alongside the script. With Python 3.9 or later, run `python reconstruct_inputs.py --base-tree BASE_EXPORT --snapshot CANDIDATE_EXPORT --output-dir NEW_DIRECTORY` under the repository's process deadline policy. The output directory must be absent and its parent must exist.

The reconstruction uses a CRLF byte conversion only for the historical deadline source, appends two fixture module declarations to the raw Linux module, and replaces one manager link attribute. The last change allowed the historical fixture-only Rust cfg to omit the otherwise unused separate-service link dependency. The published product sources and build configuration remain unchanged.

| Bundle | Inputs | Bytes | SHA-256 |
| --- | ---: | ---: | --- |
| `C-SOURCES.json` | 34 | 269981 | `93e0221d36fde430b83d1f13c853f474f15f90160a70c7d3c636d48fd697f4fc` |
| `RUST-SOURCES.json` | 23 | 651406 | `e1f5d05b9914c12384da5bfeb1dc4588ec9ac75656818ace782e444d711fe675` |

This package covers source input reconstruction. Toolchain provisioning, native object and ELF reproduction, guest setup, and raw execution evidence are separate. The reconstructor only reads bounded source files and writes the two checked JSON bundles.
