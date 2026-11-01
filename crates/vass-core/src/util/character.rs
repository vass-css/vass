//! Character utilities

/// Check if a character is a valid CSS identifier start
pub fn is_ident_start(c: char) -> bool {
  c == '_' || c == '-' || c.is_alphabetic() || is_non_ascii(c)
}

/// Check if a character is a valid CSS identifier character
pub fn is_ident_char(c: char) -> bool {
  is_ident_start(c) || c.is_ascii_digit()
}

/// Check if a character is a valid CSS name character (including escaped)
pub fn is_name_char(c: char) -> bool {
  is_ident_char(c)
}

/// Check if a character is non-ASCII (valid in CSS identifiers)
pub fn is_non_ascii(c: char) -> bool {
  !c.is_ascii()
}

/// Check if a character is a hex digit
pub fn is_hex_digit(c: char) -> bool {
  c.is_ascii_hexdigit()
}

/// Check if a character is a newline
pub fn is_newline(c: char) -> bool {
  matches!(c, '\n' | '\r' | '\u{000B}' | '\u{000C}')
}

/// Check if a character is whitespace (not including newline)
pub fn is_whitespace(c: char) -> bool {
  matches!(c, ' ' | '\t')
}

/// Check if a character is any whitespace including newline
pub fn is_whitespace_or_newline(c: char) -> bool {
  is_whitespace(c) || is_newline(c)
}

/// Check if a character is a digit
pub fn is_digit(c: char) -> bool {
  c.is_ascii_digit()
}

/// Check if a character is a letter
pub fn is_letter(c: char) -> bool {
  c.is_alphabetic()
}

/// Check if a character is valid in a number
pub fn is_number_char(c: char) -> bool {
  c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-'
}

/// Check if a character is a CSS unit character
pub fn is_unit_char(c: char) -> bool {
  c.is_ascii_lowercase() || c == '%'
}

/// Check if a character is a valid escape character
pub fn is_valid_escape(c: char) -> bool {
  !is_newline(c)
}

/// Check if a character starts a string
pub fn is_string_quote(c: char) -> bool {
  matches!(c, '"' | '\'')
}

/// Check if a character is a decimal point
pub fn is_decimal_point(c: char) -> bool {
  c == '.'
}

/// Check if a character is a sign
pub fn is_sign(c: char) -> bool {
  matches!(c, '+' | '-')
}

/// Check if a character is an exponent marker
pub fn is_exponent(c: char) -> bool {
  matches!(c, 'e' | 'E')
}

/// Convert a hex digit to its numeric value
pub fn hex_digit_value(c: char) -> Option<u32> {
  match c {
    '0'..='9' => Some(c as u32 - '0' as u32),
    'a'..='f' => Some(c as u32 - 'a' as u32 + 10),
    'A'..='F' => Some(c as u32 - 'A' as u32 + 10),
    _ => None,
  }
}

/// Escape a character for CSS
pub fn escape_char(c: char) -> String {
  match c {
    '\0' => "\\0".to_string(),
    c if c.is_ascii() && !c.is_control() => format!("\\{}", c),
    c => format!("\\{:X} ", c as u32),
  }
}

