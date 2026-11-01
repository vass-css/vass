//! Sass AST (input)
//!
//! The tree the parser builds from `.vass` (indented) or `.vcss` (braces)
//! source before evaluation.

pub mod expression;
pub mod statement;

pub use expression::Expr;
pub use statement::{Statement, Stylesheet};

/// Which source syntax a stylesheet was written in
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// Indented syntax (`.vass`), like Sass
    Indented,
    /// Brace syntax (`.vcss`), like SCSS
    Braces,
}
