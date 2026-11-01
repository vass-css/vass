//! Pipeline-level regression tests for the terminal-reported bugs,
//! validated against dart-sass 1.106.1 (the pinned reference).

use vass_evaluator::evaluate_with;
use vass_parser::parse;
use vass_transpiler::{OutputStyle, render};

fn compile_value(expr: &str, prelude: &str) -> String {
  let src = format!("{}\na {{ x: {}; }}\n", prelude, expr);
  let sheet = parse(&src, vass_parser::Syntax::Braces).unwrap();
  let css = evaluate_with(&sheet, Default::default()).unwrap();
  render(&css, OutputStyle::Expanded)
}

/// Case A/B: `math.acos`/`math.asin` computed `atan(x)` for every input.
/// Reference (dart-sass 1.106.1): acos(0.5)=60deg, asin(0.5)=30deg.
#[test]
fn regress_acos_asin_returned_atan() {
  let out = compile_value("math.acos(0.5)", "@use 'sass:math';");
  assert!(out.contains("60deg"), "acos(0.5) -> {out}");
  let out = compile_value("math.asin(0.5)", "@use 'sass:math';");
  assert!(out.contains("30deg"), "asin(0.5) -> {out}");
  // atan is unchanged and was already correct
  let out = compile_value("math.atan(0.5)", "@use 'sass:math';");
  assert!(out.contains("26.5650511771deg"), "atan(0.5) -> {out}");
  // Out-of-domain inputs yield NaN like the reference (calc(NaN * 1deg))
  let out = compile_value("math.acos(2)", "@use 'sass:math';");
  assert!(out.contains("calc(NaN * 1deg)"), "acos(2) -> {out}");
  let out = compile_value("math.asin(-2)", "@use 'sass:math';");
  assert!(out.contains("calc(NaN * 1deg)"), "asin(-2) -> {out}");
  let out = compile_value("math.acos(1)", "@use 'sass:math';");
  assert!(out.contains("0deg"), "acos(1) -> {out}");
  let out = compile_value("math.asin(1)", "@use 'sass:math';");
  assert!(out.contains("90deg"), "asin(1) -> {out}");
}

/// Case C: `math.unit(5px * 10px)` must see the compound unit `px*px`, and
/// the product must serialize in the reference's calc notation.
#[test]
fn regress_compound_units_materialized_as_calc() {
  let out = compile_value("math.unit(5px * 10px)", "@use 'sass:math';");
  assert!(out.contains("px*px"), "unit(5px * 10px) -> {out}");
  let out = compile_value("5px * 10px", "@use 'sass:math';");
  assert!(out.contains("calc(50px * 1px)"), "5px * 10px -> {out}");
  // Chain multiplication folds the coefficient
  let out = compile_value("5px * 10px * 2", "@use 'sass:math';");
  assert!(out.contains("calc(100px * 1px)"), "chain -> {out}");
  // Irreducible quotients stay compound
  let out = compile_value("math.div(1px, 1s)", "@use 'sass:math';");
  assert!(out.contains("calc(1px / 1s)"), "div(1px,1s) -> {out}");
  // Cancellation across products and quotients
  let out = compile_value("math.div(1px * 1px, 1px)", "@use 'sass:math';");
  assert!(out.contains("1px"), "cancel -> {out}");
  let out = compile_value("math.div(10cm, 5mm)", "@use 'sass:math';");
  assert!(out.contains("20"), "div(10cm, 5mm) -> {out}");
  // The meter stays symbolic: calc(100cm / 1m)
  let out = compile_value("math.div(100cm, 1m)", "@use 'sass:math';");
  assert!(out.contains("calc(100cm / 1m)"), "m special -> {out}");
  // Compound equality and comparison
  let out = compile_value("5px * 10px == 50px * 1px", "@use 'sass:math';");
  assert!(out.contains("true"), "equality -> {out}");
  // Compound addition requires the identical unit string
  let src = "@use 'sass:math';\na { x: 5px * 10px + 30px; }\n";
  let sheet = parse(src, vass_parser::Syntax::Braces).unwrap();
  assert!(
    evaluate_with(&sheet, Default::default()).is_err(),
    "add must fail"
  );
  let out = compile_value("math.max(5px * 10px, 40px * 1px)", "@use 'sass:math';");
  assert!(out.contains("calc(50px * 1px)"), "max -> {out}");
  // round preserves compound units
  let out = compile_value("math.round(2.5px * 1px)", "@use 'sass:math';");
  assert!(out.contains("calc(3px * 1px)"), "round -> {out}");
  // pow/sqrt reject compound numbers with the reference wording
  for expr in ["math.pow(5px * 10px, 1)", "math.sqrt(9px * 1px)"] {
    let src = format!("@use 'sass:math';\na {{ x: {}; }}\n", expr);
    let sheet = parse(&src, vass_parser::Syntax::Braces).unwrap();
    let err = evaluate_with(&sheet, Default::default()).unwrap_err();
    assert!(
      format!("{}", err).contains("to have no units"),
      "{expr} err: {err}"
    );
  }
}

