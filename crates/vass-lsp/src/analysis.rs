//! Lightweight source analysis: extract variables, mixins, functions and a
//! document-symbol tree directly from the raw text.
//!
//! Works on both syntaxes (`.vass` indented and `.vcss` braced) and is
//! comment/string aware. Positions are 0-based LSP positions.

use serde_json::Value;

/// A top-level or nested symbol found in the source
#[derive(Debug, Clone)]
pub struct Symbol {
  /// Symbol name (variable name without `$`, mixin/function name)
  pub name: String,
  /// Signature or value preview
  pub detail: String,
  /// 0-based line of the declaration
  pub line: u32,
  /// 0-based character of the name start
  pub col: u32,
  /// Kind of symbol
  pub kind: SymbolKind,
}

/// Kind of an indexed symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
  /// `$name: value`
  Variable,
  /// `@mixin name(params)`
  Mixin,
  /// `@function name(params)`
  Function,
  /// A style rule selector
  Rule,
  /// An at-rule such as `@media`
  AtRule,
}

/// The full index of a document
#[derive(Debug, Clone, Default)]
pub struct Index {
  /// Variables, mixins and functions declared in the document
  pub symbols: Vec<Symbol>,
  /// LSP `DocumentSymbol[]` tree
  pub doc_symbols: Vec<Value>,
}

/// Comment/string-aware copy of the source where comments are removed and
/// string contents blanked, so brace counting and keyword matching are safe.
fn code_view(source: &str) -> Vec<String> {
  let mut out_lines = Vec::new();
  let mut in_block_comment = false;
  for line in source.lines() {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
      if in_block_comment {
        if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
          in_block_comment = false;
          i += 2;
        } else {
          i += 1;
        }
        continue;
      }
      match bytes[i] {
        b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
          in_block_comment = true;
          i += 2;
        }
        b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => break,
        b'"' | b'\'' => {
          let quote = bytes[i];
          out.push(quote as char);
          i += 1;
          while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
              i += 2;
              continue;
            }
            if bytes[i] == quote {
              i += 1;
              break;
            }
            out.push(' ');
            i += 1;
          }
          out.push(quote as char);
        }
        _ => {
          out.push(bytes[i] as char);
          i += 1;
        }
      }
    }
    out_lines.push(out);
  }
  out_lines
}

fn strip_line_comment(line: &str) -> &str {
  let bytes = line.as_bytes();
  let mut in_str: Option<u8> = None;
  let mut i = 0;
  while i < bytes.len() {
    let b = bytes[i];
    match in_str {
      Some(q) => {
        if b == b'\\' {
          i += 1;
        } else if b == q {
          in_str = None;
        }
      }
      None => match b {
        b'"' | b'\'' => in_str = Some(b),
        b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => return &line[..i],
        _ => {}
      },
    }
    i += 1;
  }
  line
}

/// Find the declaration `:` in `$name: value` (skipping `:` inside parens)
fn find_decl_colon(s: &str) -> Option<usize> {
  let mut paren = 0i32;
  for (i, c) in s.char_indices() {
    match c {
      '(' => paren += 1,
      ')' => paren -= 1,
      ':' if paren == 0 => return Some(i),
      _ => {}
    }
  }
  None
}

fn sanitize_ident(raw: &str) -> String {
  raw
    .chars()
    .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
    .collect()
}

/// Is `s` (the text before `{` on a line) a `property: value` declaration
/// rather than a selector? Requires a space after the colon — pseudo-class
/// selectors like `.a:hover` / `a:focus` have none.
fn looks_like_decl(s: &str) -> bool {
  let t = s.trim();
  match t.find(':') {
    Some(i) => {
      // `color: red` has a space after ':'; `.a:hover` does not
      match t[i + 1..].chars().next() {
        Some(' ') | Some('\t') => {
          let key = &t[..i];
          !key.is_empty()
            && !key.contains([
              '.', '#', '>', '+', '~', '[', ']', '&', '*', ',', '(', ')', '@',
            ])
        }
        _ => false,
      }
    }
    None => false,
  }
}

