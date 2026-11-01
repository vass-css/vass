//! `cargo xtask version` — print the workspace version.

use crate::commands::workspace_version;

pub fn run() -> anyhow::Result<()> {
  let v = workspace_version()?;
  println!("{v}");
  Ok(())
}
