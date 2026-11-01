//! Number values with optional units and absolute-unit conversion

use std::cmp::Ordering;
use std::fmt;

use crate::error::Error;

/// Conversion factors to the canonical unit of each dimension
/// (length → cm, time → s, angle → deg, frequency → Hz, resolution → dpi)
fn unit_factor(unit: &str) -> Option<f64> {
  Some(match unit {
    // Absolute lengths → cm
    "cm" => 1.0,
    "mm" => 0.1,
    "q" => 0.025,
    "in" => 2.54,
    "pt" => 2.54 / 72.0,
    "pc" => 2.54 / 6.0,
    "px" => 2.54 / 96.0,
    // Time → s
    "s" => 1.0,
    "ms" => 0.001,
    // Angle → deg
    "deg" => 1.0,
    "grad" => 0.9,
    "rad" => 180.0 / std::f64::consts::PI,
    "turn" => 360.0,
    // Frequency → Hz
    "hz" => 1.0,
    "khz" => 1000.0,
    // Resolution → dpi
    "dpi" => 1.0,
    "dpcm" => 2.54,
    "dppx" | "x" => 96.0,
    _ => return None,
  })
}

/// Which canonical dimension a unit belongs to
fn unit_dimension(unit: &str) -> Option<&'static str> {
  Some(match unit {
    "cm" | "mm" | "q" | "in" | "pt" | "pc" | "px" => "length",
    "s" | "ms" => "time",
    "deg" | "grad" | "rad" | "turn" => "angle",
    "hz" | "khz" => "frequency",
    "dpi" | "dpcm" | "dppx" | "x" => "resolution",
    "%" => "percent",
    _ => return None,
  })
}

/// Compound units of a number: numerator × denominator factors.
///
/// The reference implementation represents `5px * 10px` as the number `50`
/// carrying the compound unit `px*px`; the `calc(...)` notation is purely a
/// serialization concern. Compound numbers behave as first-class numbers:
/// they compare, round, cancel through division and feed `math.unit()`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Compound {
  /// Multiplicative factors (`px`, `s`, ...)
  pub numerators: Vec<String>,
  /// Divisor factors (`px / 1s` carries `["s"]` here)
  pub denominators: Vec<String>,
}

impl Compound {
  /// Canonical unit string: `px*px`, `px/s`, `px/(s*s)`, `px*px/s`
  pub fn unit_string(&self) -> String {
    let num = self.numerators.join("*");
    match self.denominators.len() {
      0 => num,
      // Numerator-less quotients use the negative-exponent notation
      // (math.unit(math.div(1, 1s)) -> "s^-1"), matching the reference
      1 if num.is_empty() => format!("{}^-1", self.denominators[0]),
      1 => format!("{}/{}", num, self.denominators[0]),
      _ if num.is_empty() => format!("({})^-1", self.denominators.join("*")),
      _ => format!("{}/({})", num, self.denominators.join("*")),
    }
  }
}

/// A VASS number: an f64 with an optional unit (`10px`, `50%`, `1.5rem`)
/// and, for products/quotients of differing units, a compound unit.
#[derive(Debug, Clone, Default)]
pub struct Number {
  /// Numeric value
  pub value: f64,
  /// Unit (`px`, `em`, `%`, ...), or `None` if unitless or compound
  pub unit: Option<String>,
  /// Compound unit, present only when the unit has multiple factors
  pub compound: Option<Compound>,
}

impl Number {
  /// Create a new number with an optional unit
  pub fn new(value: f64, unit: Option<String>) -> Self {
    Self {
      value,
      unit,
      compound: None,
    }
  }

  /// Create a unitless number
  pub fn unitless(value: f64) -> Self {
    Self {
      value,
      unit: None,
      compound: None,
    }
  }

  /// Create a number with a unit
  pub fn with_unit(value: f64, unit: impl Into<String>) -> Self {
    Self {
      value,
      unit: Some(unit.into()),
      compound: None,
    }
  }

