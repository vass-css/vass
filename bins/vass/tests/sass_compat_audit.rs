//! Regression tests for the sass-compatibility audit (dart-sass 1.106.1)
//!
//! Each test reproduces a confirmed discrepancy that was fixed at the root:
//! - `sass:list` separator metadata (trailing-comma lists, `()`/`[]`,
//!   join/append auto rules)
//! - `sass:color` missing channels (`rgb(none 255 0)`), to-gamut, hwb
//!   normalization, rec2020 transfer
//! - number serialization parity (10-digit CSS rounding, calc() for
//!   non-finite values and complex units)

use vass_evaluator::evaluate;
use vass_parser::parse_braces;

fn compile(src: &str) -> String {
  let sheet = parse_braces(src).unwrap_or_else(|e| panic!("parse failed for {:?}: {}", src, e));
  let css = evaluate(&sheet).unwrap_or_else(|e| panic!("eval failed for {:?}: {}", src, e));
  vass_transpiler::render(&css, vass_transpiler::OutputStyle::Expanded)
}

/// Run a source file through the full pipeline and capture @debug stderr
/// lines the way the CLI does
fn debug_of(src: &str) -> Vec<String> {
  use std::io::Write;
  use std::sync::atomic::{AtomicU32, Ordering};
  static SEQ: AtomicU32 = AtomicU32::new(0);
  let n = SEQ.fetch_add(1, Ordering::SeqCst);
  let dir = std::env::temp_dir().join(format!("vass_audit_{}_{}", std::process::id(), n));
  std::fs::create_dir_all(&dir).expect("tempdir");
  let path = dir.join("case.scss");
  let mut f = std::fs::File::create(&path).expect("create");
  f.write_all(src.as_bytes()).expect("write");
  drop(f);
  let out = std::process::Command::new(env!("CARGO_BIN_EXE_vass"))
    .args(["compile", path.to_str().unwrap()])
    .output()
    .expect("run vass");
  let _ = std::fs::remove_dir_all(&dir);
  let stderr = String::from_utf8_lossy(&out.stderr).to_string();
  let mut lines = Vec::new();
  for l in stderr.lines() {
    if let Some(i) = l.find("DEBUG: ") {
      lines.push(l[i + 7..].to_string());
    }
  }
  lines
}

fn assert_debug(src: &str, expected: &[&str]) {
  let got = debug_of(src);
  let exp: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
  assert_eq!(got, exp, "@debug mismatch for {:?}", src);
}

// ---- sass:list: separator metadata -----------------------------------------

#[test]
fn list_trailing_comma_single_element_keeps_comma() {
  // `(red,)` is a one-element comma list; the reference reports `comma`
  assert_debug(
    "@use 'sass:list' as l; @debug l.separator((red,));",
    &["comma"],
  );
}

#[test]
fn list_empty_and_single_bracket_lists_are_undecided() {
  // The reference reports `space` for `()`, `[]` and `[a]`
  assert_debug(
    "@use 'sass:list' as l; @debug l.separator(()); @debug l.separator([]); @debug l.separator([a]);",
    &["space", "space", "space"],
  );
}

#[test]
fn list_join_auto_falls_through_undecided() {
  // join auto: (undecided, comma) -> comma; (comma, undecided) -> comma
  assert_debug(
    "@use 'sass:list' as l; @debug l.join([], (b,)); @debug l.join((a,), ()); @debug l.join((a,), (b,));",
    &["[b,]", "(a,)", "a, b"],
  );
}

#[test]
fn list_append_auto_separator() {
  // append on a scalar yields a SPACE list; on a comma list keeps comma
  assert_debug(
    "@use 'sass:list' as l; @debug l.append(a, b); @debug l.append((a,), b); @debug l.append(a, b, slash);",
    &["a b", "a, b", "a / b"],
  );
}

#[test]
fn list_explicit_separator_wins_on_empties() {
  assert_debug(
    "@use 'sass:list' as l; @debug l.separator(l.join((), (), $separator: comma));",
    &["comma"],
  );
}

#[test]
fn list_slash_requires_two_elements() {
  let sheet = parse_braces("@use 'sass:list' as l; @debug l.slash(1);").unwrap();
  assert!(evaluate(&sheet).is_err(), "slash(1) must error");
}

