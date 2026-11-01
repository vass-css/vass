# Development Setup

Install the stable Rust toolchain and the `rustfmt` and `clippy` components. The repository's `rust-toolchain.toml` selects the toolchain.

From the repository root:

```sh
cargo build --workspace
cargo run -p vass -- --help
```

The optional devcontainer installs the same editor extensions and Rust components.
