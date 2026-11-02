//! Modern CSS Color 4 spaces: `oklch`, `oklab`, `lab`, `lch`, `hwb` and the
//! `color()` functional form, with the full conversion hub
//! (sRGB ⇄ linear ⇄ XYZ D65 ⇄ Lab/Lch/Oklab/Oklch, Bradford-adapted D50,
//! display-p3/a98-rgb/prophoto-rgb/rec2020 primaries).
//!
//! Conversion constants are validated against the pinned reference
//! (dart-sass 1.106.1): red converts to
//! `oklch(62.79553639214313% 0.25768330380536064 29.233880279627897deg)` and
//! `lab(54.29054140467191% 80.80492817043522 69.89096476862427)`.

/// A CSS Color 4 color space
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
  /// sRGB (legacy `rgb()`)
  Srgb,
  /// Linear-light sRGB
  SrgbLinear,
  /// Display P3
  DisplayP3,
  /// Adobe 98 RGB
  A98Rgb,
  /// ProPhoto RGB
  ProphotoRgb,
  /// ITU-R BT.2020
  Rec2020,
  /// `xyz`, the D65-referenced XYZ space (canonical name `xyz`)
  /// CIE XYZ, D65 white point
  Xyz,
  /// CIE XYZ, D50 white point
  XyzD50,
  /// HSL (legacy)
  Hsl,
  /// HWB
  Hwb,
  /// CIE Lab, D50
  Lab,
  /// CIE LCH, D50
  Lch,
  /// Oklab
  Oklab,
  /// Oklch
  Oklch,
}

impl ColorSpace {
  /// The canonical serialization name
  pub fn name(&self) -> &'static str {
    match self {
      ColorSpace::Srgb => "srgb",
      ColorSpace::SrgbLinear => "srgb-linear",
      ColorSpace::DisplayP3 => "display-p3",
      ColorSpace::A98Rgb => "a98-rgb",
      ColorSpace::ProphotoRgb => "prophoto-rgb",
      ColorSpace::Rec2020 => "rec2020",
      ColorSpace::Xyz => "xyz",
      ColorSpace::XyzD50 => "xyz-d50",
      ColorSpace::Hsl => "hsl",
      ColorSpace::Hwb => "hwb",
      ColorSpace::Lab => "lab",
      ColorSpace::Lch => "lch",
      ColorSpace::Oklab => "oklab",
      ColorSpace::Oklch => "oklch",
    }
  }

  /// Parse a space name, accepting aliases (`xyz-d65` -> `xyz`,
  /// `rgb` -> `srgb`)
  pub fn from_name(name: &str) -> Option<ColorSpace> {
    Some(match name.to_ascii_lowercase().as_str() {
      "srgb" | "rgb" => ColorSpace::Srgb,
      "srgb-linear" | "srgb-linear " => ColorSpace::SrgbLinear,
      "display-p3" => ColorSpace::DisplayP3,
      "a98-rgb" => ColorSpace::A98Rgb,
      "prophoto-rgb" => ColorSpace::ProphotoRgb,
      "rec2020" => ColorSpace::Rec2020,
      "xyz" | "xyz-d65" => ColorSpace::Xyz,
      "xyz-d50" => ColorSpace::XyzD50,
      "hsl" => ColorSpace::Hsl,
      "hwb" => ColorSpace::Hwb,
      "lab" => ColorSpace::Lab,
      "lch" => ColorSpace::Lch,
      "oklab" => ColorSpace::Oklab,
      "oklch" => ColorSpace::Oklch,
      _ => return None,
    })
  }

  /// The legacy spaces are the ones classic Sass functions accept
  pub fn is_legacy(&self) -> bool {
    matches!(self, ColorSpace::Srgb | ColorSpace::Hsl)
  }

  /// Whether a channel is an angle (hue)
  pub fn is_polar(&self) -> bool {
    matches!(
      self,
      ColorSpace::Hsl | ColorSpace::Hwb | ColorSpace::Lch | ColorSpace::Oklch
    )
  }

  /// Channel names in declaration order
  pub fn channels(&self) -> &'static [&'static str] {
    match self {
      ColorSpace::Srgb
      | ColorSpace::SrgbLinear
      | ColorSpace::DisplayP3
      | ColorSpace::A98Rgb
      | ColorSpace::ProphotoRgb
      | ColorSpace::Rec2020 => &["red", "green", "blue"],
      ColorSpace::Xyz | ColorSpace::XyzD50 => &["x", "y", "z"],
      ColorSpace::Hsl => &["hue", "saturation", "lightness"],
      ColorSpace::Hwb => &["hue", "whiteness", "blackness"],
      ColorSpace::Lab | ColorSpace::Oklab => &["lightness", "a", "b"],
      ColorSpace::Lch | ColorSpace::Oklch => &["lightness", "chroma", "hue"],
    }
  }

  /// The unit a channel carries when read back (`deg`/`%`/none)
  pub fn channel_unit(&self, channel: &str) -> Option<&'static str> {
    match (self, channel) {
      (_, "hue") => Some("deg"),
      (_, "lightness") | (_, "saturation") | (_, "whiteness") | (_, "blackness") => Some("%"),
      _ => None,
    }
  }

  /// Scale a raw channel value into its user-facing form
  /// (lightness/saturation-family stored 0-1 read back as %)
  pub fn channel_out(&self, channel: &str, v: f64) -> f64 {
    match channel {
      "lightness" | "saturation" | "whiteness" | "blackness" => v * 100.0,
      _ => v,
    }
  }

  /// Normalize a user-facing channel value into storage form
  pub fn channel_in(&self, channel: &str, v: f64) -> f64 {
    match channel {
      "lightness" | "saturation" | "whiteness" | "blackness" => v / 100.0,
      _ => v,
    }
  }

  /// Whether the space has bounded channels (the reference's `isBounded`).
  /// Only the XYZ spaces are unbounded; `color.to-gamut()` into an
  /// unbounded space is a no-op.
  pub fn is_bounded(&self) -> bool {
    !matches!(self, ColorSpace::Xyz | ColorSpace::XyzD50)
  }

  /// The `(min, max)` clamp range for channel `i`, or `None` when the
  /// channel is unbounded (hue angles, lab a/b, lch chroma) — matching
  /// the reference `LinearChannel` bounds used by `to-gamut($method: clip)`
  pub fn channel_bounds(&self, i: usize) -> Option<(f64, f64)> {
    match self {
      // rgb-family channels are stored 0-1 in modern colors
      ColorSpace::Srgb
      | ColorSpace::SrgbLinear
      | ColorSpace::DisplayP3
      | ColorSpace::A98Rgb
      | ColorSpace::ProphotoRgb
      | ColorSpace::Rec2020 => Some((0.0, 1.0)),
      ColorSpace::Hsl | ColorSpace::Hwb => match i {
        1 | 2 => Some((0.0, 1.0)),
        _ => None, // hue is an angle
      },
      ColorSpace::Lab | ColorSpace::Lch | ColorSpace::Oklab | ColorSpace::Oklch => {
        match i {
          0 => Some((0.0, 1.0)), // lightness
          // a/b and chroma are unbounded; chroma clamps at 0 from
          // below in the reference (negative chroma -> 0)
          1 if matches!(self, ColorSpace::Lch | ColorSpace::Oklch) => Some((0.0, f64::INFINITY)),
          _ => None,
        }
      }
      ColorSpace::Xyz | ColorSpace::XyzD50 => None,
    }
  }
}