/// Unescape a CSS string
pub fn unescape_string(s: &str) -> String {
  let mut result = String::new();
  let mut chars = s.chars().peekable();

  while let Some(c) = chars.next() {
    if c == '\\' {
      if let Some(&next) = chars.peek() {
        if is_newline(next) {
          // Skip escaped newline
          chars.next();
          if next == '\r' && chars.peek() == Some(&'\n') {
            chars.next();
          }
        } else if next.is_ascii_hexdigit() {
          // Hex escape
          chars.next();
          let mut hex = String::new();
          hex.push(next);
          for _ in 0..5 {
            let Some(h) = chars.next_if(|h| h.is_ascii_hexdigit()) else {
              break;
            };
            hex.push(h);
          }
          if let Ok(code) = u32::from_str_radix(&hex, 16)
            && let Some(ch) = char::from_u32(code)
          {
            result.push(ch);
          }
          // Skip trailing space after hex escape
          if chars.peek() == Some(&' ') {
            chars.next();
          }
        } else {
          // Regular escape
          chars.next();
          result.push(next);
        }
      }
    } else {
      result.push(c);
    }
  }

  result
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_ident_start() {
    assert!(is_ident_start('a'));
    assert!(is_ident_start('Z'));
    assert!(is_ident_start('_'));
    assert!(is_ident_start('-'));
    assert!(is_ident_start('é'));
    assert!(!is_ident_start('0'));
    assert!(!is_ident_start('@'));
  }

  #[test]
  fn test_is_ident_char() {
    assert!(is_ident_char('a'));
    assert!(is_ident_char('0'));
    assert!(is_ident_char('_'));
    assert!(is_ident_char('-'));
    assert!(!is_ident_char('@'));
    assert!(!is_ident_char(' '));
  }

  #[test]
  fn test_is_hex_digit() {
    assert!(is_hex_digit('0'));
    assert!(is_hex_digit('a'));
    assert!(is_hex_digit('F'));
    assert!(!is_hex_digit('g'));
    assert!(!is_hex_digit('G'));
  }

  #[test]
  fn test_hex_digit_value() {
    assert_eq!(hex_digit_value('0'), Some(0));
    assert_eq!(hex_digit_value('a'), Some(10));
    assert_eq!(hex_digit_value('F'), Some(15));
    assert_eq!(hex_digit_value('g'), None);
  }

  #[test]
  fn test_is_newline() {
    assert!(is_newline('\n'));
    assert!(is_newline('\r'));
    assert!(!is_newline(' '));
    assert!(!is_newline('a'));
  }

  #[test]
  fn test_unescape_string() {
    assert_eq!(unescape_string("hello"), "hello");
    assert_eq!(unescape_string("\\n"), "n");
    assert_eq!(unescape_string("\\41"), "A");
    assert_eq!(unescape_string("\\41 B"), "AB");
  }

  #[test]
  fn test_is_name_char() {
    assert!(is_name_char('a'));
    assert!(is_name_char('0'));
    assert!(is_name_char('_'));
    assert!(!is_name_char('@'));
  }

  #[test]
  fn test_is_digit_and_letter() {
    assert!(is_digit('5'));
    assert!(!is_digit('a'));
    assert!(is_letter('a'));
    assert!(!is_letter('5'));
  }

  #[test]
  fn test_is_number_char() {
    assert!(is_number_char('1'));
    assert!(is_number_char('.'));
    assert!(is_number_char('e'));
    assert!(is_number_char('E'));
    assert!(is_number_char('+'));
    assert!(is_number_char('-'));
    assert!(!is_number_char('a'));
  }

  #[test]
  fn test_is_unit_char() {
    assert!(is_unit_char('p'));
    assert!(is_unit_char('x'));
    assert!(is_unit_char('%'));
    assert!(!is_unit_char('P'));
    assert!(!is_unit_char(' '));
  }

  #[test]
  fn test_is_valid_escape() {
    assert!(is_valid_escape('a'));
    assert!(!is_valid_escape('\n'));
    assert!(!is_valid_escape('\r'));
  }

  #[test]
  fn test_is_string_quote() {
    assert!(is_string_quote('"'));
    assert!(is_string_quote('\''));
    assert!(!is_string_quote('`'));
  }

  #[test]
  fn test_is_decimal_point_and_sign_and_exponent() {
    assert!(is_decimal_point('.'));
    assert!(!is_decimal_point(','));
    assert!(is_sign('+'));
    assert!(is_sign('-'));
    assert!(!is_sign('*'));
    assert!(is_exponent('e'));
    assert!(is_exponent('E'));
    assert!(!is_exponent('x'));
  }

  #[test]
  fn test_escape_char() {
    assert_eq!(escape_char('\0'), "\\0");
    assert_eq!(escape_char('a'), "\\a");
    assert_eq!(escape_char('é'), "\\E9 ");
  }

  #[test]
  fn test_unescape_string_newline_and_hex() {
    assert_eq!(unescape_string("a\\\nb"), "ab");
    assert_eq!(unescape_string("a\\\r\nb"), "ab");
    assert_eq!(unescape_string("\\41"), "A");
    assert_eq!(unescape_string("\\41 B"), "AB");
    assert_eq!(unescape_string("\\n"), "n");
    assert_eq!(unescape_string("plain"), "plain");
  }
}
