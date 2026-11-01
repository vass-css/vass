//! String utilities

use smallvec::SmallVec;

/// A small string buffer for efficient string operations
pub type SmallString = SmallVec<[u8; 32]>;

/// Quote a string for CSS output
pub fn quote_string(s: &str, quote: char) -> String {
  let mut result = String::with_capacity(s.len() + 2);
  result.push(quote);

  for c in s.chars() {
    match c {
      '\\' => result.push_str("\\\\"),
      c if c == quote => {
        result.push('\\');
        result.push(c);
      }
      '\n' => result.push_str("\\a "),
      '\r' => result.push_str("\\d "),
      '\t' => result.push_str("\\9 "),
      c if c.is_control() => {
        result.push_str(&format!("\\{:x} ", c as u32));
      }
      c => result.push(c),
    }
  }

  result.push(quote);
  result
}

/// Unquote a CSS string
pub fn unquote_string(s: &str) -> Option<(String, char)> {
  let chars: Vec<char> = s.chars().collect();
  if chars.len() < 2 {
    return None;
  }

  let first = chars[0];
  let last = chars[chars.len() - 1];

  if (first == '"' || first == '\'') && first == last {
    let inner: String = chars[1..chars.len() - 1].iter().collect();
    Some((unescape_css_string(&inner), first))
  } else {
    None
  }
}

/// Unescape a CSS string (internal content)
fn unescape_css_string(s: &str) -> String {
  let mut result = String::new();
  let mut chars = s.chars().peekable();

  while let Some(c) = chars.next() {
    if c == '\\' {
      if let Some(&next) = chars.peek() {
        match next {
          'n' | 'N' => {
            chars.next();
            result.push('\n');
          }
          'r' | 'R' => {
            chars.next();
            result.push('\r');
          }
          't' | 'T' => {
            chars.next();
            result.push('\t');
          }
          'a'..='f' | 'A'..='F' | '0'..='9' => {
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
            // Skip single trailing space or newline
            if let Some(&' ') = chars.peek() {
              chars.next();
            }
            if let Ok(code) = u32::from_str_radix(&hex, 16)
              && let Some(ch) = char::from_u32(code)
            {
              result.push(ch);
            }
          }
          _ => {
            // Regular escape
            chars.next();
            result.push(next);
          }
        }
      }
    } else {
      result.push(c);
    }
  }

  result
}

/// Check if a string is a valid CSS identifier
pub fn is_valid_ident(s: &str) -> bool {
  let mut chars = s.chars();

  match chars.next() {
    Some(c) if super::character::is_ident_start(c) => {}
    _ => return false,
  }

  chars.all(super::character::is_ident_char)
}

/// Check if a string needs quoting for CSS output
pub fn needs_quoting(s: &str) -> bool {
  if s.is_empty() {
    return true;
  }

  let mut chars = s.chars();

  // Check first character
  match chars.next() {
    // A single leading '-' is only valid if followed by an ident start char
    // or another '-' (e.g., custom properties like --my-var).
    Some('-') => match chars.next() {
      Some('-') => {}                                      // custom property, valid
      Some(c) if super::character::is_ident_start(c) => {} // vendor prefix, valid
      _ => return true,                                    // "-123", "-", etc. need quoting
    },
    Some(c) if c.is_ascii_digit() => return true,
    Some(c) if !super::character::is_ident_start(c) => return true,
    _ => {}
  }

  // Check remaining characters
  for c in chars {
    if !super::character::is_ident_char(c) {
      return true;
    }
  }

  false
}

/// Convert a string to a valid CSS identifier
pub fn to_css_ident(s: &str) -> String {
  let mut result = String::new();
  let mut first = true;

  for c in s.chars() {
    if first {
      if super::character::is_ident_start(c) {
        result.push(c);
      } else if c.is_ascii_digit() {
        result.push_str(&format!("\\3{} ", c));
      } else {
        result.push_str(&super::character::escape_char(c));
      }
      first = false;
    } else if super::character::is_ident_char(c) {
      result.push(c);
    } else {
      result.push_str(&super::character::escape_char(c));
    }
  }

  result
}

/// Trim leading and trailing whitespace from a string
pub fn trim_whitespace(s: &str) -> &str {
  s.trim_matches(|c: char| c.is_whitespace() || super::character::is_newline(c))
}

/// Check if a string starts with a newline
pub fn starts_with_newline(s: &str) -> bool {
  s.starts_with('\n') || s.starts_with("\r\n")
}

