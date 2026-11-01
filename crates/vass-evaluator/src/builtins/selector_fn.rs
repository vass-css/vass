//! `vass:selector` module — pragmatic string-based selector algebra

use vass_core::error::{Error, Result};
use vass_core::value::{ListValue, Separator, Value};

use super::{Args, str_value};

/// Dispatch `selector.<name>`; `None` if unknown
pub fn call(name: &str, args: Args) -> Option<Result<Value>> {
  let r = match name {
    "nest" => nest(args),
    "append" => append(args),
    "unify" => unify(args),
    "is-superselector" => superselector(args),
    "parse" => parse(args),
    "replace" => replace(args),
    "extend" => extend(args),
    _ => return None,
  };
  Some(r)
}

/// Read a selector argument (string or list of strings)
fn selector_text(v: &Value, fname: &str, arg: &str) -> Result<String> {
  match v {
    Value::Str(s) => Ok(s.text.clone()),
    Value::List(l) => Ok(
      l.items
        .iter()
        .map(|i| match i {
          Value::Str(s) => s.text.clone(),
          other => other.to_css(),
        })
        .collect::<Vec<_>>()
        .join(", "),
    ),
    other => Err(Error::type_error(format!(
      "{}: `${}` is not a selector (got {}).",
      fname,
      arg,
      other.inspect()
    ))),
  }
}

/// Wrap a selector result the way the reference does: selector functions
/// return a comma-separated list of complex selectors (a single selector is
/// a one-element comma list, inspected as `(.a.b,)`).
fn selector_value(s: &str) -> Value {
  let items: Vec<Value> = if s.is_empty() {
    vec![]
  } else {
    s.split(',').map(|p| str_value(p.trim(), false)).collect()
  };
  Value::List(ListValue {
    items,
    sep: Separator::Comma,
    bracketed: false,
  })
}

pub(crate) fn nest(args: Args) -> Result<Value> {
  const F: &str = "selector.nest";
  let mut current: Option<String> = None;
  for (i, v) in args.positional.iter().enumerate() {
    let sel = selector_text(v, F, if i == 0 { "selector" } else { "selectors..." })?;
    let parts: Vec<&str> = sel.split(',').map(|s| s.trim()).collect();
    let resolved: Vec<String> = match &current {
      None => parts.iter().map(|s| s.to_string()).collect(),
      Some(parent) => {
        let mut out = Vec::new();
        for p in parts {
          if p.contains('&') {
            out.push(p.replace('&', parent));
          } else {
            out.push(format!("{} {}", parent, p));
          }
        }
        out
      }
    };
    current = Some(resolved.join(", "));
  }
  Ok(selector_value(&current.unwrap_or_default()))
}

pub(crate) fn append(args: Args) -> Result<Value> {
  const F: &str = "selector.append";
  let mut current: Option<String> = None;
  for (i, v) in args.positional.iter().enumerate() {
    let sel = selector_text(v, F, if i == 0 { "selector" } else { "selectors..." })?;
    current = Some(match &current {
      None => sel,
      Some(parent) => {
        let parts: Vec<&str> = sel.split(',').map(str::trim).collect();
        parts
          .iter()
          .map(|p| {
            let combos: Vec<String> = parent
              .split(',')
              .map(|pp| {
                let pp = pp.trim();
                // The reference always concatenates the
                // appended compound onto the parent's last
                // compound ("ul" + "li" -> "ulli",
                // ".a" + ".b" -> ".a.b", verified against
                // dart-sass 1.106.1).
                format!("{}{}", pp, p)
              })
              .collect();
            combos.join(", ")
          })
          .collect::<Vec<_>>()
          .join(", ")
      }
    });
  }
  Ok(selector_value(&current.unwrap_or_default()))
}