// ---- sass:color: missing channels ------------------------------------------

#[test]
fn rgb_space_form_accepts_none_channels() {
  // The user-reported error: `rgb: $green is not a number (got none)`
  assert_debug(
    "@use 'sass:color' as c; @debug rgb(none 255 0); @debug rgb(255 none 0); @debug rgb(255 0 0 / none);",
    &["rgb(none 255 0)", "rgb(255 none 0)", "rgb(255 0 0 / none)"],
  );
}

#[test]
fn rgb_none_channels_css_output() {
  assert_eq!(
    compile("a { x: rgb(none 255 0); y: rgb(255 0 0 / none); z: rgb(none 255 0 / 0.5); }"),
    "a {\n  x: rgb(none 255 0);\n  y: rgb(255 0 0 / none);\n  z: rgb(none 255 0 / 0.5);\n}\n"
  );
}

#[test]
fn rgb_comma_form_rejects_none() {
  // The comma form is a plain Sass function call: `none` is not a number
  let sheet = parse_braces("@debug rgb(255, none, 0);").unwrap();
  assert!(evaluate(&sheet).is_err(), "comma-form none must error");
}

#[test]
fn hsl_space_form_accepts_none_channels() {
  assert_debug(
    "@debug hsl(none 50% 50%); @debug hsl(120 none 50%);",
    &["hsl(none 50% 50%)", "hsl(120deg none 50%)"],
  );
}

#[test]
fn missing_channel_quirky_reads() {
  // Missing channels read as 0 (quirky) and remember their missing-ness
  assert_debug(
    "@use 'sass:color' as c; @debug c.channel(rgb(none 255 0), 'red'); @debug c.is-missing(rgb(none 255 0), 'red'); @debug c.is-legacy(rgb(none 255 0)); @debug rgb(none 255 0) == rgb(0 255 0); @debug c.same(rgb(none 255 0), rgb(0 255 0));",
    &["0", "true", "true", "false", "true"],
  );
}

#[test]
fn rgb_none_clamps_and_normalizes_numbers() {
  assert_debug(
    "@debug rgb(none 999 0); @debug rgb(50% none 0); @debug rgb(none 100.0 0);",
    &["rgb(none 255 0)", "rgb(127.5 none 0)", "rgb(none 100 0)"],
  );
}

#[test]
fn change_threads_missing_channels() {
  assert_debug(
    "@use 'sass:color' as c; @debug c.change(rgb(none 255 0), $green: 100); @debug c.change(rgb(none 255 0), $alpha: 0.5); @debug c.change(oklch(60% 0.4 20deg), $chroma: none);",
    &[
      "rgb(none 100 0)",
      "rgb(none 255 0 / 0.5)",
      "oklch(60% none 20deg)",
    ],
  );
}

#[test]
fn adjust_and_invert_reject_missing_channels() {
  let a =
    parse_braces("@use 'sass:color' as c; @debug c.adjust(rgb(none 255 0), $red: 1);").unwrap();
  assert!(
    evaluate(&a).is_err(),
    "adjust on a missing channel must error"
  );
  let i = parse_braces("@use 'sass:color' as c; @debug c.invert(rgb(none 255 0));").unwrap();
  assert!(
    evaluate(&i).is_err(),
    "invert on missing channels must error"
  );
}

// ---- sass:color: to-gamut ---------------------------------------------------

#[test]
fn to_gamut_requires_method_and_validates_it() {
  let m =
    parse_braces("@use 'sass:color' as c; @debug c.to-gamut(oklch(60% 0.4 20deg), $space: rgb);")
      .unwrap();
  assert!(evaluate(&m).is_err(), "$method is required");
  let b = parse_braces(
    "@use 'sass:color' as c; @debug c.to-gamut(oklch(60% 0.4 20deg), $space: rgb, $method: bogus);",
  )
  .unwrap();
  assert!(evaluate(&b).is_err(), "unknown method must error");
}

#[test]
fn to_gamut_clip_converts_and_clamps_in_target_space() {
  // Clipping in sRGB and converting back to the original space (values
  // validated against dart-sass 1.106.1)
  assert_debug(
    "@use 'sass:color' as c; @debug c.to-gamut(oklch(60% 0.4 20deg), $space: rgb, $method: clip);",
    &["oklch(62.79553639214313% 0.25768330380536064 29.233880279627893deg)"],
  );
}

