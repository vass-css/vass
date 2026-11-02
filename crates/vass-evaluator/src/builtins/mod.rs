//! Built-in function library: global functions and `vass:` modules
//!
//! Pure value-in/value-out implementations live here. Builtins that need
//! evaluator state (`if()`, `call()`, existence checks, ...) are handled
//! by the evaluator before dispatch reaches this module.

mod color;
mod global;
mod list;
mod map;
mod math;
mod meta;
mod modules;
mod selector_fn;
mod string_fn;

pub(crate) use global::min_max_values;
pub use global::{call_global, global_fn_names, global_unique_id};
pub use modules::{
  BUILTIN_MODULES, call_module, is_builtin_module, module_function_names, module_variable,
  module_variables,
};

use vass_core::error::{Error, Result};
use vass_core::value::{ListValue, Separator, Value, VassMap, VassString};

/// Evaluated call arguments: positional plus named
#[derive(Debug, Default)]
pub struct Args<'a> {
  /// Positional argument values
  pub positional: Vec<&'a Value>,
  /// Named argument values in order
  pub named: Vec<(String, &'a Value)>,
}

impl<'a> Args<'a> {
  /// Fetch an argument by position, falling back to its keyword name
  pub fn get(&self, index: usize, name: &str) -> Option<&'a Value> {
    if let Some(v) = self.positional.get(index) {
      return Some(v);
    }
    self.named.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
  }

  /// Fetch a named-only argument (`$method` etc.)
  pub fn get_named(&self, name: &str) -> Option<&'a Value> {
    self.named.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
  }

  /// Fetch a required argument or fail
  pub fn require(&self, index: usize, name: &str, fname: &str) -> Result<&'a Value> {
    self
      .get(index, name)
      .ok_or_else(|| Error::type_error(format!("{}() missing argument `${}`.", fname, name)))
  }

  /// Whether extra (unrecognized) arguments were passed
  pub fn has_extra(&self, max_positional: usize) -> bool {
    self.positional.len() > max_positional || !self.named.is_empty()
  }

  /// The named argument map (for rest/keyword handling)
  pub fn named_map(&self) -> VassMap {
    VassMap {
      entries: self
        .named
        .iter()
        .map(|(k, v)| (Value::Str(VassString::unquoted(k.clone())), (*v).clone()))
        .collect(),
    }
  }
}

// ---- value coercion helpers -------------------------------------------------

/// Coerce to a number
pub fn need_number<'a>(
  v: &'a Value,
  fname: &str,
  arg: &str,
) -> Result<&'a vass_core::value::Number> {
  v.as_number().ok_or_else(|| {
    Error::type_error(format!(
      "{}: `${}` is not a number (got {}).",
      fname,
      arg,
      v.inspect()
    ))
  })
}

/// Coerce to a number value in the given unit scale (`%` → /100)
pub fn number_scaled(v: &Value, fname: &str, arg: &str) -> Result<f64> {
  let n = need_number(v, fname, arg)?;
  Ok(match n.unit.as_deref() {
    Some("%") => n.value / 100.0,
    Some(other) => {
      return Err(Error::type_error(format!(
        "{}: `${}` must be unitless or `%`, got `{}`.",
        fname, arg, other
      )));
    }
    None => n.value,
  })
}

/// Coerce to a color
pub fn need_color(v: &Value, fname: &str, arg: &str) -> Result<vass_core::value::Color> {
  match v {
    Value::Color(c) => Ok(c.clone()),
    _ => Err(Error::type_error(format!(
      "{}: `${}` is not a color (got {}).",
      fname,
      arg,
      v.inspect()
    ))),
  }
}

/// Coerce to string text (quoted or unquoted)
pub fn need_string(v: &Value, _fname: &str, arg: &str) -> Result<String> {
  match v {
    Value::Str(s) => Ok(s.text.clone()),
    // The reference rejects non-strings for $string parameters
    // (`string.length(10px)` -> "$string: 10px is not a string.")
    _ => Err(Error::type_error(format!(
      "${}: {} is not a string.",
      arg,
      v.inspect()
    ))),
  }
}

/// Coerce to an integer (1-based VASS indexing)
pub fn need_int(v: &Value, fname: &str, arg: &str) -> Result<i64> {
  let n = need_number(v, fname, arg)?;
  if !n.is_unitless() {
    return Err(Error::type_error(format!(
      "{}: `${}` must be unitless.",
      fname, arg
    )));
  }
  Ok(n.value.round() as i64)
}

/// Coerce any value to a list (maps become lists of 2-element lists;
/// single values become single-item lists)
pub fn to_list(v: &Value) -> (Vec<Value>, Separator, bool) {
  match v {
    Value::List(l) => (l.items.clone(), l.sep, l.bracketed),
    Value::Map(m) => (
      m.entries
        .iter()
        .map(|(k, val)| {
          Value::List(ListValue::new(
            vec![k.clone(), val.clone()],
            Separator::Space,
          ))
        })
        .collect(),
      Separator::Comma,
      false,
    ),
    Value::ArgList(a) => (a.list.items.clone(), Separator::Comma, false),
    // Scalars and null carry no decided separator (the reference's
    // ListSeparator.undecided): join/append auto rules fall through
    Value::Null => (vec![], Separator::Undecided, false),
    other => (vec![other.clone()], Separator::Undecided, false),
  }
}

/// Coerce to a map (arglists use their named entries)
pub fn need_map(v: &Value, fname: &str, arg: &str) -> Result<VassMap> {
  match v {
    Value::Map(m) => Ok(m.clone()),
    Value::ArgList(a) => Ok(a.as_map()),
    Value::List(l) if l.items.is_empty() => Ok(VassMap::new()),
    _ => Err(Error::type_error(format!(
      "{}: `${}` is not a map (got {}).",
      fname,
      arg,
      v.inspect()
    ))),
  }
}

/// Build a string value
pub fn str_value(text: impl Into<String>, quoted: bool) -> Value {
  Value::Str(VassString::new(text, quoted))
}

/// Build an unquoted string value
pub fn unquoted(text: impl Into<String>) -> Value {
  Value::Str(VassString::unquoted(text))
}

/// Build a unitless number value
pub fn num_value(v: f64) -> Value {
  Value::Number(vass_core::value::Number::unitless(v))
}

/// Build a number with a unit
pub fn num_unit(v: f64, unit: &str) -> Value {
  Value::Number(vass_core::value::Number::with_unit(v, unit))
}

/// An "expected N arguments" type error
pub fn arg_error(fname: &str, expected: &str, got: usize) -> Error {
  Error::type_error(format!(
    "{}() expects {} argument(s), got {}.",
    fname, expected, got
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_args_positional_and_named() {
    let a = Value::from(1.0);
    let b = Value::from(2.0);
    let args = Args {
      positional: vec![&a],
      named: vec![("x".to_string(), &b)],
    };
    assert_eq!(args.get(0, "y"), Some(&a));
    assert_eq!(args.get(1, "x"), Some(&b));
    assert!(args.require(0, "y", "f").is_ok());
    assert!(args.require(5, "z", "f").is_err());
  }

  #[test]
  fn test_to_list() {
    let (items, sep, _) = to_list(&Value::from("a"));
    assert_eq!(items.len(), 1);
    // Scalars carry no decided separator (reference `undecided`):
    // join/append auto rules fall through to the other operand
    assert_eq!(sep, Separator::Undecided);
    let (items, sep, _) = to_list(&Value::Map(VassMap::new()));
    assert_eq!(items.len(), 0);
    assert_eq!(sep, Separator::Comma);
  }
}