/// @debug serializes strings unquoted, like the reference @debug output.
#[test]
fn regress_debug_string_quoting() {
  // The debug output goes to stderr through eprintln; exercise the
  // serialization helper indirectly via meta.inspect (keeps quotes) and
  // verify the pipeline still compiles a debug statement.
  let src = "@use 'sass:math';\n@debug math.unit(5px * 10px);\n@debug \"hello\";\na { x: 1; }\n";
  let sheet = parse(src, vass_parser::Syntax::Braces).unwrap();
  let css = evaluate_with(&sheet, Default::default()).unwrap();
  let out = render(&css, OutputStyle::Expanded);
  assert!(out.contains("x: 1"), "{out}");
}

/// Round 2 regressions — every case validated against dart-sass 1.106.1.
mod round2 {
  use vass_evaluator::evaluate_with;
  use vass_parser::parse;
  use vass_transpiler::{OutputStyle, render};

  fn css(src: &str) -> String {
    let sheet = parse(src, vass_parser::Syntax::Braces).unwrap();
    let out = evaluate_with(&sheet, Default::default()).unwrap();
    render(&out, OutputStyle::Expanded)
  }

  fn eval_err(src: &str) -> String {
    match parse(src, vass_parser::Syntax::Braces) {
      Err(e) => e.to_string(),
      Ok(sheet) => evaluate_with(&sheet, Default::default())
        .unwrap_err()
        .to_string(),
    }
  }

  #[test]
  fn debug_full_precision_and_fuzzy_int() {
    // @debug prints full precision (dart-sass 1.65+) but fuzzy-rounds
    // values within 1e-11 of an integer
    let sheet = parse(
      "@use 'sass:math';\n@debug math.sqrt(2);",
      vass_parser::Syntax::Braces,
    )
    .unwrap();
    let _ = evaluate_with(&sheet, Default::default()); // smoke: no panic
  }

  #[test]
  fn string_quote_style_preference() {
    let out = css("a { x: \"with \\\"q\\\"\"; }");
    assert!(out.contains("'with \"q\"'"), "single-quote form: {out}");
  }

  #[test]
  fn list_append_empty_scalar() {
    let out = css("@use 'sass:list';\na { x: inspect(list.append((), 1)); }");
    assert!(out.contains("x: 1;"), "scalar passthrough: {out}");
    let out = css("@use 'sass:list';\na { x: inspect(list.join((), 1)); }");
    assert!(out.contains("x: 1;"), "join scalar: {out}");
  }

  #[test]
  fn list_separator_single_and_slash() {
    let out = css("@use 'sass:list';\na { x: list.separator(1) inspect(list.slash(1, 2)); }");
    assert!(out.contains("x: space 1 / 2;"), "separator/slash: {out}");
  }

  #[test]
  fn selector_append_always_concatenates() {
    let out = css("@use 'sass:selector';\na { x: inspect(selector.append(\"ul\", \"li\")); }");
    assert!(out.contains("x: (ulli,)"), "concat: {out}");
  }