  /// Create a number with a compound unit, cancelling matching
  /// numerator/denominator factors (`1px*1px / 1px` keeps one `px`).
  ///
  /// Cancellation mirrors the reference compiler: identical factors always
  /// cancel; convertible factors (px/mm, ...) cancel by scale factor except
  /// when the meter is involved, which the reference keeps symbolic.
  pub fn with_compound(
    value: f64,
    mut numerators: Vec<String>,
    mut denominators: Vec<String>,
  ) -> Self {
    let mut scale = 1.0f64;
    // Cancel exact matches, then convertible matches (same dimension,
    // known factor), keeping the numerator's unit and scaling the value.
    // The meter is exempt: the reference keeps `calc(2mm / 1m)` symbolic.
    let meter = |u: &str| u == "m";
    let mut kept: Vec<String> = Vec::new();
    while let Some(n) = numerators.pop() {
      let exact = denominators.iter().position(|d| d == &n);
      let conv = if exact.is_none() && !meter(&n) {
        denominators.iter().position(|d| {
          !meter(d)
            && unit_dimension(&n).is_some()
            && unit_dimension(&n) == unit_dimension(d)
            && unit_factor(&n).is_some()
        })
      } else {
        None
      };
      if let Some(pos) = exact.or(conv) {
        let d = denominators.remove(pos);
        if exact.is_none() {
          // The value is expressed in numerator units; cancelling
          // the denominator factor rewrites it in that factor's
          // units: value_d = value_n * factor(n) / factor(d).
          scale *= unit_factor(&n).unwrap_or(1.0) / unit_factor(&d).unwrap_or(1.0);
        }
      } else {
        kept.push(n);
      }
    }
    let mut numerators = kept;
    numerators.reverse();
    let value = value * scale;
    // A single numerator and no denominators collapses to a plain number.
    if numerators.len() == 1 && denominators.is_empty() {
      return Number::with_unit(value, numerators.pop().unwrap());
    }
    if numerators.is_empty() && denominators.is_empty() {
      return Number::unitless(value);
    }
    Self {
      value,
      unit: None,
      compound: Some(Compound {
        numerators,
        denominators,
      }),
    }
  }

  /// Whether this number carries a compound unit
  pub fn is_compound(&self) -> bool {
    self.compound.is_some()
  }

  /// Whether the number has no units at all (neither single nor compound)
  pub fn has_no_units(&self) -> bool {
    self.unit.is_none() && self.compound.is_none()
  }

  /// The unit string for `math.unit()`: `px`, `px*px`, `px/s`, `px/(s*s)`
  pub fn unit_string(&self) -> String {
    if let Some(c) = &self.compound {
      return c.unit_string();
    }
    self.unit.clone().unwrap_or_default()
  }

  /// Canonical key used for compatibility checks: `Some("px")`,
  /// `Some("px*px/s")`, `None` for unitless
  pub fn unit_key(&self) -> Option<String> {
    if let Some(c) = &self.compound {
      if c.numerators.is_empty() && c.denominators.is_empty() {
        return None;
      }
      return Some(c.unit_string());
    }
    self.unit.clone()
  }

  fn unit_name(&self) -> &str {
    self.unit.as_deref().unwrap_or("")
  }

  /// Whether the number has no unit (including compound units)
  pub fn is_unitless(&self) -> bool {
    self.has_no_units()
  }

  /// Convert this number's value into `unit`'s scale (same dimension).
  /// Only absolute (convertible) units can be converted.
  pub fn convert_to(&self, unit: &str) -> Option<Number> {
    let from = self.unit.as_deref()?;
    if from == unit {
      return Some(self.clone());
    }
    let dim = unit_dimension(from)?;
    if unit_dimension(unit) != Some(dim) || dim == "percent" {
      return None;
    }
    let f_from = unit_factor(from)?;
    let f_to = unit_factor(unit)?;
    Some(Number::with_unit(self.value * f_from / f_to, unit))
  }

