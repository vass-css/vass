//! Document formatting: a conservative, text-level re-indenter that works on
//! both syntaxes and never changes program semantics.
//!
//! * `.vcss` (braces): normalize indentation by brace depth, `prop: value;`,
//!   `selector {`, collapse blank runs, trim trailing whitespace.
//! * `.vass` (indented): keep the indentation structure (leading tabs count as
//!   one level each, two spaces per level), trim trailing whitespace,
//!   collapse blank runs.

/// Format a document. `braces` selects the `.vcss` style; `false` formats the
/// indented `.vass` style.
pub fn format_source(source: &str, braces: bool) -> String {
  if braces {
    format_braces(source)
  } else {
    format_indented(source)
  }
}

/// Does this pending line end a complete `prop: value` statement? Requires a
/// property-like prefix before a top-level `:`, a non-empty value, no open
/// brackets and no trailing continuation character (`:`, `,`, operator).
fn completes_decl(line: &str) -> bool {
  let b = line.as_bytes();
  let mut in_str: Option<u8> = None;
  let mut paren = 0i32;
  let mut i = 0;
  while i < b.len() {
    let c = b[i];
    match in_str {
      Some(q) => {
        if c == b'\\' {
          i += 2;
          continue;
        }
        if c == q {
          in_str = None;
        }
      }
      None => match c {
        b'"' | b'\'' => in_str = Some(c),
        b'(' => paren += 1,
        b')' => paren -= 1,
        b':' if paren == 0 => {
          let prefix = &line[..i];
          let prop_like = !prefix.is_empty()
            && prefix
              .chars()
              .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
          if !prop_like {
            return false;
          }
          let rest = line[i + 1..].trim_end();
          if rest.is_empty() {
            return false;
          }
          // a trailing operator/comma/colon keeps the value going
          if matches!(
            rest.as_bytes()[rest.len() - 1],
            b':' | b',' | b'+' | b'-' | b'*' | b'/' | b'(' | b'[' | b'{'
          ) {
            return false;
          }
          // unclosed brackets after the colon keep the value going
          let mut in_str2: Option<u8> = None;
          let mut open = 0i32;
          let mut j = i + 1;
          while j < b.len() {
            let c2 = b[j];
            match in_str2 {
              Some(q) => {
                if c2 == b'\\' {
                  j += 2;
                  continue;
                }
                if c2 == q {
                  in_str2 = None;
                }
              }
              None => match c2 {
                b'"' | b'\'' => in_str2 = Some(c2),
                b'(' | b'[' | b'{' => open += 1,
                b')' | b']' | b'}' => open -= 1,
                _ => {}
              },
            }
            j += 1;
          }
          return open <= 0;
        }
        _ => {}
      },
    }
    i += 1;
  }
  false
}

/// Whether the next non-blank line after offset `i` starts something that
/// looks like a `prop:` declaration — the signature of a missing `;`.
fn next_line_looks_like_decl(b: &[u8], mut i: usize) -> bool {
  loop {
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\r') {
      i += 1;
    }
    if i >= b.len() {
      return false;
    }
    if b[i] == b'\n' {
      i += 1; // blank line — keep looking
      continue;
    }
    let start = i;
    while i < b.len() && b[i] != b'\n' {
      i += 1;
    }
    return completes_decl(&String::from_utf8_lossy(&b[start..i]));
  }
}

/// Normalize `prop:value` to `prop: value` on a declaration line.
/// Only the first `:` outside strings/parens is considered, and only when the
/// prefix looks like a property name — selectors like `.a:hover` are untouched.
fn normalize_decl(line: &str) -> String {
  let b = line.as_bytes();
  let mut in_str: Option<u8> = None;
  let mut paren = 0i32;
  for (i, &c) in b.iter().enumerate() {
    match in_str {
      Some(q) => {
        if c == b'\\' {
          continue;
        }
        if c == q {
          in_str = None;
        }
      }
      None => match c {
        b'"' | b'\'' => in_str = Some(c),
        b'(' => paren += 1,
        b')' => paren -= 1,
        b':' if paren == 0 => {
          let prefix = &line[..i];
          let prop = prefix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
          if prop && !prefix.is_empty() {
            let mut rest = &line[i + 1..];
            while rest.starts_with(' ') || rest.starts_with('\t') {
              rest = &rest[1..];
            }
            return format!("{prefix}: {rest}");
          }
          return line.to_string();
        }
        _ => {}
      },
    }
  }
  line.to_string()
}

