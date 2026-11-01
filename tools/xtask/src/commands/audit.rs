//! `cargo xtask audit` — run all quality gates in sequence.

use crate::commands::{run_cargo, with_workspace};

pub fn run() -> anyhow::Result<()> {
  println!("== fmt ==");
  run_cargo(
    "fmt",
    &["--all".to_string(), "--".to_string(), "--check".to_string()],
  )?;
  println!("== lint ==");
  run_cargo(
    "clippy",
    &with_workspace(&[
      "--all-targets".to_string(),
      "--locked".to_string(),
      "--".to_string(),
      "-D".to_string(),
      "warnings".to_string(),
    ]),
  )?;
  println!("== check ==");
  run_cargo(
    "check",
    &with_workspace(&["--all-targets".to_string(), "--locked".to_string()]),
  )?;
  println!("== test ==");
  run_cargo("test", &with_workspace(&["--locked".to_string()]))?;
  println!("== doc ==");
  run_cargo(
    "doc",
    &with_workspace(&["--no-deps".to_string(), "--locked".to_string()]),
  )?;
  println!("audit: all gates passed");
  Ok(())
}
