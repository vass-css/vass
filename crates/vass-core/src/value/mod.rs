//! Value types
//!
//! VASS value model: null, booleans, numbers (with units), strings,
//! colors, lists, maps, functions, mixins, and argument lists.

pub mod color;
mod number;
mod string;

pub use color::modern;
pub use color::{Color, ColorSpace, ModernColor};
pub use number::{Number, format_f64, format_f64_compressed};
pub use string::BassString;

/// Separator used by a list value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Separator {
  /// Separated by spaces (`1px solid red`)
  Space,
  /// Separated by commas (`Arial, sans-serif`)
  Comma,
  /// Separated by slashes (`12px / 30px`) — used by `list.slash`
  Slash,
  /// No separator decided yet (`()`, `[a]`, bare scalars coerced to
  /// lists). Serializes like a space list and `list.separator()` reports
  /// `space`, but `list.join`/`list.append` with `$separator: auto` fall
  /// through to the other operand — matching the reference compiler's
  /// `ListSeparator.undecided`.
  Undecided,
}

impl Separator {
  /// The textual separator
  pub fn text(&self) -> &'static str {
    match self {
      Separator::Space | Separator::Undecided => " ",
      Separator::Comma => ", ",
      Separator::Slash => " / ",
    }
  }
}

/// A list value
#[derive(Debug, Clone, PartialEq)]
pub struct ListValue {
  /// List items
  pub items: Vec<Value>,
  /// Separator between items
  pub sep: Separator,
  /// Whether the list was written with square brackets (`[a b]`)
  pub bracketed: bool,
}

impl ListValue {
  /// Create a new list value
  pub fn new(items: Vec<Value>, sep: Separator) -> Self {
    Self {
      items,
      sep,
      bracketed: false,
    }
  }

  /// Create a new bracketed list value
  pub fn bracketed(items: Vec<Value>, sep: Separator) -> Self {
    Self {
      items,
      sep,
      bracketed: true,
    }
  }

  /// A single-item comma list that behaves like the item itself
  pub fn single(item: Value) -> Self {
    Self {
      items: vec![item],
      sep: Separator::Comma,
      bracketed: false,
    }
  }

  /// Render the list to CSS, dropping null items
  pub fn to_css(&self) -> String {
    let inner: String = self
      .items
      .iter()
      .filter(|v| !matches!(v, Value::Null))
      .map(|v| v.to_css())
      .collect::<Vec<_>>()
      .join(self.sep.text());
    if self.bracketed {
      format!("[{}]", inner)
    } else {
      inner
    }
  }
}

/// An ordered VASS map
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BassMap {
  /// Ordered key/value entries (duplicate keys not allowed after `set`)
  pub entries: Vec<(Value, Value)>,
}

impl BassMap {
  /// An empty map
  pub fn new() -> Self {
    BassMap {
      entries: Vec::new(),
    }
  }

  /// Look up a key
  pub fn get(&self, key: &Value) -> Option<&Value> {
    self
      .entries
      .iter()
      .find(|(k, _)| values_equal(k, key))
      .map(|(_, v)| v)
  }

  /// Whether the map contains a key
  pub fn contains(&self, key: &Value) -> bool {
    self.entries.iter().any(|(k, _)| values_equal(k, key))
  }

  /// Insert or update a key
  pub fn set(&mut self, key: Value, value: Value) {
    for (k, v) in self.entries.iter_mut() {
      if values_equal(k, &key) {
        *v = value;
        return;
      }
    }
    self.entries.push((key, value));
  }

  /// Remove a key, returning its value
  pub fn remove(&mut self, key: &Value) -> Option<Value> {
    let idx = self
      .entries
      .iter()
      .position(|(k, _)| values_equal(k, key))?;
    Some(self.entries.remove(idx).1)
  }

  /// The keys in insertion order
  pub fn keys(&self) -> Vec<Value> {
    self.entries.iter().map(|(k, _)| k.clone()).collect()
  }

  /// The values in insertion order
  pub fn values(&self) -> Vec<Value> {
    self.entries.iter().map(|(_, v)| v.clone()).collect()
  }