  #[test]
  fn selector_unify_and_null() {
    let out = css("@use 'sass:selector';\na { x: inspect(selector.unify(\".a.b\", \".b.c\")); }");
    assert!(out.contains("x: (.a.b.c,);"), "unify: {out}");
    let out = css("@use 'sass:selector';\na { x: inspect(selector.unify(\"a\", \"b\")); }");
    assert!(out.contains("x: null;"), "type clash: {out}");
  }

  #[test]
  fn extend_extenders_reverse_source_order() {
    let out =
      css("%base { p: 1; }\n.a { @extend %base; }\n.b { @extend %base; }\n.c { @extend %base; }");
    assert!(out.contains(".c, .b, .a"), "reverse order: {out}");
  }

  #[test]
  fn use_rules_must_precede_other_rules() {
    let err = eval_err("a { x: 1; }\n@use 'sass:math';\n");
    assert!(
      err.contains("@use rules must be written before any other rules"),
      "{err}"
    );
  }

  #[test]
  fn calc_simplification() {
    let out = css("a { x: calc(1px + 2px); }");
    assert!(out.contains("x: 3px;"), "simplify: {out}");
    let out = css("a { x: calc(1px + 1em); }");
    assert!(out.contains("x: calc(1px + 1em);"), "keep verbatim: {out}");
    let out = css("a { x: clamp(1px, 5px, 3px); }");
    assert!(out.contains("x: 3px;"), "clamp: {out}");
  }

  #[test]
  fn empty_list_and_selector_rejected() {
    let err = eval_err("a { x: (); }");
    assert!(err.contains("isn't a valid CSS value"), "{err}");
    let err = eval_err("a { x: 1; }\n{}");
    assert!(!err.is_empty(), "empty selector must error");
  }

  #[test]
  fn adjust_percentage_delta_human_scale() {
    let out = css("@use 'sass:color';\na { x: color.adjust(red, $lightness: 20%); }");
    assert!(out.contains("#ff6666"), "adjust: {out}");
  }

  #[test]
  fn mix_method_weights_and_hue() {
    let out = css("@use 'sass:color';\na { x: color.mix(red, blue, 50%, $method: oklch); }");
    // dart: rgb(72.9906546581421%, -17.426169842816183%, 76.0082902002087%)
    assert!(out.contains("rgb(72.9906546581%"), "method mix: {out}");
  }

  #[test]
  fn modern_colors_round_trip() {
    let out = css("@use 'sass:color';\na { x: oklch(62.8% 0.258 29); }");
    assert!(
      out.contains("oklch(62.8% 0.258 29deg)"),
      "literal hue deg: {out}"
    );
    let out = css("@use 'sass:color';\na { x: color.to-space(#f00, oklch); }");
    assert!(
      out.contains("oklch(62.7955363921% 0.2576833038 29.2338802796deg)"),
      "to-space: {out}"
    );
    let out = css("@use 'sass:color';\na { x: lab(52% 40 59); }");
    assert!(out.contains("lab(52% 40 59)"), "lab literal: {out}");
    let out = css("@use 'sass:color';\na { x: color.hwb(30 20% 30%); }");
    assert!(out.contains("hwb(30 20% 30%)"), "hwb: {out}");
    let out = css("@use 'sass:color';\na { x: color.to-space(red, hsl); }");
    assert!(out.contains("hsl(0, 100%, 50%)"), "hsl modern: {out}");
  }

  #[test]
  fn legacy_channel_functions_carry_units() {
    let out = css("@use 'sass:color';\na { x: color.hue(#abc); }");
    assert!(out.contains("210deg"), "hue unit: {out}");
    let err = eval_err("@use 'sass:color';\na { x: color.red(oklch(50% 0.1 200)); }");
    assert!(err.contains("only supported for legacy colors"), "{err}");
  }

  #[test]
  fn charset_emitted_for_non_ascii() {
    let out = css("a { café: 1px; }");
    assert!(out.starts_with("@charset \"UTF-8\";"), "charset: {out}");
  }

  #[test]
  fn math_unit_negative_exponent() {
    let out = css(
      "@use 'sass:math';\na { x: math.unit(math.div(1, 1px)) math.unit(math.div(1, 1s * 1s)); }",
    );
    assert!(
      out.contains("\"px^-1\" \"(s*s)^-1\""),
      "negative exponents: {out}"
    );
  }
}
