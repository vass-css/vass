//! CSS writer: serialize the CSS AST to text

use vass_core::ast::css::{CssChild, CssDecl, CssNode, CssStylesheet};

/// Serialize a stylesheet in expanded (pretty-printed) form.
///
/// Blank-line rule mirrors dart-sass exactly: only a root-level STYLE RULE
/// can end a source-statement group (dart's evaluator marks the last node
/// each root statement produced with `isGroupEnd`), so a blank line is
/// written after a style rule exactly when the following node belongs to a
/// different group. Rules expanded from ONE statement (nesting) share a
/// group and stay glued together; at-rules (`@media`, `@keyframes`,
/// `@font-face`, `@import`, ...) and comments never end a group, so nothing
/// ever follows them with a blank line. A loud comment is written INLINE
/// (after a single space, no line break) exactly when it starts on the
/// source line where the previous emitted sibling ended — this takes
/// precedence over the blank-line rule (dart's `_isTrailingComment` check
/// runs first). A block whose children are exactly one inline comment keeps
/// the single-line form (`a { /* c */ }`).
pub fn write_expanded(sheet: &CssStylesheet) -> String {
  let mut out = String::new();
  let mut prev_end_line: Option<u32> = None;
  // Some(group) iff the previous emitted node was a style rule; it ends
  // its group exactly when the next node carries a different group.
  let mut prev_rule_group: Option<u32> = None;
  // Nodes are written WITHOUT a trailing line break so a trailing root
  // comment can join the previous line (`} /* top2 */`)
  let mut pending_break = false;
  for node in &sheet.nodes {
    match node {
      CssNode::Comment(text, line) if prev_end_line == Some(*line) && !out.is_empty() => {
        // Trailing root comment: joins the previous node's line
        out.push(' ');
        out.push_str(text);
        prev_end_line = Some(comment_end_line(text, *line));
        pending_break = true;
        prev_rule_group = None;
      }
      other => {
        let group = node_group(other);
        if pending_break {
          out.push('\n');
        }
        // dart: previous.isGroupEnd → one extra line feed (blank line)
        if let Some(pg) = prev_rule_group
          && Some(pg) != group
        {
          out.push('\n');
        }
        write_node(other, 0, &mut out);
        prev_end_line = node_end_line(other);
        prev_rule_group = match other {
          CssNode::Rule { group, .. } => Some(*group),
          _ => None,
        };
        pending_break = true;
      }
    }
  }
  if pending_break {
    out.push('\n');
  }
  // The reference compiler writes ZERO bytes for a stylesheet that
  // produces no CSS (debug-only input, silent comments, empty file).
  out
}

/// The source line a root node ends on (for top-level comment placement)
fn node_end_line(node: &CssNode) -> Option<u32> {
  match node {
    CssNode::Rule { close_line, .. } | CssNode::AtRule { close_line, .. } => Some(*close_line),
    CssNode::Comment(text, line) => Some(comment_end_line(text, *line)),
    CssNode::Raw(_, line) => Some(*line),
  }
}

/// The source line a multi-line comment ends on
fn comment_end_line(text: &str, line: u32) -> u32 {
  line + text.matches('\n').count() as u32
}

/// The source-statement group of a structural node (comments/raw: none)
fn node_group(node: &CssNode) -> Option<u32> {
  match node {
    CssNode::Rule { group, .. } | CssNode::AtRule { group, .. } => Some(*group),
    _ => None,
  }
}

/// Serialize a stylesheet minified (no unnecessary whitespace)
pub fn write_compressed(sheet: &CssStylesheet) -> String {
  let mut out = String::new();
  for node in &sheet.nodes {
    write_node_compressed(node, &mut out);
  }
  // Reference compressed output ends with a line feed
  if !out.is_empty() && !out.ends_with('\n') {
    out.push('\n');
  }
  out
}

fn indent(depth: usize, out: &mut String) {
  for _ in 0..depth {
    out.push_str("  ");
  }
}