  /// Recursive merge: `other` wins; nested maps merge deeply
  pub fn deep_merge(&self, other: &BassMap) -> BassMap {
    let mut out = self.clone();
    for (k, v) in &other.entries {
      let merged = match (out.get(k), v) {
        (Some(Value::Map(a)), Value::Map(b)) => Value::Map(a.deep_merge(b)),
        _ => v.clone(),
      };
      out.set(k.clone(), merged);
    }
    out
  }
}

/// An argument list captured by a rest parameter (`$args...`):
/// positional items plus named (keyword) arguments
#[derive(Debug, Clone, PartialEq)]
pub struct ArgListValue {
  /// Positional arguments
  pub list: ListValue,
  /// Named arguments in order
  pub named: Vec<(Value, Value)>,
}

impl ArgListValue {
  /// As a plain map of the named arguments
  pub fn as_map(&self) -> BassMap {
    BassMap {
      entries: self.named.clone(),
    }
  }
}

/// Reference to a function value (resolved by name at call time)
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionRef {
  /// Function name (`rgb`, `my-fn`, `math.div`, ...)
  pub name: String,
  /// Whether this is a user-defined function
  pub user_defined: bool,
}

/// Structural equality for VASS values (numbers compare with units,
/// lists/maps compare element-wise, colors compare by channels)
pub fn values_equal(a: &Value, b: &Value) -> bool {
  match (a, b) {
    (Value::Null, Value::Null) => true,
    (Value::Bool(x), Value::Bool(y)) => x == y,
    (Value::Number(x), Value::Number(y)) => x == y,
    (Value::Str(x), Value::Str(y)) => x == y,
    (Value::Color(x), Value::Color(y)) => x == y,
    (Value::List(x), Value::List(y)) => {
      x.sep == y.sep
        && x.bracketed == y.bracketed
        && x.items.len() == y.items.len()
        && x
          .items
          .iter()
          .zip(y.items.iter())
          .all(|(i, j)| values_equal(i, j))
    }
    (Value::Map(x), Value::Map(y)) => {
      x.entries.len() == y.entries.len()
        && x
          .entries
          .iter()
          .all(|(k, v)| y.get(k).is_some_and(|ov| values_equal(v, ov)))
    }
    (Value::Function(x), Value::Function(y)) => x == y,
    (Value::Mixin(x), Value::Mixin(y)) => x == y,
    _ => false,
  }
}

/// A VASS value
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
  /// `null`
  Null,
  /// `true` / `false`
  Bool(bool),
  /// A number, possibly with a unit (`10px`)
  Number(Number),
  /// A string, quoted or unquoted
  Str(BassString),
  /// A color
  Color(Color),
  /// A list of values
  List(ListValue),
  /// A map of values (`("a": 1)`)
  Map(BassMap),
  /// A first-class function reference
  Function(FunctionRef),
  /// A mixin reference (from `get-mixin` / `meta.get-mixin`)
  Mixin(String),
  /// An argument list captured by `$args...`
  ArgList(Box<ArgListValue>),
}

impl Value {
  /// VASS truthiness: only `false` and `null` are falsy
  pub fn is_truthy(&self) -> bool {
    !matches!(self, Value::Null | Value::Bool(false))
  }

  /// Whether this is a number
  pub fn as_number(&self) -> Option<&Number> {
    match self {
      Value::Number(n) => Some(n),
      _ => None,
    }
  }

  /// Whether this is a string
  pub fn as_string(&self) -> Option<&BassString> {
    match self {
      Value::Str(s) => Some(s),
      _ => None,
    }
  }

  /// Whether this is a map
  pub fn as_map(&self) -> Option<&BassMap> {
    match self {
      Value::Map(m) => Some(m),
      _ => None,
    }
  }

