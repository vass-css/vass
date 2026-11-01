//! Diagnostic types for error reporting

use crate::token::Span;
use std::fmt;

/// Severity of a diagnostic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
  /// Error - compilation will fail
  Error,
  /// Warning - compilation continues but should be addressed
  Warning,
  /// Info - informational message
  Info,
  /// Hint - suggestion for improvement
  Hint,
}

impl fmt::Display for Severity {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Severity::Error => write!(f, "error"),
      Severity::Warning => write!(f, "warning"),
      Severity::Info => write!(f, "info"),
      Severity::Hint => write!(f, "hint"),
    }
  }
}

/// A diagnostic message
#[derive(Debug, Clone)]
pub struct Diagnostic {
  /// Severity level
  pub severity: Severity,
  /// Primary message
  pub message: String,
  /// Source location
  pub span: Option<Span>,
  /// Source file path
  pub file: Option<String>,
  /// Error code (e.g., "E0001")
  pub code: Option<String>,
  /// Actionable suggestion (`= help:` in terminal output)
  pub help: Option<String>,
  /// Related notes
  pub notes: Vec<DiagnosticNote>,
}

/// A note attached to a diagnostic
#[derive(Debug, Clone)]
pub struct DiagnosticNote {
  /// Note message
  pub message: String,
  /// Source location
  pub span: Option<Span>,
  /// Source file path
  pub file: Option<String>,
}

impl Diagnostic {
  /// Create a new diagnostic
  pub fn new(severity: Severity, message: impl Into<String>) -> Self {
    Self {
      severity,
      message: message.into(),
      span: None,
      file: None,
      code: None,
      help: None,
      notes: Vec::new(),
    }
  }

  /// Bridge from the engine's [`Error`](super::error::Error) into the shared
  /// diagnostic model — one representation feeds both the terminal renderer
  /// and the LSP publisher.
  pub fn from_error(err: &super::error::Error) -> Self {
    Diagnostic {
      severity: match err.kind {
        super::error::ErrorKind::Config | super::error::ErrorKind::Io => Severity::Warning,
        _ => Severity::Error,
      },
      message: err.message.to_string(),
      span: err.span,
      file: err.file.clone(),
      code: Some(err.code_or_default().to_string()),
      help: err.help().map(String::from),
      notes: err
        .notes
        .iter()
        .map(|n| DiagnosticNote {
          message: n.message.clone(),
          span: n.span,
          file: n.file.clone(),
        })
        .collect(),
    }
  }

  /// Create an error diagnostic
  pub fn error(message: impl Into<String>) -> Self {
    Self::new(Severity::Error, message)
  }

  /// Create a warning diagnostic
  pub fn warning(message: impl Into<String>) -> Self {
    Self::new(Severity::Warning, message)
  }

  /// Create an info diagnostic
  pub fn info(message: impl Into<String>) -> Self {
    Self::new(Severity::Info, message)
  }

  /// Create a hint diagnostic
  pub fn hint(message: impl Into<String>) -> Self {
    Self::new(Severity::Hint, message)
  }

  /// Add a span to the diagnostic
  pub fn with_span(mut self, span: Span) -> Self {
    self.span = Some(span);
    self
  }

  /// Add a file path to the diagnostic
  pub fn with_file(mut self, file: impl Into<String>) -> Self {
    self.file = Some(file.into());
    self
  }

  /// Add an error code to the diagnostic
  pub fn with_code(mut self, code: impl Into<String>) -> Self {
    self.code = Some(code.into());
    self
  }

  /// Add an actionable help suggestion
  pub fn with_help(mut self, help: impl Into<String>) -> Self {
    self.help = Some(help.into());
    self
  }

  /// Add a note to the diagnostic
  pub fn with_note(mut self, message: impl Into<String>) -> Self {
    self.notes.push(DiagnosticNote {
      message: message.into(),
      span: None,
      file: None,
    });
    self
  }

  /// Add a note with span to the diagnostic
  pub fn with_note_and_span(mut self, message: impl Into<String>, span: Span) -> Self {
    self.notes.push(DiagnosticNote {
      message: message.into(),
      span: Some(span),
      file: self.file.clone(),
    });
    self
  }

  /// Check if this is an error
  pub fn is_error(&self) -> bool {
    self.severity == Severity::Error
  }

  /// Check if this is a warning
  pub fn is_warning(&self) -> bool {
    self.severity == Severity::Warning
  }
}

impl fmt::Display for Diagnostic {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}: ", self.severity)?;

    if let Some(ref code) = self.code {
      write!(f, "[{}] ", code)?;
    }

    write!(f, "{}", self.message)?;

    if let Some(ref file) = self.file {
      write!(f, "\n  --> {}", file)?;
      if let Some(span) = self.span {
        write!(f, ":{}:{}", span.start.line, span.start.column)?;
      }
    }

    for note in &self.notes {
      write!(f, "\n  note: {}", note.message)?;
    }

    Ok(())
  }
}

