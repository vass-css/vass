//! Color values with a full RGBA/HSL model
//!
//! Channels are stored as `f64` in the 0-255 range (possibly fractional:
//! HSL-derived math yields e.g. 76.5). Every color also carries an optional
//! *source representation*: hex literals, named keywords, and function-form
//! colors (`rgb(255, 0, 136)`) remember the exact text they were written
//! with and re-emit it verbatim, matching dart-sass notation preservation.
//! Computed colors serialize per the reference rules: named match first,
//! then hex (all-integer channels), then `rgb(r%, g%, b%)` percentages.

pub mod modern;

pub use modern::{ColorSpace, ModernColor};

use std::fmt;

use crate::error::Error;
use crate::value::number::format_f64;

/// A VASS color: fractional RGB channels (0-255) plus float alpha (0.0-1.0)
///
/// Legacy colors may also carry *missing channels* (written `none` in the
/// modern space-separated syntax, e.g. `rgb(none 255 0)`). A missing channel
/// is stored as `0` (the "quirky" read value the reference compiler uses)
/// with its missing-ness tracked in `missing` so serialization and
/// `color.is-missing()` can distinguish `none` from a real zero.
#[derive(Debug, Clone)]
pub struct Color {
  /// Red channel (0-255, may be fractional; `0` when missing)
  pub r: f64,
  /// Green channel (0-255, may be fractional; `0` when missing)
  pub g: f64,
  /// Blue channel (0-255, may be fractional; `0` when missing)
  pub b: f64,
  /// Alpha (0.0-1.0; 1.0 = opaque; `0` when missing)
  pub a: f64,
  /// Source text to preserve verbatim (hex literal, keyword, or function
  /// form). `None` for colors produced by computation.
  pub literal: Option<Box<str>>,
  /// Native-space channels for modern CSS Color 4 colors (oklch, lab,
  /// color(...) etc.). `None` for legacy rgb/hsl colors.
  pub modern: Option<Box<ModernColor>>,
  /// Missing-channel bitmask for legacy colors: bit0 red, bit1 green,
  /// bit2 blue, bit3 alpha. Bit4 tags colors created through the legacy
  /// `hsl()` space so `color.space()` reports `hsl` and computed results
  /// serialize back in hsl notation (the reference keeps hsl-ness).
  pub missing: u8,
  /// Raw legacy-hsl channels `(hue degrees 0-360, saturation >= 0,
  /// lightness)` kept unclamped exactly as the reference does: the
  /// reference's `LinearChannel` definitions lower-clamp only saturation
  /// (`lowerClamped: true`) and never clamp lightness at construction, so
  /// `hsl(0 200% 150%)` keeps `200%`/`150%` for `color.channel()`,
  /// equality, `to-gamut` gamut checks and arithmetic. The rgb backing is
  /// still derived from the clamped values. `None` for rgb-space legacy
  /// colors and for colors whose hsl channels were derived from rgb.
  pub hsl_raw: Option<Box<[f64; 3]>>,
}

/// Bit flags for [`Color::missing`]
pub mod missing_flags {
  /// Legacy red channel is missing
  pub const RED: u8 = 1;
  /// Legacy green channel is missing
  pub const GREEN: u8 = 2;
  /// Legacy blue channel is missing
  pub const BLUE: u8 = 4;
  /// Legacy alpha channel is missing
  pub const ALPHA: u8 = 8;
  /// The color belongs to the legacy `hsl` space (notation tag)
  pub const HSL_SPACE: u8 = 16;
}

impl PartialEq for Color {
  fn eq(&self, other: &Self) -> bool {
    // Two legacy hsl-space colors compare their hsl CHANNELS exactly
    // (the reference's `_semiEquals` compares same-space channels with
    // `==`, no fuzz): `hsl(0 200% 150%) == hsl(0 100% 100%)` is false
    // even though both clamp to the same rgb backing.
    if self.is_hsl_space() && other.is_hsl_space() {
      let (h1, s1, l1) = self.to_hsl();
      let (h2, s2, l2) = other.to_hsl();
      return h1 == h2
        && s1 == s2
        && l1 == l2
        && self.a == other.a
        && self.missing & !missing_flags::HSL_SPACE == other.missing & !missing_flags::HSL_SPACE;
    }
    // Compare quantized channels so float noise does not break equality.
    // Missing-ness participates: `rgb(none 255 0) != rgb(0 255 0)` in the
    // reference even though the quirky read values match.
    self.missing == other.missing
      && self.r == other.r
      && self.g == other.g
      && self.b == other.b
      && self.a == other.a
  }
}

impl Eq for Color {}

