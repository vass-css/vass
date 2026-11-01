//! Indented syntax parser (`.vass`, like VASS)

use vass_core::ast::input::Syntax;
use vass_core::ast::input::statement::{Statement, Stylesheet};
use vass_core::error::Result;

use crate::lexer::Tok;

use super::base::Parser;

/// Parse a `.vass` (indented) stylesheet
pub fn parse(source: &str) -> Result<Stylesheet> {
  let mut p = Parser::with_syntax(source, Syntax::Indented)?;
  let body = p.parse_vass_block(0)?;
  Ok(Stylesheet {
    body,
    syntax: Syntax::Indented,
  })
}

impl Parser {
  /// Parse a block whose statements sit at exactly `min_indent`
  pub fn parse_vass_block(&mut self, min_indent: u32) -> Result<Vec<Statement>> {
    let mut stmts = Vec::new();
    loop {
      self.skip_trivia();
      if self.peek().kind == Tok::Eof {
        break;
      }
      let ind = self.peek().indent;
      if ind < min_indent {
        break;
      }
      if ind > min_indent {
        return Err(self.error_here("Unexpected increased indentation."));
      }
      stmts.push(self.parse_vass_stmt(ind)?);
      // a loud comment trailing a declaration line on the same source
      // line belongs to this block (mirrors brace-syntax structure)
      if let Some((text, line)) = self.pending_comment.take() {
        stmts.push(Statement::Comment { text, line });
      }
    }
    Ok(stmts)
  }

  fn parse_vass_stmt(&mut self, indent: u32) -> Result<Statement> {
    match self.peek().kind {
      Tok::Var => self.parse_var_decl(),
      Tok::At => self.parse_vass_at_stmt(indent),
      // `=name` — mixin declaration shortcut (indented syntax)
      Tok::Eq => self.parse_vass_mixin_shortcut(indent),
      // `+name` — include shortcut; `+ name` (gap) is a selector
      Tok::Plus => self.parse_vass_plus_stmt(indent),
      _ if self.peek().is_loud_comment() => {
        let t = self.bump();
        self.eat(&Tok::Newline);
        Ok(Statement::Comment {
          text: t.text,
          line: t.line,
        })
      }
      _ => self.parse_vass_line_stmt(indent),
    }
  }

  /// `=name(params)` — Sass-style mixin declaration shortcut
  fn parse_vass_mixin_shortcut(&mut self, indent: u32) -> Result<Statement> {
    let eq = self.bump(); // `=`
    if self.peek().kind != Tok::Ident || self.peek().start != eq.end {
      return Err(self.error_here("Expected a mixin name after `=`."));
    }
    let (_, idx) = self.line_end_offset();
    let child_indent = self.indent_after(idx);
    let has_block = child_indent.is_some_and(|ci| ci > indent);
    let block_indent = child_indent.filter(|ci| *ci > indent);
    self.parse_callable_decl("mixin".to_string(), true, has_block, block_indent)
  }

  /// `+name(args)` — Sass-style include shortcut. `+ name` (with a
  /// whitespace gap) is an adjacent-sibling selector, not an include.
  fn parse_vass_plus_stmt(&mut self, indent: u32) -> Result<Statement> {
    let plus = self.peek().clone();
    let next = self.peek_n(1).clone();
    if next.kind == Tok::Ident && next.start == plus.end {
      self.bump(); // `+`
      let (_, idx) = self.line_end_offset();
      let child_indent = self.indent_after(idx);
      let has_block = child_indent.is_some_and(|ci| ci > indent);
      let block_indent = child_indent.filter(|ci| *ci > indent);
      return self.parse_include(has_block, block_indent);
    }
    self.parse_vass_line_stmt(indent)
  }

  /// Indent of the next content line after `after_idx` (skipping blank
  /// lines and silent comments), or `None` at EOF
  fn indent_after(&self, after_idx: usize) -> Option<u32> {
    let mut i = after_idx;
    loop {
      let t = self.tokens.get(i)?;
      match t.kind {
        Tok::Newline => i += 1,
        _ if t.is_silent_comment() => {
          // a comment line is block content: a rule whose body
          // holds only comments still parses as a rule
          return Some(t.indent);
        }
        Tok::Eof => return None,
        _ => return Some(t.indent),
      }
    }
  }

  /// Offset where the current line ends (start of the next Newline /
  /// silent comment / EOF token), plus that token's index
  pub(crate) fn line_end_offset(&self) -> (usize, usize) {
    let mut i = self.pos;
    loop {
      let t = &self.tokens[i];
      match t.kind {
        Tok::Newline | Tok::Eof => return (t.start, i),
        _ if t.is_silent_comment() => return (t.start, i),
        _ => i += 1,
      }
    }
  }

