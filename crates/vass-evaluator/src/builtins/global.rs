//! Global built-in functions (legacy VASS API, no namespace)

use vass_core::error::{Error, Result};
use vass_core::value::format_f64;
use vass_core::value::{
  VassMap, VassString, Color, ColorSpace, ListValue, ModernColor, Number, Separator, Value,
};

use super::{
  Args, arg_error, need_color, need_int, need_map, need_number, need_string, num_unit, num_value,
  number_scaled, str_value, to_list, unquoted,
};

/// Dispatch a global built-in; `None` if the name is unknown here
pub fn call_global(name: &str, args: Args) -> Option<Result<Value>> {
  let r = match name {
    // ---- colors ----------------------------------------------------
    "rgb" => rgb(args, false),
    "rgba" => rgb(args, true),
    "hsl" => hsl(args, false),
    "hsla" => hsl(args, true),
    "lighten" => lighten(args),
    "darken" => darken(args),
    "saturate" => saturate(args),
    "desaturate" => desaturate(args),
    "grayscale" => one_color(args, "grayscale", Color::grayscale),
    "adjust-hue" => adjust_hue(args),
    "complement" => one_color(args, "complement", Color::complement),
    "invert" => invert(args),
    "mix" => mix(args),
    "red" => channel(args, "red", |c| c.red()),
    "green" => channel(args, "green", |c| c.green()),
    "blue" => channel(args, "blue", |c| c.blue()),
    "hue" => channel_unit(args, "hue", |c| c.hue(), "deg"),
    "saturation" => channel_unit(args, "saturation", |c| c.saturation() * 100.0, "%"),
    "lightness" => channel_unit(args, "lightness", |c| c.lightness() * 100.0, "%"),
    "alpha" | "opacity" => channel(args, "alpha", |c| c.alpha()),
    "opacify" | "fade-in" => adjust_alpha(args, "opacify", 1.0),
    "transparentize" | "fade-out" => adjust_alpha(args, "transparentize", -1.0),
    "adjust-color" => adjust_color(args),
    "scale-color" => scale_color(args),
    "change-color" => change_color(args),
    "ie-hex-str" => ie_hex_str(args),

    // ---- numbers ---------------------------------------------------
    "percentage" => percentage(args),
    "round" => math1(args, "round", f64::round),
    "ceil" => math1(args, "ceil", f64::ceil),
    "floor" => math1(args, "floor", f64::floor),
    "abs" => math1(args, "abs", f64::abs),
    "min" => min_max(args, true),
    "max" => min_max(args, false),
    "random" => random(args),
    "comparable" => comparable(args),
    "unit" => unit(args),
    "unitless" => unitless(args),

    // ---- strings ---------------------------------------------------
    "unquote" => unquote(args),
    "quote" => quote(args),
    "to-upper-case" => change_case(args, true),
    "to-lower-case" => change_case(args, false),
    "str-length" => str_length(args),
    "str-insert" => str_insert(args),
    "str-index" => str_index(args),
    "str-slice" => str_slice(args),
    "unique-id" => unique_id(),

    // ---- lists -----------------------------------------------------
    "length" => length(args),
    "nth" => nth(args),
    "set-nth" => set_nth(args),
    "join" => join(args),
    "append" => append(args),
    "zip" => zip(args),
    "index" => index(args),
    "list-separator" => list_separator(args),
    "is-bracketed" => is_bracketed(args),

    // ---- maps ------------------------------------------------------
    "map-get" => map_get(args),
    "map-merge" => map_merge(args),
    "map-remove" => map_remove(args),
    "map-keys" => map_keys(args),
    "map-values" => map_values(args),
    "map-has-key" => map_has_key(args),

    // ---- meta ------------------------------------------------------
    "type-of" => type_of(args),
    "inspect" => inspect(args),
    "keywords" => keywords(args),
    "if" => Err(Error::internal("if() should be handled by the evaluator")),

    // ---- selectors -------------------------------------------------
    "selector-nest" => super::selector_fn::nest(args),
    "selector-append" => super::selector_fn::append(args),
    "selector-unify" => super::selector_fn::unify(args),
    "selector-parse" => super::selector_fn::parse(args),
    "selector-replace" => super::selector_fn::replace(args),
    "selector-superselector" => super::selector_fn::superselector(args),

    _ => return None,
  };
  Some(r)
}

/// Canonical global function names (used by `vass list globals`).
///
/// Aliases are omitted; the first name of an alias group is listed
/// (e.g. `opacify` covers `fade-in`, `alpha` covers `opacity`).
pub fn global_fn_names() -> Vec<&'static str> {
  vec![
    // colors
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "lighten",
    "darken",
    "saturate",
    "desaturate",
    "grayscale",
    "adjust-hue",
    "complement",
    "invert",
    "mix",
    "red",
    "green",
    "blue",
    "hue",
    "saturation",
    "lightness",
    "alpha",
    "opacify",
    "transparentize",
    "adjust-color",
    "scale-color",
    "change-color",
    "ie-hex-str",
    // numbers
    "percentage",
    "round",
    "ceil",
    "floor",
    "abs",
    "min",
    "max",
    "random",
    "comparable",
    "unit",
    "unitless",
    // strings
    "unquote",
    "quote",
    "to-upper-case",
    "to-lower-case",
    "str-length",
    "str-insert",
    "str-index",
    "str-slice",
    "unique-id",
    // lists
    "length",
    "nth",
    "set-nth",
    "join",
    "append",
    "zip",
    "index",
    "list-separator",
    "is-bracketed",
    // maps
    "map-get",
    "map-merge",
    "map-remove",
    "map-keys",
    "map-values",
    "map-has-key",
    // meta
    "type-of",
    "inspect",
    "keywords",
    "if",
    // selectors
    "selector-nest",
    "selector-append",
    "selector-unify",
    "selector-parse",
    "selector-replace",
    "selector-superselector",
  ]
}