/// Write block children (declarations + comments) with trailing-comment
/// placement. The caller has written `sel {` WITHOUT a line break; the first
/// child anchors to the `{` line, later children to the previous child's end
/// line. Returns whether a line break is still owed before the block close.
fn write_children(children: &[CssChild], depth: usize, open_line: u32, out: &mut String) -> bool {
  let mut last_line = Some(open_line);
  let mut pending_break = true; // a line break is owed after `{` by default
  for child in children {
    match child {
      CssChild::Decl(d, end_line) => {
        if pending_break {
          out.push('\n');
        }
        indent(depth, out);
        out.push_str(&d.prop);
        out.push_str(": ");
        out.push_str(&d.value);
        out.push(';');
        pending_break = true;
        last_line = Some(*end_line);
      }
      CssChild::Comment(text, line) => {
        if last_line == Some(*line) {
          // Trailing comment: joins the previous child's line (or
          // the `{` line for the first child)
          out.push(' ');
          out.push_str(text);
          // the owed break (if any) is consumed by the join
        } else {
          if pending_break {
            out.push('\n');
          }
          indent(depth, out);
          out.push_str(text);
        }
        pending_break = true;
        last_line = Some(comment_end_line(text, *line));
      }
    }
  }
  pending_break
}

/// Whether a block keeps the single-line form `sel { /* c */ }`: exactly one
/// child, an inline-positioned single-line comment
fn is_single_line_comment_block(children: &[CssChild], open_line: u32) -> bool {
  match children {
    [CssChild::Comment(text, line)] => *line == open_line && !text.contains('\n'),
    _ => false,
  }
}

/// Write one node WITHOUT a trailing line break (the caller owns breaks)
fn write_node(node: &CssNode, depth: usize, out: &mut String) {
  match node {
    CssNode::Rule {
      selector,
      children,
      group: _,
      open_line,
      close_line: _,
    } => {
      indent(depth, out);
      if is_single_line_comment_block(children, *open_line)
        && let [CssChild::Comment(text, _)] = &children[..]
      {
        out.push_str(selector);
        out.push_str(" { ");
        out.push_str(text);
        out.push_str(" }");
        return;
      }
      out.push_str(selector);
      out.push_str(" {");
      let pending = write_children(children, depth + 1, *open_line, out);
      if pending {
        out.push('\n');
      }
      indent(depth, out);
      out.push('}');
    }
    CssNode::AtRule {
      header,
      children,
      nodes,
      group: _,
      open_line,
      close_line: _,
    } => {
      indent(depth, out);
      out.push_str(header);
      out.push_str(" {");
      let mut pending = write_children(children, depth + 1, *open_line, out);
      for n in nodes {
        if pending {
          out.push('\n');
        }
        write_node(n, depth + 1, out);
        pending = true;
      }
      if pending {
        out.push('\n');
      }
      indent(depth, out);
      out.push('}');
    }
    CssNode::Comment(text, _line) => {
      indent(depth, out);
      out.push_str(text);
    }
    CssNode::Raw(text, _line) => {
      indent(depth, out);
      out.push_str(text);
      if !text.ends_with(';') {
        out.push(';');
      }
    }
  }
}

fn write_children_compressed(children: &[CssChild], trailing: bool, out: &mut String) {
  let decls: Vec<&CssDecl> = children
    .iter()
    .filter_map(|c| match c {
      CssChild::Decl(d, _) => Some(d),
      _ => None,
    })
    .collect();
  for (i, d) in decls.iter().enumerate() {
    out.push_str(&d.prop);
    out.push(':');
    out.push_str(&d.value);
    // The last declaration of a block drops its semicolon
    let last = i + 1 == decls.len();
    if !last || trailing {
      out.push(';');
    }
  }
}