/// A color living in a non-legacy space, stored in its native channels.
/// `None` channels are "missing" (written `none`); computations treat a
/// missing channel as 0 for conversion purposes but remember it.
#[derive(Debug, Clone)]
pub struct ModernColor {
  /// The space these channels belong to
  pub space: ColorSpace,
  /// Native channels (lightness/hue/red/... per the space)
  pub ch: [Option<f64>; 3],
  /// Alpha; `None` means `none` (missing)
  pub alpha: Option<f64>,
}

impl ModernColor {
  /// Create a modern color from (possibly missing) native channels
  pub fn new(space: ColorSpace, ch: [Option<f64>; 3], alpha: Option<f64>) -> Self {
    ModernColor { space, ch, alpha }
  }

  /// The effective numeric channel for math (missing -> 0)
  pub fn value_or_zero(&self, i: usize) -> f64 {
    self.ch[i].unwrap_or(0.0)
  }
}

// ---- transfer functions ----------------------------------------------------

/// sRGB-family opto-electronic transfer (also display-p3)
pub fn srgb_to_linear(c: f64) -> f64 {
  if c.abs() <= 0.04045 {
    c / 12.92
  } else {
    (c.signum()) * (((c.abs() + 0.055) / 1.055).powf(2.4))
  }
}

/// Linear sRGB channel -> gamma-encoded sRGB channel
pub fn linear_to_srgb(c: f64) -> f64 {
  if c.abs() <= 0.0031308 {
    c * 12.92
  } else {
    c.signum() * (1.055 * c.abs().powf(1.0 / 2.4) - 0.055)
  }
}

/// a98-rgb transfer: pure gamma 563/256
pub fn a98_to_linear(c: f64) -> f64 {
  c.signum() * c.abs().powf(563.0 / 256.0)
}

/// prophoto-rgb transfer: gamma 1.8 with a linear toe
pub fn prophoto_to_linear(c: f64) -> f64 {
  let a = c.abs();
  let lin = if a <= 16.0 / 512.0 {
    a / 16.0
  } else {
    a.powf(1.8)
  };
  c.signum() * lin
}

/// Linear channel -> ProPhoto gamma encoding
pub fn linear_to_prophoto(c: f64) -> f64 {
  let a = c.abs();
  let enc = if a >= 1.0 / 512.0 {
    a.powf(1.0 / 1.8)
  } else {
    16.0 * a
  };
  c.signum() * enc
}

/// rec2020 transfer — a pure 2.4 gamma per the updated CSS Color 4 draft
/// (the pinned reference uses `sign * pow(abs, 2.4)`, not the ITU-R
/// BT.2020 piecewise from earlier drafts)
pub fn rec2020_to_linear(c: f64) -> f64 {
  c.signum() * c.abs().powf(2.4)
}

/// Linear channel -> Rec.2020 gamma encoding (pure 1/2.4 gamma)
pub fn linear_to_rec2020(c: f64) -> f64 {
  c.signum() * c.abs().powf(1.0 / 2.4)
}

/// CIE Lab constants (dart `labKappa` / `labEpsilon`)
const KAPPA: f64 = 24389.0 / 27.0;
/// CIE Lab constants (dart `labEpsilon`)
const EPS216: f64 = 216.0 / 24389.0;

fn apply(c: [f64; 3], m: [[f64; 3]; 3]) -> [f64; 3] {
  [
    m[0][0] * c[0] + m[0][1] * c[1] + m[0][2] * c[2],
    m[1][0] * c[0] + m[1][1] * c[1] + m[1][2] * c[2],
    m[2][0] * c[0] + m[2][1] * c[1] + m[2][2] * c[2],
  ]
}

// ---- matrices --------------------------------------------------------------

/// linear sRGB -> XYZ D65 (validated: red -> X 0.4123907992659595)
const SRGB_LINEAR_TO_XYZ: [[f64; 3]; 3] = [
  [0.4123907992659595, 0.35758433938387796, 0.1804807884018343],
  [0.21263900587151036, 0.7151686787677559, 0.07219231536073371],
  [0.01933081871559185, 0.11919477979462599, 0.9505321522496606],
];
const XYZ_TO_SRGB_LINEAR: [[f64; 3]; 3] = [
  [3.2409699419045213, -1.5373831775700935, -0.4986107602930033],
  [-0.9692436362808798, 1.8759675015077206, 0.04155505740717561],
  [
    0.05563007969699361,
    -0.20397695888897657,
    1.0569715142428786,
  ],
];

const P3_LINEAR_TO_XYZ: [[f64; 3]; 3] = [
  [0.4865709486482162, 0.26566769316909306, 0.19821728523436],
  [0.2289745640697488, 0.6917385218365064, 0.079286914093745],
  [0.0, 0.04511338185890264, 1.043944368900976],
];
const XYZ_TO_P3_LINEAR: [[f64; 3]; 3] = [
  [2.493496911941425, -0.9313836179191239, -0.40271078445071684],
  [
    -0.8294889695615747,
    1.7626640603183463,
    0.023624685841943577,
  ],
  [
    0.03584583024378447,
    -0.07617238926804182,
    0.9568845240076872,
  ],
];

const A98_LINEAR_TO_XYZ: [[f64; 3]; 3] = [
  [0.5766690429101305, 0.1855582379065463, 0.1882286462349947],
  [0.29734497525053605, 0.6273635662554661, 0.07529145849399788],
  [0.02703136138641234, 0.07068885253582723, 0.9913375368376388],
];
const XYZ_TO_A98_LINEAR: [[f64; 3]; 3] = [
  [
    2.0415879038107465,
    -0.5650069742788596,
    -0.34473135077831457,
  ],
  [-0.9692436362808795, 1.8759675015077202, 0.04155505740717557],
  [
    0.013444280632031142,
    -0.11836239223101838,
    1.0151749943912054,
  ],
];

const PROPHOTO_LINEAR_TO_XYZ: [[f64; 3]; 3] = [
  [0.7921669361445939, 0.14538434250139826, 0.04982283552342206],
  [0.3083038659680947, 0.5694849380074766, 0.11969188197462646],
  [0.0, 0.0506736286815106, 0.9055567198800842],
];
const XYZ_TO_PROPHOTO_LINEAR: [[f64; 3]; 3] = [
  [
    1.3457868816471583,
    -0.25557208737979464,
    -0.05110186497554526,
  ],
  [-0.5446307051249019, 1.5082477428451468, 0.02052744743642139],
  [0.0, 0.0, 1.2119675456389454],
];

