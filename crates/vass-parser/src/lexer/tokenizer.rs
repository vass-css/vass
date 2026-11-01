//! Tokenizer: source text -> token stream

use vass_core::error::{Error, Result};

use super::scanner::Scanner;

/// The lexical kind of a token
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
  /// Identifier (may start with `-`, may be `!important` / `!default`)
  Ident,
  /// Variable reference, text without the leading `$`
  Var,
  /// Number literal, possibly with a unit attached
  Number,
  /// String literal, text includes the quotes
  Str,
  /// Hex color literal
  Color,
  /// `/* ... */` or `// ...` comment, text includes delimiters
  Comment,
  /// `@` followed by an at-rule name
  At,
  /// `#` not followed by a valid hex color
  Hash,
  /// `(`
  LParen,
  /// `)`
  RParen,
  /// `{`
  LBrace,
  /// `}`
  RBrace,
  /// `,`
  Comma,
  /// `;`
  Semi,
  /// `:`
  Colon,
  /// `+`
  Plus,
  /// `-` (operator; negative numbers are lexed as `Number`)
  Minus,
  /// `*`
  Star,
  /// `/` (operator)
  Slash,
  /// `.` (selector class marker; selectors are raw-captured)
  Period,
  /// `>` (selector combinator)
  Greater,
  /// `~` (selector combinator)
  Tilde,
  /// `|` (attribute selector dash match, raw-captured)
  Pipe,
  /// `^` (attribute selector prefix match, raw-captured)
  Caret,
  /// `[`
  LBracket,
  /// `]`
  RBracket,
  /// `=` (attribute selector)
  Eq,
  /// `|=` (attribute selector: dash-prefix match)
  PipeEq,
  /// `^=` (attribute selector: prefix match)
  CaretEq,
  /// `$=` (attribute selector: suffix match)
  DollarEq,
  /// `==` (equality)
  EqEq,
  /// `!=` (inequality)
  Ne,
  /// `<` (comparison / combinator)
  Less,
  /// `<=` (comparison)
  Le,
  /// `>=` (comparison)
  Ge,
  /// `%` (modulo operator, standalone)
  Percent,
  /// `&` (parent selector; raw-captured)
  Ampersand,
  /// Line break
  Newline,
  /// End of input
  Eof,
}

/// A single lexical token
#[derive(Debug, Clone)]
pub struct Token {
  /// Token kind
  pub kind: Tok,
  /// Raw source text of the token
  pub text: String,
  /// Start byte offset
  pub start: usize,
  /// End byte offset (exclusive)
  pub end: usize,
  /// Start line (1-based)
  pub line: u32,
  /// Start column (1-based)
  pub col: u32,
  /// End line
  pub end_line: u32,
  /// End column
  pub end_col: u32,
  /// Indentation level of the line this token starts on
  /// (spaces count 1, tabs count 4)
  pub indent: u32,
  /// Whether whitespace (or a line start) immediately preceded this token
  pub ws_before: bool,
}

impl Token {
  fn new(
    kind: Tok,
    text: String,
    sc: &Scanner,
    start: (usize, u32, u32),
    indent: u32,
    ws_before: bool,
  ) -> Self {
    Token {
      kind,
      text,
      start: start.0,
      end: sc.offset(),
      line: start.1,
      col: start.2,
      end_line: sc.line(),
      end_col: sc.col(),
      indent,
      ws_before,
    }
  }

  /// Whether this is a `// ...` silent comment
  pub fn is_silent_comment(&self) -> bool {
    self.kind == Tok::Comment && self.text.starts_with("//")
  }

  /// Whether this is a `/* ... */` loud comment
  pub fn is_loud_comment(&self) -> bool {
    self.kind == Tok::Comment && self.text.starts_with("/*")
  }

  /// Whether this identifier starts with `!` (`!important`, `!default`)
  pub fn is_flag(&self) -> bool {
    self.kind == Tok::Ident && self.text.starts_with('!')
  }
}

fn is_ident_start(c: char) -> bool {
  c.is_alphabetic() || c == '_' || !c.is_ascii()
}

fn is_ident_char(c: char) -> bool {
  c.is_alphanumeric() || c == '_' || c == '-' || !c.is_ascii()
}