impl Color {
  /// Create an opaque color from channel bytes (computed representation)
  pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
      hsl_raw: None,
      modern: None,
      r: r as f64,
      g: g as f64,
      b: b as f64,
      a: 1.0,
      literal: None,
      missing: 0,
    }
  }

  /// Create a color from channel bytes with alpha (computed representation)
  pub const fn rgba(r: u8, g: u8, b: u8, a: f64) -> Color {
    Color {
      hsl_raw: None,
      modern: None,
      r: r as f64,
      g: g as f64,
      b: b as f64,
      a,
      literal: None,
      missing: 0,
    }
  }

  /// Create a color from fractional channels (computed representation)
  ///
  /// Channels are fuzzy-normalized the way the reference compiler treats
  /// them: values within `1e-8` of an integer (floating-point noise from
  /// HSL/RGB conversions) snap to the integer, so `adjust(red, $lightness:
  /// 20%)` serializes as `#ff6666`, not `rgb(100%, 40%, 40%)`.
  pub fn from_channels(r: f64, g: f64, b: f64, a: f64) -> Color {
    Color {
      hsl_raw: None,
      modern: None,
      r: fuzz_channel(r),
      g: fuzz_channel(g),
      b: fuzz_channel(b),
      a: clamp01(a),
      literal: None,
      missing: 0,
    }
  }

  /// Create a legacy color with possibly-missing channels (`rgb(none 255 0)`).
  ///
  /// Missing channels read as `0` (reference "quirky" semantics) but are
  /// remembered for serialization, `color.is-missing()` and equality.
  /// `hsl_space` tags the color as living in the legacy hsl space.
  pub fn from_legacy_channels(ch: [Option<f64>; 3], alpha: Option<f64>, hsl_space: bool) -> Color {
    let mut missing = 0u8;
    if ch[0].is_none() {
      missing |= missing_flags::RED;
    }
    if ch[1].is_none() {
      missing |= missing_flags::GREEN;
    }
    if ch[2].is_none() {
      missing |= missing_flags::BLUE;
    }
    if alpha.is_none() {
      missing |= missing_flags::ALPHA;
    }
    if hsl_space {
      missing |= missing_flags::HSL_SPACE;
    }
    let rgb = match &ch {
      [Some(r), Some(g), Some(b)] => [*r, *g, *b],
      _ => {
        // HSL-space missing colors carry h/s/l natively in `modern`
        // instead; this constructor is only called for rgb channels
        [
          ch[0].unwrap_or(0.0),
          ch[1].unwrap_or(0.0),
          ch[2].unwrap_or(0.0),
        ]
      }
    };
    Color {
      hsl_raw: None,
      r: fuzz_channel(rgb[0].clamp(0.0, 255.0)),
      g: fuzz_channel(rgb[1].clamp(0.0, 255.0)),
      b: fuzz_channel(rgb[2].clamp(0.0, 255.0)),
      a: clamp01(alpha.unwrap_or(0.0)),
      literal: None,
      modern: None,
      missing,
    }
  }

  /// Create a color from RGBA channels with 8-bit alpha (0-255)
  pub const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color {
      hsl_raw: None,
      modern: None,
      r: r as f64,
      g: g as f64,
      b: b as f64,
      a: a as f64 / 255.0,
      literal: None,
      missing: 0,
    }
  }

  /// Build a color from HWB channels (h degrees, w/b 0-1) carrying the
  /// modern `hwb` space so it serializes back in hwb notation
  pub fn from_hwb_channels(ch: [Option<f64>; 3], alpha: f64) -> Color {
    let h = ch[0].unwrap_or(0.0);
    let w = ch[1].unwrap_or(0.0);
    let b = ch[2].unwrap_or(0.0);
    let rgb = modern::hwb_to_rgb([h, w, b]);
    let mc = ModernColor::new(ColorSpace::Hwb, [ch[0], ch[1], ch[2]], Some(alpha));
    Color {
      hsl_raw: None,
      r: rgb[0] * 255.0,
      g: rgb[1] * 255.0,
      b: rgb[2] * 255.0,
      a: alpha,
      literal: None,
      modern: Some(Box::new(mc)),
      missing: 0,
    }
  }

  /// Attach a source representation to this color
  pub fn with_literal(mut self, text: &str) -> Color {
    self.literal = Some(text.into());
    self
  }

  /// Whether all channels (and alpha for hex purposes) are integral
  fn integral(&self) -> bool {
    self.r.fract() == 0.0 && self.g.fract() == 0.0 && self.b.fract() == 0.0
  }

  /// Parse a hex literal (3, 4, 6, or 8 digits, with or without `#`).
  ///
  /// 3- and 6-digit forms preserve the literal text; 4/8-digit forms
  /// produce computed colors (matching the reference serializer, which
  /// converts them to `rgba(...)` on output).
  pub fn from_hex(hex: &str) -> Result<Color, Error> {
    let raw = hex.trim_start_matches('#');
    if !matches!(raw.len(), 3 | 4 | 6 | 8) || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
      return Err(Error::type_error(format!(
        "Invalid hex color: \"{}\".",
        hex
      )));
    }
    let expand = |s: &str| -> f64 {
      let doubled: String = s.chars().flat_map(|c| [c, c]).collect();
      u8::from_str_radix(&doubled, 16).unwrap_or(0) as f64
    };
    let (r, g, b, a) = match raw.len() {
      3 => (
        expand(&raw[0..1]),
        expand(&raw[1..2]),
        expand(&raw[2..3]),
        1.0,
      ),
      4 => (
        expand(&raw[0..1]),
        expand(&raw[1..2]),
        expand(&raw[2..3]),
        expand(&raw[3..4]) / 255.0,
      ),
      6 => (
        u8::from_str_radix(&raw[0..2], 16).unwrap_or(0) as f64,
        u8::from_str_radix(&raw[2..4], 16).unwrap_or(0) as f64,
        u8::from_str_radix(&raw[4..6], 16).unwrap_or(0) as f64,
        1.0,
      ),
      _ => (
        u8::from_str_radix(&raw[0..2], 16).unwrap_or(0) as f64,
        u8::from_str_radix(&raw[2..4], 16).unwrap_or(0) as f64,
        u8::from_str_radix(&raw[4..6], 16).unwrap_or(0) as f64,
        u8::from_str_radix(&raw[6..8], 16).unwrap_or(0) as f64 / 255.0,
      ),
    };
    let mut c = Color {
      hsl_raw: None,
      r,
      g,
      b,
      a,
      literal: None,
      modern: None,
      missing: 0,
    };
    // Only the legacy-representable forms keep their literal text
    if raw.len() == 3 || raw.len() == 6 {
      c.literal = Some(hex.into());
    }
    Ok(c)
  }

  /// Canonical lowercase hex (`#aabbcc` or `#aabbccdd` when transparent).
  /// The reference serializer keeps computed colors in 6-digit form.
  pub fn hex(&self) -> String {
    let r = self.r.round().clamp(0.0, 255.0) as u8;
    let g = self.g.round().clamp(0.0, 255.0) as u8;
    let b = self.b.round().clamp(0.0, 255.0) as u8;
    if self.a >= 1.0 {
      format!("#{:02x}{:02x}{:02x}", r, g, b)
    } else {
      format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        r,
        g,
        b,
        (self.a * 255.0).round() as u8
      )
    }
  }

  /// Red channel (0-255)
  pub fn red(&self) -> f64 {
    self.r
  }

  /// Green channel (0-255)
  pub fn green(&self) -> f64 {
    self.g
  }

  /// Blue channel (0-255)
  pub fn blue(&self) -> f64 {
    self.b
  }

  /// Alpha as 0-255 (quantized)
  pub fn alpha_u8(&self) -> u8 {
    (self.a * 255.0).round() as u8
  }

  /// Alpha as 0.0-1.0
  pub fn alpha(&self) -> f64 {
    self.a
  }

  /// Hue in degrees (0-360)
  pub fn hue(&self) -> f64 {
    self.to_hsl().0
  }

  /// HSL saturation as 0.0-1.0
  pub fn saturation(&self) -> f64 {
    self.to_hsl().1
  }

  /// HSL lightness as 0.0-1.0
  pub fn lightness(&self) -> f64 {
    self.to_hsl().2
  }

  /// Convert to HSL: `(hue degrees 0-360, saturation 0-1, lightness 0-1)`
  ///
  /// Raw hsl channels (kept for hsl-space colors) take priority: the
  /// reference reads `hsl(0 200% 150%)`'s saturation as `200%`, not as a
  /// re-derivation from the clamped rgb backing.
  pub fn to_hsl(&self) -> (f64, f64, f64) {
    if let Some(ch) = &self.hsl_raw {
      return (ch[0], ch[1], ch[2]);
    }
    let r = self.r / 255.0;
    let g = self.g / 255.0;
    let b = self.b / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f64::EPSILON {
      return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
      d / (2.0 - max - min)
    } else {
      d / (max + min)
    };
    let hue = match max {
      m if m == r => ((g - b) / d + if g < b { 6.0 } else { 0.0 }) * 60.0,
      m if m == g => ((b - r) / d + 2.0) * 60.0,
      _ => ((r - g) / d + 4.0) * 60.0,
    };
    (hue, s, l)
  }

  /// Create a color from HSL components
  ///
  /// `hue` is degrees (0-360, wrapped), `s`/`l` are 0-1 (clamped).
  /// Channels stay fractional — the reference compiler keeps the exact
  /// HSL-derived values (76.5, not 77).
  ///
  /// This is the rgb-space form used when converting an rgb color through
  /// hsl math (grayscale/adjust on rgb colors): both ends clamp and no raw
  /// hsl channels are kept. For colors that LIVE in the hsl space use
  /// [`Color::from_hsl_raw`], which follows the reference's channel rules.
  pub fn from_hsl(hue: f64, s: f64, l: f64, alpha: f64) -> Color {
    let h = ((hue % 360.0) + 360.0) % 360.0;
    let s = clamp_f64(s, 0.0, 1.0);
    let l = clamp_f64(l, 0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = match hp as u32 {
      0 => (c, x, 0.0),
      1 => (x, c, 0.0),
      2 => (0.0, c, x),
      3 => (0.0, x, c),
      4 => (x, 0.0, c),
      _ => (c, 0.0, x),
    };
    Color {
      hsl_raw: None,
      modern: None,
      r: fuzz_channel(clamp_channel_f64((r1 + m) * 255.0)),
      g: fuzz_channel(clamp_channel_f64((g1 + m) * 255.0)),
      b: fuzz_channel(clamp_channel_f64((b1 + m) * 255.0)),
      a: clamp01(alpha),
      literal: None,
      missing: 0,
    }
  }

  /// Create a legacy color that LIVES in the hsl space, keeping the raw
  /// (unclamped) channels the way the reference does.
  ///
  /// Reference construction rules (`LinearChannel` in the reference
  /// colorspace module):
  /// - `hue` wraps modulo 360 (an angle channel),
  /// - `saturation` is lower-clamped at 0 only (`lowerClamped: true`,
  ///   `200%` stays `200%`),
  /// - `lightness` is never clamped at construction (`-50%` stays raw).
  ///
  /// The rgb backing is derived from the clamped-for-math values, exactly
  /// like converting `hsl(0 200% 150%)` to rgb (white). The result carries
  /// the `HSL_SPACE` tag so it serializes back in hsl notation.
  pub fn from_hsl_raw(hue: f64, s: f64, l: f64, alpha: f64) -> Color {
    let h = ((hue % 360.0) + 360.0) % 360.0;
    let s_raw = s.max(0.0);
    let l_raw = l;
    let mut c = Color::from_hsl(h, s_raw.clamp(0.0, 1.0), l_raw.clamp(0.0, 1.0), alpha);
    c.hsl_raw = Some(Box::new([h, s_raw, l_raw]));
    c.missing |= missing_flags::HSL_SPACE;
    c
  }

  /// Adjust the lightness by `amount` (percentage points as -1.0..1.0 of
  /// the 0-1 lightness scale, i.e. VASS `lighten($c, 10%)` passes 0.1)
  ///
  /// Legacy `lighten()`/`darken()` clamp the lightness to 0-100% (observed
  /// reference behavior) while keeping saturation/hue as-is. For hsl-space
  /// colors the raw channels carry through.
  pub fn adjust_lightness(&self, amount: f64) -> Color {
    let (h, s, l) = self.to_hsl();
    self.hsl_result(h, s, clamp_f64(l + amount, 0.0, 1.0), self.alpha())
  }

  /// Adjust the saturation by `amount` (same scale as lightness)
  ///
  /// Legacy `saturate()`/`desaturate()` clamp the saturation to 0-100%.
  pub fn adjust_saturation(&self, amount: f64) -> Color {
    let (h, s, l) = self.to_hsl();
    self.hsl_result(h, clamp_f64(s + amount, 0.0, 1.0), l, self.alpha())
  }

  /// Rotate the hue by `degrees`
  pub fn adjust_hue(&self, degrees: f64) -> Color {
    let (h, s, l) = self.to_hsl();
    self.hsl_result(h + degrees, s, l, self.alpha())
  }

  /// The complement (hue + 180)
  pub fn complement(&self) -> Color {
    self.adjust_hue(180.0)
  }

  /// Convert to grayscale (saturation 0), preserving lightness
  ///
  /// The reference keeps the raw lightness (`grayscale(hsl(0 200% 150%))`
  /// is `hsl(0, 0%, 150%)`) — the lightness is NOT clamped.
  pub fn grayscale(&self) -> Color {
    let (h, _s, l) = self.to_hsl();
    self.hsl_result(h, 0.0, l, self.alpha())
  }

  /// Build an hsl-based result that preserves the input color's space
  /// membership: hsl-space colors keep their raw channels via
  /// [`Color::from_hsl_raw`], rgb colors go through the clamped
  /// [`Color::from_hsl`] (staying in rgb space).
  fn hsl_result(&self, h: f64, s: f64, l: f64, alpha: f64) -> Color {
    if self.is_hsl_space() {
      Color::from_hsl_raw(h, s, l, alpha)
    } else {
      Color::from_hsl(h, s, l, alpha)
    }
  }

  /// Increase opacity by `amount` (0-1, clamped)
  pub fn opacify(&self, amount: f64) -> Color {
    Color {
      hsl_raw: self.hsl_raw.clone(),
      modern: None,
      r: self.r,
      g: self.g,
      b: self.b,
      a: clamp01(self.a + amount),
      literal: None,
      missing: self.missing & !missing_flags::ALPHA,
    }
  }

  /// Decrease opacity by `amount` (0-1, clamped)
  pub fn transparentize(&self, amount: f64) -> Color {
    self.opacify(-amount)
  }

  /// Mix two colors by weight (0 = `other`, 1 = `self`; default 0.5).
  ///
  /// Implements the reference compiler's legacy mix algorithm exactly:
  /// the user weight `w` and the alpha distance `a = a1 - a2` are both
  /// normalized to [-1, 1], combined via `(w + a) / (1 + w*a)`, then
  /// renormalized to [0, 1] as the channel weight of `self`. Alpha is
  /// combined with the raw (unadjusted) weight scale.
  pub fn mix(&self, other: &Color, weight: f64) -> Color {
    let weight_scale = clamp_f64(weight, 0.0, 1.0);
    let normalized_weight = weight_scale * 2.0 - 1.0;
    let alpha_distance = self.a - other.a;
    let combined_weight1 = if normalized_weight * alpha_distance == -1.0 {
      normalized_weight
    } else {
      (normalized_weight + alpha_distance) / (1.0 + normalized_weight * alpha_distance)
    };
    let weight1 = (combined_weight1 + 1.0) / 2.0;
    let weight2 = 1.0 - weight1;
    Color {
      hsl_raw: None,
      modern: None,
      r: clamp_channel_f64(self.r * weight1 + other.r * weight2),
      g: clamp_channel_f64(self.g * weight1 + other.g * weight2),
      b: clamp_channel_f64(self.b * weight1 + other.b * weight2),
      a: clamp01(self.a * weight_scale + other.a * (1.0 - weight_scale)),
      literal: None,
      missing: 0,
    }
  }

  /// Invert the color; `weight` 0-1 (1 = full inversion)
  pub fn invert(&self, weight: f64) -> Color {
    let w = clamp_f64(weight, 0.0, 1.0);
    let inv = |c: f64| -> f64 { clamp_channel_f64(255.0 - c) };
    let blend =
      |orig: f64, inverted: f64| -> f64 { clamp_channel_f64(inverted * w + orig * (1.0 - w)) };
    Color {
      hsl_raw: None,
      modern: None,
      r: blend(self.r, inv(self.r)),
      g: blend(self.g, inv(self.g)),
      b: blend(self.b, inv(self.b)),
      a: self.a,
      literal: None,
      missing: self.missing & !(missing_flags::RED | missing_flags::GREEN | missing_flags::BLUE),
    }
  }

  /// Whether the color is light (per dart-sass luminance > 0.5 heuristic
  /// used by `lightness`-based contrast decisions)
  pub fn is_light(&self) -> bool {
    self.lightness() > 0.5
  }

  /// The known CSS named color for these channels, if any
  fn named(&self) -> Option<&'static str> {
    if self.a < 1.0 || !self.integral() {
      return None;
    }
    let (r, g, b) = (self.r as u8, self.g as u8, self.b as u8);
    Some(match (r, g, b) {
      (0, 0, 0) => "black",
      (255, 255, 255) => "white",
      (255, 0, 0) => "red",
      (0, 128, 0) => "green",
      (0, 0, 255) => "blue",
      (255, 255, 0) => "yellow",
      (0, 255, 255) => "aqua",
      // dart-sass prefers the CSS-spec name "fuchsia" for #ff00ff
      (255, 0, 255) => "fuchsia",
      (192, 192, 192) => "silver",
      (128, 128, 128) => "gray",
      (128, 0, 0) => "maroon",
      (128, 128, 0) => "olive",
      (0, 128, 128) => "teal",
      (0, 0, 128) => "navy",
      (128, 0, 128) => "purple",
      (240, 248, 255) => "aliceblue",
      (250, 235, 215) => "antiquewhite",
      (127, 255, 212) => "aquamarine",
      (240, 255, 240) => "honeydew",
      (255, 240, 245) => "lavenderblush",
      (124, 252, 0) => "lawngreen",
      (255, 250, 205) => "lemonchiffon",
      (173, 216, 230) => "lightblue",
      (240, 128, 128) => "lightcoral",
      (224, 255, 255) => "lightcyan",
      (250, 250, 210) => "lightgoldenrodyellow",
      (211, 211, 211) => "lightgray",
      (144, 238, 144) => "lightgreen",
      (255, 182, 193) => "lightpink",
      (255, 160, 122) => "lightsalmon",
      (32, 178, 170) => "lightseagreen",
      (135, 206, 250) => "lightskyblue",
      (119, 136, 153) => "lightslategray",
      (176, 196, 222) => "lightsteelblue",
      (255, 255, 224) => "lightyellow",
      (0, 255, 0) => "lime",
      (50, 205, 50) => "limegreen",
      (250, 240, 230) => "linen",
      (102, 51, 153) => "rebeccapurple",
      (255, 20, 147) => "deeppink",
      (105, 105, 105) => "dimgray",
      (30, 144, 255) => "dodgerblue",
      (0, 0, 139) => "darkblue",
      (139, 0, 139) => "darkmagenta",
      (0, 100, 0) => "darkgreen",
      (169, 169, 169) => "darkgray",
      (184, 134, 11) => "darkgoldenrod",
      _ => return None,
    })
  }

  /// Look up a named CSS color by identifier
  pub fn from_named(name: &str) -> Option<Color> {
    let hex: &str = match name.to_ascii_lowercase().as_str() {
      "black" => "#000000",
      "white" => "#ffffff",
      "red" => "#ff0000",
      "green" => "#008000",
      "lime" => "#00ff00",
      "blue" => "#0000ff",
      "yellow" => "#ffff00",
      "cyan" | "aqua" => "#00ffff",
      "magenta" | "fuchsia" => "#ff00ff",
      "silver" => "#c0c0c0",
      "gray" | "grey" => "#808080",
      "maroon" => "#800000",
      "olive" => "#808000",
      "teal" => "#008080",
      "navy" => "#000080",
      "purple" => "#800080",
      "orange" => "#ffa500",
      "pink" => "#ffc0cb",
      "gold" => "#ffd700",
      "indigo" => "#4b0082",
      "violet" => "#ee82ee",
      "transparent" => {
        return Some(Color {
          hsl_raw: None,
          r: 0.0,
          g: 0.0,
          b: 0.0,
          a: 0.0,
          literal: None,
          modern: None,
          missing: 0,
        })
        .map(|c| c.with_literal(name));
      }
      "rebeccapurple" => "#663399",
      "aliceblue" => "#f0f8ff",
      "antiquewhite" => "#faebd7",
      "aquamarine" => "#7fffd4",
      "beige" => "#f5f5dc",
      "bisque" => "#ffe4c4",
      "brown" => "#a52a2a",
      "burlywood" => "#deb887",
      "cadetblue" => "#5f9ea0",
      "chartreuse" => "#7fff00",
      "chocolate" => "#d2691e",
      "coral" => "#ff7f50",
      "cornflowerblue" => "#6495ed",
      "cornsilk" => "#fff8dc",
      "crimson" => "#dc143c",
      "darkblue" => "#00008b",
      "darkcyan" => "#008b8b",
      "darkgoldenrod" => "#b8860b",
      "darkgray" | "darkgrey" => "#a9a9a9",
      "darkgreen" => "#006400",
      "darkkhaki" => "#bdb76b",
      "darkmagenta" => "#8b008b",
      "darkolivegreen" => "#556b2f",
      "darkorange" => "#ff8c00",
      "darkorchid" => "#9932cc",
      "darkred" => "#8b0000",
      "darksalmon" => "#e9967a",
      "darkseagreen" => "#8fbc8f",
      "darkslateblue" => "#483d8b",
      "darkslategray" | "darkslategrey" => "#2f4f4f",
      "darkturquoise" => "#00ced1",
      "darkviolet" => "#9400d3",
      "deeppink" => "#ff1493",
      "deepskyblue" => "#00bfff",
      "dimgray" | "dimgrey" => "#696969",
      "dodgerblue" => "#1e90ff",
      "firebrick" => "#b22222",
      "floralwhite" => "#fffaf0",
      "forestgreen" => "#228b22",
      "gainsboro" => "#dcdcdc",
      "ghostwhite" => "#f8f8ff",
      "goldenrod" => "#daa520",
      "greenyellow" => "#adff2f",
      "honeydew" => "#f0fff0",
      "hotpink" => "#ff69b4",
      "indianred" => "#cd5c5c",
      "ivory" => "#fffff0",
      "khaki" => "#f0e68c",
      "lavender" => "#e6e6fa",
      "lavenderblush" => "#fff0f5",
      "lawngreen" => "#7cfc00",
      "lemonchiffon" => "#fffacd",
      "lightblue" => "#add8e6",
      "lightcoral" => "#f08080",
      "lightcyan" => "#e0ffff",
      "lightgoldenrodyellow" => "#fafad2",
      "lightgray" | "lightgrey" => "#d3d3d3",
      "lightgreen" => "#90ee90",
      "lightpink" => "#ffb6c1",
      "lightsalmon" => "#ffa07a",
      "lightseagreen" => "#20b2aa",
      "lightskyblue" => "#87cefa",
      "lightslategray" | "lightslategrey" => "#778899",
      "lightsteelblue" => "#b0c4de",
      "lightyellow" => "#ffffe0",
      "limegreen" => "#32cd32",
      "linen" => "#faf0e6",
      "mediumaquamarine" => "#66cdaa",
      "mediumblue" => "#0000cd",
      "mediumorchid" => "#ba55d3",
      "mediumpurple" => "#9370db",
      "mediumseagreen" => "#3cb371",
      "mediumslateblue" => "#7b68ee",
      "mediumspringgreen" => "#00fa9a",
      "mediumturquoise" => "#48d1cc",
      "mediumvioletred" => "#c71585",
      "midnightblue" => "#191970",
      "mintcream" => "#f5fffa",
      "mistyrose" => "#ffe4e1",
      "moccasin" => "#ffe4b5",
      "navajowhite" => "#ffdead",
      "oldlace" => "#fdf5ee",
      "olivedrab" => "#6b8e23",
      "orangered" => "#ff4500",
      "orchid" => "#da70d6",
      "palegoldenrod" => "#eee8aa",
      "palegreen" => "#98fb98",
      "paleturquoise" => "#afeeee",
      "palevioletred" => "#db7093",
      "papayawhip" => "#ffefd5",
      "peachpuff" => "#ffdab9",
      "peru" => "#cd853f",
      "plum" => "#dda0dd",
      "powderblue" => "#b0e0e6",
      "rosybrown" => "#bc8f8f",
      "royalblue" => "#4169e1",
      "saddlebrown" => "#8b4513",
      "salmon" => "#fa8072",
      "sandybrown" => "#f4a460",
      "seagreen" => "#2e8b57",
      "seashell" => "#fff5ee",
      "sienna" => "#a0522d",
      "skyblue" => "#87ceeb",
      "slateblue" => "#6a5acd",
      "slategray" | "slategrey" => "#708090",
      "snow" => "#fffafa",
      "springgreen" => "#00ff7f",
      "steelblue" => "#4682b4",
      "tan" => "#d2b48c",
      "thistle" => "#d8bfd8",
      "tomato" => "#ff6347",
      "turquoise" => "#40e0d0",
      "wheat" => "#f5deb3",
      "whitesmoke" => "#f5f5f5",
      "yellowgreen" => "#9acd32",
      _ => return None,
    };
    Color::from_hex(hex).ok().map(|c| c.with_literal(name))
  }

  /// Serialize a computed color's channels: integers as integers, else as
  /// percentages of 255 (matching the reference serializer, which writes
  /// ALL channels as percentages when any is fractional)
  /// Full-precision channel serialization for @debug output
  fn serialize_channels_full(&self) -> (String, String, String, bool) {
    let integral = self.integral();
    let ch = |v: f64| -> String {
      if integral {
        format!("{}", v.round() as i64)
      } else {
        format!("{}%", fuzz_channel(v) / 255.0 * 100.0)
      }
    };
    (ch(self.r), ch(self.g), ch(self.b), integral)
  }

  fn serialize_channels(&self) -> (String, String, String, bool) {
    let integral = self.integral();
    let ch = |v: f64| -> String {
      if integral {
        format!("{}", v.round() as i64)
      } else {
        format!("{}%", format_f64(v / 255.0 * 100.0))
      }
    };
    (ch(self.r), ch(self.g), ch(self.b), integral)
  }
}

