//! `vass:map` module

use vass_core::error::{Error, Result};
use vass_core::value::{ListValue, Separator, Value};

use super::{Args, need_map};

// ---- map module ------------------------------------------------------------

/// Dispatch `map.<name>`; `None` if unknown
pub fn call_map(name: &str, args: Args) -> Option<Result<Value>> {
  let r = match name {
    "get" => get(args),
    "merge" => merge(args),
    "remove" => remove(args),
    "keys" => keys(args),
    "values" => values(args),
    "has-key" => has_key(args),
    "set" => set(args),
    "deep-merge" => deep_merge(args),
    "deep-remove" => deep_remove(args),
    "deep-get" => deep_get(args),
    _ => return None,
  };
  Some(r)
}

fn get(args: Args) -> Result<Value> {
  const F: &str = "map.get";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  if args.positional.len() < 2 {
    return Err(vass_core::error::Error::type_error(format!(
      "{}() expects a map and a key.",
      F
    )));
  }
  let mut cur = m;
  let mut result = Value::Null;
  for (i, k) in args.positional.iter().enumerate().skip(1) {
    match cur.get(k) {
      Some(Value::Map(inner)) => {
        cur = (*inner).clone();
        result = Value::Map(cur.clone());
      }
      Some(v) => {
        result = v.clone();
        if i + 1 < args.positional.len() {
          return Ok(Value::Null);
        }
      }
      None => return Ok(Value::Null),
    }
  }
  Ok(result)
}

fn merge(args: Args) -> Result<Value> {
  const F: &str = "map.merge";
  if args.positional.len() < 2 {
    return Err(vass_core::error::Error::type_error(format!(
      "{}() expects $map1, $keys..., $map2.",
      F
    )));
  }
  let a = need_map(args.require(0, "map1", F)?, F, "map1")?;
  let b = need_map(args.positional.last().unwrap(), F, "map2")?;
  // Reference signature: `map.merge($map1, $keys..., $map2)` — the last
  // argument is always the overlay map; everything in between is a key path.
  let keys: Vec<Value> = args.positional[1..args.positional.len() - 1]
    .iter()
    .map(|v| (*v).clone())
    .collect();
  if keys.is_empty() {
    let mut out = a;
    for (k, v) in b.entries {
      out.set(k, v);
    }
    return Ok(Value::Map(out));
  }
  let mut out = a;
  merge_path(&mut out, &keys, b);
  Ok(Value::Map(out))
}

/// Deep-path merge for `map.merge($map1, $keys..., $map2)`: navigate `keys`
/// through `map`, merging `overlay` into the map found at the path. A missing
/// key creates the path; an existing non-map value at the path is replaced,
/// matching the reference implementation.
fn merge_path(
  map: &mut vass_core::value::BassMap,
  keys: &[Value],
  overlay: vass_core::value::BassMap,
) {
  if keys.len() == 1 {
    let target = match map.get(&keys[0]) {
      Some(Value::Map(old)) => {
        let mut m = old.clone();
        for (k, v) in overlay.entries {
          m.set(k, v);
        }
        Value::Map(m)
      }
      // Missing key or existing non-map value: the overlay replaces it.
      _ => Value::Map(overlay),
    };
    map.set(keys[0].clone(), target);
    return;
  }
  let mut inner = match map.get(&keys[0]) {
    Some(Value::Map(m)) => m.clone(),
    _ => vass_core::value::BassMap::new(),
  };
  merge_path(&mut inner, &keys[1..], overlay);
  map.set(keys[0].clone(), Value::Map(inner));
}

fn remove(args: Args) -> Result<Value> {
  const F: &str = "map.remove";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  let mut out = m;
  for k in &args.positional[1..] {
    out.remove(k);
  }
  Ok(Value::Map(out))
}

fn keys(args: Args) -> Result<Value> {
  const F: &str = "map.keys";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  // An empty map's keys/values inspect as `()` (a space-separated empty
  // list), matching the reference.
  let sep = if m.keys().is_empty() {
    Separator::Space
  } else {
    Separator::Comma
  };
  Ok(Value::List(ListValue::new(m.keys(), sep)))
}

fn values(args: Args) -> Result<Value> {
  const F: &str = "map.values";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  let sep = if m.values().is_empty() {
    Separator::Space
  } else {
    Separator::Comma
  };
  Ok(Value::List(ListValue::new(m.values(), sep)))
}

