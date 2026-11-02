//! `cargo xtask lint` — run cargo clippy.

use crate::commands::{run_cargo, with_workspace};

pub fn run(args: &[String]) -> anyhow::Result<()> {
  let mut a = with_workspace(&[
    "--all-targets".to_string(),
    "--locked".to_string(),
    "--".to_string(),
    "-D".to_string(),
    "warnings".to_string(),
  ]);
  a.extend_from_slice(args);
  run_cargo("clippy", &a)
}

#[cfg(test)]
mod tests {

  #[test]
  fn test_lint_args() {
    let _: Vec<String> = vec![];
  }

  #[test]
  fn test_lint_run_exists() {
    let _ = std::any::type_name::<fn(&[String]) -> anyhow::Result<()>>();
  }
}