const REC2020_LINEAR_TO_XYZ: [[f64; 3]; 3] = [
  [0.6369580483012914, 0.14461690358620832, 0.16888097516417205],
  [0.2627002120112671, 0.6779980715188708, 0.05930171646986196],
  [0.0, 0.028072693049087428, 1.060985057710791],
];
const XYZ_TO_REC2020_LINEAR: [[f64; 3]; 3] = [
  [1.716651187971268, -0.355670783776392, -0.2533662815874327],
  [-0.6666843518324893, 1.6164812366349395, 0.01576854581391113],
  [
    0.017639857445310783,
    -0.042770613257808524,
    0.9421031212354739,
  ],
];

/// Bradford chromatic adaptation D65 -> D50
const XYZ_D65_TO_D50: [[f64; 3]; 3] = [
  [
    1.0479298208405488,
    0.022946793341019088,
    -0.05019222954313557,
  ],
  [
    0.029627815688159344,
    0.990434484573249,
    -0.01707382502938514,
  ],
  [
    -0.009243058152591178,
    0.015055144896577895,
    0.7518742899580008,
  ],
];
const XYZ_D50_TO_D65: [[f64; 3]; 3] = [
  [
    0.9554734527042182,
    -0.023098536874261423,
    0.0632593086610217,
  ],
  [
    -0.028369706963208136,
    1.0099954580058226,
    0.021041398966943008,
  ],
  [
    0.012314001688319899,
    -0.020507696433477912,
    1.3303659366080753,
  ],
];

/// D50 reference white
const D50: [f64; 3] = [0.9642956764295946, 1.0, 0.8251046025104602];

// ---- Oklab -----------------------------------------------------------------

const XYZ_D65_TO_LMS: [[f64; 3]; 3] = [
  [0.819022437996703, 0.3619062600528904, -0.1288737815209879],
  [0.0329836539323885, 0.9292868615863434, 0.0361446663506424],
  [0.0481771893596242, 0.2642395317527308, 0.6335478284694309],
];
const LMS_TO_XYZ_D65: [[f64; 3]; 3] = [
  [1.2268798758459243, -0.5578149944602171, 0.2813910456659646],
  [
    -0.04057574521480084,
    1.1122868032803173,
    -0.07171105806551635,
  ],
  [
    -0.07637293667466007,
    -0.42149333240224324,
    1.5869240198367818,
  ],
];

// ---- hub: dart-faithful conversion routing ---------------------------------
//
// The pinned reference (dart-sass 1.106.1) converts between spaces with
// precomputed direct matrices per space pair and a dedicated LMS hub for
// the Oklab family (`math.pow(abs, 1/3)` cube roots, not `cbrt`). This
// module mirrors that routing so conversions agree with the reference to
// the last representable digit for the audited spaces.

/// dart's `math.pow(x, 3)`
fn pow3(x: f64) -> f64 {
  x.powf(3.0)
}

/// dart's `_cubeRootPreservingSign`: `math.pow(|x|, 1/3) * sign`
fn cbrt_signed(x: f64) -> f64 {
  x.abs().powf(1.0 / 3.0) * x.signum()
}

/// dart's LMS matrix row-major 9-tuples (exact values from conversions.dart)
const OKLAB_TO_LMS: [[f64; 3]; 3] = [
  [1.0000000000000002, 0.3963377773761749, 0.2158037573099136],
  [
    0.9999999999999998,
    -0.10556134581565854,
    -0.06385417282581334,
  ],
  [0.9999999999999999, -0.0894841775298118, -1.2914855480194094],
];
const LMS_TO_OKLAB: [[f64; 3]; 3] = [
  [0.210454268309314, 0.7936177747023054, -0.0040720430116193],
  [1.9779985324311684, -2.42859224204858, 0.450593709617411],
  [0.0259040424655478, 0.7827717124575296, -0.8086757549230774],
];
const LINEAR_SRGB_TO_LMS: [[f64; 3]; 3] = [
  [0.412221469470763, 0.5363325372617348, 0.0514459932675022],
  [0.2119034958178252, 0.6806995506452342, 0.1073969535369405],
  [0.08830245919005641, 0.2817188391361215, 0.6299787016738221],
];
const LMS_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
  [4.076741636075958, -3.307711539258062, 0.23096990318210417],
  [-1.268437973285032, 2.609757349287689, -0.3413193760026571],
  [-0.00419607613867551, -0.7034186179359363, 1.707614694074612],
];
const LINEAR_SRGB_TO_XYZ_D50: [[f64; 3]; 3] = [
  [0.43606574687426936, 0.3851515095901596, 0.14307841996513868],
  [0.22249317711056518, 0.7168870130944824, 0.06061980979495235],
  [0.01392392146316939, 0.09708132423141015, 0.7140993568158807],
];
const XYZ_D50_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
  [3.1341358529001178, -1.617385998018042, -0.49066221791109754],
  [-0.9787954765557777, 1.9162543773959884, 0.03344287339036693],
  [0.07195539255794733, -0.228976759815182, 1.4053860351131182],
];
const XYZ_D50_TO_LMS: [[f64; 3]; 3] = [
  [
    0.7707000420431172,
    0.34924840261939616,
    -0.11202351884164681,
  ],
  [0.00559649248368848, 0.9370723401136769, 0.06972568836252771],
  [0.04633714262191069, 0.25277531574310524, 0.851458076746796],
];
const LMS_TO_XYZ_D50: [[f64; 3]; 3] = [
  [1.288586218172706, -0.5378717444973745, 0.2135812027542364],
  [
    -0.00253387643187372,
    1.0923167988719165,
    -0.08978292244004273,
  ],
  [
    -0.06937382305734124,
    -0.29500839894431263,
    1.1894868245121142,
  ],
];
const LINEAR_SRGB_TO_LINEAR_DISPLAY_P3: [[f64; 3]; 3] = [
  [0.8224619687143623, 0.17753803128563775, 0.0],
  [0.03319419885096161, 0.9668058011490384, 0.0],
  [0.01708263072112003, 0.07239744066396346, 0.9105199286149165],
];
const LINEAR_DISPLAY_P3_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
  [1.2249401762805598, -0.22494017628055996, 0.0],
  [-0.04205695470968816, 1.042056954709688, 0.0],
  [
    -0.01963755459033443,
    -0.07863604555063188,
    1.0982736001409663,
  ],
];
const LINEAR_SRGB_TO_LINEAR_A98: [[f64; 3]; 3] = [
  [0.7151256068556247, 0.28487439314437535, 0.0],
  [0.0, 1.0, 0.0],
  [0.0, 0.04116194845011846, 0.9588380515498816],
];
const LINEAR_A98_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
  [1.3983557439607783, -0.3983557439607783, 0.0],
  [0.0, 1.0, 0.0],
  [0.0, -0.04292898929447326, 1.0429289892944733],
];
const LINEAR_SRGB_TO_LINEAR_PROPHOTO: [[f64; 3]; 3] = [
  [0.5292769776226116, 0.33015450197849283, 0.14056852039889556],
  [0.09836585954044917, 0.8734707129069618, 0.028163427552589],
  [0.01687534092138684, 0.11765941425612084, 0.8654652448224923],
];
const LINEAR_PROPHOTO_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
  [2.034414882493475, -0.7276357899341342, -0.3067450595828618],
  [
    -0.22882573163305037,
    1.2317425411901048,
    -0.00291680955705449,
  ],
  [
    -0.00855882878391742,
    -0.1532667021380372,
    1.1618255309219547,
  ],
];
const LINEAR_SRGB_TO_LINEAR_REC2020: [[f64; 3]; 3] = [
  [0.627403895934699, 0.3292830383778837, 0.04331306568741722],
  [0.06909728935823208, 0.9195403950754587, 0.01136231556630917],
  [0.01639143887515027, 0.08801330787722575, 0.895595253247624],
];
const LINEAR_REC2020_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
  [
    1.6604910021084345,
    -0.5876411387885495,
    -0.07284986331988487,
  ],
  [
    -0.12455047452159074,
    1.1328998971259603,
    -0.00834942260436947,
  ],
  [
    -0.0181507633549053,
    -0.10057889800800737,
    1.1187296613629127,
  ],
];

