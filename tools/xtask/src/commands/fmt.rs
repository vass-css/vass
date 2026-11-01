//! `cargo xtask fmt` — run cargo fmt --check.

use crate::commands::run_cargo;

pub fn run() -> anyhow::Result<()> {
  run_cargo(
    "fmt",
    &["--all".to_string(), "--".to_string(), "--check".to_string()],
  )
}
