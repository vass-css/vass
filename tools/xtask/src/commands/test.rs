//! `cargo xtask test` — run cargo test.

use crate::commands::{run_cargo, with_workspace};

pub fn run(args: &[String]) -> anyhow::Result<()> {
  run_cargo("test", &with_workspace(args))
}