/// dart computes the D50 white from xy chromaticity at runtime
fn d50_white() -> [f64; 3] {
  [0.3457 / 0.3585, 1.0, (1.0 - 0.3457 - 0.3585) / 0.3585]
}

/// CIE Lab -> XYZ (D50), L on the 0-100 scale like the reference
fn lab_to_xyz_d50(lab01: [f64; 3]) -> [f64; 3] {
  let d50 = d50_white();
  let l = lab01[0] * 100.0;
  let a = lab01[1];
  let b = lab01[2];
  let fy = (l + 16.0) / 116.0;
  let fx = fy + a / 500.0;
  let fz = fy - b / 200.0;
  // Y comes from lightness directly (reference lab.dart)
  let yr = if l > KAPPA * EPS216 {
    pow3(fy)
  } else {
    l / KAPPA
  };
  let x = f_to_xz(fx) * d50[0];
  let z = f_to_xz(fz) * d50[2];
  [x, yr * d50[1], z]
}

/// Reference `_convertFToXorZ`
fn f_to_xz(component: f64) -> f64 {
  let cubed = pow3(component);
  if cubed > EPS216 {
    cubed
  } else {
    (116.0 * component - 16.0) / KAPPA
  }
}

/// XYZ (D50) -> CIE Lab, L stored 0-1
fn xyz_d50_to_lab(xyz: [f64; 3]) -> [f64; 3] {
  let d50 = d50_white();
  let f = |c: f64| -> f64 {
    if c > EPS216 {
      c.powf(1.0 / 3.0)
    } else {
      (KAPPA * c + 16.0) / 116.0
    }
  };
  let f0 = f(xyz[0] / d50[0]);
  let f1 = f(xyz[1] / d50[1]);
  let f2 = f(xyz[2] / d50[2]);
  [
    (116.0 * f1 - 16.0) / 100.0,
    500.0 * (f0 - f1),
    200.0 * (f1 - f2),
  ]
}

/// LMS -> Oklab channels (cube roots then matrix)
fn lms_to_oklab(lms: [f64; 3]) -> [f64; 3] {
  let scaled = [
    cbrt_signed(lms[0]),
    cbrt_signed(lms[1]),
    cbrt_signed(lms[2]),
  ];
  apply(scaled, LMS_TO_OKLAB)
}

/// Oklab channels -> LMS (matrix then cubes)
fn oklab_to_lms(lab: [f64; 3]) -> [f64; 3] {
  let lms = apply(lab, OKLAB_TO_LMS);
  [pow3(lms[0]), pow3(lms[1]), pow3(lms[2])]
}

/// Convert linear-sRGB channels into the given space's native channels
pub fn from_srgb_linear(lin: [f64; 3], space: ColorSpace) -> [f64; 3] {
  match space {
    ColorSpace::SrgbLinear => lin,
    ColorSpace::Srgb => [
      linear_to_srgb(lin[0]),
      linear_to_srgb(lin[1]),
      linear_to_srgb(lin[2]),
    ],
    ColorSpace::DisplayP3 => {
      let v = apply(lin, LINEAR_SRGB_TO_LINEAR_DISPLAY_P3);
      [
        linear_to_srgb(v[0]),
        linear_to_srgb(v[1]),
        linear_to_srgb(v[2]),
      ]
    }
    ColorSpace::A98Rgb => {
      let v = apply(lin, LINEAR_SRGB_TO_LINEAR_A98);
      [
        linear_to_a98(v[0]),
        linear_to_a98(v[1]),
        linear_to_a98(v[2]),
      ]
    }
    ColorSpace::ProphotoRgb => {
      let v = apply(lin, LINEAR_SRGB_TO_LINEAR_PROPHOTO);
      [
        linear_to_prophoto(v[0]),
        linear_to_prophoto(v[1]),
        linear_to_prophoto(v[2]),
      ]
    }
    ColorSpace::Rec2020 => {
      let v = apply(lin, LINEAR_SRGB_TO_LINEAR_REC2020);
      [
        linear_to_rec2020(v[0]),
        linear_to_rec2020(v[1]),
        linear_to_rec2020(v[2]),
      ]
    }
    ColorSpace::Xyz => apply(lin, SRGB_LINEAR_TO_XYZ),
    ColorSpace::XyzD50 => apply(lin, LINEAR_SRGB_TO_XYZ_D50),
    ColorSpace::Lab => xyz_d50_to_lab(apply(lin, LINEAR_SRGB_TO_XYZ_D50)),
    ColorSpace::Lch => lab_to_lch(xyz_d50_to_lab(apply(lin, LINEAR_SRGB_TO_XYZ_D50))),
    ColorSpace::Oklab => lms_to_oklab(apply(lin, LINEAR_SRGB_TO_LMS)),
    ColorSpace::Oklch => lab_to_lch(lms_to_oklab(apply(lin, LINEAR_SRGB_TO_LMS))),
    ColorSpace::Hsl => {
      let (h, s, l) = linear_rgb_to_hsl([
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ]);
      [h, s, l]
    }
    ColorSpace::Hwb => {
      let rgb = [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ];
      let (h, w, b) = rgb_to_hwb(rgb);
      [h, w, b]
    }
  }
}

