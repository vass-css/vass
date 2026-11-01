//! `cargo xtask bench` — run cargo bench.

use crate::commands::run_cargo;

pub fn run(args: &[String]) -> anyhow::Result<()> {
  run_cargo("bench", args)
}
