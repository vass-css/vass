//! Token kinds

use std::fmt;

/// A position in source code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
  /// Byte offset from start of file
  pub offset: usize,
  /// Line number (1-based)
  pub line: u32,
  /// Column number (1-based)
  pub column: u32,
}

impl Position {
  /// Create a new position
  pub fn new(offset: usize, line: u32, column: u32) -> Self {
    Self {
      offset,
      line,
      column,
    }
  }

  /// Create a position at the start of a file
  pub fn start() -> Self {
    Self::new(0, 1, 1)
  }
}

impl Default for Position {
  fn default() -> Self {
    Self::start()
  }
}

impl fmt::Display for Position {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}:{}", self.line, self.column)
  }
}

impl PartialOrd for Position {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Position {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.offset.cmp(&other.offset)
  }
}

/// Token kinds produced by the lexer
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Kind {
  // === Literals ===
  /// Number literal (e.g., `42`, `3.14`, `10px`)
  Number,
  /// String literal (e.g., `"hello"`, `'world'`)
  String,
  /// Identifier (e.g., `color`, `my-var`)
  Ident,
  /// Color literal (e.g., `#fff`, `#aabbcc`)
  Color,
  /// Boolean literal (`true` or `false`)
  Boolean,
  /// Null literal
  Null,

  // === Whitespace and Comments ===
  /// Whitespace (spaces, tabs)
  Whitespace,
  /// Newline
  Newline,
  /// Single-line comment (`//`)
  SingleLineComment,
  /// Multi-line comment (`/* */`)
  MultiLineComment,

  // === Punctuation ===
  /// Left parenthesis `(`
  LParen,
  /// Right parenthesis `)`
  RParen,
  /// Left bracket `[`
  LBracket,
  /// Right bracket `]`
  RBracket,
  /// Left brace `{`
  LBrace,
  /// Right brace `}`
  RBrace,
  /// Comma `,`
  Comma,
  /// Semicolon `;`
  Semicolon,
  /// Colon `:`
  Colon,
  /// Period `.`
  Dot,
  /// Hash `#` (for id selectors or colors)
  Hash,
  /// At sign `@` (for at-rules)
  At,

  // === Operators ===
  /// Plus `+`
  Plus,
  /// Minus `-`
  Minus,
  /// Asterisk `*`
  Star,
  /// Slash `/`
  Slash,
  /// Percent `%`
  Percent,
  /// Equals `=`
  Equals,
  /// Not equals `!=`
  NotEquals,
  /// Greater than `>`
  Greater,
  /// Less than `<`
  Less,
  /// Greater than or equal `>=`
  GreaterEq,
  /// Less than or equal `<=`
  LessEq,
  /// And `and`
  And,
  /// Or `or`
  Or,
  /// Not `not`
  Not,

  // === Special ===
  /// Variable prefix `$`
  Variable,
  /// Parent selector `&`
  Parent,
  /// Interpolation `#{}`
  Interpolation,
  /// Namespace separator `.`
  Namespace,
  /// Assignment `:`
  Assignment,
  /// `...` spread operator
  Spread,

  // === At-Rules ===
  /// `@import`
  AtImport,
  /// `@use`
  AtUse,
  /// `@forward`
  AtForward,
  /// `@mixin`
  AtMixin,
  /// `@include`
  AtInclude,
  /// `@function`
  AtFunction,
  /// `@return`
  AtReturn,
  /// `@if`
  AtIf,
  /// `@else`
  AtElse,
  /// `@each`
  AtEach,
  /// `@for`
  AtFor,
  /// `@while`
  AtWhile,
  /// `@media`
  AtMedia,
  /// `@supports`
  AtSupports,
  /// `@extend`
  AtExtend,
  /// `@at-root`
  AtRoot,
  /// `@error`
  AtError,
  /// `@warn`
  AtWarn,
  /// `@debug`
  AtDebug,
  /// `@content`
  AtContent,
  /// Generic at-rule identifier
  AtRule(String),

  // === Special Tokens ===
  /// End of file
  Eof,
  /// Unknown/invalid token
  Unknown(char),
}

impl Kind {
  /// Check if this is a literal token
  pub fn is_literal(&self) -> bool {
    matches!(
      self,
      Kind::Number | Kind::String | Kind::Ident | Kind::Color | Kind::Boolean | Kind::Null
    )
  }

  /// Check if this is whitespace
  pub fn is_whitespace(&self) -> bool {
    matches!(self, Kind::Whitespace | Kind::Newline)
  }

  /// Check if this is a comment
  pub fn is_comment(&self) -> bool {
    matches!(self, Kind::SingleLineComment | Kind::MultiLineComment)
  }