/// Convert a space's native channels into linear-sRGB
pub fn to_srgb_linear(ch: [f64; 3], space: ColorSpace) -> [f64; 3] {
  match space {
    ColorSpace::SrgbLinear => ch,
    ColorSpace::Srgb => [
      srgb_to_linear(ch[0]),
      srgb_to_linear(ch[1]),
      srgb_to_linear(ch[2]),
    ],
    ColorSpace::DisplayP3 => {
      let lin = [
        srgb_to_linear(ch[0]),
        srgb_to_linear(ch[1]),
        srgb_to_linear(ch[2]),
      ];
      apply(lin, LINEAR_DISPLAY_P3_TO_LINEAR_SRGB)
    }
    ColorSpace::A98Rgb => {
      let lin = [
        a98_to_linear(ch[0]),
        a98_to_linear(ch[1]),
        a98_to_linear(ch[2]),
      ];
      apply(lin, LINEAR_A98_TO_LINEAR_SRGB)
    }
    ColorSpace::ProphotoRgb => {
      let lin = [
        prophoto_to_linear(ch[0]),
        prophoto_to_linear(ch[1]),
        prophoto_to_linear(ch[2]),
      ];
      apply(lin, LINEAR_PROPHOTO_TO_LINEAR_SRGB)
    }
    ColorSpace::Rec2020 => {
      let lin = [
        rec2020_to_linear(ch[0]),
        rec2020_to_linear(ch[1]),
        rec2020_to_linear(ch[2]),
      ];
      apply(lin, LINEAR_REC2020_TO_LINEAR_SRGB)
    }
    ColorSpace::Xyz => apply(ch, XYZ_TO_SRGB_LINEAR),
    ColorSpace::XyzD50 => apply(ch, XYZ_D50_TO_LINEAR_SRGB),
    ColorSpace::Lab => apply(lab_to_xyz_d50(ch), XYZ_D50_TO_LINEAR_SRGB),
    ColorSpace::Lch => apply(lab_to_xyz_d50(lch_to_lab(ch)), XYZ_D50_TO_LINEAR_SRGB),
    ColorSpace::Oklab => apply(oklab_to_lms(ch), LMS_TO_LINEAR_SRGB),
    ColorSpace::Oklch => apply(oklab_to_lms(lch_to_lab(ch)), LMS_TO_LINEAR_SRGB),
    ColorSpace::Hsl => {
      let rgb = hsl_to_linear_rgb(ch);
      [
        srgb_to_linear(rgb[0]),
        srgb_to_linear(rgb[1]),
        srgb_to_linear(rgb[2]),
      ]
    }
    ColorSpace::Hwb => {
      let rgb = hwb_to_rgb(ch);
      [
        srgb_to_linear(rgb[0]),
        srgb_to_linear(rgb[1]),
        srgb_to_linear(rgb[2]),
      ]
    }
  }
}

/// Convert native channels between two spaces following the reference's
/// routing (LMS hub for the Oklab family, XYZ D50 for the Lab family,
/// direct matrices between linear spaces)
pub fn convert_native(ch: [f64; 3], from: ColorSpace, to: ColorSpace) -> [f64; 3] {
  if from == to {
    return ch;
  }
  match from {
    ColorSpace::Oklch => convert_native(lch_to_lab(ch), ColorSpace::Oklab, to),
    ColorSpace::Oklab => {
      if to == ColorSpace::Oklch {
        return lab_to_lch(ch);
      }
      convert_from_lms(oklab_to_lms(ch), to)
    }
    ColorSpace::Lch => convert_native(lch_to_lab(ch), ColorSpace::Lab, to),
    ColorSpace::Lab => {
      if to == ColorSpace::Lch {
        return lab_to_lch(ch);
      }
      convert_from_xyz_d50(lab_to_xyz_d50(ch), to)
    }
    ColorSpace::Hsl => convert_native(hsl_to_linear_rgb(ch), ColorSpace::Srgb, to),
    ColorSpace::Hwb => convert_native(hwb_to_rgb(ch), ColorSpace::Srgb, to),
    ColorSpace::Srgb => convert_from_lin(
      [
        srgb_to_linear(ch[0]),
        srgb_to_linear(ch[1]),
        srgb_to_linear(ch[2]),
      ],
      to,
    ),
    ColorSpace::SrgbLinear => convert_from_lin(ch, to),
    ColorSpace::DisplayP3 => convert_from_lin(
      [
        srgb_to_linear(ch[0]),
        srgb_to_linear(ch[1]),
        srgb_to_linear(ch[2]),
      ],
      to,
    ),
    ColorSpace::A98Rgb => convert_from_lin(
      [
        a98_to_linear(ch[0]),
        a98_to_linear(ch[1]),
        a98_to_linear(ch[2]),
      ],
      to,
    ),
    ColorSpace::ProphotoRgb => convert_from_lin(
      [
        prophoto_to_linear(ch[0]),
        prophoto_to_linear(ch[1]),
        prophoto_to_linear(ch[2]),
      ],
      to,
    ),
    ColorSpace::Rec2020 => convert_from_lin(
      [
        rec2020_to_linear(ch[0]),
        rec2020_to_linear(ch[1]),
        rec2020_to_linear(ch[2]),
      ],
      to,
    ),
    ColorSpace::Xyz => convert_from_xyz_d65(ch, to),
    ColorSpace::XyzD50 => convert_from_xyz_d50(ch, to),
  }
}

/// LMS -> destination (reference `LmsColorSpace.convert`)
fn convert_from_lms(lms: [f64; 3], to: ColorSpace) -> [f64; 3] {
  match to {
    ColorSpace::Oklab | ColorSpace::Oklch | ColorSpace::Lch | ColorSpace::Lab => {
      let oklab = lms_to_oklab(lms);
      match to {
        ColorSpace::Oklch | ColorSpace::Lch => lab_to_lch(oklab),
        _ => oklab,
      }
    }
    ColorSpace::Srgb
    | ColorSpace::SrgbLinear
    | ColorSpace::DisplayP3
    | ColorSpace::A98Rgb
    | ColorSpace::ProphotoRgb
    | ColorSpace::Rec2020 => {
      let lin = match to {
        ColorSpace::SrgbLinear | ColorSpace::Srgb => apply(lms, LMS_TO_LINEAR_SRGB),
        // These pairs go through linear sRGB (direct LMS matrices are
        // only needed for the audited spaces)
        _ => {
          let linsrgb = apply(lms, LMS_TO_LINEAR_SRGB);
          match to {
            ColorSpace::DisplayP3 => apply(linsrgb, LINEAR_SRGB_TO_LINEAR_DISPLAY_P3),
            ColorSpace::A98Rgb => apply(linsrgb, LINEAR_SRGB_TO_LINEAR_A98),
            ColorSpace::ProphotoRgb => apply(linsrgb, LINEAR_SRGB_TO_LINEAR_PROPHOTO),
            _ => apply(linsrgb, LINEAR_SRGB_TO_LINEAR_REC2020),
          }
        }
      };
      match to {
        ColorSpace::SrgbLinear => lin,
        _ => [
          linear_to_srgb(lin[0]),
          linear_to_srgb(lin[1]),
          linear_to_srgb(lin[2]),
        ],
      }
    }
    ColorSpace::Hsl | ColorSpace::Hwb => {
      let lin = apply(lms, LMS_TO_LINEAR_SRGB);
      let rgb = [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ];
      if to == ColorSpace::Hsl {
        let (h, s, l) = linear_rgb_to_hsl(rgb);
        [h, s, l]
      } else {
        let (h, w, b) = rgb_to_hwb(rgb);
        [h, w, b]
      }
    }
    ColorSpace::Xyz => apply(lms, LMS_TO_XYZ_D65),
    ColorSpace::XyzD50 => apply(lms, LMS_TO_XYZ_D50),
  }
}