struct Writer {
  out: String,
  depth: usize,
  line: String,
  pending_blank: bool,
  just_emitted: bool,
  /// skip leading whitespace of a continuation line
  skip_ws: bool,
}

impl Writer {
  fn indent(&self) -> String {
    "  ".repeat(self.depth)
  }

  /// Append pending statement text as its own line; `is_decl` selects
  /// `prop: value` normalization. Adds a trailing `;` if missing.
  fn emit_pending(&mut self, is_decl: bool) {
    let pending = std::mem::take(&mut self.line);
    let mut t = pending.trim().to_string();
    if t.is_empty() {
      return;
    }
    if is_decl {
      t = normalize_decl(&t);
      if !t.ends_with(';') {
        t.push(';');
      }
    }
    if self.pending_blank && !self.out.is_empty() {
      self.out.push('\n');
    }
    self.pending_blank = false;
    self.out.push_str(&self.indent());
    self.out.push_str(&t);
    self.out.push('\n');
    self.just_emitted = true;
  }

  /// Emit a raw (already final) line such as a comment
  fn emit_raw(&mut self, text: &str) {
    if self.pending_blank && !self.out.is_empty() {
      self.out.push('\n');
    }
    self.pending_blank = false;
    self.out.push_str(&self.indent());
    self.out.push_str(text);
    self.out.push('\n');
    self.just_emitted = true;
  }
}

