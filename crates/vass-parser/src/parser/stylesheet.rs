//! Shared statement and expression parsing used by both syntax parsers
//!
//! Expression precedence (low → high):
//! comma list → space list → `or` → `and` → `not` → `==`/`!=` →
//! `<`/`>`/`<=`/`>=` → `+`/`-` → `*`/`/`/`%` → unary → primary

use vass_core::ast::input::Syntax;
use vass_core::ast::input::expression::{BinaryOp, CallArg, Expr, Param, StrPart, UnaryOp};
use vass_core::ast::input::statement::{CallableDecl, IfChain, ImportTarget, Statement, UseRule};
use vass_core::error::{Error, FixKind, Result};
use vass_core::token::{Position, Span};
use vass_core::value::Separator;

use crate::lexer::Tok;

use super::base::{Parser, span_of};

/// Raw-passthrough CSS functions (contents are not parsed as VASS
/// expressions; only `#{}` interpolation is evaluated).
/// This matches the reference compiler's special-function set: `url()`,
/// `element()` and `expression()` keep their contents verbatim, while
/// `calc()`/`clamp()` keep calc-expression text (the reference simplifies
/// pure-numeric calc; VASS passes it through). Every other function's
/// arguments are parsed as real expressions and re-serialized, so numbers
/// normalize (`cubic-bezier(.4)` -> `cubic-bezier(0.4)`).
const RAW_CSS_FNS: &[&str] = &["url", "element", "expression", "calc", "clamp"];

impl Parser {
  /// Whether the current token can begin a value expression item
  pub fn at_value_start(&self) -> bool {
    let t = self.peek();
    match t.kind {
      Tok::Number | Tok::Str | Tok::Color | Tok::Var | Tok::LParen | Tok::LBracket | Tok::Hash => {
        true
      }
      Tok::Minus => false,
      Tok::Ident => !t.is_flag(),
      _ => false,
    }
  }

  /// Parse a full value: one or more space lists joined by commas
  /// (e.g. `1px solid #fff`, `Arial, sans-serif`)
  pub fn parse_value_expr(&mut self) -> Result<Expr> {
    let mut items: Vec<Expr> = vec![self.parse_space_list()?];
    let mut trailing = false;
    while self.at(&Tok::Comma) {
      self.bump();
      self.skip_loud_comments();
      self.try_cross_newline_for_value();
      if !self.at_value_start() {
        trailing = true;
        break; // trailing comma
      }
      items.push(self.parse_space_list()?);
    }
    if items.len() == 1 {
      if trailing {
        // A trailing comma makes a one-element comma list even at
        // statement level (`$x: red,;` -> `(red,)`), matching the
        // reference parser
        return Ok(Expr::List {
          items,
          separator: Separator::Comma,
          bracketed: false,
        });
      }
      Ok(items.pop().unwrap())
    } else {
      Ok(Expr::List {
        items,
        separator: Separator::Comma,
        bracketed: false,
      })
    }
  }

  /// In brace syntax a value may continue on the next line (`\n` is plain
  /// whitespace). Try to skip newlines/loud comments to reach the
  /// continuation; restores the position when the next token instead starts
  /// a new statement (`$var:`, `@rule`, `prop:`), so a forgotten `;` keeps
  /// its lenient statement-boundary recovery. Returns whether it crossed.
  fn try_cross_newline_for_value(&mut self) -> bool {
    if self.syntax != Syntax::Braces {
      return false;
    }
    let before = self.pos;
    self.skip_newlines();
    if self.pos == before {
      return false;
    }
    self.skip_loud_comments();
    if self.newline_starts_statement() {
      self.pos = before;
      return false;
    }
    true
  }

  /// After crossing a newline inside a value: does the next token begin a
  /// new statement rather than continue the value? (`$var:`, `@rule`,
  /// `prop:`, or a selector head like `.class` / `element:pseudo`)
  fn newline_starts_statement(&self) -> bool {
    match self.peek().kind {
      Tok::Var | Tok::At | Tok::Period => true,
      Tok::Ident => self
        .tokens
        .get(self.pos + 1)
        .is_some_and(|t| t.kind == Tok::Colon),
      _ => false,
    }
  }

  /// Skip trivia after a binary operator so the right operand may sit on
  /// the next line (brace syntax; the reference treats `\n` as whitespace)
  fn skip_trivia_after_op(&mut self) {
    self.skip_loud_comments();
    if self.syntax == Syntax::Braces {
      self.skip_newlines();
      self.skip_loud_comments();
    }
  }

  /// A space-separated list of logical expressions
  fn parse_space_list(&mut self) -> Result<Expr> {
    self.skip_loud_comments();
    self.try_cross_newline_for_value();
    let mut items: Vec<Expr> = vec![self.parse_or()?];
    loop {
      self.skip_loud_comments();
      self.try_cross_newline_for_value();
      if !self.at_value_start() {
        break;
      }
      items.push(self.parse_or()?);
    }
    if items.len() == 1 {
      Ok(items.pop().unwrap())
    } else {
      Ok(Expr::List {
        items,
        separator: Separator::Space,
        bracketed: false,
      })
    }
  }

  fn parse_or(&mut self) -> Result<Expr> {
    let mut left = self.parse_and()?;
    loop {
      self.skip_loud_comments();
      self.try_cross_newline_for_value();
      if !(matches!(self.peek().kind, Tok::Ident) && self.peek().text == "or") {
        break;
      }
      self.bump();
      self.skip_trivia_after_op();
      let right = self.parse_and()?;
      left = Expr::Binary {
        op: BinaryOp::Or,
        left: Box::new(left),
        right: Box::new(right),
        allow_slash_div: false,
      };
    }
    Ok(left)
  }

  fn parse_and(&mut self) -> Result<Expr> {
    let mut left = self.parse_not()?;
    loop {
      self.skip_loud_comments();
      self.try_cross_newline_for_value();
      if !(matches!(self.peek().kind, Tok::Ident) && self.peek().text == "and") {
        break;
      }
      self.bump();
      self.skip_trivia_after_op();
      let right = self.parse_not()?;
      left = Expr::Binary {
        op: BinaryOp::And,
        left: Box::new(left),
        right: Box::new(right),
        allow_slash_div: false,
      };
    }
    Ok(left)
  }

  fn parse_not(&mut self) -> Result<Expr> {
    if matches!(self.peek().kind, Tok::Ident) && self.peek().text == "not" {
      self.bump();
      self.skip_trivia_after_op();
      let expr = self.parse_not()?;
      return Ok(Expr::Unary {
        op: UnaryOp::Not,
        expr: Box::new(expr),
      });
    }
    self.parse_comparison()
  }