// ---- helpers ---------------------------------------------------------------

fn one_color(args: Args, fname: &str, f: impl Fn(&Color) -> Color) -> Result<Value> {
  let c = need_color(args.require(0, "color", fname)?, fname, "color")?;
  Ok(Value::Color(tag_hsl(&c, f(&c))))
}

fn channel(args: Args, fname: &str, f: impl Fn(&Color) -> f64) -> Result<Value> {
  let c = need_color(args.require(0, "color", fname)?, fname, "color")?;
  Ok(num_value(f(&c)))
}

/// Channel getter whose result carries its natural unit (`deg`/`%`),
/// matching the reference serialization of legacy channel functions
fn channel_unit(args: Args, fname: &str, f: impl Fn(&Color) -> f64, unit: &str) -> Result<Value> {
  let c = need_color(args.require(0, "color", fname)?, fname, "color")?;
  Ok(Value::Number(vass_core::value::Number::new(
    f(&c),
    Some(unit.to_string()),
  )))
}

/// Parse an rgb channel (number 0-255 or percentage)
fn channel_byte(v: &Value, fname: &str, arg: &str) -> Result<f64> {
  let n = need_number(v, fname, arg)?;
  let val = match n.unit.as_deref() {
    Some("%") => n.value / 100.0 * 255.0,
    None => n.value,
    Some(other) => {
      return Err(Error::type_error(format!(
        "{}: `${}` must be unitless or a percentage, got `{}`.",
        fname, arg, other
      )));
    }
  };
  Ok(val.clamp(0.0, 255.0))
}

fn alpha_f64(v: Option<&Value>, fname: &str, arg: &str) -> Result<f64> {
  match v {
    None => Ok(1.0),
    Some(v) => {
      let n = need_number(v, fname, arg)?;
      let a = match n.unit.as_deref() {
        Some("%") => n.value / 100.0,
        _ => n.value,
      };
      Ok(a.clamp(0.0, 1.0))
    }
  }
}

fn rgb(args: Args, _rgba: bool) -> Result<Value> {
  const F: &str = "rgb";
  // Every color produced by rgb()/rgba() serializes in the canonical
  // comma function form (name normalizes rgb<->rgba by alpha), matching
  // the reference compiler's notation preservation. Colors with missing
  // (`none`) channels serialize in the space-separated form instead.
  fn finish(c: Color) -> Value {
    if c.has_missing() {
      // The missing-channel form is derived from the channels (never
      // a pinned literal) so @debug keeps full float precision
      return Value::Color(c);
    }
    let text = c.canonical_fn_text();
    Value::Color(c.with_literal(&text))
  }
  if args.positional.len() == 1 && args.named.is_empty() {
    // `rgba($color, $alpha)` or space-list `rgb(255 0 0)`
    let v = args.positional[0];
    return match v {
      Value::List(l) if l.sep == Separator::Space && l.items.len() >= 3 => {
        // Modern syntax `rgb(0 0 0 / 50%)`: the literal slash between
        // the last channel and alpha survives evaluation as the
        // unquoted string `0/50%` (slash-as-division is preserved
        // outside math contexts, matching the reference compiler).
        if l.items.len() == 3
          && let Some((chan, alpha)) = split_slash_channels(&l.items[2])
        {
          let r = channel_or_none(&l.items[0], F, "red")?;
          let g = channel_or_none(&l.items[1], F, "green")?;
          let b = channel_or_none(&chan, F, "blue")?;
          let a = alpha_or_none(Some(&alpha), F, "alpha")?;
          return Ok(finish(Color::from_legacy_channels([r, g, b], a, false)));
        }
        let r = channel_or_none(&l.items[0], F, "red")?;
        let g = channel_or_none(&l.items[1], F, "green")?;
        let b = channel_or_none(&l.items[2], F, "blue")?;
        let a = alpha_or_none(l.items.get(3), F, "alpha")?;
        Ok(finish(Color::from_legacy_channels([r, g, b], a, false)))
      }
      Value::Color(c) => Ok(Value::Color(c.clone())),
      _ => Err(Error::type_error("rgb() expects 3 channels or a color.")),
    };
  }
  // `rgb($color, $alpha)` two-arg form
  if args.positional.len() == 2 {
    let c = need_color(args.positional[0], F, "color")?;
    let a = alpha_f64(Some(args.positional[1]), F, "alpha")?;
    return Ok(finish(Color::from_channels(c.r, c.g, c.b, a)));
  }
  // Comma form: `none` channels are rejected by the number coercion below
  // (matching the reference: `$green: none is not a number.`)
  let r = channel_byte(args.require(0, "red", F)?, F, "red")?;
  let g = channel_byte(args.require(1, "green", F)?, F, "green")?;
  let b = channel_byte(args.require(2, "blue", F)?, F, "blue")?;
  let a = alpha_f64(args.get(3, "alpha"), F, "alpha")?;
  Ok(finish(Color::from_channels(r, g, b, a)))
}

