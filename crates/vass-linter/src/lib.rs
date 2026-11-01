//! # VASS Linter
//!
//! Linter for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Configurable lint rules
//! - Warning and error levels
//! - Auto-fix support

#![warn(missing_docs)]
#![allow(dead_code)]

/// Crate result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Linter errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// Lint rule error
  #[error("Lint error: {0}")]
  LintRule(String),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_error_display() {
    let err = Error::LintRule("bad rule".to_string());
    assert!(err.to_string().contains("Lint error: bad rule"));
  }

  #[test]
  fn test_result_type() {
    let _: Result<String> = Ok("ok".to_string());
    let _: Result<String> = Err(Error::LintRule("x".to_string()));
  }
}
