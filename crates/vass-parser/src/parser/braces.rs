//! Brace syntax parser (`.vcss`, like VCSS)

use vass_core::ast::input::Syntax;
use vass_core::ast::input::statement::{Statement, Stylesheet};
use vass_core::error::Result;

use crate::lexer::Tok;

use super::base::Parser;

/// Parse a `.vcss` (braces) stylesheet
pub fn parse(source: &str) -> Result<Stylesheet> {
  let mut p = Parser::with_syntax(source, Syntax::Braces)?;
  let body = p.parse_vcss_stmts(/* top */ true)?;
  Ok(Stylesheet {
    body,
    syntax: Syntax::Braces,
  })
}

impl Parser {
  /// Parse statements until EOF (top level) or an unmatched `}`
  pub fn parse_vcss_stmts(&mut self, top: bool) -> Result<Vec<Statement>> {
    let mut stmts = Vec::new();
    // @use/@forward must precede every other statement in a stylesheet
    let mut seen_member = false;
    loop {
      self.skip_trivia();
      match self.peek().kind {
        Tok::Eof => {
          if top {
            break;
          } else {
            return Err(self.error_here("Expected `}` to close block."));
          }
        }
        Tok::RBrace => {
          if top {
            return Err(self.error_here("Unexpected `}`."));
          } else {
            break;
          }
        }
        _ => {}
      }
      let stmt_start = self.peek().clone();
      let stmt = self.parse_vcss_stmt()?;
      match &stmt {
        Statement::Comment { .. } => {}
        Statement::Use(_) | Statement::Forward(_) => {
          if !top {
            // dart-sass: nested `@use`/`@forward` is a hard error
            return Err(self.error_at(&stmt_start, "This at-rule is not allowed here."));
          }
          if seen_member {
            return Err(self.error_at(
              &stmt_start,
              "@use rules must be written before any other rules.",
            ));
          }
        }
        _ => seen_member = true,
      }
      stmts.push(stmt);
    }
    Ok(stmts)
  }

  fn parse_vcss_stmt(&mut self) -> Result<Statement> {
    match self.peek().kind {
      Tok::Var => self.parse_var_decl(),
      Tok::At => {
        let at = self.bump();
        let (kind_at_end, _idx) = self.scan_statement_head();
        let has_block = kind_at_end == Tok::LBrace;
        self.parse_at_stmt(&at, has_block, None)
      }
      _ if self.peek().is_loud_comment() => {
        let t = self.bump();
        self.eat(&Tok::Newline);
        Ok(Statement::Comment {
          text: t.text,
          line: t.line,
        })
      }
      _ => {
        // Look ahead (at depth 0) to decide rule vs declaration:
        // `{` first -> style rule (or nested properties); `;` / `}` /
        // newline first -> declaration.
        let (kind_at_end, idx) = self.scan_statement_head();
        match kind_at_end {
          Tok::LBrace => {
            // Colon at top level before `{`: nested properties
            // (`font: 12px { ... }`) vs a style rule (`a:hover`)
            if let Some(colon_idx) = self.find_top_level_colon(idx)
              && self.head_is_declaration(colon_idx, idx)
            {
              let colon = &self.tokens[colon_idx];
              let prop = self.raw_from(self.peek().start, colon);
              self.pos = colon_idx;
              return self.parse_declaration_from(prop);
            }
            let brace = &self.tokens[idx];
            let selector = self.raw_from(self.peek().start, brace);
            if selector.is_empty() {
              // `{}` with no selector (reference: "expected
              // end of rule.")
              return Err(self.error_here("expected end of rule."));
            }
            self.pos = idx; // jump onto `{`
            self.bump(); // `{`
            let open_line = self.tokens[self.pos - 1].line;
            let body = self.parse_vcss_stmts(false)?;
            self.expect(&Tok::RBrace, "Expected `}`.")?;
            let close_line = self.tokens[self.pos - 1].line;
            Ok(Statement::StyleRule {
              selector,
              body,
              open_line,
              close_line,
            })
          }
          _ => self.parse_vcss_declaration(),
        }
      }
    }
  }

