#!/usr/bin/env bash
# VASS verification script.
# Runs formatting, clippy, check, and tests across the workspace.
set -euo pipefail

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo clippy --workspace --all-targets --locked -- -D warnings"
cargo clippy --workspace --all-targets --locked -- -D warnings

echo "==> cargo check --workspace --all-targets --locked"
cargo check --workspace --all-targets --locked

echo "==> cargo test --workspace --locked"
cargo test --workspace --locked

echo "==> verification complete"
