# VASS Testing Strategy

VASS tests are organized by scope and target. All tests run through the Cargo workspace.

## Test locations

- Root `tests/` — CLI fixture tests (`tests/cli`), language fixtures (`tests/vass`, `tests/vcss`), integration tests (`tests/integration`), and watch tests (`tests/watch`).
- `bins/vass/tests` — CLI fixture tests scoped to the `vass` binary.
- `crates/*/tests/` — crate-level integration tests where present.
- Unit tests live alongside source in each crate.

## Running tests

Run the complete workspace suite:

```
cargo test --workspace --locked
```

Run tests for one crate:

```
cargo test -p vass-parser --locked
```

Run tests for one binary:

```
cargo test --bin vass --locked
```

Run a single test by name:

```
cargo test --workspace --locked test_name
```

## Fixtures

CLI and language fixtures are directory-based input/expected-output pairs. Add new fixtures under the relevant `tests/` subdirectory following the existing naming conventions. Do not modify existing fixtures without updating expected outputs.

## Benchmarks

Criterion benchmarks in `bench/` measure performance regressions and are not part of `cargo test --workspace`.

## CI

Tests run on every push and pull request via `.github/workflows/ci.yml` on the stable toolchain.