impl Color {
  /// Whether any legacy channel (or alpha) is missing (`none`)
  pub fn has_missing(&self) -> bool {
    self.missing
      & (missing_flags::RED | missing_flags::GREEN | missing_flags::BLUE | missing_flags::ALPHA)
      != 0
  }

  /// Whether this legacy color was created in the hsl space
  pub fn is_hsl_space(&self) -> bool {
    self.missing & missing_flags::HSL_SPACE != 0
  }

  /// The name of the first missing rgb channel (`red`/`green`/`blue`),
  /// or `alpha` when only alpha is missing; `None` when nothing is missing
  pub fn first_missing_channel(&self) -> Option<&'static str> {
    if self.missing & missing_flags::RED != 0 {
      Some("red")
    } else if self.missing & missing_flags::GREEN != 0 {
      Some("green")
    } else if self.missing & missing_flags::BLUE != 0 {
      Some("blue")
    } else if self.missing & missing_flags::ALPHA != 0 {
      Some("alpha")
    } else {
      None
    }
  }

  /// Serialize a legacy channel value in the modern space-separated form:
  /// integers as integers, otherwise the exact number (reference emits
  /// `rgb(127.5 none 0)` for `rgb(50% none 0)`)
  fn missing_channel_text(v: f64, css: bool) -> String {
    let r = v.round();
    let v = if (v - r).abs() < 1e-11 { r } else { v };
    if css { format_f64(v) } else { format!("{}", v) }
  }

  /// The `rgb(none 255 0 / 0.5)` space-separated form for legacy colors
  /// with missing channels (`css`: 10-digit precision for CSS output,
  /// full precision for @debug)
  fn missing_rgb_form(&self, css: bool) -> String {
    let ch = |bit: u8, v: f64| -> String {
      if self.missing & bit != 0 {
        "none".to_string()
      } else {
        Self::missing_channel_text(v, css)
      }
    };
    let alpha = if self.missing & missing_flags::ALPHA != 0 {
      " / none".to_string()
    } else if self.a < 1.0 {
      format!(" / {}", Self::missing_channel_text(self.a, css))
    } else {
      String::new()
    };
    format!(
      "rgb({} {} {}{})",
      ch(missing_flags::RED, self.r),
      ch(missing_flags::GREEN, self.g),
      ch(missing_flags::BLUE, self.b),
      alpha
    )
  }

  /// Canonical function-form text: `rgb(r, g, b)` when opaque, else
  /// `rgba(r, g, b, a)`. Colors with missing channels serialize in the
  /// space-separated form (`rgb(none 255 0)`) like the reference.
  pub fn canonical_fn_text(&self) -> String {
    if self.has_missing() {
      return self.missing_rgb_form(true);
    }
    let (r, g, b, _) = self.serialize_channels();
    if self.a >= 1.0 {
      format!("rgb({}, {}, {})", r, g, b)
    } else {
      format!("rgba({}, {}, {}, {})", r, g, b, format_f64(self.a))
    }
  }

  /// Render as CSS text
  /// Serialize like the reference @debug output: full float precision
  /// for computed channels (rgb(...) percents, modern spaces)
  pub fn to_debug_string(&self) -> String {
    if let Some(m) = &self.modern {
      return m.to_css(false);
    }
    if self.has_missing() {
      return self.missing_rgb_form(false);
    }
    if let Some(t) = &self.literal {
      return t.to_string();
    }
    if self.is_hsl_space() {
      return self.hsl_comma_form(false);
    }
    if self.a >= 1.0 {
      if self.integral() {
        if let Some(name) = self.named() {
          return name.to_string();
        }
        return self.hex();
      }
      let (r, g, b, _) = self.serialize_channels_full();
      return format!("rgb({}, {}, {})", r, g, b);
    }
    let (r, g, b, _) = self.serialize_channels_full();
    format!("rgba({}, {}, {}, {})", r, g, b, self.a)
  }

  /// The `hsl(h, s%, l%)` comma form for computed colors in the legacy
  /// hsl space (the reference keeps hsl notation for them)
  fn hsl_comma_form(&self, css: bool) -> String {
    let (h, s, l) = self.to_hsl();
    let h = ((h % 360.0) + 360.0) % 360.0;
    let num = |v: f64| -> String { if css { format_f64(v) } else { format!("{}", v) } };
    let hr = h.round();
    let h = if (h - hr).abs() < 1e-11 { hr } else { h };
    // Snap HSL-roundtrip float noise (50.000000000000014 -> 50)
    let snap = |v: f64| -> f64 {
      let r = (v * 100.0).round() / 100.0;
      if (v - r).abs() < 1e-9 { r } else { v }
    };
    let (s, l) = (snap(s * 100.0), snap(l * 100.0));
    if self.a >= 1.0 {
      format!("hsl({}, {}%, {}%)", num(h), num(s), num(l))
    } else {
      format!(
        "hsla({}, {}%, {}%, {})",
        num(h),
        num(s),
        num(l),
        num(self.a)
      )
    }
  }

  /// to_css helper
  pub fn to_css(&self) -> String {
    // Modern CSS Color 4 spaces serialize in their native notation
    if let Some(m) = &self.modern {
      return m.to_css(true);
    }
    // Missing channels force the space-separated form (reference rule);
    // checked before the literal so normalized numbers are re-emitted
    if self.has_missing() {
      return self.missing_rgb_form(true);
    }
    // Preserve the source notation verbatim when available
    if let Some(t) = &self.literal {
      return t.to_string();
    }
    if self.is_hsl_space() {
      return self.hsl_comma_form(true);
    }
    if self.a >= 1.0 {
      if self.integral() {
        if let Some(name) = self.named() {
          return name.to_string();
        }
        self.hex()
      } else {
        let (r, g, b, _) = self.serialize_channels();
        format!("rgb({}, {}, {})", r, g, b)
      }
    } else {
      let (r, g, b, _) = self.serialize_channels();
      format!("rgba({}, {}, {}, {})", r, g, b, format_f64(self.a))
    }
  }
}

