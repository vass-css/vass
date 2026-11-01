//! Modern, colorful diagnostic rendering for terminals.
//!
//! Turns an [`Error`] plus its source text into a vivid, developer-friendly
//! layout: emoji-headed severity + code, a location pin, numbered source
//! lines with caret pointers, help hints, quick-fix previews, and notes.
//!
//! Color is opt-in per call; use [`color_enabled`] to honor `NO_COLOR`,
//! `TERM=dumb` and non-TTY streams. Every render is fully readable without
//! color — ANSI sequences are decoration, never information.

use super::error::{Error, ErrorKind, FixKind};

/// ANSI reset sequence
const RESET: &str = "\u{1b}[0m";
/// ANSI bold
const BOLD: &str = "\u{1b}[1m";
/// ANSI dim/faint
const DIM: &str = "\u{1b}[2m";
/// ANSI italic
const ITALIC: &str = "\u{1b}[3m";
/// ANSI underline
const UNDERLINE: &str = "\u{1b}[4m";
/// ANSI black
const BLACK: &str = "\u{1b}[30m";
/// ANSI red
const RED: &str = "\u{1b}[31m";
/// ANSI green
const GREEN: &str = "\u{1b}[32m";
/// ANSI yellow
const YELLOW: &str = "\u{1b}[33m";
/// ANSI blue
const BLUE: &str = "\u{1b}[34m";
/// ANSI magenta
const MAGENTA: &str = "\u{1b}[35m";
/// ANSI cyan
const CYAN: &str = "\u{1b}[36m";
/// ANSI white
const WHITE: &str = "\u{1b}[37m";
/// ANSI bright red
const BRIGHT_RED: &str = "\u{1b}[91m";
/// ANSI bright yellow
const BRIGHT_YELLOW: &str = "\u{1b}[93m";
/// ANSI bright blue
const BRIGHT_BLUE: &str = "\u{1b}[94m";
/// ANSI bright magenta
const BRIGHT_MAGENTA: &str = "\u{1b}[95m";
/// ANSI bright cyan
const BRIGHT_CYAN: &str = "\u{1b}[96m";

/// Decide whether colored output is appropriate for a stream.
///
/// Honors the de-facto standards:
/// * `NO_COLOR` set to any non-empty value → never color
/// * `CLICOLOR=0` → never color
/// * `TERM=dumb` → never color
/// * stream not a terminal (pipe/redirect/CI) → never color
pub fn color_enabled(stream_is_terminal: bool) -> bool {
  if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
    return false;
  }
  if std::env::var_os("CLICOLOR").is_some_and(|v| v == "0") {
    return false;
  }
  if std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false) {
    return false;
  }
  stream_is_terminal
}

/// Tiny ANSI helper; a no-op when `color` is off.
struct Paint {
  color: bool,
}

impl Paint {
  fn paint(&self, code: &str, text: &str) -> String {
    if self.color {
      format!("{code}{text}{RESET}")
    } else {
      text.to_string()
    }
  }
  fn bold(&self, t: &str) -> String {
    self.paint(BOLD, t)
  }
  fn dim(&self, t: &str) -> String {
    self.paint(DIM, t)
  }
  fn italic(&self, t: &str) -> String {
    self.paint(ITALIC, t)
  }
  fn red(&self, t: &str) -> String {
    self.paint(RED, t)
  }
  fn green(&self, t: &str) -> String {
    self.paint(GREEN, t)
  }
  fn yellow(&self, t: &str) -> String {
    self.paint(YELLOW, t)
  }
  fn blue(&self, t: &str) -> String {
    self.paint(BLUE, t)
  }
  fn magenta(&self, t: &str) -> String {
    self.paint(MAGENTA, t)
  }
  fn cyan(&self, t: &str) -> String {
    self.paint(CYAN, t)
  }
  fn white(&self, t: &str) -> String {
    self.paint(WHITE, t)
  }
  fn red_bold(&self, t: &str) -> String {
    self.paint(&format!("{BOLD}{RED}"), t)
  }
  fn green_bold(&self, t: &str) -> String {
    self.paint(&format!("{BOLD}{GREEN}"), t)
  }
  fn yellow_bold(&self, t: &str) -> String {
    self.paint(&format!("{BOLD}{YELLOW}"), t)
  }
  fn blue_bold(&self, t: &str) -> String {
    self.paint(&format!("{BOLD}{BLUE}"), t)
  }
  fn magenta_bold(&self, t: &str) -> String {
    self.paint(&format!("{BOLD}{MAGENTA}"), t)
  }
  fn cyan_bold(&self, t: &str) -> String {
    self.paint(&format!("{BOLD}{CYAN}"), t)
  }
}

