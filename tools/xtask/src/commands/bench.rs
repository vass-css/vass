//! `cargo xtask bench` — run cargo bench.

use crate::commands::run_cargo;

pub fn run(args: &[String]) -> anyhow::Result<()> {
  run_cargo("bench", args)
}

#[cfg(test)]
mod tests {

  #[test]
  fn test_bench_args() {
    let _ = vec!["--verbose".to_string()];
  }

  #[test]
  fn test_bench_run_exists() {
    let _ = std::any::type_name::<fn(&[String]) -> anyhow::Result<()>>();
  }
}
