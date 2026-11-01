//! Shared parser infrastructure: token cursor and helpers

use vass_core::ast::input::Syntax;
use vass_core::error::{Error, Result};
use vass_core::token::{Position, Span};

use crate::lexer::{Tok, Token, tokenize};

/// A recursive-descent parser over a flat token stream
pub struct Parser {
  /// Original source (for raw-text slicing of selectors and at-rules)
  pub src: String,
  /// Token stream (always ends with `Tok::Eof`)
  pub tokens: Vec<Token>,
  /// Cursor into `tokens`
  pub pos: usize,
  /// Which syntax this parser is reading
  pub syntax: Syntax,
  /// A loud comment captured on a declaration line, to be emitted as its
  /// own statement on the next call (indented syntax trailing comments)
  pub pending_comment: Option<(String, u32)>,
}

impl Parser {
  /// Create a parser over `source` (brace syntax by default)
  pub fn new(source: &str) -> Result<Self> {
    Self::with_syntax(source, Syntax::Braces)
  }

  /// Create a parser with an explicit syntax
  pub fn with_syntax(source: &str, syntax: Syntax) -> Result<Self> {
    // Strip a leading UTF-8 BOM so `src` and the token stream share the
    // same coordinates: raw-text slicing (`raw_between` & friends) indexes
    // `src` with token byte offsets. With a BOM left in `src` every slice
    // shifts by 3 bytes, silently corrupting selectors and property names
    // (e.g. `.card` -> `.ca`) and producing cascading parse errors —
    // the classic "mixin not working" report for files saved on Windows.
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    Ok(Parser {
      src: source.to_string(),
      tokens: tokenize(source)?,
      pos: 0,
      syntax,
      pending_comment: None,
    })
  }

  /// Peek at the current token
  pub fn peek(&self) -> &Token {
    &self.tokens[self.pos.min(self.tokens.len() - 1)]
  }

  /// Peek `n` tokens ahead
  pub fn peek_n(&self, n: usize) -> &Token {
    &self.tokens[(self.pos + n).min(self.tokens.len() - 1)]
  }

  /// Consume and return the current token
  pub fn bump(&mut self) -> Token {
    let tok = self.peek().clone();
    if self.pos < self.tokens.len() - 1 {
      self.pos += 1;
    }
    tok
  }

  /// Whether the current token has kind `k`
  pub fn at(&self, k: &Tok) -> bool {
    &self.peek().kind == k
  }

  /// Consume the current token if it has kind `k`; report whether it did
  pub fn eat(&mut self, k: &Tok) -> bool {
    if self.at(k) {
      self.bump();
      true
    } else {
      false
    }
  }

  /// Consume kind `k` or fail with `message`
  pub fn expect(&mut self, k: &Tok, message: impl Into<Box<str>>) -> Result<Token> {
    if self.at(k) {
      Ok(self.bump())
    } else {
      Err(self.error_here(message))
    }
  }

  /// Skip newline tokens
  pub fn skip_newlines(&mut self) {
    while self.at(&Tok::Newline) {
      self.bump();
    }
  }

  /// Skip silent (`//`) comments followed by their newline
  pub fn skip_silent_comments(&mut self) {
    while self.peek().is_silent_comment() {
      self.bump();
      self.eat(&Tok::Newline);
    }
  }

  /// Skip blank lines and silent comments (but keep loud comments)
  pub fn skip_trivia(&mut self) {
    loop {
      if self.at(&Tok::Newline) || self.peek().is_silent_comment() {
        self.bump();
        self.eat(&Tok::Newline);
      } else {
        break;
      }
    }
  }

  /// Skip loud (`/* ... */`) comment tokens. The reference treats a loud
  /// comment inside a declaration VALUE as whitespace and drops it from
  /// the output (`color: red /* c */` serializes as `color: red`), so the
  /// value parser skips them between items and at statement ends.
  pub fn skip_loud_comments(&mut self) {
    while self.peek().is_loud_comment() {
      self.bump();
    }
  }

  /// Advance past tokens until an `LBrace` at paren depth 0 (for vcss
  /// at-rule headers). Interpolation braces (`#{`) are counted as depth.
  pub fn skip_to_lbrace(&mut self) {
    let mut depth = 0i32;
    loop {
      let prev_hash = self
        .pos
        .checked_sub(1)
        .map(|p| self.tokens[p].kind == Tok::Hash)
        .unwrap_or(false);
      match self.peek().kind {
        Tok::LParen | Tok::LBracket => depth += 1,
        Tok::RParen | Tok::RBracket => depth -= 1,
        Tok::LBrace if depth == 0 && !prev_hash => return,
        Tok::LBrace => depth += 1,
        Tok::RBrace => depth -= 1,
        Tok::Eof => return,
        _ => {}
      }
      self.bump();
    }
  }

  /// Advance past tokens until `;`, a newline, or EOF at depth 0
  /// (does not consume the terminator)
  pub fn skip_to_terminator(&mut self) {
    let mut depth = 0i32;
    loop {
      let t = self.peek();
      match t.kind {
        Tok::LParen | Tok::LBrace | Tok::LBracket => depth += 1,
        Tok::RParen | Tok::RBrace | Tok::RBracket => depth -= 1,
        Tok::Semi | Tok::Newline | Tok::Eof if depth <= 0 => return,
        Tok::Eof => return,
        _ => {}
      }
      self.bump();
    }
  }

  /// Raw source text between the start of `a` and the start of `b`, trimmed
  pub fn raw_between(&self, a: &Token, b: &Token) -> String {
    self.src[a.start..b.start].trim().to_string()
  }

  /// Raw source text between the start of `a` and the end of `b`, trimmed
  pub fn raw_through(&self, a: &Token, b: &Token) -> String {
    self.src[a.start..b.end].trim().to_string()
  }

  /// Raw source text from a byte offset to the start of `b`, trimmed
  pub fn raw_from(&self, start: usize, b: &Token) -> String {
    self.src[start..b.start].trim().to_string()
  }

  /// A syntax error anchored at the current token
  pub fn error_here(&self, message: impl Into<Box<str>>) -> Error {
    Error::syntax(message).with_span(span_of(self.peek()))
  }

  /// A syntax error anchored at an explicit token
  pub fn error_at(&self, tok: &Token, message: impl Into<Box<str>>) -> Error {
    Error::syntax(message).with_span(span_of(tok))
  }
}

/// The core span covered by a token
pub fn span_of(tok: &Token) -> Span {
  Span::from_positions(
    Position::new(tok.start, tok.line, tok.col),
    Position::new(tok.end, tok.end_line, tok.end_col),
  )
}