  /// Check if this is trivia (whitespace or comment)
  pub fn is_trivia(&self) -> bool {
    self.is_whitespace() || self.is_comment()
  }

  /// Check if this is an operator
  pub fn is_operator(&self) -> bool {
    matches!(
      self,
      Kind::Plus
        | Kind::Minus
        | Kind::Star
        | Kind::Slash
        | Kind::Percent
        | Kind::Equals
        | Kind::NotEquals
        | Kind::Greater
        | Kind::Less
        | Kind::GreaterEq
        | Kind::LessEq
        | Kind::And
        | Kind::Or
        | Kind::Not
    )
  }

  /// Check if this is an at-rule keyword
  pub fn is_at_rule_keyword(&self) -> bool {
    matches!(
      self,
      Kind::AtImport
        | Kind::AtUse
        | Kind::AtForward
        | Kind::AtMixin
        | Kind::AtInclude
        | Kind::AtFunction
        | Kind::AtReturn
        | Kind::AtIf
        | Kind::AtElse
        | Kind::AtEach
        | Kind::AtFor
        | Kind::AtWhile
        | Kind::AtMedia
        | Kind::AtSupports
        | Kind::AtExtend
        | Kind::AtRoot
        | Kind::AtError
        | Kind::AtWarn
        | Kind::AtDebug
        | Kind::AtContent
    )
  }

  /// Check if this is an EOF-like token
  pub fn is_eof(&self) -> bool {
    matches!(self, Kind::Eof)
  }
}