/// Parse an rgb channel in the modern space-separated form: `none` marks a
/// missing channel, numbers scale like `channel_byte` (percent -> 0-255)
fn channel_or_none(v: &Value, fname: &str, arg: &str) -> Result<Option<f64>> {
  if let Value::Str(s) = v
    && !s.quoted
    && s.text.eq_ignore_ascii_case("none")
  {
    return Ok(None);
  }
  Ok(Some(channel_byte(v, fname, arg)?))
}

/// Parse an alpha value that may be `none` (missing); percent scales to 0-1
fn alpha_or_none(v: Option<&Value>, fname: &str, arg: &str) -> Result<Option<f64>> {
  match v {
    None => Ok(Some(1.0)),
    Some(Value::Str(s)) if !s.quoted && s.text == "none" => Ok(None),
    Some(other) => {
      let n = need_number(other, fname, arg)?;
      let a = match n.unit.as_deref() {
        Some("%") => n.value / 100.0,
        _ => n.value,
      };
      Ok(Some(a.clamp(0.0, 1.0)))
    }
  }
}

/// Parse canonical number text produced by `Number::to_css` (`0`, `50%`,
/// `0.5`) back into a Value::Number, or None if it is not a plain number.
fn parse_number_text(text: &str) -> Option<Value> {
  let t = text.trim();
  if let Some(p) = t.strip_suffix('%') {
    p.parse::<f64>().ok().map(|v| num_unit(v, "%"))
  } else {
    t.parse::<f64>().ok().map(num_value)
  }
}

/// dart-sass modern color syntax: `rgb(0 0 0 / 50%)` reaches the evaluator
/// with the last channel as the literal-slash string `0/50%`. Split it into
/// (channel, alpha) values; `none` parts pass through as the unquoted
/// `none` string for the missing-channel paths. None when the text is not
/// a `num/num` literal.
fn split_slash_channels(v: &Value) -> Option<(Value, Value)> {
  match v {
    Value::Str(s) if !s.quoted => {
      let (a, b) = s.text.split_once('/')?;
      let chan = if a.trim().eq_ignore_ascii_case("none") {
        Value::Str(VassString::unquoted("none"))
      } else {
        parse_number_text(a)?
      };
      let alpha = if b.trim().eq_ignore_ascii_case("none") {
        Value::Str(VassString::unquoted("none"))
      } else {
        parse_number_text(b)?
      };
      Some((chan, alpha))
    }
    _ => None,
  }
}

fn hsl(args: Args, _hsla: bool) -> Result<Value> {
  const F: &str = "hsl";
  // `hsl(210 50% 50%)` space form
  if args.positional.len() == 1
    && args.named.is_empty()
    && let Value::List(l) = args.positional[0]
  {
    if l.sep == Separator::Space && l.items.len() >= 3 {
      // Modern syntax `hsl(210 50% 50% / 50%)`: unwrap the
      // literal-slash `num/num` string in the last position.
      if l.items.len() == 3
        && let Some((light, alpha)) = split_slash_channels(&l.items[2])
      {
        return hsl_space_form(&l.items[0], &l.items[1], &light, Some(&alpha));
      }
      return hsl_space_form(&l.items[0], &l.items[1], &l.items[2], l.items.get(3));
    }
    return Err(Error::type_error("hsl() expects 3 components."));
  }
  let h = args.require(0, "hue", F)?;
  let s = args.require(1, "saturation", F)?;
  let l = args.require(2, "lightness", F)?;
  let a = args.get(3, "alpha");
  hsl_from(h, s, l, a)
}

/// Space-separated hsl form — unlike the comma form this accepts `none`
/// channels (missing channels), matching the reference
fn hsl_space_form(hv: &Value, sv: &Value, lv: &Value, av: Option<&Value>) -> Result<Value> {
  const F: &str = "hsl";
  let h = hsl_channel_or_none(hv, F, "hue")?;
  let s = hsl_channel_or_none(sv, F, "saturation")?;
  let l = hsl_channel_or_none(lv, F, "lightness")?;
  let a = alpha_or_none(av, F, "alpha")?;
  if h.is_none() || s.is_none() || l.is_none() || a.is_none() {
    // Missing channels: build an hsl-space color with `none` channels
    let mc = ModernColor::new(
      ColorSpace::Hsl,
      [
        h.map(|v| v.rem_euclid(360.0)),
        s.map(|v| (v.max(0.0)) / 100.0),
        l.map(|v| v / 100.0),
      ],
      a,
    );
    return Ok(Value::Color(mc.into_color()));
  }
  hsl_from(hv, sv, lv, av)
}

/// hsl channel: `none` marks a missing channel; otherwise a number whose
/// unit is degrees (hue) or a percentage/unitless (s/l, on the 0-100 scale)
fn hsl_channel_or_none(v: &Value, fname: &str, arg: &str) -> Result<Option<f64>> {
  if let Value::Str(s) = v
    && !s.quoted
    && s.text.eq_ignore_ascii_case("none")
  {
    return Ok(None);
  }
  let n = need_number(v, fname, arg)?;
  Ok(Some(n.value))
}

