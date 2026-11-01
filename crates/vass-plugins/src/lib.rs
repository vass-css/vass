//! # VASS Plugins
//!
//! Plugin system for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Custom function plugins
//! - Custom importers
//! - Dart VASS compatibility adapter
//! - WASM plugin support

#![warn(missing_docs)]
#![allow(dead_code)]

/// Crate result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Plugin errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// Plugin not found
  #[error("Plugin not found: {0}")]
  NotFound(String),

  /// Plugin load error
  #[error("Failed to load plugin: {0}")]
  LoadError(String),

  /// Plugin execution error
  #[error("Plugin execution error: {0}")]
  ExecutionError(String),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_error_display() {
    let err = Error::NotFound("my-plugin".to_string());
    assert!(err.to_string().contains("Plugin not found: my-plugin"));
    let err = Error::LoadError("bad load".to_string());
    assert!(err.to_string().contains("Failed to load plugin: bad load"));
    let err = Error::ExecutionError("bad exec".to_string());
    assert!(err.to_string().contains("Plugin execution error: bad exec"));
  }

  #[test]
  fn test_result_type() {
    let _: Result<String> = Ok("ok".to_string());
    let _: Result<String> = Err(Error::NotFound("x".to_string()));
  }
}
