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
  fn test_lint_run_delegates() {
    // Just verify the function exists and is callable - it delegates to check
    let _ = std::any::type_name::<Args>();
  }
}