struct Scope {
  /// Scope closes when brace depth drops to this value (braces syntax)
  close_at: u32,
  /// Header-line indent (indented syntax)
  indent: u32,
  node: Value,
}

fn attach(node: Value, stack: &mut [Scope], roots: &mut Vec<Value>) {
  if let Some(parent) = stack.last_mut() {
    parent.node["children"].as_array_mut().unwrap().push(node);
  } else {
    roots.push(node);
  }
}

/// Scan a source and build the symbol index + document-symbol tree.
pub fn analyze(source: &str) -> Index {
  let code = code_view(source);
  let braces = code.iter().any(|l| l.contains('{'));

  let mut symbols: Vec<Symbol> = Vec::new();
  let mut doc_symbols: Vec<Value> = Vec::new();
  let mut stack: Vec<Scope> = Vec::new();
  let mut depth: u32 = 0;

  for (ln, raw) in code.iter().enumerate() {
    let line_no = ln as u32;
    let line = strip_line_comment(raw).trim_end();
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
      continue;
    }
    let indent = (line.len() - trimmed.len()) as u32;
    let opens = trimmed.matches('{').count() as u32;
    let closes = trimmed.matches('}').count() as u32;

    // ---- indented syntax: close scopes at indent <= current ----
    if !braces {
      while let Some(top) = stack.last() {
        if top.indent >= indent {
          let s = stack.pop().unwrap();
          attach(s.node, &mut stack, &mut doc_symbols);
        } else {
          break;
        }
      }
    }

    // ---- variable declarations (both syntaxes) ----
    if trimmed.starts_with('$')
      && let Some(colon) = find_decl_colon(trimmed)
    {
      let name = sanitize_ident(&trimmed[1..colon]);
      if !name.is_empty() {
        let value = trimmed[colon + 1..].trim().trim_end_matches(';').trim();
        symbols.push(Symbol {
          detail: format!("${name}: {value}"),
          line: line_no,
          col: indent + 1,
          kind: SymbolKind::Variable,
          name: name.clone(),
        });
        // leaf node for the document outline
        let var_node = serde_json::json!({
            "name": format!("${name}"),
            "kind": 13,
            "detail": value,
            "range": {
                "start": { "line": line_no, "character": indent },
                "end": { "line": line_no, "character": line.len() as u32 },
            },
            "selectionRange": {
                "start": { "line": line_no, "character": indent + 1 },
                "end": { "line": line_no, "character": indent + 1 + name.len() as u32 },
            },
            "children": [],
        });
        if let Some(parent) = stack.last_mut() {
          parent.node["children"]
            .as_array_mut()
            .unwrap()
            .push(var_node);
        } else {
          doc_symbols.push(var_node);
        }
      }
    }

    // ---- mixin / function declarations (both syntaxes) ----
    for (marker, kind) in [
      ("@mixin ", SymbolKind::Mixin),
      ("@function ", SymbolKind::Function),
    ] {
      if let Some(rest) = trimmed.strip_prefix(marker) {
        let head = rest.split('{').next().unwrap_or(rest);
        let head = head.trim().trim_end_matches('{').trim();
        let (name, params) = match head.split_once('(') {
          Some((n, p)) => (
            n.trim().to_string(),
            p.split(')').next().unwrap_or("").trim().to_string(),
          ),
          None => (head.to_string(), String::new()),
        };
        if !name.is_empty() {
          let name_col = indent + (rest.as_ptr() as usize - trimmed.as_ptr() as usize) as u32;
          symbols.push(Symbol {
            detail: format!("{marker}{name}({params})"),
            line: line_no,
            col: name_col,
            kind,
            name,
          });
        }
      }
    }

    // ---- document-tree headers ----
    let is_at = trimmed.starts_with('@');
    let header = if braces {
      let before = trimmed.split('{').next().unwrap_or("");
      opens > 0 && !before.trim().is_empty() && (is_at || !looks_like_decl(before))
    } else {
      next_line_deeper(&code, ln, indent)
    };

    if header {
      let (name, sym_kind, name_col) = if is_at {
        let head = trimmed.split_whitespace().next().unwrap_or("@");
        let at_name = head.trim_start_matches('@');
        if at_name == "mixin" || at_name == "function" {
          let nm = symbols
            .iter()
            .rev()
            .find(|s| s.kind != SymbolKind::Variable && s.line == line_no)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| at_name.to_string());
          (
            nm,
            if at_name == "mixin" {
              SymbolKind::Mixin
            } else {
              SymbolKind::Function
            },
            indent + 1,
          )
        } else {
          (at_name.to_string(), SymbolKind::AtRule, indent)
        }
      } else {
        let sel = if braces {
          trimmed.split('{').next().unwrap_or(trimmed).trim()
        } else {
          trimmed.trim()
        };
        (sel.to_string(), SymbolKind::Rule, indent)
      };
      let node = serde_json::json!({
          "name": name,
          "kind": symbol_lsp_kind(sym_kind),
          "range": {
              "start": { "line": line_no, "character": name_col },
              "end": { "line": line_no, "character": line.len() as u32 },
          },
          "selectionRange": {
              "start": { "line": line_no, "character": name_col },
              "end": { "line": line_no, "character": name_col + name.len() as u32 },
          },
          "children": [],
      });
      if braces {
        // this scope closes when depth returns to the pre-line depth
        stack.push(Scope {
          close_at: depth,
          indent,
          node,
        });
      } else {
        stack.push(Scope {
          close_at: 0,
          indent,
          node,
        });
      }
    }

    // ---- advance state ----
    if braces {
      depth = (depth + opens).saturating_sub(closes);
      while let Some(top) = stack.last() {
        if depth <= top.close_at {
          let s = stack.pop().unwrap();
          attach(s.node, &mut stack, &mut doc_symbols);
        } else {
          break;
        }
      }
    }
  }

  while let Some(s) = stack.pop() {
    attach(s.node, &mut stack, &mut doc_symbols);
  }

  Index {
    symbols,
    doc_symbols,
  }
}

