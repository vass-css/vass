//! `vass:color` module — legacy + modern CSS Color 4 spaces

use vass_core::error::{Error, Result};
use vass_core::value::color::missing_flags;
use vass_core::value::{Color, ColorSpace, ModernColor, Separator, Value};

use super::{Args, need_color, unquoted};

/// Dispatch `color.<name>`; `None` if unknown
pub fn call(name: &str, args: Args) -> Option<Result<Value>> {
  let r = match name {
    "channel" => channel_named(args),
    "space" => space_of(args),
    "same" => same(args),
    "is-legacy" => is_legacy(args),
    "is-in-gamut" => is_in_gamut(args),
    "is-missing" => is_missing(args),
    "is-powerless" => is_powerless(args),
    "to-space" => to_space(args),
    "to-gamut" => to_gamut(args),
    "hwb" => hwb_ctor(args),
    "red" => legacy_channel(args, "red"),
    "green" => legacy_channel(args, "green"),
    "blue" => legacy_channel(args, "blue"),
    "hue" => legacy_channel(args, "hue"),
    "saturation" => legacy_channel(args, "saturation"),
    "lightness" => legacy_channel(args, "lightness"),
    "alpha" | "opacity" => channel_alpha(args),
    "whiteness" => legacy_channel(args, "whiteness"),
    "blackness" => legacy_channel(args, "blackness"),
    "mix" => mix(args),
    "invert" => invert(args),
    "grayscale" => simple(args, |c| c.grayscale()),
    "complement" => simple(args, |c| c.complement()),
    "ie-hex-str" => ie_hex(args),
    "change" => change(args),
    "adjust" => adjust(args),
    "scale" => scale(args),
    _ => return None,
  };
  Some(r)
}

/// Canonical names of every member (used by `vass list modules`)
pub fn member_names() -> Vec<&'static str> {
  vec![
    "channel",
    "space",
    "same",
    "is-legacy",
    "is-in-gamut",
    "is-missing",
    "is-powerless",
    "to-space",
    "to-gamut",
    "hwb",
    "red",
    "green",
    "blue",
    "hue",
    "saturation",
    "lightness",
    "alpha",
    "opacity",
    "whiteness",
    "blackness",
    "mix",
    "invert",
    "grayscale",
    "complement",
    "ie-hex-str",
    "change",
    "adjust",
    "scale",
  ]
}

// ---- helpers ---------------------------------------------------------------

/// Whether the color lives in a modern (non-legacy) space
fn modern_of(c: &Color) -> Option<&ModernColor> {
  c.modern.as_deref()
}

/// Convert any color into `target` space (as a Color with `modern` set when
/// the target is non-legacy)
pub fn color_to_space(c: &Color, target: ColorSpace) -> Color {
  if target.is_legacy() {
    // Legacy target: convert through srgb/hsl legacy model
    return match target {
      ColorSpace::Srgb => Color::from_channels(c.r, c.g, c.b, c.a),
      ColorSpace::Hsl => {
        let (h, s, l) = c.to_hsl();
        ModernColor::new(ColorSpace::Hsl, [Some(h), Some(s), Some(l)], Some(c.a)).into_color()
      }
      _ => unreachable!(),
    };
  }
  if let Some(m) = modern_of(c) {
    // modern -> modern: via linear-srgb hub
    let native = [m.value_or_zero(0), m.value_or_zero(1), m.value_or_zero(2)];
    let lin = modern::to_srgb_linear(native, m.space);
    let out = modern::from_srgb_linear(lin, target);
    ModernColor::new(
      target,
      [Some(out[0]), Some(out[1]), Some(out[2])],
      Some(m.alpha.unwrap_or(1.0)),
    )
    .into_color()
  } else {
    // legacy -> modern
    let lin = [
      modern::srgb_to_linear(c.r / 255.0),
      modern::srgb_to_linear(c.g / 255.0),
      modern::srgb_to_linear(c.b / 255.0),
    ];
    let out = modern::from_srgb_linear(lin, target);
    ModernColor::new(
      target,
      [Some(out[0]), Some(out[1]), Some(out[2])],
      Some(c.a),
    )
    .into_color()
  }
}

use vass_core::value::modern;

/// Read one native channel (with optional conversion to a read space)
fn read_channel(
  c: &Color,
  name: &str,
  read_space: Option<ColorSpace>,
) -> Result<(f64, Option<&'static str>)> {
  let target_space =
    read_space.unwrap_or_else(|| modern_of(c).map(|m| m.space).unwrap_or(ColorSpace::Srgb));
  if name == "alpha" {
    return Ok((c.a, None));
  }
  // Validate the channel belongs to the (read) space
  let space_name_for_err = |s: ColorSpace| {
    if s == ColorSpace::Srgb {
      "rgb".to_string()
    } else {
      s.name().to_string()
    }
  };
  if !target_space.channels().contains(&name) {
    // Legacy colors have no stored space: route the channel by name
    // through the matching legacy model instead of erroring.
    let legacy_names = [
      "red",
      "green",
      "blue",
      "hue",
      "saturation",
      "lightness",
      "whiteness",
      "blackness",
    ];
    if modern_of(c).is_none() && legacy_names.contains(&name) {
      // fall through to the legacy read paths below
    } else {
      return Err(Error::type_error(format!(
        "Color space {} doesn't have a channel with this name.",
        space_name_for_err(target_space)
      )));
    }
  }
  // Convert a snapshot into the read space
  let converted = if let Some(m) = modern_of(c) {
    if m.space == target_space {
      None
    } else {
      Some(color_to_space(c, target_space))
    }
  } else if target_space != ColorSpace::Srgb {
    Some(color_to_space(c, target_space))
  } else {
    None
  };
  if let Some(m) = modern_of(converted.as_ref().unwrap_or(c)) {
    let idx = target_space
      .channels()
      .iter()
      .position(|&n| n == name)
      .unwrap();
    let raw = m.ch[idx];
    let v = target_space.channel_out(name, raw.unwrap_or(0.0));
    return Ok((v, target_space.channel_unit(name)));
  }
  // Legacy read paths
  let v = match name {
    "whiteness" => {
      let rgb = [c.r / 255.0, c.g / 255.0, c.b / 255.0];
      let (_h, w, _b) = modern::rgb_to_hwb(rgb);
      w * 100.0
    }
    "blackness" => {
      let rgb = [c.r / 255.0, c.g / 255.0, c.b / 255.0];
      let (_h, _w, b) = modern::rgb_to_hwb(rgb);
      b * 100.0
    }
    "red" => c.r,
    "green" => c.g,
    "blue" => c.b,
    "hue" => c.hue(),
    "saturation" => c.saturation() * 100.0,
    "lightness" => c.lightness() * 100.0,
    _ => unreachable!(),
  };
  let unit = match name {
    "hue" => Some("deg"),
    "saturation" | "lightness" | "whiteness" | "blackness" => Some("%"),
    _ => None,
  };
  Ok((v, unit))
}

