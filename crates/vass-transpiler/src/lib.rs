//! # VASS Transpiler
//!
//! CSS output generation for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Expanded and compressed CSS serialization
//! - (Source maps arrive in a later phase)

#![warn(missing_docs)]
#![allow(dead_code)]

pub mod css_writer;
pub mod minifier;
pub mod source_map;

pub use css_writer::{write_compressed, write_expanded};
pub use vass_core::error::{Error, Result};

use vass_core::ast::css::CssStylesheet;

/// Output style selector
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStyle {
  /// Pretty-printed with indentation
  Expanded,
  /// Minified
  Compressed,
}

/// Render a CSS stylesheet as text
pub fn render(sheet: &CssStylesheet, style: OutputStyle) -> String {
  let out = match style {
    OutputStyle::Expanded => write_expanded(sheet),
    OutputStyle::Compressed => crate::minifier::minify(sheet),
  };
  // Non-ASCII output requires a @charset declaration (reference behavior)
  if out.bytes().any(|b| b >= 0x80) && !out.contains("@charset") {
    return format!("@charset \"UTF-8\";\n{}", out);
  }
  out
}

/// Render a CSS stylesheet in expanded form
pub fn render_expanded(sheet: &CssStylesheet) -> String {
  write_expanded(sheet)
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::ast::css::{CssChild, CssNode};

  #[test]
  fn test_render_expanded() {
    let sheet = CssStylesheet {
      nodes: vec![CssNode::Rule {
        selector: ".a".to_string(),
        children: vec![CssChild::decl("color", "red")],
        group: 0,
        open_line: 1,
        close_line: 2,
      }],
    };
    let result = render(&sheet, OutputStyle::Expanded);
    assert!(result.contains(".a"));
    assert!(result.contains("color: red"));
  }

  #[test]
  fn test_render_compressed() {
    let sheet = CssStylesheet {
      nodes: vec![CssNode::Rule {
        selector: ".a".to_string(),
        children: vec![CssChild::decl("color", "red")],
        group: 0,
        open_line: 1,
        close_line: 2,
      }],
    };
    let result = render(&sheet, OutputStyle::Compressed);
    assert_eq!(result, ".a{color:red}\n");
  }

  #[test]
  fn test_render_expanded_fn() {
    let sheet = CssStylesheet {
      nodes: vec![CssNode::Rule {
        selector: ".a".to_string(),
        children: vec![CssChild::decl("color", "red")],
        group: 0,
        open_line: 1,
        close_line: 2,
      }],
    };
    let result = render_expanded(&sheet);
    assert!(result.contains(".a"));
  }

  #[test]
  fn test_output_style_debug() {
    let _ = format!("{:?}", OutputStyle::Expanded);
    let _ = format!("{:?}", OutputStyle::Compressed);
  }

  #[test]
  fn test_output_style_partial_eq() {
    assert_eq!(OutputStyle::Expanded, OutputStyle::Expanded);
    assert_ne!(OutputStyle::Expanded, OutputStyle::Compressed);
  }
}
