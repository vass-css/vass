//! `vass:meta` module
//!
//! Some meta functions need evaluator state (call, get-function,
//! existence checks) and are intercepted by the evaluator itself.

use vass_core::error::Result;
use vass_core::value::{BassMap, ListValue, Separator, Value};

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
          _ => BassMap::new(),
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
