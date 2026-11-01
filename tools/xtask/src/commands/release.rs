//! `cargo xtask release <version>` — validate release readiness.
//!
//! This command validates that the given version matches the workspace,
//! all publishable crates declare it, and quality gates pass. Publishing
//! itself is intentionally NOT performed by xtask — it is driven by the
//! GitHub Actions release workflow which has the necessary OIDC/trusted
//! publishing permissions.

use crate::commands::{check_all_crate_versions, run_cargo, with_workspace, workspace_version};

pub fn run(version: &str) -> anyhow::Result<()> {
  let v = version.strip_prefix('v').unwrap_or(version);
  println!("== version check ==");
  let ws = workspace_version()?;
  if ws != v {
    anyhow::bail!("workspace version {ws} does not match requested {v}");
  }
  println!("workspace version: {ws}");
  check_all_crate_versions(v)?;
  println!("all crates declare version {v}");

  println!("== quality gates ==");
  run_cargo(
    "fmt",
    &["--all".to_string(), "--".to_string(), "--check".to_string()],
  )?;
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
  run_cargo(
    "check",
    &with_workspace(&["--all-targets".to_string(), "--locked".to_string()]),
  )?;
  run_cargo("test", &with_workspace(&["--locked".to_string()]))?;
  run_cargo(
    "build",
    &with_workspace(&["--release".to_string(), "--locked".to_string()]),
  )?;

  println!("== dry-run package ==");
  run_cargo("package", &["--locked".to_string()])?;

  println!("release {v}: all validation passed");
  println!("NOTE: publishing is performed by the GitHub Actions release workflow.");
  Ok(())
}
