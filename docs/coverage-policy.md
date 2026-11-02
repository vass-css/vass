# VASS Coverage Policy

VASS measures coverage in two separate modes: **stable line coverage** and **nightly branch coverage**. They are kept distinct and are never aggregated into a single number.

## Stable line coverage

Measured on the stable toolchain with `cargo-tarpaulin`. This mode tracks which lines of code are executed by the test suite. It runs on every CI build and must not regress below the project baseline.

```
./scripts/coverage-line.sh
```

## Nightly branch coverage

Measured on the nightly toolchain with source-based instrumentation (`-C instrument-coverage`) and `llvm-cov`. This mode tracks branch decisions taken by tests and is reported separately from line coverage.

```
./scripts/coverage-branch.sh
```

## Separation

Line coverage and branch coverage are separate concerns:

- Line coverage answers "which lines ran?"
- Branch coverage answers "which branches ran?"

They are produced by different toolchains and scripts and are reported independently. Do not combine them into a single coverage percentage.

## Thresholds

Any new public API or logic change must be accompanied by tests covering the new lines and branches. Coverage regressions on stable line coverage block merge; nightly branch coverage is informational and trends upward over time.

## Artifacts

Coverage reports are written to `target/` and CI-uploaded artifacts. Coverage data files (`*.info`, `lcov.info`) are not committed to the repository.
