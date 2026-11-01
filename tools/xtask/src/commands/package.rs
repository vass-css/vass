//! `cargo xtask package` — package crates for publishing.

use crate::commands::run_cargo;

pub fn run(args: &[String]) -> anyhow::Result<()> {
  run_cargo("package", args)
}