fn hsl_from(hv: &Value, sv: &Value, lv: &Value, av: Option<&Value>) -> Result<Value> {
  const F: &str = "hsl";
  let h = need_number(hv, F, "hue")?.value;
  // The reference clamps saturation at 0 from below (never from above);
  // lightness stays raw in the display text
  let s_pct = need_number(sv, F, "saturation")?.value.max(0.0);
  let l_pct = need_number(lv, F, "lightness")?.value;
  let a = match av {
    Some(v) => {
      let n = need_number(v, F, "alpha")?;
      match n.unit.as_deref() {
        Some("%") => n.value / 100.0,
        _ => n.value,
      }
    }
    None => 1.0,
  };
  // from_hsl_raw stores the RAW channels (saturation lower-clamped at 0
  // only, lightness unclamped) and tags the color as living in the legacy
  // hsl space, matching the reference's channel construction rules —
  // `hsl(0 200% 150%)` keeps 200%/150% for channel reads, equality and
  // gamut checks while the rgb backing clamps to white.
  let c = Color::from_hsl_raw(h, s_pct / 100.0, l_pct / 100.0, a.clamp(0.0, 1.0));
  // Every color produced by hsl()/hsla() serializes in canonical comma
  // function form: hue normalized to 0-360 (unit dropped), saturation and
  // lightness as written (unitless treated as %), name hsl<->hsla by alpha.
  let h_norm = ((h % 360.0) + 360.0) % 360.0;
  let text = if a >= 1.0 {
    format!(
      "hsl({}, {}%, {}%)",
      format_f64(h_norm),
      format_f64(s_pct),
      format_f64(l_pct)
    )
  } else {
    format!(
      "hsla({}, {}%, {}%, {})",
      format_f64(h_norm),
      format_f64(s_pct),
      format_f64(l_pct),
      format_f64(a.clamp(0.0, 1.0))
    )
  };
  Ok(Value::Color(c.with_literal(&text)))
}

fn lighten(args: Args) -> Result<Value> {
  const F: &str = "lighten";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let amount = number_scaled(args.require(1, "amount", F)?, F, "amount")?;
  Ok(Value::Color(tag_hsl(&c, c.adjust_lightness(amount))))
}

fn darken(args: Args) -> Result<Value> {
  const F: &str = "darken";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let amount = number_scaled(args.require(1, "amount", F)?, F, "amount")?;
  Ok(Value::Color(tag_hsl(&c, c.adjust_lightness(-amount))))
}

fn saturate(args: Args) -> Result<Value> {
  const F: &str = "saturate";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let amount = number_scaled(args.require(1, "amount", F)?, F, "amount")?;
  Ok(Value::Color(tag_hsl(&c, c.adjust_saturation(amount))))
}

fn desaturate(args: Args) -> Result<Value> {
  const F: &str = "desaturate";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let amount = number_scaled(args.require(1, "amount", F)?, F, "amount")?;
  Ok(Value::Color(tag_hsl(&c, c.adjust_saturation(-amount))))
}

/// Keep the legacy hsl-space tag on computed results (the reference keeps
/// hsl notation for colors created in the hsl space)
fn tag_hsl(source: &Color, mut out: Color) -> Color {
  if source.is_hsl_space() && out.modern.is_none() {
    out.missing |= vass_core::value::color::missing_flags::HSL_SPACE;
  }
  out
}

fn adjust_hue(args: Args) -> Result<Value> {
  const F: &str = "adjust-hue";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let deg = need_number(args.require(1, "degrees", F)?, F, "degrees")?.value;
  Ok(Value::Color(tag_hsl(&c, c.adjust_hue(deg))))
}

fn invert(args: Args) -> Result<Value> {
  const F: &str = "invert";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let weight = match args.get(1, "weight") {
    Some(v) => number_scaled(v, F, "weight")?,
    None => 1.0,
  };
  Ok(Value::Color(c.invert(weight)))
}

fn mix(args: Args) -> Result<Value> {
  const F: &str = "mix";
  let c1 = need_color(args.require(0, "color1", F)?, F, "color1")?;
  let c2 = need_color(args.require(1, "color2", F)?, F, "color2")?;
  let weight = match args.get(2, "weight") {
    Some(v) => number_scaled(v, F, "weight")?,
    None => 0.5,
  };
  Ok(Value::Color(c1.mix(&c2, weight)))
}

fn adjust_alpha(args: Args, fname: &str, sign: f64) -> Result<Value> {
  let c = need_color(args.require(0, "color", fname)?, fname, "color")?;
  let amount = number_scaled(args.require(1, "amount", fname)?, fname, "amount")?;
  Ok(Value::Color(c.opacify(amount * sign)))
}

fn adjust_color(args: Args) -> Result<Value> {
  const F: &str = "adjust-color";
  let mut c = need_color(args.require(0, "color", F)?, F, "color")?;
  let get = |name: &str| -> Result<Option<f64>> {
    match args.named.iter().find(|(n, _)| n == name) {
      Some((_, v)) => Ok(Some(need_number(v, F, name)?.value)),
      None => Ok(None),
    }
  };
  let dr = get("red")?;
  let dg = get("green")?;
  let db = get("blue")?;
  let dh = get("hue")?;
  let ds = get("saturation")?;
  let dl = get("lightness")?;
  let da = get("alpha")?;
  if dr.is_some() || dg.is_some() || db.is_some() {
    if dh.is_some() || ds.is_some() || dl.is_some() {
      return Err(Error::type_error(
        "adjust-color: cannot adjust RGB and HSL channels together.",
      ));
    }
    let adj = |base: f64, d: Option<f64>| -> f64 { (base + d.unwrap_or(0.0)).clamp(0.0, 255.0) };
    let alpha = (c.a + da.unwrap_or(0.0)).clamp(0.0, 1.0);
    c = Color::from_channels(adj(c.r, dr), adj(c.g, dg), adj(c.b, db), alpha);
  } else if dh.is_some() || ds.is_some() || dl.is_some() {
    let (h, s, l) = c.to_hsl();
    let h = h + dh.unwrap_or(0.0);
    let s = (s * 100.0 + ds.unwrap_or(0.0)) / 100.0;
    let l = (l * 100.0 + dl.unwrap_or(0.0)) / 100.0;
    let a = c.a + da.unwrap_or(0.0);
    // Reference channel construction rules: hsl-space colors keep raw
    // channels (saturation lower-clamped only, lightness unclamped —
    // `adjust-color(hsl(0 50% 50%), $lightness: 100%)` is `hsl(0, 50%,
    // 150%)`); rgb colors clamp through the hsl roundtrip.
    c = if c.is_hsl_space() {
      Color::from_hsl_raw(h, s, l, a.clamp(0.0, 1.0))
    } else {
      Color::from_hsl(h, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0), a.clamp(0.0, 1.0))
    };
  } else if let Some(a) = da {
    c = c.opacify(a);
  }
  Ok(Value::Color(c))
}