  /// Advance the cursor to just past the end of the current line
  fn consume_rest_of_line(&mut self) {
    let (_, idx) = self.line_end_offset();
    self.pos = idx;
    self.eat(&Tok::Newline);
  }

  fn parse_vass_at_stmt(&mut self, indent: u32) -> Result<Statement> {
    let at = self.bump(); // At
    let (_, idx) = self.line_end_offset();
    let child_indent = self.indent_after(idx);
    let has_block = child_indent.is_some_and(|ci| ci > indent);
    let block_indent = child_indent.filter(|ci| *ci > indent);
    self.parse_at_stmt(&at, has_block, block_indent)
  }

  /// Reconstruct text from tokens in [start_idx, end_idx), collapsing
  /// spaces that appear only because #{...} interpolation ends with `}`.
  /// This ensures `.#{$c} -zone` becomes `.#{$c}-zone` (no spurious space).
  fn line_text_from_tokens(&self, start_idx: usize, end_idx: usize) -> String {
    let mut result = String::new();
    let mut prev_ends_brace = false;
    for i in start_idx..end_idx {
      let t = &self.tokens[i];
      let text = &self.src[t.start..t.end];
      if !result.is_empty() && !prev_ends_brace && t.ws_before {
        result.push(' ');
      }
      result.push_str(text);
      prev_ends_brace = text.ends_with('}');
    }
    result.trim().to_string()
  }

  fn parse_vass_line_stmt(&mut self, indent: u32) -> Result<Statement> {
    let (_, idx) = self.line_end_offset();
    let line_text = self.line_text_from_tokens(self.pos, idx);
    let child_indent = self.indent_after(idx);

    if child_indent.is_some_and(|ci| ci > indent) {
      // Nested properties (`font: 12px` + children) vs a style rule
      // (`a:hover` + children): a top-level colon whose head looks
      // like a property name means nested properties
      if let Some(colon_idx) = self.find_line_colon(idx)
        && self.head_is_declaration(colon_idx, idx)
      {
        let prop = self.line_text_from_tokens(self.pos, colon_idx);
        self.pos = colon_idx;
        return self.parse_vass_declaration(prop, indent, idx);
      }
      // A style rule
      self.pos = idx;
      self.eat(&Tok::Newline);
      let ci = child_indent.unwrap();
      let open_line = self.tokens[idx].line;
      let body = self.parse_vass_block(ci)?;
      let close_line = self.tokens[self.pos.saturating_sub(1)].line;
      Ok(Statement::StyleRule {
        selector: line_text,
        body,
        open_line,
        close_line,
      })
    } else {
      // A declaration: `prop: value` (prop may be interpolated or a
      // custom property); find the first top-level colon on the line
      let colon_idx = self
        .find_line_colon(idx)
        .ok_or_else(|| self.error_here("Expected `property: value` or an indented block."))?;
      let prop = self.line_text_from_tokens(self.pos, colon_idx);
      self.pos = colon_idx;
      self.parse_vass_declaration(prop, indent, idx)
    }
  }

  /// Find the first top-level `:` on the current line (up to `line_idx`)
  fn find_line_colon(&self, line_idx: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = self.pos;
    while i < line_idx {
      let t = &self.tokens[i];
      match t.kind {
        Tok::LParen | Tok::LBrace | Tok::LBracket => depth += 1,
        Tok::RParen | Tok::RBrace | Tok::RBracket => depth -= 1,
        Tok::Colon if depth == 0 => return Some(i),
        _ => {}
      }
      i += 1;
    }
    None
  }

