//! # VASS Core
//!
//! Core crate providing AST definitions, value types, error handling, and tokens
//! for the VASS CSS preprocessor.
//!
//! ## Structure
//!
//! - `ast` - Abstract Syntax Tree definitions (VASS input and CSS output)
//! - `value` - Value types (numbers, colors, lists, maps, etc.)
//! - `token` - Lexer token types
//! - `error` - Error types and diagnostics
//! - `util` - Utility functions

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]
#![allow(dead_code)]

pub mod ast;
pub mod error;
pub mod token;
pub mod util;
pub mod value;

// These modules will be implemented in later phases
// pub mod callable;
// pub mod module;

/// Crate result type alias
pub type Result<T> = std::result::Result<T, error::Error>;

/// Prelude module for commonly used types
pub mod prelude {
  pub use crate::Result;
  pub use crate::error::Error;
  pub use crate::token::{Kind, Span, Token};
}
