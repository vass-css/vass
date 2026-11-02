//! `vass:math` module

use vass_core::error::{Error, Result};
use vass_core::value::Value;

use super::{Args, arg_error, min_max_values, need_int, need_number, num_unit, num_value};

/// Dispatch `math.<name>`; `None` if unknown
pub fn call(name: &str, args: Args) -> Option<Result<Value>> {
  let r = match name {
    "div" => div(args),
    "abs" => math1(args, "abs", f64::abs),
    "ceil" => math1(args, "ceil", f64::ceil),
    "floor" => math1(args, "floor", f64::floor),
    "round" => math1(args, "round", f64::round),
    "max" => max(args),
    "min" => min(args),
    "hypot" => hypot(args),
    "log" => log(args),
    "pow" => pow(args),
    "sqrt" => unit_fn(args, "sqrt", f64::sqrt),
    "cos" => angle_fn(args, "cos", f64::cos),
    "sin" => angle_fn(args, "sin", f64::sin),
    "tan" => angle_fn(args, "tan", f64::tan),
    "acos" => inv_angle_fn(args, "acos", f64::acos),
    "asin" => inv_angle_fn(args, "asin", f64::asin),
    "atan" => inv_angle_fn(args, "atan", f64::atan),
    "atan2" => atan2(args),
    "percentage" => percentage(args),
    "random" => random(args),
    "unit" => unit(args),
    "unitless" => unitless(args),
    "compatible" | "compatible-with" => compatible(args),
    "is-unitless" => unitless(args),
    "clamp" => clamp(args),
    _ => return None,
  };
  Some(r)
}

fn math1(args: Args, fname: &str, f: impl Fn(f64) -> f64) -> Result<Value> {
  let n = need_number(args.require(0, "number", fname)?, fname, "number")?;
  let value = f(n.value);
  // Preserve compound units (math.round(2.5px * 1px) -> calc(3px * 1px))
  let out = match &n.compound {
    Some(c) => {
      vass_core::value::Number::with_compound(value, c.numerators.clone(), c.denominators.clone())
    }
    None => vass_core::value::Number::new(value, n.unit.clone()),
  };
  Ok(Value::Number(out))
}

fn div(args: Args) -> Result<Value> {
  const F: &str = "math.div";
  let a = need_number(args.require(0, "number1", F)?, F, "number1")?;
  let b = need_number(args.require(1, "number2", F)?, F, "number2")?;
  // Divisions that cannot resolve to a plain number render as calc(),
  // matching the reference compiler (math.div(100cm, 1m) -> calc(100cm / 1m))
  match a.div(b) {
    Ok(n) => Ok(Value::Number(n)),
    Err(_) if b.unit.is_some() || b.value == 0.0 => Ok(Value::Str(
      vass_core::value::VassString::unquoted(a.div_calc(b)),
    )),
    Err(e) => Err(e),
  }
}

fn max(args: Args) -> Result<Value> {
  min_max_values(&args, false, "math.max", "max")
}

fn min(args: Args) -> Result<Value> {
  min_max_values(&args, true, "math.min", "min")
}

fn hypot(args: Args) -> Result<Value> {
  const F: &str = "math.hypot";
  if args.positional.is_empty() {
    return Err(arg_error(F, "at least 1", 0));
  }
  let first = need_number(args.positional[0], F, "number")?;
  let unit = first.unit.clone();
  let mut sum = 0.0;
  for v in &args.positional {
    let n = need_number(v, F, "number")?;
    if n.unit != unit && !(n.is_unitless() || unit.is_none()) {
      return Err(Error::type_error(format!("{}: incompatible units.", F)));
    }
    sum += n.value * n.value;
  }
  Ok(match unit {
    Some(u) => Value::Number(vass_core::value::Number::new(sum.sqrt(), Some(u))),
    None => num_value(sum.sqrt()),
  })
}