/// Emoji + color by error kind
fn kind_style(kind: &ErrorKind) -> (&'static str, &'static str, fn(&Paint, &str) -> String) {
  match kind {
    ErrorKind::Parse => ("🚨", BRIGHT_RED, |p, t| p.red_bold(t)),
    ErrorKind::Syntax => ("🚨", BRIGHT_RED, |p, t| p.red_bold(t)),
    ErrorKind::Type => ("⚡", BRIGHT_YELLOW, |p, t| p.yellow_bold(t)),
    ErrorKind::UndefinedVariable => ("🔍", BRIGHT_MAGENTA, |p, t| p.magenta_bold(t)),
    ErrorKind::UndefinedFunction => ("🔍", BRIGHT_MAGENTA, |p, t| p.magenta_bold(t)),
    ErrorKind::UndefinedMixin => ("🔍", BRIGHT_MAGENTA, |p, t| p.magenta_bold(t)),
    ErrorKind::Duplicate => ("📋", BRIGHT_CYAN, |p, t| p.cyan_bold(t)),
    ErrorKind::Import => ("📦", BRIGHT_BLUE, |p, t| p.blue_bold(t)),
    ErrorKind::Io => ("💾", BRIGHT_YELLOW, |p, t| p.yellow_bold(t)),
    ErrorKind::Config => ("⚙️ ", BRIGHT_YELLOW, |p, t| p.yellow_bold(t)),
    ErrorKind::Runtime => ("💥", BRIGHT_RED, |p, t| p.red_bold(t)),
    ErrorKind::Internal => ("🐛", BRIGHT_RED, |p, t| p.red_bold(t)),
  }
}

/// Severity header: config/IO problems are environmental, so they headline
/// as `warning`; language failures headline as `error`.
fn header(err: &Error, p: &Paint) -> String {
  let (emoji, _color, style) = kind_style(&err.kind);
  let word = match err.kind {
    ErrorKind::Config | ErrorKind::Io => p.bold("warning"),
    _ => style(p, "error"),
  };
  let code = p.dim(&format!("[{}]", err.code_or_default()));
  format!("{emoji} {word}{code}: {}", err.message)
}

/// Expand tabs to 4-space stops in `s`. Returns the expanded line plus, for
/// every character index (0..=len), its display column after expansion.
fn expand_tabs(s: &str) -> (String, Vec<usize>) {
  let mut out = String::with_capacity(s.len());
  let mut cols = Vec::with_capacity(s.chars().count() + 1);
  let mut col = 0usize;
  for ch in s.chars() {
    cols.push(col);
    if ch == '\t' {
      let width = 4 - (col % 4);
      for _ in 0..width {
        out.push(' ');
      }
      col += width;
    } else {
      out.push(ch);
      col += 1;
    }
  }
  cols.push(col); // sentinel: one past the last char
  (out, cols)
}

/// Byte offset within the line `text` → tab-expanded display column of the
/// character that starts there. Falls back to the stored column-1 mapping
/// when the offset is outside the line (generated/dummy spans).
fn display_col(text: &str, rel_offset: usize, stored_col: u32, cols: &[usize]) -> usize {
  let chars = text.chars().count();
  if rel_offset <= text.len() {
    let idx = text[..floor_boundary(text, rel_offset)].chars().count();
    cols.get(idx).copied().unwrap_or(*cols.last().unwrap_or(&0))
  } else {
    // Offsets disagree with the source: trust the stored column (1-based,
    // semantics per the lexer) as a char index, best effort.
    let idx = stored_col.saturating_sub(1) as usize;
    cols.get(idx.min(chars)).copied().unwrap_or(0)
  }
}

/// Render one source line with gutter, text and caret row underneath.
/// `start_idx`/`end_idx` are *character* indices within `line_text`.
fn render_snippet_line(
  out: &mut String,
  p: &Paint,
  line_no: u32,
  line_text: &str,
  start_idx: usize,
  end_idx: usize,
) {
  let (expanded, cols) = expand_tabs(line_text);
  let num = line_no.to_string();
  let gutter_w = num.len().max(2);
  let pad = |n: usize| " ".repeat(n);
  // `   12 | source`
  out.push_str(&p.dim(&format!("   {}{} | ", pad(gutter_w - num.len()), num)));
  out.push_str(&expanded);
  out.push('\n');
  // `      | ^^^^` — caret columns follow tab expansion so the markers land
  // exactly under the offending characters.
  let last = *cols.last().unwrap_or(&0);
  let start = cols.get(start_idx).copied().unwrap_or(last);
  let end = cols.get(end_idx).copied().unwrap_or(last);
  let width = end.saturating_sub(start).max(1);
  out.push_str(&p.dim(&format!("   {} | ", pad(gutter_w))));
  out.push_str(&pad(start));
  out.push_str(&p.red(&"^".repeat(width)));
  out.push('\n');
}