/// Normalize newlines in a string
pub fn normalize_newlines(s: &str) -> String {
  s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Count the number of newlines in a string
pub fn count_newlines(s: &str) -> usize {
  s.matches('\n').count()
}

/// Get the indentation level of a line
pub fn get_indentation(line: &str) -> usize {
  line.chars().take_while(|&c| c == ' ' || c == '\t').count()
}

/// Check if a string is a valid CSS color name
pub fn is_color_name(s: &str) -> bool {
  // Common CSS color names
  matches!(
    s.to_lowercase().as_str(),
    "aliceblue"
      | "antiquewhite"
      | "aqua"
      | "aquamarine"
      | "azure"
      | "beige"
      | "bisque"
      | "black"
      | "blanchedalmond"
      | "blue"
      | "blueviolet"
      | "brown"
      | "burlywood"
      | "cadetblue"
      | "chartreuse"
      | "chocolate"
      | "coral"
      | "cornflowerblue"
      | "cornsilk"
      | "crimson"
      | "cyan"
      | "darkblue"
      | "darkcyan"
      | "darkgoldenrod"
      | "darkgray"
      | "darkgreen"
      | "darkgrey"
      | "darkkhaki"
      | "darkmagenta"
      | "darkolivegreen"
      | "darkorange"
      | "darkorchid"
      | "darkred"
      | "darksalmon"
      | "darkseagreen"
      | "darkslateblue"
      | "darkslategray"
      | "darkslategrey"
      | "darkturquoise"
      | "darkviolet"
      | "deeppink"
      | "deepskyblue"
      | "dimgray"
      | "dimgrey"
      | "dodgerblue"
      | "firebrick"
      | "floralwhite"
      | "forestgreen"
      | "fuchsia"
      | "gainsboro"
      | "ghostwhite"
      | "gold"
      | "goldenrod"
      | "gray"
      | "green"
      | "greenyellow"
      | "grey"
      | "honeydew"
      | "hotpink"
      | "indianred"
      | "indigo"
      | "ivory"
      | "khaki"
      | "lavender"
      | "lavenderblush"
      | "lawngreen"
      | "lemonchiffon"
      | "lightblue"
      | "lightcoral"
      | "lightcyan"
      | "lightgoldenrodyellow"
      | "lightgray"
      | "lightgreen"
      | "lightgrey"
      | "lightpink"
      | "lightsalmon"
      | "lightseagreen"
      | "lightskyblue"
      | "lightslategray"
      | "lightslategrey"
      | "lightsteelblue"
      | "lightyellow"
      | "lime"
      | "limegreen"
      | "linen"
      | "magenta"
      | "maroon"
      | "mediumaquamarine"
      | "mediumblue"
      | "mediumorchid"
      | "mediumpurple"
      | "mediumseagreen"
      | "mediumslateblue"
      | "mediumspringgreen"
      | "mediumturquoise"
      | "mediumvioletred"
      | "midnightblue"
      | "mintcream"
      | "mistyrose"
      | "moccasin"
      | "navajowhite"
      | "navy"
      | "oldlace"
      | "olive"
      | "olivedrab"
      | "orange"
      | "orangered"
      | "orchid"
      | "palegoldenrod"
      | "palegreen"
      | "paleturquoise"
      | "palevioletred"
      | "papayawhip"
      | "peachpuff"
      | "peru"
      | "pink"
      | "plum"
      | "powderblue"
      | "purple"
      | "rebeccapurple"
      | "red"
      | "rosybrown"
      | "royalblue"
      | "saddlebrown"
      | "salmon"
      | "sandybrown"
      | "seagreen"
      | "seashell"
      | "sienna"
      | "silver"
      | "skyblue"
      | "slateblue"
      | "slategray"
      | "slategrey"
      | "snow"
      | "springgreen"
      | "steelblue"
      | "tan"
      | "teal"
      | "thistle"
      | "tomato"
      | "turquoise"
      | "violet"
      | "wheat"
      | "white"
      | "whitesmoke"
      | "yellow"
      | "yellowgreen"
      | "transparent"
      | "currentcolor"
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_quote_string() {
    assert_eq!(quote_string("hello", '"'), "\"hello\"");
    assert_eq!(quote_string("hello\"world", '"'), "\"hello\\\"world\"");
    assert_eq!(quote_string("hello\nworld", '\''), "'hello\\a world'");
  }

  #[test]
  fn test_unquote_string() {
    assert_eq!(
      unquote_string("\"hello\""),
      Some(("hello".to_string(), '"'))
    );
    assert_eq!(
      unquote_string("\'hello\'"),
      Some(("hello".to_string(), '\''))
    );
    assert_eq!(unquote_string("hello"), None);
  }

  #[test]
  fn test_is_valid_ident() {
    assert!(is_valid_ident("color"));
    assert!(is_valid_ident("-webkit-transition"));
    assert!(is_valid_ident("_private"));
    assert!(!is_valid_ident("123color"));
    assert!(!is_valid_ident("@import"));
  }

  #[test]
  fn test_needs_quoting() {
    assert!(needs_quoting("123"));
    assert!(needs_quoting("hello world"));
    assert!(needs_quoting("-123"));
    assert!(needs_quoting("-"));
    assert!(!needs_quoting("color"));
    assert!(!needs_quoting("-webkit-transition"));
    assert!(!needs_quoting("--my-var"));
  }

  #[test]
  fn test_to_css_ident() {
    assert_eq!(to_css_ident("color"), "color");
    assert_eq!(to_css_ident("123"), "\\31 23");
  }

  #[test]
  fn test_normalize_newlines() {
    assert_eq!(normalize_newlines("a\r\nb\rc\n"), "a\nb\nc\n");
  }

  #[test]
  fn test_count_newlines() {
    assert_eq!(count_newlines("a\nb\nc\n"), 3);
    assert_eq!(count_newlines("hello"), 0);
  }

  #[test]
  fn test_get_indentation() {
    assert_eq!(get_indentation("  hello"), 2);
    assert_eq!(get_indentation("\thello"), 1);
    assert_eq!(get_indentation("hello"), 0);
  }

  #[test]
  fn test_is_color_name() {
    assert!(is_color_name("red"));
    assert!(is_color_name("RED"));
    assert!(is_color_name("transparent"));
    assert!(!is_color_name("notacolor"));
  }
}