/// Whether a complex selector's last compound starts with a type selector
/// (element name rather than `.class` / `#id` / `:pseudo` / `[attr]` / `*`)
fn ends_with_type_selector(complex: &str) -> bool {
  complex
    .split_whitespace()
    .last()
    .map(|c| {
      !c.starts_with('.')
        && !c.starts_with('#')
        && !c.starts_with(':')
        && !c.starts_with('[')
        && !c.starts_with('*')
        && !c.starts_with('&')
    })
    .unwrap_or(false)
}

pub(crate) fn unify(args: Args) -> Result<Value> {
  const F: &str = "selector.unify";
  let a = selector_text(args.require(0, "selector1", F)?, F, "selector1")?;
  let b = selector_text(args.require(1, "selector2", F)?, F, "selector2")?;
  let mut out: Vec<String> = Vec::new();
  for pa in a.split(',') {
    for pb in b.split(',') {
      let pa = pa.trim();
      let pb = pb.trim();
      let ac: Vec<&str> = pa.split_whitespace().collect();
      let bc: Vec<&str> = pb.split_whitespace().collect();
      let merged: Option<Vec<String>> = match (ac.len(), bc.len()) {
        // Single compounds: intersect simple selectors, deduping.
        // Two distinct type selectors cannot unify (null).
        (1, 1) => unify_compounds(ac[0], bc[0]),
        // Shared compound on the a/b boundary: the reference emits
        // both interleavings (`.a .b` × `.c .b` -> `.a .c .b, .c .a .b`)
        _ if ac.last() == bc.first() && ac.last().is_some() => {
          let shared = ac.last().unwrap();
          let mut r1: Vec<String> = ac[..ac.len() - 1].iter().map(|s| s.to_string()).collect();
          r1.extend(bc.iter().map(|s| s.to_string()));
          let mut r2: Vec<String> = bc.iter().map(|s| s.to_string()).collect();
          r2.extend(ac[..ac.len() - 1].iter().map(|s| s.to_string()));
          let _ = shared;
          for r in [r1, r2] {
            let text = r.join(" ");
            if !out.contains(&text) {
              out.push(text);
            }
          }
          continue;
        }
        _ => None,
      };
      match merged {
        Some(m) => {
          // Simple selectors of one compound join with no separator
          let text = m.concat();
          if !text.is_empty() && !out.contains(&text) {
            out.push(text);
          }
        }
        None => {
          if ac.len() == 1 && bc.len() == 1 {
            // Compound unification failed (e.g. two type
            // selectors) -> the pair contributes nothing
            continue;
          }
          let text = format!("{}{}", pa, pb);
          if !out.contains(&text) {
            out.push(text);
          }
        }
      }
    }
  }
  if out.is_empty() {
    return Ok(Value::Null);
  }
  Ok(selector_value(&out.join(", ")))
}

/// Unify two compound selectors (`.a.b` × `.b.c` -> `.a.b.c`), returning
/// `None` when they cannot share a compound (two distinct type selectors).
fn unify_compounds(a: &str, b: &str) -> Option<Vec<String>> {
  let sa = split_simple_selectors(a);
  let sb = split_simple_selectors(b);
  let type_a = sa.iter().find(|s| is_type_selector(s));
  let type_b = sb.iter().find(|s| is_type_selector(s));
  if let (Some(ta), Some(tb)) = (type_a, type_b)
    && !ta.eq_ignore_ascii_case(tb)
  {
    return None;
  }
  let mut out: Vec<String> = sa.clone();
  for s in sb {
    if !out.contains(&s) {
      out.push(s);
    }
  }
  Some(out)
}

/// Split a compound into simple selectors (respecting brackets/parens)
fn split_simple_selectors(compound: &str) -> Vec<String> {
  let mut parts = Vec::new();
  let mut cur = String::new();
  let mut depth = 0i32;
  for ch in compound.chars() {
    match ch {
      '(' | '[' => {
        depth += 1;
        cur.push(ch);
      }
      ')' | ']' => {
        depth -= 1;
        cur.push(ch);
      }
      '.' | '#' | ':' | '%' if depth == 0 && !cur.is_empty() => {
        parts.push(cur.clone());
        cur = ch.to_string();
      }
      _ => cur.push(ch),
    }
  }
  if !cur.is_empty() {
    parts.push(cur);
  }
  parts
}