  /// Scan forward at depth 0 until `{`, `;`, `}`, newline, or EOF.
  /// Interpolation braces (`#{`) count toward depth. Returns (kind, index).
  /// A newline is skipped (treated as whitespace) when the next significant
  /// token is `{` — the reference allows a rule's opening brace on its own
  /// line (`.a\n{ v: 1; }`) and multi-line selectors.
  fn scan_statement_head(&self) -> (Tok, usize) {
    let mut depth = 0usize;
    let mut i = self.pos;
    loop {
      let t = &self.tokens[i];
      let prev_hash = i
        .checked_sub(1)
        .map(|p| self.tokens[p].kind == Tok::Hash)
        .unwrap_or(false);
      match t.kind {
        Tok::LParen | Tok::LBracket => depth += 1,
        Tok::RParen | Tok::RBracket => depth = depth.saturating_sub(1),
        Tok::LBrace if depth == 0 && !prev_hash => return (Tok::LBrace, i),
        Tok::LBrace => depth += 1,
        Tok::Semi | Tok::RBrace | Tok::Eof if depth == 0 => return (t.kind.clone(), i),
        // Newlines are plain whitespace inside a statement head in
        // brace syntax (`@media screen,\n  print { ... }`): keep
        // scanning for the `{` / `;` / `}` terminator.
        Tok::RBrace => depth = depth.saturating_sub(1),
        _ => {}
      }
      i += 1;
      if i >= self.tokens.len() {
        return (Tok::Eof, self.tokens.len() - 1);
      }
    }
  }

