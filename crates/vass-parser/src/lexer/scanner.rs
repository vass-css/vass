//! Low-level character scanner with position tracking

/// A saved position in the scan, for backtracking
#[derive(Debug, Clone, Copy)]
pub struct Marker {
  index: usize,
  byte: usize,
  line: u32,
  col: u32,
}

/// A cursor over source characters.
///
/// Tracks byte offsets (for raw source slicing) plus line/column for
/// diagnostics.
pub struct Scanner<'a> {
  src: &'a str,
  chars: Vec<(usize, char)>, // (byte offset, char)
  index: usize,
  end_byte: usize,
  line: u32,
  col: u32,
}

impl<'a> Scanner<'a> {
  /// Create a scanner over `src`
  pub fn new(src: &'a str) -> Self {
    let chars: Vec<(usize, char)> = src.char_indices().collect();
    let end_byte = src.len();
    Self {
      src,
      chars,
      index: 0,
      end_byte,
      line: 1,
      col: 1,
    }
  }

  /// The full source text
  pub fn source(&self) -> &'a str {
    self.src
  }

  /// Current byte offset
  pub fn offset(&self) -> usize {
    match self.chars.get(self.index) {
      Some(&(byte, _)) => byte,
      None => self.end_byte,
    }
  }

  /// Current line (1-based)
  pub fn line(&self) -> u32 {
    self.line
  }

  /// Current column (1-based, in characters)
  pub fn col(&self) -> u32 {
    self.col
  }

  /// Peek at the current character without consuming it
  pub fn peek(&self) -> Option<char> {
    self.chars.get(self.index).map(|&(_, c)| c)
  }

  /// Peek at the character after the current one
  pub fn peek2(&self) -> Option<char> {
    self.chars.get(self.index + 1).map(|&(_, c)| c)
  }

  /// Peek two characters ahead of the current one
  pub fn peek3(&self) -> Option<char> {
    self.chars.get(self.index + 2).map(|&(_, c)| c)
  }

  /// Consume and return the current character
  pub fn bump(&mut self) -> Option<char> {
    let c = self.peek()?;
    self.index += 1;
    if c == '\n' {
      self.line += 1;
      self.col = 1;
    } else {
      self.col += 1;
    }
    Some(c)
  }

  /// Save the current position
  pub fn mark(&self) -> Marker {
    Marker {
      index: self.index,
      byte: self.offset(),
      line: self.line,
      col: self.col,
    }
  }

  /// Restore a saved position
  pub fn reset(&mut self, marker: Marker) {
    self.index = marker.index;
    self.line = marker.line;
    self.col = marker.col;
  }

  /// Consume `n` characters
  pub fn advance(&mut self, n: usize) {
    for _ in 0..n {
      if self.bump().is_none() {
        break;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic_scan() {
    let mut s = Scanner::new("ab\ncd");
    assert_eq!(s.peek(), Some('a'));
    assert_eq!(s.offset(), 0);
    s.bump();
    s.bump();
    assert_eq!(s.peek(), Some('\n'));
    s.bump();
    assert_eq!(s.line(), 2);
    assert_eq!(s.col(), 1);
    assert_eq!(s.peek(), Some('c'));
    assert_eq!(s.offset(), 3);
  }

  #[test]
  fn test_mark_reset() {
    let mut s = Scanner::new("hello");
    let m = s.mark();
    s.advance(3);
    assert_eq!(s.peek(), Some('l'));
    s.reset(m);
    assert_eq!(s.peek(), Some('h'));
  }
}