/// Render the full modern block for `err`.
///
/// `source_for` maps a file path to its text; when it returns `None` (or the
/// error has no span) the renderer falls back to the flat layout so no
/// information is ever lost.
pub fn render(err: &Error, source_for: &dyn Fn(&str) -> Option<String>, color: bool) -> String {
  let p = Paint { color };
  let mut out = String::new();

  // ── Header ──────────────────────────────────────────────
  out.push_str(&header(err, &p));
  out.push('\n');

  let Some(file) = err.file.clone() else {
    out.push_str(&render_quickfix(err, &p));
    out.push_str(&render_notes(err, &p));
    out.push_str(&render_help_footer(err, &p));
    return out;
  };
  let Some(span) = err.span else {
    out.push_str(&p.cyan(&format!("  📍 {}\n", file)));
    out.push_str(&render_quickfix(err, &p));
    out.push_str(&render_notes(err, &p));
    out.push_str(&render_help_footer(err, &p));
    return out;
  };
  let Some(source) = source_for(&file) else {
    out.push_str(&p.cyan(&format!(
      "  📍 {}:{}:{}\n",
      file, span.start.line, span.start.column
    )));
    out.push_str(&render_quickfix(err, &p));
    out.push_str(&render_notes(err, &p));
    out.push_str(&render_help_footer(err, &p));
    return out;
  };

  out.push_str(&p.cyan(&format!(
    "  📍 {}:{}:{}\n",
    file, span.start.line, span.start.column
  )));

  // Separator line
  if p.color {
    out.push_str(&p.dim(&format!("  {}\n", "─".repeat(60))));
  }

  // Locate the error line by its 1-based line number, tracking byte ranges.
  let mut line_start = 0usize;
  let mut line_text: Option<&str> = None;
  for (i, line) in source.split_inclusive('\n').enumerate() {
    if i + 1 == span.start.line as usize {
      let trimmed = line.strip_suffix('\n').unwrap_or(line);
      let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
      line_text = Some(trimmed);
      break;
    }
    line_start += line.len();
  }
  let Some(line_text) = line_text else {
    // Line number beyond the file (EOF edge): flat fallback.
    out.push_str(&render_quickfix(err, &p));
    out.push_str(&render_notes(err, &p));
    out.push_str(&render_help_footer(err, &p));
    return out;
  };
  let line_bytes_end = line_start + line_text.len();

  let rel = if span.start.offset >= line_start && span.start.offset <= line_bytes_end {
    Some(span.start.offset - line_start)
  } else {
    None // force stored-column fallback
  };
  let end_rel = if span.end.offset >= line_start && span.end.offset <= line_bytes_end {
    Some(span.end.offset - line_start)
  } else {
    None
  };
  // Character indices of the span within the line (tab-expansion-aware
  // display columns are derived inside render_snippet_line).
  let start_idx = match rel {
    Some(off) => line_text[..floor_boundary(line_text, off)].chars().count(),
    None => (span.start.column.saturating_sub(1) as usize).min(line_text.chars().count()),
  };
  let end_idx = match end_rel {
    Some(off) if span.end.line == span.start.line => {
      line_text[..floor_boundary(line_text, off)].chars().count()
    }
    // Multi-line spans underline through end-of-line.
    _ => line_text.chars().count(),
  };
  let end_idx = end_idx.max(start_idx + 1);

  render_snippet_line(&mut out, &p, span.start.line, line_text, start_idx, end_idx);

  // ── Quick fix ───────────────────────────────────────────
  out.push_str(&render_quickfix(err, &p));
  // ── Notes ───────────────────────────────────────────────
  out.push_str(&render_notes(err, &p));
  // ── Help footer ─────────────────────────────────────────
  out.push_str(&render_help_footer(err, &p));
  out
}

/// Largest byte index `<= i` that is a char boundary of `text`.
fn floor_boundary(text: &str, i: usize) -> usize {
  let mut i = i.min(text.len());
  while i > 0 && !text.is_char_boundary(i) {
    i -= 1;
  }
  i
}

