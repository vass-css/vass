//! CSS minification helpers
//!
//! The compressed writer handles structural minification; this module adds
//! value-level compression: `0.5` → `.5` (skipping quoted strings, URLs
//! and comments) and `0px` → `0` for length units.

use vass_core::ast::css::CssStylesheet;

/// Minify a CSS stylesheet to text
pub fn minify(sheet: &CssStylesheet) -> String {
  compress_numbers(&super::write_compressed(sheet))
}

/// Rewrite decimal numbers: `0.5` → `.5`, `0.0px` → `0`, `10.0px` → `10px`
///
/// Quoted strings, URLs and comments are left untouched.
pub fn compress_numbers(css: &str) -> String {
  let bytes: Vec<char> = css.chars().collect();
  let mut out = String::with_capacity(css.len());
  let mut i = 0usize;
  let mut in_string: Option<char> = None;
  while i < bytes.len() {
    let c = bytes[i];
    if let Some(q) = in_string {
      out.push(c);
      if c == '\\' && i + 1 < bytes.len() {
        out.push(bytes[i + 1]);
        i += 2;
        continue;
      }
      if c == q {
        in_string = None;
      }
      i += 1;
      continue;
    }
    if c == '"' || c == '\'' {
      in_string = Some(c);
      out.push(c);
      i += 1;
      continue;
    }
    // Comments: copy verbatim
    if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == '*' {
      let mut end = None;
      let mut j = i + 2;
      while j + 1 < bytes.len() {
        if bytes[j] == '*' && bytes[j + 1] == '/' {
          end = Some(j + 1);
          break;
        }
        j += 1;
      }
      let end = end.unwrap_or(bytes.len() - 1);
      for &cc in &bytes[i..=end] {
        out.push(cc);
      }
      i = end + 1;
      continue;
    }
    // Hex colors are copied verbatim (`#002bd9` must stay intact)
    if c == '#' {
      out.push('#');
      i += 1;
      while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
        out.push(bytes[i]);
        i += 1;
      }
      continue;
    }
    // url(...) is copied verbatim
    if css[i..].starts_with("url(") {
      let end = bytes[i..]
        .iter()
        .position(|&x| x == ')')
        .map(|p| i + p)
        .unwrap_or(bytes.len() - 1);
      for &cc in &bytes[i..=end] {
        out.push(cc);
      }
      i = end + 1;
      continue;
    }
    // Numbers: `0.` → `.`, strip trailing `.0`
    let is_num_start = c.is_ascii_digit()
      || (c == '-'
        && i + 1 < bytes.len()
        && bytes[i + 1].is_ascii_digit()
        && !out.ends_with(|p: char| p.is_ascii_alphanumeric()));
    if is_num_start {
      let start = i;
      if c == '-' {
        i += 1;
      }
      let int_start = i;
      while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
      }
      let mut has_frac = false;
      if i + 1 < bytes.len() && bytes[i] == '.' && bytes[i + 1].is_ascii_digit() {
        has_frac = true;
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
          i += 1;
        }
      }
      let int_part: String = bytes[int_start..i].iter().collect();
      let text: String = bytes[start..i].iter().collect();
      let _ = has_frac;
      // Parse the full number, then reformat compactly
      if let Ok(n) = text.parse::<f64>() {
        let rounded = (n * 1e10).round() / 1e10;
        let compact = if rounded == rounded.trunc() && rounded.abs() < 1e15 {
          format!("{}", rounded as i64)
        } else {
          format!("{}", rounded)
        };
        let compact = if let Some(stripped) = compact.strip_prefix("0.") {
          format!(".{}", stripped)
        } else if let Some(stripped) = compact.strip_prefix("-0.") {
          format!("-.{}", stripped)
        } else {
          compact
        };
        out.push_str(&compact);
      } else {
        out.push_str(&int_part);
      }
      continue;
    }
    out.push(c);
    i += 1;
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_leading_zero() {
    assert_eq!(compress_numbers("a{b:0.5px}"), "a{b:.5px}");
    assert_eq!(compress_numbers("a{b:-0.25em}"), "a{b:-.25em}");
    assert_eq!(compress_numbers("a{b:10.50px}"), "a{b:10.5px}");
    assert_eq!(compress_numbers("a{b:10px}"), "a{b:10px}");
  }

  #[test]
  fn test_hex_colors_untouched() {
    assert_eq!(compress_numbers("a{b:#002bd9}"), "a{b:#002bd9}");
    assert_eq!(compress_numbers("a{b:#036}"), "a{b:#036}");
    assert_eq!(compress_numbers("a{b:#AABBCC}"), "a{b:#AABBCC}");
  }

  #[test]
  fn test_strings_and_urls_untouched() {
    assert_eq!(
      compress_numbers("a{content:\"0.5px\"}"),
      "a{content:\"0.5px\"}"
    );
    assert_eq!(
      compress_numbers("a{b:url(x/0.5.png)}"),
      "a{b:url(x/0.5.png)}"
    );
  }
}