fn scale_color(args: Args) -> Result<Value> {
  const F: &str = "scale-color";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let scale = |base: f64, pct: Option<f64>, max: f64| -> f64 {
    match pct {
      Some(p) if p > 0.0 => base + (max - base) * p / 100.0,
      Some(p) => base * (1.0 + p / 100.0),
      None => base,
    }
  };
  let get = |name: &str| -> Option<f64> {
    args
      .named
      .iter()
      .find(|(n, _)| n == name)
      .and_then(|(_, v)| v.as_number())
      .map(|n| n.value)
  };
  let sl = get("lightness");
  let ss = get("saturation");
  if sl.is_some() || ss.is_some() {
    let (h, s, l) = c.to_hsl();
    // Reference `_scaleChannel` short-circuits at the bounds and scales
    // toward min for negative factors; hsl-space colors keep raw channels
    let scale_ch = |base: f64, pct: Option<f64>, min: f64, max: f64| -> f64 {
      match pct {
        None => base,
        Some(0.0) => base,
        Some(p) if p > 0.0 => {
          if base >= max {
            base
          } else {
            base + (max - base) * p / 100.0
          }
        }
        Some(p) => {
          if base <= min {
            base
          } else {
            base + (base - min) * p / 100.0
          }
        }
      }
    };
    let s2 = scale_ch(s * 100.0, ss, 0.0, 100.0) / 100.0;
    let l2 = scale_ch(l * 100.0, sl, 0.0, 100.0) / 100.0;
    let a = scale_ch(c.alpha() * 100.0, get("alpha"), 0.0, 100.0) / 100.0;
    let out = if c.is_hsl_space() {
      Color::from_hsl_raw(h, s2, l2, a.clamp(0.0, 1.0))
    } else {
      Color::from_hsl(h, s2, l2, a.clamp(0.0, 1.0))
    };
    return Ok(Value::Color(out));
  }
  let r = scale(c.r, get("red"), 255.0);
  let g = scale(c.g, get("green"), 255.0);
  let b = scale(c.b, get("blue"), 255.0);
  let a = scale(c.a * 100.0, get("alpha"), 100.0) / 100.0;
  Ok(Value::Color(Color::from_channels(
    r.clamp(0.0, 255.0),
    g.clamp(0.0, 255.0),
    b.clamp(0.0, 255.0),
    a.clamp(0.0, 1.0),
  )))
}

fn change_color(args: Args) -> Result<Value> {
  const F: &str = "change-color";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let get = |name: &str| -> Option<f64> {
    args
      .named
      .iter()
      .find(|(n, _)| n == name)
      .and_then(|(_, v)| v.as_number())
      .map(|n| n.value)
  };
  let r = get("red").map(|v| v.clamp(0.0, 255.0));
  let g = get("green").map(|v| v.clamp(0.0, 255.0));
  let b = get("blue").map(|v| v.clamp(0.0, 255.0));
  let a = get("alpha").map(|v| v.clamp(0.0, 1.0));
  if r.is_some() || g.is_some() || b.is_some() || a.is_some() {
    return Ok(Value::Color(Color::from_channels(
      r.unwrap_or(c.r),
      g.unwrap_or(c.g),
      b.unwrap_or(c.b),
      a.unwrap_or(c.a),
    )));
  }
  let hue = get("hue");
  let sat = get("saturation");
  let light = get("lightness");
  let alpha = get("alpha");
  if hue.is_some() || sat.is_some() || light.is_some() || alpha.is_some() {
    let (h, s, l) = c.to_hsl();
    // hsl-space colors keep raw channels: `change-color(hsl(0 50% 50%),
    // $saturation: 250%)` is `hsl(0, 250%, 50%)` in the reference
    let out = if c.is_hsl_space() {
      Color::from_hsl_raw(
        hue.unwrap_or(h),
        sat.map(|v| v / 100.0).unwrap_or(s),
        light.map(|v| v / 100.0).unwrap_or(l),
        alpha.unwrap_or(c.a).clamp(0.0, 1.0),
      )
    } else {
      Color::from_hsl(
        hue.unwrap_or(h),
        sat.map(|v| v / 100.0).unwrap_or(s),
        light.map(|v| v / 100.0).unwrap_or(l),
        alpha.unwrap_or(c.a).clamp(0.0, 1.0),
      )
    };
    return Ok(Value::Color(out));
  }
  Ok(Value::Color(c))
}

fn ie_hex_str(args: Args) -> Result<Value> {
  const F: &str = "ie-hex-str";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  Ok(unquoted(format!(
    "#{:02X}{:02X}{:02X}{:02X}",
    c.alpha_u8(),
    c.r.round() as u8,
    c.g.round() as u8,
    c.b.round() as u8
  )))
}