  fn parse_comparison(&mut self) -> Result<Expr> {
    let mut left = self.parse_add()?;
    loop {
      let op = match self.peek().kind {
        Tok::EqEq => BinaryOp::Eq,
        Tok::Ne => BinaryOp::Ne,
        Tok::Less => BinaryOp::Lt,
        Tok::Greater => BinaryOp::Gt,
        Tok::Le => BinaryOp::Le,
        Tok::Ge => BinaryOp::Ge,
        _ => break,
      };
      self.skip_loud_comments();
      self.try_cross_newline_for_value();
      self.bump();
      self.skip_trivia_after_op();
      let right = self.parse_add()?;
      left = Expr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
        allow_slash_div: false,
      };
    }
    Ok(left)
  }

  fn parse_add(&mut self) -> Result<Expr> {
    let mut left = self.parse_mul()?;
    loop {
      self.skip_loud_comments();
      self.try_cross_newline_for_value();
      let op = match self.peek().kind {
        Tok::Plus => BinaryOp::Add,
        Tok::Minus => BinaryOp::Sub,
        _ => break,
      };
      self.bump();
      self.skip_trivia_after_op();
      let right = self.parse_mul()?;
      left = Expr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
        allow_slash_div: false,
      };
    }
    Ok(left)
  }

  fn parse_mul(&mut self) -> Result<Expr> {
    let mut left = self.parse_unary()?;
    loop {
      self.skip_loud_comments();
      self.try_cross_newline_for_value();
      let op = match self.peek().kind {
        Tok::Star => BinaryOp::Mul,
        Tok::Slash => BinaryOp::Div,
        Tok::Percent => BinaryOp::Mod,
        _ => break,
      };
      self.bump();
      self.skip_trivia_after_op();
      let right = self.parse_unary()?;
      // `/` is only division in "operator context": inside parens, or
      // when either side is not a plain literal (per VASS slash-div
      // semantics: `$a/2` divides, `12px/30px` stays a separator)
      let mut allow_slash_div = false;
      if op == BinaryOp::Div {
        let is_literal = |e: &Expr| {
          matches!(e, Expr::Number { .. } | Expr::Color { .. })
            || matches!(e, Expr::Str { quoted: false, .. })
        };
        allow_slash_div = !(is_literal(&left) && is_literal(&right));
      }
      left = Expr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
        allow_slash_div,
      };
    }
    Ok(left)
  }

  fn parse_unary(&mut self) -> Result<Expr> {
    match self.peek().kind {
      Tok::Minus => {
        self.bump();
        self.skip_trivia_after_op();
        let expr = self.parse_unary()?;
        Ok(Expr::Unary {
          op: UnaryOp::Neg,
          expr: Box::new(expr),
        })
      }
      Tok::Plus => {
        self.bump();
        self.skip_trivia_after_op();
        self.parse_unary()
      }
      _ => self.parse_primary(),
    }
  }

  fn parse_primary(&mut self) -> Result<Expr> {
    let tok = self.bump();
    match tok.kind {
      Tok::Number => Ok(parse_number_token(&tok.text)),
      Tok::Color => Ok(Expr::Color {
        hex: tok.text.clone(),
      }),
      Tok::Str => Ok(string_to_interp(&tok.text, true)?),
      Tok::Var => {
        // Namespaced variable: `module.$name`
        if self.at(&Tok::Period) && self.peek_n(1).kind == Tok::Ident {
          self.bump(); // .
          let name = self.bump(); // ident
          return Ok(Expr::ModuleVar {
            namespace: tok.text.clone(),
            name: name.text.clone(),
            span: span_of(&tok),
          });
        }
        Ok(Expr::Variable {
          name: tok.text.clone(),
          span: span_of(&tok),
        })
      }
      Tok::LParen => self.parse_paren(&tok),
      Tok::LBracket => self.parse_bracket_list(&tok),
      Tok::Hash => {
        // `##{...}`: a literal `#` glued to interpolation — the
        // reference compiler emits `#<value>` (e.g. `url(##{$id})`,
        // `##{$n}` inside content). Consume the leading `#` as text.
        let mut prefix_text = String::new();
        if !self.at(&Tok::LBrace) && self.at(&Tok::Hash) && self.peek_n(1).kind == Tok::LBrace {
          self.bump(); // second `#` (first is `tok`, literal text)
          prefix_text.push('#');
        }
        // Interpolation `#{...}` (adjacent text attaches: `#{$n}0px`)
        if self.at(&Tok::LBrace) {
          self.bump();
          let inner = self.parse_value_expr()?;
          self.expect(&Tok::RBrace, "Expected `}` to close interpolation.")?;
          let mut parts: Vec<StrPart> = Vec::new();
          if !prefix_text.is_empty() {
            parts.push(StrPart::Text(prefix_text));
          }
          parts.push(StrPart::Expr(Box::new(inner)));
          self.attach_adjacent_text(&mut parts)?;
          if parts.len() == 1
            && let StrPart::Expr(e) = parts.pop().unwrap()
          {
            return Ok(Expr::InterpValue { expr: e });
          }
          Ok(Expr::Interp {
            parts,
            quoted: false,
          })
        } else {
          Err(self.error_at(&tok, "Expected a hex color after `#`."))
        }
      }
      Tok::Ident => {
        if tok.is_flag() {
          return Err(self.error_at(&tok, "Unexpected flag in value."));
        }
        // Value keywords
        match tok.text.as_str() {
          "null" => return Ok(Expr::Null),
          "true" => return Ok(Expr::Bool(true)),
          "false" => return Ok(Expr::Bool(false)),
          _ => {}
        }
        // Namespaced variable: `module.$name`
        if self.at(&Tok::Period) && self.peek().start == tok.end && self.peek_n(1).kind == Tok::Var
        {
          self.bump(); // .
          let var = self.bump(); // $name
          return Ok(Expr::ModuleVar {
            namespace: tok.text.clone(),
            name: var.text,
            span: span_of(&tok),
          });
        }
        // Namespaced call: `math.div(...)` / `list.append(...)`
        if self.at(&Tok::Period)
          && self.peek().start == tok.end
          && self.peek_n(1).kind == Tok::Ident
          && self.peek_n(2).kind == Tok::LParen
          && self.peek_n(2).start == self.peek_n(1).end
        {
          self.bump(); // .
          let member = self.bump(); // ident
          let full = format!("{}.{}", tok.text, member.text);
          let args = self.parse_call_args_color_aware(&tok.text)?;
          return Ok(Expr::Call {
            name: full,
            args,
            plain_css: false,
            span: span_of(&tok),
          });
        }
        // Ident immediately followed by interpolation: `a-#{$b}`
        if self.at(&Tok::Hash) && self.peek().start == tok.end && self.peek_n(1).kind == Tok::LBrace
        {
          let mut parts: Vec<StrPart> = vec![StrPart::Text(tok.text.clone())];
          self.attach_adjacent_text(&mut parts)?;
          return Ok(Expr::Interp {
            parts,
            quoted: false,
          });
        }
        // Function call with no space before `(`: real call
        if self.at(&Tok::LParen) && self.peek().start == tok.end {
          let name = tok.text.clone();
          // Raw passthrough for known plain-CSS functions
          if RAW_CSS_FNS.iter().any(|f| f.eq_ignore_ascii_case(&name)) {
            return self.capture_raw_fn(&name, tok.start);
          }
          let args = self.parse_call_args_color_aware(&name)?;
          return Ok(Expr::Call {
            name,
            args,
            plain_css: false,
            span: span_of(&tok),
          });
        }
        // Named colors become color literals (`red`, `transparent`, ...)
        // — the raw keyword text is preserved for verbatim output
        if vass_core::value::Color::from_named(&tok.text).is_some() {
          return Ok(Expr::Color {
            hex: tok.text.clone(),
          });
        }
        Ok(Expr::Str {
          text: decode_css_escapes(&tok.text),
          quoted: false,
        })
      }
      _ => Err(self.expected_value_error(&tok)),
    }
  }

  /// Rich "Expected a value." error: names the offending token and adds a
  /// hint for the most common real-world causes (smart quotes pasted from
  /// Word/web, JS template literals). Keeps the original sentence so
  /// existing tooling that matches on `Expected a value.` keeps working.
  fn expected_value_error(&self, tok: &crate::lexer::Token) -> Error {
    let text = tok.text.trim();
    let mut msg = match tok.kind {
      Tok::Eof => "Expected a value, found end of file.".to_string(),
      Tok::Newline => "Expected a value, found a newline.".to_string(),
      _ => format!("Expected a value, found `{}`.", text),
    };
    if text.contains('\u{201c}')
      || text.contains('\u{201d}')
      || text.contains('\u{2018}')
      || text.contains('\u{2019}')
    {
      msg.push_str(
                " (smart quotes `\u{201c}\u{201d}` aren't valid here — replace them with straight quotes `\"`)",
            );
    } else if text.contains("${") {
      msg.push_str(" (JS template literals aren't valid here — VASS interpolates with `#{...}`)");
    }
    self.error_at(tok, msg.as_str())
  }

  /// Parse `[ ... ]`: a bracketed list (empty, space- or comma-separated).
  /// The reference compiler rejects bracketed maps.
  fn parse_bracket_list(&mut self, _open: &crate::lexer::Token) -> Result<Expr> {
    self.skip_newlines();
    if self.eat(&Tok::RBracket) {
      // `[]`: no separator decided yet (the reference reports `space`
      // from list.separator but lets join/append auto fall through)
      return Ok(Expr::List {
        items: vec![],
        separator: Separator::Undecided,
        bracketed: true,
      });
    }
    let first = self.parse_space_list()?;
    if self.at(&Tok::Colon) {
      return Err(self.error_here("Brackets aren't allowed for maps."));
    }
    if self.at(&Tok::Comma) {
      // Comma-separated bracketed list: `[a, b]`
      let mut items = vec![first];
      while self.eat(&Tok::Comma) {
        self.skip_newlines();
        if self.at(&Tok::RBracket) {
          break; // trailing comma
        }
        items.push(self.parse_space_list()?);
      }
      self.skip_newlines();
      self.expect(&Tok::RBracket, "Expected `]` to close list.")?;
      return Ok(Expr::List {
        items,
        separator: Separator::Comma,
        bracketed: true,
      });
    }
    // Space-separated bracketed list: the space list itself is the
    // content (`[1 2 3]` is a 3-element bracketed list, not a nested one)
    self.skip_newlines();
    self.expect(&Tok::RBracket, "Expected `]` to close list.")?;
    match first {
      Expr::List {
        items,
        separator: Separator::Space,
        ..
      } => Ok(Expr::List {
        items,
        separator: Separator::Space,
        bracketed: true,
      }),
      // A single element (`[a]`) has no decided separator — the
      // reference treats it as undecided for join/append auto rules
      single => Ok(Expr::List {
        items: vec![single],
        separator: Separator::Undecided,
        bracketed: true,
      }),
    }
  }

  /// Parse `( ... )`: a map, a comma list, or a grouping
  fn parse_paren(&mut self, _open: &crate::lexer::Token) -> Result<Expr> {
    self.skip_newlines();
    if self.eat(&Tok::RParen) {
      // `()`: no separator decided yet (the reference reports `space`
      // from list.separator but lets join/append auto fall through)
      return Ok(Expr::List {
        items: vec![],
        separator: Separator::Undecided,
        bracketed: false,
      });
    }
    // Parse the first item with a comma-STOPPING parser so that a
    // trailing comma stays available for the comma-list branch below
    // (`(red,)` is a one-element comma list, not a grouping)
    let first = self.parse_space_list()?;
    // Map: `key: value`
    if self.at(&Tok::Colon) {
      self.bump();
      let mut entries: Vec<(Expr, Expr)> = Vec::new();
      let value = self.parse_space_list()?;
      entries.push((first, value));
      while self.eat(&Tok::Comma) {
        self.skip_newlines();
        if self.at(&Tok::RParen) {
          break; // trailing comma
        }
        let key = self.parse_space_list()?;
        self.expect(&Tok::Colon, "Expected `:` in map.")?;
        let value = self.parse_space_list()?;
        entries.push((key, value));
      }
      self.skip_newlines();
      self.expect(&Tok::RParen, "Expected `)` to close map.")?;
      return Ok(Expr::Map { entries });
    }
    // Comma list
    if self.at(&Tok::Comma) {
      let mut items: Vec<Expr> = vec![first];
      while self.eat(&Tok::Comma) {
        self.skip_newlines();
        if self.at(&Tok::RParen) {
          break; // trailing comma: `(red,)` keeps its comma list
        }
        items.push(self.parse_space_list()?);
      }
      self.skip_newlines();
      self.expect(&Tok::RParen, "Expected `)` to close list.")?;
      return Ok(Expr::List {
        items,
        separator: Separator::Comma,
        bracketed: false,
      });
    }
    self.skip_newlines();
    self.expect(&Tok::RParen, "Expected `)`.")?;
    let mut inner = first;
    force_div(&mut inner);
    Ok(Expr::Paren {
      expr: Box::new(inner),
      slash_div: true,
    })
  }

  /// Consume tokens that are directly adjacent (no whitespace) and are
  /// plain text (number/ident) or further interpolation, appending to
  /// `parts` — implements `#{$n}0px` and `a-#{$b}-c`
  fn attach_adjacent_text(&mut self, parts: &mut Vec<StrPart>) -> Result<()> {
    loop {
      if self.peek().start != self.tokens[self.pos - 1].end {
        break;
      }
      match self.peek().kind {
        Tok::Number | Tok::Ident => {
          let t = self.bump();
          parts.push(StrPart::Text(t.text));
        }
        Tok::Hash if self.peek_n(1).kind == Tok::LBrace => {
          self.bump(); // #
          self.bump(); // {
          let inner = self.parse_value_expr()?;
          self.expect(&Tok::RBrace, "Expected `}` to close interpolation.")?;
          parts.push(StrPart::Expr(Box::new(inner)));
        }
        _ => break,
      }
    }
    Ok(())
  }

  /// Parse call arguments: `expr, $name: expr, $list...`
  /// CSS color functions use `/` as an alpha separator, NOT division:
  /// `rgb(0 0 0 / 0.5)` keeps the alpha. Everything else treats call
  /// arguments as division context (`cos(3.14/2)` divides).
  pub fn parse_call_args_color_aware(&mut self, name: &str) -> Result<Vec<CallArg>> {
    let color_fn = matches!(
      name
        .rsplit('.')
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase()
        .as_str(),
      "rgb"
        | "rgba"
        | "hsl"
        | "hsla"
        | "hwb"
        | "oklch"
        | "oklab"
        | "lab"
        | "lch"
        | "color"
        | "device-cmyk"
    );
    let mut args = self.parse_call_args()?;
    if !color_fn {
      for a in &mut args {
        force_div(&mut a.expr);
      }
    }
    Ok(args)
  }

  /// Parse a parenthesized call-argument list: `(name: expr, ...)`.
  /// dart-sass allows the first argument to start on its own line.
  pub fn parse_call_args(&mut self) -> Result<Vec<CallArg>> {
    self.expect(&Tok::LParen, "Expected `(`.")?;
    // dart-sass allows the first argument to start on its own line:
    // `foo(\n  1px,\n)` — skip the newlines right after the open paren.
    self.skip_newlines();
    let mut args = Vec::new();
    while !self.at(&Tok::RParen) {
      let name = if self.peek().kind == Tok::Var && self.peek_n(1).kind == Tok::Colon {
        let n = self.bump();
        self.bump(); // :
        Some(n.text)
      } else {
        None
      };
      let expr = self.parse_space_list()?;
      let rest = self.at(&Tok::Period)
        && self.peek_n(1).kind == Tok::Period
        && self.peek_n(2).kind == Tok::Period;
      if rest {
        self.bump();
        self.bump();
        self.bump();
      }
      args.push(CallArg { expr, name, rest });
      if self.eat(&Tok::Comma) {
        self.skip_newlines();
      } else {
        break;
      }
    }
    self.skip_newlines();
    if !self.eat(&Tok::RParen) {
      let tok = self.peek().clone();
      return Err(
        self
          .error_at(&tok, "Expected `)` to close arguments.")
          .with_help("close the argument list; arguments may span multiple lines, one per line"),
      );
    }
    Ok(args)
  }

  /// Capture `name(...)` raw source text (plain CSS passthrough),
  /// converting `#{...}` occurrences into interpolation parts
  fn capture_raw_fn(&mut self, _name: &str, start: usize) -> Result<Expr> {
    self.expect(&Tok::LParen, "Expected `(`.")?;
    let mut depth = 1usize;
    loop {
      let t = self.bump();
      match t.kind {
        Tok::LParen => depth += 1,
        Tok::RParen => {
          depth -= 1;
          if depth == 0 {
            break;
          }
        }
        Tok::LBrace => depth += 1,
        Tok::RBrace => depth = depth.saturating_sub(1),
        Tok::Newline => {
          return Err(self.error_at(&t, "Unterminated function call."));
        }
        Tok::Eof => {
          return Err(self.error_at(&t, "Expected `)` to close function call."));
        }
        _ => {}
      }
    }
    let end = self.tokens[self.pos - 1].end;
    let raw = self.src[start..end].to_string();
    string_to_interp(&raw, false)
  }

  /// Parse `$name` [`:` default] [`...`] separated by commas
  pub fn parse_params(&mut self) -> Result<Vec<Param>> {
    self.expect(&Tok::LParen, "Expected `(`.")?;
    // Multi-line parameter lists: `@mixin foo(\n  $a,\n  $b\n)` — like
    // dart-sass, allow the first parameter to start on its own line.
    self.skip_newlines();
    let mut params = Vec::new();
    while !self.at(&Tok::RParen) {
      let var = self.expect(&Tok::Var, "Expected a variable name in parameters.")?;
      let default = if self.eat(&Tok::Colon) {
        Some(self.parse_space_list()?)
      } else {
        None
      };
      let rest = self.at(&Tok::Period)
        && self.peek_n(1).kind == Tok::Period
        && self.peek_n(2).kind == Tok::Period;
      if rest {
        self.bump();
        self.bump();
        self.bump();
      }
      params.push(Param {
        name: var.text,
        default,
        rest,
      });
      if self.eat(&Tok::Comma) {
        self.skip_newlines();
      } else {
        break;
      }
    }
    self.skip_newlines();
    self.expect(&Tok::RParen, "Expected `)` to close parameters.")?;
    Ok(params)
  }

  /// Parse `$name: value !default !global;`
  pub fn parse_var_decl(&mut self) -> Result<Statement> {
    let var_tok = self.bump(); // Var
    let name = var_tok.text.clone();
    self.expect(&Tok::Colon, format!("Expected `:` after `${}`.", name))?;
    // In brace syntax the value may start on the line after the colon
    // (`$icons:\n  "eye" "\\f112" 12px,\n  ...;`)
    if self.syntax == Syntax::Braces {
      self.skip_loud_comments();
      self.skip_newlines();
      self.skip_loud_comments();
    }
    let value = self.parse_value_expr()?;
    let mut guarded = false;
    let mut global = false;
    loop {
      match self.peek().text.as_str() {
        "!default" if matches!(self.peek().kind, Tok::Ident) => {
          self.bump();
          guarded = true;
        }
        "!global" if matches!(self.peek().kind, Tok::Ident) => {
          self.bump();
          global = true;
        }
        _ => break,
      }
    }
    self.eat_statement_end()?;
    Ok(Statement::VariableDecl {
      name,
      value,
      guarded,
      global,
    })
  }

  /// Consume `;` and/or a newline that terminate a statement
  ///
  /// In brace syntax (`.vcss`) statements are `;`-terminated exactly like
  /// SCSS: a newline does **not** end a statement (dart-sass reports
  /// `expected ";"` there too). A `;` may only be omitted right before
  /// the closing `}` or EOF. The indented syntax (`.vass`) keeps using
  /// newlines as legal terminators.
  pub fn eat_statement_end(&mut self) -> Result<()> {
    self.skip_loud_comments();
    match self.peek().kind {
      Tok::Semi => {
        self.bump();
        self.eat(&Tok::Newline);
        Ok(())
      }
      Tok::Newline if self.syntax == Syntax::Indented => {
        self.bump();
        Ok(())
      }
      Tok::Newline => Err(self.missing_semicolon()),
      Tok::RBrace | Tok::Eof => Ok(()),
      Tok::Comment if self.peek().is_silent_comment() => Ok(()),
      _ if self.syntax == Syntax::Braces => Err(self.missing_semicolon()),
      _ => Err(self.error_here("Expected `;` or a newline.")),
    }
  }

  /// A brace-syntax statement ended without its required `;`.
  /// Anchored at the end of the previous token — exactly where the `;`
  /// belongs — with a one-keystroke quick fix (`E0013`).
  fn missing_semicolon(&self) -> Error {
    let prev = &self.tokens[self.pos.saturating_sub(1)];
    let (line, col) = (prev.end_line, prev.end_col);
    let span = Span::from_positions(
      Position::new(prev.end, line, col),
      Position::new(prev.end, line, col),
    );
    Error::syntax("Expected `;`.")
      .with_span(span)
      .with_code("E0013")
      .with_help(
        "Every statement in `.vcss` must end with a `;` \
                 (a `;` is only optional right before the closing `}`).",
      )
      .with_fix(
        "Insert `;`",
        FixKind::Insert {
          line,
          column: col,
          text: ";".to_string(),
        },
      )
  }

  /// Parse a declaration `prop: value` with optional nested properties,
  /// custom-property raw values, and `!important`
  pub fn parse_declaration_from(&mut self, prop: String) -> Result<Statement> {
    self.expect(&Tok::Colon, format!("Expected `:` after `{}`.", prop))?;
    // In brace syntax a value may start on the line after the colon
    // (`background:\n  linear-gradient(red, blue);`)
    if self.syntax == Syntax::Braces {
      self.skip_loud_comments();
      self.skip_newlines();
    }
    let custom = prop.starts_with("--");
    if custom {
      // Custom property: raw value through `;` (balanced braces/parens)
      let raw = self.capture_custom_value()?;
      self.eat_statement_end()?;
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
    // Value omitted entirely (nested properties without a base value)
    if matches!(
      self.peek().kind,
      Tok::Newline | Tok::LBrace | Tok::Eof | Tok::Semi
    ) {
      let children = self.parse_nested_children()?;
      let end_line = self.tokens[self.pos.saturating_sub(1)].line;
      return Ok(Statement::Declaration {
        prop,
        value: None,
        important: false,
        children,
        custom: false,
        custom_raw: None,
        end_line,
      });
    }
    let value = self.parse_value_expr()?;
    let mut important = false;
    if self.peek().text == "!important" && matches!(self.peek().kind, Tok::Ident) {
      self.bump();
      important = true;
    }
    let children = self.parse_nested_children()?;
    if children.is_empty() {
      self.eat_statement_end()?;
    }
    let end_line = self.tokens[self.pos.saturating_sub(1)].line;
    Ok(Statement::Declaration {
      prop,
      value: Some(value),
      important,
      children,
      custom: false,
      custom_raw: None,
      end_line,
    })
  }

  /// Parse nested property children (`{ ... }` or an indented block)
  fn parse_nested_children(&mut self) -> Result<Vec<Statement>> {
    match self.syntax {
      Syntax::Braces => {
        if !self.at(&Tok::LBrace) {
          return Ok(Vec::new());
        }
        self.bump();
        let body = self.parse_vcss_stmts(false)?;
        self.expect(&Tok::RBrace, "Expected `}` to close nested properties.")?;
        Ok(body)
      }
      Syntax::Indented => Ok(Vec::new()), // handled by the vass parser
    }
  }

  /// Capture a custom property value: raw text with brace/paren balance,
  /// stopping at `;` or line end at depth 0. Spaces after `}` (end of
  /// #{...} interpolation) are collapsed so `--x: #{1 + 2} px` becomes
  /// `--x: 3px` not `--x: 3 px`.
  pub fn capture_custom_value(&mut self) -> Result<String> {
    let mut depth = 0i32;
    let mut parts: Vec<String> = Vec::new();
    loop {
      let t = self.peek();
      match t.kind {
        Tok::LParen | Tok::LBrace | Tok::LBracket => depth += 1,
        Tok::RParen | Tok::RBrace | Tok::RBracket => depth -= 1,
        Tok::Semi | Tok::Eof if depth <= 0 => break,
        Tok::Newline if depth <= 0 => break,
        Tok::Eof => break,
        _ => {}
      }
      let text = &self.src[t.start..t.end];
      if !parts.is_empty() && t.ws_before {
        parts.push(" ".to_string());
      }
      parts.push(text.to_string());
      self.bump();
    }
    Ok(parts.join("").trim().to_string())
  }

  /// Skip to (and consume) the end of the current line
  pub fn finish_line(&mut self) {
    while !self.at(&Tok::Newline) && !self.at(&Tok::Eof) {
      self.bump();
    }
    self.eat(&Tok::Newline);
  }

  // ---- at-rule dispatch -------------------------------------------------

  /// Parse an at-rule after the `At` token was bumped; `at` is that token.
  /// `block_kind` says what follows the header line: `{`, `;`/EOL.
  pub fn parse_at_stmt(
    &mut self,
    at: &crate::lexer::Token,
    has_block: bool,
    block_indent: Option<u32>,
  ) -> Result<Statement> {
    let name = at.text.clone();
    match name.as_str() {
      "mixin" => self.parse_callable_decl(name, true, has_block, block_indent),
      "function" => self.parse_callable_decl(name, false, has_block, block_indent),
      "include" => self.parse_include(has_block, block_indent),
      "if" => self.parse_if_chain(block_indent),
      "each" => self.parse_each(has_block, block_indent),
      "for" => self.parse_for(has_block, block_indent),
      "while" => self.parse_while(has_block, block_indent),
      "return" => {
        let value = self.parse_value_expr()?;
        self.eat_statement_end()?;
        Ok(Statement::Return { value })
      }
      "extend" => self.parse_extend(),
      "at-root" => self.parse_at_root(has_block, block_indent),
      "debug" | "warn" | "error" => {
        let value = self.parse_value_expr()?;
        self.eat_statement_end()?;
        Ok(match name.as_str() {
          "debug" => Statement::Debug { value },
          "warn" => Statement::Warn { value },
          _ => Statement::Error { value },
        })
      }
      "content" => {
        let args = if self.at(&Tok::LParen) {
          self.parse_call_args()?
        } else {
          Vec::new()
        };
        self.eat_statement_end()?;
        Ok(Statement::Content { args })
      }
      "use" => {
        let rule = self.parse_use_rule(false, at)?;
        self.eat_statement_end()?;
        Ok(Statement::Use(rule))
      }
      "forward" => {
        let rule = self.parse_use_rule(true, at)?;
        self.eat_statement_end()?;
        Ok(Statement::Forward(rule))
      }
      "import" => self.parse_import_stmt(),
      "media" | "supports" | "keyframes" | "-webkit-keyframes" | "-moz-keyframes"
      | "-o-keyframes" | "font-face" | "page" | "layer" | "container" | "scope" | "viewport"
      | "-moz-document" | "-ms-viewport" | "property" | "nest" => {
        let params = self.at_params_text(at);
        if has_block {
          let body = self.parse_block_after_header(block_indent)?;
          let close_line = self.tokens[self.pos.saturating_sub(1)].line;
          Ok(Statement::AtRule {
            name,
            params,
            body,
            open_line: at.line,
            close_line,
          })
        } else {
          self.finish_at_header()?;
          Ok(Statement::AtRuleStmt {
            raw: format!("{}{}", at.text, params),
            line: at.line,
          })
        }
      }
      "else" => Err(self.error_at(at, "Unexpected `@else` (no preceding `@if`).")),
      "charset" | "namespace" | "apply" => {
        let params = self.at_params_text(at);
        self.finish_at_header()?;
        let raw = if params.is_empty() {
          format!("@{}", name)
        } else {
          format!("@{} {}", name, params)
        };
        Ok(Statement::AtRuleStmt {
          raw: ensure_semi(raw),
          line: at.line,
        })
      }
      _ => {
        // Unknown at-rule: block if one follows, else raw statement
        if has_block {
          let params = self.at_params_text(at);
          let body = self.parse_block_after_header(block_indent)?;
          let close_line = self.tokens[self.pos.saturating_sub(1)].line;
          Ok(Statement::AtRule {
            name,
            params,
            body,
            open_line: at.line,
            close_line,
          })
        } else {
          let params = self.at_params_text(at);
          self.finish_at_header()?;
          let raw = if params.is_empty() {
            format!("@{}", name)
          } else {
            format!("@{} {}", name, params)
          };
          Ok(Statement::AtRuleStmt {
            raw: ensure_semi(raw),
            line: at.line,
          })
        }
      }
    }
  }

  /// Classify a `{`-terminated head as a nested-properties declaration
  /// (true) or a style rule (false).
  ///
  /// `colon_idx` is the index of the first top-level `:` between the
  /// current position and `brace_idx`.
  pub fn head_is_declaration(&self, colon_idx: usize, brace_idx: usize) -> bool {
    let colon = &self.tokens[colon_idx];
    let first_start = self.tokens[self.pos].start;
    let before = self.src[first_start..colon.start].trim();
    let after = self.src[colon.end..self.tokens[brace_idx].start].trim();

    // The part before `:` must look like a property name: a plain
    // identifier, a custom property, or interpolation
    let prop_like = before.starts_with("--")
      || (!before.is_empty()
        && before
          .chars()
          .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '$'));
    let has_interp = before.contains("#{");
    if !prop_like && !has_interp {
      return false; // selectors like `.a:hover`, `&:focus`, `input[x]:y`
    }
    if before.contains('#') && !has_interp && !before.starts_with("--") {
      return false; // `#id:hover`
    }

    // Empty / value-like content after `:` -> nested properties
    if after.is_empty() {
      return true;
    }
    let first_ch = after.chars().next().unwrap();
    if first_ch.is_ascii_digit()
      || matches!(first_ch, '.' | '#' | '$' | '(' | '"' | '\'')
      || after.starts_with("#{")
      || after.starts_with("-")
    {
      return true; // `font: 12px {`, `color: #fff {` ...
    }

    // A lone identifier after `:` is ambiguous (`a:hover` vs `color: red`):
    // a known property name before the colon means nested properties,
    // otherwise treat as a pseudo-class selector.
    let prop_name = before.trim_start_matches("--");
    is_known_property(prop_name)
  }

  /// Raw parameter text after the `At` token, up to the header terminator
  /// (`{`, `;`, newline, or EOF) at depth 0
  fn at_params_text(&self, at: &crate::lexer::Token) -> String {
    let mut depth = 0i32;
    let mut i = self.pos;
    let end = loop {
      let t = &self.tokens[i.min(self.tokens.len() - 1)];
      let prev_hash = i
        .checked_sub(1)
        .map(|p| self.tokens[p].kind == Tok::Hash)
        .unwrap_or(false);
      let end_now = match t.kind {
        Tok::Semi | Tok::Eof => depth == 0,
        // In brace syntax an at-rule prelude may span lines
        // (`@media screen,\n  print { ... }`)
        Tok::Newline => depth == 0 && self.syntax != Syntax::Braces,
        Tok::LBrace => depth == 0 && !prev_hash,
        Tok::RBrace => depth <= 0,
        _ => false,
      };
      if end_now {
        break t.start;
      }
      match t.kind {
        Tok::LParen | Tok::LBracket | Tok::LBrace => depth += 1,
        Tok::RParen | Tok::RBracket | Tok::RBrace => depth -= 1,
        _ => {}
      }
      i += 1;
      if i >= self.tokens.len() {
        break t.end;
      }
    };
    let mut text = self.src[at.end..end.max(at.end)].trim().to_string();
    if self.syntax == Syntax::Braces && text.contains('\n') {
      // Multi-line prelude: collapse the embedded newline(s) to single
      // spaces the way the reference normalizes the header
      text = text.split_whitespace().collect::<Vec<&str>>().join(" ");
    }
    text
  }

  /// Consume the remainder of the at-rule header line (`;` / newline)
  fn finish_at_header(&mut self) -> Result<()> {
    match self.syntax {
      Syntax::Braces => {
        if !matches!(
          self.peek().kind,
          Tok::Semi | Tok::Newline | Tok::Eof | Tok::RBrace
        ) {
          self.skip_to_terminator();
        }
        self.eat_statement_end()
      }
      Syntax::Indented => {
        // Jump to the end of the header line and consume it
        if !self.at(&Tok::Newline) && !self.at(&Tok::Eof) {
          self.skip_to_terminator();
        }
        if self.at(&Tok::Semi) {
          self.bump();
        }
        self.eat(&Tok::Newline);
        Ok(())
      }
    }
  }

  /// Parse the block body after an at-rule header
  fn parse_block_after_header(&mut self, block_indent: Option<u32>) -> Result<Vec<Statement>> {
    match self.syntax {
      Syntax::Braces => {
        if !self.at(&Tok::LBrace) {
          self.skip_to_lbrace();
        }
        self.expect(&Tok::LBrace, "Expected `{` after at-rule.")?;
        let body = self.parse_vcss_stmts(false)?;
        self.expect(&Tok::RBrace, "Expected `}` to close at-rule.")?;
        Ok(body)
      }
      Syntax::Indented => {
        let ci = block_indent.ok_or_else(|| self.error_here("Expected an indented block."))?;
        // Consume the rest of the header line before the block
        if !self.at(&Tok::Newline) && !self.at(&Tok::Eof) {
          self.skip_to_terminator();
        }
        self.eat(&Tok::Newline);
        self.parse_vass_block(ci)
      }
    }
  }

  /// `@mixin` / `@function`
  pub(crate) fn parse_callable_decl(
    &mut self,
    kind: String,
    is_mixin: bool,
    has_block: bool,
    block_indent: Option<u32>,
  ) -> Result<Statement> {
    let name_tok = self.expect(&Tok::Ident, format!("Expected a {} name.", kind))?;
    let params = if self.at(&Tok::LParen) {
      self.parse_params()?
    } else {
      Vec::new()
    };
    if !has_block {
      return Err(self.error_here(format!("Expected a block after `@{}`.", kind)));
    }
    let body = self.parse_block_after_header(block_indent)?;
    let decl = CallableDecl {
      name: name_tok.text,
      params,
      body,
    };
    Ok(if is_mixin {
      Statement::MixinDecl(decl)
    } else {
      Statement::FunctionDecl(decl)
    })
  }

  /// `@include name(args) using ($x) { ... }` / `@include ns.name(...)`
  pub(crate) fn parse_include(
    &mut self,
    has_block: bool,
    block_indent: Option<u32>,
  ) -> Result<Statement> {
    let name_tok = self.expect(&Tok::Ident, "Expected a mixin name after `@include`.")?;
    let mut name = name_tok.text.clone();
    // Namespaced mixin: `module.name`
    if self.at(&Tok::Period)
      && self.peek().start == name_tok.end
      && self.peek_n(1).kind == Tok::Ident
    {
      self.bump(); // .
      let member = self.bump(); // ident
      name = format!("{}.{}", name, member.text);
    }
    let args = if self.at(&Tok::LParen) && self.peek().start == self.tokens[self.pos - 1].end {
      self.parse_call_args()?
    } else {
      Vec::new()
    };
    // `using ($x, $y)`
    let mut using = Vec::new();
    if matches!(self.peek().kind, Tok::Ident) && self.peek().text == "using" {
      self.bump();
      using = self.parse_params()?;
    }
    let content = if has_block {
      Some(self.parse_block_after_header(block_indent)?)
    } else {
      self.finish_at_header()?;
      None
    };
    Ok(Statement::Include {
      name,
      args,
      content,
      using,
    })
  }

  /// `@if` with `@else if` / `@else` chain
  fn parse_if_chain(&mut self, block_indent: Option<u32>) -> Result<Statement> {
    let cond = self.parse_or()?;
    let body = self.parse_block_after_header(block_indent)?;
    let mut clauses: Vec<(Expr, Vec<Statement>)> = vec![(cond, body)];
    let mut else_body: Option<Vec<Statement>> = None;
    loop {
      // `@else` must directly follow (after trivia)
      let save = self.pos;
      self.skip_trivia();
      if self.peek().kind == Tok::At && self.peek().text == "else" {
        self.bump(); // @else
        if matches!(self.peek().kind, Tok::Ident) && self.peek().text == "if" {
          self.bump(); // if
          let cond = self.parse_or()?;
          let body = self.parse_block_after_header(block_indent)?;
          clauses.push((cond, body));
        } else {
          let body = self.parse_block_after_header(block_indent)?;
          else_body = Some(body);
          break;
        }
      } else {
        self.pos = save;
        break;
      }
    }
    Ok(Statement::If(IfChain { clauses, else_body }))
  }

  /// `@each $a, $b in $list`
  fn parse_each(&mut self, has_block: bool, block_indent: Option<u32>) -> Result<Statement> {
    let mut vars = Vec::new();
    loop {
      let var = self.expect(&Tok::Var, "Expected a variable in `@each`.")?;
      vars.push(var.text);
      if !self.eat(&Tok::Comma) {
        break;
      }
    }
    let in_tok = self.expect(&Tok::Ident, "Expected `in` in `@each`.")?;
    if in_tok.text != "in" {
      return Err(self.error_at(&in_tok, "Expected `in` in `@each`."));
    }
    let list = self.parse_value_expr()?;
    if !has_block {
      return Err(self.error_here("Expected a block after `@each`."));
    }
    let body = self.parse_block_after_header(block_indent)?;
    Ok(Statement::Each { vars, list, body })
  }

  /// `@for $i from a through b` / `@for $i from a to b`
  fn parse_for(&mut self, has_block: bool, block_indent: Option<u32>) -> Result<Statement> {
    let var = self.expect(&Tok::Var, "Expected a variable in `@for`.")?;
    let from_kw = self.expect(&Tok::Ident, "Expected `from` in `@for`.")?;
    if from_kw.text != "from" {
      return Err(self.error_at(&from_kw, "Expected `from` in `@for`."));
    }
    // Bounds are single expressions (no space lists) so that the
    // `to`/`through` keyword is not swallowed as a list item
    let from = self.parse_or()?;
    let bound_kw = self.expect(&Tok::Ident, "Expected `to` or `through` in `@for`.")?;
    let inclusive = match bound_kw.text.as_str() {
      "through" => true,
      "to" => false,
      other => {
        return Err(
          Error::syntax(format!(
            "Expected `to` or `through` in `@for`, got `{}`.",
            other
          ))
          .with_span(span_of(&bound_kw)),
        );
      }
    };
    let to = self.parse_or()?;
    if !has_block {
      return Err(self.error_here("Expected a block after `@for`."));
    }
    let body = self.parse_block_after_header(block_indent)?;
    Ok(Statement::For {
      var: var.text,
      from,
      to,
      inclusive,
      body,
    })
  }

  /// `@while cond`
  fn parse_while(&mut self, has_block: bool, block_indent: Option<u32>) -> Result<Statement> {
    let cond = self.parse_or()?;
    if !has_block {
      return Err(self.error_here("Expected a block after `@while`."));
    }
    let body = self.parse_block_after_header(block_indent)?;
    Ok(Statement::While { cond, body })
  }

  /// `@extend .target !optional;`
  fn parse_extend(&mut self) -> Result<Statement> {
    let start = self.peek().start;
    let mut depth = 0i32;
    loop {
      let t = self.peek();
      match t.kind {
        Tok::LParen | Tok::LBracket => depth += 1,
        Tok::RParen | Tok::RBracket => depth -= 1,
        Tok::Semi | Tok::Newline | Tok::Eof if depth <= 0 => break,
        Tok::Eof => break,
        _ => {}
      }
      self.bump();
    }
    let end = if self.pos > 0 {
      self.tokens[self.pos - 1].end.max(start)
    } else {
      start
    };
    let mut text = self.src[start..end].trim().to_string();
    let mut optional = false;
    if let Some(stripped) = text.strip_suffix("!optional") {
      optional = true;
      text = stripped.trim().to_string();
    }
    self.eat_statement_end()?;
    Ok(Statement::Extend {
      target: text,
      optional,
    })
  }

  /// `@at-root (query) { ... }`
  fn parse_at_root(&mut self, has_block: bool, block_indent: Option<u32>) -> Result<Statement> {
    let query = if self.at(&Tok::LParen) {
      let start = self.peek().start;
      let mut depth = 0i32;
      loop {
        let t = self.peek();
        match t.kind {
          Tok::LParen => depth += 1,
          Tok::RParen => {
            depth -= 1;
            if depth == 0 {
              self.bump();
              break;
            }
          }
          Tok::Eof => break,
          _ => {}
        }
        self.bump();
      }
      let end = self.tokens[self.pos - 1].end;
      Some(self.src[start..end].trim().to_string())
    } else {
      None
    };
    if !has_block {
      return Err(self.error_here("Expected a block after `@at-root`."));
    }
    // Selector form: `@at-root .b { ... }` / `@at-root (query) .b { ... }`
    // — the selector and block together form a style rule at root
    let at_block = match self.syntax {
      Syntax::Braces => self.at(&Tok::LBrace),
      Syntax::Indented => matches!(self.peek().kind, Tok::Newline | Tok::Eof),
    };
    if !at_block {
      let sel_start = self.peek().start;
      let sel_start_tok = self.pos;
      let end = match self.syntax {
        Syntax::Braces => {
          let mut i = self.pos;
          let mut depth = 0i32;
          let found = loop {
            if i >= self.tokens.len() {
              break None;
            }
            match self.tokens[i].kind {
              Tok::LParen | Tok::LBracket => depth += 1,
              Tok::RParen | Tok::RBracket => depth -= 1,
              Tok::LBrace if depth == 0 => break Some(i),
              Tok::Eof => break None,
              _ => {}
            }
            i += 1;
          };
          match found {
            Some(i) => {
              let end_off = self.tokens[i].start;
              self.pos = i;
              end_off
            }
            None => {
              let (end_off, idx) = self.line_end_offset();
              self.pos = idx;
              end_off
            }
          }
        }
        Syntax::Indented => {
          let (end_off, idx) = self.line_end_offset();
          self.pos = idx;
          end_off
        }
      };
      let selector = self.src[sel_start..end].trim().to_string();
      let open_line = self.tokens[sel_start_tok].line;
      let body = self.parse_block_after_header(block_indent)?;
      let close_line = self.tokens[self.pos.saturating_sub(1)].line;
      return Ok(Statement::AtRoot {
        query,
        body: vec![Statement::StyleRule {
          selector,
          body,
          open_line,
          close_line,
        }],
      });
    }
    let body = self.parse_block_after_header(block_indent)?;
    Ok(Statement::AtRoot { query, body })
  }

  /// `@use "url" as ns with ($a: 1)` / `@forward "url" as pre-* show ...`
  /// Span from the `@` that opens the rule to the end of the last token
  /// (including a trailing `;` when present) — dart-sass underlines the
  /// whole `@use`/`@forward` statement in resolution errors.
  fn rule_span_so_far(&self, at: &crate::lexer::Token) -> vass_core::token::Span {
    let last = self.tokens[self.pos.saturating_sub(1)].clone();
    let (end_offset, end_line, end_col) = if self.at(&Tok::Semi) {
      let semi = self.peek();
      (semi.end, semi.end_line, semi.end_col)
    } else {
      (last.end, last.end_line, last.end_col)
    };
    vass_core::token::Span::from_positions(
      vass_core::token::Position::new(at.start, at.line, at.col),
      vass_core::token::Position::new(end_offset, end_line, end_col),
    )
  }

  fn parse_use_rule(&mut self, is_forward: bool, at: &crate::lexer::Token) -> Result<UseRule> {
    let url_tok = self.peek().clone();
    let url = match url_tok.kind {
      Tok::Str => {
        self.bump();
        strip_quotes(&url_tok.text)
      }
      Tok::Ident => {
        self.bump();
        url_tok.text.clone()
      }
      _ => return Err(self.error_here("Expected a module URL after `@use`/`@forward`.")),
    };
    let mut rule = UseRule {
      url,
      namespace: None,
      with: Vec::new(),
      show: Vec::new(),
      hide: Vec::new(),
      prefix: None,
      span: vass_core::token::Span::dummy(),
    };
    // `as ns` / `as *` / `as prefix-*`
    if matches!(self.peek().kind, Tok::Ident) && self.peek().text == "as" {
      self.bump();
      let ns_tok = self.peek().clone();
      if matches!(ns_tok.kind, Tok::Star) {
        self.bump();
        rule.namespace = Some(String::new());
      } else if matches!(ns_tok.kind, Tok::Ident) {
        self.bump();
        // forward prefix: `as name-*` (the tokenizer may lex `name-`
        // as one identifier, or `name` + Minus + Star)
        if self.at(&Tok::Star) && ns_tok.text.ends_with('-') {
          self.bump();
          // `as i-*`: the dash is part of the prefix (i-color)
          rule.prefix = Some(ns_tok.text.clone());
          if is_forward {
            rule.namespace = Some(String::new());
          }
        } else if self.at(&Tok::Minus) && self.peek_n(1).kind == Tok::Star {
          self.bump();
          self.bump();
          rule.prefix = Some(format!("{}-", ns_tok.text));
          if is_forward {
            rule.namespace = Some(String::new());
          }
        } else {
          rule.namespace = Some(ns_tok.text);
        }
      } else {
        return Err(self.error_here("Expected a namespace after `as`."));
      }
    }
    // `with ($a: 1, ...)`
    if matches!(self.peek().kind, Tok::Ident) && self.peek().text == "with" {
      self.bump();
      self.expect(&Tok::LParen, "Expected `(` after `with`.")?;
      while !self.at(&Tok::RParen) {
        let var = self.expect(&Tok::Var, "Expected a variable in `with`.")?;
        self.expect(&Tok::Colon, "Expected `:` after `$` variable.")?;
        let value = self.parse_value_expr()?;
        rule.with.push((var.text, value));
        if !self.eat(&Tok::Comma) {
          break;
        }
      }
      self.expect(&Tok::RParen, "Expected `)` to close `with`.")?;
    }
    // `show` / `hide` lists (forward only)
    loop {
      let t = self.peek().clone();
      if matches!(t.kind, Tok::Ident) && t.text == "show" {
        self.bump();
        rule.show = self.parse_member_list()?;
      } else if matches!(t.kind, Tok::Ident) && t.text == "hide" {
        self.bump();
        rule.hide = self.parse_member_list()?;
      } else {
        break;
      }
    }
    rule.span = self.rule_span_so_far(at);
    Ok(rule)
  }

  /// `name-*, $var, fn()` member list for `@forward show/hide`
  fn parse_member_list(&mut self) -> Result<Vec<String>> {
    let mut members = Vec::new();
    loop {
      let t = self.peek().clone();
      match t.kind {
        Tok::Ident => {
          self.bump();
          let mut name = t.text;
          // wildcard suffix `name-*` or plain `*`
          if self.at(&Tok::Minus) && self.peek_n(1).kind == Tok::Star {
            self.bump();
            self.bump();
            name.push_str("-*");
          } else if self.at(&Tok::Star) && name == "as" {
            // handled elsewhere
          }
          members.push(name);
        }
        Tok::Var => {
          self.bump();
          members.push(format!("${}", t.text));
        }
        Tok::Period => {
          self.bump();
          let n = self.expect(&Tok::Ident, "Expected a name after `.`.")?;
          members.push(format!(".{}", n.text));
        }
        _ => return Err(self.error_here("Expected a member name.")),
      }
      if !self.eat(&Tok::Comma) {
        break;
      }
    }
    Ok(members)
  }

  /// `@import "a", url(b.css) screen;`
  fn parse_import_stmt(&mut self) -> Result<Statement> {
    let mut targets = Vec::new();
    loop {
      let t = self.peek().clone();
      let (url, quoted, plain) = match t.kind {
        Tok::Str => {
          self.bump();
          (strip_quotes(&t.text), true, false)
        }
        Tok::Ident if t.text == "url" && self.peek_n(1).kind == Tok::LParen => {
          self.bump(); // consume the `url` ident; capture starts at `(`
          let expr = self.capture_raw_fn("url", t.start)?;
          let text = match expr {
            Expr::Str { text, .. } => text,
            Expr::Interp { parts, .. } => parts
              .iter()
              .map(|p| match p {
                StrPart::Text(s) => s.clone(),
                StrPart::Expr(_) => String::new(),
              })
              .collect(),
            _ => String::new(),
          };
          (text, false, true)
        }
        Tok::Ident => {
          self.bump();
          (t.text.clone(), false, false)
        }
        _ => return Err(self.error_here("Expected a URL after `@import`.")),
      };
      // Media query suffix (raw text through the terminator)
      let media_start = self.peek().start;
      let mut media: Option<String> = None;
      if !self.at(&Tok::Semi) && !self.at(&Tok::Newline) && !self.at(&Tok::Eof) {
        let mut depth = 0i32;
        loop {
          let tk = self.peek();
          match tk.kind {
            Tok::LParen | Tok::LBracket => depth += 1,
            Tok::RParen | Tok::RBracket => depth -= 1,
            Tok::Semi | Tok::Newline | Tok::Eof | Tok::Comma if depth <= 0 => break,
            Tok::Eof => break,
            _ => {}
          }
          self.bump();
        }
        let end = self.tokens[self.pos - 1].end.max(media_start);
        let m = self.src[media_start..end].trim().to_string();
        if !m.is_empty() {
          media = Some(m);
        }
      }
      let plain_css = plain
        || media.is_some()
        || url.ends_with(".css")
        || url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("//");
      // Span covers this target only (URL through media suffix) so a
      // failed VASS import points at its own URL, not the whole rule
      let last = self.tokens[self.pos.saturating_sub(1)].clone();
      let span = vass_core::token::Span::from_positions(
        vass_core::token::Position::new(t.start, t.line, t.col),
        vass_core::token::Position::new(last.end, last.end_line, last.end_col),
      );
      targets.push(ImportTarget {
        url,
        quoted,
        media,
        plain_css,
        span,
      });
      if !self.eat(&Tok::Comma) {
        break;
      }
    }
    self.eat_statement_end()?;
    Ok(Statement::Import(targets))
  }
}

