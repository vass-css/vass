//! `vass:list` module

use vass_core::error::Result;
use vass_core::value::{ListValue, Separator, Value};

use super::{Args, need_int, num_value, str_value, to_list};

/// Known `list.` function names
const LIST_FNS: &[&str] = &[
  "length",
  "nth",
  "set-nth",
  "join",
  "append",
  "zip",
  "index",
  "separator",
  "is-bracketed",
  "slash",
];

/// Dispatch `list.<name>`; `None` if unknown
pub fn call_list(name: &str, args: Args) -> Option<Result<Value>> {
  if !LIST_FNS.contains(&name) {
    return None;
  }
  let r = (|| -> Result<Value> {
    match name {
      "length" => {
        let (items, _, _) = to_list(args.require(0, "list", "list.length")?);
        Ok(num_value(items.len() as f64))
      }
      "nth" => nth(args),
      "set-nth" => set_nth(args),
      "join" => join(args),
      "append" => append(args),
      "zip" => zip(args),
      "index" => index(args),
      "separator" => separator(args),
      "is-bracketed" => is_bracketed(args),
      "slash" => slash(args),
      _ => unreachable!("checked above"),
    }
  })();
  Some(r)
}

fn nth(args: Args) -> Result<Value> {
  const F: &str = "list.nth";
  let (items, _, _) = to_list(args.require(0, "list", F)?);
  let n = need_int(args.require(1, "n", F)?, F, "n")?;
  if items.is_empty() {
    return Err(vass_core::error::Error::type_error(
      "list.nth: list is empty.",
    ));
  }
  let len = items.len() as i64;
  let idx = if n < 0 { len + n } else { n - 1 };
  if idx < 0 || idx >= len {
    return Err(vass_core::error::Error::type_error(format!(
      "list.nth: index {} out of range.",
      n
    )));
  }
  Ok(items[idx as usize].clone())
}

fn set_nth(args: Args) -> Result<Value> {
  const F: &str = "list.set-nth";
  let (mut items, sep, bracketed) = to_list(args.require(0, "list", F)?);
  let n = need_int(args.require(1, "n", F)?, F, "n")?;
  let value = args.require(2, "value", F)?.clone();
  let len = items.len() as i64;
  let idx = if n < 0 { len + n } else { n - 1 };
  if idx < 0 || idx >= len {
    return Err(vass_core::error::Error::type_error(format!(
      "list.set-nth: index {} out of range.",
      n
    )));
  }
  items[idx as usize] = value;
  Ok(Value::List(ListValue {
    items,
    sep,
    bracketed,
  }))
}

fn join(args: Args) -> Result<Value> {
  const F: &str = "list.join";
  let av = args.require(0, "list1", F)?;
  let bv = args.require(1, "list2", F)?;
  let (a, sep_a, br_a) = to_list(av);
  let (b, sep_b, _br_b) = to_list(bv);
  // $separator: auto — undecided falls through to the other operand
  // ((undecided, undecided) -> space; (undecided, s) -> s; (s, _) -> s)
  let sep = match args.get(2, "separator") {
    Some(Value::Str(s)) => match s.text.as_str() {
      "auto" => auto_join_sep(sep_a, sep_b),
      "comma" => Separator::Comma,
      "space" => Separator::Space,
      "slash" => Separator::Slash,
      other => {
        return Err(vass_core::error::Error::type_error(format!(
          "$separator: Must be \"space\", \"comma\", \"slash\", or \"auto\". (got \"{}\")",
          other
        )));
      }
    },
    _ => auto_join_sep(sep_a, sep_b),
  };
  // $bracketed: auto keeps list1's brackets; any other value is truthy
  let bracketed = match args.get(3, "bracketed") {
    None => br_a,
    Some(Value::Str(s)) if s.text == "auto" => br_a,
    Some(v) => v.is_truthy(),
  };
  // Joining with an empty list returns the other operand unchanged
  // (a scalar stays a scalar: join((), 1) -> 1, reference behavior).
  if a.is_empty() && b.is_empty() {
    return Ok(Value::List(ListValue {
      items: vec![],
      sep,
      bracketed,
    }));
  }
  if a.is_empty() {
    return Ok(Value::List(ListValue {
      items: b,
      sep,
      bracketed,
    }));
  }
  if b.is_empty() {
    return Ok(Value::List(ListValue {
      items: a,
      sep,
      bracketed,
    }));
  }
  let mut items = a;
  items.extend(b);
  Ok(Value::List(ListValue {
    items,
    sep,
    bracketed,
  }))
}

