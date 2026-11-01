//! ESLint-crisp-style diagnostic rendering for terminals.
//!
//! The companion module [`super::render`] produces the rustc-style block;
//! this one produces the layout the user asked for — per-file blocks with
//! a heavy rule, a `🔥 N errors 🔥` headline, one source line per problem
//! with `↑` carets under the exact columns, a `└─ ✖` message tree when
//! several problems share a line, and `… (lines X-Y clean)` markers for
//! quiet stretches:
//!
//! ```text
//! ────────────────────────────────────────────────────────
//! [1/3]  styles/home.vcss 🔥 2 errors 🔥
//! ────────────────────────────────────────────────────────
//!
//!    2: .b { width: (10px; }
//!                   ↑
//!                   └─ ✖ Unmatched `(` - this paren is never closed. [ E0014 ]
//!
//! … (line 3 clean)
//!
//!    4: .c { height: 20px)); }
//!                         ↑  ↑
//!                         │  └─ ✖ Unmatched `)`. [ E0014 ]
//!                         │
//!                         └─ ✖ Unmatched `)`. [ E0014 ]
//! ```
//!
//! Like [`super::render`], color is opt-in per call and output stays fully
//! readable without ANSI.

use super::diagnostic::{Diagnostic, Severity};

/// The heavy horizontal rule drawn above and below each file block
const RULE: &str = "────────────────────────────────────────────────────────";

/// Minimum gutter width for line numbers (ESLint-crisp uses 4)
const MIN_GUTTER: usize = 4;

struct Palette {
  color: bool,
}

impl Palette {
  fn wrap(&self, code: &str, s: &str) -> String {
    if self.color {
      format!("\u{1b}[{code}m{s}\u{1b}[0m")
    } else {
      s.to_string()
    }
  }
  fn dim(&self, s: &str) -> String {
    self.wrap("2", s)
  }
  fn bold(&self, s: &str) -> String {
    self.wrap("1", s)
  }
  fn red(&self, s: &str) -> String {
    self.wrap("31", s)
  }
  fn yellow(&self, s: &str) -> String {
    self.wrap("33", s)
  }
  fn cyan(&self, s: &str) -> String {
    self.wrap("36", s)
  }
  /// Severity accent: red for errors, yellow for everything softer
  fn mark(&self, sev: Severity, s: &str) -> String {
    match sev {
      Severity::Error => self.red(s),
      _ => self.yellow(s),
    }
  }
}

/// Count helper: `"1 error"` / `"3 errors"`
fn plural(n: usize, word: &str) -> String {
  if n == 1 {
    format!("{n} {word}")
  } else {
    format!("{n} {word}s")
  }
}

