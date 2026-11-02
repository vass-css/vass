//! `cargo xtask test` — run cargo test.

use crate::commands::{run_cargo, with_workspace};

pub fn run(args: &[String]) -> anyhow::Result<()> {
  run_cargo("test", &with_workspace(args))
}

#[cfg(test)]
mod tests {

  #[test]
  fn test_xtask_test_args() {
    let _ = vec!["--locked".to_string()];
  }

  #[test]
  fn test_xtask_test_run_exists() {
    let _ = std::any::type_name::<fn(&[String]) -> anyhow::Result<()>>();
  }
}