fn write_node_compressed(node: &CssNode, out: &mut String) {
  match node {
    CssNode::Rule {
      selector,
      children,
      group: _,
      open_line: _,
      close_line: _,
    } => {
      out.push_str(selector);
      out.push('{');
      write_children_compressed(children, false, out);
      out.push('}');
    }
    CssNode::AtRule {
      header,
      children,
      nodes,
      group: _,
      open_line: _,
      close_line: _,
    } => {
      out.push_str(header);
      out.push('{');
      // Declarations before nested rules keep their semicolons
      let has_nodes = !nodes.is_empty();
      write_children_compressed(children, has_nodes, out);
      for n in nodes {
        write_node_compressed(n, out);
      }
      out.push('}');
    }
    CssNode::Comment(text, _) => {
      // `/*! ... */` comments survive compression; the rest are dropped
      if text.starts_with("/*!") {
        out.push_str(text);
      }
    }
    CssNode::Raw(text, _) => {
      out.push_str(text);
      if !text.ends_with(';') {
        out.push(';');
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::ast::css::{CssDecl, CssStylesheet};

  #[test]
  fn test_expanded() {
    let sheet = CssStylesheet {
      nodes: vec![CssNode::Rule {
        selector: ".a".into(),
        children: vec![CssChild::Decl(
          CssDecl {
            prop: "color".into(),
            value: "red".into(),
          },
          1,
        )],
        group: 0,
        open_line: 1,
        close_line: 1,
      }],
    };
    assert_eq!(write_expanded(&sheet), ".a {\n  color: red;\n}\n");
  }

  #[test]
  fn test_compressed() {
    let sheet = CssStylesheet {
      nodes: vec![CssNode::Rule {
        selector: ".a".into(),
        children: vec![CssChild::Decl(
          CssDecl {
            prop: "color".into(),
            value: "red".into(),
          },
          1,
        )],
        group: 0,
        open_line: 1,
        close_line: 1,
      }],
    };
    // dart-sass --style=compressed ends with a single trailing newline
    assert_eq!(write_compressed(&sheet), ".a{color:red}\n");
  }

  #[test]
  fn test_comments_in_rules() {
    let sheet = CssStylesheet {
      nodes: vec![CssNode::Rule {
        selector: ".a".into(),
        children: vec![
          CssChild::Comment("/* head */".into(), 1),
          CssChild::Decl(
            CssDecl {
              prop: "color".into(),
              value: "red".into(),
            },
            2,
          ),
        ],
        group: 0,
        open_line: 1,
        close_line: 2,
      }],
    };
    let out = write_expanded(&sheet);
    assert!(out.contains("/* head */"));
    // dart-sass --style=compressed ends with a single trailing newline
    assert_eq!(write_compressed(&sheet), ".a{color:red}\n");
  }

  #[test]
  fn test_trailing_comment() {
    // `a {\n  /* c1 */ color: red; /* c2 */\n}` — the reference keeps
    // `/* c2 */` on the declaration line and breaks `/* c1 */` onto its
    // own line
    let sheet = CssStylesheet {
      nodes: vec![CssNode::Rule {
        selector: "a".into(),
        children: vec![
          CssChild::Comment("/* c1 */".into(), 2),
          CssChild::Decl(
            CssDecl {
              prop: "color".into(),
              value: "red".into(),
            },
            2,
          ),
          CssChild::Comment("/* c2 */".into(), 2),
        ],
        group: 0,
        open_line: 1,
        close_line: 3,
      }],
    };
    assert_eq!(
      write_expanded(&sheet),
      "a {\n  /* c1 */\n  color: red; /* c2 */\n}\n"
    );
  }

  #[test]
  fn test_single_line_comment_block() {
    let sheet = CssStylesheet {
      nodes: vec![CssNode::Rule {
        selector: "n".into(),
        children: vec![CssChild::Comment("/* only */".into(), 1)],
        group: 0,
        open_line: 1,
        close_line: 1,
      }],
    };
    assert_eq!(write_expanded(&sheet), "n { /* only */ }\n");
  }

  #[test]
  fn test_nested_at_rule() {
    let sheet = CssStylesheet {
      nodes: vec![CssNode::AtRule {
        header: "@media screen".into(),
        children: vec![],
        group: 0,
        open_line: 1,
        close_line: 3,
        nodes: vec![CssNode::Rule {
          selector: ".a".into(),
          children: vec![CssChild::Decl(
            CssDecl {
              prop: "color".into(),
              value: "red".into(),
            },
            2,
          )],
          group: 0,
          open_line: 1,
          close_line: 2,
        }],
      }],
    };
    let out = write_expanded(&sheet);
    assert!(out.contains("@media screen {\n  .a {"));
  }
}