/// Render one file's diagnostic block: rule, headline, source frames, tree.
///
/// `index` is the `[i/n]` counter over files that have problems (`None`
/// for single-file renders like `vass compile` failures).
pub fn render_file_block(
  file: &str,
  diags: &[Diagnostic],
  source: Option<&str>,
  index: Option<(usize, usize)>,
  color: bool,
) -> String {
  let p = Palette { color };
  let mut out = String::new();

  let errors = diags
    .iter()
    .filter(|d| d.severity == Severity::Error)
    .count();
  let warnings = diags.len() - errors;

  // ── headline ──────────────────────────────────────────────────────
  let label = if errors > 0 {
    p.mark(
      Severity::Error,
      &format!("🔥 {} 🔥", plural(errors, "error")),
    )
  } else {
    p.mark(
      Severity::Warning,
      &format!("⚠ {} ⚠", plural(warnings, "warning")),
    )
  };
  let prefix = match index {
    Some((i, n)) => format!("[{i}/{n}]  "),
    None => String::new(),
  };
  out.push_str(&p.dim(RULE));
  out.push('\n');
  out.push_str(&format!("{}{} {}\n", prefix, p.bold(file), label));
  out.push_str(&p.dim(RULE));
  out.push('\n');

  // ── split framed (with span) vs plain (no span) diagnostics ───────
  let mut framed: Vec<&Diagnostic> = diags.iter().filter(|d| d.span.is_some()).collect();
  framed.sort_by_key(|d| {
    let s = d.span.unwrap();
    (s.start.line, s.start.column)
  });
  let plain: Vec<&Diagnostic> = diags.iter().filter(|d| d.span.is_none()).collect();

  let max_line = framed
    .last()
    .and_then(|d| d.span)
    .map(|s| s.start.line)
    .unwrap_or(1);
  let gutter = gutter_width(max_line);

  // ── source frames, grouped by line ────────────────────────────────
  let mut group: Vec<&Diagnostic> = Vec::new();
  let mut prev_line: Option<u32> = None;
  for d in framed {
    let line = d.span.unwrap().start.line;
    match prev_line {
      Some(pl) if line != pl => {
        flush_group(&mut out, &p, &group, source, gutter);
        group.clear();
        gap_marker(&mut out, &p, pl, line);
      }
      _ => {}
    }
    group.push(d);
    prev_line = Some(line);
  }
  if !group.is_empty() {
    flush_group(&mut out, &p, &group, source, gutter);
  }

  // ── span-less diagnostics: plain rows, no frame possible ──────────
  for d in plain {
    out.push('\n');
    out.push_str(&mark_row(&p, d));
    out.push('\n');
    // help/notes still deserve a row when there is no frame
    out.push_str(&help_rows(&p, d));
  }

  out
}

/// Render the closing summary for a whole `check` run.
pub fn render_summary(
  errors: usize,
  warnings: usize,
  failed_files: usize,
  total_files: usize,
  color: bool,
) -> String {
  let p = Palette { color };
  if errors == 0 && warnings == 0 {
    return format!(
      "{}\n",
      p.wrap(
        "32",
        &format!("✔ {total_files} file(s) checked — no problems found")
      )
    );
  }
  let mut parts: Vec<String> = Vec::new();
  if errors > 0 {
    parts.push(p.mark(Severity::Error, &format!("✖ {}", plural(errors, "error"))));
  }
  if warnings > 0 {
    parts.push(p.mark(
      Severity::Warning,
      &format!("⚠ {}", plural(warnings, "warning")),
    ));
  }
  parts.push(format!(
    "{} of {total_files} file(s) failed",
    p.bold(&failed_files.to_string())
  ));
  let mut out = String::new();
  out.push_str(&p.dim(RULE));
  out.push('\n');
  out.push_str(&parts.join("  "));
  out.push('\n');
  if errors > 0 {
    out.push_str(&p.dim("  run `vass explain <CODE>` for any code above\n"));
  }
  out
}

/// Render a single-diagnostic block (used by `compile`/`build` failures).
pub fn render_single(file: &str, diag: &Diagnostic, source: Option<&str>, color: bool) -> String {
  render_file_block(file, std::slice::from_ref(diag), source, None, color)
}

/// Gutter width: at least 4 chars, wider for 5-digit line numbers
fn gutter_width(max_line: u32) -> usize {
  MIN_GUTTER.max(max_line.to_string().len())
}

/// `✖ message [ code ]` — shared by framed and plain rows
fn mark_row(p: &Palette, d: &Diagnostic) -> String {
  let code = match &d.code {
    Some(c) => format!(" {}", p.dim(&format!("[ {c} ]"))),
    None => String::new(),
  };
  format!("{} {}{}", p.mark(d.severity, "✖"), d.message, code)
}

/// `= help:` / `= note:` rows under a message (dim, rustc-style labels)
fn help_rows(p: &Palette, d: &Diagnostic) -> String {
  let mut out = String::new();
  if let Some(help) = &d.help {
    out.push_str(&format!("{} {}\n", p.cyan("   = help:"), help));
  }
  for n in &d.notes {
    out.push_str(&format!("{} {}\n", p.dim("   = note:"), n.message));
  }
  out
}