/// XYZ D65 -> destination
fn convert_from_xyz_d65(xyz: [f64; 3], to: ColorSpace) -> [f64; 3] {
  match to {
    ColorSpace::Xyz => xyz,
    ColorSpace::XyzD50 => apply(xyz, XYZ_D65_TO_D50),
    ColorSpace::Lab | ColorSpace::Lch => {
      let lab = xyz_d50_to_lab(apply(xyz, XYZ_D65_TO_D50));
      if to == ColorSpace::Lch {
        lab_to_lch(lab)
      } else {
        lab
      }
    }
    ColorSpace::Oklab | ColorSpace::Oklch => {
      let oklab = lms_to_oklab(apply(xyz, XYZ_D65_TO_LMS));
      if to == ColorSpace::Oklch {
        lab_to_lch(oklab)
      } else {
        oklab
      }
    }
    ColorSpace::SrgbLinear => apply(xyz, XYZ_TO_SRGB_LINEAR),
    ColorSpace::Srgb => {
      let lin = apply(xyz, XYZ_TO_SRGB_LINEAR);
      [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ]
    }
    ColorSpace::DisplayP3 => {
      let lin = apply(xyz, XYZ_TO_P3_LINEAR);
      [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ]
    }
    ColorSpace::A98Rgb => {
      let lin = apply(xyz, XYZ_TO_A98_LINEAR);
      [
        linear_to_a98(lin[0]),
        linear_to_a98(lin[1]),
        linear_to_a98(lin[2]),
      ]
    }
    ColorSpace::ProphotoRgb => {
      let lin = apply(apply(xyz, XYZ_D65_TO_D50), XYZ_TO_PROPHOTO_LINEAR);
      [
        linear_to_prophoto(lin[0]),
        linear_to_prophoto(lin[1]),
        linear_to_prophoto(lin[2]),
      ]
    }
    ColorSpace::Rec2020 => {
      let lin = apply(xyz, XYZ_TO_REC2020_LINEAR);
      [
        linear_to_rec2020(lin[0]),
        linear_to_rec2020(lin[1]),
        linear_to_rec2020(lin[2]),
      ]
    }
    ColorSpace::Hsl | ColorSpace::Hwb => {
      let lin = apply(xyz, XYZ_TO_SRGB_LINEAR);
      let rgb = [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ];
      if to == ColorSpace::Hsl {
        let (h, s, l) = linear_rgb_to_hsl(rgb);
        [h, s, l]
      } else {
        let (h, w, b) = rgb_to_hwb(rgb);
        [h, w, b]
      }
    }
  }
}

/// XYZ D50 -> destination
fn convert_from_xyz_d50(xyz: [f64; 3], to: ColorSpace) -> [f64; 3] {
  match to {
    ColorSpace::XyzD50 => xyz,
    ColorSpace::Xyz => apply(xyz, XYZ_D50_TO_D65),
    ColorSpace::Lab | ColorSpace::Lch => {
      let lab = xyz_d50_to_lab(xyz);
      if to == ColorSpace::Lch {
        lab_to_lch(lab)
      } else {
        lab
      }
    }
    ColorSpace::Oklab | ColorSpace::Oklch => {
      let oklab = lms_to_oklab(apply(xyz, XYZ_D50_TO_LMS));
      if to == ColorSpace::Oklch {
        lab_to_lch(oklab)
      } else {
        oklab
      }
    }
    ColorSpace::SrgbLinear => apply(xyz, XYZ_D50_TO_LINEAR_SRGB),
    ColorSpace::Srgb => {
      let lin = apply(xyz, XYZ_D50_TO_LINEAR_SRGB);
      [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ]
    }
    ColorSpace::DisplayP3 => {
      // composed via linear sRGB (direct pair not needed by audited paths)
      let lin = apply(
        apply(xyz, XYZ_D50_TO_LINEAR_SRGB),
        LINEAR_SRGB_TO_LINEAR_DISPLAY_P3,
      );
      [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ]
    }
    ColorSpace::A98Rgb => {
      let lin = apply(
        apply(xyz, XYZ_D50_TO_LINEAR_SRGB),
        LINEAR_SRGB_TO_LINEAR_A98,
      );
      [
        linear_to_a98(lin[0]),
        linear_to_a98(lin[1]),
        linear_to_a98(lin[2]),
      ]
    }
    ColorSpace::ProphotoRgb => {
      let lin = apply(
        apply(xyz, XYZ_D50_TO_LINEAR_SRGB),
        LINEAR_SRGB_TO_LINEAR_PROPHOTO,
      );
      [
        linear_to_prophoto(lin[0]),
        linear_to_prophoto(lin[1]),
        linear_to_prophoto(lin[2]),
      ]
    }
    ColorSpace::Rec2020 => {
      let lin = apply(
        apply(xyz, XYZ_D50_TO_LINEAR_SRGB),
        LINEAR_SRGB_TO_LINEAR_REC2020,
      );
      [
        linear_to_rec2020(lin[0]),
        linear_to_rec2020(lin[1]),
        linear_to_rec2020(lin[2]),
      ]
    }
    ColorSpace::Hsl | ColorSpace::Hwb => {
      let lin = apply(xyz, XYZ_D50_TO_LINEAR_SRGB);
      let rgb = [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ];
      if to == ColorSpace::Hsl {
        let (h, s, l) = linear_rgb_to_hsl(rgb);
        [h, s, l]
      } else {
        let (h, w, b) = rgb_to_hwb(rgb);
        [h, w, b]
      }
    }
  }
}

/// Linear-space channels -> destination (reference `convertLinear`)
fn convert_from_lin(lin: [f64; 3], to: ColorSpace) -> [f64; 3] {
  match to {
    ColorSpace::SrgbLinear => lin,
    ColorSpace::Srgb => [
      linear_to_srgb(lin[0]),
      linear_to_srgb(lin[1]),
      linear_to_srgb(lin[2]),
    ],
    ColorSpace::DisplayP3 => {
      let v = apply(lin, LINEAR_SRGB_TO_LINEAR_DISPLAY_P3);
      [
        linear_to_srgb(v[0]),
        linear_to_srgb(v[1]),
        linear_to_srgb(v[2]),
      ]
    }
    ColorSpace::A98Rgb => {
      let v = apply(lin, LINEAR_SRGB_TO_LINEAR_A98);
      [
        linear_to_a98(v[0]),
        linear_to_a98(v[1]),
        linear_to_a98(v[2]),
      ]
    }
    ColorSpace::ProphotoRgb => {
      let v = apply(lin, LINEAR_SRGB_TO_LINEAR_PROPHOTO);
      [
        linear_to_prophoto(v[0]),
        linear_to_prophoto(v[1]),
        linear_to_prophoto(v[2]),
      ]
    }
    ColorSpace::Rec2020 => {
      let v = apply(lin, LINEAR_SRGB_TO_LINEAR_REC2020);
      [
        linear_to_rec2020(v[0]),
        linear_to_rec2020(v[1]),
        linear_to_rec2020(v[2]),
      ]
    }
    ColorSpace::Xyz => apply(lin, SRGB_LINEAR_TO_XYZ),
    ColorSpace::XyzD50 => apply(lin, LINEAR_SRGB_TO_XYZ_D50),
    ColorSpace::Lab => xyz_d50_to_lab(apply(lin, LINEAR_SRGB_TO_XYZ_D50)),
    ColorSpace::Lch => lab_to_lch(xyz_d50_to_lab(apply(lin, LINEAR_SRGB_TO_XYZ_D50))),
    ColorSpace::Oklab => lms_to_oklab(apply(lin, LINEAR_SRGB_TO_LMS)),
    ColorSpace::Oklch => lab_to_lch(lms_to_oklab(apply(lin, LINEAR_SRGB_TO_LMS))),
    ColorSpace::Hsl => {
      let rgb = [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ];
      let (h, s, l) = linear_rgb_to_hsl(rgb);
      [h, s, l]
    }
    ColorSpace::Hwb => {
      let rgb = [
        linear_to_srgb(lin[0]),
        linear_to_srgb(lin[1]),
        linear_to_srgb(lin[2]),
      ];
      let (h, w, b) = rgb_to_hwb(rgb);
      [h, w, b]
    }
  }
}

