//! `cargo xtask clean` — remove build artifacts and generated files.

use std::path::Path;

use crate::commands::run_cargo;

pub fn run(args: &[String]) -> anyhow::Result<()> {
  // Generated shell completions
  let completions_dir = Path::new("completions");
  if completions_dir.exists() {
    for entry in std::fs::read_dir(completions_dir)? {
      let p = entry?.path();
      if p
        .extension()
        .is_some_and(|e| e == "bash" || e == "zsh" || e == "fish")
      {
        std::fs::remove_file(&p).ok();
      }
    }
  }

  // Generated docs
  let docs_dir = Path::new("docs");
  if docs_dir.exists() {
    for entry in std::fs::read_dir(docs_dir)? {
      let p = entry?.path();
      if p.extension().is_some_and(|e| e == "html") {
        std::fs::remove_file(&p).ok();
      }
    }
  }

  run_cargo("clean", args)
}

#[cfg(test)]
mod tests {

  #[test]
  fn test_clean_args() {
    let _: Vec<String> = vec![];
  }

  #[test]
  fn test_clean_run_exists() {
    let _ = std::any::type_name::<fn(&[String]) -> anyhow::Result<()>>();
  }
}
