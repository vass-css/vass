//! Parser module
//!
//! Entry points for parsing `.vass` (indented) and `.vcss` (braces) source.

pub mod base;
pub mod braces;
pub mod indented;
pub mod selector;
pub mod stylesheet;

pub use stylesheet::{parse_expr_snippet, parse_interp_text};
pub use vass_core::ast::input::Syntax;
pub use vass_core::ast::input::statement::Stylesheet;
pub use vass_core::error::Result;

/// Parse `source` using `syntax`
pub fn parse(source: &str, syntax: Syntax) -> Result<Stylesheet> {
  match syntax {
    Syntax::Indented => indented::parse(source),
    Syntax::Braces => braces::parse(source),
  }
}

/// Guess the syntax from a file extension
pub fn detect_syntax(path: &std::path::Path) -> Syntax {
  match path.extension().and_then(|e| e.to_str()) {
    Some("vass") => Syntax::Indented,
    _ => Syntax::Braces,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_braces() {
    let result = parse(".a { color: red; }", Syntax::Braces);
    assert!(result.is_ok());
  }

  #[test]
  fn test_parse_indented() {
    let result = parse(".a\n  color: red\n", Syntax::Indented);
    assert!(result.is_ok());
  }

  #[test]
  fn test_detect_syntax_vass() {
    let path = std::path::Path::new("test.vass");
    assert_eq!(detect_syntax(path), Syntax::Indented);
  }

  #[test]
  fn test_detect_syntax_vcss() {
    let path = std::path::Path::new("test.vcss");
    assert_eq!(detect_syntax(path), Syntax::Braces);
  }

  #[test]
  fn test_detect_syntax_unknown() {
    let path = std::path::Path::new("test.css");
    assert_eq!(detect_syntax(path), Syntax::Braces);
  }

  #[test]
  fn test_detect_syntax_no_extension() {
    let path = std::path::Path::new("test");
    assert_eq!(detect_syntax(path), Syntax::Braces);
  }
}