// ---- numbers ---------------------------------------------------------------

fn math1(args: Args, fname: &str, f: impl Fn(f64) -> f64) -> Result<Value> {
  let n = need_number(args.require(0, "number", fname)?, fname, "number")?;
  Ok(Value::Number(Number::new(f(n.value), n.unit.clone())))
}

fn percentage(args: Args) -> Result<Value> {
  const F: &str = "percentage";
  let n = need_number(args.require(0, "number", F)?, F, "number")?;
  if n.unit.is_some() {
    return Err(Error::type_error(format!(
      "{}: `${}` must be unitless.",
      F, "number"
    )));
  }
  Ok(num_unit(n.value * 100.0, "%"))
}

fn min_max(args: Args, is_min: bool) -> Result<Value> {
  let fname = if is_min { "min" } else { "max" };
  min_max_values(&args, is_min, fname, fname)
}

/// Compare two Sass numbers on a common footing; `None` when their units are
/// incompatible — the reference compiler then keeps min()/max() as plain CSS
/// calculations (e.g. `min(1px, 2%)` stays a function call).
pub(crate) fn cmp_numbers(a: &Number, b: &Number) -> Option<std::cmp::Ordering> {
  if a.unit.is_none() || b.unit.is_none() || a.unit == b.unit {
    a.value.partial_cmp(&b.value)
  } else {
    let (ua, ub) = (a.unit.as_ref()?, b.unit.as_ref()?);
    if let Some(c) = a.convert_to(ub) {
      c.value.partial_cmp(&b.value)
    } else if let Some(c) = b.convert_to(ua) {
      a.value.partial_cmp(&c.value)
    } else {
      None
    }
  }
}

/// Shared min()/max() evaluation used by the global builtins and `sass:math`.
/// `fname` is used in error messages, `calc_name` in the preserved-CSS output
/// (always bare `min`/`max`, never module-qualified).
pub(crate) fn min_max_values(
  args: &Args,
  is_min: bool,
  fname: &str,
  calc_name: &str,
) -> Result<Value> {
  if args.positional.is_empty() {
    return Err(arg_error(fname, "at least 1", 0));
  }
  use std::cmp::Ordering;
  let mut best = args.positional[0].clone();
  let mut comparable = true;
  for v in &args.positional[1..] {
    let (a, b) = (best.as_number(), v.as_number());
    if let (Some(x), Some(y)) = (a, b) {
      match cmp_numbers(x, y) {
        Some(ord) => {
          let take = if is_min {
            ord == Ordering::Greater
          } else {
            ord == Ordering::Less
          };
          if take {
            best = (*v).clone();
          }
        }
        None => {
          comparable = false;
          break;
        }
      }
    }
  }
  if comparable {
    Ok(best)
  } else {
    let joined = args
      .positional
      .iter()
      .map(|v| v.to_css())
      .collect::<Vec<_>>()
      .join(", ");
    Ok(Value::Str(VassString::unquoted(format!(
      "{}({})",
      calc_name, joined
    ))))
  }
}

fn random(args: Args) -> Result<Value> {
  use std::time::{SystemTime, UNIX_EPOCH};
  const F: &str = "random";
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
    .unwrap_or(42);
  match args.get(0, "limit") {
    None => {
      // 0..1 with 8 decimal digits of entropy
      let frac = (nanos % 1_0000_0000) as f64 / 1_0000_0000.0;
      Ok(num_value(frac))
    }
    Some(v) => {
      let limit = need_int(v, F, "limit")?;
      if limit < 1 {
        return Err(Error::type_error(format!(
          "{}: `$limit` must be >= 1, got {}.",
          F, limit
        )));
      }
      Ok(num_value((nanos % limit as u64) as f64 + 1.0))
    }
  }
}

fn comparable(args: Args) -> Result<Value> {
  const F: &str = "comparable";
  let a = need_number(args.require(0, "number1", F)?, F, "number1")?;
  let b = need_number(args.require(1, "number2", F)?, F, "number2")?;
  let ok = a.unit == b.unit
    || a.is_unitless()
    || b.is_unitless()
    || a.convert_to(b.unit.as_deref().unwrap_or("")).is_some();
  Ok(Value::Bool(ok))
}

fn unit(args: Args) -> Result<Value> {
  const F: &str = "unit";
  let n = need_number(args.require(0, "number", F)?, F, "number")?;
  Ok(str_value(n.unit.clone().unwrap_or_default(), true))
}

fn unitless(args: Args) -> Result<Value> {
  const F: &str = "unitless";
  let n = need_number(args.require(0, "number", F)?, F, "number")?;
  Ok(Value::Bool(n.is_unitless()))
}

// ---- strings ---------------------------------------------------------------

fn unquote(args: Args) -> Result<Value> {
  const F: &str = "unquote";
  let v = args.require(0, "string", F)?;
  Ok(match v {
    Value::Str(s) => Value::Str(VassString::unquoted(s.text.clone())),
    Value::Number(n) => unquoted(n.to_css()),
    Value::Color(c) => unquoted(c.to_css()),
    other => unquoted(other.to_css()),
  })
}

fn quote(args: Args) -> Result<Value> {
  const F: &str = "quote";
  let v = args.require(0, "string", F)?;
  let text = need_string(v, F, "string")?;
  Ok(str_value(text, true))
}

