#!/usr/bin/env bash
# VASS nightly coverage script.
# LLVM source-based instrumentation measures lines, functions, regions, and branches.
# Branch coverage is separate from stable Tarpaulin line coverage.
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
diagnostics="target/coverage/uncovered-items.txt"
mkdir -p "$(dirname "$report")"

echo "==> cleaning previous LLVM coverage data"
cargo +nightly llvm-cov clean --workspace

echo "==> running nightly workspace coverage"
set +e
cargo +nightly llvm-cov test \
  --workspace --locked \
  --branch \
  --lcov \
  --show-missing-lines \
  --output-path "$report" \
  --fail-under-lines 100 \
  --fail-under-file-lines 100 \
  --fail-under-functions 100 \
  --fail-under-regions 100
coverage_status=$?
set -e

if [[ ! -s "$report" ]]; then
  echo "error: LLVM coverage did not produce $report" >&2
  exit 1
fi

echo "==> enumerating every uncovered LCOV line, function, and branch"
awk -F: '
  /^SF:/ {
    file = substr($0, 4)
  }
  /^DA:/ {
    split(substr($0, 4), fields, ",")
    if (fields[2] == 0) {
      printf "LINE %s:%s\n", file, fields[1]
      uncovered_lines++
    }
  }
  /^FNDA:/ {
    split(substr($0, 6), fields, ",")
    if (fields[1] == 0) {
      printf "FUNCTION %s:%s\n", file, fields[2]
      uncovered_functions++
    }
  }
  /^BRDA:/ {
    split(substr($0, 6), fields, ",")
    if (fields[4] == "-" || fields[4] == 0) {
      printf "BRANCH %s:%s block=%s branch=%s\n", file, fields[1], fields[2], fields[3]
      uncovered_branches++
    }
  }
  END {
    printf "Uncovered lines: %d\n", uncovered_lines
    printf "Uncovered functions: %d\n", uncovered_functions
    printf "Uncovered branch outcomes: %d\n", uncovered_branches
  }
' "$report" > "$diagnostics"

cat "$diagnostics"
echo "==> detailed diagnostics: $diagnostics"

if [[ "$coverage_status" -ne 0 ]]; then
  echo "error: one or more line/function/region coverage metrics are below 100%%" >&2
  exit "$coverage_status"
fi

echo "==> verifying branch outcome totals"
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
