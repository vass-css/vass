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
