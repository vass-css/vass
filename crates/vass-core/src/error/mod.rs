//! Error module
//!
//! Provides error types and diagnostics for the VASS CSS preprocessor.

pub mod crisp;
mod diagnostic;
#[allow(clippy::module_inception)]
mod error;
pub mod render;

pub use diagnostic::*;
pub use error::*;
pub use render::{color_enabled, render};

/// Crate-wide result alias, re-exported for convenience
pub use crate::Result;