fn change_case(args: Args, upper: bool) -> Result<Value> {
  const F: &str = "to-upper-case";
  let v = args.require(0, "string", F)?;
  match v {
    Value::Str(s) => {
      let text = if upper {
        s.text.to_ascii_uppercase()
      } else {
        s.text.to_ascii_lowercase()
      };
      Ok(Value::Str(VassString::new(text, s.quoted)))
    }
    _ => Err(Error::type_error(format!(
      "{}: `${}` is not a string.",
      F, "string"
    ))),
  }
}

fn str_length(args: Args) -> Result<Value> {
  const F: &str = "str-length";
  let s = need_string(args.require(0, "string", F)?, F, "string")?;
  Ok(num_value(s.chars().count() as f64))
}

fn str_insert(args: Args) -> Result<Value> {
  const F: &str = "str-insert";
  let s = need_string(args.require(0, "string", F)?, F, "string")?;
  let insert = need_string(args.require(1, "insert", F)?, F, "insert")?;
  let idx = need_int(args.require(2, "index", F)?, F, "index")?;
  let quoted = matches!(args.positional[0], Value::Str(s) if s.quoted);
  let chars: Vec<char> = s.chars().collect();
  let len = chars.len() as i64;
  let pos = if idx <= 0 {
    0
  } else if idx >= len {
    len
  } else {
    idx - 1
  };
  let pos = pos.clamp(0, len) as usize;
  let mut out: String = chars[..pos].iter().collect();
  out.push_str(&insert);
  out.extend(chars[pos..].iter());
  Ok(str_value(out, quoted))
}

fn str_index(args: Args) -> Result<Value> {
  const F: &str = "str-index";
  let s = need_string(args.require(0, "string", F)?, F, "string")?;
  let sub = need_string(args.require(1, "substring", F)?, F, "substring")?;
  match s.find(&sub) {
    Some(byte_idx) => {
      let n = s[..byte_idx].chars().count();
      Ok(num_value(n as f64 + 1.0))
    }
    None => Ok(Value::Null),
  }
}

fn str_slice(args: Args) -> Result<Value> {
  const F: &str = "str-slice";
  let s = need_string(args.require(0, "string", F)?, F, "string")?;
  let start = need_int(args.require(1, "start-at", F)?, F, "start-at")?;
  let end = match args.get(2, "end-at") {
    Some(v) if !matches!(v, Value::Null) => need_int(v, F, "end-at")?,
    _ => i64::MAX,
  };
  let quoted = matches!(args.positional[0], Value::Str(s) if s.quoted);
  let chars: Vec<char> = s.chars().collect();
  let len = chars.len() as i64;
  let norm = |i: i64| -> i64 { if i < 0 { len + i + 1 } else { i } };
  let a = norm(start).clamp(1, len.max(1));
  let b = if end == i64::MAX {
    len
  } else {
    norm(end).clamp(0, len)
  };
  if a > b {
    return Ok(str_value("", quoted));
  }
  let out: String = chars[(a - 1) as usize..b as usize].iter().collect();
  Ok(str_value(out, quoted))
}

fn unique_id() -> Result<Value> {
  Ok(global_unique_id())
}

/// Shared unique-id generator (also used by `string.unique-id`)
pub fn global_unique_id() -> Value {
  use std::sync::atomic::{AtomicU64, Ordering};
  static COUNTER: AtomicU64 = AtomicU64::new(0);
  let n = COUNTER.fetch_add(1, Ordering::Relaxed);
  unquoted(format!("u{:x}", n))
}

// ---- lists -----------------------------------------------------------------

fn length(args: Args) -> Result<Value> {
  const F: &str = "length";
  let (items, _, _) = to_list(args.require(0, "list", F)?);
  Ok(num_value(items.len() as f64))
}

fn nth(args: Args) -> Result<Value> {
  const F: &str = "nth";
  let (items, _, _) = to_list(args.require(0, "list", F)?);
  let n = need_int(args.require(1, "n", F)?, F, "n")?;
  if items.is_empty() {
    return Err(Error::type_error("nth: list is empty."));
  }
  let len = items.len() as i64;
  let idx = if n < 0 { len + n } else { n - 1 };
  if idx < 0 || idx >= len {
    return Err(Error::type_error(format!(
      "nth: index {} out of range for a list of length {}.",
      n, len
    )));
  }
  Ok(items[idx as usize].clone())
}