  /// Find the first top-level `:` between the cursor and `limit_idx`
  fn find_top_level_colon(&self, limit_idx: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = self.pos;
    while i < limit_idx {
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

  fn parse_vcss_declaration(&mut self) -> Result<Statement> {
    // Property may be a plain ident, custom property, or interpolation;
    // capture the raw text before the first top-level colon
    let head_end = self.scan_statement_head().1;
    let colon_idx = self
      .find_top_level_colon(head_end)
      .ok_or_else(|| self.error_here("Expected `:` in property declaration."))?;
    let colon = &self.tokens[colon_idx];
    let prop = self.raw_from(self.peek().start, colon);
    self.pos = colon_idx;
    self.parse_declaration_from(prop)
  }
}

/// Ensure a raw statement ends with `;`
#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::ast::input::expression::Expr;
  use vass_core::ast::input::statement::Statement;

  fn decl_of(stmt: &Statement) -> (&str, &str) {
    match stmt {
      Statement::Declaration {
        prop,
        value: Some(Expr::Str { text, .. }),
        ..
      } => (prop.as_str(), text.as_str()),
      // Bare color keywords parse to typed color literals carrying hex text
      Statement::Declaration {
        prop,
        value: Some(Expr::Color { hex }),
        ..
      } => (prop.as_str(), hex.as_str()),
      _ => panic!("expected declaration, got {:?}", stmt),
    }
  }

  #[test]
  fn test_basic_rule() {
    let sheet = parse(".a { color: red; }").unwrap();
    assert_eq!(sheet.body.len(), 1);
    match &sheet.body[0] {
      Statement::StyleRule { selector, body, .. } => {
        assert_eq!(selector, ".a");
        assert_eq!(body.len(), 1);
        assert_eq!(decl_of(&body[0]), ("color", "red"));
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_variable_decl() {
    let sheet = parse("$x: 10px;\n.a { width: $x; }").unwrap();
    assert!(matches!(sheet.body[0], Statement::VariableDecl { .. }));
  }

  // `.vcss` is SCSS-strict: a missing `;` is an error (E0013) carrying a
  // one-keystroke quick fix — except right before the closing `}`.
  #[test]
  fn test_missing_semicolon_is_error() {
    let err = parse("$x: 10px\n.a { width: $x; }").unwrap_err();
    assert_eq!(err.code_or_default(), "E0013");
    assert!(err.to_string().contains(";"));
    match err.fix().map(|f| &f.kind) {
      Some(vass_core::error::FixKind::Insert { line, text, .. }) => {
        assert_eq!(*line, 1);
        assert_eq!(text.as_str(), ";");
      }
      other => panic!("expected Insert fix, got {other:?}"),
    }
  }

  #[test]
  fn test_missing_semicolon_between_declarations() {
    // `color: red` followed by another declaration: needs `;`
    assert!(parse(".a { color: red\n  padding: 10px; }").is_err());
    // ...while the same text with the `;` parses to two declarations
    let sheet = parse(".a { color: red;\n  padding: 10px; }").unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { body, .. } => assert_eq!(body.len(), 2),
      other => panic!("unexpected: {other:?}"),
    }
  }

  #[test]
  fn test_semi_optional_before_closing_brace() {
    let sheet = parse(".a { width: $x }\n.b { color: red }").unwrap();
    assert_eq!(sheet.body.len(), 2);
  }

  #[test]
  fn test_selector_with_pseudo_colon() {
    let sheet = parse("a:hover { color: blue; }").unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { selector, .. } => assert_eq!(selector, "a:hover"),
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_custom_property() {
    let sheet = parse(".a { --main-color: #123; }").unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { body, .. } => match &body[0] {
        Statement::Declaration { prop, custom, .. } => {
          assert_eq!(prop, "--main-color");
          assert!(*custom);
        }
        other => panic!("unexpected: {:?}", other),
      },
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_at_rule_statement() {
    let sheet = parse("@import \"other.vass\";").unwrap();
    assert!(matches!(sheet.body[0], Statement::Import(_)));
  }

  #[test]
  fn test_media_block() {
    let sheet = parse("@media (min-width: 100px) { .a { color: red; } }").unwrap();
    match &sheet.body[0] {
      Statement::AtRule { name, body, .. } => {
        assert_eq!(name, "media");
        assert_eq!(body.len(), 1);
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_nested_properties() {
    let sheet = parse(".a { font: 12px { family: serif; } }").unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { body, .. } => match &body[0] {
        Statement::Declaration { prop, children, .. } => {
          assert_eq!(prop, "font");
          assert_eq!(children.len(), 1);
        }
        other => panic!("unexpected: {:?}", other),
      },
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_mixin_parse() {
    let sheet = parse("@mixin flex($dir: row) { display: flex; }").unwrap();
    match &sheet.body[0] {
      Statement::MixinDecl(decl) => {
        assert_eq!(decl.name, "flex");
        assert_eq!(decl.params.len(), 1);
        assert_eq!(decl.body.len(), 1);
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_if_else_parse() {
    let sheet = parse("@if $a { .x { a: b } } @else { .y { a: b } }").unwrap();
    match &sheet.body[0] {
      Statement::If(chain) => {
        assert_eq!(chain.clauses.len(), 1);
        assert!(chain.else_body.is_some());
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_use_parse() {
    let sheet = parse("@use \"vass:math\" as m;").unwrap();
    match &sheet.body[0] {
      Statement::Use(rule) => {
        assert_eq!(rule.url, "vass:math");
        assert_eq!(rule.namespace.as_deref(), Some("m"));
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_selector_interpolation() {
    let sheet = parse("#{$name} { color: red; }").unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { selector, .. } => assert_eq!(selector, "#{$name}"),
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_keyframes() {
    let sheet = parse("@keyframes fade { from { opacity: 0; } to { opacity: 1; } }").unwrap();
    match &sheet.body[0] {
      Statement::AtRule { name, body, .. } => {
        assert_eq!(name, "keyframes");
        assert_eq!(body.len(), 2);
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  // --- multi-line call args / params (dart-sass parity) -------------------

  #[test]
  fn test_multiline_call_args() {
    // First argument starts on its own line — must parse like dart-sass.
    let sheet =
      parse("@debug color.channel(\n  hsl(80deg 30% 50%),\n  \"hue\",\n  $space: oklch\n);")
        .unwrap();
    assert_eq!(sheet.body.len(), 1);
  }

  #[test]
  fn test_multiline_mixin_params_and_include() {
    let sheet = parse(
            "@mixin foo(\n  $x,\n  $y: 2px\n) {\n  margin: $x $y;\n}\n.btn {\n  @include foo(\n    1px\n  );\n}",
        )
        .unwrap();
    assert!(
      matches!(sheet.body[0], Statement::MixinDecl(_)),
      "mixin decl expected"
    );
    assert!(
      matches!(sheet.body[1], Statement::StyleRule { .. }),
      "style rule expected"
    );
  }

  #[test]
  fn test_multiline_property_value() {
    let sheet = parse(".box {\n  color: rgb(\n    10,\n    20,\n    30\n  );\n}").unwrap();
    match &sheet.body[0] {
      Statement::StyleRule { body, .. } => assert_eq!(body.len(), 1),
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_multiline_map_literal() {
    let sheet = parse("$m: (\n  a: 1,\n  b: 2,\n);").unwrap();
    assert!(matches!(sheet.body[0], Statement::VariableDecl { .. }));
  }
}