/// Whether a simple selector is a type (element) selector
fn is_type_selector(simple: &str) -> bool {
  let first = simple.chars().next().unwrap_or('*');
  !matches!(first, '.' | '#' | ':' | '%' | '*' | '@' | '&')
    && simple
      .chars()
      .next()
      .map(|c| c.is_alphanumeric() || c == '_' || c == '-')
      .unwrap_or(false)
}

pub(crate) fn superselector(args: Args) -> Result<Value> {
  const F: &str = "selector.is-superselector";
  let a = selector_text(args.require(0, "super", F)?, F, "super")?;
  let b = selector_text(args.require(1, "sub", F)?, F, "sub")?;
  // Pragmatic: every sub complex must contain the super's last compound
  // as a whitespace-bounded substring
  let is_super = b.split(',').all(|sub| {
    sub
      .split_whitespace()
      .any(|compound| compound.contains(a.trim()))
  }) || b.contains(&a);
  Ok(Value::Bool(is_super))
}

pub(crate) fn parse(args: Args) -> Result<Value> {
  const F: &str = "selector.parse";
  let s = selector_text(args.require(0, "selector", F)?, F, "selector")?;
  // Reference behaviour (dart-sass 1.106/1.107, probed): `selector.parse`
  // returns a comma list whose elements are each complex selector's source
  // text (whitespace-normalised). It is structurally permissive — leading,
  // trailing and repeated combinators are all accepted (`a >`, `>`) — and
  // rejects only (a) any parent selector `&` ("Parent selectors aren't
  // allowed here") and (b) lexical garbage like `..` / `:` (no identifier).
  for complex in s.split(',') {
    let mut any = false;
    for tok in complex.split_whitespace() {
      any = true;
      if tok.contains('&') {
        return Err(Error::type_error(format!(
          "{}: $selector: Parent selectors aren't allowed here.",
          F
        )));
      }
      if tok.chars().all(|c| c == '.' || c == ':') {
        return Err(Error::type_error(format!(
          "{}: $selector: \"{}\" is not a valid CSS selector.",
          F, s
        )));
      }
    }
    if !any {
      // An empty complex (`""`, `"a, , b"`) has no selector at all.
      return Err(Error::type_error(format!(
        "{}: $selector: \"{}\" is not a valid CSS selector.",
        F, s
      )));
    }
  }
  let items: Vec<Value> = s
    .split(',')
    .map(|complex| {
      str_value(
        complex.split_whitespace().collect::<Vec<_>>().join(" "),
        false,
      )
    })
    .collect();
  Ok(Value::List(ListValue::new(items, Separator::Comma)))
}

pub(crate) fn replace(args: Args) -> Result<Value> {
  const F: &str = "selector.replace";
  let s = selector_text(args.require(0, "selector", F)?, F, "selector")?;
  let orig = selector_text(args.require(1, "original", F)?, F, "original")?;
  let new = selector_text(args.require(2, "replacement", F)?, F, "replacement")?;
  Ok(selector_value(&s.replace(&orig, &new)))
}

/// `selector.extend($selector, $extendee, $extender)` — @extend semantics:
/// the original selector is always kept; for each complex containing the
/// extendee, copies are added with each occurrence substituted, including
/// cross products when the extendee appears multiple times
/// (`.a .b > .b` → `.a .b > .b, .a .x > .b, .a .b > .x, .a .x > .x`).
pub(crate) fn extend(args: Args) -> Result<Value> {
  const F: &str = "selector.extend";
  let s = selector_text(args.require(0, "selector", F)?, F, "selector")?;
  let extendee = selector_text(args.require(1, "extendee", F)?, F, "extendee")?;
  let extender = selector_text(args.require(2, "extender", F)?, F, "extender")?;
  let mut out: Vec<String> = Vec::new();
  for complex in s.split(',').map(str::trim) {
    out.push(complex.to_string());
    for ext in extender.split(',').map(str::trim) {
      for variant in extend_complex(complex, &extendee, ext) {
        if !out.contains(&variant) {
          out.push(variant);
        }
      }
    }
  }
  // Deduplicate while preserving order (@extend output is a set)
  let mut seen: Vec<String> = Vec::new();
  out.retain(|c| {
    if seen.contains(c) {
      false
    } else {
      seen.push(c.clone());
      true
    }
  });
  Ok(selector_value(&out.join(", ")))
}

