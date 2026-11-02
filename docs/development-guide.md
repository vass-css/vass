# VASS Development Guide

This guide covers setting up and working on the VASS repository.

## Prerequisites

- Rust toolchain (stable channel, MSRV 1.99) — managed by `rust-toolchain.toml`.
- `rustfmt` and `clippy` components.

Install the toolchain:

```
rustup install stable
rustup component add rustfmt clippy
```

## Building

Build the whole workspace:

```
cargo build --workspace
```

Build a single crate:

```
cargo build -p vass-parser
```

Build a binary:

```
cargo build --bin vass
```

## Formatting and linting

Check formatting:

```
cargo fmt --all -- --check
```

Apply formatting:

```
cargo fmt --all
```

Run clippy with warnings denied:

```
cargo clippy --workspace --all-targets --locked -- -D warnings
```

## Checking and testing

Check all workspace targets:

```
cargo check --workspace --all-targets --locked
```

Run the full test suite:

```
cargo test --workspace --locked
```

Test a single crate:

```
cargo test -p vass-parser
```

Run a specific test:

```
cargo test -p vass-parser test_name
```

## Benchmarks

Criterion benchmarks live in `bench/`:

```
cargo bench --package vass-benchmarks --no-run
cargo bench --package vass-benchmarks
```

## Project structure

- `crates/` — library crates
- `bins/` — executable crates
- `tests/` — CLI fixtures, language fixtures, integration tests, watch tests
- `tools/` — helper tooling (xtask)
- `docs/` — engineering documentation
- `scripts/` — verification and coverage scripts
