//! CSS AST (output)
//!
//! The flat tree the evaluator produces; the transpiler serializes it.

/// A parsed-and-evaluated CSS stylesheet
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CssStylesheet {
  /// Top-level nodes
  pub nodes: Vec<CssNode>,
}

/// A single declaration (`property: value`)
#[derive(Debug, Clone, PartialEq)]
pub struct CssDecl {
  /// Property name
  pub prop: String,
  /// Resolved value text
  pub value: String,
}

/// A child of a rule or at-rule block: a declaration or a comment
#[derive(Debug, Clone, PartialEq)]
pub enum CssChild {
  /// `property: value`; the u32 is the source line the declaration ends on
  /// (trailing-comment placement)
  Decl(CssDecl, u32),
  /// A preserved comment; the u32 is the source line the comment starts on
  Comment(String, u32),
}

/// A node in the output CSS
#[derive(Debug, Clone, PartialEq)]
pub enum CssNode {
  /// `selector { decls }`
  Rule {
    /// Comma-joined selector text
    selector: String,
    /// Declarations and comments
    children: Vec<CssChild>,
    /// Source-statement group: the writer blank-lines between root
    /// nodes of different groups only
    group: u32,
    /// Source line of the opening `{` (trailing-comment anchor for the
    /// first emitted child)
    open_line: u32,
    /// Source line of the closing `}`
    close_line: u32,
  },
  /// A block at-rule (`@media ... { ... }`) containing rules
  AtRule {
    /// Header text (e.g. `@media (min-width: 768px)`)
    header: String,
    /// Declarations and comments directly inside the block
    children: Vec<CssChild>,
    /// Nested nodes
    nodes: Vec<CssNode>,
    /// Source-statement group (see `Rule::group`)
    group: u32,
    /// Source line of the opening `{`
    open_line: u32,
    /// Source line of the closing `}`
    close_line: u32,
  },
  /// A preserved comment (the u32 is its source start line)
  Comment(String, u32),
  /// Raw passthrough text (`@import "...";`, `@charset "UTF-8";`); the u32
  /// is the source line it starts on
  Raw(String, u32),
}

impl CssChild {
  /// Convenience constructor for a declaration child (end line unknown)
  pub fn decl(prop: impl Into<String>, value: impl Into<String>) -> CssChild {
    CssChild::Decl(
      CssDecl {
        prop: prop.into(),
        value: value.into(),
      },
      0,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_child_constructors() {
    let c = CssChild::decl("color", "red");
    match c {
      CssChild::Decl(d, _) => {
        assert_eq!(d.prop, "color");
        assert_eq!(d.value, "red");
      }
      other => panic!("unexpected: {:?}", other),
    }
  }
}