/// Linear channel -> Adobe 98 gamma encoding
pub fn linear_to_a98(c: f64) -> f64 {
  c.signum() * c.abs().powf(256.0 / 563.0)
}

// ---- Lab / Lch -------------------------------------------------------------

/// Lab-style LCH with `deg` hue in degrees
pub fn lab_to_lch(lab: [f64; 3]) -> [f64; 3] {
  let c = (lab[1] * lab[1] + lab[2] * lab[2]).sqrt();
  let mut h = lab[2].atan2(lab[1]).to_degrees();
  if h < 0.0 {
    h += 360.0;
  }
  [lab[0], c, h]
}

/// LCH (hue in degrees) -> Lab
pub fn lch_to_lab(lch: [f64; 3]) -> [f64; 3] {
  let rad = lch[2].to_radians();
  [lch[0], lch[1] * rad.cos(), lch[1] * rad.sin()]
}

// ---- Oklab / Oklch ---------------------------------------------------------

// ---- HSL / HWB (0-1 scales) ------------------------------------------------

/// sRGB (0-1) -> HSL with h in degrees, s/l in 0-1
pub fn linear_rgb_to_hsl(rgb: [f64; 3]) -> (f64, f64, f64) {
  let max = rgb[0].max(rgb[1]).max(rgb[2]);
  let min = rgb[0].min(rgb[1]).min(rgb[2]);
  let l = (max + min) / 2.0;
  if max == min {
    return (0.0, 0.0, l);
  }
  let d = max - min;
  let s = if l > 0.5 {
    d / (2.0 - max - min)
  } else {
    d / (max + min)
  };
  let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
  let h = if max == r {
    60.0 * (((g - b) / d) + if g < b { 6.0 } else { 0.0 })
  } else if max == g {
    60.0 * (((b - r) / d) + 2.0)
  } else {
    60.0 * (((r - g) / d) + 4.0)
  };
  (h, s, l)
}

/// HSL (h degrees, s/l 0-1) -> sRGB (0-1)
pub fn hsl_to_linear_rgb(hsl: [f64; 3]) -> [f64; 3] {
  let (h, s, l) = (hsl[0], hsl[1], hsl[2]);
  let hd = h / 360.0;
  let sd = s;
  let ld = l;
  if sd == 0.0 {
    return [ld, ld, ld];
  }
  // CSS Color 4 `f(n)` conversion, matching the reference bit-for-bit:
  // `a = s * min(l, 1-l)` may go negative for out-of-range lightness
  // (`hsl(0 200% 150%)`), which the older q/p formulation mishandles.
  let f = |n: f64| -> f64 {
    let k = (n + hd * 12.0) % 12.0;
    let a = sd * ld.min(1.0 - ld);
    ld - a * (-1.0f64).max((k - 3.0).min(9.0 - k).min(1.0))
  };
  [f(0.0), f(8.0), f(4.0)]
}

/// sRGB (0-1) -> HWB (h degrees, w/b 0-1)
pub fn rgb_to_hwb(rgb: [f64; 3]) -> (f64, f64, f64) {
  let (h, _s, _l) = linear_rgb_to_hsl(rgb);
  let w = rgb[0].min(rgb[1]).min(rgb[2]);
  let b = 1.0 - rgb[0].max(rgb[1]).max(rgb[2]);
  (h, w, b)
}