/// A collection of diagnostics
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
  /// All diagnostics
  diagnostics: Vec<Diagnostic>,
  /// Whether any errors exist
  has_errors: bool,
}

impl Diagnostics {
  /// Create an empty diagnostics collection
  pub fn new() -> Self {
    Self::default()
  }

  /// Add a diagnostic
  pub fn add(&mut self, diagnostic: Diagnostic) {
    if diagnostic.is_error() {
      self.has_errors = true;
    }
    self.diagnostics.push(diagnostic);
  }

  /// Add an error
  pub fn error(&mut self, message: impl Into<String>) {
    self.add(Diagnostic::error(message));
  }

  /// Add a warning
  pub fn warning(&mut self, message: impl Into<String>) {
    self.add(Diagnostic::warning(message));
  }

  /// Check if there are any errors
  pub fn has_errors(&self) -> bool {
    self.has_errors
  }

  /// Check if there are any diagnostics
  pub fn is_empty(&self) -> bool {
    self.diagnostics.is_empty()
  }

  /// Get the number of diagnostics
  pub fn len(&self) -> usize {
    self.diagnostics.len()
  }

  /// Get all diagnostics
  pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
    self.diagnostics.iter()
  }

  /// Get all diagnostics (mutable)
  pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Diagnostic> {
    self.diagnostics.iter_mut()
  }

  /// Convert to a vector
  pub fn into_vec(self) -> Vec<Diagnostic> {
    self.diagnostics
  }

  /// Extend with another diagnostics collection
  pub fn extend(&mut self, other: Diagnostics) {
    if other.has_errors {
      self.has_errors = true;
    }
    self.diagnostics.extend(other.diagnostics);
  }
}

impl IntoIterator for Diagnostics {
  type Item = Diagnostic;
  type IntoIter = std::vec::IntoIter<Diagnostic>;

  fn into_iter(self) -> Self::IntoIter {
    self.diagnostics.into_iter()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_diagnostic_creation() {
    let diag = Diagnostic::error("Test error");
    assert!(diag.is_error());
    assert!(!diag.is_warning());
  }

  #[test]
  fn test_diagnostics() {
    let mut diags = Diagnostics::new();
    assert!(diags.is_empty());
    assert!(!diags.has_errors());

    diags.error("Test error");
    assert!(!diags.is_empty());
    assert!(diags.has_errors());

    diags.warning("Test warning");
    assert_eq!(diags.len(), 2);
  }

  #[test]
  fn test_severity_display() {
    assert_eq!(Severity::Error.to_string(), "error");
    assert_eq!(Severity::Warning.to_string(), "warning");
    assert_eq!(Severity::Info.to_string(), "info");
    assert_eq!(Severity::Hint.to_string(), "hint");
  }

  #[test]
  fn test_diagnostic_builder_chain() {
    let span = Span::new(0, 5, 1, 1);
    let diag = Diagnostic::error("test")
      .with_span(span)
      .with_file("test.vass")
      .with_code("E0001")
      .with_help("fix this")
      .with_note("related");
    assert!(diag.is_error());
    assert_eq!(diag.code.as_deref(), Some("E0001"));
    assert_eq!(diag.help.as_deref(), Some("fix this"));
    assert_eq!(diag.notes.len(), 1);
    assert_eq!(diag.notes[0].message, "related");
  }

  #[test]
  fn test_diagnostic_note_with_span() {
    let span = Span::new(0, 5, 1, 1);
    let diag = Diagnostic::error("test")
      .with_file("test.vass")
      .with_note_and_span("note with span", span);
    assert_eq!(diag.notes.len(), 1);
    assert_eq!(diag.notes[0].file.as_deref(), Some("test.vass"));
    assert!(diag.notes[0].span.is_some());
  }

  #[test]
  fn test_diagnostic_display_with_code() {
    let diag = Diagnostic::error("test error").with_code("E0001");
    let s = diag.to_string();
    assert!(s.contains("error: [E0001] test error"));
  }

  #[test]
  fn test_diagnostics_iter() {
    let mut diags = Diagnostics::new();
    diags.error("e1");
    diags.warning("w1");
    let count = diags.iter().count();
    assert_eq!(count, 2);
    let vec = diags.into_vec();
    assert_eq!(vec.len(), 2);
  }

  #[test]
  fn test_diagnostics_extend() {
    let mut d1 = Diagnostics::new();
    d1.error("e1");
    let mut d2 = Diagnostics::new();
    d2.warning("w1");
    d1.extend(d2);
    assert_eq!(d1.len(), 2);
    assert!(d1.has_errors());
  }

  #[test]
  fn test_diagnostic_is_warning() {
    let diag = Diagnostic::warning("warn");
    assert!(diag.is_warning());
    assert!(!diag.is_error());
    let diag = Diagnostic::info("info");
    assert!(!diag.is_error());
    assert!(!diag.is_warning());
    let diag = Diagnostic::hint("hint");
    assert!(!diag.is_error());
    assert!(!diag.is_warning());
  }
}
