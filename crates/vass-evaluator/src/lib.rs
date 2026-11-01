//! # VASS Evaluator
//!
//! Expression evaluator and built-in functions for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Expression evaluation
//! - Variable scoping
//! - Nested rule flattening with `&` support
//! - Block at-rule handling (`@media`, ...)

#![warn(missing_docs)]
#![allow(dead_code)]

pub mod builtins;
pub mod environment;
pub mod visitor;

pub use vass_core::error::{Error, Result};
pub use visitor::evaluate::{EvalOptions, evaluate, evaluate_with};

/// Evaluate a parsed stylesheet into CSS
pub fn evaluate_stylesheet(
  sheet: &vass_core::ast::input::statement::Stylesheet,
) -> Result<vass_core::ast::css::CssStylesheet> {
  evaluate(sheet)
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_parser::{parse, Syntax};

  #[test]
  fn test_evaluate_stylesheet_basic() {
    let sheet = parse(".a { color: red; }", Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
    let css = result.unwrap();
    assert!(!css.nodes.is_empty());
  }

  #[test]
  fn test_evaluate_stylesheet_with_variables() {
    let sheet = parse("$color: red;\n.a { color: $color; }", Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }

  #[test]
  fn test_evaluate_stylesheet_with_nesting() {
    let sheet = parse(".a { .b { color: red; } }", Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }

  #[test]
  fn test_evaluate_stylesheet_empty() {
    let sheet = parse("", Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }
}