  /// The `type-of` name for this value
  pub fn type_name(&self) -> &'static str {
    match self {
      Value::Null => "null",
      Value::Bool(_) => "bool",
      Value::Number(_) => "number",
      Value::Str(_) => "string",
      Value::Color(_) => "color",
      Value::List(_) => "list",
      Value::Map(_) => "map",
      Value::Function(_) => "function",
      Value::Mixin(_) => "mixin",
      Value::ArgList(_) => "arglist",
    }
  }

  /// Render this value as CSS text (unquoted strings stay unquoted)
  pub fn to_css(&self) -> String {
    match self {
      Value::Null => "null".to_string(),
      Value::Bool(b) => b.to_string(),
      Value::Number(n) => n.to_css(),
      Value::Str(s) => s.to_css(),
      Value::Color(c) => c.to_css(),
      Value::List(l) => l.to_css(),
      Value::Map(m) => {
        // Maps are not valid in CSS output; render in VASS syntax
        let inner = m
          .entries
          .iter()
          .map(|(k, v)| format!("{}: {}", k.to_css(), v.to_css()))
          .collect::<Vec<_>>()
          .join(", ");
        format!("({})", inner)
      }
      Value::Function(f) => format!("get-function(\"{}\")", f.name),
      Value::Mixin(name) => format!("get-mixin(\"{}\")", name),
      Value::ArgList(a) => a.list.to_css(),
    }
  }

  /// Render this value in VASS source form (`inspect()`)
  pub fn inspect(&self) -> String {
    self.inspect_inner(true, true)
  }

  /// Render like the reference compiler's `@debug` output:
  /// - a top-level string is unquoted (`@debug "hello"` -> `hello`)
  /// - strings nested inside lists/maps keep their quotes (`["a", "b"]`)
  /// - numbers keep full f64 precision at every level
  pub fn to_debug_string(&self) -> String {
    match self {
      Value::Str(s) => s.text.clone(),
      other => other.inspect_inner(false, true),
    }
  }

  // `css_precision`: true -> 10-digit rounding (inspect mode);
  // false -> full-precision numbers (@debug mode). Strings are quoted in
  // both when `quote` is set.
  fn inspect_inner(&self, css_precision: bool, quote: bool) -> String {
    match self {
      Value::Null => "null".to_string(),
      Value::Bool(b) => b.to_string(),
      Value::Number(n) => {
        if css_precision {
          n.to_css()
        } else {
          n.to_debug_string()
        }
      }
      Value::Str(s) => {
        if s.quoted && quote {
          s.to_css()
        } else {
          s.text.clone()
        }
      }
      Value::Color(c) => {
        // inspect() shows literal colors in their source notation and
        // computed colors in their serialized form; @debug keeps
        // full float precision
        if css_precision {
          if let Some(t) = &c.literal {
            t.to_string()
          } else {
            c.to_css()
          }
        } else {
          c.to_debug_string()
        }
      }
      Value::List(l) => {
        let mut items: Vec<String> = Vec::new();
        for v in &l.items {
          let s = v.inspect_inner(css_precision, quote);
          // A comma-separated list nested inside a space-separated
          // list needs parens to stay unambiguous (`1 2 (3, 4)`)
          if l.sep == Separator::Space
            && let Value::List(inner) = v
            && inner.sep == Separator::Comma
            && inner.items.len() > 1
            && !inner.bracketed
          {
            items.push(format!("({})", s));
            continue;
          }
          items.push(s);
        }
        let inner = items.join(l.sep.text());
        if l.items.is_empty() {
          // An empty list inspects as `()`/`[]` regardless of separator
          if l.bracketed {
            "[]".to_string()
          } else {
            "()".to_string()
          }
        } else if l.items.len() == 1 && l.sep == Separator::Comma {
          // The reference inspects a single-element comma list with
          // a trailing comma: `(1,)`, `[1,]`
          if l.bracketed {
            format!("[{},]", inner)
          } else {
            format!("({},)", inner)
          }
        } else if l.bracketed {
          format!("[{}]", inner)
        } else {
          inner
        }
      }
      Value::Map(m) => {
        let inner = m
          .entries
          .iter()
          .map(|(k, v)| {
            format!(
              "{}: {}",
              k.inspect_inner(css_precision, quote),
              v.inspect_inner(css_precision, quote)
            )
          })
          .collect::<Vec<_>>()
          .join(", ");
        format!("({})", inner)
      }
      other => other.to_css(),
    }
  }
}