/// The reference's join auto-separator rule over (list1, list2) separators
fn auto_join_sep(sep_a: Separator, sep_b: Separator) -> Separator {
  match (sep_a, sep_b) {
    (Separator::Undecided, Separator::Undecided) => Separator::Space,
    (Separator::Undecided, s) => s,
    (s, _) => s,
  }
}

fn append(args: Args) -> Result<Value> {
  const F: &str = "list.append";
  let av = args.require(0, "list", F)?;
  let value = args.require(1, "val", F)?.clone();
  let (mut items, sep_orig, bracketed) = to_list(av);
  // $separator: auto keeps the input's separator unless it is undecided
  // (scalars and `()`), in which case the result is space-separated
  let sep = match args.get(2, "separator") {
    Some(Value::Str(s)) => match s.text.as_str() {
      "auto" => auto_append_sep(sep_orig),
      "comma" => Separator::Comma,
      "space" => Separator::Space,
      "slash" => Separator::Slash,
      other => {
        return Err(vass_core::error::Error::type_error(format!(
          "$separator: Must be \"space\", \"comma\", \"slash\", or \"auto\". (got \"{}\")",
          other
        )));
      }
    },
    _ => auto_append_sep(sep_orig),
  };
  // Appending to an empty list yields the value itself (a scalar stays a
  // scalar: list.append((), 1) -> 1, reference behavior) unless an
  // explicit separator was requested.
  let explicit_sep = matches!(args.get(2, "separator"), Some(Value::Str(s)) if s.text != "auto");
  if items.is_empty() && !bracketed && !explicit_sep {
    return Ok(value);
  }
  items.push(value);
  Ok(Value::List(ListValue {
    items,
    sep,
    bracketed,
  }))
}

/// The reference's append auto-separator rule
fn auto_append_sep(sep: Separator) -> Separator {
  if sep == Separator::Undecided {
    Separator::Space
  } else {
    sep
  }
}

fn zip(args: Args) -> Result<Value> {
  const F: &str = "list.zip";
  let lists: Vec<Vec<Value>> = args.positional.iter().map(|v| to_list(v).0).collect();
  let min_len = lists.iter().map(|l| l.len()).min().unwrap_or(0);
  let mut items = Vec::with_capacity(min_len);
  for i in 0..min_len {
    items.push(Value::List(ListValue::new(
      lists.iter().map(|l| l[i].clone()).collect(),
      Separator::Space,
    )));
  }
  Ok(Value::List(ListValue::new(items, Separator::Comma)))
}

fn index(args: Args) -> Result<Value> {
  const F: &str = "list.index";
  let (items, _, _) = to_list(args.require(0, "list", F)?);
  let needle = args.require(1, "value", F)?;
  for (i, item) in items.iter().enumerate() {
    if vass_core::value::values_equal(item, needle) {
      return Ok(num_value(i as f64 + 1.0));
    }
  }
  Ok(Value::Null)
}

fn separator(args: Args) -> Result<Value> {
  let v = args.require(0, "list", "list.separator")?;
  // The reference returns an unquoted string; undecided separators (empty
  // lists, `[a]`, scalars) report `space`, slash lists report `slash`
  let name = match v {
    Value::List(l) => match l.sep {
      Separator::Comma => "comma",
      Separator::Slash => "slash",
      Separator::Space | Separator::Undecided => "space",
    },
    _ => "space",
  };
  Ok(str_value(name, false))
}

fn is_bracketed(args: Args) -> Result<Value> {
  let (_, _, bracketed) = to_list(args.require(0, "list", "list.is-bracketed")?);
  Ok(Value::Bool(bracketed))
}

fn slash(args: Args) -> Result<Value> {
  let mut items: Vec<Value> = Vec::new();
  for v in &args.positional {
    items.push((*v).clone());
  }
  if items.len() < 2 {
    return Err(vass_core::error::Error::type_error(
      "At least two elements are required.",
    ));
  }
  Ok(Value::List(ListValue {
    items,
    sep: Separator::Slash,
    bracketed: false,
  }))
}
