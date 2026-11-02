//! Source map generation
//!
//! Placeholder for this phase; full source map support arrives later.

use vass_core::ast::css::CssStylesheet;

/// Generate a source map for the given stylesheet (not yet implemented)
pub fn generate(_sheet: &CssStylesheet) -> Option<String> {
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_generate_returns_none() {
    let sheet = CssStylesheet { nodes: vec![] };
    assert_eq!(generate(&sheet), None);
  }
}