/// Tokenize `source` into a flat token stream ending with `Tok::Eof`
pub fn tokenize(source: &str) -> Result<Vec<Token>> {
  // Strip a leading UTF-8 BOM (U+FEFF). Windows editors and PowerShell
  // pipelines frequently save files with one; without stripping it the BOM
  // glues onto the first token and silently breaks the first `@rule` or
  // selector (e.g. `\u{feff}@mixin` is no longer an at-rule).
  let source = source.strip_prefix('\u{feff}').unwrap_or(source);
  let mut sc = Scanner::new(source);
  let mut tokens: Vec<Token> = Vec::new();
  let mut indent: u32 = 0;
  let mut at_line_start = true;
  let mut saw_ws = false;

  while let Some(c) = sc.peek() {
    // Count indentation at line starts
    if at_line_start {
      if c == ' ' {
        indent += 1;
        saw_ws = true;
        sc.bump();
        continue;
      }
      if c == '\t' {
        indent += 4;
        saw_ws = true;
        sc.bump();
        continue;
      }
      at_line_start = false;
    }

    // Remember where this token starts
    let start = (sc.offset(), sc.line(), sc.col());
    let last_kind = tokens.last().map(|t| t.kind.clone());
    let ws_before = saw_ws;

    let kind = match c {
      '\n' => {
        sc.bump();
        tokens.push(Token::new(
          Tok::Newline,
          "\n".into(),
          &sc,
          start,
          indent,
          ws_before,
        ));
        at_line_start = true;
        indent = 0;
        saw_ws = false;
        continue;
      }
      '\r' => {
        sc.bump();
        if sc.peek() == Some('\n') {
          sc.bump();
        }
        tokens.push(Token::new(
          Tok::Newline,
          "\n".into(),
          &sc,
          start,
          indent,
          ws_before,
        ));
        at_line_start = true;
        indent = 0;
        saw_ws = false;
        continue;
      }
      ' '
      | '\t'
      | '\u{00a0}'
      | '\u{1680}'
      | '\u{2000}'..='\u{200b}'
      | '\u{202f}'
      | '\u{205f}'
      | '\u{3000}' => {
        // Unicode spaces (NBSP from web/Word pastes, etc.) are
        // whitespace exactly like a plain space, matching the
        // reference compiler. They never count toward indentation.
        saw_ws = true;
        sc.bump();
        continue;
      }

      '/' if sc.peek2() == Some('/') => {
        while let Some(ch) = sc.peek() {
          if ch == '\n' {
            break;
          }
          sc.bump();
        }
        Tok::Comment
      }
      '/' if sc.peek2() == Some('*') => {
        sc.bump();
        sc.bump();
        loop {
          match sc.peek() {
            None => {
              return Err(
                Error::syntax("Unterminated comment.")
                  .with_span(vass_core::token::Span::at(start.0, start.1, start.2)),
              );
            }
            Some('*') if sc.peek2() == Some('/') => {
              sc.bump();
              sc.bump();
              break;
            }
            _ => {
              sc.bump();
            }
          }
        }
        Tok::Comment
      }
      '/' => {
        sc.bump();
        Tok::Slash
      }

      '"' | '\'' => {
        let quote = c;
        sc.bump();
        // `#{ ... }` interpolation may contain quotes and nested
        // braces; only a quote at interpolation depth 0 terminates
        // the string (reference behavior: "a#{"b"}c" is valid)
        let mut interp_depth = 0usize;
        loop {
          match sc.peek() {
            None | Some('\n') => {
              return Err(
                Error::syntax("Unterminated string.")
                  .with_span(vass_core::token::Span::at(start.0, start.1, start.2)),
              );
            }
            Some('\\') => {
              sc.bump();
              sc.bump();
            }
            Some('#') if interp_depth == 0 && sc.peek2() == Some('{') => {
              sc.bump();
              sc.bump();
              interp_depth += 1;
            }
            Some('{') if interp_depth > 0 => {
              sc.bump();
              interp_depth += 1;
            }
            Some('}') if interp_depth > 0 => {
              sc.bump();
              interp_depth -= 1;
            }
            Some(ch) if interp_depth == 0 && ch == quote => {
              sc.bump();
              break;
            }
            Some(inner) if interp_depth > 0 && (inner == '"' || inner == '\'') => {
              // nested quoted string inside interpolation:
              // consume it wholly so its quotes don't confuse
              // brace tracking
              sc.bump();
              loop {
                match sc.peek() {
                  None | Some('\n') => {
                    return Err(
                      Error::syntax("Unterminated string.")
                        .with_span(vass_core::token::Span::at(start.0, start.1, start.2)),
                    );
                  }
                  Some('\\') => {
                    sc.bump();
                    sc.bump();
                  }
                  Some(q) if q == inner => {
                    sc.bump();
                    break;
                  }
                  _ => {
                    sc.bump();
                  }
                }
              }
            }
            _ => {
              sc.bump();
            }
          }
        }
        Tok::Str
      }

      '#' => {
        let m = sc.mark();
        sc.bump();
        let mut hex_len = 0usize;
        while let Some(h) = sc.peek() {
          if h.is_ascii_hexdigit() && hex_len < 8 {
            hex_len += 1;
            sc.bump();
          } else {
            break;
          }
        }
        if matches!(hex_len, 3 | 4 | 6 | 8) && !sc.peek().is_some_and(is_ident_char) {
          Tok::Color
        } else {
          sc.reset(m); // back onto '#'; emit a lone Hash token
          sc.bump();
          Tok::Hash
        }
      }

      '$' => {
        sc.bump();
        // `b$="x"` in an attribute selector is a suffix-match
        // operator, not a variable — the lexer is context-free so it
        // must accept `$=` wherever it appears (raw selector capture
        // uses the source text, not the token kind)
        if sc.peek() == Some('=') {
          sc.bump();
          Tok::DollarEq
        } else {
          let mut name = String::new();
          while let Some(ch) = sc.peek() {
            if is_ident_char(ch) {
              name.push(ch);
              sc.bump();
            } else {
              break;
            }
          }
          if name.is_empty() {
            return Err(
              Error::syntax("Expected an identifier after `$`.")
                .with_span(vass_core::token::Span::at(start.0, start.1, start.2)),
            );
          }
          tokens.push(Token::new(Tok::Var, name, &sc, start, indent, ws_before));
          continue;
        }
      }

      '@' => {
        sc.bump();
        let mut name = String::new();
        while let Some(ch) = sc.peek() {
          if is_ident_char(ch) {
            name.push(ch);
            sc.bump();
          } else {
            break;
          }
        }
        if name.is_empty() {
          return Err(
            Error::syntax("Expected an at-rule name after `@`.")
              .with_span(vass_core::token::Span::at(start.0, start.1, start.2)),
          );
        }
        tokens.push(Token::new(Tok::At, name, &sc, start, indent, ws_before));
        continue;
      }

      '!' => {
        if sc.peek2() == Some('=') {
          sc.bump();
          sc.bump();
          Tok::Ne
        } else {
          sc.bump();
          let mut name = String::from("!");
          while let Some(ch) = sc.peek() {
            if ch.is_ascii_alphabetic() {
              name.push(ch);
              sc.bump();
            } else {
              break;
            }
          }
          if name == "!" {
            return Err(
              Error::syntax("Expected `important` or `default` after `!`.")
                .with_span(vass_core::token::Span::at(start.0, start.1, start.2)),
            );
          }
          tokens.push(Token::new(Tok::Ident, name, &sc, start, indent, ws_before));
          continue;
        }
      }

      '(' => {
        sc.bump();
        Tok::LParen
      }
      ')' => {
        sc.bump();
        Tok::RParen
      }
      '{' => {
        sc.bump();
        Tok::LBrace
      }
      '}' => {
        sc.bump();
        Tok::RBrace
      }
      ',' => {
        sc.bump();
        Tok::Comma
      }
      ';' => {
        sc.bump();
        Tok::Semi
      }
      ':' => {
        sc.bump();
        Tok::Colon
      }
      '+' => {
        sc.bump();
        Tok::Plus
      }
      '*' => {
        sc.bump();
        Tok::Star
      }
      '.' => {
        if sc.peek2().is_some_and(|n| n.is_ascii_digit()) {
          // Leading-dot decimal (`.5`, `.8em`) — supported like
          // dart-sass; tokenizes as a plain number.
          tokenize_number(&mut sc);
          Tok::Number
        } else {
          sc.bump();
          Tok::Period
        }
      }
      '>' => {
        sc.bump();
        if sc.peek() == Some('=') {
          sc.bump();
          Tok::Ge
        } else {
          Tok::Greater
        }
      }
      '<' => {
        sc.bump();
        if sc.peek() == Some('=') {
          sc.bump();
          Tok::Le
        } else {
          Tok::Less
        }
      }
      '=' => {
        sc.bump();
        if sc.peek() == Some('=') {
          sc.bump();
          Tok::EqEq
        } else {
          Tok::Eq
        }
      }
      '%' => {
        sc.bump();
        Tok::Percent
      }
      '~' => {
        sc.bump();
        Tok::Tilde
      }
      '|' => {
        sc.bump();
        // `|=` attribute dash-prefix match (raw-captured selectors
        // only need the character to lex, not a dedicated meaning)
        if sc.peek() == Some('=') {
          sc.bump();
          Tok::PipeEq
        } else {
          Tok::Pipe
        }
      }
      '^' => {
        sc.bump();
        // `^=` attribute prefix match
        if sc.peek() == Some('=') {
          sc.bump();
          Tok::CaretEq
        } else {
          Tok::Caret
        }
      }
      '[' => {
        sc.bump();
        Tok::LBracket
      }
      ']' => {
        sc.bump();
        Tok::RBracket
      }
      '&' => {
        sc.bump();
        Tok::Ampersand
      }

      '-' => {
        // Disambiguate: negative number (`-5px`), identifier
        // (`-webkit-x`, `--custom`), or minus operator (`10 - 5`).
        // VASS rules: `10 -5` is a list containing -5 (whitespace
        // before the minus), `10-5` is subtraction.
        let next = sc.peek2();
        let starts_number = match next {
          Some(n) if n.is_ascii_digit() => ws_before || !is_binary_context(&last_kind),
          // Negative leading-dot decimal: `-.5`
          Some('.') if sc.peek3().is_some_and(|d| d.is_ascii_digit()) => {
            ws_before || !is_binary_context(&last_kind)
          }
          _ => false,
        };
        if starts_number {
          sc.bump(); // consume '-'
          tokenize_number(&mut sc);
          Tok::Number
        } else if next == Some('-') {
          // Custom property (`--foo`) or `-` + identifier: consume
          // as identifier (custom props may start `--`)
          while let Some(ch) = sc.peek() {
            if is_ident_char(ch) {
              sc.bump();
            } else {
              break;
            }
          }
          Tok::Ident
        } else if next.is_some_and(is_ident_start) || next == Some('_') {
          while let Some(ch) = sc.peek() {
            if is_ident_char(ch) {
              sc.bump();
            } else {
              break;
            }
          }
          Tok::Ident
        } else {
          sc.bump();
          Tok::Minus
        }
      }

      _ if is_digit_or_signed_number(&sc) => {
        tokenize_number(&mut sc);
        Tok::Number
      }

      _ if is_ident_start(c) || (c == '-' && sc.peek2().is_some_and(is_ident_start)) => {
        while let Some(ch) = sc.peek() {
          if is_ident_char(ch) {
            sc.bump();
          } else {
            break;
          }
        }
        Tok::Ident
      }

      '\\' => {
        // CSS escape sequence (ident-position): `\` + 1-6 hex digits
        // (+ one optional whitespace separator) or `\` + any single
        // char. The token keeps the RAW source text — property names
        // and selectors are raw-captured, so `\66 oo: 1px` is
        // re-emitted verbatim exactly like the reference compiler.
        sc.bump();
        let mut hex_count = 0usize;
        while let Some(c) = sc.peek() {
          if c.is_ascii_hexdigit() && hex_count < 6 {
            hex_count += 1;
            sc.bump();
          } else {
            break;
          }
        }
        if hex_count > 0 {
          // one optional whitespace separator terminates the escape
          if matches!(sc.peek(), Some(' ') | Some('\t')) {
            sc.bump();
          } else if sc.peek() == Some('\r') {
            sc.bump();
            if sc.peek() == Some('\n') {
              sc.bump();
            }
          } else if matches!(sc.peek(), Some('\n')) {
            sc.bump();
          }
        } else {
          // `\` + any single non-hex character
          sc.bump();
        }
        while let Some(ch) = sc.peek() {
          if is_ident_char(ch) {
            sc.bump();
          } else {
            break;
          }
        }
        Tok::Ident
      }

      other => {
        return Err(
          Error::syntax(format!("Invalid character: `{}`.", other))
            .with_span(vass_core::token::Span::at(start.0, start.1, start.2)),
        );
      }
    };

    let text = sc.source()[start.0..sc.offset()].to_string();
    tokens.push(Token::new(kind, text, &sc, start, indent, ws_before));
    saw_ws = false;
  }

  let eof_off = sc.offset();
  tokens.push(Token {
    kind: Tok::Eof,
    text: String::new(),
    start: eof_off,
    end: eof_off,
    line: sc.line(),
    col: sc.col(),
    end_line: sc.line(),
    end_col: sc.col(),
    indent: 0,
    ws_before: false,
  });

  Ok(tokens)
}