// ---- members ---------------------------------------------------------------

fn channel_named(args: Args) -> Result<Value> {
  const F: &str = "color.channel";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let name = match args.require(1, "name", F)? {
    Value::Str(s) => s.text.to_ascii_lowercase(),
    other => {
      return Err(Error::type_error(format!(
        "{}: `$name` must be a string, got {}.",
        F,
        other.inspect()
      )));
    }
  };
  // Optional $space overrides the read space
  let read_space = match args.get(2, "space") {
    Some(Value::Str(s)) => Some(
      ColorSpace::from_name(&s.text)
        .ok_or_else(|| Error::type_error(format!("$space: Unknown color space \"{}\".", s.text)))?,
    ),
    Some(Value::Null) | None => None,
    Some(other) => {
      return Err(Error::type_error(format!(
        "{}: `$space` must be a string, got {}.",
        F,
        other.inspect()
      )));
    }
  };
  let (v, unit) = read_channel(&c, &name, read_space)?;
  Ok(Value::Number(vass_core::value::Number::new(
    v,
    unit.map(String::from),
  )))
}

/// Whether the color is legacy in the reference's terms: no modern space,
/// or the legacy hsl space (which VASS models as a modern Hsl color)
fn is_legacy_color(c: &Color) -> bool {
  match modern_of(c) {
    None => true,
    Some(m) => m.space == ColorSpace::Hsl,
  }
}

fn space_of(args: Args) -> Result<Value> {
  const F: &str = "color.space";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let name = match modern_of(&c) {
    Some(m) => m.space.name().to_string(),
    None => {
      if c.is_hsl_space() {
        "hsl".to_string()
      } else {
        "rgb".to_string()
      }
    }
  };
  Ok(Value::Str(vass_core::value::BassString::quoted(name)))
}

fn same(args: Args) -> Result<Value> {
  const F: &str = "color.same";
  let a = need_color(args.require(0, "color1", F)?, F, "color1")?;
  let b = need_color(args.require(1, "color2", F)?, F, "color2")?;
  // same() compares after converting into a common space
  let (sa, sb) = (
    modern_of(&a).map(|m| m.space).unwrap_or(ColorSpace::Srgb),
    modern_of(&b).map(|m| m.space).unwrap_or(ColorSpace::Srgb),
  );
  let eq = if sa == sb {
    match (modern_of(&a), modern_of(&b)) {
      (Some(ma), Some(mb)) => channels_eq(ma, mb) && fuzzy_eq(a.a, b.a),
      // Legacy comparison ignores missing-ness (quirky zeros), like
      // the reference's no-missing xyz conversion in color.same()
      _ => fuzzy_eq(a.r, b.r) && fuzzy_eq(a.g, b.g) && fuzzy_eq(a.b, b.b) && fuzzy_eq(a.a, b.a),
    }
  } else {
    // Convert b into a's space and compare channels there
    let b2 = color_to_space(&b, sa);
    match (modern_of(&a), modern_of(&b2)) {
      (Some(ma), Some(mb)) => channels_eq(ma, mb) && fuzzy_eq(a.a, b2.a),
      _ => fuzzy_eq(a.r, b2.r) && fuzzy_eq(a.g, b2.g) && fuzzy_eq(a.b, b2.b) && fuzzy_eq(a.a, b2.a),
    }
  };
  Ok(Value::Bool(eq))
}

fn channels_eq(a: &ModernColor, b: &ModernColor) -> bool {
  // Missing channels compare as their quirky zero (reference: same()
  // converts through a no-missing snapshot)
  for i in 0..3 {
    if !fuzzy_eq(a.ch[i].unwrap_or(0.0), b.ch[i].unwrap_or(0.0)) {
      return false;
    }
  }
  true
}

fn fuzzy_eq(x: f64, y: f64) -> bool {
  (x - y).abs() < 1e-6
}

fn is_legacy(args: Args) -> Result<Value> {
  const F: &str = "color.is-legacy";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  Ok(Value::Bool(is_legacy_color(&c)))
}

fn is_in_gamut(args: Args) -> Result<Value> {
  const F: &str = "color.is-in-gamut";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  Ok(Value::Bool(in_gamut(&c)))
}

fn in_gamut(c: &Color) -> bool {
  match modern_of(c) {
    None => {
      // Legacy hsl-space colors check their RAW channels: the
      // reference reports `color.is-in-gamut(hsl(0 50% -50%))` as
      // false because lightness -50% is out of bounds, even though
      // the rgb backing clamps into range.
      if c.is_hsl_space() {
        let (_, s, l) = c.to_hsl();
        (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&l)
      } else {
        true
      }
    }
    Some(m) => match m.space {
      ColorSpace::Hsl => {
        let s = m.value_or_zero(1);
        let l = m.value_or_zero(2);
        (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&l)
      }
      ColorSpace::Hwb => {
        let w = m.value_or_zero(1);
        let b = m.value_or_zero(2);
        w >= 0.0 && b >= 0.0 && w + b <= 1.0
      }
      ColorSpace::Lab | ColorSpace::Lch | ColorSpace::Oklab | ColorSpace::Oklch => {
        let l = m.value_or_zero(0);
        (0.0..=1.0).contains(&l)
      }
      _ => [0, 1, 2].iter().all(|&i| match m.ch[i] {
        None => true,
        Some(v) => (-1e-9..=1.0 + 1e-9).contains(&v),
      }),
    },
  }
}

fn is_missing(args: Args) -> Result<Value> {
  const F: &str = "color.is-missing";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  // The reference requires an explicit channel argument
  let channel = match args.require(1, "channel", F)? {
    Value::Str(s) => s.text.to_ascii_lowercase(),
    other => {
      return Err(Error::type_error(format!(
        "{}: `$channel` must be a string, got {}.",
        F,
        other.inspect()
      )));
    }
  };
  let missing = match modern_of(&c) {
    Some(m) => match channel.as_str() {
      "alpha" => m.alpha.is_none(),
      name => m
        .space
        .channels()
        .iter()
        .position(|&n| n == name)
        .map(|i| m.ch[i].is_none())
        .unwrap_or(false),
    },
    None => match channel.as_str() {
      "alpha" => c.missing & missing_flags::ALPHA != 0,
      "red" => c.missing & missing_flags::RED != 0,
      "green" => c.missing & missing_flags::GREEN != 0,
      "blue" => c.missing & missing_flags::BLUE != 0,
      _ => false,
    },
  };
  Ok(Value::Bool(missing))
}