#[test]
fn to_gamut_clip_preserves_missing_same_space() {
  assert_debug(
    "@use 'sass:color' as c; @debug c.to-gamut(rgb(none 999 0), $space: rgb, $method: clip);",
    &["rgb(none 255 0)"],
  );
}

#[test]
fn to_gamut_unbounded_space_is_noop() {
  assert_debug(
    "@use 'sass:color' as c; @debug c.to-gamut(color(xyz 1.5 -0.5 2.5), $space: xyz, $method: clip);",
    &["color(xyz 1.5 -0.5 2.5)"],
  );
}

#[test]
fn to_gamut_lab_oklch_linear_channels_unbounded() {
  // lab a/b and lch chroma have no upper bound; clipping leaves them alone
  assert_debug(
    "@use 'sass:color' as c; @debug c.to-gamut(lch(50% 200 20deg), $space: lch, $method: clip); @debug c.to-gamut(lab(50% 200 -200), $space: lab, $method: clip);",
    &["lch(50% 200 20deg)", "lab(50% 200 -200)"],
  );
}

#[test]
fn to_gamut_local_minde_reference_values() {
  // CSS Color 4 local-MINDE algorithm (JND 0.02), validated against the
  // pinned reference
  assert_debug(
    "@use 'sass:color' as c; @debug c.to-gamut(oklch(50% 0.35 140deg), $space: rgb, $method: local-minde);",
    &["oklch(50.25322858252043% 0.17099887410092224 142.49534504144384deg)"],
  );
}

// ---- sass:color: hwb normalization + rec2020 --------------------------------

#[test]
fn hwb_normalizes_whiteness_blackness_sum() {
  assert_debug(
    "@debug hwb(0 150% 150%); @debug hwb(0 150% 50%);",
    &["hwb(0 50% 50%)", "hwb(0 75% 25%)"],
  );
}

#[test]
fn rec2020_uses_pure_24_gamma() {
  // CSS Color 4 updated rec2020 to a pure 2.4 gamma (matches the reference)
  assert_debug(
    "@use 'sass:color' as c; @debug c.to-space(#f00, rec2020);",
    &["color(rec2020 0.8234641019745034 0.3284294460170811 0.1803393289805469)"],
  );
}

// ---- number serialization parity --------------------------------------------

#[test]
fn css_output_rounds_to_10_digits_debug_keeps_precision() {
  assert_eq!(
    compile("a { b: 2.718281828459045; tiny: 1e-11; }"),
    "a {\n  b: 2.7182818285;\n  tiny: 0;\n}\n"
  );
  assert_debug("@debug 2.718281828459045;", &["2.718281828459045"]);
}

#[test]
fn nan_infinity_and_complex_units_use_calc() {
  // The pinned reference emits calc(NaN)/calc(infinity) and calc() for
  // numbers with complex units
  assert_debug(
    "@use 'sass:math' as m; @debug m.sqrt(-1); @debug m.div(1, 0); @debug m.div(-1, 0); @debug m.div(20px, 1s);",
    &[
      "calc(NaN)",
      "calc(infinity)",
      "calc(-infinity)",
      "calc(20px / 1s)",
    ],
  );
}

#[test]
fn plain_css_passthrough_flattens_space_args() {
  // `rgb(var(--x) 255 0)` re-serializes with comma-separated parts
  assert_debug("@debug rgb(var(--x) 255 0);", &["rgb(var(--x), 255, 0)"]);
}

// ---- round 4: legacy hsl raw channels (reference LinearChannel rules) ------
//
// The reference's channel definitions lower-clamp ONLY saturation at
// construction (`hsl(0 -50% x)` reads saturation as 0%) and never clamp
// lightness; `200%`/`150%` stay raw for channel reads, equality and gamut
// checks while the rgb backing clamps for conversion.

#[test]
fn hsl_raw_channels_survive_construction() {
  assert_debug(
    "@use 'sass:color' as c;
        @debug hsl(0 200% 150%);
        @debug hsl(0 -50% -50%);
        @debug c.channel(hsl(0 200% 150%), 'saturation');
        @debug c.channel(hsl(0 50% 150%), 'lightness');",
    &["hsl(0, 200%, 150%)", "hsl(0, 0%, -50%)", "200%", "150%"],
  );
}

