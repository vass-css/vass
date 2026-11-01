# Architecture

VASS is organized as a Rust workspace. Source text flows through parsing, evaluation, and CSS serialization; the CLI, language server, embedded interface, and WASM interface compose those libraries for different consumers.

The parser owns syntax detection and syntax trees. The evaluator resolves values and modules into CSS syntax trees. The transpiler serializes CSS, while `vass-io` handles filesystem-oriented compilation. Shared syntax trees and diagnostics live in `vass-core`.

See [the codebase map](codebase-map.md) and [crate dependencies](crate-dependencies.md) for navigation and package boundaries.
