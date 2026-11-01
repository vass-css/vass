//! `cargo xtask verify` — run fmt, check, and test.

use crate::commands::{run_cargo, with_workspace};

pub fn run() -> anyhow::Result<()> {
  println!("== fmt ==");
  run_cargo(
    "fmt",
    &["--all".to_string(), "--".to_string(), "--check".to_string()],
  )?;
  println!("== check ==");
  run_cargo(
    "check",
    &with_workspace(&["--all-targets".to_string(), "--locked".to_string()]),
  )?;
  println!("== test ==");
  run_cargo("test", &with_workspace(&["--locked".to_string()]))?;
  println!("verify: all gates passed");
  Ok(())
}