/// Formatter for the braced syntax.
fn format_braces(source: &str) -> String {
  let mut w = Writer {
    out: String::new(),
    depth: 0,
    line: String::new(),
    pending_blank: false,
    just_emitted: false,
    skip_ws: false,
  };
  let b = source.as_bytes();
  let mut i = 0;

  while i < b.len() {
    let c = b[i];
    i += 1;
    match c {
      b'"' | b'\'' => {
        w.line.push(c as char);
        while i < b.len() {
          let s = b[i];
          i += 1;
          w.line.push(s as char);
          if s == b'\\' {
            if i < b.len() {
              w.line.push(b[i] as char);
              i += 1;
            }
            continue;
          }
          if s == c {
            break;
          }
        }
        w.just_emitted = false;
      }
      b'/' if i < b.len() && b[i] == b'/' => {
        // line comment: keep it on the same line as any pending code
        let pending = std::mem::take(&mut w.line);
        let t = pending.trim().to_string();
        i += 1; // skip second '/'
        let mut comment = String::from("//");
        while i < b.len() && b[i] != b'\n' {
          comment.push(b[i] as char);
          i += 1;
        }
        let comment = comment.trim_end().to_string();
        let full = if t.is_empty() {
          comment
        } else {
          format!("{t} {comment}")
        };
        if w.pending_blank && !w.out.is_empty() {
          w.out.push('\n');
          w.pending_blank = false;
        }
        w.out.push_str(&w.indent());
        w.out.push_str(&full);
        w.out.push('\n');
        w.just_emitted = true;
      }
      b'/' if i < b.len() && b[i] == b'*' => {
        i += 1; // skip '*'
        let mut comment = String::from("/*");
        while i < b.len() {
          if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
            i += 2;
            comment.push_str("*/");
            break;
          }
          comment.push(b[i] as char);
          i += 1;
        }
        w.emit_pending(false);
        if w.pending_blank && !w.out.is_empty() {
          w.out.push('\n');
          w.pending_blank = false;
        }
        for cl in comment.lines() {
          w.out.push_str(&w.indent());
          w.out.push_str(cl.trim());
          w.out.push('\n');
        }
        w.just_emitted = true;
      }
      b'{' => {
        let pending = std::mem::take(&mut w.line);
        let t = pending.trim().to_string();
        // trailing `// comment` on the same source line (string-aware)
        let mut comment: Option<String> = None;
        let mut j = i;
        let mut in_str: Option<u8> = None;
        while j < b.len() && b[j] != b'\n' {
          match in_str {
            Some(q) => {
              if b[j] == b'\\' {
                j += 1;
              } else if b[j] == q {
                in_str = None;
              }
            }
            None => {
              if b[j] == b'"' || b[j] == b'\'' {
                in_str = Some(b[j]);
              } else if b[j] == b'/' && j + 1 < b.len() && b[j + 1] == b'/' {
                comment = Some(String::from_utf8_lossy(&b[j..]).trim_end().to_string());
                break;
              }
            }
          }
          j += 1;
        }
        if let Some(cm) = &comment {
          i = j + cm.len(); // resume right after the comment
        }
        if w.pending_blank && !w.out.is_empty() && !t.is_empty() {
          w.out.push('\n');
        }
        w.pending_blank = false;
        w.out.push_str(&w.indent());
        if !t.is_empty() {
          w.out.push_str(&t);
          if let Some(cm) = &comment {
            w.out.push_str(" { ");
            w.out.push_str(cm);
          } else {
            w.out.push_str(" {");
          }
          w.out.push('\n');
        } else {
          w.out.push_str("{\n");
        }
        w.depth += 1;
        w.just_emitted = true;
      }
      b'}' => {
        w.emit_pending(true);
        w.depth = w.depth.saturating_sub(1);
        w.out.push_str(&w.indent());
        w.out.push_str("}\n");
        w.just_emitted = true;
        w.pending_blank = false;
      }
      b';' => {
        w.line.push(';');
        w.emit_pending(true);
      }
      b'\n' => {
        if !w.line.trim().is_empty() {
          if completes_decl(&w.line) && next_line_looks_like_decl(b, i) {
            // Two `;`-less declarations in a row (an E0013 error
            // the parser will report) — emit the first one with
            // its `;` instead of MERGING both into one value.
            w.emit_pending(true);
          } else {
            // genuine continuation across lines: selectors,
            // at-rule headers, wrapped values, trailing operators
            w.line.push(' ');
            w.skip_ws = true;
          }
        } else if !w.just_emitted {
          // blank line between statements
          w.pending_blank = true;
        }
        w.just_emitted = false;
      }
      _ => {
        if w.skip_ws {
          if c == b' ' || c == b'\t' {
            continue;
          }
          w.skip_ws = false;
        }
        w.line.push(c as char);
        w.just_emitted = false;
      }
    }
  }
  w.emit_pending(true);
  if w.out == "\n" {
    w.out.clear();
  }
  w.out
}

/// Conservative formatter for the indented syntax: leading indentation is
/// normalized to two-space levels (a tab counts as one level), interior
/// whitespace runs collapse to single spaces outside strings.
fn format_indented(source: &str) -> String {
  let mut out_lines: Vec<String> = Vec::new();
  let mut blank_run = false;
  for raw in source.lines() {
    let code = raw;
    // split leading whitespace (tabs = 1 level each, 2 spaces = 1 level)
    let mut tabs = 0usize;
    let mut spaces = 0usize;
    let mut split_at = code.len();
    for (idx, ch) in code.char_indices() {
      match ch {
        '\t' => {
          tabs += 1;
          spaces = 0;
        }
        ' ' => spaces += 1,
        _ => {
          split_at = idx;
          break;
        }
      }
    }
    let rest = collapse_ws(code[split_at..].trim_end());
    if rest.is_empty() {
      blank_run = !out_lines.is_empty();
      continue;
    }
    if blank_run {
      out_lines.push(String::new());
      blank_run = false;
    }
    let level = tabs + spaces / 2;
    out_lines.push(format!("{}{}", "  ".repeat(level), rest));
  }
  let mut s = out_lines.join("\n");
  if !s.is_empty() {
    s.push('\n');
  }
  s
}

