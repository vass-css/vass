# Codebase Map

- `crates/vass-core`: shared ASTs, values, and diagnostics.
- `crates/vass-parser`: lexing, parsing, syntax detection, and balance checks.
- `crates/vass-evaluator`: evaluation and built-in modules.
- `crates/vass-transpiler`: CSS output and source maps.
- `crates/vass-io`: file loading and compilation orchestration.
- `crates/vass-linter` and `crates/vass-fmt`: linting and formatting libraries.
- `crates/vass-lsp`: language-server functionality.
- `crates/vass-embedded` and `crates/vass-wasm`: embedding interfaces.
- `crates/vass-plugins`: plugin support.
- `bins/`: executable entry points and CLI integration tests.
- `tests/`: CLI fixtures and behavior-focused regression suites.
- `editors/vscode`: VS Code language extension.
- `bench/`: Criterion benchmarks for core compilation stages.
- `tools/xtask`: repository maintenance commands.