/// Ensure a raw statement ends with `;`
pub(crate) fn ensure_semi(raw: String) -> String {
  if raw.ends_with(';') {
    raw
  } else {
    format!("{};", raw)
  }
}

/// Common CSS property names, used to disambiguate nested properties
/// (`font: 12px { ... }`) from pseudo-selectors (`a:hover { ... }`)
const KNOWN_PROPS: &[&str] = &[
  "accent-color",
  "align-content",
  "align-items",
  "align-self",
  "all",
  "animation",
  "animation-delay",
  "animation-direction",
  "animation-duration",
  "animation-fill-mode",
  "animation-iteration-count",
  "animation-name",
  "animation-play-state",
  "animation-timing-function",
  "appearance",
  "aspect-ratio",
  "backdrop-filter",
  "backface-visibility",
  "background",
  "background-attachment",
  "background-blend-mode",
  "background-clip",
  "background-color",
  "background-image",
  "background-origin",
  "background-position",
  "background-repeat",
  "background-size",
  "block-size",
  "border",
  "border-block",
  "border-block-end",
  "border-block-start",
  "border-bottom",
  "border-bottom-color",
  "border-bottom-left-radius",
  "border-bottom-right-radius",
  "border-bottom-style",
  "border-bottom-width",
  "border-collapse",
  "border-color",
  "border-end-end-radius",
  "border-end-start-radius",
  "border-image",
  "border-image-outset",
  "border-image-repeat",
  "border-image-slice",
  "border-image-source",
  "border-image-width",
  "border-inline",
  "border-inline-end",
  "border-inline-start",
  "border-left",
  "border-left-color",
  "border-left-style",
  "border-left-width",
  "border-radius",
  "border-right",
  "border-right-color",
  "border-right-style",
  "border-right-width",
  "border-spacing",
  "border-start-end-radius",
  "border-start-start-radius",
  "border-style",
  "border-top",
  "border-top-color",
  "border-top-left-radius",
  "border-top-right-radius",
  "border-top-style",
  "border-top-width",
  "border-width",
  "bottom",
  "box-decoration-break",
  "box-shadow",
  "box-sizing",
  "break-after",
  "break-before",
  "break-inside",
  "caption-side",
  "caret-color",
  "clear",
  "clip",
  "clip-path",
  "color",
  "color-scheme",
  "column-count",
  "column-fill",
  "column-gap",
  "column-rule",
  "column-rule-color",
  "column-rule-style",
  "column-rule-width",
  "column-span",
  "column-width",
  "columns",
  "contain",
  "content",
  "content-visibility",
  "counter-increment",
  "counter-reset",
  "cursor",
  "direction",
  "display",
  "empty-cells",
  "fill",
  "filter",
  "flex",
  "flex-basis",
  "flex-direction",
  "flex-flow",
  "flex-grow",
  "flex-shrink",
  "flex-wrap",
  "float",
  "font",
  "font-family",
  "font-feature-settings",
  "font-kerning",
  "font-size",
  "font-size-adjust",
  "font-stretch",
  "font-style",
  "font-synthesis",
  "font-variant",
  "font-variant-caps",
  "font-weight",
  "gap",
  "grid",
  "grid-area",
  "grid-auto-columns",
  "grid-auto-flow",
  "grid-auto-rows",
  "grid-column",
  "grid-column-end",
  "grid-column-gap",
  "grid-column-start",
  "grid-gap",
  "grid-row",
  "grid-row-end",
  "grid-row-gap",
  "grid-row-start",
  "grid-template",
  "grid-template-areas",
  "grid-template-columns",
  "grid-template-rows",
  "hanging-punctuation",
  "height",
  "hyphens",
  "image-orientation",
  "image-rendering",
  "inline-size",
  "inset",
  "inset-block",
  "inset-block-end",
  "inset-block-start",
  "inset-inline",
  "inset-inline-end",
  "inset-inline-start",
  "isolation",
  "justify-content",
  "justify-items",
  "justify-self",
  "left",
  "letter-spacing",
  "line-break",
  "line-height",
  "list-style",
  "list-style-image",
  "list-style-position",
  "list-style-type",
  "margin",
  "margin-block",
  "margin-block-end",
  "margin-block-start",
  "margin-bottom",
  "margin-inline",
  "margin-inline-end",
  "margin-inline-start",
  "margin-left",
  "margin-right",
  "margin-top",
  "mask",
  "mask-clip",
  "mask-composite",
  "mask-image",
  "mask-mode",
  "mask-origin",
  "mask-position",
  "mask-repeat",
  "mask-size",
  "max-block-size",
  "max-height",
  "max-inline-size",
  "max-width",
  "min-block-size",
  "min-height",
  "min-inline-size",
  "min-width",
  "mix-blend-mode",
  "object-fit",
  "object-position",
  "offset",
  "offset-anchor",
  "offset-distance",
  "offset-path",
  "offset-rotate",
  "opacity",
  "order",
  "outline",
  "outline-color",
  "outline-offset",
  "outline-style",
  "outline-width",
  "overflow",
  "overflow-anchor",
  "overflow-clip-margin",
  "overflow-wrap",
  "overflow-x",
  "overflow-y",
  "overscroll-behavior",
  "overscroll-behavior-x",
  "overscroll-behavior-y",
  "padding",
  "padding-block",
  "padding-block-end",
  "padding-block-start",
  "padding-bottom",
  "padding-inline",
  "padding-inline-end",
  "padding-inline-start",
  "padding-left",
  "padding-right",
  "padding-top",
  "page-break-after",
  "page-break-before",
  "page-break-inside",
  "paint-order",
  "perspective",
  "perspective-origin",
  "place-content",
  "place-items",
  "place-self",
  "pointer-events",
  "position",
  "quotes",
  "resize",
  "right",
  "rotate",
  "row-gap",
  "scale",
  "scroll-behavior",
  "scroll-margin",
  "scroll-padding",
  "scroll-snap-align",
  "scroll-snap-stop",
  "scroll-snap-type",
  "shape-outside",
  "stroke",
  "tab-size",
  "table-layout",
  "text-align",
  "text-align-last",
  "text-combine-upright",
  "text-decoration",
  "text-decoration-color",
  "text-decoration-line",
  "text-decoration-skip-ink",
  "text-decoration-style",
  "text-decoration-thickness",
  "text-emphasis",
  "text-indent",
  "text-justify",
  "text-orientation",
  "text-overflow",
  "text-rendering",
  "text-shadow",
  "text-transform",
  "text-underline-offset",
  "text-underline-position",
  "top",
  "touch-action",
  "transform",
  "transform-box",
  "transform-origin",
  "transform-style",
  "transition",
  "transition-delay",
  "transition-duration",
  "transition-property",
  "transition-timing-function",
  "translate",
  "unicode-bidi",
  "user-select",
  "vertical-align",
  "visibility",
  "white-space",
  "widows",
  "width",
  "will-change",
  "word-break",
  "word-spacing",
  "word-wrap",
  "writing-mode",
  "z-index",
  "zoom",
];

