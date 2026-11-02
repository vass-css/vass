//! `vass:meta` module
//!
//! Some meta functions need evaluator state (call, get-function,
//! existence checks) and are intercepted by the evaluator itself.

use vass_core::error::Result;
use vass_core::value::{ListValue, Separator, Value, VassMap};

use super::{Args, str_value};

/// Dispatch `meta.<name>`; `None` if unknown (or evaluator-intercepted)
pub fn call(name: &str, args: Args) -> Option<Result<Value>> {
  if !matches!(
    name,
    "type-of" | "inspect" | "keywords" | "calc-name" | "calc-args"
  ) {
    return None;
  }
  let r = (|| -> Result<Value> {
    match name {
      "type-of" => Ok(str_value(
        args.require(0, "value", "meta.type-of")?.type_name(),
        false,
      )),
      "inspect" => Ok(str_value(
        args.require(0, "value", "meta.inspect")?.inspect(),
        false,
      )),
      "keywords" => match args.require(0, "args", "meta.keywords")? {
        Value::ArgList(a) => Ok(Value::Map(a.as_map())),
        other => Ok(Value::Map(match other {
          Value::Map(m) => m.clone(),
          _ => VassMap::new(),
        })),
      },
      // Calculations are represented as unquoted `name(...)` strings;
      // decompose them textually, matching the reference API shape
      "calc-name" => {
        let v = args.require(0, "calc", "meta.calc-name")?;
        let s = calc_text(v, "meta.calc-name")?;
        let name = s.split('(').next().unwrap_or("").trim().to_string();
        if name.is_empty() {
          return Err(vass_core::error::Error::type_error(
            "meta.calc-name: $calc must be a calculation.",
          ));
        }
        Ok(str_value(name, false))
      }
      "calc-args" => {
        let v = args.require(0, "calc", "meta.calc-args")?;
        let s = calc_text(v, "meta.calc-args")?;
        let open = s.find('(').ok_or_else(|| {
          vass_core::error::Error::type_error("meta.calc-args: $calc must be a calculation.")
        })?;
        let inner = s[open + 1..].trim_end_matches(')');
        let mut items: Vec<Value> = Vec::new();
        let mut depth = 0usize;
        let mut cur = String::new();
        for ch in inner.chars() {
          match ch {
            '(' => {
              depth += 1;
              cur.push(ch);
            }
            ')' => {
              depth = depth.saturating_sub(1);
              cur.push(ch);
            }
            ',' if depth == 0 => {
              items.push(str_value(cur.trim().to_string(), false));
              cur.clear();
            }
            _ => cur.push(ch),
          }
        }
        if !cur.trim().is_empty() {
          items.push(str_value(cur.trim().to_string(), false));
        }
        Ok(Value::List(ListValue::new(items, Separator::Comma)))
      }
      _ => unreachable!("checked above"),
    }
  })();
  Some(r)
}

/// Read a calculation argument (an unquoted `name(...)` string)
fn calc_text(v: &Value, fname: &str) -> Result<String> {
  match v {
    Value::Str(s) if !s.quoted => Ok(s.text.clone()),
    other => Err(vass_core::error::Error::type_error(format!(
      "{}: $calc must be a calculation (got {}).",
      fname,
      other.inspect()
    ))),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::value::{Value, VassMap, VassString};

  #[test]
  fn test_calc_text_valid_unquoted() {
    let v = Value::Str(VassString::unquoted("calc(10px + 20px)".to_string()));
    let result = calc_text(&v, "meta.test");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "calc(10px + 20px)");
  }

  #[test]
  fn test_calc_text_quoted_string_rejected() {
    let v = Value::Str(VassString::quoted("calc(10px)".to_string()));
    let result = calc_text(&v, "meta.test");
    assert!(result.is_err());
  }

  #[test]
  fn test_calc_text_number_rejected() {
    let v = Value::Number(vass_core::value::Number::unitless(42.0));
    let result = calc_text(&v, "meta.test");
    assert!(result.is_err());
  }

  #[test]
  fn test_calc_text_empty_unquoted() {
    let v = Value::Str(VassString::unquoted("".to_string()));
    let result = calc_text(&v, "meta.test");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
  }

  #[test]
  fn test_call_unknown_name_returns_none() {
    let args = Args {
      positional: Vec::new(),
      named: Vec::new(),
    };
    let result = call("nonexistent", args);
    assert!(result.is_none());
  }

  #[test]
  fn test_call_type_of() {
    let num = Value::Number(vass_core::value::Number::unitless(42.0));
    let args = Args {
      positional: vec![&num],
      named: Vec::new(),
    };
    let result = call("type-of", args);
    assert!(result.is_some());
    let result = result.unwrap().unwrap();
    assert_eq!(result, Value::from("number"));
  }

  #[test]
  fn test_call_inspect() {
    let s = Value::from("hello");
    let args = Args {
      positional: vec![&s],
      named: Vec::new(),
    };
    let result = call("inspect", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_call_keywords_map() {
    let map = VassMap::new();
    let binding = Value::Map(map);
    let args = Args {
      positional: vec![&binding],
      named: Vec::new(),
    };
    let result = call("keywords", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_call_keywords_non_map() {
    let s = Value::from("not a map");
    let args = Args {
      positional: vec![&s],
      named: Vec::new(),
    };
    let result = call("keywords", args);
    assert!(result.is_some());
    let result = result.unwrap().unwrap();
    match result {
      Value::Map(m) => assert!(m.entries.is_empty()),
      _ => panic!("expected Map"),
    }
  }

  #[test]
  fn test_call_missing_arg() {
    let args = Args {
      positional: Vec::new(),
      named: Vec::new(),
    };
    let result = call("type-of", args);
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(result.is_err());
  }

  #[test]
  fn test_call_calc_name_rejects_empty_name() {
    let s = Value::Str(VassString::unquoted("(something)".to_string()));
    let args = Args {
      positional: vec![&s],
      named: Vec::new(),
    };
    let result = call("calc-name", args);
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(result.is_err());
  }

  #[test]
  fn test_call_calc_args_rejects_no_paren() {
    let s = Value::from("not-a-calc");
    let args = Args {
      positional: vec![&s],
      named: Vec::new(),
    };
    let result = call("calc-args", args);
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(result.is_err());
  }
}