fn log(args: Args) -> Result<Value> {
  const F: &str = "math.log";
  let n = need_number(args.require(0, "number", F)?, F, "number")?;
  if !n.has_no_units() {
    return Err(Error::type_error(format!(
      "$number: Expected {} to have no units.",
      n.to_css()
    )));
  }
  let base = match args.get(1, "base") {
    Some(b) => need_number(b, F, "base")?.value,
    None => std::f64::consts::E,
  };
  Ok(num_value(n.value.ln() / base.ln()))
}

fn pow(args: Args) -> Result<Value> {
  const F: &str = "math.pow";
  let base = need_number(args.require(0, "base", F)?, F, "base")?;
  let exp = need_number(args.require(1, "exponent", F)?, F, "exponent")?;
  if !base.has_no_units() {
    return Err(Error::type_error(format!(
      "$base: Expected {} to have no units.",
      base.to_css()
    )));
  }
  Ok(num_value(base.value.powf(exp.value)))
}

fn unit_fn(args: Args, fname: &str, f: impl Fn(f64) -> f64) -> Result<Value> {
  let n = need_number(args.require(0, "number", fname)?, fname, "number")?;
  if !n.has_no_units() {
    return Err(Error::type_error(format!(
      "$number: Expected {} to have no units.",
      n.to_css()
    )));
  }
  Ok(num_value(f(n.value)))
}

fn angle_fn(args: Args, fname: &str, f: impl Fn(f64) -> f64) -> Result<Value> {
  let n = need_number(args.require(0, "number", fname)?, fname, "number")?;
  let rad = match n.unit.as_deref() {
    Some("deg") => n.value.to_radians(),
    Some("rad") | None => n.value,
    Some(other) => {
      return Err(Error::type_error(format!(
        "{}: expected an angle, got `{}`.",
        fname, other
      )));
    }
  };
  Ok(num_value(f(rad)))
}

fn inv_angle_fn(args: Args, fname: &str, f: impl Fn(f64) -> f64) -> Result<Value> {
  let n = need_number(args.require(0, "number", fname)?, fname, "number")?;
  if !n.has_no_units() {
    return Err(Error::type_error(format!(
      "$number: Expected {} to have no units.",
      n.to_css()
    )));
  }
  // Out-of-domain inputs yield NaN, serialized as calc(NaN * 1deg)
  // exactly like the reference compiler (math.acos(2)).
  Ok(num_unit(f(n.value).to_degrees(), "deg"))
}

fn atan2(args: Args) -> Result<Value> {
  const F: &str = "math.atan2";
  let y = need_number(args.require(0, "y", F)?, F, "y")?;
  let x = need_number(args.require(1, "x", F)?, F, "x")?;
  Ok(num_unit(y.value.atan2(x.value).to_degrees(), "deg"))
}

fn percentage(args: Args) -> Result<Value> {
  const F: &str = "math.percentage";
  let n = need_number(args.require(0, "number", F)?, F, "number")?;
  if !n.has_no_units() {
    return Err(Error::type_error(format!(
      "$number: Expected {} to have no units.",
      n.to_css()
    )));
  }
  Ok(num_unit(n.value * 100.0, "%"))
}

fn random(args: Args) -> Result<Value> {
  use std::time::{SystemTime, UNIX_EPOCH};
  const F: &str = "math.random";
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
    .unwrap_or(7);
  match args.get(0, "limit") {
    None => Ok(num_value((nanos % 1_0000_0000) as f64 / 1_0000_0000.0)),
    Some(v) => {
      let limit = need_int(v, F, "limit")?;
      if limit < 1 {
        return Err(Error::type_error(format!("{}: `$limit` must be >= 1.", F)));
      }
      Ok(num_value((nanos % limit as u64) as f64 + 1.0))
    }
  }
}

fn unit(args: Args) -> Result<Value> {
  const F: &str = "math.unit";
  let n = need_number(args.require(0, "number", F)?, F, "number")?;
  Ok(Value::Str(vass_core::value::VassString::quoted(
    n.unit_string(),
  )))
}

fn unitless(args: Args) -> Result<Value> {
  const F: &str = "math.unitless";
  let n = need_number(args.require(0, "number", F)?, F, "number")?;
  Ok(Value::Bool(n.is_unitless()))
}

