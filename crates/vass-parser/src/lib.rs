//! # VASS Parser
//!
//! Lexer and parser for the VASS CSS preprocessor.
//!
//! ## Supported Syntaxes
//!
//! - `.vass` - Indented syntax (similar to VASS)
//! - `.vcss` - VCSS-style bracket syntax

#![warn(missing_docs)]
#![allow(dead_code)]

pub mod lexer;
pub mod lint;
pub mod parser;

pub use lint::scan_balance;
pub use parser::{detect_syntax, parse, parse_expr_snippet, parse_interp_text};
pub use vass_core::ast::input::Syntax;
pub use vass_core::error::{Error, Result};

/// Parse a stylesheet, guessing the syntax from `path`'s extension
pub fn parse_file(
  source: &str,
  path: &std::path::Path,
) -> Result<vass_core::ast::input::statement::Stylesheet> {
  parse(source, detect_syntax(path))
}

/// Parse a stylesheet written in the indented syntax
pub fn parse_indented(source: &str) -> Result<vass_core::ast::input::statement::Stylesheet> {
  parse(source, Syntax::Indented)
}

/// Parse a stylesheet written in the brace syntax
pub fn parse_braces(source: &str) -> Result<vass_core::ast::input::statement::Stylesheet> {
  parse(source, Syntax::Braces)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_file_vass() {
    let source = ".a\n  color: red\n";
    let path = std::path::Path::new("test.vass");
    let result = parse_file(source, path);
    assert!(result.is_ok());
  }

  #[test]
  fn test_parse_file_vcss() {
    let source = ".a { color: red; }";
    let path = std::path::Path::new("test.vcss");
    let result = parse_file(source, path);
    assert!(result.is_ok());
  }

  #[test]
  fn test_parse_indented() {
    let result = parse_indented(".a\n  color: red\n");
    assert!(result.is_ok());
  }

  #[test]
  fn test_parse_braces() {
    let result = parse_braces(".a { color: red; }");
    assert!(result.is_ok());
  }

  #[test]
  fn test_parse_syntax_error() {
    let result = parse_braces(".a { color: ; }");
    assert!(result.is_err());
  }
}