#[test]
fn hsl_out_of_gamut_to_gamut_clips_channel_bounds() {
  // The pinned reference clips in the TARGET space: `hsl(0 200% 150%)`
  // clips to `hsl(0, 100%, 100%)`; the cross-space path follows the CSS
  // Color 4 f(n) conversion (`hsl(180, 100%, 75%)`)
  assert_debug(
    "@use 'sass:color' as c;
        @debug c.to-gamut(hsl(0 200% 150%), $space: hsl, $method: clip);
        @debug c.to-gamut(hsl(0 -50% -50%), $space: hsl, $method: clip);
        @debug c.to-gamut(hsl(0 200% 150%), $space: rgb, $method: clip);
        @debug c.to-gamut(hsl(0 200% 150%), $space: rgb, $method: local-minde);",
    &[
      "hsl(0, 100%, 100%)",
      "hsl(0, 0%, 0%)",
      "hsl(180, 100%, 75%)",
      "hsl(0, 0%, 100%)",
    ],
  );
}

#[test]
fn hsl_is_in_gamut_checks_raw_channels() {
  assert_debug(
    "@use 'sass:color' as c;
        @debug c.is-in-gamut(hsl(0 200% 150%));
        @debug c.is-in-gamut(hsl(0 50% -50%));
        @debug c.is-in-gamut(hsl(0 50% 50%));",
    &["false", "false", "true"],
  );
}

#[test]
fn hsl_equality_compares_raw_channels_exactly() {
  assert_debug(
    "@debug hsl(0 200% 150%) == hsl(0 100% 100%);
        @debug hsl(0 200% 150%) == hsl(0 200% 150%);
        @debug hsl(0 50% 50%) == red;",
    &["false", "true", "false"],
  );
}

#[test]
fn legacy_hsl_arithmetic_keeps_raw_channels() {
  // adjust: lightness raw (+100% -> 150%), saturation lower-clamped only;
  // lighten/darken/saturate clamp to 0-100%; grayscale keeps raw lightness
  assert_debug(
    "@use 'sass:color' as c;
        @debug c.adjust(hsl(0 50% 50%), $lightness: 100%);
        @debug c.adjust(hsl(0 50% 50%), $saturation: -100%);
        @debug lighten(hsl(0 50% 50%), 100%);
        @debug saturate(hsl(0 50% 50%), 100%);
        @debug grayscale(hsl(0 200% 150%));
        @debug adjust-hue(hsl(0 200% 150%), 45deg);",
    &[
      "hsl(0, 50%, 150%)",
      "hsl(0, 0%, 50%)",
      "hsl(0, 50%, 100%)",
      "hsl(0, 100%, 50%)",
      "hsl(0, 0%, 150%)",
      "hsl(45, 200%, 150%)",
    ],
  );
}

#[test]
fn legacy_hsl_change_and_scale_keep_raw_channels() {
  assert_debug(
    "@use 'sass:color' as c;
        @debug c.change(hsl(0 50% 50%), $saturation: 250%);
        @debug c.change(hsl(0 50% 50%), $lightness: -50%);
        @debug c.scale(hsl(0 200% 150%), $lightness: 100%);
        @debug c.scale(hsl(0 50% 50%), $lightness: 100%);
        @debug adjust-color(hsl(0 50% 50%), $lightness: 100%);
        @debug change-color(hsl(0 50% 50%), $saturation: 250%);",
    &[
      "hsl(0, 250%, 50%)",
      "hsl(0, 50%, -50%)",
      "hsl(0, 200%, 150%)",
      "hsl(0, 50%, 100%)",
      "hsl(0, 50%, 150%)",
      "hsl(0, 250%, 50%)",
    ],
  );
}

// ---- round 4: trailing loud comment placement ------------------------------
//
// A loud comment is written inline when it starts on the source line where
// the previous emitted sibling ended (or the `{` line for the first child).

#[test]
fn trailing_comment_stays_on_declaration_line() {
  assert_eq!(
    compile("a {\n  /* c1 */ color: red; /* c2 */\n}\n"),
    "a {\n  /* c1 */\n  color: red; /* c2 */\n}\n"
  );
}