/// Render the quick-fix suggestion (from `ErrorExtra.fix`).
fn render_quickfix(err: &Error, p: &Paint) -> String {
  let Some(extra) = err.extra.as_ref() else {
    return String::new();
  };
  let Some(fix) = &extra.fix else {
    return String::new();
  };
  let mut out = String::new();
  out.push_str(&p.dim("   \n"));
  match &fix.kind {
    FixKind::Replace(text) => {
      out.push_str(&p.green_bold("   🔧 fix: "));
      out.push_str(&p.white(&fix.title));
      out.push('\n');
      out.push_str(&p.dim(&format!("      → replace with: `{}`\n", text)));
    }
    FixKind::Insert { line, column, text } => {
      out.push_str(&p.green_bold("   🔧 fix: "));
      out.push_str(&p.white(&fix.title));
      out.push('\n');
      out.push_str(&p.dim(&format!(
        "      → insert at {}:{}\n      `{}`\n",
        line,
        column,
        text.trim_end()
      )));
    }
  }
  out
}

/// `= help:` / `= note:` trailer lines.
fn render_notes(err: &Error, p: &Paint) -> String {
  let mut out = String::new();
  if let Some(help) = err.help() {
    out.push_str(&p.cyan_bold("   💡 help: "));
    out.push_str(&p.white(help));
    out.push('\n');
  }
  for note in &err.notes {
    out.push_str(&p.blue_bold("   📝 note: "));
    out.push_str(&p.white(&note.message));
    // A note attributed to another file gets its own location line so
    // terminal users can find it too (the LSP shows it as related info).
    if let Some(f) = &note.file {
      let loc = note
        .span
        .map(|s| format!(":{}:{}", s.start.line, s.start.column))
        .unwrap_or_default();
      out.push('\n');
      out.push_str(&p.dim(&format!("             📍 {f}{loc}")));
    }
    out.push('\n');
  }
  out
}

