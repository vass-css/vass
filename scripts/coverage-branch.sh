#!/usr/bin/env bash
# VASS nightly coverage script.
# Measures line, function, region, and branch coverage with LLVM source-based
# instrumentation. The stable Tarpaulin job remains a separate line baseline.
set -euo pipefail

if ! cargo +nightly --version >/dev/null 2>&1; then
  echo "error: the nightly Rust toolchain is required for branch coverage" >&2
  exit 1
fi

if ! cargo +nightly llvm-cov --version >/dev/null 2>&1; then
  echo "==> installing cargo-llvm-cov"
  cargo +nightly install cargo-llvm-cov --locked
fi

report="target/coverage/branch.lcov"
mkdir -p "$(dirname "$report")"

echo "==> cleaning previous LLVM coverage data"
cargo +nightly llvm-cov clean --workspace

echo "==> running nightly workspace coverage"
set +e
cargo +nightly llvm-cov test \
  --workspace --locked \
  --branch \
  --lcov \
  --output-path "$report" \
  --fail-under-lines 100 \
  --fail-under-file-lines 100 \
  --fail-under-functions 100 \
  --fail-under-regions 100
coverage_status=$?

if [[ ! -s "$report" ]]; then
  echo "error: LLVM coverage did not produce $report" >&2
  exit 1
fi

echo "==> checking every instrumented branch outcome"
awk -F: '
  /^BRF:/ { total += $2 }
  /^BRH:/ { covered += $2 }
  END {
    if (total == 0) {
      print "error: no branch coverage data was produced" > "/dev/stderr"
      exit 2
    }

    printf "Branch coverage: %d/%d (%.2f%%)\n", covered, total, 100 * covered / total

    if (covered != total) {
      printf "error: %d branch outcomes remain uncovered\n", total - covered > "/dev/stderr"
      exit 1
    }
  }
' "$report"
branch_status=$?
set -e

if [[ "$coverage_status" -ne 0 || "$branch_status" -ne 0 ]]; then
  echo "error: one or more required coverage dimensions are below 100%%" >&2
  exit 1
fi

echo "==> nightly line, function, region, and branch coverage: 100%"
echo "==> report: $report"
