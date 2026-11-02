#!/usr/bin/env bash
# VASS stable line coverage script.
# Measures which lines are executed by the test suite on the stable toolchain.
# Requires cargo-tarpaulin; installs it if missing.
set -euo pipefail

if ! command -v cargo-tarpaulin >/dev/null 2>&1; then
  echo "==> installing cargo-tarpaulin"
  cargo install cargo-tarpaulin --locked
fi

echo "==> running stable line coverage (cargo-tarpaulin)"
cargo tarpaulin --workspace --out lcov --engine Llvm

echo "==> stable line coverage complete (see lcov.info / target/tarpaulin)"