fn is_powerless(args: Args) -> Result<Value> {
  const F: &str = "color.is-powerless";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let name = match args.require(1, "channel", F)? {
    Value::Str(s) => s.text.to_ascii_lowercase(),
    other => {
      return Err(Error::type_error(format!(
        "{}: `$channel` must be a string, got {}.",
        F,
        other.inspect()
      )));
    }
  };
  let powerless = {
    // The channel must belong to the color's own space; the reference
    // errors instead of answering for foreign channels (`is-powerless(
    // #f00, "hue")` errors even though legacy colors can convert).
    // Legacy hsl() colors carry the hsl-space tag (round-3 semantics)
    let space = if let Some(m) = modern_of(&c) {
      m.space
    } else if c.is_hsl_space() {
      ColorSpace::Hsl
    } else {
      ColorSpace::Srgb
    };
    if name != "alpha" && !space.channels().contains(&name.as_str()) {
      return Err(Error::type_error(format!(
        "$channel: Color {} doesn't have a channel named \"{}\".",
        c, name
      )));
    }
    match modern_of(&c) {
      Some(m) => match m.space {
        // hue is powerless when saturation is 0
        ColorSpace::Hsl if name == "hue" => fuzzy_eq(m.value_or_zero(1), 0.0),
        ColorSpace::Hwb if name == "hue" => m.value_or_zero(1) + m.value_or_zero(2) >= 1.0,
        ColorSpace::Lch | ColorSpace::Oklch if name == "hue" => fuzzy_eq(m.value_or_zero(1), 0.0),
        _ => false,
      },
      // legacy hsl: powerless hue at 0 saturation
      None if space == ColorSpace::Hsl => name == "hue" && fuzzy_eq(c.saturation(), 0.0),
      None => false,
    }
  };
  Ok(Value::Bool(powerless))
}

fn to_space(args: Args) -> Result<Value> {
  const F: &str = "color.to-space";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let target = match args.require(1, "space", F)? {
    Value::Str(s) => ColorSpace::from_name(&s.text)
      .ok_or_else(|| Error::type_error(format!("$space: Unknown color space \"{}\".", s.text)))?,
    other => {
      return Err(Error::type_error(format!(
        "{}: `$space` must be a string, got {}.",
        F,
        other.inspect()
      )));
    }
  };
  Ok(Value::Color(color_to_space(&c, target)))
}

fn to_gamut(args: Args) -> Result<Value> {
  const F: &str = "color.to-gamut";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  // $space defaults to the color's own space (reference `_spaceOrDefault`)
  let own = own_space(&c);
  let space = match args.get(1, "space") {
    None | Some(Value::Null) => own,
    Some(Value::Str(s)) => ColorSpace::from_name(&s.text)
      .ok_or_else(|| Error::type_error(format!("$space: Unknown color space \"{}\".", s.text)))?,
    Some(other) => {
      return Err(Error::type_error(format!(
        "{}: `$space` must be a string, got {}.",
        F,
        other.inspect()
      )));
    }
  };
  // The reference REQUIRES $method (forwards-compatibility with the CSS spec)
  let method = match args.get(2, "method") {
    None | Some(Value::Null) => {
      return Err(Error::type_error(
        "color.to-gamut() requires a $method argument for forwards-compatibility with changes in the CSS spec. Suggestion: $method: local-minde",
      ));
    }
    Some(Value::Str(s)) => match s.text.as_str() {
      "clip" => GamutMethod::Clip,
      "local-minde" => GamutMethod::LocalMinde,
      other => {
        return Err(Error::type_error(format!(
          "Unknown gamut map method \"{}\".",
          other
        )));
      }
    },
    Some(other) => {
      return Err(Error::type_error(format!(
        "{}: `$method` must be a string, got {}.",
        F,
        other.inspect()
      )));
    }
  };
  // Unbounded spaces (xyz) are always "in gamut"
  if !space.is_bounded() {
    return Ok(Value::Color(c));
  }
  // Snapshot the color into the target space. The same-space path
  // preserves missing channels (the reference's toSpace short-circuit);
  // conversions fill them with their quirky zero, matching the
  // reference's `legacyMissing: false` on the way back.
  let in_space = if space == own {
    snapshot_same_space_preserving(&c).unwrap_or_else(|| snapshot(&c, space))
  } else {
    snapshot(&c, space)
  };
  // Already in gamut: the reference returns the color unchanged (keeping
  // its source notation and missing channels) — but only when no space
  // conversion happened (a cross-space roundtrip fills missing channels)
  if in_gamut_work(&in_space) {
    if space == own {
      return Ok(Value::Color(c));
    }
    return Ok(Value::Color(work_to_color(&in_space, own)));
  }
  let mapped = match method {
    GamutMethod::Clip => clip_work(&in_space),
    GamutMethod::LocalMinde => local_minde_work(&in_space),
  };
  // Convert back to the color's original space
  Ok(Value::Color(work_to_color(&mapped, own)))
}

/// The space a color lives in (legacy rgb/hsl kept in their own spaces)
fn own_space(c: &Color) -> ColorSpace {
  match modern_of(c) {
    Some(m) => m.space,
    None => {
      if c.is_hsl_space() {
        ColorSpace::Hsl
      } else {
        ColorSpace::Srgb
      }
    }
  }
}

#[derive(Clone, Copy, PartialEq)]
enum GamutMethod {
  Clip,
  LocalMinde,
}

/// A color snapshot in a specific space with per-channel presence. Modern
/// rgb spaces store channels 0-1; legacy hsl stores (deg, 0-1, 0-1).
#[derive(Clone, Debug)]
struct WorkColor {
  space: ColorSpace,
  ch: [Option<f64>; 3],
  alpha: Option<f64>,
}

impl WorkColor {
  fn value(&self, i: usize) -> f64 {
    self.ch[i].unwrap_or(0.0)
  }
}

/// Snapshot `c` into `space` (missing channels become their quirky zero
/// unless the color already lives in `space`)
fn snapshot(c: &Color, space: ColorSpace) -> WorkColor {
  let alpha = Some(c.alpha());
  match modern_of(c) {
    Some(m) => {
      if m.space == space {
        return WorkColor {
          space,
          ch: m.ch,
          alpha: m.alpha,
        };
      }
      let native = [m.value_or_zero(0), m.value_or_zero(1), m.value_or_zero(2)];
      let lin = modern::to_srgb_linear(native, m.space);
      let out = modern::from_srgb_linear(lin, space);
      WorkColor {
        space,
        ch: [Some(out[0]), Some(out[1]), Some(out[2])],
        alpha,
      }
    }
    None => {
      // Legacy color (rgb-backed; hsl-tagged reads its hsl channels)
      if c.is_hsl_space() || space == ColorSpace::Hsl {
        let (h, s, l) = c.to_hsl();
        if space == ColorSpace::Hsl {
          return WorkColor {
            space,
            ch: [Some(h), Some(s), Some(l)],
            alpha,
          };
        }
        let lin = modern::to_srgb_linear([h, s, l], ColorSpace::Hsl);
        let out = modern::from_srgb_linear(lin, space);
        return WorkColor {
          space,
          ch: [Some(out[0]), Some(out[1]), Some(out[2])],
          alpha,
        };
      }
      // Legacy rgb into an rgb-family or other space
      let lin = [
        modern::srgb_to_linear(c.r / 255.0),
        modern::srgb_to_linear(c.g / 255.0),
        modern::srgb_to_linear(c.b / 255.0),
      ];
      let out = modern::from_srgb_linear(lin, space);
      WorkColor {
        space,
        ch: [Some(out[0]), Some(out[1]), Some(out[2])],
        alpha,
      }
    }
  }
}

