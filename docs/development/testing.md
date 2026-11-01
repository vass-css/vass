# Testing

Run the complete Rust test suite with `cargo test --workspace`. For faster iteration, target a package with `cargo test -p vass-parser` or a specific integration target.

CLI fixture tests live under `bins/vass/tests`; language and integration fixtures live under the root `tests/` tree. Criterion benchmarks are in `bench/` and can be compiled with `cargo bench --package vass-benchmarks --no-run`.