/// Whether `name` is a known CSS property
pub(crate) fn is_known_property(name: &str) -> bool {
  KNOWN_PROPS.contains(&name)
}

/// Mark every `/` inside a parenthesized expression as real division
pub(crate) fn force_div(expr: &mut Expr) {
  match expr {
    Expr::Binary {
      op: BinaryOp::Div,
      left,
      right,
      allow_slash_div,
      ..
    } => {
      *allow_slash_div = true;
      force_div(left);
      force_div(right);
    }
    Expr::Binary { left, right, .. } => {
      force_div(left);
      force_div(right);
    }
    Expr::Paren { expr, .. } => force_div(expr),
    Expr::Unary { expr, .. } => force_div(expr),
    Expr::List { items, .. } => {
      for item in items {
        force_div(item);
      }
    }
    Expr::Map { entries, .. } => {
      for (_k, v) in entries {
        force_div(v);
      }
    }
    _ => {}
  }
}

/// Split a number token like `-2.5em` into value and unit
pub fn parse_number_token(text: &str) -> Expr {
  let (num_part, unit) = split_number_unit(text);
  let value: f64 = num_part.parse().unwrap_or(0.0);
  Expr::Number {
    value,
    unit: if unit.is_empty() {
      None
    } else {
      Some(unit.to_string())
    },
  }
}