/// Blank line + `… (lines X-Y clean)` between two framed groups
fn gap_marker(out: &mut String, p: &Palette, prev: u32, next: u32) {
  let a = prev + 1;
  let b = next - 1;
  let text = if a > b {
    return; // adjacent lines, nothing between
  } else if a == b {
    format!("… (line {a} clean)")
  } else {
    format!("… (lines {a}-{b} clean)")
  };
  out.push('\n');
  out.push_str(&p.dim(&text));
  out.push('\n');
}

/// Render one source line plus the caret/message tree for its group.
///
/// `group` is non-empty; every member shares the same start line. Rows:
///
/// ```text
///    86:     diamond: '◆', ...
///                 ↑      ↑
///                 │      └─ ✖ second message [ E0001 ]
///                 │
///                 └─ ✖ first message [ E0001 ]
/// ```
fn flush_group(
  out: &mut String,
  p: &Palette,
  group: &[&Diagnostic],
  source: Option<&str>,
  gutter: usize,
) {
  let Some(first) = group.first() else { return };
  let line = first.span.unwrap().start.line;
  out.push('\n'); // blank line before every framed group

  // Locate the line's text and byte offset (for span sanity checks)
  let mut line_start = 0usize;
  let mut raw: Option<&str> = None;
  if let Some(src) = source {
    for (i, l) in src.split_inclusive('\n').enumerate() {
      if i + 1 == line as usize {
        let t = l.strip_suffix('\n').unwrap_or(l);
        let t = t.strip_suffix('\r').unwrap_or(t);
        raw = Some(t);
        break;
      }
      line_start += l.len();
    }
  }

  let Some(raw) = raw else {
    // Line beyond the file (EOF edge): plain row, still precise.
    out.push_str(&p.dim(&format!("{line:>gutter$}: ")));
    out.push_str(&mark_row(p, first));
    out.push('\n');
    out.push_str(&help_rows(p, first));
    return;
  };

  let (expanded, cols) = expand_tabs(raw);

  // source row: `  86:     diamond: ...`
  out.push_str(&p.dim(&format!("{line:>gutter$}: ")));
  out.push_str(&expanded);
  out.push('\n');

  // display column (0-based, tab-expanded) of each diagnostic
  let col_of = |d: &Diagnostic| -> usize {
    let span = d.span.unwrap();
    let chars = expanded.chars().count();
    let line_end = line_start + raw.len();
    // Trust the byte offset only when it actually lands on this line;
    // otherwise fall back to the stored 1-based column as a char index.
    let char_idx = if span.start.offset >= line_start && span.start.offset <= line_end {
      raw[..floor_boundary(raw, span.start.offset - line_start)]
        .chars()
        .count()
    } else {
      span.start.column.saturating_sub(1) as usize
    }
    .min(chars);
    *cols.get(char_idx).unwrap_or(cols.last().unwrap_or(&0))
  };

  let cols_by_diag: Vec<usize> = group.iter().map(|d| col_of(d)).collect();
  let row_width = cols_by_diag.iter().copied().max().unwrap_or(0) + 3;

  // caret row: one `↑` per diagnostic (deduped by column)
  let mut carets: Vec<char> = vec![' '; row_width];
  for &c in &cols_by_diag {
    if c < carets.len() {
      carets[c] = '↑';
    }
  }
  out.push_str(&pad(gutter + 2));
  out.push_str(trailing(&carets.iter().collect::<String>()));
  out.push('\n');

  // message tree: rightmost error reported first, bars for the rest
  for j in (0..group.len()).rev() {
    let d = group[j];
    let cj = cols_by_diag[j];
    let mut row: Vec<char> = vec![' '; row_width];
    for &c in &cols_by_diag[..j] {
      if c < row.len() {
        row[c] = '│';
      }
    }
    if cj + 1 < row.len() {
      row[cj] = '└';
      row[cj + 1] = '─';
    } else {
      // Degenerate column at the very end: widen is impossible, use `└`
      row.resize(cj + 2, ' ');
      row[cj] = '└';
      row[cj + 1] = '─';
    }
    out.push_str(&pad(gutter + 2));
    out.push_str(trailing(&row.iter().collect::<String>()));
    out.push(' ');
    out.push_str(&mark_row(p, d));
    out.push('\n');

    if j > 0 {
      let mut sep: Vec<char> = vec![' '; row_width];
      for &c in &cols_by_diag[..j] {
        if c < sep.len() {
          sep[c] = '│';
        }
      }
      out.push_str(&pad(gutter + 2));
      out.push_str(trailing(&sep.iter().collect::<String>()));
      out.push('\n');
    }
  }

  // help/notes only when a single diagnostic owns the line (a tree with
  // interleaved helps would be unreadable)
  if group.len() == 1 {
    out.push_str(&help_rows(p, group[0]));
  }
}