use modern::{linear_to_srgb, to_srgb_linear};

impl ModernColor {
  /// Wrap into a legacy-backed Color carrying the modern channels
  pub fn into_color(self) -> Color {
    let (r, g, b, a) = self.to_legacy();
    Color {
      hsl_raw: None,
      r,
      g,
      b,
      a,
      literal: None,
      modern: Some(Box::new(self)),
      missing: 0,
    }
  }

  /// Serialize in the space's native notation.
  /// `css`: true -> 10-digit precision (CSS output), false -> full
  /// precision (@debug output), matching the reference's two modes.
  pub fn to_css(&self, css: bool) -> String {
    let fmt = |v: f64| {
      if css { format_f64(v) } else { format!("{}", v) }
    };
    let ch = |i: usize| -> String {
      match self.ch[i] {
        None => "none".to_string(),
        Some(v) => {
          let r = v.round();
          let v = if (v - r).abs() < 1e-11 { r } else { v };
          let out = self.space.channel_out(self.space.channels()[i], v);
          // Snap percentage-scale float noise from the 0-1 storage
          // roundtrip (55.000000000000004 -> 55)
          let out = if matches!(self.space.channel_unit(self.space.channels()[i]), Some("%")) {
            let r = out.round();
            if (out - r).abs() < 1e-9 { r } else { out }
          } else {
            out
          };
          let unit = self
            .space
            .channel_unit(self.space.channels()[i])
            .unwrap_or("");
          let text = fmt(out);
          match (unit, text.ends_with('%') || text.ends_with("deg")) {
            ("%", _) => format!("{}%", text),
            ("deg", _) => format!("{}deg", text),
            _ => text,
          }
        }
      }
    };
    let alpha = match self.alpha {
      None => " / none".to_string(),
      Some(a) if a < 1.0 => format!(" / {}", fmt(a)),
      _ => String::new(),
    };
    // hsl/hwb hues serialize without a unit suffix; also snap float
    // noise to integers like the reference's fuzzy channel handling
    let hue = |i: usize| -> String {
      match self.ch[i] {
        None => "none".to_string(),
        Some(v) => {
          let r = v.round();
          let v = if (v - r).abs() < 1e-11 { r } else { v };
          fmt(v)
        }
      }
    };
    match self.space {
      ColorSpace::Srgb
      | ColorSpace::SrgbLinear
      | ColorSpace::DisplayP3
      | ColorSpace::A98Rgb
      | ColorSpace::ProphotoRgb
      | ColorSpace::Rec2020
      | ColorSpace::Xyz
      | ColorSpace::XyzD50 => {
        format!(
          "color({} {} {} {}{})",
          self.space.name(),
          ch(0),
          ch(1),
          ch(2),
          alpha
        )
      }
      // hsl keeps the legacy comma form; hwb uses spaces; both keep a
      // bare hue (no deg suffix) like the reference. hsl colors WITH
      // missing channels switch to the space-separated modern syntax
      // (`hsl(none 50% 50%)`, hue carrying a `deg` suffix when present)
      ColorSpace::Hsl if self.ch.iter().any(|c| c.is_none()) || self.alpha.is_none() => {
        let h = match self.ch[0] {
          None => "none".to_string(),
          Some(_) => format!("{}deg", hue(0)),
        };
        format!("hsl({} {} {}{})", h, ch(1), ch(2), alpha)
      }
      ColorSpace::Hsl => {
        format!("hsl({}, {}, {}{})", hue(0), ch(1), ch(2), alpha)
      }
      ColorSpace::Hwb if self.ch.iter().any(|c| c.is_none()) || self.alpha.is_none() => {
        let h = match self.ch[0] {
          None => "none".to_string(),
          Some(_) => format!("{}deg", hue(0)),
        };
        format!("hwb({} {} {}{})", h, ch(1), ch(2), alpha)
      }
      ColorSpace::Hwb => {
        format!("hwb({} {} {}{})", hue(0), ch(1), ch(2), alpha)
      }
      ColorSpace::Lab | ColorSpace::Oklab => {
        format!(
          "{}({} {} {}{})",
          self.space.name(),
          ch(0),
          ch(1),
          ch(2),
          alpha
        )
      }
      ColorSpace::Lch | ColorSpace::Oklch => {
        format!(
          "{}({} {} {}{})",
          self.space.name(),
          ch(0),
          ch(1),
          ch(2),
          alpha
        )
      }
    }
  }