fn split_number_unit(text: &str) -> (&str, &str) {
  let bytes = text.as_bytes();
  let mut end = bytes.len();
  while end > 0 {
    let c = bytes[end - 1] as char;
    if c == '%' || c.is_ascii_alphabetic() {
      end -= 1;
    } else {
      break;
    }
  }
  (&text[..end], &text[end..])
}

/// Strip surrounding quotes from a string token's text
pub(crate) fn strip_quotes(text: &str) -> String {
  let inner = text
    .strip_prefix('"')
    .or_else(|| text.strip_prefix('\''))
    .unwrap_or(text);
  let inner = inner
    .strip_suffix('"')
    .or_else(|| inner.strip_suffix('\''))
    .unwrap_or(inner);
  // Unescape CSS string escapes (`\"`, `\\`, `\a9` codepoints,
  // `\` + newline line continuation) — the reference compiler parses
  // escapes at lex time and re-serializes canonically.
  unescape_css_string(inner)
}

/// Decode CSS string escapes in `s`.
/// `\<hex 1-6 digits>[whitespace]` -> codepoint; `\<char>` -> literal char;
/// `\` + newline -> line continuation (removed).
fn unescape_css_string(s: &str) -> String {
  if !s.contains('\\') {
    return s.to_string();
  }
  let mut out = String::with_capacity(s.len());
  let mut chars = s.chars().peekable();
  while let Some(c) = chars.next() {
    if c != '\\' {
      out.push(c);
      continue;
    }
    match chars.peek() {
      None => {}
      Some('\n') => {
        chars.next();
      }
      Some('\r') => {
        chars.next();
        if chars.peek() == Some(&'\n') {
          chars.next();
        }
      }
      Some(&h) if h.is_ascii_hexdigit() => {
        let mut hex = String::new();
        let mut count = 0;
        while count < 6 {
          match chars.peek() {
            Some(&h2) if h2.is_ascii_hexdigit() => {
              hex.push(h2);
              chars.next();
              count += 1;
            }
            _ => break,
          }
        }
        // One optional whitespace after the escape is consumed
        if let Some(&w) = chars.peek() {
          match w {
            ' ' | '\t' => {
              chars.next();
            }
            '\r' => {
              chars.next();
              if chars.peek() == Some(&'\n') {
                chars.next();
              }
            }
            '\n' => {
              chars.next();
            }
            _ => {}
          }
        }
        match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
          Some(ch) => out.push(ch),
          None => out.push('\u{FFFD}'),
        }
      }
      Some(&n) => {
        chars.next();
        out.push(n);
      }
    }
  }
  out
}

