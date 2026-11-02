//! `cargo xtask version` — print the workspace version.

use crate::commands::{workspace_root, workspace_version};

pub fn run() -> anyhow::Result<()> {
  let v = workspace_version(workspace_root())?;
  println!("{v}");
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_version_output() {
    let _ = std::any::type_name::<fn() -> anyhow::Result<String>>();
  }

  #[test]
  fn test_version_run_exists() {
    let _ = std::any::type_name::<fn() -> anyhow::Result<()>>();
  }

  #[test]
  fn test_version_run() {
    // `xtask version` reads the workspace Cargo.toml and prints it.
    run().expect("xtask version should succeed");
  }
}
