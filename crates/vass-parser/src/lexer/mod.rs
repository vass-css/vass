//! Lexer module
//!
//! Turns source text into a flat token stream. Whitespace between tokens is
//! skipped (except it drives indentation tracking for the indented syntax);
//! newlines and comments are emitted as tokens so the parsers can use them.

pub mod scanner;
pub mod tokenizer;

pub use scanner::Scanner;
pub use tokenizer::{Tok, Token, tokenize};

use vass_core::error::Error;
use vass_core::token::{Position, Span};

/// Build a core [`Span`] from a token's position information
pub(crate) fn token_span(tok: &Token) -> Span {
  Span::from_positions(
    Position::new(tok.start, tok.line, tok.col),
    Position::new(tok.end, tok.end_line, tok.end_col),
  )
}

/// Convenience: a syntax error located at a token
pub(crate) fn error_at(tok: &Token, message: impl Into<Box<str>>) -> Error {
  Error::syntax(message).with_span(token_span(tok))
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::token::Position;

  #[test]
  fn test_token_span() {
    let tok = Token {
      kind: Tok::Ident,
      text: "hello".to_string(),
      start: 0,
      end: 5,
      line: 1,
      col: 1,
      end_line: 1,
      end_col: 6,
      indent: 0,
      ws_before: false,
    };
    let span = token_span(&tok);
    assert_eq!(span.start.offset, 0);
    assert_eq!(span.start.line, 1);
    assert_eq!(span.start.column, 1);
    assert_eq!(span.end.offset, 5);
    assert_eq!(span.end.line, 1);
    assert_eq!(span.end.column, 6);
  }

  #[test]
  fn test_error_at() {
    let tok = Token {
      kind: Tok::Hash,
      text: "#".to_string(),
      start: 10,
      end: 11,
      line: 3,
      col: 5,
      end_line: 3,
      end_col: 6,
      indent: 0,
      ws_before: true,
    };
    let err = error_at(&tok, "unexpected character".to_string());
    assert!(err.message.contains("unexpected character"));
    assert!(err.span.is_some());
    assert_eq!(err.span.unwrap().start.line, 3);
  }

  #[test]
  fn test_position_new() {
    let p = Position::new(42, 5, 3);
    assert_eq!(p.offset, 42);
    assert_eq!(p.line, 5);
    assert_eq!(p.column, 3);
  }

  #[test]
  fn test_position_start() {
    let p = Position::start();
    assert_eq!(p.offset, 0);
    assert_eq!(p.line, 1);
    assert_eq!(p.column, 1);
  }
}