/// Convert token text into an [`Expr`], splitting out `#{...}` interpolation
pub(crate) fn string_to_interp(text: &str, quoted: bool) -> Result<Expr> {
  if !text.contains("#{") {
    let content = if quoted {
      strip_quotes(text)
    } else {
      text.to_string()
    };
    return Ok(Expr::Str {
      text: content,
      quoted,
    });
  }
  let src = if quoted {
    strip_quotes(text)
  } else {
    text.to_string()
  };
  let chars: Vec<char> = src.chars().collect();
  let mut parts: Vec<StrPart> = Vec::new();
  let mut lit = String::new();
  let mut i = 0usize;
  while i < chars.len() {
    if chars[i] == '\\' && i + 1 < chars.len() {
      // Unescape the pair (\\ -> \, \\" -> ", ...); a hex escape's
      // full form is handled below through the same path since only
      // `#{` boundaries matter during this pass.
      lit.push(chars[i + 1]);
      i += 2;
      continue;
    }
    if chars[i] == '#' && i + 1 < chars.len() && chars[i + 1] == '{' {
      // find matching close brace
      let mut depth = 1usize;
      let mut j = i + 2;
      while j < chars.len() && depth > 0 {
        match chars[j] {
          '{' => depth += 1,
          '}' => depth -= 1,
          _ => {}
        }
        if depth == 0 {
          break;
        }
        j += 1;
      }
      if depth != 0 {
        return Err(Error::syntax("Unterminated interpolation in string."));
      }
      if !lit.is_empty() {
        parts.push(StrPart::Text(std::mem::take(&mut lit)));
      }
      let inner: String = chars[i + 2..j].iter().collect();
      let inner_expr = parse_expr_snippet(&inner)?;
      parts.push(StrPart::Expr(Box::new(inner_expr)));
      i = j + 1;
      continue;
    }
    lit.push(chars[i]);
    i += 1;
  }
  if !lit.is_empty() {
    parts.push(StrPart::Text(lit));
  }
  Ok(Expr::Interp { parts, quoted })
}

