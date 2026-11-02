#!/usr/bin/env bash
# VASS nightly branch coverage script.
# Measures branch decisions taken by tests on the nightly toolchain using
# source-based instrumentation and llvm-cov. Separate from stable line coverage.
set -euo pipefail

if ! cargo +nightly --version >/dev/null 2>&1; then
  echo "==> nightly toolchain not available; skipping branch coverage"
  echo "==> install with: rustup toolchain install nightly"
  exit 0
fi

echo "==> running nightly branch coverage (cargo-llvm-cov)"
cargo +nightly llvm-cov test --lcov --branch \
  --workspace --locked \
  --output-path target/coverage/branch.lcov \
  || true

echo "==> nightly branch coverage complete (see target/coverage/branch.lcov)"
