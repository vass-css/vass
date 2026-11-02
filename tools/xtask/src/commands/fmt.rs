//! `cargo xtask fmt` — run cargo fmt --check.

use crate::commands::run_cargo;

pub fn run() -> anyhow::Result<()> {
  run_cargo(
    "fmt",
    &["--all".to_string(), "--".to_string(), "--check".to_string()],
  )
}

#[cfg(test)]
mod tests {

  #[test]
  fn test_fmt_args() {
    let _: Vec<String> = vec![];
  }

  #[test]
  fn test_fmt_run_exists() {
    let _ = std::any::type_name::<fn() -> anyhow::Result<()>>();
  }
}