/// Collapse runs of spaces/tabs to a single space, preserving string contents.
fn collapse_ws(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  let mut in_str: Option<char> = None;
  let mut pending_space = false;
  let mut chars = s.chars().peekable();
  while let Some(c) = chars.next() {
    match in_str {
      Some(q) => {
        out.push(c);
        if c == '\\' {
          if let Some(&n) = chars.peek() {
            out.push(n);
            chars.next();
          }
        } else if c == q {
          in_str = None;
        }
      }
      None => {
        if c == '"' || c == '\'' {
          if pending_space {
            out.push(' ');
            pending_space = false;
          }
          in_str = Some(c);
          out.push(c);
        } else if c == ' ' || c == '\t' {
          pending_space = !out.is_empty();
        } else {
          if pending_space {
            out.push(' ');
            pending_space = false;
          }
          out.push(c);
        }
      }
    }
  }
  if pending_space {
    out.push(' ');
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn formats_vcss_basic() {
    let src = ".a{color: red;\npadding:2px}\n.b {\n  color: blue }\n";
    let out = format_source(src, true);
    assert_eq!(
      out,
      ".a {\n  color: red;\n  padding: 2px;\n}\n.b {\n  color: blue;\n}\n"
    );
  }

  #[test]
  fn collapses_blank_lines_vcss() {
    let src = ".a {\n  color: red;\n}\n\n\n\n.b {\n  color: blue;\n}\n";
    let out = format_source(src, true);
    assert_eq!(out, ".a {\n  color: red;\n}\n\n.b {\n  color: blue;\n}\n");
  }

  #[test]
  fn keeps_comments_vcss() {
    let src = "// header\n.a { // inline\n  color: red; /* after */\n}\n";
    let out = format_source(src, true);
    assert!(out.starts_with("// header\n.a { // inline\n"), "{out}");
    assert!(out.contains("/* after */"), "{out}");
  }

  #[test]
  fn nested_depth_vcss() {
    let src = ".a { .b { color: red; } }\n";
    let out = format_source(src, true);
    assert_eq!(out, ".a {\n  .b {\n    color: red;\n  }\n}\n");
  }

  #[test]
  fn strings_and_pseudo_selectors_preserved() {
    let src = "a:hover { content: \"a:b\"; }\n.a{color:red}\n";
    let out = format_source(src, true);
    assert!(out.contains("a:hover {"), "{out}");
    assert!(out.contains("content: \"a:b\";"), "{out}");
    assert!(out.contains("color: red;"), "{out}");
  }

  #[test]
  fn decl_without_semicolon_before_brace() {
    let src = ".a {\n  color: red\n}\n";
    let out = format_source(src, true);
    assert_eq!(out, ".a {\n  color: red;\n}\n");
  }

  #[test]
  fn multiline_value_joined() {
    let src = ".a {\n  margin: 10px\n    20px;\n}\n";
    let out = format_source(src, true);
    assert_eq!(out, ".a {\n  margin: 10px 20px;\n}\n");
  }

  #[test]
  fn no_spurious_blank_after_open_brace() {
    let src = ".a {\n  color: red;\n  padding: 2px;\n}\n";
    let out = format_source(src, true);
    assert_eq!(out, src);
  }

  #[test]
  fn formats_bass_basic() {
    let src = "$x:\tred\n\n\n.a\n\tcolor: $x\n  margin: 0\n";
    let out = format_source(src, false);
    assert_eq!(out, "$x: red\n\n.a\n  color: $x\n  margin: 0\n");
  }

  #[test]
  fn vass_preserves_space_indent_levels() {
    let src = ".a\n  color: red\n  .b\n    color: blue\n";
    let out = format_source(src, false);
    assert_eq!(out, src);
  }

  #[test]
  fn vass_comment_strip() {
    let src = "// top\n.a\n  color: red // trailing\n";
    let out = format_source(src, false);
    assert!(out.starts_with("// top\n.a\n"), "{out}");
    assert!(out.contains("color: red // trailing"));
  }

  #[test]
  fn empty_document_stays_empty() {
    assert_eq!(format_source("", true), "");
    assert_eq!(format_source("", false), "");
    assert_eq!(format_source("\n\n", true), "");
  }
}
