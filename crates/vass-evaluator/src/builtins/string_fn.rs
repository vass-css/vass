//! `vass:string` module

use vass_core::error::Result;
use vass_core::value::{ListValue, Separator, Value};

use super::{Args, need_int, need_string, num_value, str_value};

/// Dispatch `string.<name>`; `None` if unknown
pub fn call_string(name: &str, args: Args) -> Option<Result<Value>> {
  let r = match name {
    "quote" => quote(args),
    "unquote" => unquote(args),
    "to-upper-case" => change_case(args, true),
    "to-lower-case" => change_case(args, false),
    "length" => length(args),
    "insert" => insert(args),
    "index" => index(args),
    "slice" => slice(args),
    "split" => split(args),
    "unique-id" => Ok(super::global_unique_id()),
    _ => return None,
  };
  Some(r)
}

fn quote(args: Args) -> Result<Value> {
  let s = need_string(
    args.require(0, "string", "string.quote")?,
    "string.quote",
    "string",
  )?;
  Ok(str_value(s, true))
}

fn unquote(args: Args) -> Result<Value> {
  let v = args.require(0, "string", "string.unquote")?;
  Ok(match v {
    Value::Str(s) => Value::Str(vass_core::value::BassString::unquoted(s.text.clone())),
    // Non-strings pass through unchanged (string.unquote(10px) -> 10px,
    // still a number — reference behavior)
    other => other.clone(),
  })
}

fn change_case(args: Args, upper: bool) -> Result<Value> {
  const F: &str = "string";
  let v = args.require(0, "string", F)?;
  match v {
    Value::Str(s) => {
      let text = if upper {
        s.text.to_ascii_uppercase()
      } else {
        s.text.to_ascii_lowercase()
      };
      Ok(Value::Str(vass_core::value::BassString::new(
        text, s.quoted,
      )))
    }
    other => Ok(other.clone()),
  }
}

fn length(args: Args) -> Result<Value> {
  let s = need_string(
    args.require(0, "string", "string.length")?,
    "string.length",
    "string",
  )?;
  Ok(num_value(s.chars().count() as f64))
}

fn insert(args: Args) -> Result<Value> {
  const F: &str = "string.insert";
  let s = need_string(args.require(0, "string", F)?, F, "string")?;
  let ins = need_string(args.require(1, "insert", F)?, F, "insert")?;
  let idx = need_int(args.require(2, "index", F)?, F, "index")?;
  let quoted = matches!(args.positional[0], Value::Str(s) if s.quoted);
  let chars: Vec<char> = s.chars().collect();
  let len = chars.len() as i64;
  let pos = if idx > 0 {
    (idx - 1).clamp(0, len)
  } else if idx < 0 {
    // Negative indexes count from the end: -1 appends after the last
    // character (reference behavior: insert("abcd", "X", -1) -> abcdX)
    (len + idx + 1).clamp(0, len)
  } else {
    0
  };
  let pos = pos.clamp(0, len) as usize;
  let mut out: String = chars[..pos].iter().collect();
  out.push_str(&ins);
  out.extend(chars[pos..].iter());
  Ok(str_value(out, quoted))
}

fn index(args: Args) -> Result<Value> {
  const F: &str = "string.index";
  let s = need_string(args.require(0, "string", F)?, F, "string")?;
  let sub = need_string(args.require(1, "substring", F)?, F, "substring")?;
  match s.find(&sub) {
    Some(b) => Ok(num_value(s[..b].chars().count() as f64 + 1.0)),
    None => Ok(Value::Null),
  }
}

fn slice(args: Args) -> Result<Value> {
  const F: &str = "string.slice";
  let s = need_string(args.require(0, "string", F)?, F, "string")?;
  let start = need_int(args.require(1, "start-at", F)?, F, "start-at")?;
  let end = match args.get(2, "end-at") {
    Some(v) if !matches!(v, Value::Null) => need_int(v, F, "end-at")?,
    _ => i64::MAX,
  };
  let quoted = matches!(args.positional[0], Value::Str(s) if s.quoted);
  let chars: Vec<char> = s.chars().collect();
  let len = chars.len() as i64;
  let norm = |i: i64| if i < 0 { len + i + 1 } else { i };
  let a = norm(start).clamp(1, len.max(1));
  let b = if end == i64::MAX {
    len
  } else {
    norm(end).clamp(0, len)
  };
  if a > b {
    return Ok(str_value("", quoted));
  }
  Ok(str_value(
    chars[(a - 1) as usize..b as usize]
      .iter()
      .collect::<String>(),
    quoted,
  ))
}

fn split(args: Args) -> Result<Value> {
  const F: &str = "string.split";
  let s = need_string(args.require(0, "string", F)?, F, "string")?;
  let sep = need_string(args.require(1, "separator", F)?, F, "separator")?;
  let limit = match args.get(2, "limit") {
    Some(v) if !matches!(v, Value::Null) => need_int(v, F, "limit")? as usize,
    _ => usize::MAX,
  };
  let mut parts: Vec<Value> = Vec::new();
  // The reference returns a *bracketed* comma list, and an empty separator
  // splits into individual characters (string.split("abc", "")).
  if sep.is_empty() {
    for ch in s.chars().take(limit) {
      parts.push(str_value(ch.to_string(), true));
    }
    return Ok(Value::List(ListValue {
      items: parts,
      sep: Separator::Comma,
      bracketed: true,
    }));
  }
  let mut rest = s.as_str();
  loop {
    if parts.len() + 1 >= limit {
      break;
    }
    match rest.find(&sep) {
      Some(b) => {
        parts.push(str_value(&rest[..b], true));
        rest = &rest[b + sep.len()..];
      }
      None => break,
    }
  }
  parts.push(str_value(rest, true));
  Ok(Value::List(ListValue {
    items: parts,
    sep: Separator::Comma,
    bracketed: true,
  }))
}
