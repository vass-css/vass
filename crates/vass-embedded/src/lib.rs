//! # VASS Embedded
//!
//! Embedded protocol for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Host-compiler communication
//! - Protocol buffer encoding/decoding
//! - Importer dispatching

#![warn(missing_docs)]
#![allow(dead_code)]

/// Crate result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Embedded protocol errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// Protocol error
  #[error("Protocol error: {0}")]
  Protocol(String),

  /// IO error
  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_error_display() {
    let err = Error::Protocol("bad protocol".to_string());
    assert!(err.to_string().contains("Protocol error: bad protocol"));
  }

  #[test]
  fn test_result_type() {
    let _: Result<String> = Ok("ok".to_string());
    let _: Result<String> = Err(Error::Protocol("x".to_string()));
  }
}