/// Does the next non-empty code line have a deeper indent?
fn next_line_deeper(code: &[String], ln: usize, indent: u32) -> bool {
  for later in code.iter().skip(ln + 1) {
    let t = later.trim_start();
    if t.is_empty() {
      continue;
    }
    return (later.len() - t.len()) as u32 > indent;
  }
  false
}

fn symbol_lsp_kind(k: SymbolKind) -> u32 {
  match k {
    SymbolKind::Variable => 13,
    SymbolKind::Mixin => 6,     // Method
    SymbolKind::Function => 12, // Function
    SymbolKind::Rule => 5,      // Class
    SymbolKind::AtRule => 3,    // Namespace
  }
}

/// Find the word (identifier or `$var`) at the given 0-based position.
/// Returns `(word, start_col, end_col)`.
pub fn word_at(source: &str, line: u32, character: u32) -> Option<(String, u32, u32)> {
  let line_text = source.lines().nth(line as usize)?;
  let bytes = line_text.as_bytes();
  let ch = (character as usize).min(bytes.len());
  let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
  let mut start = ch;
  while start > 0 && is_word(bytes[start - 1]) {
    start -= 1;
  }
  if start > 0 && bytes[start - 1] == b'$' {
    start -= 1;
  }
  let mut end = ch;
  while end < bytes.len() && is_word(bytes[end]) {
    end += 1;
  }
  if start >= end {
    return None;
  }
  Some((line_text[start..end].to_string(), start as u32, end as u32))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn indexes_vcss_symbols() {
    let src = "// comment $fake: red\n\
                   $accent: #e63946 !default;\n\
                   $big: $accent;\n\
                   \n\
                   @mixin card($pad: 16px) {\n\
                     padding: $pad;\n\
                   }\n\
                   @function double($n) {\n\
                     @return $n * 2;\n\
                   }\n\
                   .card { color: $accent; }\n\
                   @media (min-width: 100px) {\n\
                     .a { color: red; }\n\
                   }\n";
    let idx = analyze(src);
    let vars: Vec<_> = idx
      .symbols
      .iter()
      .filter(|s| s.kind == SymbolKind::Variable)
      .collect();
    assert_eq!(vars.len(), 2, "got {vars:?}");
    assert_eq!(vars[0].name, "accent");
    assert_eq!(vars[0].detail, "$accent: #e63946 !default");
    assert!(
      idx
        .symbols
        .iter()
        .any(|s| s.name == "card" && s.kind == SymbolKind::Mixin)
    );
    assert!(
      idx
        .symbols
        .iter()
        .any(|s| s.name == "double" && s.kind == SymbolKind::Function)
    );
    assert!(idx.doc_symbols.iter().any(|d| d["name"] == "media"));
    assert!(idx.doc_symbols.iter().any(|d| d["name"] == ".card"));
  }

  #[test]
  fn indexes_bass_symbols() {
    let src = "$c: red\n@mixin m($x)\n  color: $x\n.a\n  color: $c\n";
    let idx = analyze(src);
    assert!(
      idx
        .symbols
        .iter()
        .any(|s| s.name == "c" && s.kind == SymbolKind::Variable)
    );
    assert!(
      idx
        .symbols
        .iter()
        .any(|s| s.name == "m" && s.kind == SymbolKind::Mixin)
    );
    assert!(idx.doc_symbols.iter().any(|d| d["name"] == ".a"));
    assert!(idx.doc_symbols.iter().any(|d| d["name"] == "m"));
  }

  #[test]
  fn block_comments_blank_out() {
    let src = "/* $hidden: red */\n$real: blue;\n";
    let idx = analyze(src);
    assert_eq!(idx.symbols.len(), 1);
    assert_eq!(idx.symbols[0].name, "real");
  }

  #[test]
  fn braces_inside_strings_do_not_confuse() {
    let src = "$a: \"{\";\n.x { content: $a; }\n";
    let idx = analyze(src);
    assert_eq!(idx.symbols.len(), 1);
    assert!(idx.doc_symbols.iter().any(|d| d["name"] == ".x"));
  }

  #[test]
  fn nested_doc_symbols_vcss() {
    let src = ".outer {\n  .inner { color: red; }\n}\n";
    let idx = analyze(src);
    assert_eq!(idx.doc_symbols.len(), 1, "roots: {:?}", idx.doc_symbols);
    let kids = idx.doc_symbols[0]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0]["name"], ".inner");
  }

  #[test]
  fn nested_doc_symbols_bass() {
    let src = ".a\n  .b\n    color: red\n  .c\n    color: blue\n";
    let idx = analyze(src);
    assert_eq!(idx.doc_symbols.len(), 1, "roots: {:?}", idx.doc_symbols);
    let kids = idx.doc_symbols[0]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0]["name"], ".b");
    assert_eq!(kids[1]["name"], ".c");
  }

  #[test]
  fn pseudo_class_is_not_decl() {
    let src = ".card:hover { color: red; }\n.card:active {\n  color: blue;\n}\n";
    let idx = analyze(src);
    assert!(idx.doc_symbols.iter().any(|d| d["name"] == ".card:hover"));
    assert!(idx.doc_symbols.iter().any(|d| d["name"] == ".card:active"));
  }

  #[test]
  fn word_at_includes_dollar() {
    let src = "color: $accent;\n";
    let (w, s, e) = word_at(src, 0, 10).unwrap();
    assert_eq!(w, "$accent");
    assert_eq!(s, 7);
    assert_eq!(e, 14);
  }
}