#[test]
fn first_inline_comment_joins_the_brace_line() {
  assert_eq!(
    compile("a { /* c1 */ color: red; /* c2 */ }\n"),
    "a { /* c1 */\n  color: red; /* c2 */\n}\n"
  );
}

#[test]
fn single_inline_comment_block_stays_one_line() {
  assert_eq!(compile("n { /* only */ }\n"), "n { /* only */ }\n");
  assert_eq!(
    compile("d { e { color: red } /* after */ }\n"),
    "d e {\n  color: red;\n}\nd { /* after */ }\n"
  );
}

#[test]
fn comment_on_own_line_stays_on_own_line() {
  assert_eq!(
    compile("h {\n  color: red;\n  /* own line */\n  margin: 0;\n}\n"),
    "h {\n  color: red;\n  /* own line */\n  margin: 0;\n}\n"
  );
}

#[test]
fn top_level_trailing_comment_joins_the_rule_line() {
  assert_eq!(
    compile("p { color: red; } /* sep */ q { color: blue; }\n"),
    "p {\n  color: red;\n} /* sep */\nq {\n  color: blue;\n}\n"
  );
}

#[test]
fn hoisted_nested_rule_respects_source_order() {
  // the nested rule appears BEFORE the trailing comment in the source, so
  // `y z {...}` is emitted before `y { /* a */ }`
  assert_eq!(
    compile("y { z { color: red } /* a */ }\n"),
    "y z {\n  color: red;\n}\ny { /* a */ }\n"
  );
}

// ---- round 4: attribute selector value normalization -----------------------
//
// The reference parses selectors and re-serializes attribute values: a
// quoted value that spells a valid CSS ident is written unquoted, otherwise
// re-quoted preferring double quotes.

#[test]
fn attr_values_ident_safe_are_unquoted() {
  assert_eq!(compile("a[b='c'] { x: 1 }\n"), "a[b=c] {\n  x: 1;\n}\n");
  assert_eq!(
    compile("a > b + c ~ d .e:fOCUS:not([x='y'])::before { color: red; }\n"),
    "a > b + c ~ d .e:fOCUS:not([x=y])::before {\n  color: red;\n}\n"
  );
}

#[test]
fn attr_values_needing_quotes_use_double_quotes() {
  assert_eq!(
    compile("a[b='c d'] { x: 1 }\na[b='1'] { x: 1 }\na[b='a\"b'] { x: 1 }\n"),
    "a[b=\"c d\"] {\n  x: 1;\n}\n\na[b=\"1\"] {\n  x: 1;\n}\n\na[b='a\"b'] {\n  x: 1;\n}\n"
  );
}

#[test]
fn attr_selector_whitespace_is_normalized() {
  assert_eq!(
    compile("a[ b ~= 'c' ] { x: 1 }\n"),
    "a[b~=c] {\n  x: 1;\n}\n"
  );
}

// ---- round 4: attribute matcher operators ----------------------------------
//
// The reference supports all six attribute matcher operators; `|=`, `^=`
// and `$=` were rejected by the lexer before.

#[test]
fn all_attribute_matcher_operators_parse() {
  assert_eq!(
    compile("a[b|='c'] { x: 1 }\na[b^='c'] { x: 1 }\na[b$='c'] { x: 1 }\n"),
    "a[b|=c] {\n  x: 1;\n}\n\na[b^=c] {\n  x: 1;\n}\n\na[b$=c] {\n  x: 1;\n}\n"
  );
}

// ---- round 4: escape sequences in property names ---------------------------
//
// Property names are raw-captured: `\66 oo` (the escape for `f` followed by
// `oo`) is re-emitted verbatim exactly like the reference.

#[test]
fn escaped_property_name_is_preserved_verbatim() {
  assert_eq!(
    compile("a { \\66 oo: 1px; }\n"),
    "a {\n  \\66 oo: 1px;\n}\n"
  );
}

// ---- round 4: loud comments inside values are dropped ----------------------

#[test]
fn loud_comment_inside_value_is_dropped() {
  assert_eq!(
    compile("u { color: red /* c */; margin: 0 }\n"),
    "u {\n  color: red;\n  margin: 0;\n}\n"
  );
}
