# Crate Dependencies

The primary compilation path is `vass-parser` -> `vass-evaluator` -> `vass-transpiler`, with `vass-core` providing shared types. `vass-io` composes those stages for files, and `bins/vass` exposes them through the CLI.

The LSP, embedded, and WASM crates are consumer-facing layers over core functionality. Keep shared language behavior in the owning library crate rather than duplicating it in a binary or interface layer.

The workspace dependency table in the root `Cargo.toml` is the canonical source for shared crate paths and common dependencies.