/// After which tokens a `-digit` sequence is a negative number rather
/// than binary subtraction. `10 - 5` (after a value) is subtraction;
/// `a: -5px` (after `:`) is a negative number.
fn is_binary_context(last_kind: &Option<Tok>) -> bool {
  matches!(
    last_kind,
    Some(Tok::Number)
      | Some(Tok::Str)
      | Some(Tok::Color)
      | Some(Tok::RParen)
      | Some(Tok::Var)
      | Some(Tok::Ident)
  )
}

/// Whether the scanner sits on an unsigned number (`10`, `.5`)
fn is_digit_or_signed_number(sc: &Scanner) -> bool {
  let c = match sc.peek() {
    Some(c) => c,
    None => return false,
  };
  if c.is_ascii_digit() {
    return true;
  }
  if c == '.' && sc.peek2().is_some_and(|n| n.is_ascii_digit()) {
    return true;
  }
  false
}

/// Consume digits, decimals and an attached unit
fn tokenize_number(sc: &mut Scanner) {
  while let Some(c) = sc.peek() {
    if c.is_ascii_digit() {
      sc.bump();
    } else {
      break;
    }
  }
  if sc.peek() == Some('.') && sc.peek2().is_some_and(|n| n.is_ascii_digit()) {
    sc.bump();
    while let Some(c) = sc.peek() {
      if c.is_ascii_digit() {
        sc.bump();
      } else {
        break;
      }
    }
  }
  // Scientific-notation exponent (CSS Syntax: `1e3`, `1E-5`, `2.5e+10`).
  // Only when `e`/`E` is followed by a digit or a signed digit — so `1em`
  // still lexes as number + unit, matching the CSS tokenizer rules.
  if sc.peek().is_some_and(|c| c == 'e' || c == 'E') {
    let is_exponent = match sc.peek2() {
      Some(d) if d.is_ascii_digit() => true,
      Some(s) if s == '+' || s == '-' => sc.peek3().is_some_and(|d| d.is_ascii_digit()),
      _ => false,
    };
    if is_exponent {
      sc.bump(); // consume e/E
      if matches!(sc.peek(), Some('+') | Some('-')) {
        sc.bump();
      }
      while sc.peek().is_some_and(|c| c.is_ascii_digit()) {
        sc.bump();
      }
    }
  }
  // Attached unit: letters or %
  if sc
    .peek()
    .is_some_and(|c| c.is_ascii_alphabetic() || c == '%' || !c.is_ascii())
  {
    while let Some(c) = sc.peek() {
      if c.is_ascii_alphabetic() || c == '%' || !c.is_ascii() {
        sc.bump();
      } else {
        break;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn kinds(src: &str) -> Vec<Tok> {
    tokenize(src).unwrap().into_iter().map(|t| t.kind).collect()
  }

  #[test]
  fn test_simple_tokens() {
    assert_eq!(
      kinds(".a { color: red; }"),
      vec![
        Tok::Period,
        Tok::Ident,
        Tok::LBrace,
        Tok::Ident,
        Tok::Colon,
        Tok::Ident,
        Tok::Semi,
        Tok::RBrace,
        Tok::Eof
      ]
    );
  }

  #[test]
  fn test_numbers_and_units() {
    let toks = tokenize("10px 1.5em -3 50%").unwrap();
    let texts: Vec<&str> = toks
      .iter()
      .filter(|t| t.kind != Tok::Eof)
      .map(|t| t.text.as_str())
      .collect();
    assert_eq!(texts, vec!["10px", "1.5em", "-3", "50%"]);
  }

  #[test]
  fn test_leading_dot_decimals() {
    // `.5`-style decimals lex as ONE number token (dart-sass parity);
    // a Period token is only emitted when not followed by a digit.
    let toks = tokenize(".5").unwrap();
    assert_eq!(toks[0].kind, Tok::Number);
    assert_eq!(toks[0].text, ".5");

    let toks = tokenize(".8em").unwrap();
    assert_eq!(toks[0].kind, Tok::Number);
    assert_eq!(toks[0].text, ".8em");

    let toks = tokenize("-.5px").unwrap();
    assert_eq!(toks[0].kind, Tok::Number);
    assert_eq!(toks[0].text, "-.5px");

    // Class selectors still lex as Period + Ident
    let toks = tokenize(".btn").unwrap();
    assert_eq!(toks[0].kind, Tok::Period);
    assert_eq!(toks[1].kind, Tok::Ident);
    assert_eq!(toks[1].text, "btn");

    // `.5.5` -> number then... dart-sass: `.5` then `.5`; ensure both numbers
    let toks = tokenize(".5 .5").unwrap();
    assert_eq!(toks[0].text, ".5");
    assert_eq!(toks[1].text, ".5");

    // `10.5` unchanged
    let toks = tokenize("10.5").unwrap();
    assert_eq!(toks[0].kind, Tok::Number);
    assert_eq!(toks[0].text, "10.5");
  }

  #[test]
  fn test_scientific_notation_numbers() {
    // CSS exponent literals lex as ONE number token
    for (src, text) in [
      ("1e3", "1e3"),
      ("1E5", "1E5"),
      ("1e-5", "1e-5"),
      ("2.5e+10", "2.5e+10"),
      ("2e3px", "2e3px"),
      ("1E2ms", "1E2ms"),
    ] {
      let toks = tokenize(src).unwrap();
      assert_eq!(toks[0].kind, Tok::Number, "src={src}");
      assert_eq!(toks[0].text, text, "src={src}");
    }
    // `1em` is a dimension (unit), NOT an exponent
    let toks = tokenize("1em").unwrap();
    assert_eq!(toks[0].kind, Tok::Number);
    assert_eq!(toks[0].text, "1em");
    // `1ex` likewise
    let toks = tokenize("1ex").unwrap();
    assert_eq!(toks[0].text, "1ex");
  }

  #[test]
  fn test_minus_as_operator() {
    // spaces on both sides -> subtraction
    let toks = tokenize("10 - 5").unwrap();
    assert_eq!(toks[1].kind, Tok::Minus);
    // no space before -> subtraction
    let toks = tokenize("10-5").unwrap();
    assert_eq!(toks[1].kind, Tok::Minus);
    // space before, none after -> negative number (space list item)
    let toks = tokenize("10 -5").unwrap();
    assert_eq!(toks[1].kind, Tok::Number);
    assert_eq!(toks[1].text, "-5");
    // after colon -> negative number
    let toks = tokenize("a: -5px").unwrap();
    assert_eq!(toks[2].kind, Tok::Number);
    assert_eq!(toks[2].text, "-5px");
  }

  #[test]
  fn test_variables_and_strings() {
    let toks = tokenize("$color: \"red\"").unwrap();
    assert_eq!(toks[0].kind, Tok::Var);
    assert_eq!(toks[0].text, "color");
    assert_eq!(toks[2].kind, Tok::Str);
  }

  #[test]
  fn test_colors() {
    let toks = tokenize("#fff #aabbcc #x").unwrap();
    assert_eq!(toks[0].kind, Tok::Color);
    assert_eq!(toks[1].kind, Tok::Color);
    assert_eq!(toks[2].kind, Tok::Hash);
  }

  #[test]
  fn test_comments() {
    let toks = tokenize("// hi\n/* multi */").unwrap();
    assert_eq!(toks[0].kind, Tok::Comment);
    assert!(toks[0].is_silent_comment());
    assert_eq!(toks[1].kind, Tok::Newline);
    assert_eq!(toks[2].kind, Tok::Comment);
    assert!(toks[2].is_loud_comment());
  }

  #[test]
  fn test_indent_tracking() {
    let toks = tokenize("a\n  b").unwrap();
    assert_eq!(toks[0].indent, 0);
    assert_eq!(toks[2].indent, 2);
  }

  #[test]
  fn test_at_rules() {
    let toks = tokenize("@media screen").unwrap();
    assert_eq!(toks[0].kind, Tok::At);
    assert_eq!(toks[0].text, "media");
  }

  #[test]
  fn test_flags() {
    let toks = tokenize("$x: 1 !default").unwrap();
    assert_eq!(toks[3].kind, Tok::Ident);
    assert_eq!(toks[3].text, "!default");
  }
}
