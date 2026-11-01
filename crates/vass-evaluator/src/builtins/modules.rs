//! `vass:` namespace dispatch and member listings

use vass_core::error::Result;
use vass_core::value::Value;

use super::Args;

/// The list of known built-in module namespaces
pub const BUILTIN_MODULES: &[&str] =
  &["math", "color", "string", "list", "map", "selector", "meta"];

/// Dispatch a namespaced module function (`math.div`, ...)
pub fn call_module(ns: &str, name: &str, args: Args) -> Option<Result<Value>> {
  match ns {
    "math" => super::math::call(name, args),
    "color" => super::color::call(name, args),
    "string" => super::string_fn::call_string(name, args),
    "list" => super::list::call_list(name, args),
    "map" => super::map::call_map(name, args),
    "selector" => super::selector_fn::call(name, args),
    "meta" => super::meta::call(name, args),
    _ => None,
  }
}

/// Whether `ns` is a built-in module
pub fn is_builtin_module(ns: &str) -> bool {
  BUILTIN_MODULES.contains(&ns)
}

/// All exported function names of a built-in module
pub fn module_function_names(ns: &str) -> Vec<&'static str> {
  match ns {
    "math" => vec![
      "abs",
      "acos",
      "asin",
      "atan",
      "atan2",
      "ceil",
      "clamp",
      "compatible",
      "cos",
      "div",
      "exp",
      "floor",
      "hypot",
      "is-unitless",
      "log",
      "max",
      "min",
      "percentage",
      "pow",
      "random",
      "round",
      "sin",
      "sqrt",
      "tan",
      "unit",
      "unitless",
    ],
    "color" => vec![
      "adjust",
      "alpha",
      "blue",
      "change",
      "channel",
      "complement",
      "grayscale",
      "green",
      "hue",
      "ie-hex-str",
      "invert",
      "is-legacy",
      "is-missing",
      "is-powerless",
      "lightness",
      "mix",
      "opacity",
      "red",
      "same",
      "saturation",
      "scale",
      "space",
      "to-gamut",
      "to-space",
      "hwb",
    ],
    "string" => vec![
      "index",
      "insert",
      "length",
      "quote",
      "slice",
      "split",
      "to-lower-case",
      "to-upper-case",
      "unique-id",
      "unquote",
    ],
    "list" => vec![
      "append",
      "index",
      "is-bracketed",
      "join",
      "length",
      "nth",
      "separator",
      "set-nth",
      "slash",
      "zip",
    ],
    "map" => vec![
      "deep-get",
      "deep-merge",
      "get",
      "has-key",
      "keys",
      "merge",
      "remove",
      "set",
      "values",
    ],
    "selector" => vec![
      "append",
      "extend",
      "is-superselector",
      "nest",
      "parse",
      "replace",
      "unify",
    ],
    "meta" => vec![
      "calc-args",
      "calc-name",
      "call",
      "content-exists",
      "feature-exists",
      "function-exists",
      "get-function",
      "get-mixin",
      "global-variable-exists",
      "inspect",
      "keywords",
      "mixin-exists",
      "module-functions",
      "module-variables",
      "type-of",
      "variable-exists",
    ],
    _ => vec![],
  }
}

/// Exported variables of a built-in module (math.$pi etc.)
pub fn module_variable(ns: &str, name: &str) -> Option<Value> {
  match (ns, name) {
    ("math", "pi") => Some(Value::Number(vass_core::value::Number::unitless(
      std::f64::consts::PI,
    ))),
    ("math", "e") => Some(Value::Number(vass_core::value::Number::unitless(
      std::f64::consts::E,
    ))),
    ("math", "epsilon") => Some(Value::Number(vass_core::value::Number::unitless(
      f64::EPSILON,
    ))),
    ("math", "max-safe-integer") => Some(Value::Number(vass_core::value::Number::unitless(
      9_007_199_254_740_991.0,
    ))),
    ("math", "min-safe-integer") => Some(Value::Number(vass_core::value::Number::unitless(
      -9_007_199_254_740_991.0,
    ))),
    ("math", "max-number") => Some(Value::Number(vass_core::value::Number::unitless(f64::MAX))),
    ("math", "min-number") => Some(Value::Number(vass_core::value::Number::unitless(5e-324))),
    _ => None,
  }
}

/// All exported variables of a built-in module
pub fn module_variables(ns: &str) -> Vec<(&'static str, Value)> {
  match ns {
    "math" => [
      "pi",
      "e",
      "epsilon",
      "max-safe-integer",
      "min-safe-integer",
      "max-number",
      "min-number",
    ]
    .iter()
    .filter_map(|n| module_variable(ns, n).map(|v| (*n, v)))
    .collect(),
    _ => vec![],
  }
}
