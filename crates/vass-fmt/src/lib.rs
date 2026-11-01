//! # VASS Formatter
//!
//! Formatter for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Consistent code formatting
//! - Configurable style options
//! - Both .vass and .vcss support

#![warn(missing_docs)]
#![allow(dead_code)]

/// Crate result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Formatter errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// Format error
  #[error("Format error: {0}")]
  Format(String),

  /// Parse error
  #[error("{0}")]
  Parse(String),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_error_display() {
    let err = Error::Format("bad format".to_string());
    assert!(err.to_string().contains("Format error: bad format"));
    let err = Error::Parse("bad parse".to_string());
    assert!(err.to_string().contains("bad parse"));
  }

  #[test]
  fn test_result_type() {
    let _: Result<String> = Ok("ok".to_string());
    let _: Result<String> = Err(Error::Format("x".to_string()));
  }
}