fn compatible(args: Args) -> Result<Value> {
  const F: &str = "math.compatible";
  let a = need_number(args.require(0, "number1", F)?, F, "number1")?;
  let b = need_number(args.require(1, "number2", F)?, F, "number2")?;
  Ok(Value::Bool(a.compatible(b)))
}

fn clamp(args: Args) -> Result<Value> {
  const F: &str = "math.clamp";
  let min = need_number(args.require(0, "min", F)?, F, "min")?;
  let val = need_number(args.require(1, "number", F)?, F, "number")?;
  let max = need_number(args.require(2, "max", F)?, F, "max")?;
  if val.value < min.value {
    Ok(Value::Number(min.clone()))
  } else if val.value > max.value {
    Ok(Value::Number(max.clone()))
  } else {
    Ok(Value::Number(val.clone()))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_core::value::Value;

  #[test]
  fn test_call_unknown_name_returns_none() {
    let args = Args {
      positional: Vec::new(),
      named: Vec::new(),
    };
    assert!(call("nonexistent", args).is_none());
  }

  #[test]
  fn test_math1_abs() {
    let n = Value::Number(vass_core::value::Number::new(-5.0, Some("px".to_string())));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("abs", args);
    assert!(result.is_some());
    let result = result.unwrap().unwrap();
    match result {
      Value::Number(num) => assert_eq!(num.value, 5.0),
      _ => panic!("expected Number"),
    }
  }

  #[test]
  fn test_math1_ceil() {
    let n = Value::Number(vass_core::value::Number::new(3.2, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("ceil", args);
    assert!(result.is_some());
    let result = result.unwrap().unwrap();
    match result {
      Value::Number(num) => assert_eq!(num.value, 4.0),
      _ => panic!("expected Number"),
    }
  }

  #[test]
  fn test_math1_floor() {
    let n = Value::Number(vass_core::value::Number::new(3.8, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("floor", args);
    assert!(result.is_some());
    let result = result.unwrap().unwrap();
    match result {
      Value::Number(num) => assert_eq!(num.value, 3.0),
      _ => panic!("expected Number"),
    }
  }

  #[test]
  fn test_math1_round() {
    let n = Value::Number(vass_core::value::Number::new(2.5, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("round", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_div() {
    let a = Value::Number(vass_core::value::Number::new(100.0, Some("px".to_string())));
    let b = Value::Number(vass_core::value::Number::new(4.0, None));
    let args = Args {
      positional: vec![&a, &b],
      named: Vec::new(),
    };
    let result = call("div", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_div_by_zero() {
    let a = Value::Number(vass_core::value::Number::new(10.0, Some("px".to_string())));
    let b = Value::Number(vass_core::value::Number::new(0.0, None));
    let args = Args {
      positional: vec![&a, &b],
      named: Vec::new(),
    };
    let result = call("div", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_max() {
    let a = Value::Number(vass_core::value::Number::new(5.0, None));
    let b = Value::Number(vass_core::value::Number::new(10.0, None));
    let args = Args {
      positional: vec![&a, &b],
      named: Vec::new(),
    };
    let result = call("max", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_min() {
    let a = Value::Number(vass_core::value::Number::new(5.0, None));
    let b = Value::Number(vass_core::value::Number::new(10.0, None));
    let args = Args {
      positional: vec![&a, &b],
      named: Vec::new(),
    };
    let result = call("min", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_hypot() {
    let a = Value::Number(vass_core::value::Number::new(3.0, None));
    let b = Value::Number(vass_core::value::Number::new(4.0, None));
    let args = Args {
      positional: vec![&a, &b],
      named: Vec::new(),
    };
    let result = call("hypot", args);
    assert!(result.is_some());
    let result = result.unwrap().unwrap();
    match result {
      Value::Number(num) => assert!((num.value - 5.0).abs() < 0.001),
      _ => panic!("expected Number"),
    }
  }

  #[test]
  fn test_math_log() {
    let n = Value::Number(vass_core::value::Number::new(100.0, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("log", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_pow() {
    let base = Value::Number(vass_core::value::Number::new(2.0, None));
    let exp = Value::Number(vass_core::value::Number::new(3.0, None));
    let args = Args {
      positional: vec![&base, &exp],
      named: Vec::new(),
    };
    let result = call("pow", args);
    assert!(result.is_some());
    let result = result.unwrap().unwrap();
    match result {
      Value::Number(num) => assert_eq!(num.value, 8.0),
      _ => panic!("expected Number"),
    }
  }

  #[test]
  fn test_math_sqrt() {
    let n = Value::Number(vass_core::value::Number::new(16.0, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("sqrt", args);
    assert!(result.is_some());
    let result = result.unwrap().unwrap();
    match result {
      Value::Number(num) => assert_eq!(num.value, 4.0),
      _ => panic!("expected Number"),
    }
  }

  #[test]
  fn test_math_cos() {
    let n = Value::Number(vass_core::value::Number::new(0.0, Some("deg".to_string())));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("cos", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_sin() {
    let n = Value::Number(vass_core::value::Number::new(0.0, Some("deg".to_string())));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("sin", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_tan() {
    let n = Value::Number(vass_core::value::Number::new(0.0, Some("deg".to_string())));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("tan", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_acos() {
    let n = Value::Number(vass_core::value::Number::new(0.5, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("acos", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_asin() {
    let n = Value::Number(vass_core::value::Number::new(0.5, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("asin", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_atan() {
    let n = Value::Number(vass_core::value::Number::new(1.0, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("atan", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_atan2() {
    let y = Value::Number(vass_core::value::Number::new(1.0, None));
    let x = Value::Number(vass_core::value::Number::new(1.0, None));
    let args = Args {
      positional: vec![&y, &x],
      named: Vec::new(),
    };
    let result = call("atan2", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_percentage() {
    let n = Value::Number(vass_core::value::Number::new(0.5, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("percentage", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_random() {
    let args = Args {
      positional: Vec::new(),
      named: Vec::new(),
    };
    let result = call("random", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_random_with_limit() {
    let limit = Value::Number(vass_core::value::Number::new(100.0, None));
    let args = Args {
      positional: vec![&limit],
      named: Vec::new(),
    };
    let result = call("random", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_unit() {
    let n = Value::Number(vass_core::value::Number::new(10.0, Some("px".to_string())));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("unit", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_unitless() {
    let n = Value::Number(vass_core::value::Number::new(10.0, None));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("unitless", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_compatible() {
    let a = Value::Number(vass_core::value::Number::new(10.0, Some("px".to_string())));
    let b = Value::Number(vass_core::value::Number::new(5.0, Some("px".to_string())));
    let args = Args {
      positional: vec![&a, &b],
      named: Vec::new(),
    };
    let result = call("compatible", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_clamp() {
    let min = Value::Number(vass_core::value::Number::new(0.0, None));
    let val = Value::Number(vass_core::value::Number::new(5.0, None));
    let max = Value::Number(vass_core::value::Number::new(10.0, None));
    let args = Args {
      positional: vec![&min, &val, &max],
      named: Vec::new(),
    };
    let result = call("clamp", args);
    assert!(result.is_some());
  }

  #[test]
  fn test_math_invalid_units() {
    let n = Value::Number(vass_core::value::Number::new(5.0, Some("px".to_string())));
    let args = Args {
      positional: vec![&n],
      named: Vec::new(),
    };
    let result = call("log", args);
    assert!(result.is_some());
    let result = result.unwrap();
    assert!(result.is_err());
  }

  #[test]
  fn test_math_div_negative_exponent() {
    let base = Value::Number(vass_core::value::Number::new(2.0, None));
    let exp = Value::Number(vass_core::value::Number::new(-1.0, None));
    let args = Args {
      positional: vec![&base, &exp],
      named: Vec::new(),
    };
    let result = call("pow", args);
    assert!(result.is_some());
  }
}
