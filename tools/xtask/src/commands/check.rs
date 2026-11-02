//! `cargo xtask check` — run cargo check.

use crate::commands::{run_cargo, with_workspace};

pub fn run(args: &[String]) -> anyhow::Result<()> {
  run_cargo("check", &with_workspace(args))
}
#[cfg(test)]
mod tests {

  #[test]
  fn test_check_args() {
    let _ = vec!["--locked".to_string()];
  }

  #[test]
  fn test_check_run_exists() {
    let _ = std::any::type_name::<fn(&[String]) -> anyhow::Result<()>>();
  }
}
