//! String values

use std::fmt;

/// A VASS string: quoted (`"hello"`) or unquoted (`hello`)
#[derive(Debug, Clone, PartialEq)]
pub struct BassString {
  /// The string content (without quotes)
  pub text: String,
  /// Whether the string is quoted in CSS output
  pub quoted: bool,
}

impl BassString {
  /// Create a new string value
  pub fn new(text: impl Into<String>, quoted: bool) -> Self {
    Self {
      text: text.into(),
      quoted,
    }
  }

  /// Create an unquoted string (identifiers, function names, ...)
  pub fn unquoted(text: impl Into<String>) -> Self {
    Self::new(text, false)
  }

  /// Create a quoted string
  pub fn quoted(text: impl Into<String>) -> Self {
    Self::new(text, true)
  }

  /// Render as CSS, adding quotes if needed
  pub fn to_css(&self) -> String {
    if self.quoted {
      self.to_css_quoted()
    } else {
      self.text.clone()
    }
  }

  /// Serialize a quoted string the way the reference compiler does:
  /// prefer the quote style absent from the content, escape backslashes
  /// and the active quote, and escape characters CSS cannot represent
  /// literally — line feeds, C0 controls (except tab), DEL and the
  /// Private Use Area (`content: "\f112"` round-trips as an escape).
  fn to_css_quoted(&self) -> String {
    // The reference prefers the quote style absent from the string:
    // `"with \"q\""` serializes as `'with "q"'`. Only strings that
    // contain both quote styles keep double quotes with escapes.
    let force_double = self.text.contains('"') && self.text.contains('\'');
    let (open, close) = if force_double || !self.text.contains('"') {
      ('"', '"')
    } else {
      ('\'', '\'')
    };
    let chars: Vec<char> = self.text.chars().collect();
    let mut out = String::with_capacity(self.text.len() + 2);
    out.push(open);
    for (i, &c) in chars.iter().enumerate() {
      match c {
        '\\' => out.push_str("\\\\"),
        c if c == open => {
          if force_double {
            out.push('\\');
          }
          out.push(c);
        }
        '\n' => out.push_str("\\a"),
        '\t' => out.push('\t'),
        c if c <= '\u{1F}' || c == '\u{7F}' => {
          push_css_escape(&mut out, c, chars.get(i + 1).copied())
        }
        c if ('\u{E000}'..='\u{F8FF}').contains(&c) => {
          push_css_escape(&mut out, c, chars.get(i + 1).copied())
        }
        c => out.push(c),
      }
    }
    out.push(close);
    out
  }
}

/// Write `\xxxx` (lowercase hex) for a character that cannot appear
/// literally in a CSS string; a terminator space is required when the next
/// character would otherwise continue the escape (hex digit or whitespace).
fn push_css_escape(out: &mut String, c: char, next: Option<char>) {
  out.push('\\');
  out.push_str(&format!("{:x}", c as u32));
  if let Some(n) = next
    && (n.is_ascii_hexdigit() || n == ' ' || n == '\t')
  {
    out.push(' ');
  }
}

impl fmt::Display for BassString {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.to_css())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_to_css() {
    assert_eq!(BassString::unquoted("solid").to_css(), "solid");
    assert_eq!(BassString::quoted("hello").to_css(), "\"hello\"");
    // Reference behavior: a string containing double quotes (but no
    // single quotes) is serialized with single quotes instead of
    // backslash-escaping.
    assert_eq!(BassString::quoted("a\"b").to_css(), "'a\"b'");
    // Both quote styles present -> double quotes with escapes
    assert_eq!(BassString::quoted("a\"b'c").to_css(), "\"a\\\"b'c\"");
    // Backslashes are re-escaped
    assert_eq!(BassString::quoted("a\\b").to_css(), "\"a\\\\b\"");
    // Private Use Area characters are escaped like the reference does
    assert_eq!(BassString::quoted("\u{f112}").to_css(), "\"\\f112\"");
    assert_eq!(BassString::quoted("\u{e000}").to_css(), "\"\\e000\"");
    // Non-PUA symbols stay literal
    assert_eq!(BassString::quoted("\u{2192}").to_css(), "\"\u{2192}\"");
    // Line feed becomes the \a escape
    assert_eq!(BassString::quoted("a\nb").to_css(), "\"a\\ab\"");
    // An escape followed by a hex digit needs a terminator space
    assert_eq!(BassString::quoted("\u{f112}a").to_css(), "\"\\f112 a\"");
  }
}