/// Same-space snapshot that PRESERVES missing channels (the reference's
/// same-space short-circuit in `toSpace` keeps `none` channels)
fn snapshot_same_space_preserving(c: &Color) -> Option<WorkColor> {
  let space = own_space(c);
  match modern_of(c) {
    Some(m) if m.space == space => Some(WorkColor {
      space,
      ch: m.ch,
      alpha: m.alpha,
    }),
    None => {
      if c.is_hsl_space() {
        // hsl-tagged legacy colors carry their RAW hsl channels
        // (out-of-range values included) — the same-space short
        // circuit sees them unclamped, which is what makes
        // `to-gamut(hsl(0 200% 150%), $space: hsl, $method: clip)`
        // clip to `hsl(0, 100%, 100%)`
        return c.hsl_raw.as_deref().map(|ch| WorkColor {
          space,
          ch: [Some(ch[0]), Some(ch[1]), Some(ch[2])],
          alpha: Some(c.alpha()),
        });
      }
      // Legacy rgb into Srgb: keep missing-ness, scale 0-255 -> 0-1
      let ch = [
        if c.missing & missing_flags::RED != 0 {
          None
        } else {
          Some(c.r / 255.0)
        },
        if c.missing & missing_flags::GREEN != 0 {
          None
        } else {
          Some(c.g / 255.0)
        },
        if c.missing & missing_flags::BLUE != 0 {
          None
        } else {
          Some(c.b / 255.0)
        },
      ];
      Some(WorkColor {
        space,
        ch,
        alpha: if c.missing & missing_flags::ALPHA != 0 {
          None
        } else {
          Some(c.a)
        },
      })
    }
    _ => None,
  }
}

/// Build the result color in `space` and convert back to `own`
fn work_to_color(w: &WorkColor, own: ColorSpace) -> Color {
  let target = if w.space == own {
    w.clone()
  } else {
    let native = [w.value(0), w.value(1), w.value(2)];
    let lin = modern::to_srgb_linear(native, w.space);
    let out = modern::from_srgb_linear(lin, own);
    WorkColor {
      space: own,
      ch: [Some(out[0]), Some(out[1]), Some(out[2])],
      alpha: w.alpha,
    }
  };
  match target.space {
    ColorSpace::Srgb => {
      // Legacy rgb result; channels scale back to 0-255
      let ch = [
        target.ch[0].map(|v| v * 255.0),
        target.ch[1].map(|v| v * 255.0),
        target.ch[2].map(|v| v * 255.0),
      ];
      Color::from_legacy_channels(ch, target.alpha, false)
    }
    ColorSpace::Hsl => {
      let mc = ModernColor::new(ColorSpace::Hsl, target.ch, target.alpha);
      mc.into_color()
    }
    other => {
      let mc = ModernColor::new(other, target.ch, target.alpha);
      mc.into_color()
    }
  }
}

/// Clamp a snapshot's bounded channels into its space's gamut
fn clip_work(w: &WorkColor) -> WorkColor {
  let mut ch = w.ch;
  for (i, slot) in ch.iter_mut().enumerate() {
    if let (Some(v), Some((min, max))) = (*slot, w.space.channel_bounds(i)) {
      *slot = Some(v.clamp(min, max));
    }
  }
  WorkColor {
    space: w.space,
    ch,
    alpha: w.alpha,
  }
}

/// Whether the snapshot is inside its own space's gamut
fn in_gamut_work(w: &WorkColor) -> bool {
  for i in 0..3 {
    if let (Some(v), Some((min, max))) = (w.ch[i], w.space.channel_bounds(i))
      && (v < min - 1e-9 || v > max + 1e-9)
    {
      return false;
    }
  }
  true
}

/// Convert a snapshot into another space through the linear-sRGB hub
fn convert_work(w: &WorkColor, target: ColorSpace) -> WorkColor {
  if w.space == target {
    return w.clone();
  }
  let native = [w.value(0), w.value(1), w.value(2)];
  let lin = modern::to_srgb_linear(native, w.space);
  let out = modern::from_srgb_linear(lin, target);
  WorkColor {
    space: target,
    ch: [Some(out[0]), Some(out[1]), Some(out[2])],
    alpha: w.alpha,
  }
}

/// The snapshot's channels in oklch (L 0-1, chroma, hue degrees)
fn to_oklch_work(w: &WorkColor) -> [f64; 3] {
  let native = [w.value(0), w.value(1), w.value(2)];
  let lin = modern::to_srgb_linear(native, w.space);
  modern::from_srgb_linear(lin, ColorSpace::Oklch)
}

/// The snapshot's channels in oklab (L 0-1, a, b)
fn to_oklab_work(w: &WorkColor) -> [f64; 3] {
  let native = [w.value(0), w.value(1), w.value(2)];
  let lin = modern::to_srgb_linear(native, w.space);
  modern::from_srgb_linear(lin, ColorSpace::Oklab)
}

/// The ΔEOK color difference between two snapshots (CSS Color 4 formula)
fn delta_eok_work(a: &WorkColor, b: &WorkColor) -> f64 {
  let la = to_oklab_work(a);
  let lb = to_oklab_work(b);
  ((la[0] - lb[0]).powi(2) + (la[1] - lb[1]).powi(2) + (la[2] - lb[2]).powi(2)).sqrt()
}