fn pad(n: usize) -> String {
  " ".repeat(n)
}

/// Strip trailing spaces so rows never end in whitespace
fn trailing(s: &str) -> &str {
  s.trim_end_matches(' ')
}

/// Expand tabs to 4-space stops (same rules as [`super::render`]).
/// Returns the expanded line plus the display column of every char index.
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
  cols.push(col);
  (out, cols)
}

/// Largest byte index `<= i` that is a char boundary of `text`
fn floor_boundary(text: &str, i: usize) -> usize {
  let mut i = i.min(text.len());
  while i > 0 && !text.is_char_boundary(i) {
    i -= 1;
  }
  i
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::token::Span;

  fn diag(line: u32, col: u32, msg: &str) -> Diagnostic {
    Diagnostic::error(msg)
      .with_code("E0014")
      .with_span(Span::new(0, 1, line, col))
  }

  const SRC: &str = ".a { color: red; }\n.b { width: (10px; }\n.c { ok }\n";

  #[test]
  fn single_error_block() {
    // `(` on line 2, col 14
    let d = diag(2, 14, "Unmatched `(` - this paren is never closed.");
    let out = render_file_block("home.vcss", &[d], Some(SRC), Some((1, 3)), false);
    assert!(out.contains("[1/3]  home.vcss 🔥 1 error 🔥"), "{out}");
    assert!(out.contains("   2: .b { width: (10px; }"), "{out}");
    // caret sits under the `(`: gutter(4)+2 = 6 prefix, col 14 → 6+13
    let caret = out.lines().find(|l| l.contains('↑')).unwrap();
    assert_eq!(caret.find('↑').unwrap(), 6 + 13, "{out}");
    assert!(
      out.contains("└─ ✖ Unmatched `(` - this paren is never closed. [ E0014 ]"),
      "{out}"
    );
  }

  #[test]
  fn tree_reports_rightmost_first() {
    // two errors on line 3 (`.c { ok }`), columns 5 and 8
    let d1 = diag(3, 5, "first");
    let d2 = diag(3, 8, "second");
    let out = render_file_block("x.vcss", &[d1, d2], Some(SRC), None, false);
    let lines: Vec<&str> = out.lines().collect();
    let msg_first = lines.iter().position(|l| l.contains("second")).unwrap();
    let msg_second = lines.iter().position(|l| l.contains("first")).unwrap();
    assert!(msg_first < msg_second, "rightmost first: {out}");
    // carets row has both marks
    let caret = lines.iter().find(|l| l.contains('↑')).unwrap();
    assert_eq!(caret.matches('↑').count(), 2, "{out}");
    // tree bars present
    assert!(lines.iter().any(|l| l.contains('│')), "{out}");
  }

  #[test]
  fn gap_marker_between_groups() {
    let d1 = diag(1, 5, "a");
    let d2 = diag(3, 5, "b");
    let out = render_file_block("x.vcss", &[d1, d2], Some(SRC), None, false);
    assert!(out.contains("… (line 2 clean)"), "{out}");
  }

  #[test]
  fn wide_gap_marker() {
    let d1 = diag(1, 5, "a");
    let d2 = diag(10, 5, "b");
    let out = render_file_block("x.vcss", &[d1, d2], Some(SRC), None, false);
    assert!(out.contains("… (lines 2-9 clean)"), "{out}");
  }

  #[test]
  fn adjacent_lines_have_no_marker() {
    let d1 = diag(1, 5, "a");
    let d2 = diag(2, 5, "b");
    let out = render_file_block("x.vcss", &[d1, d2], Some(SRC), None, false);
    assert!(!out.contains("clean"), "{out}");
  }

  #[test]
  fn warning_headline_when_no_errors() {
    let d = Diagnostic::warning("deprecated")
      .with_code("W0001")
      .with_span(Span::new(0, 1, 1, 5));
    let out = render_file_block("x.vcss", &[d], Some(SRC), None, false);
    assert!(out.contains("⚠ 1 warning ⚠"), "{out}");
  }

  #[test]
  fn summary_counts_and_clean_state() {
    let s = render_summary(2, 1, 3, 9, false);
    assert!(s.contains("✖ 2 errors"), "{s}");
    assert!(s.contains("⚠ 1 warning"), "{s}");
    assert!(s.contains("3 of 9 file(s) failed"), "{s}");
    let clean = render_summary(0, 0, 0, 4, false);
    assert!(clean.contains("✔ 4 file(s) checked"), "{clean}");
  }

  #[test]
  fn color_off_has_no_ansi() {
    let d = diag(2, 14, "boom");
    let out = render_file_block("x.vcss", &[d], Some(SRC), Some((1, 2)), false);
    assert!(!out.contains('\u{1b}'), "{out}");
  }

  #[test]
  fn color_on_wraps_marks() {
    let d = diag(2, 14, "boom");
    let out = render_file_block("x.vcss", &[d], Some(SRC), None, true);
    assert!(out.contains("\u{1b}[31m✖"), "{out}");
  }

  #[test]
  fn missing_line_falls_back_to_plain_row() {
    let d = diag(99, 1, "beyond eof");
    let out = render_file_block("x.vcss", &[d], Some(SRC), None, false);
    assert!(out.contains("99: ✖ beyond eof"), "{out}");
    assert!(!out.contains('↑'), "{out}");
  }

  #[test]
  fn unicode_caret_alignment() {
    // `rééd` — the error targets the final `d` (col 16)
    let src = ".a {\n  content: \"rééd\";\n}\n";
    let d = diag(2, 16, "bad");
    let out = render_file_block("x.vcss", &[d], Some(src), None, false);
    let src_line = out.lines().find(|l| l.contains("rééd")).unwrap();
    let caret = out.lines().find(|l| l.contains('↑')).unwrap();
    // strip the `   2: ` gutter, then count chars before the final `d`
    let source_part = src_line.split_once(": ").unwrap().1;
    let d_col = source_part.split('d').next().unwrap().chars().count();
    assert_eq!(caret.find('↑').unwrap(), 6 + d_col, "{out}");
  }

  #[test]
  fn tabs_expand_and_align() {
    let src = ".a {\n\twidth:\t(10px;\n}\n";
    // `(` after `\twidth:\t`: tab-expands to col 12 (0-based 11)
    let d = diag(2, 9, "unmatched");
    let out = render_file_block("x.vcss", &[d], Some(src), None, false);
    let src_line = out.lines().find(|l| l.contains("(10px")).unwrap();
    let caret = out.lines().find(|l| l.contains('↑')).unwrap();
    let paren_col = src_line.find('(').unwrap();
    assert_eq!(caret.find('↑').unwrap(), paren_col, "{out}");
  }

  #[test]
  fn help_renders_under_single_error() {
    let d = Diagnostic::error("boom")
      .with_code("E0001")
      .with_span(Span::new(0, 1, 1, 3))
      .with_help("close the brace");
    let out = render_file_block("x.vcss", &[d], Some(SRC), None, false);
    assert!(out.contains("= help: close the brace"), "{out}");
  }
}