/// Parse text containing `#{...}` interpolation into an expression tree
/// (used by the evaluator for raw selectors, params and custom properties)
pub fn parse_interp_text(text: &str) -> Result<Expr> {
  string_to_interp(text, false)
}

/// Parse a standalone expression snippet (used by the evaluator to resolve
/// interpolation inside raw selectors / at-rule params / custom properties)
pub fn parse_expr_snippet(src: &str) -> Result<Expr> {
  let mut p = Parser::new(src)?;
  let expr = p.parse_value_expr()?;
  if !p.at(&Tok::Eof) {
    return Err(p.error_here("Unexpected trailing tokens in expression."));
  }
  Ok(expr)
}

/// Decode CSS escape sequences inside an unquoted VALUE string (reference
/// behaviour: `c: \1F600` emits the literal character, `tab\there` emits
/// `tabthere`). Property names and selectors keep raw escapes by design
/// (raw-captured), matching the documented round-4 behaviour.
pub(crate) fn decode_css_escapes(s: &str) -> String {
  if !s.contains('\\') {
    return s.to_string();
  }
  let mut out = String::with_capacity(s.len());
  let mut chars = s.chars().peekable();
  while let Some(c) = chars.next() {
    if c != '\\' {
      out.push(c);
      continue;
    }
    let mut hex = String::new();
    while hex.len() < 6 {
      match chars.peek() {
        Some(h) if h.is_ascii_hexdigit() => {
          hex.push(*h);
          chars.next();
        }
        _ => break,
      }
    }
    if !hex.is_empty() {
      // one whitespace separator terminates the escape
      if let Some(w) = chars.peek() {
        if *w == ' ' || *w == '\t' {
          chars.next();
        } else if *w == '\r' {
          chars.next();
          if chars.peek() == Some(&'\n') {
            chars.next();
          }
        } else if *w == '\n' {
          chars.next();
        }
      }
      if let Some(cp) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
        out.push(cp);
      }
    } else if let Some(any) = chars.next() {
      out.push(any);
    }
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::ast::input::Syntax;

  fn parse_val(src: &str) -> Expr {
    let mut p = Parser::new(src).unwrap();
    let e = p.parse_value_expr().unwrap();
    assert!(p.at(&Tok::Eof), "did not consume all of `{}`", src);
    e
  }

  #[test]
  fn test_number_token() {
    match parse_number_token("10px") {
      Expr::Number { value, unit } => {
        assert_eq!(value, 10.0);
        assert_eq!(unit.as_deref(), Some("px"));
      }
      _ => panic!("expected number"),
    }
    match parse_number_token("-2.5") {
      Expr::Number { value, unit } => {
        assert_eq!(value, -2.5);
        assert_eq!(unit, None);
      }
      _ => panic!("expected number"),
    }
    match parse_number_token("50%") {
      Expr::Number { value, unit } => {
        assert_eq!(value, 50.0);
        assert_eq!(unit.as_deref(), Some("%"));
      }
      _ => panic!("expected number"),
    }
  }

  #[test]
  fn test_arithmetic_precedence() {
    // 1 + 2 * 3
    let e = parse_val("1 + 2 * 3");
    match e {
      Expr::Binary {
        op: BinaryOp::Add,
        right,
        ..
      } => {
        assert!(matches!(
          *right,
          Expr::Binary {
            op: BinaryOp::Mul,
            ..
          }
        ));
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_space_list() {
    let e = parse_val("1px solid #fff");
    match e {
      Expr::List {
        items, separator, ..
      } => {
        assert_eq!(items.len(), 3);
        assert_eq!(separator, Separator::Space);
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_comma_list() {
    let e = parse_val("Arial, sans-serif");
    match e {
      Expr::List { separator, .. } => assert_eq!(separator, Separator::Comma),
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_call() {
    let e = parse_val("rgb(255, 0, 0)");
    match e {
      Expr::Call { name, args, .. } => {
        assert_eq!(name, "rgb");
        assert_eq!(args.len(), 3);
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_namespaced_call() {
    let e = parse_val("math.div(10, 2)");
    match e {
      Expr::Call { name, .. } => assert_eq!(name, "math.div"),
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_namespaced_var() {
    let e = parse_val("math.$pi");
    match e {
      Expr::ModuleVar {
        namespace, name, ..
      } => {
        assert_eq!(namespace, "math");
        assert_eq!(name, "pi");
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_map_literal() {
    let e = parse_val("(\"a\": 1, \"b\": 2)");
    match e {
      Expr::Map { entries } => assert_eq!(entries.len(), 2),
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_paren_grouping() {
    let e = parse_val("(1 + 2) * 3");
    assert!(matches!(
      e,
      Expr::Binary {
        op: BinaryOp::Mul,
        ..
      }
    ));
  }

  #[test]
  fn test_comparison() {
    let e = parse_val("1px == 1px");
    assert!(matches!(
      e,
      Expr::Binary {
        op: BinaryOp::Eq,
        ..
      }
    ));
    let e = parse_val("$a != $b");
    assert!(matches!(
      e,
      Expr::Binary {
        op: BinaryOp::Ne,
        ..
      }
    ));
    let e = parse_val("1 <= 2 and 3 >= 2");
    match e {
      Expr::Binary {
        op: BinaryOp::And, ..
      } => {}
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_interpolation_in_string() {
    let e = parse_val("\"a-#{1 + 1}-b\"");
    match e {
      Expr::Interp { parts, quoted } => {
        assert!(quoted);
        assert_eq!(parts.len(), 3);
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_div_semantics() {
    // literal/literal stays a separator
    let e = parse_val("12px/30px");
    match e {
      Expr::Binary {
        op: BinaryOp::Div,
        allow_slash_div,
        ..
      } => {
        assert!(!allow_slash_div);
      }
      other => panic!("unexpected: {:?}", other),
    }
    // variable / literal divides
    let e = parse_val("$a/2");
    match e {
      Expr::Binary {
        op: BinaryOp::Div,
        allow_slash_div,
        ..
      } => {
        assert!(allow_slash_div);
      }
      other => panic!("unexpected: {:?}", other),
    }
    // parens force division
    let e = parse_val("(1px/2)");
    match e {
      Expr::Paren { expr, slash_div } => {
        assert!(slash_div);
        assert!(matches!(
          *expr,
          Expr::Binary {
            op: BinaryOp::Div,
            ..
          }
        ));
      }
      other => panic!("unexpected: {:?}", other),
    }
  }

  #[test]
  fn test_syntax_field_default() {
    // The syntax field drives block parsing; expressions are shared
    let p = Parser::new("$x: 1").unwrap();
    assert!(p.syntax == Syntax::Indented || p.syntax == Syntax::Braces);
  }
}