  /// Whether two numbers may participate in `+`/`-`: single convertible
  /// units follow the existing rules; compound units require an identical
  /// unit string (reference behavior).
  pub fn compatible(&self, other: &Number) -> bool {
    match (&self.compound, &other.compound) {
      (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => {
        // Compound units are only compatible with an identical unit
        // string (the reference has no compound↔single conversion).
        self.unit_key().is_some() && self.unit_key() == other.unit_key()
      }
      (None, None) => {
        if self.unit == other.unit || self.unit.is_none() || other.unit.is_none() {
          return true;
        }
        // Convertible absolute units (px + cm, ms + s, ...)
        match (&self.unit, &other.unit) {
          (Some(a), Some(b)) => unit_dimension(a) == unit_dimension(b) && unit_factor(a).is_some(),
          _ => false,
        }
      }
    }
  }

  fn require_compatible(&self, other: &Number, op: char) -> Result<(), Error> {
    if self.compatible(other) {
      Ok(())
    } else if self.is_compound() || other.is_compound() {
      // Reference wording for compound mismatches (uses the calc form)
      Err(Error::type_error(format!(
        "{} and {} have incompatible units.",
        self.to_css(),
        other.to_css()
      )))
    } else {
      Err(Error::type_error(format!(
        "Incompatible units {}{} {} {}{}.",
        self.value,
        self.unit_name(),
        op,
        other.value,
        other.unit_name()
      )))
    }
  }

  /// The unit to carry on a result: `self`'s, unless it has none
  fn result_unit<'a>(&'a self, other: &'a Number) -> Option<String> {
    self.unit.clone().or_else(|| other.unit.clone())
  }

  /// Addition; units must be compatible (convertible units are converted)
  pub fn add(&self, other: &Number) -> Result<Number, Error> {
    self.require_compatible(other, '+')?;
    if let Some(c) = &self.compound {
      return Ok(Number::with_compound(
        self.value + other.value,
        c.numerators.clone(),
        c.denominators.clone(),
      ));
    }
    if let Some(c) = &other.compound {
      return Ok(Number::with_compound(
        self.value + other.value,
        c.numerators.clone(),
        c.denominators.clone(),
      ));
    }
    let unit = self.result_unit(other);
    let ov = self.coerce(other, &unit);
    Ok(Number::new(self.value + ov, unit))
  }

  /// Subtraction; units must be compatible (convertible units are converted)
  pub fn sub(&self, other: &Number) -> Result<Number, Error> {
    self.require_compatible(other, '-')?;
    if let Some(c) = &self.compound {
      return Ok(Number::with_compound(
        self.value - other.value,
        c.numerators.clone(),
        c.denominators.clone(),
      ));
    }
    if let Some(c) = &other.compound {
      return Ok(Number::with_compound(
        self.value - other.value,
        c.numerators.clone(),
        c.denominators.clone(),
      ));
    }
    let unit = self.result_unit(other);
    let ov = self.coerce(other, &unit);
    Ok(Number::new(self.value - ov, unit))
  }

  /// Coerce `other` into this unit's scale when both are convertible
  fn coerce(&self, other: &Number, unit: &Option<String>) -> f64 {
    if let (Some(u), Some(ou)) = (unit, &other.unit)
      && u != ou
      && let Some(conv) = other.convert_to(u)
    {
      return conv.value;
    }
    other.value
  }

  /// Multiplication. Numbers of differing units produce a compound-unit
  /// number (`5px * 10px` -> `50` with unit `px*px`, serialized as
  /// `calc(50px * 1px)`) exactly like the reference compiler.
  pub fn mul(&self, other: &Number) -> Result<Number, Error> {
    if self.has_no_units() {
      return Ok(Number::new(self.value * other.value, other.unit.clone()).with_compound_of(other));
    }
    if other.has_no_units() {
      return Ok(Number::new(self.value * other.value, self.unit.clone()).with_compound_of(self));
    }
    // Both sides carry units: concatenate factors.
    let (mut n, mut d) = self.factors();
    let (n2, d2) = other.factors();
    n.extend(n2);
    d.extend(d2);
    Ok(Number::with_compound(self.value * other.value, n, d))
  }

  /// Factors as (numerators, denominators); single units become one numerator
  fn factors(&self) -> (Vec<String>, Vec<String>) {
    match &self.compound {
      Some(c) => (c.numerators.clone(), c.denominators.clone()),
      None => (
        self.unit.clone().map(|u| vec![u]).unwrap_or_default(),
        Vec::new(),
      ),
    }
  }