  /// Convert to legacy sRGB channels (0-255) and alpha, filling missing
  /// channels with 0 for the round trip through the legacy representation.
  pub fn to_legacy(&self) -> (f64, f64, f64, f64) {
    let native = [
      self.value_or_zero(0),
      self.value_or_zero(1),
      self.value_or_zero(2),
    ];
    let lin = to_srgb_linear(native, self.space);
    let srgb = [
      linear_to_srgb(lin[0]).clamp(0.0, 1.0),
      linear_to_srgb(lin[1]).clamp(0.0, 1.0),
      linear_to_srgb(lin[2]).clamp(0.0, 1.0),
    ];
    (
      srgb[0] * 255.0,
      srgb[1] * 255.0,
      srgb[2] * 255.0,
      self.alpha.unwrap_or(1.0),
    )
  }
}

/// Clamp a percentage/degree value into `min..=max`
fn clamp_f64(v: f64, min: f64, max: f64) -> f64 {
  v.clamp(min, max)
}

/// Snap a color channel to the nearest integer when it is within `1e-8`
/// of it (floating-point noise tolerance, matching the reference
/// compiler's fuzzy channel handling)
fn fuzz_channel(v: f64) -> f64 {
  let r = v.round();
  if (v - r).abs() < 1e-8 { r } else { v }
}

