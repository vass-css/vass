//! `vass lint` is an alias for the existing file checker.

pub use super::check::Args;

/// Run the lint command.
pub async fn run(args: Args) -> anyhow::Result<()> {
  super::check::run(args).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_lint_args_exist() {
    let _ = Args {
      files: vec!["test.vass".to_string()],
      format: super::super::check::Format::Crisp,
      parse_only: false,
    };
  }
}
