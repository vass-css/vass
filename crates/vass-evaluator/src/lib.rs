//! # VASS Evaluator
//!
//! Expression evaluator and built-in functions for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Expression evaluation
//! - Variable scoping
//! - Nested rule flattening with `&` support
//! - Block at-rule handling (`@media`, ...)

#![warn(missing_docs)]
#![allow(dead_code)]

pub mod builtins;
pub mod environment;
pub mod visitor;

pub use vass_core::error::{Error, Result};
pub use visitor::evaluate::{EvalOptions, evaluate, evaluate_with};

/// Evaluate a parsed stylesheet into CSS
pub fn evaluate_stylesheet(
  sheet: &vass_core::ast::input::statement::Stylesheet,
) -> Result<vass_core::ast::css::CssStylesheet> {
  evaluate(sheet)
}

#[cfg(test)]
mod tests {
  use super::*;
  use vass_parser::{Syntax, parse};

  #[test]
  fn test_evaluate_stylesheet_basic() {
    let sheet = parse(".a { color: red; }", Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
    let css = result.unwrap();
    assert!(!css.nodes.is_empty());
  }

  #[test]
  fn test_evaluate_stylesheet_with_variables() {
    let sheet = parse("$color: red;\n.a { color: $color; }", Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }

  #[test]
  fn test_evaluate_stylesheet_with_nesting() {
    let sheet = parse(".a { .b { color: red; } }", Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }

  #[test]
  fn test_evaluate_stylesheet_empty() {
    let sheet = parse("", Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }

  // ── meta builtins ──
  #[test]
  fn test_meta_type_of() {
    let src = "@use \"vass:meta\";\n.a { content: meta.type-of(12px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
    let css = format!("{:?}", result.unwrap().nodes);
    assert!(css.contains("number"));
  }

  #[test]
  fn test_meta_inspect() {
    let src = "@use \"vass:meta\";\n.a { content: meta.inspect(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_meta_keywords() {
    let src = "@use \"vass:meta\";\n.a { content: meta.keywords((color: red, width: 10px)); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  // ── map builtins ──
  #[test]
  fn test_map_get() {
    let src = "@use \"vass:map\";\n.a { content: map.get((color: red, width: 10px), color); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_merge() {
    let src = "@use \"vass:map\";\n.a { content: map.merge((color: red), (width: 10px)); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_remove() {
    let src = "@use \"vass:map\";\n.a { content: map.remove((color: red, width: 10px), color); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_keys() {
    let src = "@use \"vass:map\";\n.a { content: map.keys((color: red, width: 10px)); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_values() {
    let src = "@use \"vass:map\";\n.a { content: map.values((color: red, width: 10px)); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_has_key() {
    let src = "@use \"vass:map\";\n.a { content: map.has-key((color: red, width: 10px), color); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_set() {
    let src = "@use \"vass:map\";\n.a { content: map.set((color: red), width, 20px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_deep_merge() {
    let src = "@use \"vass:map\";\n.a { content: map.deep-merge((color: red), (width: 10px)); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_deep_get() {
    let src = "@use \"vass:map\";\n.a { content: map.deep-get((color: (red: true)), color, red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_map_deep_remove() {
    let src =
      "@use \"vass:map\";\n.a { content: map.deep-remove((color: (red: true)), color, red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  // ── string builtins ──
  #[test]
  fn test_string_quote() {
    let src = "@use \"vass:string\";\n.a { content: string.quote(hello); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_unquote() {
    let src = "@use \"vass:string\";\n.a { content: string.unquote(\"hello\"); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_to_upper_case() {
    let src = "@use \"vass:string\";\n.a { content: string.to-upper-case(hello); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_to_lower_case() {
    let src = "@use \"vass:string\";\n.a { content: string.to-lower-case(HELLO); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_length() {
    let src = "@use \"vass:string\";\n.a { content: string.length(\"hello world\"); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_insert() {
    let src = "@use \"vass:string\";\n.a { content: string.insert(\"hello\", \"X\", 3); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_index() {
    let src = "@use \"vass:string\";\n.a { content: string.index(\"hello world\", \"world\"); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_slice() {
    let src = "@use \"vass:string\";\n.a { content: string.slice(\"hello\", 1, 3); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_split() {
    let src = "@use \"vass:string\";\n.a { content: string.split(\"a,b,c\", \",\"); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_string_unique_id() {
    let src = "@use \"vass:string\";\n.a { content: string.unique-id(); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  // ── modules builtins ──
  #[test]
  fn test_modules_builtin_list() {
    let src = "@use \"vass:math\";\n.a { content: math.abs(-5px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_modules_color() {
    let src = "@use \"vass:color\";\n.a { content: color.red(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_modules_list() {
    let src = "@use \"vass:list\";\n.a { content: list.length(1px 2px 3px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_modules_selector() {
    let src = "@use \"vass:selector\";\n.a { content: selector.append(\".a\", \".b\"); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  // ── math builtins ──
  #[test]
  fn test_math_abs() {
    let src = "@use \"vass:math\";\n.a { content: math.abs(-5px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_math_ceil() {
    let src = "@use \"vass:math\";\n.a { content: math.ceil(3.5px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_math_floor() {
    let src = "@use \"vass:math\";\n.a { content: math.floor(3.5px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_math_round() {
    let src = "@use \"vass:math\";\n.a { content: math.round(3.5px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_math_max() {
    let src = "@use \"vass:math\";\n.a { content: math.max(1px, 10px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_math_min() {
    let src = "@use \"vass:math\";\n.a { content: math.min(1px, 10px); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_math_percent() {
    let src = "@use \"vass:math\";\n.a { content: math.percent(0.5); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  // ── color builtins ──
  #[test]
  fn test_color_red() {
    let src = "@use \"vass:color\";\n.a { content: color.red(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_green() {
    let src = "@use \"vass:color\";\n.a { content: color.green(green); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_blue() {
    let src = "@use \"vass:color\";\n.a { content: color.blue(blue); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_mix() {
    let src = "@use \"vass:color\";\n.a { content: color.mix(red, blue, 50%); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  // ── color channel tests ──
  #[test]
  fn test_color_channel_red() {
    let src = "@use \"vass:color\";\n.a { content: color.channel(red, \"red\"); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_space() {
    let src = "@use \"vass:color\";\n.a { content: color.space(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_same() {
    let src = "@use \"vass:color\";\n.a { content: color.same(red, red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_is_legacy() {
    let src = "@use \"vass:color\";\n.a { content: color.is-legacy(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_is_in_gamut() {
    let src = "@use \"vass:color\";\n.a { content: color.is-in-gamut(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_is_missing() {
    let src = "@use \"vass:color\";\n.a { content: color.is-missing(red, \"red\"); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_is_powerless() {
    let src = "@use \"vass:color\";\n.a { content: color.is-powerless(red, \"red\"); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_hwb() {
    let src = "@use \"vass:color\";\n.a { content: color.hwb(0, 50%, 50%); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_invert() {
    let src = "@use \"vass:color\";\n.a { content: color.invert(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_grayscale() {
    let src = "@use \"vass:color\";\n.a { content: color.grayscale(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_complement() {
    let src = "@use \"vass:color\";\n.a { content: color.complement(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_ie_hex() {
    let src = "@use \"vass:color\";\n.a { content: color.ie-hex-str(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_change() {
    let src = "@use \"vass:color\";\n.a { content: color.change(red, $red: 100); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_adjust() {
    let src = "@use \"vass:color\";\n.a { content: color.adjust(red, $lightness: 10%); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_scale() {
    let src = "@use \"vass:color\";\n.a { content: color.scale(red, $lightness: 50%); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_alpha() {
    let src = "@use \"vass:color\";\n.a { content: color.alpha(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_legacy_channel_hue() {
    let src = "@use \"vass:color\";\n.a { content: color.hue(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_legacy_channel_saturation() {
    let src = "@use \"vass:color\";\n.a { content: color.saturation(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_color_legacy_channel_lightness() {
    let src = "@use \"vass:color\";\n.a { content: color.lightness(red); }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  // ── control flow tests ──
  #[test]
  fn test_control_if_true() {
    let src = ".a { @if true { color: red; } }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_control_if_false() {
    let src = ".a { @if false { color: red; } }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_control_if_else_true() {
    let src = ".a { @if true { color: red; } @else { color: blue; } }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_control_if_else_false() {
    let src = ".a { @if false { color: red; } @else { color: blue; } }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_control_each() {
    let src = ".a { @each $v in (1px, 2px, 3px) { margin: $v; } }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_control_for() {
    let src = ".a { @for $i from 1 through 3 { margin: $i * 1px; } }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_control_while() {
    let src = ".a { $i: 0; @while $i < 3 { margin: $i * 1px; $i: $i + 1; } }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  // ── error path tests ──
  #[test]
  fn test_eval_undefined_variable() {
    let src = ".a { color: $undefined_var; }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_err());
  }

  #[test]
  fn test_eval_division_by_zero() {
    let src = ".a { width: 1px / 0; }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }

  #[test]
  fn test_eval_nested_blocks() {
    let src = ".a { .b { .c { color: red; } } }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }

  #[test]
  fn test_eval_interpolation_in_selector() {
    let src = "$ns: test;\n.#{$ns} { color: red; }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok(), "{:?}", result.err());
  }

  #[test]
  fn test_eval_empty_block() {
    let src = ".a { }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }

  #[test]
  fn test_eval_multiple_declarations() {
    let src = ".a { color: red; width: 10px; height: 20px; }";
    let sheet = parse(src, Syntax::Braces).unwrap();
    let result = evaluate_stylesheet(&sheet);
    assert!(result.is_ok());
  }
}