fn has_key(args: Args) -> Result<Value> {
  const F: &str = "map.has-key";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  if args.positional.len() < 2 {
    return Err(vass_core::error::Error::type_error(format!(
      "{}() expects a map and a key.",
      F
    )));
  }
  // Reference semantics: a key path only exists if every intermediate
  // value along the way is itself a map.
  let n = args.positional.len();
  let mut cur = m;
  for (i, k) in args.positional.iter().enumerate().skip(1) {
    let last = i + 1 == n;
    match cur.get(k) {
      Some(Value::Map(inner)) => {
        if last {
          return Ok(Value::Bool(true));
        }
        cur = (*inner).clone();
      }
      Some(_) => return Ok(Value::Bool(last)),
      None => return Ok(Value::Bool(false)),
    }
  }
  Ok(Value::Bool(true))
}

fn set(args: Args) -> Result<Value> {
  const F: &str = "map.set";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  if args.positional.len() < 3 {
    return Err(vass_core::error::Error::type_error(format!(
      "{}() expects $map, $keys..., $value.",
      F
    )));
  }
  let value = (*args.positional.last().unwrap()).clone();
  let keys: Vec<Value> = args.positional[1..args.positional.len() - 1]
    .iter()
    .map(|v| (*v).clone())
    .collect();
  let mut out = m;
  set_path(&mut out, &keys, value);
  Ok(Value::Map(out))
}

fn set_path(map: &mut vass_core::value::BassMap, keys: &[Value], value: Value) {
  if keys.len() == 1 {
    map.set(keys[0].clone(), value);
    return;
  }
  let inner = map
    .get(&keys[0])
    .cloned()
    .unwrap_or(Value::Map(vass_core::value::BassMap::new()));
  let mut inner_map = match inner {
    Value::Map(m) => m,
    _ => vass_core::value::BassMap::new(),
  };
  set_path(&mut inner_map, &keys[1..], value);
  map.set(keys[0].clone(), Value::Map(inner_map));
}

fn deep_merge(args: Args) -> Result<Value> {
  const F: &str = "map.deep-merge";
  let a = need_map(args.require(0, "map1", F)?, F, "map1")?;
  let b = need_map(args.require(1, "map2", F)?, F, "map2")?;
  Ok(Value::Map(a.deep_merge(&b)))
}

fn deep_get(args: Args) -> Result<Value> {
  const F: &str = "map.deep-get";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  if args.positional.len() < 2 {
    return Err(vass_core::error::Error::type_error(format!(
      "{}() expects $map and $keys...",
      F
    )));
  }
  let mut cur = Value::Map(m);
  for k in &args.positional[1..] {
    match &cur {
      Value::Map(mm) => match mm.get(k) {
        Some(v) => cur = v.clone(),
        None => return Ok(Value::Null),
      },
      _ => return Ok(Value::Null),
    }
  }
  Ok(cur)
}

/// `map.deep-remove($map, $key, $keys...)`: remove a value from a nested map
/// by walking the key path; a single key removes at the top level.
fn deep_remove(args: Args) -> Result<Value> {
  const F: &str = "map.deep-remove";
  let map_val = args.require(0, "map", F)?;
  let first = args.require(1, "key", F)?.clone();
  let rest: Vec<Value> = args
    .positional
    .iter()
    .skip(2)
    .map(|v| (*v).clone())
    .collect();
  remove_nested(map_val, &first, &rest, F)
}

fn remove_nested(map_val: &Value, key: &Value, rest: &[Value], fname: &str) -> Result<Value> {
  let mut m = need_map_ref(map_val, fname)?;
  let idx = match m
    .entries
    .iter()
    .position(|(k, _)| vass_core::value::values_equal(k, key))
  {
    Some(i) => i,
    // Reference behaviour: a missing key at any level returns the map
    // unchanged (single-key calls behave like `map.remove`).
    None => return Ok(Value::Map(m)),
  };
  if rest.is_empty() {
    m.entries.remove(idx);
    return Ok(Value::Map(m));
  }
  let (_, inner) = m.entries[idx].clone();
  if !matches!(inner, Value::Map(_)) {
    // A non-map value in the middle of the path: nothing to remove.
    return Ok(Value::Map(m));
  }
  let new_inner = remove_nested(&inner, &rest[0], &rest[1..], fname)?;
  m.entries[idx].1 = new_inner;
  Ok(Value::Map(m))
}

/// Borrow a map value without consuming it — mirrors `need_map`'s tolerance
/// (arg lists and empty lists count as maps)
fn need_map_ref(v: &Value, fname: &str) -> Result<vass_core::value::BassMap> {
  match v {
    Value::Map(m) => Ok(m.clone()),
    Value::ArgList(a) => Ok(a.as_map()),
    Value::List(l) if l.items.is_empty() => Ok(vass_core::value::BassMap::new()),
    other => Err(Error::type_error(format!(
      "{}: `$map` is not a map (got {}).",
      fname,
      other.inspect()
    ))),
  }
}