  /// Internal helper: keep the compound unit of `src` when its value merged
  /// with a unitless multiplier
  fn with_compound_of(self, src: &Number) -> Number {
    match &src.compound {
      Some(c) => Number::with_compound(self.value, c.numerators.clone(), c.denominators.clone()),
      None => self,
    }
  }

  /// Division. Differing units cancel where possible (`math.div(10px, 2px)`
  /// -> `5`); irreducible quotients become compound-unit numbers
  /// (`math.div(1px, 1s)` -> `calc(1px / 1s)`), matching the reference.
  /// Division by zero yields the IEEE result (`calc(infinity)`,
  /// `calc(-infinity)`, `calc(NaN)`) exactly like the reference compiler.
  pub fn div(&self, other: &Number) -> Result<Number, Error> {
    // Build factor lists: self's numerators join other's denominators.
    let (mut n, mut d) = self.factors();
    let (n2, d2) = other.factors();
    d.extend(n2);
    n.extend(d2);
    Ok(Number::with_compound(self.value / other.value, n, d))
  }

  /// Division that cannot resolve to a plain number, rendered the way the
  /// reference compiler does: operands are normalized so the denominator is
  /// `1<unit>` (`math.div(10mm, 5m)` -> `calc(2mm / 1m)`), and division by
  /// zero yields `calc(infinity)` / `calc(infinity * 1<unit>)`.
  pub fn div_calc(&self, other: &Number) -> String {
    if other.value == 0.0 {
      return match &self.unit {
        Some(u) => format!("calc(infinity * 1{})", u),
        None => "calc(infinity)".to_string(),
      };
    }
    let numerator = Number::new(self.value / other.value, self.unit.clone());
    let denominator = match &other.unit {
      Some(u) => Number::with_unit(1.0, u.clone()),
      None => Number::unitless(1.0),
    };
    format!("calc({} / {})", numerator.to_css(), denominator.to_css())
  }

  /// Modulo (VASS `%` operator); units must be compatible
  pub fn modulo(&self, other: &Number) -> Result<Number, Error> {
    self.require_compatible(other, '%')?;
    if other.value == 0.0 {
      return Err(Error::type_error("Modulo by zero."));
    }
    if let Some(c) = &self.compound {
      return Ok(Number::with_compound(
        self.value - other.value * (self.value / other.value).floor(),
        c.numerators.clone(),
        c.denominators.clone(),
      ));
    }
    let unit = self.result_unit(other);
    let ov = self.coerce(other, &unit);
    Ok(Number::new(
      self.value - ov * (self.value / ov).floor(),
      unit,
    ))
  }

  /// Negate the value, keeping the unit
  pub fn negate(&self) -> Number {
    match &self.compound {
      Some(c) => Number::with_compound(-self.value, c.numerators.clone(), c.denominators.clone()),
      None => Number::new(-self.value, self.unit.clone()),
    }
  }

  /// Whether this number equals zero
  pub fn is_zero(&self) -> bool {
    self.value == 0.0
  }
}

impl PartialEq for Number {
  fn eq(&self, other: &Self) -> bool {
    // Compound numbers compare by unit string + value.
    if self.is_compound() || other.is_compound() {
      return self.unit_key() == other.unit_key() && self.value == other.value;
    }
    if self.unit == other.unit {
      return self.value == other.value;
    }
    // Convertible units compare by value in the left's scale
    if let (Some(a), Some(_b)) = (&self.unit, &other.unit)
      && let Some(conv) = other.convert_to(a)
    {
      return self.value == conv.value;
    }
    false
  }
}

impl PartialOrd for Number {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    // Compound numbers only order against an identical unit string.
    if self.is_compound() || other.is_compound() {
      if self.unit_key() == other.unit_key() {
        return self.value.partial_cmp(&other.value);
      }
      return None;
    }
    if self.unit == other.unit {
      return self.value.partial_cmp(&other.value);
    }
    if let (Some(a), Some(_b)) = (&self.unit, &other.unit)
      && let Some(conv) = other.convert_to(a)
    {
      return self.value.partial_cmp(&conv.value);
    }
    // Unitless compares with anything numerically
    self.value.partial_cmp(&other.value)
  }
}

impl fmt::Display for Number {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.to_css())
  }
}