/// The CSS Color 4 local-MINDE gamut mapping algorithm (ported from the
/// pinned reference implementation: JND 0.02, epsilon 0.0001)
fn local_minde_work(m: &WorkColor) -> WorkColor {
  const JND: f64 = 0.02;
  const EPSILON: f64 = 0.0001;
  const FUZZY: f64 = 1e-11;

  let origin_oklch = to_oklch_work(m);
  let lightness = origin_oklch[0];
  let hue = m.ch[2]
    .filter(|_| {
      matches!(
        m.space,
        ColorSpace::Lch | ColorSpace::Oklch | ColorSpace::Hsl | ColorSpace::Hwb
      )
    })
    .or(Some(origin_oklch[2]));
  let alpha = m.alpha;

  if lightness >= 1.0 - FUZZY {
    // White, expressed in the mapping space (legacy colors go through
    // rgb(255,255,255); modern spaces set the raw channels to 1)
    return if is_legacy_space(m.space) {
      convert_work(
        &WorkColor {
          space: ColorSpace::Srgb,
          ch: [Some(1.0); 3],
          alpha,
        },
        m.space,
      )
    } else {
      WorkColor {
        space: m.space,
        ch: [Some(1.0); 3],
        alpha,
      }
    };
  }
  if lightness <= 0.0 + FUZZY {
    return convert_work(
      &WorkColor {
        space: ColorSpace::Srgb,
        ch: [Some(0.0); 3],
        alpha,
      },
      m.space,
    );
  }

  let clipped = clip_work(m);
  if delta_eok_work(&clipped, m) < JND {
    return clipped;
  }

  let mut min = 0.0f64;
  let mut max = origin_oklch[1];
  let mut min_in_gamut = true;
  let mut clipped = clipped;
  while max - min > EPSILON {
    let chroma = (min + max) / 2.0;
    let current = convert_work(
      &WorkColor {
        space: ColorSpace::Oklch,
        ch: [Some(lightness), Some(chroma), hue],
        alpha,
      },
      m.space,
    );
    if min_in_gamut && in_gamut_work(&current) {
      min = chroma;
      continue;
    }
    clipped = clip_work(&current);
    let e = delta_eok_work(&clipped, &current);
    if e < JND {
      if JND - e < EPSILON {
        return clipped;
      }
      min_in_gamut = false;
      min = chroma;
    } else {
      max = chroma;
    }
  }
  clipped
}

fn is_legacy_space(s: ColorSpace) -> bool {
  matches!(s, ColorSpace::Srgb | ColorSpace::Hsl)
}

/// `color.hwb(h w% b%)` constructor (also reachable through legacy hwb())
fn hwb_ctor(args: Args) -> Result<Value> {
  const F: &str = "color.hwb";
  let mut vals: Vec<Value> = Vec::new();
  for v in &args.positional {
    if let Value::List(l) = v
      && l.sep == Separator::Space
    {
      vals.extend(l.items.iter().cloned());
      continue;
    }
    vals.push((*v).clone());
  }
  if vals.is_empty() {
    return Err(Error::type_error(format!(
      "{}: at least 1 argument required.",
      F
    )));
  }
  Ok(Value::Color(hwb_channels(&vals)?))
}

fn hwb_channels(vals: &[Value]) -> Result<Color> {
  let mut ch: [Option<f64>; 3] = [None; 3];
  let mut alpha = 1.0f64;
  let mut ci = 0usize;
  for v in vals {
    match v {
      Value::Number(n) => {
        let val = match n.unit.as_deref() {
          Some("%") => n.value / 100.0,
          Some("deg") | None => n.value,
          _ => n.value,
        };
        if ci < 3 {
          ch[ci] = Some(val);
          ci += 1;
        } else {
          alpha = val;
        }
      }
      Value::Str(s) if !s.quoted && s.text == "none" => {
        if ci < 3 {
          ch[ci] = None;
          ci += 1;
        }
      }
      other => {
        return Err(Error::type_error(format!(
          "color.hwb: invalid channel {}.",
          other.inspect()
        )));
      }
    }
  }
  // The reference normalizes whiteness/blackness when their sum exceeds 1
  // (`hwb(0 150% 150%)` is stored as `hwb(0 50% 50%)`)
  let (w, b) = (ch[1].unwrap_or(0.0), ch[2].unwrap_or(0.0));
  if w + b > 1.0 {
    let sum = w + b;
    ch[1] = ch[1].map(|v| v / sum);
    ch[2] = ch[2].map(|v| v / sum);
  }
  Ok(Color::from_hwb_channels(ch, alpha.clamp(0.0, 1.0)))
}

/// Legacy channel getters reject modern colors, per the reference
/// (the legacy hsl space still accepts them)
fn legacy_channel(args: Args, name: &str) -> Result<Value> {
  const F: &str = "color";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  if let Some(m) = modern_of(&c)
    && m.space != ColorSpace::Hsl
  {
    return Err(Error::type_error(format!(
      "color.{}() is only supported for legacy colors. Please use color.channel() instead with an explicit $space argument.",
      name
    )));
  }
  let (v, unit) = match name {
    "red" => (c.r, None),
    "green" => (c.g, None),
    "blue" => (c.b, None),
    "hue" => (c.hue(), Some("deg")),
    "saturation" => (c.saturation() * 100.0, Some("%")),
    "lightness" => (c.lightness() * 100.0, Some("%")),
    "whiteness" | "blackness" => (0.0, Some("%")),
    _ => unreachable!(),
  };
  Ok(Value::Number(vass_core::value::Number::new(
    v,
    unit.map(String::from),
  )))
}

fn channel_alpha(args: Args) -> Result<Value> {
  const F: &str = "color";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let a = match modern_of(&c) {
    Some(m) => m.alpha.unwrap_or(0.0),
    None => c.a,
  };
  let _ = F;
  Ok(Value::Number(vass_core::value::Number::new(a, None)))
}

fn simple(args: Args, f: impl Fn(&Color) -> Color) -> Result<Value> {
  const F: &str = "color";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  Ok(Value::Color(f(&c)))
}