  /// Parse an indented-syntax declaration with the cursor on the `:`.
  /// `line_idx` is the line-end (newline/EOF) token index.
  fn parse_vass_declaration(
    &mut self,
    prop: String,
    indent: u32,
    line_idx: usize,
  ) -> Result<Statement> {
    self.bump(); // `:`

    // Custom property: raw value (interpolation evaluated later)
    if prop.starts_with("--") {
      let raw = self.capture_custom_value()?;
      self.consume_rest_of_line();
      let end_line = self.tokens[self.pos.saturating_sub(1)].line;
      return Ok(Statement::Declaration {
        prop,
        value: None,
        important: false,
        children: Vec::new(),
        custom: true,
        custom_raw: Some(raw),
        end_line,
      });
    }

    // Optional base value
    let (value, important) = if !self.at(&Tok::Newline) && !self.at(&Tok::Eof) {
      let v = self.parse_value_expr()?;
      let mut important = false;
      if self.peek().text == "!important" && matches!(self.peek().kind, Tok::Ident) {
        self.bump();
        important = true;
      }
      // A loud comment trailing the value on the same line becomes its
      // own statement (mirrors the brace syntax's structure)
      if self.peek().is_loud_comment() {
        let t = self.bump();
        self.pending_comment = Some((t.text, t.line));
      }
      (Some(v), important)
    } else {
      (None, false)
    };

    // Nested property children (indented block), if any
    let child_indent = self.indent_after(line_idx);
    let children = if child_indent.is_some_and(|ci| ci > indent) {
      self.pos = line_idx;
      self.eat(&Tok::Newline);
      let ci = child_indent.unwrap();
      self.parse_vass_block(ci)?
    } else {
      self.pos = line_idx;
      self.eat(&Tok::Newline);
      Vec::new()
    };

    let end_line = self.tokens[self.pos.saturating_sub(1)].line;
    Ok(Statement::Declaration {
      prop,
      value,
      important,
      children,
      custom: false,
      custom_raw: None,
      end_line,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::ast::input::statement::Statement;

  #[test]
  fn test_basic_indented() {
    let src = ".button\n  background: red\n  padding: 1px\n";
    let sheet = parse(src).unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { selector, body, .. } => {
        assert_eq!(selector, ".button");
        assert_eq!(body.len(), 2);
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_nesting() {
    let src = ".a\n  color: red\n  &:hover\n    color: blue\n";
    let sheet = parse(src).unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { body, .. } => {
        assert_eq!(body.len(), 2);
        match &body[1] {
          Statement::StyleRule { selector, .. } => assert_eq!(selector, "&:hover"),
          other => panic!("unexpected: {:?}", other),
        }
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_variables() {
    let src = "$x: 10px\n.a\n  width: $x\n";
    let sheet = parse(src).unwrap();
    assert!(matches!(sheet.body[0], Statement::VariableDecl { .. }));
  }

  #[test]
  fn test_media() {
    let src = "@media screen\n  .a\n    color: red\n";
    let sheet = parse(src).unwrap();
    match &sheet.body[0] {
      Statement::AtRule { name, body, .. } => {
        assert_eq!(name, "media");
        assert_eq!(body.len(), 1);
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_import_statement() {
    let src = "@import \"other.vass\"\n.a\n  color: red\n";
    let sheet = parse(src).unwrap();
    assert!(matches!(sheet.body[0], Statement::Import(_)));
  }

  #[test]
  fn test_silent_comment_ignored() {
    let src = "// a comment\n.a\n  color: red\n";
    let sheet = parse(src).unwrap();
    assert_eq!(sheet.body.len(), 1);
  }

  #[test]
  fn test_mixin_and_include() {
    let src = "@mixin flex\n  display: flex\n.a\n  @include flex\n";
    let sheet = parse(src).unwrap();
    assert!(matches!(sheet.body[0], Statement::MixinDecl(_)));
    match &sheet.body[1] {
      Statement::StyleRule { body, .. } => {
        assert!(matches!(body[0], Statement::Include { .. }));
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_mixin_shortcut_eq() {
    let src = "=card($pad: 4px)\n  padding: $pad\n.a\n  +card(8px)\n";
    let sheet = parse(src).unwrap();
    match &sheet.body[0] {
      Statement::MixinDecl(d) => {
        assert_eq!(d.name, "card");
        assert_eq!(d.params.len(), 1);
      }
      other => panic!("unexpected: {:?}", other),
    }
    match &sheet.body[1] {
      Statement::StyleRule { body, .. } => match &body[0] {
        Statement::Include { name, args, .. } => {
          assert_eq!(name, "card");
          assert_eq!(args.len(), 1);
        }
        other => panic!("unexpected: {:?}", other),
      },
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_plus_gap_is_selector_not_include() {
    let src = ".a\n  + p\n    color: red\n";
    let sheet = parse(src).unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { body, .. } => match &body[0] {
        Statement::StyleRule { selector, .. } => assert_eq!(selector, "+ p"),
        other => panic!("expected sibling selector, got {:?}", other),
      },
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_include_shortcut_no_parens() {
    let src = "=flex\n  display: flex\n.a\n  +flex\n";
    let sheet = parse(src).unwrap();
    assert!(matches!(sheet.body[0], Statement::MixinDecl(_)));
    match &sheet.body[1] {
      Statement::StyleRule { body, .. } => match &body[0] {
        Statement::Include { name, args, .. } => {
          assert_eq!(name, "flex");
          assert!(args.is_empty());
        }
        other => panic!("unexpected: {:?}", other),
      },
      other => panic!("unexpected: {:?}", other),
    }
  }
}