/// Format a float like the VASS default precision (10 significant digits),
/// stripping float noise and trailing zeros.
/// Negative zero is preserved (`-0px`), matching the reference compiler.
pub fn format_f64(v: f64) -> String {
  if v == 0.0 && v.is_sign_negative() {
    return "-0".to_string();
  }
  if v == v.trunc() && v.abs() < 1e15 {
    return format!("{}", v as i64);
  }
  if v.abs() >= 1e15 {
    // Beyond the exact-integer path: dart-sass prints the shortest
    // round-trip digits in positional notation (JS `Number.toString`
    // with the exponent removed). Rust's `Display` for f64 produces
    // exactly that — never in scientific notation.
    return format!("{}", v);
  }
  // dart-sass precision: round to 10 digits AFTER the decimal point,
  // then strip trailing zeros (1.0000000001 stays; 0.501960784313... →
  // 0.5019607843)
  let s = format!("{:.10}", v);
  let s = s.trim_end_matches('0').trim_end_matches('.');
  if s.is_empty() || s == "-" {
    return "0".to_string();
  }
  s.to_string()
}

/// Count significant digits in a decimal string representation
fn significant_digits(s: &str) -> usize {
  let body = s.trim_start_matches('-').replace('.', "");
  let stripped = body.trim_start_matches('0');
  if stripped.is_empty() {
    // 0.000xyz — count after leading zeros
    let after = body.trim_start_matches('0');
    return after.len();
  }
  stripped.len()
}

/// Compressed variant: `0.5` → `.5` (dart-sass compressed output)
pub fn format_f64_compressed(v: f64) -> String {
  let s = format_f64(v);
  if let Some(stripped) = s.strip_prefix("0.") {
    format!(".{}", stripped)
  } else if let Some(stripped) = s.strip_prefix("-0.") {
    format!("-.{}", stripped)
  } else {
    s
  }
}

impl Number {
  /// Render like the reference compiler's `@debug` output: numbers keep
  /// full f64 precision (`@debug math.sqrt(2)` -> `1.4142135623730951`),
  /// except values within the reference's epsilon (1e-11) of an integer,
  /// which are fuzzy-rounded (`tan(45deg)` -> `1`, `$epsilon` -> `0`).
  pub fn to_debug_string(&self) -> String {
    if !self.value.is_finite() {
      return self.to_css();
    }
    let rounded = self.value.round();
    let fuzzy_int = (self.value - rounded).abs() < 1e-11 && rounded.abs() < 1e15;
    if let Some(c) = &self.compound {
      let n = if fuzzy_int {
        Number {
          value: rounded,
          unit: None,
          compound: Some(c.clone()),
        }
      } else {
        self.clone()
      };
      return n.to_css();
    }
    let value = if fuzzy_int { rounded } else { self.value };
    match &self.unit {
      Some(u) => format!("{}{}", value, u),
      None => format!("{}", value),
    }
  }

