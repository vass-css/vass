# VASS Agent Guidelines

This document provides guidance for agents and contributors working on the VASS CSS preprocessor repository.

## Repository layout

VASS is a Rust Cargo workspace:

- `crates/` — library crates (`vass-core`, `vass-parser`, `vass-evaluator`, `vass-transpiler`, `vass-embedded`, `vass-io`, `vass-plugins`, `vass-lsp`, `vass-wasm`, `vass-linter`, `vass-fmt`)
- `bins/` — executable crates (`vass`, `vass-lsp-server`)
- `tests/` — CLI fixtures, language fixtures, integration tests, watch tests
- `tools/` — helper tooling (`xtask`)
- `bench/` — criterion benchmarks
- `docs/` — engineering documentation
- `scripts/` — verification and coverage scripts
- `.github/workflows/` — CI configuration

The workspace root `Cargo.toml` defines the members, shared dependencies, and profiles. Do not change compiler implementation or existing application behavior.

## Toolchain

- MSRV: Rust 1.99 (edition 2024), declared in the root `Cargo.toml` under `[workspace.package] rust-version`.
- `rust-toolchain.toml` pins the stable channel with `rustfmt` and `clippy` components and the `minimal` profile for `x86_64-unknown-linux-gnu`.
- Preserve `rust-toolchain.toml` and the MSRV. Do not add unnecessary dependencies.

## Commands

Run validation with `scripts/verify.sh`:

```
./scripts/verify.sh
```

Run stable line coverage:

```
./scripts/coverage-line.sh
```

Run nightly branch coverage:

```
./scripts/coverage-branch.sh
```

Individual cargo commands:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
```

## Documentation

- `docs/architecture.md` — system architecture
- `docs/development-guide.md` — development setup and workflow
- `docs/testing-strategy.md` — test organization and execution
- `docs/coverage-policy.md` — line vs branch coverage policy
- `docs/quality-standards.md` — formatting, linting, and quality bars

Existing docs under `docs/architecture/`, `docs/development/`, and `docs/src/` are preserved and remain the canonical UI documentation source.

## Pull requests

- Keep changes scoped; do not modify the compiler implementation or alter existing behavior.
- Run `scripts/verify.sh` before pushing.
- Update documentation when user-facing behavior or project structure changes.
- Do not commit secrets or keys.