/// Clamp a fractional channel value into 0.0..=255.0
fn clamp_channel_f64(v: f64) -> f64 {
  v.clamp(0.0, 255.0)
}

/// Clamp alpha into 0.0..=1.0
fn clamp01(v: f64) -> f64 {
  v.clamp(0.0, 1.0)
}

impl fmt::Display for Color {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.to_css())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_from_hex() {
    let c = Color::from_hex("#abc").unwrap();
    assert_eq!((c.r, c.g, c.b), (170.0, 187.0, 204.0));
    assert_eq!(c.a, 1.0);
    let c = Color::from_hex("#AABBCCDD").unwrap();
    assert!((c.a - 0.8666666666666667).abs() < 1e-9);
    assert_eq!(
      Color::from_hex("#12345").unwrap_err().kind,
      crate::error::ErrorKind::Type
    );
  }

  #[test]
  fn test_literal_preservation() {
    // Hex literals keep their exact source text
    assert_eq!(Color::from_hex("#F00").unwrap().to_css(), "#F00");
    assert_eq!(Color::from_hex("#ff0088").unwrap().to_css(), "#ff0088");
    // 8-digit hex converts to rgba() with full-precision alpha
    assert_eq!(
      Color::from_hex("#ff008880").unwrap().to_css(),
      "rgba(255, 0, 136, 0.5019607843)"
    );
    // Keywords keep their name
    assert_eq!(Color::from_named("red").unwrap().to_css(), "red");
  }

  #[test]
  fn test_hsl_roundtrip() {
    for hex in ["#ff0000", "#00ff00", "#0000ff", "#336699", "#abcdef"] {
      let c = Color::from_hex(hex).unwrap();
      let (h, s, l) = c.to_hsl();
      let back = Color::from_hsl(h, s, l, 1.0);
      assert_eq!(back.hex(), c.hex(), "roundtrip failed for {}", hex);
    }
  }

  #[test]
  fn test_lighten_darken() {
    // #336699 = hsl(210, 50%, 40%)
    let c = Color::from_hex("#336699").unwrap();
    assert_eq!(c.hue(), 210.0);
    // Computed channels stay fractional; serialization uses percentages
    // (#336699 lighten 10%: hsl lightness 0.4 → 0.5 → channels 25/50/75%)
    assert_eq!(c.adjust_lightness(0.1).to_css(), "rgb(25%, 50%, 75%)");
    assert_eq!(c.adjust_lightness(-0.1).to_css(), "rgb(15%, 30%, 45%)");
  }

  #[test]
  fn test_computed_serialization() {
    // Integral channels: named match, then hex
    assert_eq!(
      Color::from_hex("#ff0000").unwrap().invert(1.0).to_css(),
      "aqua"
    );
    assert_eq!(
      Color::from_channels(255.0, 51.0, 102.0, 1.0).to_css(),
      "#ff3366"
    );
    // Fractional channels: percentages
    assert_eq!(
      Color::from_channels(127.5, 0.0, 127.5, 1.0).to_css(),
      "rgb(50%, 0%, 50%)"
    );
    // Alpha < 1 with integral channels
    assert_eq!(
      Color::from_channels(0.0, 0.0, 0.0, 0.7).to_css(),
      "rgba(0, 0, 0, 0.7)"
    );
  }

  #[test]
  fn test_named_lookup() {
    assert_eq!(Color::from_named("rebeccapurple").unwrap().hex(), "#663399");
    assert_eq!(
      Color::from_named("transparent").unwrap().to_css(),
      "transparent"
    );
    assert!(Color::from_named("notacolor").is_none());
  }

  #[test]
  fn test_mix() {
    let a = Color::from_hex("#ff0000").unwrap();
    let b = Color::from_hex("#0000ff").unwrap();
    assert_eq!(a.mix(&b, 0.5).hex(), "#800080");
  }

  #[test]
  fn test_canonical_fn_text() {
    // rgb()/rgba() results serialize in canonical comma form with the
    // name chosen by alpha (dart-sass 1.106.1 parity)
    let c = Color::from_channels(255.0, 0.0, 0.0, 1.0);
    assert_eq!(c.canonical_fn_text(), "rgb(255, 0, 0)");
    let c = Color::from_channels(0.0, 0.0, 0.0, 0.5);
    assert_eq!(c.canonical_fn_text(), "rgba(0, 0, 0, 0.5)");
    // Fractional channels serialize as percentages
    let c = Color::from_channels(127.5, 0.0, 0.0, 0.5);
    assert_eq!(c.canonical_fn_text(), "rgba(50%, 0%, 0%, 0.5)");
  }

  #[test]
  fn test_alpha_zero_serializes_as_rgba() {
    // Computed alpha-0 colors output rgba() text, NOT `transparent`
    // (only the `transparent` keyword literal preserves that name)
    let c = Color::from_channels(0.0, 0.0, 0.0, 0.0);
    assert_eq!(c.to_css(), "rgba(0, 0, 0, 0)");
  }

  #[test]
  fn test_mix_legacy_alpha_weighting() {
    // Verified against dart-sass 1.106.1 ground truth
    let t = Color::from_channels(0.0, 0.0, 0.0, 0.2);
    let o = Color::from_channels(0.0, 0.0, 0.0, 0.8);
    // alpha: a1*w + a2*(1-w) = 0.05 + 0.6
    assert!((t.mix(&o, 0.25).a - 0.65).abs() < 1e-9);
    // channels use the alpha-distance-adjusted weight:
    // mix(rgba(255,0,0,0.5), #00f) -> rgba(25%, 0%, 75%, 0.75)
    let r = Color::from_channels(255.0, 0.0, 0.0, 0.5);
    let b = Color::from_channels(0.0, 0.0, 255.0, 1.0);
    assert_eq!(r.mix(&b, 0.5).to_css(), "rgba(25%, 0%, 75%, 0.75)");
    // mix(rgba(255,0,0,0.4), rgba(0,0,255,0.8), 50%) -> rgba(30%, 0%, 70%, 0.6)
    let r = Color::from_channels(255.0, 0.0, 0.0, 0.4);
    let b = Color::from_channels(0.0, 0.0, 255.0, 0.8);
    assert_eq!(r.mix(&b, 0.5).to_css(), "rgba(30%, 0%, 70%, 0.6)");
  }

  #[test]
  fn test_hue_rotation() {
    let c = Color::from_hex("#ff0000").unwrap();
    assert_eq!(c.adjust_hue(120.0).hex(), "#00ff00");
    assert_eq!(c.adjust_hue(240.0).hex(), "#0000ff");
    assert_eq!(c.complement().hex(), "#00ffff");
  }
}
