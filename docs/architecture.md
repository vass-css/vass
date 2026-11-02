# VASS Architecture

VASS is a modern CSS preprocessor written in Rust, organized as a Cargo workspace. Source text flows through parsing, evaluation, and CSS serialization, with CLI, language server, embedded, and WASM interfaces composing those libraries for different consumers.

## Crate boundaries

- `vass-core` — shared syntax trees, diagnostics, and common types used across the workspace.
- `vass-parser` — syntax detection and parsing of VASS/CSS source into syntax trees.
- `vass-evaluator` — value resolution, modules, and evaluation into CSS syntax trees.
- `vass-transpiler` — CSS serialization from syntax trees.
- `vass-io` — filesystem-oriented compilation.
- `vass-plugins` — plugin system for extensibility.
- `vass-lsp` — language server protocol implementation.
- `vass-wasm` — WASM interface for browser and Node consumers.
- `vass-linter` — lint rules for stylesheets.
- `vass-fmt` — source formatting.

## Executables

- `bins/vass` — command-line interface.
- `bins/vass-lsp-server` — language server executable.

## Data flow

Source text → parser (syntax tree) → evaluator (resolved values/modules) → transpiler (CSS output) → emitted by `vass-io`, `vass`, `vass-lsp`, or `vass-wasm` depending on the consumer.

See `docs/architecture/codebase-map.md` and `docs/architecture/crate-dependencies.md` for navigation and package boundaries.