fn mix(args: Args) -> Result<Value> {
  const F: &str = "color.mix";
  let c1 = need_color(args.require(0, "color1", F)?, F, "color1")?;
  let c2 = args
    .require(1, "color2", F)
    .and_then(|v| need_color(v, F, "color2"))?;
  let weight = match args.get(2, "weight") {
    Some(v) => super::number_scaled(v, F, "weight")?,
    None => 0.5,
  };
  let method = match args.get_named("method") {
    Some(Value::Str(s)) => Some(s.text.to_ascii_lowercase()),
    _ => None,
  };
  // hsl is a LEGACY space in the reference: mixing hsl colors never
  // requires a $method (only truly modern spaces do)
  let modern_involved = modern_of(&c1).is_some_and(|m| m.space != ColorSpace::Hsl)
    || modern_of(&c2).is_some_and(|m| m.space != ColorSpace::Hsl);
  if modern_involved || method.is_some() {
    let method = method.ok_or_else(|| {
      Error::type_error(format!(
        "$color1: To use color.mix() with non-legacy color {}, you must provide a $method.",
        c1.to_css()
      ))
    })?;
    let space = match method.as_str() {
      "oklch" | "oklab" | "lab" | "lch" | "hsl" => ColorSpace::from_name(&method).unwrap(),
      other => {
        return Err(Error::type_error(format!(
          "$method: Unknown mix method \"{}\".",
          other
        )));
      }
    };
    let a = color_to_space(&c1, space);
    let b = color_to_space(&c2, space);
    let (ma, mb) = (
      modern_of(&a).unwrap().clone(),
      modern_of(&b).unwrap().clone(),
    );
    // number_scaled already yields a 0-1 fraction
    let w = weight;
    let mut out = [Some(0.0); 3];
    for (i, cname) in space.channels().iter().enumerate() {
      let (av, bv) = match (ma.ch[i], mb.ch[i]) {
        (Some(x), Some(y)) => (x, y),
        (Some(x), None) | (None, Some(x)) => (x, x),
        (None, None) => (0.0, 0.0),
      };
      // Hue channels interpolate along the shorter circular arc
      let value = if *cname == "hue" {
        let d = ((bv - av + 540.0).rem_euclid(360.0)) - 180.0;
        (av + d * (1.0 - w)).rem_euclid(360.0)
      } else {
        av * w + bv * (1.0 - w)
      };
      out[i] = Some(value);
    }
    let alpha = ma.alpha.unwrap_or(0.0) * w + mb.alpha.unwrap_or(0.0) * (1.0 - w);
    // The reference converts the mixed color back to legacy rgb
    // (out-of-gamut channels serialize as raw percentages, unclamped)
    let native = [
      out[0].unwrap_or(0.0),
      out[1].unwrap_or(0.0),
      out[2].unwrap_or(0.0),
    ];
    let lin = modern::to_srgb_linear(native, space);
    let srgb = [
      modern::linear_to_srgb(lin[0]) * 255.0,
      modern::linear_to_srgb(lin[1]) * 255.0,
      modern::linear_to_srgb(lin[2]) * 255.0,
    ];
    return Ok(Value::Color(Color::from_channels(
      srgb[0], srgb[1], srgb[2], alpha,
    )));
  }
  Ok(Value::Color(c1.mix(&c2, weight)))
}

fn invert(args: Args) -> Result<Value> {
  const F: &str = "color.invert";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let weight = match args.get(1, "weight") {
    Some(v) => super::number_scaled(v, F, "weight")?,
    None => 1.0,
  };
  // The reference refuses to invert colors with missing channels
  if let Some(ch) = c.first_missing_channel()
    && ch != "alpha"
  {
    return Err(Error::type_error(format!(
      "${}: Because the CSS working group is still deciding on the best behavior, Sass doesn't currently support modifying missing channels (color: {}).",
      ch,
      c.to_debug_string()
    )));
  }
  Ok(Value::Color(c.invert(weight)))
}

fn ie_hex(args: Args) -> Result<Value> {
  const F: &str = "color.ie-hex-str";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  Ok(unquoted(format!(
    "#{:02X}{:02X}{:02X}{:02X}",
    c.alpha_u8(),
    c.r.round() as u8,
    c.g.round() as u8,
    c.b.round() as u8
  )))
}

/// A named channel update: (channel name, new value or `None` when the
/// argument was `none`, original unit)
pub type ChannelUpdate = (String, Option<f64>, Option<String>);

/// Resolve a named-channel update from a named argument, validating that the
/// channel exists in the color's space (reference errors on unknown channels).
/// `none` removes the channel (missing); the value is `None` then.
fn named_channel_update(c: &Color, args: &Args, fname: &str) -> Result<Option<ChannelUpdate>> {
  for (name, v) in &args.named {
    if name == "alpha" {
      continue;
    }
    let known = match modern_of(c) {
      Some(m) => m.space.channels().contains(&name.as_str()),
      None => matches!(
        name.as_str(),
        "red" | "green" | "blue" | "hue" | "saturation" | "lightness" | "whiteness" | "blackness"
      ),
    };
    if !known {
      let space_name = match modern_of(c) {
        Some(m) => m.space.name().to_string(),
        None => {
          if c.is_hsl_space() {
            "hsl".to_string()
          } else {
            "rgb".to_string()
          }
        }
      };
      let display = if space_name == "srgb" {
        "rgb".to_string()
      } else {
        space_name
      };
      return Err(Error::type_error(format!(
        "${}: Color space {} doesn't have a channel with this name.",
        name, display
      )));
    }
    // `none` marks the channel as missing
    if let Value::Str(s) = v
      && !s.quoted
      && s.text == "none"
    {
      return Ok(Some((name.clone(), None, None)));
    }
    let n = v
      .as_number()
      .ok_or_else(|| Error::type_error(format!("{}: ${} must be a number.", fname, name)))?;
    // Callers interpret the raw value + unit (change() sets a channel,
    // adjust() adds a delta; % scales into stored 0-1 ranges there).
    return Ok(Some((name.clone(), Some(n.value), n.unit.clone())));
  }
  Ok(None)
}

/// The reference refuses to modify a missing channel (CSS WG undecided)
fn reject_missing_channel(c: &Color, channel: &str) -> Result<()> {
  let missing = match modern_of(c) {
    Some(m) => match channel {
      "alpha" => m.alpha.is_none(),
      name => m
        .space
        .channels()
        .iter()
        .position(|&n| n == name)
        .map(|i| m.ch[i].is_none())
        .unwrap_or(false),
    },
    None => match channel {
      "alpha" => c.missing & missing_flags::ALPHA != 0,
      "red" => c.missing & missing_flags::RED != 0,
      "green" => c.missing & missing_flags::GREEN != 0,
      "blue" => c.missing & missing_flags::BLUE != 0,
      _ => false,
    },
  };
  if missing {
    Err(Error::type_error(format!(
      "${}: Because the CSS working group is still deciding on the best behavior, Sass doesn't currently support modifying missing channels (color: {}).",
      channel,
      c.to_debug_string()
    )))
  } else {
    Ok(())
  }
}

