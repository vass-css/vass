//! Token-level sanity scans for `vass check`.
//!
//! The parser is fail-fast: it stops at the first malformed statement, so a
//! broken file would surface only ONE diagnostic even when five brackets are
//! unbalanced. This scanner walks the raw token stream (the tokenizer already
//! skips strings and comments, so brackets inside them can never fool it) and
//! reports EVERY unbalanced bracket it meets — each with a precise span — so
//! the crisp reporter can show several carets over the same file, even the
//! same line.

use vass_core::error::Error;
use vass_core::token::Span;

use crate::lexer::{Tok, Token, tokenize};

/// The closing token expected for an opening one
fn closer_for(open: &Tok) -> Option<Tok> {
  match open {
    Tok::LParen => Some(Tok::RParen),
    Tok::LBrace => Some(Tok::RBrace),
    Tok::LBracket => Some(Tok::RBracket),
    _ => None,
  }
}

/// The opening token expected for a closing one
fn opener_for(close: &Tok) -> Option<Tok> {
  match close {
    Tok::RParen => Some(Tok::LParen),
    Tok::RBrace => Some(Tok::LBrace),
    Tok::RBracket => Some(Tok::LBracket),
    _ => None,
  }
}

/// The character a bracket token represents
fn glyph(kind: &Tok) -> &'static str {
  match kind {
    Tok::LParen | Tok::RParen => "(",
    Tok::LBrace | Tok::RBrace => "{",
    Tok::LBracket | Tok::RBracket => "[",
    _ => "?",
  }
}

/// The character a CLOSING bracket token represents (for "expected a `]`"
/// style messages, where `[` alone would read backwards)
fn close_glyph(kind: &Tok) -> &'static str {
  match kind {
    Tok::RParen => ")",
    Tok::RBrace => "}",
    Tok::RBracket => "]",
    _ => "?",
  }
}

fn span_of(tok: &Token) -> Span {
  Span::new(tok.start, tok.end, tok.line, tok.col)
}

/// A closer with no matching opener anywhere on the stack
fn unmatched_error(tok: &Token, want: &Tok) -> Error {
  Error::syntax(format!(
    "Unmatched `{}` - there is no `{}` to match it.",
    tok.text,
    glyph(want),
  ))
  .with_code("E0014")
  .with_span(span_of(tok))
}

/// An opener that was never closed
fn unclosed_error(tok: &Token) -> Error {
  let want = closer_for(&tok.kind).unwrap();
  Error::syntax(format!(
    "Unclosed `{}` - this block is never closed (expected a `{}`).",
    tok.text,
    close_glyph(&want),
  ))
  .with_code("E0014")
  .with_span(span_of(tok))
}

/// Scan `source` for unbalanced brackets. Returns one diagnostic per
/// problem — unmatched closers, mismatched nestings and openers left open
/// at EOF — in source order. An empty result means the scan found nothing
/// (a tokenizer failure also returns empty; the parse will report that).
pub fn scan_balance(source: &str) -> Vec<Error> {
  let tokens = match tokenize(source) {
    Ok(t) => t,
    Err(_) => return Vec::new(),
  };

  let mut open_stack: Vec<&Token> = Vec::new();
  let mut errs: Vec<Error> = Vec::new();

  for tok in &tokens {
    match tok.kind {
      Tok::LParen | Tok::LBrace | Tok::LBracket => open_stack.push(tok),
      Tok::RParen | Tok::RBrace | Tok::RBracket => {
        let want = opener_for(&tok.kind).unwrap();
        match open_stack.last().map(|t| &t.kind) {
          Some(k) if *k == want => {
            open_stack.pop();
          }
          Some(_) => {
            if let Some(pos) = open_stack.iter().rposition(|t| t.kind == want) {
              // The closer matches a DEEPER opener: every
              // opener above it was never closed. Report each
              // at its own position (source order), then let
              // the closer consume its real partner.
              for t in &open_stack[pos + 1..] {
                errs.push(unclosed_error(t).with_note_and_span(
                  format!("not closed before this `{}`", tok.text,),
                  span_of(tok),
                ));
              }
              open_stack.truncate(pos);
              open_stack.pop();
            } else {
              errs.push(unmatched_error(tok, &want));
            }
          }
          None => {
            errs.push(unmatched_error(tok, &want));
          }
        }
      }
      _ => {}
    }
  }

  // Openers still on the stack at EOF were never closed
  for tok in &open_stack {
    errs.push(unclosed_error(tok));
  }

  errs
}

#[cfg(test)]
mod tests {
  use super::*;

  fn scan(src: &str) -> Vec<String> {
    scan_balance(src)
      .into_iter()
      .map(|e| e.message.to_string())
      .collect()
  }

  #[test]
  fn clean_vcss_has_no_findings() {
    assert!(scan(".a { color: rgb(1, 2, 3); }").is_empty());
    assert!(scan("@media (min-width: 100px) { .a { color: red; } }").is_empty());
    // brackets inside strings and comments must not confuse the scan
    assert!(scan(".a { content: \"(}[]\"; /* ) */ }").is_empty());
    // sass interpolation keeps its balance
    assert!(scan(".a-#{1 + 2} { color: red; }").is_empty());
    // url() with unbalanced-looking content is still balanced as tokens
    assert!(scan(".a { background: url(x.png); }").is_empty());
  }

  #[test]
  fn clean_bass_has_no_findings() {
    // indented syntax with interpolation and nested calls
    assert!(scan("$x: calc(100% - (2 * 10px))\n.a\n  color: $x\n").is_empty());
  }

  #[test]
  fn unmatched_closer_is_reported() {
    let msgs = scan(".c { height: 20px)); }\n");
    assert_eq!(msgs.len(), 2, "{msgs:?}");
    assert!(msgs[0].contains("Unmatched `)`"), "{msgs:?}");
  }

  #[test]
  fn unclosed_opener_is_reported() {
    let msgs = scan(".b { width: (10px; }\n");
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    assert!(msgs[0].contains("Unclosed `(`"), "{msgs:?}");
  }

  #[test]
  fn unclosed_bracket_names_the_right_closer() {
    let msgs = scan(".a { x: [1, 2; }\n");
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    assert!(
      msgs[0].contains("Unclosed `[`") && msgs[0].contains("expected a `]`"),
      "{msgs:?}"
    );
  }

  #[test]
  fn opener_closed_by_the_wrong_bracket_is_unclosed() {
    // the `}` implicitly closes `.a {`, leaving `(` open before it
    let msgs = scan(".a {\n  ( }\n");
    assert_eq!(msgs.len(), 1, "{msgs:?}");
    assert!(msgs[0].contains("Unclosed `(`"), "{msgs:?}");
  }

  #[test]
  fn multiple_problems_across_lines() {
    let msgs = scan(".a {\n  color: (red;\n}\n.b { x: y)); }\n");
    // line 2: unclosed `(`; line 4: two unmatched `)`
    assert_eq!(msgs.len(), 3, "{msgs:?}");
  }

  #[test]
  fn every_error_carries_code_e0014() {
    for e in scan_balance(".a { x: ((y; }\n") {
      assert_eq!(e.code_or_default(), "E0014");
    }
  }
}
