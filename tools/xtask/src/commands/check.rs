//! `cargo xtask check` — run cargo check.

use crate::commands::{run_cargo, with_workspace};

pub fn run(args: &[String]) -> anyhow::Result<()> {
  run_cargo("check", &with_workspace(args))
}
