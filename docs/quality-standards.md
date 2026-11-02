# VASS Quality Standards

VASS enforces formatting, linting, compiling, and testing standards across the entire workspace. These standards are enforced locally via `scripts/verify.sh` and in CI via `.github/workflows/ci.yml`.

## Formatting

All Rust source must be formatted with `rustfmt` according to `rustfmt.toml`:

```
cargo fmt --all -- --check
```

Formatting violations fail CI. Run `cargo fmt --all` to apply.

## Linting

Clippy must run clean with warnings denied across all workspace targets:

```
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Do not introduce `allow` attributes to silence clippy warnings; fix the underlying issue instead.

## Compilation

The workspace must compile cleanly with all targets:

```
cargo check --workspace --all-targets --locked
```

## Testing

The full test suite must pass:

```
cargo test --workspace --locked
```

New features and bug fixes must include tests. Existing tests must not be removed or weakened.

## Toolchain

- MSRV: Rust 1.99 (edition 2024).
- `rust-toolchain.toml` pins stable with `rustfmt` and `clippy`.
- Preserve the MSRV and toolchain file.

## Dependencies

Do not add unnecessary dependencies. Prefer workspace-managed dependencies declared in the root `Cargo.toml`.

## CI gates

CI runs formatting, clippy, check, tests, builds, and documentation on every push and pull request. All gates must pass before merge.