/// One-line discoverability footer: every rendered diagnostic points at
/// `vass explain <code>` where all causes and fixes are documented.
fn render_help_footer(err: &Error, p: &Paint) -> String {
  let code = err.code_or_default();
  let mut out = String::new();
  out.push('\n');
  out.push_str(&p.dim(&format!(
    "   {} run `vass explain {}` for every cause and fix\n",
    p.italic("📘"),
    code
  )));
  out
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::token::Span;

  const SRC: &str = ".card {\n  color: ;\n  width: 10px;\n}\n";

  fn err_at(line: u32, col: u32, off: usize, end: usize) -> Error {
    Error::syntax("Expected a value, found `;`.")
      .with_span(Span::new(off, end, line, col))
      .with_file("test.vcss")
  }

  fn no_source(_: &str) -> Option<String> {
    None
  }
  fn yes_source(f: &str) -> Option<String> {
    (f == "test.vcss").then(|| SRC.to_string())
  }

  /// Split the rendered block into (source-line text, caret-row text)
  /// portions after the `N | ` gutter.
  fn split_source_and_caret(out: &str) -> (String, String) {
    let src_line = out
      .lines()
      .find(|l| {
        l.contains(" | ")
          && !l.contains('^')
          && !l.contains("📍")
          && !l.contains("error[")
          && !l.contains("= ")
          && !l.contains("💡")
          && !l.contains("📝")
          && !l.contains("🔧")
          && !l.contains("📘")
      })
      .unwrap();
    let s = src_line.split_once(" | ").unwrap().1.to_string();
    let caret_line = out.lines().find(|l| l.contains('^')).unwrap();
    let c = caret_line.split_once(" | ").unwrap().1.to_string();
    (s, c)
  }

  #[test]
  fn snippet_shows_line_and_caret() {
    // `.card {\n` is 8 bytes; `;` on line 2 sits at byte 8+9 = 17, col 10.
    let out = render(&err_at(2, 10, 17, 18), &yes_source, false);
    assert!(out.contains("Expected a value, found `;`."), "{out}");
    assert!(out.contains("📍 test.vcss:2:10"), "{out}");
    assert!(out.contains("color: ;"), "{out}");
    let (src_text, caret_text) = split_source_and_caret(&out);
    // caret must align under the `;` (last char of the line text)
    let semi = src_text.rfind(';').unwrap();
    let caret = caret_text.find('^').unwrap();
    assert_eq!(semi, caret, "caret must align under the token: {out}");
    assert_eq!(caret_text.matches('^').count(), 1, "{out}");
  }

  #[test]
  fn multi_char_span_gets_multiple_carets() {
    // `width` spans bytes 22..28 on line 3, col 3.
    let out = render(&err_at(3, 3, 22, 28), &yes_source, false);
    let caret_line = out.lines().find(|l| l.contains('^')).unwrap();
    assert_eq!(caret_line.matches('^').count(), 6, "{out}");
  }

  #[test]
  fn unicode_columns_align() {
    let src = ".a {\n  content: \"rééd\";\n}\n";
    // line 2 starts at byte 5. `d` (final) is char index 15, byte offset
    // 5 + 17 = 22 (both é's take 2 bytes each).
    let e = Error::syntax("bad")
      .with_span(Span::new(22, 23, 2, 17))
      .with_file("test.vcss");
    let lookup = |f: &str| (f == "test.vcss").then(|| src.to_string());
    let out = render(&e, &lookup, false);
    let (src_text, caret_text) = split_source_and_caret(&out);
    // The `d` after `réé`: its char index must equal the caret's column.
    let d_chars_before = src_text.split("d").next().unwrap().chars().count();
    assert_eq!(
      d_chars_before,
      caret_text.find('^').unwrap(),
      "unicode caret alignment: {out}"
    );
    assert!(src_text.contains("rééd"), "{out}");
  }

  #[test]
  fn tabs_expand_and_align() {
    let src = ".a {\n\tcolor:\tred;\n}\n";
    // line 2 starts at byte 5; `red` begins at rel byte 8 → absolute 13.
    let e = Error::syntax("bad")
      .with_span(Span::new(13, 16, 2, 9))
      .with_file("test.vcss");
    let lookup = |f: &str| (f == "test.vcss").then(|| src.to_string());
    let out = render(&e, &lookup, false);
    assert!(
      out.contains("    color:  red;"),
      "tab expanded to 4 stops: {out}"
    );
    let (src_text, caret_text) = split_source_and_caret(&out);
    // `red` starts at expanded column 12 (tab at col 10 → next stop 12);
    // the caret row must match that column and cover 3 chars.
    let r_col = src_text.find("red").unwrap();
    let caret = caret_text.find('^').unwrap();
    assert_eq!(r_col, caret, "caret under `red` after tab expansion: {out}");
    assert_eq!(caret_text.matches('^').count(), 3, "{out}");
  }

  #[test]
  fn help_and_notes_render() {
    let e = err_at(2, 10, 16, 17)
      .with_help("provide a valid value after `:`")
      .with_note("while parsing a declaration");
    let out = render(&e, &yes_source, false);
    assert!(
      out.contains("💡 help: provide a valid value after `:`"),
      "{out}"
    );
    assert!(
      out.contains("📝 note: while parsing a declaration"),
      "{out}"
    );
  }

  #[test]
  fn quickfix_render() {
    let e = err_at(2, 10, 16, 17).with_fix(
      "Insert `;`",
      FixKind::Insert {
        line: 2,
        column: 11,
        text: ";\n".to_string(),
      },
    );
    let out = render(&e, &yes_source, false);
    assert!(out.contains("🔧 fix: Insert `;`"), "{out}");
    assert!(out.contains("→ insert at 2:11"), "{out}");
  }

  #[test]
  fn missing_source_falls_back_to_flat() {
    let out = render(&err_at(2, 10, 16, 17), &no_source, false);
    assert!(out.contains("📍 test.vcss:2:10"), "{out}");
    assert!(!out.contains('^'), "no caret without source: {out}");
    assert!(out.contains("error[E0001]:"), "{out}");
  }

  #[test]
  fn color_off_produces_no_ansi() {
    let e = err_at(2, 10, 16, 17).with_help("fix it");
    let out = render(&e, &yes_source, false);
    assert!(!out.contains('\u{1b}'), "{out}");
  }

  #[test]
  fn color_on_wraps_severity() {
    let e = err_at(2, 10, 16, 17);
    let out = render(&e, &yes_source, true);
    assert!(out.contains("\u{1b}[1m\u{1b}[31merror"), "{out}");
  }

  #[test]
  fn gutter_aligns_multi_digit_lines() {
    let mut src = String::new();
    for i in 0..11 {
      src.push_str(&format!("line {}\n", i));
    }
    // line 10 = "line 9\n", starts at byte 70; the `9` char is at byte 75.
    let e = Error::syntax("bad")
      .with_span(Span::new(75, 76, 10, 6))
      .with_file("big.vcss");
    let lookup = |f: &str| (f == "big.vcss").then(|| src.clone());
    let out = render(&e, &lookup, false);
    assert!(out.contains("10 | line 9"), "{out}");
    // Only the error line prints; with a 2-digit gutter the caret row is
    // `   | ` (three spaces before the bar).
    let caret_line = out.lines().find(|l| l.contains('^')).unwrap();
    assert_eq!(caret_line.matches('^').count(), 1, "{out}");
    assert!(caret_line.starts_with("      | "), "{out}");
  }
}
