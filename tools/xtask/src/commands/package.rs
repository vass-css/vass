//! `cargo xtask package` — package crates for publishing.

use crate::commands::run_cargo;

pub fn run(args: &[String]) -> anyhow::Result<()> {
  run_cargo("package", args)
}

#[cfg(test)]
mod tests {

  #[test]
  fn test_package_args() {
    let _: Vec<String> = vec![];
  }

  #[test]
  fn test_package_run_exists() {
    let _ = std::any::type_name::<fn(&[String]) -> anyhow::Result<()>>();
  }
}