fn change(args: Args) -> Result<Value> {
  const F: &str = "color.change";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let alpha_named = args
    .named
    .iter()
    .find(|(name, _)| name == "alpha")
    .map(|(_, v)| (*v).clone());
  // $alpha: none marks alpha as missing
  let alpha_none = matches!(&alpha_named, Some(Value::Str(s)) if !s.quoted && s.text == "none");
  let alpha = match &alpha_named {
    Some(v) if !alpha_none => v.as_number().map(|x| x.value),
    _ => None,
  };
  if let Some((name, val, unit)) = named_channel_update(&c, &args, F)? {
    match modern_of(&c) {
      Some(m) => {
        let mut ch = m.ch;
        let idx = m.space.channels().iter().position(|&n| n == name).unwrap();
        match val {
          None => {
            // `none` removes the channel
            ch[idx] = None;
          }
          Some(val) => {
            // % sets the channel on the human scale; store 0-1
            let stored = if unit.as_deref() == Some("%") {
              val / 100.0
            } else {
              val
            };
            ch[idx] = Some(stored);
          }
        }
        let new_alpha = if alpha_none {
          None
        } else {
          Some(alpha.unwrap_or(m.alpha.unwrap_or(1.0)))
        };
        return Ok(Value::Color(
          ModernColor::new(m.space, ch, new_alpha).into_color(),
        ));
      }
      None => {
        return match name.as_str() {
          "red" | "green" | "blue" => {
            let mut missing = c.missing;
            let bit = match name.as_str() {
              "red" => missing_flags::RED,
              "green" => missing_flags::GREEN,
              _ => missing_flags::BLUE,
            };
            let mut r = c.r;
            let mut g = c.g;
            let mut b = c.b;
            match val {
              None => {
                missing |= bit;
                match name.as_str() {
                  "red" => r = 0.0,
                  "green" => g = 0.0,
                  _ => b = 0.0,
                }
              }
              Some(val) => {
                missing &= !bit;
                match name.as_str() {
                  "red" => r = val,
                  "green" => g = val,
                  _ => b = val,
                }
              }
            }
            let new_a = if alpha_none {
              None
            } else {
              Some(alpha.unwrap_or(c.a))
            };
            let mut out = Color::from_legacy_channels([Some(r), Some(g), Some(b)], new_a, false);
            out.missing = missing;
            if c.is_hsl_space() {
              out.missing |= missing_flags::HSL_SPACE;
            }
            Ok(Value::Color(out))
          }
          "hue" | "saturation" | "lightness" => {
            // Legacy hsl change() always interprets s/l on the
            // percentage scale (unitless included, reference
            // behavior)
            let (h, s, l) = c.to_hsl();
            let h2 = match (name.as_str(), val) {
              ("hue", None) => None,
              ("hue", Some(v)) => Some(v),
              _ => Some(h),
            };
            let s2 = match (name.as_str(), val) {
              ("saturation", None) => None,
              ("saturation", Some(v)) => Some(v / 100.0),
              _ => Some(s),
            };
            let l2 = match (name.as_str(), val) {
              ("lightness", None) => None,
              ("lightness", Some(v)) => Some(v / 100.0),
              _ => Some(l),
            };
            if h2.is_none() || s2.is_none() || l2.is_none() {
              // Missing hsl channels: build an hsl-space color
              let mc = ModernColor::new(
                ColorSpace::Hsl,
                [h2.map(|v| v.rem_euclid(360.0)), s2, l2],
                if alpha_none {
                  None
                } else {
                  Some(alpha.unwrap_or(c.a))
                },
              );
              return Ok(Value::Color(mc.into_color()));
            }
            // Reference construction rules for legacy hsl
            // channels (LinearChannel): hue wraps, saturation is
            // lower-clamped at 0 only, lightness stays raw. The
            // hsl-space membership carries through
            // `from_hsl_raw` for hsl colors; rgb colors keep the
            // clamped rgb-space result.
            let out = if c.is_hsl_space() {
              Color::from_hsl_raw(
                h2.unwrap_or(0.0),
                s2.unwrap_or(0.0),
                l2.unwrap_or(0.0),
                if alpha_none {
                  0.0
                } else {
                  alpha.unwrap_or(c.a)
                },
              )
            } else {
              Color::from_hsl(
                h2.unwrap_or(0.0),
                s2.unwrap_or(0.0),
                l2.unwrap_or(0.0),
                if alpha_none {
                  0.0
                } else {
                  alpha.unwrap_or(c.a)
                },
              )
            };
            let mut out = out;
            if let Some(a) = alpha {
              out.a = a.clamp(0.0, 1.0);
            }
            Ok(Value::Color(out))
          }
          _ => Ok(Value::Color(c)),
        };
      }
    }
  }
  if alpha_none {
    return Ok(Value::Color(match modern_of(&c) {
      Some(m) => ModernColor::new(m.space, m.ch, None).into_color(),
      None => {
        Color::from_legacy_channels([Some(c.r), Some(c.g), Some(c.b)], None, c.is_hsl_space())
      }
    }));
  }
  if let Some(a) = alpha {
    let a = a.clamp(0.0, 1.0);
    return Ok(Value::Color(match modern_of(&c) {
      Some(m) => ModernColor::new(m.space, m.ch, Some(a)).into_color(),
      None => {
        // Alpha-only change: missing-ness of the other channels is
        // preserved (the reference keeps `rgb(none 255 0 / 0.5)`)
        let mut out =
          Color::from_legacy_channels([Some(c.r), Some(c.g), Some(c.b)], Some(a), c.is_hsl_space());
        out.missing = c.missing & !missing_flags::ALPHA;
        out
      }
    }));
  }
  Ok(Value::Color(c))
}