  /// Render the number (with unit) as CSS text.
  ///
  /// Compound units serialize in the reference compiler's calc notation:
  /// the coefficient rides on the first numerator factor, every other
  /// factor is written as `1<unit>` (`50` with unit `px*px` becomes
  /// `calc(50px * 1px)`). Non-finite values serialize as
  /// `calc(NaN)` / `calc(infinity * 1px)`.
  pub fn to_css(&self) -> String {
    if !self.value.is_finite() {
      // NaN / ±infinity: the reference wraps these in calc()
      let word = if self.value.is_nan() {
        "NaN"
      } else if self.value > 0.0 {
        "infinity"
      } else if self.value < 0.0 {
        "-infinity"
      } else {
        "infinity"
      };
      return match &self.unit {
        Some(u) => format!("calc({} * 1{})", word, u),
        None => format!("calc({})", word),
      };
    }
    if let Some(c) = &self.compound {
      // The reference keeps full f64 precision inside calc()
      // (0.1px * 0.2px -> calc(0.020000000000000004px * 1px)).
      let coef = format!("{}", self.value);
      let mut parts: Vec<String> = Vec::new();
      if let Some(first) = c.numerators.first() {
        parts.push(format!("{}{}", coef, first));
        for u in &c.numerators[1..] {
          parts.push(format!("1{}", u));
        }
      } else {
        parts.push(coef);
      }
      for d in &c.denominators {
        parts.push(format!("1{}", d));
      }
      // numerators join with `*`, denominators join with `/`
      let n_count = if c.numerators.is_empty() {
        1
      } else {
        c.numerators.len()
      };
      let mut out = parts[0].clone();
      for (i, part) in parts.iter().enumerate().skip(1) {
        if i < n_count {
          out.push_str(" * ");
        } else {
          out.push_str(" / ");
        }
        out.push_str(part);
      }
      return format!("calc({})", out);
    }
    match &self.unit {
      Some(u) => format!("{}{}", format_f64(self.value), u),
      None => format_f64(self.value),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_to_css() {
    assert_eq!(Number::unitless(10.0).to_css(), "10");
    assert_eq!(Number::with_unit(10.0, "px").to_css(), "10px");
    assert_eq!(Number::with_unit(1.5, "rem").to_css(), "1.5rem");
    assert_eq!(Number::with_unit(50.0, "%").to_css(), "50%");
    assert_eq!(Number::with_unit(0.1 + 0.2, "px").to_css(), "0.3px");
  }

  #[test]
  fn test_format_f64_dart_compat() {
    // Large magnitudes use shortest-roundtrip digits, positional
    // notation (dart-sass JS number formatting) — never exponent form
    assert_eq!(format_f64(1e15), "1000000000000000");
    assert_eq!(format_f64(1e21), "1000000000000000000000");
    assert_eq!(format_f64(123456789012345678901.0), "123456789012345680000");
    assert_eq!(format_f64(9007199254740991.0), "9007199254740991");
    // f64::MAX: shortest digits + zeros, no binary-expansion noise
    let max_str = format_f64(f64::MAX);
    assert!(max_str.starts_with("17976931348623157"), "{max_str}");
    assert_eq!(max_str.len(), 309);
    // Tiny magnitudes collapse to 0 like dart
    assert_eq!(format_f64(5e-324), "0");
    assert_eq!(format_f64(1e-11), "0");
    // Precision-10 rounding with zero stripping
    assert_eq!(format_f64(0.0000000001), "0.0000000001");
    assert_eq!(format_f64(1.0 / 3.0), "0.3333333333");
    assert_eq!(format_f64(-0.0), "-0");
  }

  #[test]
  fn test_arithmetic() {
    assert_eq!(
      Number::with_unit(1.0, "px")
        .add(&Number::unitless(2.0))
        .unwrap()
        .to_css(),
      "3px"
    );
    assert_eq!(
      Number::with_unit(2.0, "px")
        .mul(&Number::unitless(3.0))
        .unwrap()
        .to_css(),
      "6px"
    );
    assert_eq!(
      Number::with_unit(10.0, "px")
        .div(&Number::unitless(4.0))
        .unwrap()
        .to_css(),
      "2.5px"
    );
  }

  #[test]
  fn test_unit_conversion() {
    let px = Number::with_unit(96.0, "px");
    let inch = Number::with_unit(1.0, "in");
    assert_eq!(px.add(&inch).unwrap().to_css(), "192px");
    assert_eq!(inch.sub(&px).unwrap().to_css(), "0in");
    assert_eq!(
      Number::with_unit(1000.0, "ms")
        .add(&Number::unitless(1.0))
        .unwrap()
        .to_css(),
      "1001ms"
    );
  }

  #[test]
  fn test_incompatible_units() {
    let a = Number::with_unit(1.0, "px");
    let b = Number::with_unit(1.0, "em");
    assert!(a.add(&b).is_err());
    // Multiplication of differing units is valid in the reference
    // compiler: it yields a compound-unit number (calc(1px * 1em)).
    assert_eq!(a.mul(&b).unwrap().to_css(), "calc(1px * 1em)");
    assert!(
      Number::with_unit(1.0, "s")
        .add(&Number::with_unit(1.0, "px"))
        .is_err()
    );
  }

  #[test]
  fn test_compound_units() {
    // Reference-verified compound unit semantics (dart-sass 1.106.1)
    let prod = Number::with_unit(5.0, "px")
      .mul(&Number::with_unit(10.0, "px"))
      .unwrap();
    assert_eq!(prod.to_css(), "calc(50px * 1px)");
    assert_eq!(prod.unit_string(), "px*px");
    // Chain with a unitless factor folds the coefficient
    let chain = prod.mul(&Number::unitless(2.0)).unwrap();
    assert_eq!(chain.to_css(), "calc(100px * 1px)");
    // Division keeps irreducible quotients compound
    let q = Number::with_unit(1.0, "px")
      .div(&Number::with_unit(1.0, "s"))
      .unwrap();
    assert_eq!(q.to_css(), "calc(1px / 1s)");
    assert_eq!(q.unit_string(), "px/s");
    // Equal units cancel: math.div(10px, 2px) -> 5
    let c = Number::with_unit(10.0, "px")
      .div(&Number::with_unit(2.0, "px"))
      .unwrap();
    assert!(c.is_unitless());
    assert_eq!(c.to_css(), "5");
    // Compound equality ignores calc notation
    assert_eq!(
      prod,
      Number::with_compound(50.0, vec!["px".into(), "px".into()], vec![])
    );
    // Comparisons need the same unit string
    assert!(prod < Number::with_compound(100.0, vec!["px".into(), "px".into()], vec![]));
    // Addition requires identical unit strings
    assert!(
      Number::with_compound(50.0, vec!["px".into(), "px".into()], vec![])
        .add(&Number::with_unit(30.0, "px"))
        .is_err()
    );
    // Compound serialization keeps the coefficient on the first factor
    let r = Number::with_compound(2.5, vec!["px".into(), "px".into()], vec![]);
    assert_eq!(r.to_css(), "calc(2.5px * 1px)");
    // Meter stays symbolic in division (calc(100cm / 1m))
    let m = Number::with_unit(100.0, "cm")
      .div(&Number::with_unit(1.0, "m"))
      .unwrap();
    assert_eq!(m.to_css(), "calc(100cm / 1m)");
  }

  #[test]
  fn test_nonfinite_serialization() {
    assert_eq!(Number::unitless(f64::NAN).to_css(), "calc(NaN)");
    assert_eq!(
      Number::with_unit(f64::INFINITY, "deg").to_css(),
      "calc(infinity * 1deg)"
    );
    assert_eq!(
      Number::with_unit(f64::NEG_INFINITY, "px").to_css(),
      "calc(-infinity * 1px)"
    );
  }

  #[test]
  fn test_div_by_zero() {
    // Reference-verified: division by zero yields IEEE specials in
    // calc() notation instead of erroring.
    assert_eq!(
      Number::unitless(1.0)
        .div(&Number::unitless(0.0))
        .unwrap()
        .to_css(),
      "calc(infinity)"
    );
    assert_eq!(
      Number::unitless(-1.0)
        .div(&Number::unitless(0.0))
        .unwrap()
        .to_css(),
      "calc(-infinity)"
    );
    assert_eq!(
      Number::unitless(0.0)
        .div(&Number::unitless(0.0))
        .unwrap()
        .to_css(),
      "calc(NaN)"
    );
    assert_eq!(
      Number::with_unit(1.0, "px")
        .div(&Number::unitless(0.0))
        .unwrap()
        .to_css(),
      "calc(infinity * 1px)"
    );
  }

  #[test]
  fn test_modulo() {
    assert_eq!(
      Number::unitless(7.0)
        .modulo(&Number::unitless(3.0))
        .unwrap()
        .to_css(),
      "1"
    );
    assert_eq!(
      Number::with_unit(5.0, "px")
        .modulo(&Number::with_unit(3.0, "px"))
        .unwrap()
        .to_css(),
      "2px"
    );
  }

  #[test]
  fn test_compressed_format() {
    assert_eq!(format_f64_compressed(0.5), ".5");
    assert_eq!(format_f64_compressed(-0.25), "-.25");
    assert_eq!(format_f64_compressed(1.5), "1.5");
    assert_eq!(format_f64_compressed(0.0), "0");
  }

  #[test]
  fn test_equality_with_conversion() {
    assert_eq!(Number::with_unit(96.0, "px"), Number::with_unit(1.0, "in"));
    assert!(Number::with_unit(1.0, "px") < Number::with_unit(1.0, "in"));
  }
}
