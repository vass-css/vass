//! VASS expressions (value expressions on the right side of `:`)

use crate::token::Span;

/// A binary operator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
  /// `+`
  Add,
  /// `-`
  Sub,
  /// `*`
  Mul,
  /// `/` (division only in operator context; otherwise a CSS separator)
  Div,
  /// `%` (modulo)
  Mod,
  /// `==`
  Eq,
  /// `!=`
  Ne,
  /// `<`
  Lt,
  /// `>`
  Gt,
  /// `<=`
  Le,
  /// `>=`
  Ge,
  /// `and`
  And,
  /// `or`
  Or,
}

impl BinaryOp {
  /// The operator as text
  pub fn as_str(&self) -> &'static str {
    match self {
      BinaryOp::Add => "+",
      BinaryOp::Sub => "-",
      BinaryOp::Mul => "*",
      BinaryOp::Div => "/",
      BinaryOp::Mod => "%",
      BinaryOp::Eq => "==",
      BinaryOp::Ne => "!=",
      BinaryOp::Lt => "<",
      BinaryOp::Gt => ">",
      BinaryOp::Le => "<=",
      BinaryOp::Ge => ">=",
      BinaryOp::And => "and",
      BinaryOp::Or => "or",
    }
  }
}

/// A unary operator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
  /// `-x`
  Neg,
  /// `+x`
  Pos,
  /// `not x`
  Not,
}

/// One positional/named/rest argument in a call
#[derive(Debug, Clone, PartialEq)]
pub struct CallArg {
  /// The argument expression
  pub expr: Expr,
  /// Named argument name (`$name:`), if any
  pub name: Option<String>,
  /// Whether this is a rest spread (`$list...`)
  pub rest: bool,
}

/// One part of a string or interpolation
#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
  /// Literal text
  Text(String),
  /// An interpolated expression (`#{...}`)
  Expr(Box<Expr>),
}

/// A mixin/function parameter
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
  /// Parameter name without `$`
  pub name: String,
  /// Default value expression, if any
  pub default: Option<Expr>,
  /// Whether this is a rest parameter (`$args...`)
  pub rest: bool,
}

/// A value expression
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
  /// `null`
  Null,
  /// `true` / `false`
  Bool(bool),
  /// A number literal with optional unit
  Number {
    /// Numeric value
    value: f64,
    /// Unit (`px`, `%`, ...), if any
    unit: Option<String>,
  },
  /// A string literal
  Str {
    /// Content without quotes
    text: String,
    /// Whether it was quoted in source
    quoted: bool,
  },
  /// A string containing interpolation (`"a#{x}b"` or `a#{x}b`)
  Interp {
    /// Literal and expression parts
    parts: Vec<StrPart>,
    /// Whether the whole was quoted in source
    quoted: bool,
  },
  /// A bare interpolation in a value position (`#{...}`)
  InterpValue {
    /// The inner expression
    expr: Box<Expr>,
  },
  /// A hex color literal
  Color {
    /// Hex text including `#`
    hex: String,
  },
  /// A variable reference (`$name`)
  Variable {
    /// Variable name without `$`
    name: String,
    /// Source location, for diagnostics
    span: Span,
  },
  /// A namespaced variable reference (`module.$name`)
  ModuleVar {
    /// Module namespace
    namespace: String,
    /// Variable name without `$`
    name: String,
    /// Source location, for diagnostics
    span: Span,
  },
  /// A unary operation
  Unary {
    /// The operator
    op: UnaryOp,
    /// The operand
    expr: Box<Expr>,
  },
  /// A binary operation
  Binary {
    /// The operator
    op: BinaryOp,
    /// Left operand
    left: Box<Expr>,
    /// Right operand
    right: Box<Expr>,
    /// Whether `/` appeared between two literal numbers in plain CSS
    /// position (keep the slash as a separator instead of dividing)
    allow_slash_div: bool,
  },
  /// A space- or comma-separated list
  List {
    /// Items
    items: Vec<Expr>,
    /// Separator
    separator: crate::value::Separator,
    /// Whether wrapped in square brackets
    bracketed: bool,
  },
  /// A map literal (`("a": 1, "b": 2)`)
  Map {
    /// Ordered key/value entries
    entries: Vec<(Expr, Expr)>,
  },
  /// A function call (`name(args...)`), including namespaced calls
  /// (`math.div(...)` — the name carries the namespace prefix)
  Call {
    /// Function name (dotted for modules)
    name: String,
    /// Call arguments
    args: Vec<CallArg>,
    /// Whether the call is a known plain-CSS function to pass through
    plain_css: bool,
    /// Source location, for diagnostics
    span: crate::token::Span,
  },
  /// A parenthesized single expression (grouping)
  Paren {
    /// The inner expression
    expr: Box<Expr>,
    /// Whether `/` inside should divide (parenthesized arithmetic)
    slash_div: bool,
  },
}

impl Expr {
  /// Whether this expression can begin a list item (used by the parser)
  pub fn is_list_start_hint(&self) -> bool {
    !matches!(self, Expr::Null)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_binary_op_as_str() {
    assert_eq!(BinaryOp::Add.as_str(), "+");
    assert_eq!(BinaryOp::Sub.as_str(), "-");
    assert_eq!(BinaryOp::Mul.as_str(), "*");
    assert_eq!(BinaryOp::Div.as_str(), "/");
    assert_eq!(BinaryOp::Mod.as_str(), "%");
    assert_eq!(BinaryOp::Eq.as_str(), "==");
    assert_eq!(BinaryOp::Ne.as_str(), "!=");
    assert_eq!(BinaryOp::Lt.as_str(), "<");
    assert_eq!(BinaryOp::Gt.as_str(), ">");
    assert_eq!(BinaryOp::Le.as_str(), "<=");
    assert_eq!(BinaryOp::Ge.as_str(), ">=");
    assert_eq!(BinaryOp::And.as_str(), "and");
    assert_eq!(BinaryOp::Or.as_str(), "or");
  }

  #[test]
  fn test_expr_is_list_start_hint() {
    assert!(!Expr::Null.is_list_start_hint());
    assert!(Expr::Bool(true).is_list_start_hint());
    assert!(
      Expr::Number {
        value: 1.0,
        unit: None
      }
      .is_list_start_hint()
    );
    assert!(
      Expr::Str {
        text: "x".to_string(),
        quoted: true
      }
      .is_list_start_hint()
    );
  }

  #[test]
  fn test_expr_variants() {
    let _ = Expr::Null;
    let _ = Expr::Bool(false);
    let _ = Expr::Number {
      value: 42.0,
      unit: Some("px".to_string()),
    };
    let _ = Expr::Str {
      text: "hello".to_string(),
      quoted: true,
    };
    let _ = Expr::Color {
      hex: "#ff0000".to_string(),
    };
    let _ = Expr::Variable {
      name: "x".to_string(),
      span: Span::dummy(),
    };
  }
}