fn adjust(args: Args) -> Result<Value> {
  const F: &str = "color.adjust";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  let alpha_delta = args
    .named
    .iter()
    .find(|(name, _)| name == "alpha")
    .and_then(|(_, v)| v.as_number())
    .map(|x| x.value);
  // The reference errors when the channel being modified is missing
  if let Some((name, _, _)) = named_channel_update(&c, &args, F)? {
    reject_missing_channel(&c, &name)?;
  }
  if let Some((name, val, unit)) = named_channel_update(&c, &args, F)? {
    // adjust() deltas must be numbers (`none` has no delta meaning)
    let val = val
      .ok_or_else(|| Error::type_error(format!("{}: ${} must be a number, got none.", F, name)))?;
    match modern_of(&c) {
      Some(m) => {
        let mut ch = m.ch;
        let idx = m.space.channels().iter().position(|&n| n == name).unwrap();
        // hue is additive modulo 360; % deltas scale into 0-1
        let base = m.ch[idx].unwrap_or(0.0);
        let delta = if unit.as_deref() == Some("%") {
          val / 100.0
        } else {
          val
        };
        let mut nv = base + delta;
        if name == "hue" {
          nv = nv.rem_euclid(360.0);
        }
        ch[idx] = Some(nv);
        let alpha = match (alpha_delta, m.alpha) {
          (Some(d), Some(a)) => Some((a + d).clamp(0.0, 1.0)),
          (Some(d), None) => Some(d.clamp(0.0, 1.0)),
          (None, a) => a,
        };
        return Ok(Value::Color(
          ModernColor::new(m.space, ch, alpha).into_color(),
        ));
      }
      None => {
        let _ = &unit;
        return match name.as_str() {
          "red" | "green" | "blue" => {
            let dr = if name == "red" { Some(val) } else { None };
            let dg = if name == "green" { Some(val) } else { None };
            let db = if name == "blue" { Some(val) } else { None };
            let adj =
              |base: f64, d: Option<f64>| -> f64 { (base + d.unwrap_or(0.0)).clamp(0.0, 255.0) };
            let mut out = Color::from_legacy_channels(
              [Some(adj(c.r, dr)), Some(adj(c.g, dg)), Some(adj(c.b, db))],
              Some((c.a + alpha_delta.unwrap_or(0.0)).clamp(0.0, 1.0)),
              false,
            );
            out.missing = c.missing;
            Ok(Value::Color(out))
          }
          // Legacy adjust() adds deltas on the human scale
          // ($lightness: 20% adds 20 percentage points).
          // Reference construction rules apply per channel:
          // saturation lower-clamps at 0 (never above), lightness
          // stays raw — observed: `color.adjust(hsl(0 50% 50%),
          // $lightness: 100%)` is `hsl(0, 50%, 150%)`.
          "hue" | "saturation" | "lightness" => {
            let (h, s, l) = c.to_hsl();
            let dh = if name == "hue" { Some(val) } else { None };
            let ds = if name == "saturation" {
              Some(val)
            } else {
              None
            };
            let dl = if name == "lightness" { Some(val) } else { None };
            let hue2 = (h + dh.unwrap_or(0.0)).rem_euclid(360.0);
            let sat2 = (s * 100.0 + ds.unwrap_or(0.0)) / 100.0;
            let light2 = (l * 100.0 + dl.unwrap_or(0.0)) / 100.0;
            let alpha2 = (c.a + alpha_delta.unwrap_or(0.0)).clamp(0.0, 1.0);
            let out = if c.is_hsl_space() {
              // hsl-space colors keep raw channels: saturation
              // lower-clamped only (from_hsl_raw), lightness
              // unclamped
              Color::from_hsl_raw(hue2, sat2, light2, alpha2)
            } else {
              // rgb colors convert through hsl and back; the
              // rgb result clamps naturally
              Color::from_hsl(
                hue2,
                (sat2 * 100.0).clamp(0.0, 100.0) / 100.0,
                (light2 * 100.0).clamp(0.0, 100.0) / 100.0,
                alpha2,
              )
            };
            Ok(Value::Color(out))
          }
          _ => Ok(Value::Color(c)),
        };
      }
    }
  }
  if let Some(d) = alpha_delta {
    let a = (c.a + d).clamp(0.0, 1.0);
    return Ok(Value::Color(match modern_of(&c) {
      Some(m) => ModernColor::new(m.space, m.ch, Some(a)).into_color(),
      None => {
        let mut out =
          Color::from_legacy_channels([Some(c.r), Some(c.g), Some(c.b)], Some(a), c.is_hsl_space());
        out.missing = c.missing & !missing_flags::ALPHA;
        out
      }
    }));
  }
  Ok(Value::Color(c))
}

fn scale(args: Args) -> Result<Value> {
  const F: &str = "color.scale";
  let c = need_color(args.require(0, "color", F)?, F, "color")?;
  // Reference `_scaleChannel`: positive factors scale toward `max` and
  // short-circuit when the base is already >= max; negative factors scale
  // toward `min` with the symmetric short-circuit (`lab` a/b have min
  // -125, so they CAN scale below zero).
  let scale = |base: f64, pct: Option<f64>, min: f64, max: f64| -> f64 {
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
  let get = |n: &str| {
    args
      .named
      .iter()
      .find(|(name, _)| name == n)
      .and_then(|(_, v)| v.as_number())
      .map(|x| x.value)
  };
  // Modern colors scale native channels by percent toward the bound
  if let Some(m) = modern_of(&c) {
    let mut ch = m.ch;
    for (i, cname) in m.space.channels().iter().enumerate() {
      if let Some(p) = get(cname) {
        // The reference errors when a scaled channel is missing
        if ch[i].is_none() {
          return Err(Error::type_error(format!(
            "${}: Because the CSS working group is still deciding on the best behavior, Sass doesn't currently support modifying missing channels (color: {}).",
            cname,
            c.to_debug_string()
          )));
        }
        let base = ch[i].unwrap_or(0.0);
        // Channel scale bounds follow the reference: lightness 0-100%
        // (stored 0-1), lab a/b +-125, oklab a/b +-0.4; hue is not
        // scalable. Percentage channels are scaled on the reference's
        // raw 0-100 scale so the arithmetic is bit-identical.
        if *cname == "lightness"
          || *cname == "saturation"
          || *cname == "whiteness"
          || *cname == "blackness"
        {
          let base100 = base * 100.0;
          let scaled = scale(base100, Some(p), 0.0, 100.0);
          ch[i] = Some((scaled / 100.0).clamp(0.0, 1.0));
        } else {
          let (min, max) = match (m.space, *cname) {
            (ColorSpace::Lab, "a") | (ColorSpace::Lab, "b") => (-125.0, 125.0),
            (ColorSpace::Oklab, "a") | (ColorSpace::Oklab, "b") => (-0.4, 0.4),
            _ => (0.0, 1.0),
          };
          ch[i] = Some(scale(base, Some(p), min, max).clamp(min, max));
        }
      }
    }
    let alpha = scale(m.alpha.unwrap_or(1.0) * 100.0, get("alpha"), 0.0, 100.0) / 100.0;
    return Ok(Value::Color(
      ModernColor::new(m.space, ch, Some(alpha.clamp(0.0, 1.0))).into_color(),
    ));
  }
  let sl = get("lightness");
  let ss = get("saturation");
  if sl.is_some() || ss.is_some() {
    let (h, s, l) = c.to_hsl();
    // Reference `_scaleChannel`: positive factors scale toward `max` and
    // SHORT-CIRCUIT when the base is already >= max; negative factors
    // scale toward `min` with the symmetric short-circuit. An
    // out-of-range base (`hsl(0 200% 150%)`) therefore stays untouched.
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
    let a = scale_ch(c.a * 100.0, get("alpha"), 0.0, 100.0) / 100.0;
    let out = if c.is_hsl_space() {
      Color::from_hsl_raw(h, s2, l2, a.clamp(0.0, 1.0))
    } else {
      Color::from_hsl(h, s2, l2, a.clamp(0.0, 1.0))
    };
    return Ok(Value::Color(out));
  }
  let mut out = Color::from_channels(
    scale(c.r, get("red"), 0.0, 255.0).clamp(0.0, 255.0),
    scale(c.g, get("green"), 0.0, 255.0).clamp(0.0, 255.0),
    scale(c.b, get("blue"), 0.0, 255.0).clamp(0.0, 255.0),
    (scale(c.a * 100.0, get("alpha"), 0.0, 100.0) / 100.0).clamp(0.0, 1.0),
  );
  out.missing = c.missing;
  Ok(Value::Color(out))
}