impl From<bool> for Value {
  fn from(b: bool) -> Self {
    Value::Bool(b)
  }
}

impl From<f64> for Value {
  fn from(v: f64) -> Self {
    Value::Number(Number::unitless(v))
  }
}

impl From<&str> for Value {
  fn from(s: &str) -> Self {
    Value::Str(BassString::unquoted(s))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_truthiness() {
    assert!(Value::Bool(true).is_truthy());
    assert!(!Value::Bool(false).is_truthy());
    assert!(!Value::Null.is_truthy());
    assert!(Value::Number(Number::unitless(0.0)).is_truthy());
  }

  #[test]
  fn test_list_to_css_drops_nulls() {
    let list = ListValue::new(
      vec![
        Value::Number(Number::new(1.0, Some("px".into()))),
        Value::Null,
        Value::Str(BassString::unquoted("solid")),
      ],
      Separator::Space,
    );
    assert_eq!(list.to_css(), "1px solid");
  }

  #[test]
  fn test_bracketed_list() {
    let list = ListValue::bracketed(vec![Value::from("a"), Value::from("b")], Separator::Space);
    assert_eq!(list.to_css(), "[a b]");
  }

  #[test]
  fn test_map_ops() {
    let mut m = BassMap::new();
    m.set(Value::from("a"), Value::from(1.0));
    m.set(Value::from("b"), Value::from(2.0));
    assert_eq!(m.get(&Value::from("a")), Some(&Value::from(1.0)));
    assert!(m.contains(&Value::from("b")));
    assert_eq!(m.keys().len(), 2);
    m.set(Value::from("a"), Value::from(9.0));
    assert_eq!(m.get(&Value::from("a")), Some(&Value::from(9.0)));
    assert_eq!(m.remove(&Value::from("a")), Some(Value::from(9.0)));
    assert_eq!(m.entries.len(), 1);
  }

  #[test]
  fn test_equality() {
    assert!(values_equal(
      &Value::List(ListValue::new(vec![Value::from(1.0)], Separator::Comma)),
      &Value::List(ListValue::new(vec![Value::from(1.0)], Separator::Comma)),
    ));
    assert!(!values_equal(
      &Value::Number(Number::with_unit(1.0, "px")),
      &Value::Number(Number::with_unit(1.0, "em")),
    ));
    assert!(values_equal(
      &Value::Map(BassMap::new()),
      &Value::Map(BassMap::new()),
    ));
  }

  #[test]
  fn test_inspect() {
    assert_eq!(Value::Str(BassString::unquoted("hi")).inspect(), "hi");
    assert_eq!(Value::Map(BassMap::new()).inspect(), "()");
    let mut m = BassMap::new();
    m.set(Value::from("k"), Value::from(1.5));
    assert_eq!(Value::Map(m).inspect(), "(k: 1.5)");
  }

  #[test]
  fn test_type_names() {
    assert_eq!(Value::Null.type_name(), "null");
    assert_eq!(Value::Bool(true).type_name(), "bool");
    assert_eq!(Value::Number(Number::unitless(1.0)).type_name(), "number");
    assert_eq!(Value::from("x").type_name(), "string");
    assert_eq!(Value::Color(Color::rgb(0, 0, 0)).type_name(), "color");
    assert_eq!(
      Value::List(ListValue::new(vec![], Separator::Comma)).type_name(),
      "list"
    );
    assert_eq!(Value::Map(BassMap::new()).type_name(), "map");
  }

  #[test]
  fn test_separator_text() {
    assert_eq!(Separator::Space.text(), " ");
    assert_eq!(Separator::Comma.text(), ", ");
    assert_eq!(Separator::Slash.text(), " / ");
    assert_eq!(Separator::Undecided.text(), " ");
  }

  #[test]
  fn test_list_value_new() {
    let v = Value::Null;
    let list = ListValue::new(vec![v], Separator::Comma);
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.sep, Separator::Comma);
    assert!(!list.bracketed);
  }
}