/// HWB -> sRGB (0-1)
pub fn hwb_to_rgb(hwb: [f64; 3]) -> [f64; 3] {
  let (h, w, b) = (hwb[0], hwb[1], hwb[2]);
  if w + b >= 1.0 {
    let gray = w / (w + b);
    return [gray, gray, gray];
  }
  let base = hsl_to_linear_rgb([h, 1.0, 0.5]);
  let mut out = [0.0; 3];
  for i in 0..3 {
    out[i] = base[i] * (1.0 - w - b) + w;
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Reference values validated against dart-sass 1.106.1
  #[test]
  fn red_to_modern_spaces_matches_reference() {
    let lin = [1.0, 0.0, 0.0];
    let oklch = from_srgb_linear(lin, ColorSpace::Oklch);
    // oklch(62.79553639214313% 0.25768330380536064 29.233880279627897deg)
    assert!(
      (oklch[0] * 100.0 - 62.79553639214313).abs() < 1e-6,
      "{:?}",
      oklch
    );
    assert!((oklch[1] - 0.25768330380536064).abs() < 1e-6, "{:?}", oklch);
    assert!((oklch[2] - 29.233880279627897).abs() < 1e-4, "{:?}", oklch);
    let lab = from_srgb_linear(lin, ColorSpace::Lab);
    // lab(54.29054140467191% 80.80492817043522 69.89096476862427)
    assert!(
      (lab[0] * 100.0 - 54.29054140467191).abs() < 1e-4,
      "{:?}",
      lab
    );
    assert!((lab[1] - 80.80492817043522).abs() < 1e-4, "{:?}", lab);
    assert!((lab[2] - 69.89096476862427).abs() < 1e-4, "{:?}", lab);
    let xyz = from_srgb_linear(lin, ColorSpace::Xyz);
    assert!((xyz[0] - 0.4123907992659595).abs() < 1e-12);
    let p3 = from_srgb_linear(lin, ColorSpace::DisplayP3);
    assert!((p3[0] - 0.9174875573251656).abs() < 1e-12, "{:?}", p3);
    // Round trip
    let back = to_srgb_linear(oklch, ColorSpace::Oklch);
    assert!((back[0] - 1.0).abs() < 1e-9 && back[1].abs() < 1e-9 && back[2].abs() < 1e-9);
  }
  #[test]
  fn every_color_space_exposes_consistent_metadata() {
    let spaces = [
      ColorSpace::Srgb,
      ColorSpace::SrgbLinear,
      ColorSpace::DisplayP3,
      ColorSpace::A98Rgb,
      ColorSpace::ProphotoRgb,
      ColorSpace::Rec2020,
      ColorSpace::Xyz,
      ColorSpace::XyzD50,
      ColorSpace::Hsl,
      ColorSpace::Hwb,
      ColorSpace::Lab,
      ColorSpace::Lch,
      ColorSpace::Oklab,
      ColorSpace::Oklch,
    ];

    for space in spaces {
      assert_eq!(ColorSpace::from_name(space.name()), Some(space));
      assert!(!space.channels().is_empty());
      for channel in space.channels() {
        let value = 12.5;
        let stored = space.channel_in(channel, value);
        assert!((space.channel_out(channel, stored) - value).abs() < 1e-12);
        if *channel == "hue" {
          assert_eq!(space.channel_unit(channel), Some("deg"));
        }
      }
      assert_eq!(space.channel_unit("not-a-channel"), None);
      for index in 0..3 {
        let _ = space.channel_bounds(index);
      }
      let _ = space.is_legacy();
      let _ = space.is_polar();
      let _ = space.is_bounded();
    }

    assert_eq!(ColorSpace::from_name("RGB"), Some(ColorSpace::Srgb));
    assert_eq!(ColorSpace::from_name("xyz-d65"), Some(ColorSpace::Xyz));
    assert_eq!(ColorSpace::from_name("srgb-linear"), Some(ColorSpace::SrgbLinear));
    assert_eq!(ColorSpace::from_name("not-a-space"), None);

    assert_eq!(ColorSpace::Srgb.is_legacy(), true);
    assert_eq!(ColorSpace::Hsl.is_legacy(), true);
    assert_eq!(ColorSpace::DisplayP3.is_legacy(), false);
    assert_eq!(ColorSpace::Oklch.is_polar(), true);
    assert_eq!(ColorSpace::Lab.is_polar(), false);
    assert_eq!(ColorSpace::Xyz.is_bounded(), false);
    assert_eq!(ColorSpace::XyzD50.is_bounded(), false);
    assert_eq!(ColorSpace::Srgb.is_bounded(), true);
  }

  #[test]
  fn modern_color_missing_channels_read_as_zero() {
    let color = ModernColor::new(
      ColorSpace::Oklab,
      [Some(0.25), None, Some(-0.1)],
      None,
    );

    assert_eq!(color.value_or_zero(0), 0.25);
    assert_eq!(color.value_or_zero(1), 0.0);
    assert_eq!(color.value_or_zero(2), -0.1);
    assert_eq!(color.alpha, None);
  }

  #[test]
  fn transfer_functions_cover_linear_and_gamma_ranges() {
    assert!((srgb_to_linear(0.02) - 0.02 / 12.92).abs() < 1e-12);
    assert!(srgb_to_linear(0.5) > 0.0);
    assert!(srgb_to_linear(-0.5) < 0.0);
    assert!((linear_to_srgb(0.001) - 0.001 * 12.92).abs() < 1e-12);
    assert!(linear_to_srgb(0.5) > 0.0);
    assert!(linear_to_srgb(-0.5) < 0.0);

    assert!(a98_to_linear(0.5) > 0.0);
    assert!(a98_to_linear(-0.5) < 0.0);
    assert!(linear_to_a98(0.5) > 0.0);
    assert!(linear_to_a98(-0.5) < 0.0);

    assert!((prophoto_to_linear(0.01) - 0.01 / 16.0).abs() < 1e-12);
    assert!(prophoto_to_linear(0.5) > 0.0);
    assert!(prophoto_to_linear(-0.5) < 0.0);
    assert!(linear_to_prophoto(0.001) > 0.0);
    assert!(linear_to_prophoto(0.5) > 0.0);
    assert!(linear_to_prophoto(-0.5) < 0.0);

    assert!(rec2020_to_linear(0.5) > 0.0);
    assert!(rec2020_to_linear(-0.5) < 0.0);
    assert!(linear_to_rec2020(0.5) > 0.0);
    assert!(linear_to_rec2020(-0.5) < 0.0);
  }

  #[test]
  fn conversion_hub_exercises_every_source_and_destination_space() {
    let spaces = [
      ColorSpace::Srgb,
      ColorSpace::SrgbLinear,
      ColorSpace::DisplayP3,
      ColorSpace::A98Rgb,
      ColorSpace::ProphotoRgb,
      ColorSpace::Rec2020,
      ColorSpace::Xyz,
      ColorSpace::XyzD50,
      ColorSpace::Hsl,
      ColorSpace::Hwb,
      ColorSpace::Lab,
      ColorSpace::Lch,
      ColorSpace::Oklab,
      ColorSpace::Oklch,
    ];

    for from in spaces {
      for to in spaces {
        let converted = convert_native([0.25, 0.5, 0.75], from, to);
        assert!(
          converted.iter().all(|channel| channel.is_finite()),
          "non-finite conversion from {:?} to {:?}: {:?}",
          from,
          to,
          converted
        );
        if from == to {
          assert_eq!(converted, [0.25, 0.5, 0.75]);
        }
      }
    }
  }

  #[test]
  fn hsl_and_hwb_conversions_cover_achromatic_and_chromatic_edges() {
    let gray = linear_rgb_to_hsl([0.4, 0.4, 0.4]);
    assert_eq!(gray, (0.0, 0.0, 0.4));

    let red_high = linear_rgb_to_hsl([1.0, 0.5, 0.75]);
    assert!(red_high.0 > 0.0 && red_high.1 > 0.0 && red_high.2 > 0.5);
    let red_low = linear_rgb_to_hsl([1.0, 0.0, 0.0]);
    assert_eq!(red_low.0, 0.0);
    let green = linear_rgb_to_hsl([0.0, 1.0, 0.0]);
    assert_eq!(green.0, 120.0);
    let blue = linear_rgb_to_hsl([0.0, 0.0, 1.0]);
    assert_eq!(blue.0, 240.0);

    assert_eq!(hsl_to_linear_rgb([180.0, 0.0, 0.25]), [0.25, 0.25, 0.25]);
    let hsl_rgb = hsl_to_linear_rgb([210.0, 0.7, 0.6]);
    assert!(hsl_rgb.iter().all(|channel| channel.is_finite()));

    let hwb = rgb_to_hwb([0.2, 0.4, 0.8]);
    assert!(hwb.0 >= 0.0 && hwb.0 <= 360.0);
    assert_eq!(hwb_to_rgb([0.0, 0.8, 0.4]), [2.0 / 3.0; 3]);
    let rgb = hwb_to_rgb([60.0, 0.2, 0.3]);
    assert!(rgb.iter().all(|channel| channel.is_finite()));
  }

  #[test]
  fn lab_lch_conversion_handles_negative_hue_and_round_trips() {
    let lch = lab_to_lch([0.6, -0.2, -0.3]);
    assert!(lch[2] >= 0.0 && lch[2] < 360.0);
    let lab = lch_to_lab(lch);
    assert!((lab[0] - 0.6).abs() < 1e-12);
    assert!((lab[1] + 0.2).abs() < 1e-12);
    assert!((lab[2] + 0.3).abs() < 1e-12);
  }

}