fn set_nth(args: Args) -> Result<Value> {
  const F: &str = "set-nth";
  let (mut items, sep, bracketed) = to_list(args.require(0, "list", F)?);
  let n = need_int(args.require(1, "n", F)?, F, "n")?;
  let value = args.require(2, "value", F)?.clone();
  if items.is_empty() {
    return Err(Error::type_error("set-nth: list is empty."));
  }
  let len = items.len() as i64;
  let idx = if n < 0 { len + n } else { n - 1 };
  if idx < 0 || idx >= len {
    return Err(Error::type_error(format!(
      "set-nth: index {} out of range.",
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
  const F: &str = "join";
  let (a, sep_a, br_a) = to_list(args.require(0, "list1", F)?);
  let (b, sep_b, br_b) = to_list(args.require(1, "list2", F)?);
  let sep = match args.get(2, "separator") {
    Some(Value::Str(s)) if s.text == "comma" => Separator::Comma,
    Some(Value::Str(s)) if s.text == "space" => Separator::Space,
    _ => {
      if a.len() > 1 {
        sep_a
      } else if b.len() > 1 {
        sep_b
      } else {
        Separator::Space
      }
    }
  };
  let bracketed = match args.get(3, "bracketed") {
    Some(Value::Str(s)) if s.text == "auto" => br_a || br_b,
    Some(Value::Bool(b)) => *b,
    Some(Value::Null) => br_a || br_b,
    _ => br_a || br_b,
  };
  let mut items = a;
  items.extend(b);
  Ok(Value::List(ListValue {
    items,
    sep,
    bracketed,
  }))
}

fn append(args: Args) -> Result<Value> {
  const F: &str = "append";
  let (mut items, sep_orig, bracketed) = to_list(args.require(0, "list", F)?);
  let value = args.require(1, "val", F)?.clone();
  let sep = match args.get(2, "separator") {
    Some(Value::Str(s)) if s.text == "comma" => Separator::Comma,
    Some(Value::Str(s)) if s.text == "space" => Separator::Space,
    _ => sep_orig,
  };
  items.push(value);
  Ok(Value::List(ListValue {
    items,
    sep,
    bracketed,
  }))
}

fn zip(args: Args) -> Result<Value> {
  const F: &str = "zip";
  if args.positional.is_empty() {
    return Ok(Value::List(ListValue::new(vec![], Separator::Comma)));
  }
  let lists: Vec<Vec<Value>> = args.positional.iter().map(|v| to_list(v).0).collect();
  let min_len = lists.iter().map(|l| l.len()).min().unwrap_or(0);
  let mut items = Vec::with_capacity(min_len);
  for i in 0..min_len {
    let row: Vec<Value> = lists.iter().map(|l| l[i].clone()).collect();
    items.push(Value::List(ListValue::new(row, Separator::Space)));
  }
  Ok(Value::List(ListValue::new(items, Separator::Comma)))
}

fn index(args: Args) -> Result<Value> {
  const F: &str = "index";
  let (items, _, _) = to_list(args.require(0, "list", F)?);
  let needle = args.require(1, "value", F)?;
  for (i, item) in items.iter().enumerate() {
    if vass_core::value::values_equal(item, needle) {
      return Ok(num_value(i as f64 + 1.0));
    }
  }
  Ok(Value::Null)
}

fn list_separator(args: Args) -> Result<Value> {
  const F: &str = "list-separator";
  let (_, sep, _) = to_list(args.require(0, "list", F)?);
  Ok(str_value(
    if sep == Separator::Comma {
      "comma"
    } else {
      "space"
    },
    true,
  ))
}

fn is_bracketed(args: Args) -> Result<Value> {
  const F: &str = "is-bracketed";
  let (_, _, bracketed) = to_list(args.require(0, "list", F)?);
  Ok(Value::Bool(bracketed))
}

// ---- maps ------------------------------------------------------------------

fn map_get(args: Args) -> Result<Value> {
  const F: &str = "map-get";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  let key = args.require(1, "key", F)?.clone();
  let mut cur = m;
  let mut val: Option<Value> = None;
  for k in std::iter::once(key).chain(
    // deep path: map-get($map, $key1, $key2, ...)
    args.positional.iter().skip(2).map(|v| (*v).clone()),
  ) {
    match cur.get(&k) {
      Some(Value::Map(inner)) => {
        val = Some(Value::Map((*inner).clone()));
        cur = (*inner).clone();
      }
      Some(other) => {
        val = Some(other.clone());
        cur = VassMap::new();
      }
      None => return Ok(Value::Null),
    }
  }
  Ok(val.unwrap_or(Value::Null))
}

fn map_merge(args: Args) -> Result<Value> {
  const F: &str = "map-merge";
  let a = need_map(args.require(0, "map1", F)?, F, "map1")?;
  let b = need_map(args.require(1, "map2", F)?, F, "map2")?;
  let mut out = a;
  for (k, v) in b.entries {
    out.set(k, v);
  }
  Ok(Value::Map(out))
}

fn map_remove(args: Args) -> Result<Value> {
  const F: &str = "map-remove";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  let mut out = m;
  for k in &args.positional[1..] {
    out.remove(k);
  }
  Ok(Value::Map(out))
}

fn map_keys(args: Args) -> Result<Value> {
  const F: &str = "map-keys";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  Ok(Value::List(ListValue::new(m.keys(), Separator::Comma)))
}

fn map_values(args: Args) -> Result<Value> {
  const F: &str = "map-values";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  Ok(Value::List(ListValue::new(m.values(), Separator::Comma)))
}

fn map_has_key(args: Args) -> Result<Value> {
  const F: &str = "map-has-key";
  let m = need_map(args.require(0, "map", F)?, F, "map")?;
  if args.positional.len() < 2 {
    return Err(arg_error(F, "2+", args.positional.len()));
  }
  let mut cur = m;
  for k in &args.positional[1..] {
    match cur.get(k) {
      Some(Value::Map(inner)) => cur = (*inner).clone(),
      Some(_) => return Ok(Value::Bool(true)),
      None => return Ok(Value::Bool(false)),
    }
  }
  Ok(Value::Bool(true))
}

// ---- meta ------------------------------------------------------------------

fn type_of(args: Args) -> Result<Value> {
  const F: &str = "type-of";
  let v = args.require(0, "value", F)?;
  Ok(str_value(v.type_name(), false))
}

fn inspect(args: Args) -> Result<Value> {
  const F: &str = "inspect";
  let v = args.require(0, "value", F)?;
  Ok(str_value(v.inspect(), false))
}

fn keywords(args: Args) -> Result<Value> {
  const F: &str = "keywords";
  let v = args.require(0, "args", F)?;
  match v {
    Value::ArgList(a) => Ok(Value::Map(a.as_map())),
    _ => Err(Error::type_error(format!(
      "{}: `${}` is not an arglist.",
      F, "args"
    ))),
  }
}