impl fmt::Display for Kind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Kind::Number => write!(f, "number"),
      Kind::String => write!(f, "string"),
      Kind::Ident => write!(f, "identifier"),
      Kind::Color => write!(f, "color"),
      Kind::Boolean => write!(f, "boolean"),
      Kind::Null => write!(f, "null"),
      Kind::Whitespace => write!(f, "whitespace"),
      Kind::Newline => write!(f, "newline"),
      Kind::SingleLineComment => write!(f, "comment"),
      Kind::MultiLineComment => write!(f, "comment"),
      Kind::LParen => write!(f, "'('"),
      Kind::RParen => write!(f, "')'"),
      Kind::LBracket => write!(f, "'['"),
      Kind::RBracket => write!(f, "']'"),
      Kind::LBrace => write!(f, "'{{'"),
      Kind::RBrace => write!(f, "'}}'"),
      Kind::Comma => write!(f, "','"),
      Kind::Semicolon => write!(f, "';'"),
      Kind::Colon => write!(f, "':'"),
      Kind::Dot => write!(f, "'.'"),
      Kind::Hash => write!(f, "'#'"),
      Kind::At => write!(f, "'@'"),
      Kind::Plus => write!(f, "'+'"),
      Kind::Minus => write!(f, "'-'"),
      Kind::Star => write!(f, "'*'"),
      Kind::Slash => write!(f, "'/'"),
      Kind::Percent => write!(f, "'%'"),
      Kind::Equals => write!(f, "'='"),
      Kind::NotEquals => write!(f, "'!='"),
      Kind::Greater => write!(f, "'>'"),
      Kind::Less => write!(f, "'<'"),
      Kind::GreaterEq => write!(f, "'>='"),
      Kind::LessEq => write!(f, "'<='"),
      Kind::And => write!(f, "'and'"),
      Kind::Or => write!(f, "'or'"),
      Kind::Not => write!(f, "'not'"),
      Kind::Variable => write!(f, "variable"),
      Kind::Parent => write!(f, "'&'"),
      Kind::Interpolation => write!(f, "#{{}}"),
      Kind::Namespace => write!(f, "namespace"),
      Kind::Assignment => write!(f, "':'"),
      Kind::Spread => write!(f, "'...'"),
      Kind::AtImport => write!(f, "@import"),
      Kind::AtUse => write!(f, "@use"),
      Kind::AtForward => write!(f, "@forward"),
      Kind::AtMixin => write!(f, "@mixin"),
      Kind::AtInclude => write!(f, "@include"),
      Kind::AtFunction => write!(f, "@function"),
      Kind::AtReturn => write!(f, "@return"),
      Kind::AtIf => write!(f, "@if"),
      Kind::AtElse => write!(f, "@else"),
      Kind::AtEach => write!(f, "@each"),
      Kind::AtFor => write!(f, "@for"),
      Kind::AtWhile => write!(f, "@while"),
      Kind::AtMedia => write!(f, "@media"),
      Kind::AtSupports => write!(f, "@supports"),
      Kind::AtExtend => write!(f, "@extend"),
      Kind::AtRoot => write!(f, "@at-root"),
      Kind::AtError => write!(f, "@error"),
      Kind::AtWarn => write!(f, "@warn"),
      Kind::AtDebug => write!(f, "@debug"),
      Kind::AtContent => write!(f, "@content"),
      Kind::AtRule(name) => write!(f, "@{}", name),
      Kind::Eof => write!(f, "end of file"),
      Kind::Unknown(c) => write!(f, "unknown '{}'", c),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_literal() {
    assert!(Kind::Number.is_literal());
    assert!(Kind::String.is_literal());
    assert!(Kind::Ident.is_literal());
    assert!(Kind::Color.is_literal());
    assert!(Kind::Boolean.is_literal());
    assert!(Kind::Null.is_literal());
    assert!(!Kind::Plus.is_literal());
    assert!(!Kind::Eof.is_literal());
  }

  #[test]
  fn test_is_whitespace_and_comment() {
    assert!(Kind::Whitespace.is_whitespace());
    assert!(Kind::Newline.is_whitespace());
    assert!(!Kind::Ident.is_whitespace());
    assert!(Kind::SingleLineComment.is_comment());
    assert!(Kind::MultiLineComment.is_comment());
    assert!(!Kind::Ident.is_comment());
  }

  #[test]
  fn test_is_trivia() {
    assert!(Kind::Whitespace.is_trivia());
    assert!(Kind::SingleLineComment.is_trivia());
    assert!(!Kind::Ident.is_trivia());
  }

  #[test]
  fn test_is_operator() {
    assert!(Kind::Plus.is_operator());
    assert!(Kind::Minus.is_operator());
    assert!(Kind::Star.is_operator());
    assert!(Kind::Slash.is_operator());
    assert!(Kind::Percent.is_operator());
    assert!(Kind::Equals.is_operator());
    assert!(Kind::NotEquals.is_operator());
    assert!(Kind::Greater.is_operator());
    assert!(Kind::Less.is_operator());
    assert!(Kind::GreaterEq.is_operator());
    assert!(Kind::LessEq.is_operator());
    assert!(Kind::And.is_operator());
    assert!(Kind::Or.is_operator());
    assert!(Kind::Not.is_operator());
    assert!(!Kind::Ident.is_operator());
  }

  #[test]
  fn test_is_at_rule_keyword() {
    assert!(Kind::AtImport.is_at_rule_keyword());
    assert!(Kind::AtUse.is_at_rule_keyword());
    assert!(Kind::AtMedia.is_at_rule_keyword());
    assert!(Kind::AtIf.is_at_rule_keyword());
    assert!(!Kind::Ident.is_at_rule_keyword());
    assert!(!Kind::AtRule("custom".to_string()).is_at_rule_keyword());
  }

  #[test]
  fn test_is_eof() {
    assert!(Kind::Eof.is_eof());
    assert!(!Kind::Ident.is_eof());
  }

  #[test]
  fn test_position_new_and_start() {
    let p = Position::new(10, 3, 5);
    assert_eq!(p.offset, 10);
    assert_eq!(p.line, 3);
    assert_eq!(p.column, 5);
    let s = Position::start();
    assert_eq!(s.offset, 0);
    assert_eq!(s.line, 1);
    assert_eq!(s.column, 1);
  }

  #[test]
  fn test_position_default_and_display() {
    let d = Position::default();
    assert_eq!(d, Position::start());
    let p = Position::new(0, 10, 5);
    assert_eq!(p.to_string(), "10:5");
  }

  #[test]
  fn test_position_partial_ord() {
    let a = Position::new(0, 1, 1);
    let b = Position::new(5, 2, 1);
    assert!(a < b);
    assert!(b > a);
    assert_eq!(a.partial_cmp(&b), Some(std::cmp::Ordering::Less));
  }

  #[test]
  fn test_position_ord() {
    let a = Position::new(0, 1, 1);
    let b = Position::new(5, 1, 1);
    assert!(a < b);
  }

  #[test]
  fn test_kind_debug() {
    let s = format!("{:?}", Kind::Ident);
    assert_eq!(s, "Ident");
    let s = format!("{:?}", Kind::Unknown('?'));
    assert!(s.contains("Unknown"));
  }

  #[test]
  fn test_kind_unknown() {
    let k = Kind::Unknown('?');
    assert!(!k.is_literal());
    assert!(!k.is_operator());
    assert!(!k.is_trivia());
  }

  #[test]
  fn test_at_rule_variant() {
    let k = Kind::AtRule("custom".to_string());
    match k {
      Kind::AtRule(name) => assert_eq!(name, "custom"),
      _ => panic!("unexpected"),
    }
  }

  #[test]
  fn test_kind_hash_and_eq() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Kind::Ident);
    set.insert(Kind::Number);
    assert!(set.contains(&Kind::Ident));
    assert!(!set.contains(&Kind::String));
  }
}