/// All extension variants of one complex selector for a single extender
/// (cross product over every compound containing the extendee).
fn extend_complex(complex: &str, extendee: &str, extender: &str) -> Vec<String> {
  let parts: Vec<&str> = complex.split_whitespace().collect();
  let mut cur: Vec<String> = Vec::new();
  let mut out: Vec<String> = Vec::new();
  extend_go(&parts, 0, 0, extendee, extender, &mut cur, &mut out);
  out
}

/// Recursive cross-product walker; `subs` counts substitutions so far.
fn extend_go(
  parts: &[&str],
  idx: usize,
  subs: usize,
  extendee: &str,
  extender: &str,
  cur: &mut Vec<String>,
  out: &mut Vec<String>,
) {
  if idx == parts.len() {
    if subs > 0 {
      out.push(cur.join(" "));
    }
    return;
  }
  let part = parts[idx];
  if part == ">" || part == "+" || part == "~" {
    cur.push(part.to_string());
    extend_go(parts, idx + 1, subs, extendee, extender, cur, out);
    cur.pop();
    return;
  }
  // Option 1: keep the compound as-is
  cur.push(part.to_string());
  extend_go(parts, idx + 1, subs, extendee, extender, cur, out);
  cur.pop();
  // Option 2: substitute the extendee within this compound
  if let Some(sub) = substitute_compound(part, extendee, extender) {
    cur.push(sub);
    extend_go(parts, idx + 1, subs + 1, extendee, extender, cur, out);
    cur.pop();
  }
}

/// Replace the extendee inside one compound; `None` when it does not appear.
fn substitute_compound(compound: &str, extendee: &str, extender: &str) -> Option<String> {
  if compound == extendee {
    return Some(extender.to_string());
  }
  if extendee.starts_with('.')
    && extender.starts_with('.')
    && class_token_contains(compound, extendee)
  {
    let stripped = strip_class_token(compound, extendee);
    if stripped.is_empty() {
      return Some(extender.to_string());
    }
    // `.a.b` extending `.b` with `.c` → `.a.c`
    return Some(format!("{}{}", stripped, extender));
  }
  None
}

/// Whether compound `compound` contains `class` (e.g. ".b") as a whole
/// class token (so `.a.b` and `a.b` match `.b`, but `.bbc` does not match
/// `.b`). A leading `.` always starts a new token, so only the right
/// boundary needs checking.
fn class_token_contains(compound: &str, class: &str) -> bool {
  compound.match_indices(class).any(|(i, _)| {
    !compound[i + class.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '-' || c == '_')
  })
}

/// Remove the first whole occurrence of `class` (e.g. ".b") from a compound
/// (e.g. ".a.b" → ".a", ".b.c" → ".c", "a.b" → "a"), respecting class-token
/// boundaries. Returns the compound unchanged when there is no whole match.
fn strip_class_token(compound: &str, class: &str) -> String {
  for (i, _) in compound.match_indices(class) {
    if !compound[i + class.len()..]
      .starts_with(|c: char| c.is_alphanumeric() || c == '-' || c == '_')
    {
      let mut s = String::with_capacity(compound.len() - class.len());
      s.push_str(&compound[..i]);
      s.push_str(&compound[i + class.len()..]);
      return s;
    }
  }
  compound.to_string()
}
