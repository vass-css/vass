# Contributing

## Development

Install the stable Rust toolchain with `rustfmt` and `clippy`, then build from the repository root with `cargo build --workspace`.

## Before Submitting

Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and the relevant tests. Include a regression test for behavior changes and update documentation when user-visible behavior changes.

Keep pull requests focused and describe the motivation, implementation, and validation performed.
