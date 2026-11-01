//! Span types for tracking source locations

use super::kind::Position;
use std::fmt;

/// A span representing a range in source code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
  /// Start position
  pub start: Position,
  /// End position
  pub end: Position,
}

impl Span {
  /// Create a new span
  pub fn new(start_offset: usize, end_offset: usize, line: u32, column: u32) -> Self {
    Self {
      start: Position::new(start_offset, line, column),
      end: Position::new(
        end_offset,
        line,
        column + (end_offset - start_offset) as u32,
      ),
    }
  }

  /// Create a span from two positions
  pub fn from_positions(start: Position, end: Position) -> Self {
    Self { start, end }
  }

  /// Create a span at a single position
  pub fn at(offset: usize, line: u32, column: u32) -> Self {
    let pos = Position::new(offset, line, column);
    Self {
      start: pos,
      end: pos,
    }
  }

  /// Create a zero-length span at the start
  pub fn start() -> Self {
    Self::at(0, 1, 1)
  }

  /// Create a dummy span for generated code
  pub fn dummy() -> Self {
    Self::start()
  }

  /// Get the byte range
  pub fn range(&self) -> std::ops::Range<usize> {
    self.start.offset..self.end.offset
  }

  /// Get the length of the span in bytes
  pub fn len(&self) -> usize {
    self.end.offset.saturating_sub(self.start.offset)
  }

  /// Check if the span is empty
  pub fn is_empty(&self) -> bool {
    self.start.offset == self.end.offset
  }

  /// Check if this span contains a position
  pub fn contains(&self, pos: Position) -> bool {
    pos >= self.start && pos <= self.end
  }

  /// Check if this span contains an offset
  pub fn contains_offset(&self, offset: usize) -> bool {
    offset >= self.start.offset && offset <= self.end.offset
  }

  /// Merge two spans
  pub fn merge(&self, other: &Span) -> Self {
    Self {
      start: std::cmp::min(self.start, other.start),
      end: std::cmp::max(self.end, other.end),
    }
  }
}

impl Default for Span {
  fn default() -> Self {
    Self::dummy()
  }
}

impl fmt::Display for Span {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.start.line == self.end.line {
      write!(f, "{}:{}", self.start.line, self.start.column)
    } else {
      write!(
        f,
        "{}:{}-{}:{}",
        self.start.line, self.start.column, self.end.line, self.end.column
      )
    }
  }
}

/// A token with its span and value
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
  /// Token kind
  pub kind: super::Kind,
  /// Source span
  pub span: Span,
  /// Raw text
  pub text: String,
}

impl Token {
  /// Create a new token
  pub fn new(kind: super::Kind, span: Span, text: impl Into<String>) -> Self {
    Self {
      kind,
      span,
      text: text.into(),
    }
  }

  /// Create a simple token with just kind and text
  pub fn simple(kind: super::Kind, text: impl Into<String>) -> Self {
    Self {
      kind,
      span: Span::dummy(),
      text: text.into(),
    }
  }

  /// Create an EOF token
  pub fn eof() -> Self {
    Self {
      kind: super::Kind::Eof,
      span: Span::dummy(),
      text: String::new(),
    }
  }

  /// Check if this is an EOF token
  pub fn is_eof(&self) -> bool {
    self.kind.is_eof()
  }

  /// Check if this is trivia
  pub fn is_trivia(&self) -> bool {
    self.kind.is_trivia()
  }
}

impl fmt::Display for Token {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.text.is_empty() || self.text.len() > 20 {
      write!(f, "{}", self.kind)
    } else {
      write!(f, "{} \"{}\"", self.kind, self.text)
    }
  }
}

/// A multi-file span that includes file information
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileSpan {
  /// File path or identifier
  pub file: String,
  /// The span within the file
  pub span: Span,
}

impl FileSpan {
  /// Create a new file span
  pub fn new(file: impl Into<String>, span: Span) -> Self {
    Self {
      file: file.into(),
      span,
    }
  }

  /// Create a dummy file span
  pub fn dummy() -> Self {
    Self {
      file: String::new(),
      span: Span::dummy(),
    }
  }
}

impl fmt::Display for FileSpan {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.file.is_empty() {
      write!(f, "{}", self.span)
    } else {
      write!(f, "{}:{}", self.file, self.span)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::token::Kind;

  #[test]
  fn test_span_creation() {
    let span = Span::new(0, 5, 1, 1);
    assert_eq!(span.start.offset, 0);
    assert_eq!(span.end.offset, 5);
    assert_eq!(span.len(), 5);
  }

  #[test]
  fn test_span_merge() {
    let span1 = Span::new(0, 5, 1, 1);
    let span2 = Span::new(10, 15, 2, 1);
    let merged = span1.merge(&span2);
    assert_eq!(merged.start.offset, 0);
    assert_eq!(merged.end.offset, 15);
  }

  #[test]
  fn test_span_contains() {
    let span = Span::new(5, 10, 1, 6);
    let inside = Position::new(7, 1, 8);
    let outside = Position::new(15, 2, 1);
    assert!(span.contains(inside));
    assert!(!span.contains(outside));
  }

  #[test]
  fn test_token() {
    let token = Token::new(Kind::Ident, Span::new(0, 5, 1, 1), "color");
    assert_eq!(token.kind, Kind::Ident);
    assert_eq!(token.text, "color");
    assert!(!token.is_eof());
  }

  #[test]
  fn test_token_eof() {
    let token = Token::eof();
    assert!(token.is_eof());
  }

  #[test]
  fn test_file_span() {
    let span = FileSpan::new("test.vass", Span::new(0, 5, 1, 1));
    assert_eq!(span.to_string(), "test.vass:1:1");
  }
}
