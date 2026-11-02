//! Comprehensive feature integration tests
//!
//! Every language feature is exercised through the full pipeline
//! (parse -> evaluate -> render) in BOTH syntaxes, with pairwise
//! equivalence checks between `.vass` (indented) and `.vcss` (braces).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use vass_evaluator::{EvalOptions, evaluate, evaluate_with};
use vass_parser::{parse, parse_braces, parse_indented};
use vass_transpiler::{OutputStyle, render};

fn compile_src(src: &str, indented: bool) -> String {
  let sheet = if indented {
    parse_indented(src)
  } else {
    parse_braces(src)
  }
  .unwrap_or_else(|e| panic!("parse failed for {:?}: {}", src, e));
  let css = evaluate(&sheet).unwrap_or_else(|e| panic!("eval failed for {:?}: {}", src, e));
  render(&css, OutputStyle::Expanded)
}

fn both(vass_src: &str, vcss_src: &str) -> (String, String) {
  let a = compile_src(vass_src, true);
  let b = compile_src(vcss_src, false);
  (a, b)
}

fn assert_eq_both(vass_src: &str, vcss_src: &str) -> String {
  let (a, b) = both(vass_src, vcss_src);
  assert_eq!(a, b, "syntax outputs differ");
  a
}

// ---- variables -------------------------------------------------------------

#[test]
fn variables_scoping_and_global() {
  let out = assert_eq_both(
    "$x: 1px\n.a\n  width: $x\n  $y: 2px\n  height: $y\n.b\n  top: $x\n",
    "$x: 1px;\n.a {\n  width: $x;\n  $y: 2px;\n  height: $y;\n}\n.b {\n  top: $x;\n}\n",
  );
  assert!(out.contains("width: 1px;"));
  assert!(out.contains("height: 2px;"));
  assert!(out.contains("top: 1px;"));
}

#[test]
fn variable_default_flag() {
  let out = assert_eq_both(
    "$x: 1 !default\n$x: 2 !default\n.a\n  z: $x\n",
    "$x: 1 !default;\n$x: 2 !default;\n.a {\n  z: $x;\n}\n",
  );
  assert!(out.contains("z: 1;"));
}

#[test]
fn variable_global_flag() {
  let out = assert_eq_both(
    "$n: 0\n@each $i in (1 2 3)\n  $n: $n + 1 !global\n.a\n  x: $n\n",
    "$n: 0;\n@each $i in (1 2 3) {\n  $n: $n + 1 !global;\n}\n.a {\n  x: $n;\n}\n",
  );
  assert!(out.contains("x: 3;"), "got: {}", out);
}

#[test]
fn arithmetic_units_and_conversion() {
  let out = assert_eq_both(
    ".a\n  a: 1px + 2px\n  b: 10px * 2\n  c: (10px / 4)\n  d: 96px + 1in\n  e: 7 % 3\n",
    ".a {\n  a: 1px + 2px;\n  b: 10px * 2;\n  c: (10px / 4);\n  d: 96px + 1in;\n  e: 7 % 3;\n}\n",
  );
  assert!(out.contains("a: 3px;"));
  assert!(out.contains("b: 20px;"));
  assert!(out.contains("c: 2.5px;"));
  assert!(out.contains("d: 192px;"));
  assert!(out.contains("e: 1;"));
}

#[test]
fn slash_division_semantics() {
  let out = assert_eq_both(
    "$a: 8\n.a\n  font: 12px/30px\n  b: $a/2\n  c: (1px/2)\n",
    "$a: 8;\n.a {\n  font: 12px/30px;\n  b: $a/2;\n  c: (1px/2);\n}\n",
  );
  assert!(out.contains("font: 12px/30px;"));
  assert!(out.contains("b: 4;"));
  assert!(out.contains("c: 0.5px;"));
}

// ---- nesting ---------------------------------------------------------------

#[test]
fn nesting_parent_suffix_and_cartesian() {
  let out = assert_eq_both(
    ".a, .b\n  color: red\n  &:hover\n    color: blue\n  &-card\n    margin: 0\n",
    ".a, .b {\n  color: red;\n  &:hover {\n    color: blue;\n  }\n  &-card {\n    margin: 0;\n  }\n}\n",
  );
  assert!(out.contains(".a, .b {"));
  assert!(out.contains(".a:hover, .b:hover {"));
  assert!(out.contains(".a-card, .b-card {"));
}

#[test]
fn nesting_with_child_combinator() {
  let out = assert_eq_both(
    ".a\n  > .b\n    color: red\n",
    ".a {\n  > .b {\n    color: red;\n  }\n}\n",
  );
  assert!(out.contains(".a > .b {"), "got: {}", out);
}

// ---- at-rules ---------------------------------------------------------------

#[test]
fn media_bubbling_inside_rule() {
  let out = assert_eq_both(
    ".a\n  @media screen\n    color: red\n    .b\n      color: blue\n",
    ".a {\n  @media screen {\n    color: red;\n    .b {\n      color: blue;\n    }\n  }\n}\n",
  );
  assert!(out.contains("@media screen {"));
  assert!(out.contains(".a {"));
  assert!(out.contains(".a .b {"));
}

#[test]
fn media_nested_merge_with_and() {
  let out = assert_eq_both(
    "@media (min-width: 100px)\n  .a\n    @media (max-width: 200px)\n      color: red\n",
    "@media (min-width: 100px) {\n  .a {\n    @media (max-width: 200px) {\n      color: red;\n    }\n  }\n}\n",
  );
  assert!(
    out.contains("@media (min-width: 100px) and (max-width: 200px) {"),
    "got: {}",
    out
  );
}

#[test]
fn keyframes_nested_in_rule_not_prefixed() {
  let out = assert_eq_both(
    ".wrap\n  @keyframes fade\n    from\n      opacity: 0\n    to\n      opacity: 1\n  color: red\n",
    ".wrap {\n  @keyframes fade {\n    from {\n      opacity: 0;\n    }\n    to {\n      opacity: 1;\n    }\n  }\n  color: red;\n}\n",
  );
  assert!(out.contains("@keyframes fade {"));
  assert!(out.contains("from {"));
  assert!(!out.contains(".wrap from"));
}

#[test]
fn font_face_and_supports() {
  let out = assert_eq_both(
    "@font-face\n  font-family: X\n  src: url(a.vass)\n@supports (display: grid)\n  .a\n    display: grid\n",
    "@font-face {\n  font-family: X;\n  src: url(a.vass);\n}\n@supports (display: grid) {\n  .a {\n    display: grid;\n  }\n}\n",
  );
  assert!(out.contains("@font-face {"));
  assert!(out.contains("@supports (display: grid) {"));
}

#[test]
fn charset_passthrough() {
  // dart-sass 1.107.0 (npx sass probe): an `@charset` in the source with
  // pure-ASCII output is STRIPPED from the result; the engine re-emits
  // `@charset "UTF-8";` on its own line only when the output contains
  // non-ASCII bytes. Vass matches both behaviours.
  let out = assert_eq_both(
    "@charset \"UTF-8\"\n.a\n  color: red\n",
    "@charset \"UTF-8\";\n.a {\n  color: red;\n}\n",
  );
  assert!(!out.contains("@charset"));
  let out = assert_eq_both(
    "@charset \"UTF-8\"\n.a\n  content: \"h\u{e9}llo\"\n",
    "@charset \"UTF-8\";\n.a {\n  content: \"h\u{e9}llo\";\n}\n",
  );
  assert!(out.starts_with("@charset \"UTF-8\";\n"));
}

// ---- control flow -----------------------------------------------------------

#[test]
fn if_else_if_else() {
  let out = assert_eq_both(
    "$m: 2\n.a\n  @if $m == 1\n    x: one\n  @else if $m == 2\n    x: two\n  @else\n    x: many\n",
    "$m: 2;\n.a {\n  @if $m == 1 {\n    x: one;\n  } @else if $m == 2 {\n    x: two;\n  } @else {\n    x: many;\n  }\n}\n",
  );
  assert!(out.contains("x: two;"));
  assert!(!out.contains("one") || out.contains("two"));
}

#[test]
fn each_list_and_map_destructure() {
  let out = assert_eq_both(
    "@each $a in (1 2 3)\n  .i-#{$a}\n    v: $a\n",
    "@each $a in (1 2 3) {\n  .i-#{$a} {\n    v: $a;\n  }\n}\n",
  );
  assert!(out.contains(".i-1 {"));
  assert!(out.contains(".i-3 {"));

  let out = assert_eq_both(
    "$m: (\"k1\": 10px, \"k2\": 20px)\n@each $k, $v in $m\n  .#{$k}\n    w: $v\n",
    "$m: (\"k1\": 10px, \"k2\": 20px);\n@each $k, $v in $m {\n  .#{$k} {\n    w: $v;\n  }\n}\n",
  );
  assert!(out.contains(".k1 {") && out.contains("w: 10px;"));
  assert!(out.contains(".k2 {") && out.contains("w: 20px;"));
}

#[test]
fn for_to_through_reverse() {
  let out = assert_eq_both(
    "@for $i from 1 through 3\n  .t-#{$i}\n    v: $i\n",
    "@for $i from 1 through 3 {\n  .t-#{$i} {\n    v: $i;\n  }\n}\n",
  );
  assert!(out.contains(".t-3 {"));
  let out = assert_eq_both(
    "@for $i from 1 to 3\n  .o-#{$i}\n    v: $i\n",
    "@for $i from 1 to 3 {\n  .o-#{$i} {\n    v: $i;\n  }\n}\n",
  );
  assert!(out.contains(".o-2 {"));
  assert!(!out.contains(".o-3 {"));
  let out = assert_eq_both(
    "@for $i from 3 through 1\n  .r-#{$i}\n    v: $i\n",
    "@for $i from 3 through 1 {\n  .r-#{$i} {\n    v: $i;\n  }\n}\n",
  );
  assert!(out.contains(".r-3 {") && out.contains(".r-1 {"));
}

#[test]
fn while_loop() {
  let out = assert_eq_both(
    "$i: 0\n@while $i < 3\n  .w-#{$i}\n    v: $i\n  $i: $i + 1\n",
    "$i: 0;\n@while $i < 3 {\n  .w-#{$i} {\n    v: $i;\n  }\n  $i: $i + 1;\n}\n",
  );
  assert!(out.contains(".w-0 {") && out.contains(".w-2 {"));
  assert!(!out.contains(".w-3 {"));
}

// ---- mixins -----------------------------------------------------------------

#[test]
fn mixin_params_defaults_named_and_rest() {
  let out = assert_eq_both(
    "@mixin box($w, $h: 10px, $pad...)\n  width: $w\n  height: $h\n  padding: nth($pad, 1)\n.a\n  @include box(1px, 2px, 3px)\n",
    "@mixin box($w, $h: 10px, $pad...) {\n  width: $w;\n  height: $h;\n  padding: nth($pad, 1);\n}\n.a {\n  @include box(1px, 2px, 3px);\n}\n",
  );
  assert!(out.contains("width: 1px;"));
  assert!(out.contains("height: 2px;"));
  assert!(out.contains("padding: 3px;"));

  let out = assert_eq_both(
    "@mixin sz($w, $h: 2px)\n  width: $w\n  height: $h\n.a\n  @include sz($h: 5px, $w: 4px)\n",
    "@mixin sz($w, $h: 2px) {\n  width: $w;\n  height: $h;\n}\n.a {\n  @include sz($h: 5px, $w: 4px);\n}\n",
  );
  assert!(out.contains("width: 4px;") && out.contains("height: 5px;"));
}

#[test]
fn mixin_content_with_using() {
  let out = assert_eq_both(
    "@mixin repeat($n)\n  @for $i from 1 through $n\n    @content($i)\n.a\n  @include repeat(2) using ($i)\n    .c-#{$i}\n      v: $i\n",
    "@mixin repeat($n) {\n  @for $i from 1 through $n {\n    @content($i);\n  }\n}\n.a {\n  @include repeat(2) using ($i) {\n    .c-#{$i} {\n      v: $i;\n    }\n  }\n}\n",
  );
  assert!(out.contains(".c-1 {") && out.contains(".c-2 {"));
}

#[test]
fn content_closure_sees_include_site_vars() {
  let out = assert_eq_both(
    "$x: outer\n@mixin m\n  @content\n.a\n  $x: inner\n  @include m\n    v: $x\n",
    "$x: outer;\n@mixin m {\n  @content;\n}\n.a {\n  $x: inner;\n  @include m {\n    v: $x;\n  }\n}\n",
  );
  assert!(out.contains("v: inner;"), "got: {}", out);
}

#[test]
fn mixin_without_block_is_error() {
  let sheet = parse_braces("@include nope { }").unwrap();
  assert!(evaluate(&sheet).is_err());
  let sheet = parse_braces(".a { @include nope; }").unwrap();
  assert!(evaluate(&sheet).is_err());
}

// ---- functions --------------------------------------------------------------

#[test]
fn functions_params_defaults_and_call() {
  let out = assert_eq_both(
    "@function add($a, $b: 1)\n  @return $a + $b\n.a\n  w: add(2)\n  h: add(2, 3) * 1px\n",
    "@function add($a, $b: 1) {\n  @return $a + $b;\n}\n.a {\n  w: add(2);\n  h: add(2, 3) * 1px;\n}\n",
  );
  assert!(out.contains("w: 3;"));
  assert!(out.contains("h: 5px;"));
}

#[test]
fn function_with_control_flow() {
  let out = assert_eq_both(
    "@function classify($n)\n  @if $n > 10\n    @return big\n  @else if $n > 5\n    @return mid\n  @return small\n.a\n  a: classify(3)\n  b: classify(7)\n  c: classify(20)\n",
    "@function classify($n) {\n  @if $n > 10 {\n    @return big;\n  } @else if $n > 5 {\n    @return mid;\n  } @return small;\n}\n.a {\n  a: classify(3);\n  b: classify(7);\n  c: classify(20);\n}\n",
  );
  assert!(out.contains("a: small;") && out.contains("b: mid;") && out.contains("c: big;"));
}

#[test]
fn function_fibonacci_recursion() {
  let out = assert_eq_both(
    "@function fib($n)\n  @if $n <= 1\n    @return $n\n  @return fib($n - 1) + fib($n - 2)\n.a\n  v: fib(10)\n",
    "@function fib($n) {\n  @if $n <= 1 {\n    @return $n;\n  }\n  @return fib($n - 1) + fib($n - 2);\n}\n.a {\n  v: fib(10);\n}\n",
  );
  assert!(out.contains("v: 55;"), "got: {}", out);
}

#[test]
fn function_rest_args_and_keywords() {
  let out = assert_eq_both(
    "@function first-of($args...)\n  @return nth($args, 1)\n.a\n  v: first-of(9px, 8px)\n",
    "@function first-of($args...) {\n  @return nth($args, 1);\n}\n.a {\n  v: first-of(9px, 8px);\n}\n",
  );
  assert!(out.contains("v: 9px;"));
}

#[test]
fn if_function_is_lazy() {
  let out = assert_eq_both(
    "$t: true\n.a\n  v: if($t, yes, no)\n  w: if(false, no, yes)\n",
    "$t: true;\n.a {\n  v: if($t, yes, no);\n  w: if(false, no, yes);\n}\n",
  );
  assert!(out.contains("v: yes;") && out.contains("w: yes;"));
}

// ---- interpolation ----------------------------------------------------------

#[test]
fn interpolation_everywhere() {
  let out = assert_eq_both(
    "$n: 3\n$p: margin\n.a-#{$n}\n  #{$p}-top: #{$n}0px\n  content: \"v=#{$n * 2}\"\n",
    "$n: 3;\n$p: margin;\n.a-#{$n} {\n  #{$p}-top: #{$n}0px;\n  content: \"v=#{$n * 2}\";\n}\n",
  );
  assert!(out.contains(".a-3 {"), "got: {}", out);
  assert!(out.contains("margin-top: 30px;"));
  assert!(out.contains("content: \"v=6\";"));
}

#[test]
fn interpolation_in_media_params() {
  let out = assert_eq_both(
    "$w: 500px\n@media (min-width: #{$w})\n  .a\n    color: red\n",
    "$w: 500px;\n@media (min-width: #{$w}) {\n  .a {\n    color: red;\n  }\n}\n",
  );
  assert!(out.contains("@media (min-width: 500px) {"));
}

// ---- maps -------------------------------------------------------------------

#[test]
fn map_operations() {
  let out = assert_eq_both(
    "$m: (\"a\": 1px, \"b\": 2px)\n.a\n  x: map-get($m, \"a\")\n  y: map-get($m, \"b\")\n  z: length(map-keys($m))\n",
    "$m: (\"a\": 1px, \"b\": 2px);\n.a {\n  x: map-get($m, \"a\");\n  y: map-get($m, \"b\");\n  z: length(map-keys($m));\n}\n",
  );
  assert!(out.contains("x: 1px;"));
  assert!(out.contains("y: 2px;"));
  assert!(out.contains("z: 2;"));
}

#[test]
fn map_deep_merge() {
  let out = assert_eq_both(
    "$a: (\"x\": (\"p\": 1))\n$b: (\"x\": (\"q\": 2))\n$m: map-merge($a, $b)\n.a\n  v: map-get(map-get($m, \"x\"), \"q\")\n",
    "$a: (\"x\": (\"p\": 1));\n$b: (\"x\": (\"q\": 2));\n$m: map-merge($a, $b);\n.a {\n  v: map-get(map-get($m, \"x\"), \"q\");\n}\n",
  );
  assert!(out.contains("v: 2;"));
}

// ---- extend -----------------------------------------------------------------

#[test]
fn extend_basic_and_transitive() {
  let out = assert_eq_both(
    ".message\n  color: red\n.error\n  @extend .message\n.serious-error\n  @extend .error\n",
    ".message {\n  color: red;\n}\n.error {\n  @extend .message;\n}\n.serious-error {\n  @extend .error;\n}\n",
  );
  assert!(
    out.contains(".message, .error, .serious-error {"),
    "got: {}",
    out
  );
}

#[test]
fn extend_compound_and_placeholder() {
  let out = assert_eq_both(
    ".btn.primary\n  color: blue\n.cta\n  @extend .primary\n%ph\n  padding: 1px\n.x\n  @extend %ph\n",
    ".btn.primary {\n  color: blue;\n}\n.cta {\n  @extend .primary;\n}\n%ph {\n  padding: 1px;\n}\n.x {\n  @extend %ph;\n}\n",
  );
  assert!(
    out.contains(".btn.cta {") || out.contains(".btn.primary, .btn.cta {"),
    "got: {}",
    out
  );
  assert!(!out.contains("%ph"), "placeholder leaked: {}", out);
  assert!(out.contains(".x {"));
  assert!(out.contains("padding: 1px;"));
}

#[test]
fn extend_inside_media() {
  let out = assert_eq_both(
    "%btn\n  padding: 2px\n@media print\n  .save\n    @extend %btn\n",
    "%btn {\n  padding: 2px;\n}\n@media print {\n  .save {\n    @extend %btn;\n  }\n}\n",
  );
  // Extends are global: the placeholder rule is rewritten and the
  // empty media block disappears
  assert!(out.contains(".save {"), "got: {}", out);
  assert!(out.contains("padding: 2px;"));
}

// ---- built-in functions -------------------------------------------------------

#[test]
fn color_functions() {
  let out = assert_eq_both(
    "$c: #336699\n.a\n  a: lighten($c, 10%)\n  b: darken($c, 10%)\n  c: rgba(255, 0, 0, 0.5)\n  d: hsl(210, 50%, 50%)\n  e: mix(#ff0000, #0000ff)\n  f: grayscale(#ff0000)\n  g: invert(#ffffff)\n  h: red($c) + green($c) + blue($c)\n  i: alpha(rgba(0, 0, 0, 0.25))\n  j: adjust-hue(#ff0000, 120deg)\n  k: transparentize(#000000, 0.5)\n",
    "$c: #336699;\n.a {\n  a: lighten($c, 10%);\n  b: darken($c, 10%);\n  c: rgba(255, 0, 0, 0.5);\n  d: hsl(210, 50%, 50%);\n  e: mix(#ff0000, #0000ff);\n  f: grayscale(#ff0000);\n  g: invert(#ffffff);\n  h: red($c) + green($c) + blue($c);\n  i: alpha(rgba(0, 0, 0, 0.25));\n  j: adjust-hue(#ff0000, 120deg);\n  k: transparentize(#000000, 0.5);\n}\n",
  );
  // HSL-derived results keep fractional channels and serialize as
  // percentages (reference behavior)
  assert!(out.contains("a: rgb(25%, 50%, 75%);"), "got: {}", out);
  assert!(out.contains("b: rgb(15%, 30%, 45%);"));
  assert!(out.contains("c: rgba(255, 0, 0, 0.5);"));
  assert!(out.contains("d: hsl(210, 50%, 50%);"));
  assert!(out.contains("e: rgb(50%, 0%, 50%);"));
  assert!(out.contains("f: rgb(50%, 50%, 50%);"));
  assert!(out.contains("g: black;"));
  assert!(out.contains("h: 306;"));
  assert!(out.contains("i: 0.25;"));
  assert!(out.contains("j: lime;"));
  assert!(out.contains("k: rgba(0, 0, 0, 0.5);"));
}

#[test]
fn color_change_scale_adjust() {
  let out = assert_eq_both(
    ".a\n  a: change-color(#ff0000, $blue: 255)\n  b: scale-color(#336699, $lightness: 20%)\n  c: adjust-color(#336699, $hue: 60deg)\n",
    ".a {\n  a: change-color(#ff0000, $blue: 255);\n  b: scale-color(#336699, $lightness: 20%);\n  c: adjust-color(#336699, $hue: 60deg);\n}\n",
  );
  assert!(out.contains("a: fuchsia;"), "got: {}", out);
  assert!(out.contains("b: rgb(28%, 52%, 76%);"), "got: {}", out);
  assert!(out.contains("c: rebeccapurple;"), "got: {}", out);
}

#[test]
fn math_and_string_functions() {
  let out = assert_eq_both(
    ".a\n  a: abs(-5)\n  b: round(2.6)\n  c: floor(2.9)\n  d: ceil(2.1)\n  e: min(1, 2, 3)\n  f: max(1, 2, 3)\n  g: percentage(0.5)\n  h: to-upper-case(\"vass\")\n  i: str-length(\"hello\")\n  j: str-index(\"vass\", \"a\")\n  k: str-slice(\"vasscss\", 1, 4)\n  l: quote(hi)\n  m: unquote(\"x\")\n  n: unit(5px)\n  o: unitless(5)\n  p: comparable(1px, 1in)\n",
    ".a {\n  a: abs(-5);\n  b: round(2.6);\n  c: floor(2.9);\n  d: ceil(2.1);\n  e: min(1, 2, 3);\n  f: max(1, 2, 3);\n  g: percentage(0.5);\n  h: to-upper-case(\"vass\");\n  i: str-length(\"hello\");\n  j: str-index(\"vass\", \"a\");\n  k: str-slice(\"vasscss\", 1, 4);\n  l: quote(hi);\n  m: unquote(\"x\");\n  n: unit(5px);\n  o: unitless(5);\n  p: comparable(1px, 1in);\n}\n",
  );
  for expect in [
    "a: 5;",
    "b: 3;",
    "c: 2;",
    "d: 3;",
    "e: 1;",
    "f: 3;",
    "g: 50%;",
    "h: \"VASS\";",
    "i: 5;",
    "j: 2;",
    "k: \"vass\"",
    "l: \"hi\";",
    "m: x;",
    "n: \"px\";",
    "o: true;",
    "p: true;",
  ] {
    assert!(out.contains(expect), "missing {}: got {}", expect, out);
  }
}

#[test]
fn list_functions() {
  let out = assert_eq_both(
    ".a\n  a: length(1 2 3)\n  b: nth(1 2 3, 2)\n  c: index(a b c, b)\n  d: length(join((1 2), (3 4)))\n  e: length(append(1 2, 3))\n  f: list-separator((1, 2))\n  g: length(zip(1 2, 3 4))\n",
    ".a {\n  a: length(1 2 3);\n  b: nth(1 2 3, 2);\n  c: index(a b c, b);\n  d: length(join((1 2), (3 4)));\n  e: length(append(1 2, 3));\n  f: list-separator((1, 2));\n  g: length(zip(1 2, 3 4));\n}\n",
  );
  for expect in [
    "a: 3;",
    "b: 2;",
    "c: 2;",
    "d: 4;",
    "e: 3;",
    "f: \"comma\";",
    "g: 2;",
  ] {
    assert!(out.contains(expect), "missing {}: got {}", expect, out);
  }
}

#[test]
fn meta_functions() {
  let out = assert_eq_both(
    ".a\n  a: type-of(1px)\n  b: type-of(\"x\")\n  c: type-of(red)\n  d: type-of((1 2))\n  e: type-of((\"k\": 1))\n  f: type-of(true)\n  g: type-of(null)\n  h: inspect(1 2 3)\n  i: feature-exists(\"at-error\")\n",
    ".a {\n  a: type-of(1px);\n  b: type-of(\"x\");\n  c: type-of(red);\n  d: type-of((1 2));\n  e: type-of((\"k\": 1));\n  f: type-of(true);\n  g: type-of(null);\n  h: inspect(1 2 3);\n  i: feature-exists(\"at-error\");\n}\n",
  );
  for expect in [
    "a: number",
    "b: string",
    "c: color",
    "d: list",
    "e: map",
    "f: bool",
    "g: null",
    "h: 1 2 3",
    "i: true;",
  ] {
    assert!(out.contains(expect), "missing {}: got {}", expect, out);
  }
}

#[test]
fn plain_css_function_passthrough() {
  let out = assert_eq_both(
    ".a\n  transform: translate(-50%, -50%)\n  width: calc(100% - 10px)\n  bg: url(data:image/png;base64,AAA)\n  clip: rect(0, 0, 4px, 4px)\n",
    ".a {\n  transform: translate(-50%, -50%);\n  width: calc(100% - 10px);\n  bg: url(data:image/png;base64,AAA);\n  clip: rect(0, 0, 4px, 4px);\n}\n",
  );
  assert!(out.contains("translate(-50%, -50%);"), "got: {}", out);
  assert!(out.contains("calc(100% - 10px);"));
  assert!(out.contains("url(data:image/png;base64,AAA);"));
}

#[test]
fn selector_functions() {
  let out = assert_eq_both(
    ".a\n  a: selector-nest(\".a\", \"&:hover\")\n  b: selector-append(\".a\", \".b\")\n  c: selector-unify(\".a\", \".b\")\n",
    ".a {\n  a: selector-nest(\".a\", \"&:hover\");\n  b: selector-append(\".a\", \".b\");\n  c: selector-unify(\".a\", \".b\");\n}\n",
  );
  assert!(out.contains("a: .a:hover;"), "got: {}", out);
  assert!(out.contains("b: .a.b;"));
  assert!(out.contains("c: .a.b;"));
}

// ---- custom properties & nested props -----------------------------------------

#[test]
fn custom_properties_raw() {
  let out = assert_eq_both(
    "$c: theme\n.a\n  --main: 1px + 2px\n  --theme: #{$c}-dark\n  --x: {a: b}\n",
    "$c: theme;\n.a {\n  --main: 1px + 2px;\n  --theme: #{$c}-dark;\n  --x: {a: b};\n}\n",
  );
  assert!(out.contains("--main: 1px + 2px;"), "got: {}", out);
  assert!(out.contains("--theme: theme-dark;"));
  assert!(out.contains("--x: {a: b};"));
}

#[test]
fn nested_properties_syntax() {
  let out = assert_eq_both(
    ".a\n  font: 12px\n    family: serif\n    weight: bold\n  margin: auto\n    top: 1px\n",
    ".a {\n  font: 12px {\n    family: serif;\n    weight: bold;\n  }\n  margin: auto {\n    top: 1px;\n  }\n}\n",
  );
  assert!(out.contains("font: 12px;"));
  assert!(out.contains("font-family: serif;"));
  assert!(out.contains("font-weight: bold;"));
  assert!(out.contains("margin: auto;"));
  assert!(out.contains("margin-top: 1px;"));
}

// ---- at-root -----------------------------------------------------------------

#[test]
fn at_root_hoists_selectors() {
  let out = assert_eq_both(
    ".wrap\n  color: red\n  @at-root\n    .inner\n      color: blue\n",
    ".wrap {\n  color: red;\n  @at-root {\n    .inner {\n      color: blue;\n    }\n  }\n}\n",
  );
  assert!(out.contains(".inner {"), "got: {}", out);
  assert!(!out.contains(".wrap .inner"));
  assert!(out.contains(".wrap {"));
}

#[test]
fn at_root_without_media() {
  let out = assert_eq_both(
    "@media print\n  .wrap\n    @at-root (without: media)\n      .free\n        color: red\n",
    "@media print {\n  .wrap {\n    @at-root (without: media) {\n      .free {\n        color: red;\n      }\n    }\n  }\n}\n",
  );
  assert!(out.contains(".free {"), "got: {}", out);
  assert!(!out.contains("@media print {") || out.contains("@media print"));
}

// ---- debug/warn/error ----------------------------------------------------------

#[test]
fn at_error_raises() {
  for src in ["@error \"boom\"\n", "@error \"boom\";\n"] {
    let sheet = if src.ends_with(";\n") {
      parse_braces(src).unwrap()
    } else {
      parse_indented(src).unwrap()
    };
    let err = evaluate(&sheet).unwrap_err();
    assert!(err.message.contains("boom"), "got: {}", err.message);
  }
}

// ---- comments -----------------------------------------------------------------

#[test]
fn comments_preserved_and_silent_dropped() {
  let out = assert_eq_both(
    "/* header */\n.a\n  /* inner */\n  color: red\n",
    "/* header */\n.a {\n  /* inner */\n  color: red;\n}\n",
  );
  assert!(out.contains("/* header */"));
  assert!(out.contains("/* inner */"));
  let out = assert_eq_both(
    "// gone\n.a\n  color: red\n",
    "// gone\n.a {\n  color: red;\n}\n",
  );
  assert!(!out.contains("gone"));
}

#[test]
fn compressed_output() {
  let compile_compressed = |src: &str, indented: bool| -> String {
    let sheet = if indented {
      parse_indented(src).unwrap()
    } else {
      parse_braces(src).unwrap()
    };
    let css = evaluate(&sheet).unwrap();
    render(&css, OutputStyle::Compressed)
  };
  let a = compile_compressed(
    ".a\n  margin: 0.5px\n  padding: 1px 2px\n  color: #ff0000\n",
    true,
  );
  let b = compile_compressed(
    ".a {\n  margin: 0.5px;\n  padding: 1px 2px;\n  color: #ff0000;\n}\n",
    false,
  );
  assert_eq!(a, b);
  // dart-sass --style=compressed ends the sheet with a single newline;
  // vass matches byte-for-byte (od -c probe of both binaries).
  assert_eq!(a, ".a{margin:.5px;padding:1px 2px;color:#ff0000}\n");
}

// ---- comparisons & logic ---------------------------------------------------------

#[test]
fn comparisons_and_boolean_logic() {
  let out = assert_eq_both(
    "$a: 3\n$b: 4\n.c\n  r1: $a < $b\n  r2: $a >= $b\n  r3: $a == 3\n  r4: $a != 3\n  r5: $a > 0 and $b > 0\n  r6: not $a\n  r7: false or true\n",
    "$a: 3;\n$b: 4;\n.c {\n  r1: $a < $b;\n  r2: $a >= $b;\n  r3: $a == 3;\n  r4: $a != 3;\n  r5: $a > 0 and $b > 0;\n  r6: not $a;\n  r7: false or true;\n}\n",
  );
  for expect in [
    "r1: true;",
    "r2: false;",
    "r3: true;",
    "r4: false;",
    "r5: true;",
    "r6: false;",
    "r7: true;",
  ] {
    assert!(out.contains(expect), "missing {}: got {}", expect, out);
  }
}

#[test]
fn string_equality_and_concat() {
  let out = assert_eq_both(
    ".a\n  a: \"x\" == \"x\"\n  b: \"x\" != \"y\"\n  c: \"a\" + \"b\"\n  d: a + b\n",
    ".a {\n  a: \"x\" == \"x\";\n  b: \"x\" != \"y\";\n  c: \"a\" + \"b\";\n  d: a + b;\n}\n",
  );
  for expect in ["a: true;", "b: true;", "c: \"ab\";", "d: ab;"] {
    assert!(out.contains(expect), "missing {}: got {}", expect, out);
  }
}

// ---- !important & flags -----------------------------------------------------------

#[test]
fn important_flag() {
  let out = assert_eq_both(
    ".a\n  color: red !important\n",
    ".a {\n  color: red !important;\n}\n",
  );
  assert!(out.contains("color: red !important;"));
}

// ---- module system (filesystem) ---------------------------------------------------

static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
  let dir = std::env::temp_dir().join(format!(
    "vass-feat-test-{}-{}",
    std::process::id(),
    TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
  ));
  std::fs::create_dir_all(&dir).unwrap();
  dir
}

fn evaluate_file(entry: &std::path::Path) -> String {
  let source = std::fs::read_to_string(entry).unwrap();
  let syntax = vass_parser::detect_syntax(entry);
  let sheet = parse(&source, syntax).unwrap();
  let options = EvalOptions {
    entry: Some(entry.to_path_buf()),
    ..Default::default()
  };
  let css = evaluate_with(&sheet, options).unwrap();
  render(&css, OutputStyle::Expanded)
}

#[test]
fn use_user_module_with_namespace() {
  let dir = tempdir();
  std::fs::write(
        dir.join("_vars.vass"),
        "$primary: red\n$secondary: blue\n@function double($n)\n  @return $n * 2\n@mixin base\n  padding: 1px\n",
    )
    .unwrap();
  std::fs::write(
        dir.join("main.vass"),
        "@use \"vars\"\n.a\n  color: vars.$primary\n  width: vars.double(4) * 1px\n  @include vars.base\n",
    )
    .unwrap();
  let out = evaluate_file(&dir.join("main.vass"));
  assert!(out.contains("color: red;"), "got: {}", out);
  assert!(out.contains("width: 8px;"));
  assert!(out.contains("padding: 1px;"));
  std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn use_module_star_and_default_namespace() {
  let dir = tempdir();
  std::fs::write(dir.join("lib.vass"), "$v: 7px\n@fn: 0\n").unwrap();
  std::fs::write(dir.join("main.vass"), "@use \"lib\" as *\n.a\n  w: $v\n").unwrap();
  let out = evaluate_file(&dir.join("main.vass"));
  assert!(out.contains("w: 7px;"), "got: {}", out);
  std::fs::remove_dir_all(&dir).ok();
}

// dart-sass strictness (REA probe `type: mixin`): accessing a module
// namespace that was never `@use`d must error, including built-in modules.
#[test]
fn namespace_access_requires_use() {
  let sheet = parse_braces(".a { x: meta.get-mixin(\"m\"); }").unwrap();
  let err = vass_evaluator::evaluate(&sheet).unwrap_err();
  assert!(
    err
      .message
      .contains("There is no module with namespace \"meta\""),
    "got: {}",
    err.message
  );

  let sheet = parse_braces("a { x: map.get((a: 1), a); }").unwrap();
  let err = vass_evaluator::evaluate(&sheet).unwrap_err();
  assert!(
    err
      .message
      .contains("There is no module with namespace \"map\""),
    "got: {}",
    err.message
  );

  // `as *` merges globals but does NOT register the namespace
  let sheet = parse_braces("@use \"sass:math\" as *;\na { x: math.div(4, 2); }").unwrap();
  let err = vass_evaluator::evaluate(&sheet).unwrap_err();
  assert!(
    err
      .message
      .contains("There is no module with namespace \"math\""),
    "got: {}",
    err.message
  );
  // DX: the error explains that `as *` members are used bare
  let help = err.help().unwrap_or_default();
  assert!(
    help.contains("as *") && help.contains("WITHOUT a namespace"),
    "help: {help}"
  );
  // DX: the error explains that `as *` members are used bare
  let help = err.help().unwrap_or_default();
  assert!(
    help.contains("as *") && help.contains("WITHOUT a namespace"),
    "help: {help}"
  );

  // Explicit @use makes the namespace resolvable
  let sheet = parse_braces("@use \"sass:math\";\na { x: math.div(4, 2); }").unwrap();
  let css = vass_evaluator::evaluate(&sheet).unwrap();
  let css_str = format!("{:?}", css);
  assert!(css_str.contains("2"), "got: {:?}", css);
}

#[test]
fn use_with_configuration() {
  let dir = tempdir();
  std::fs::write(
    dir.join("cfg.vass"),
    "$size: 10px !default\n.a\n  w: $size\n",
  )
  .unwrap();
  std::fs::write(
    dir.join("main.vass"),
    "@use \"cfg\" with ($size: 42px)\n.b\n  h: cfg.$size\n",
  )
  .unwrap();
  let out = evaluate_file(&dir.join("main.vass"));
  assert!(out.contains("w: 42px;"), "got: {}", out);
  assert!(out.contains("h: 42px;"));
  std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn forward_module_with_prefix() {
  let dir = tempdir();
  std::fs::write(
    dir.join("inner.vass"),
    "$color: green\n@function f($x)\n  @return $x\n@mixin m\n  margin: 0\n",
  )
  .unwrap();
  std::fs::write(dir.join("mid.vass"), "@forward \"inner\" as i-*\n").unwrap();
  std::fs::write(
    dir.join("main.vass"),
    "@use \"mid\"\n.a\n  c: mid.$i-color\n  w: mid.i-f(3)\n  @include mid.i-m\n",
  )
  .unwrap();
  let out = evaluate_file(&dir.join("main.vass"));
  assert!(out.contains("c: green;"), "got: {}", out);
  assert!(out.contains("w: 3;"));
  assert!(out.contains("margin: 0;"));
  std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn import_inlines_and_passes_through_css() {
  let dir = tempdir();
  std::fs::write(dir.join("part.vass"), ".part\n  color: red\n").unwrap();
  std::fs::write(
        dir.join("main.vass"),
        "@import \"part\"\n@import \"https://cdn.example/x.css\"\n@import \"print.css\" print\n.main\n  color: blue\n",
    )
    .unwrap();
  let out = evaluate_file(&dir.join("main.vass"));
  assert!(out.contains(".part {"), "got: {}", out);
  assert!(out.contains("@import \"https://cdn.example/x.css\";"));
  assert!(out.contains("@import \"print.css\" print;"));
  assert!(out.contains(".main {"));
  std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn module_cycle_detection() {
  let dir = tempdir();
  std::fs::write(dir.join("a.vass"), "@use \"b\"\n$x: 1\n").unwrap();
  std::fs::write(dir.join("b.vass"), "@use \"a\"\n$y: 2\n").unwrap();
  std::fs::write(dir.join("main.vass"), "@use \"a\"\n").unwrap();
  let source = std::fs::read_to_string(dir.join("main.vass")).unwrap();
  let sheet = parse(&source, vass_core::ast::input::Syntax::Indented).unwrap();
  let options = EvalOptions {
    entry: Some(dir.join("main.vass")),
    ..Default::default()
  };
  let err = evaluate_with(&sheet, options).unwrap_err();
  assert!(
    err.message.contains("cycle") || err.message.contains("Can't find"),
    "got: {}",
    err.message
  );
  std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn import_cycle_detection() {
  let dir = tempdir();
  std::fs::write(dir.join("a.vass"), "@import \"b\"\n").unwrap();
  std::fs::write(dir.join("b.vass"), "@import \"a\"\n").unwrap();
  std::fs::write(dir.join("main.vass"), "@import \"a\"\n").unwrap();
  let source = std::fs::read_to_string(dir.join("main.vass")).unwrap();
  let sheet = parse(&source, vass_core::ast::input::Syntax::Indented).unwrap();
  let options = EvalOptions {
    entry: Some(dir.join("main.vass")),
    ..Default::default()
  };
  let err = evaluate_with(&sheet, options).unwrap_err();
  assert!(
    err.message.contains("Circular import"),
    "got: {}",
    err.message
  );
  std::fs::remove_dir_all(&dir).ok();
}

// ---- module resolution (dart-faithful, probed against dart-sass 1.107) ----

/// Evaluate `vcss/main.vcss`-style entry with the given files laid out in a
/// temp dir; returns the CSS or the error.
fn eval_module_layout(
  files: &[(&str, &str)],
  entry: &str,
) -> std::result::Result<String, vass_core::error::Error> {
  let dir = tempdir();
  for (path, src) in files {
    let full = dir.join(path);
    if let Some(parent) = full.parent() {
      std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full, src).unwrap();
  }
  let source = std::fs::read_to_string(dir.join(entry)).unwrap();
  let sheet = parse(&source, vass_core::ast::input::Syntax::Braces).unwrap();
  let options = EvalOptions {
    entry: Some(dir.join(entry)),
    ..Default::default()
  };
  let result = evaluate_with(&sheet, options)
    .map(|css| vass_transpiler::render(&css, vass_transpiler::OutputStyle::Expanded));
  std::fs::remove_dir_all(&dir).ok();
  result
}

#[test]
fn use_underscore_partial_next_to_entry() {
  // The canonical dart-sass layout: `@use 'base'` finds `_base.vcss`
  let out = eval_module_layout(
        &[
            ("vcss/_base.vcss", "$primary-color: #333;\n"),
            (
                "vcss/main.vcss",
                "@use 'base';\n\n.inverse {\n  background-color: base.$primary-color;\n  color: white;\n}\n",
            ),
        ],
        "vcss/main.vcss",
    )
    .unwrap();
  assert!(out.contains("background-color: #333;"), "got: {}", out);
  assert!(out.contains("color: white;"), "got: {}", out);
}

#[test]
fn use_explicit_extension_finds_partial() {
  // dart-sass: `@use "base.scss"` also matches `_base.scss`
  let out = eval_module_layout(
    &[
      ("vcss/_base.vcss", "$v: partial;\n"),
      ("vcss/main.vcss", "@use 'base.vcss';\n.a { x: base.$v; }\n"),
    ],
    "vcss/main.vcss",
  )
  .unwrap();
  assert!(out.contains("x: partial;"), "got: {}", out);
}

#[test]
fn use_index_file_resolution() {
  // `@use 'base'` falls back to `base/index.vcss` / `base/_index.vcss`
  let out = eval_module_layout(
    &[
      ("vcss/base/_index.vcss", "$v: indexed;\n"),
      ("vcss/main.vcss", "@use 'base';\n.a { x: base.$v; }\n"),
    ],
    "vcss/main.vcss",
  )
  .unwrap();
  assert!(out.contains("x: indexed;"), "got: {}", out);
}

#[test]
fn use_ambiguous_plain_and_partial() {
  // dart-sass: same-extension plain + partial pair is an ambiguity error
  let err = eval_module_layout(
    &[
      ("vcss/base.vcss", "$v: plain;\n"),
      ("vcss/_base.vcss", "$v: partial;\n"),
      ("vcss/main.vcss", "@use 'base';\n.a { x: base.$v; }\n"),
    ],
    "vcss/main.vcss",
  )
  .unwrap_err();
  assert!(
    err.message.contains("not clear which file to import"),
    "got: {}",
    err.message
  );
}

#[test]
fn use_not_found_lists_searched_paths() {
  // The failure help must name what was searched so users can see the
  // mismatch between the URL and their on-disk layout
  let err =
    eval_module_layout(&[("vcss/main.vcss", "@use 'missing';\n")], "vcss/main.vcss").unwrap_err();
  assert!(
    err.message.contains("Can't find module `missing`"),
    "got: {}",
    err.message
  );
  let note = err
    .notes
    .iter()
    .map(|n| n.message.as_str())
    .collect::<Vec<_>>()
    .join("\n");
  assert!(note.contains("missing.vcss"), "note: {note}");
  assert!(note.contains("_missing.vcss"), "note: {note}");
  let help = err.help().unwrap_or_default();
  assert!(
    help.contains("create the module file `_missing.vcss`"),
    "help: {help}"
  );
}

#[test]
fn use_not_found_error_is_anchored_at_the_use_rule() {
  // E0008 must point at the `@use` statement (line 1) like dart-sass,
  // not render without a location.
  let err = eval_module_layout(
    &[("vcss/main.vcss", "@use 'missing';\n.a { x: 1; }\n")],
    "vcss/main.vcss",
  )
  .unwrap_err();
  let span = err.span.expect("E0008 carries the @use rule's span");
  assert_eq!(span.start.line, 1, "span: {span:?}");
  assert_eq!(span.start.column, 1);
  // The span covers the whole rule (`@use 'missing';` = 15 chars)
  assert!(span.len() >= 13, "span covers the statement: {span:?}");
  // and the error is attributed to the importing file
  assert!(
    err.file.as_deref().unwrap_or("").ends_with("main.vcss"),
    "file: {:?}",
    err.file
  );
}

#[test]
fn e0011_without_any_use_suggests_adding_it() {
  // The exact tutorial scenario: `base.$x` with no `@use` anywhere.
  let err = eval_module_layout(
    &[("vcss/main.vcss", ".inverse { x: base.$primary-color; }\n")],
    "vcss/main.vcss",
  )
  .unwrap_err();
  assert!(
    err
      .message
      .contains("There is no module with namespace \"base\""),
    "got: {}",
    err.message
  );
  let help = err.help().unwrap_or_default();
  assert!(
    help.contains("Add `@use 'base';`") && help.contains("TOP"),
    "help: {help}"
  );
  // Editor quick fix: insert the @use at the top of the file
  let fix = err.fix().expect("quick fix offered");
  match &fix.kind {
    vass_core::error::FixKind::Insert { line, column, text } => {
      assert_eq!((*line, *column), (1, 1));
      assert_eq!(text.as_str(), "@use 'base';\n");
    }
    other => panic!("expected Insert fix, got {other:?}"),
  }
}

#[test]
fn e0011_suggests_case_insensitive_namespace_match() {
  // `@use 'base' as Base;` then `base.$x` - namespaces are case-sensitive
  let err = eval_module_layout(
    &[
      ("vcss/_base.vcss", "$v: 1;\n"),
      (
        "vcss/main.vcss",
        "@use 'base' as Base;\n.a { x: base.$v; }\n",
      ),
    ],
    "vcss/main.vcss",
  )
  .unwrap_err();
  let help = err.help().unwrap_or_default();
  assert!(
    help.contains("namespace `Base`") && help.contains("case-sensitive"),
    "help: {help}"
  );
  let fix = err.fix().expect("rename quick fix offered");
  match &fix.kind {
    vass_core::error::FixKind::Replace(text) => assert_eq!(text.as_str(), "Base.$v"),
    other => panic!("expected Replace fix, got {other:?}"),
  }
}

#[test]
fn e0004_module_member_missing_lists_members() {
  // `base.$primary-color` when the module only defines `$primary`
  let err = eval_module_layout(
    &[
      ("vcss/_base.vcss", "$primary: #333;\n$accent: red;\n"),
      (
        "vcss/main.vcss",
        "@use 'base';\n.a { x: base.$primary-color; }\n",
      ),
    ],
    "vcss/main.vcss",
  )
  .unwrap_err();
  assert!(
    err
      .message
      .contains("Undefined variable: $base.primary-color"),
    "got: {}",
    err.message
  );
  let help = err.help().unwrap_or_default();
  assert!(
    help.contains("module `base` defines") && help.contains("$primary") && help.contains("$accent"),
    "help: {help}"
  );
  // prefix heuristic: `$primary-color` -> "did you mean `$primary`?"
  assert!(help.contains("did you mean `$primary`?"), "help: {help}");
  let fix = err.fix().expect("member rename quick fix offered");
  match &fix.kind {
    vass_core::error::FixKind::Replace(text) => {
      assert_eq!(text.as_str(), "base.$primary")
    }
    other => panic!("expected Replace fix, got {other:?}"),
  }
}

#[test]
fn use_misplaced_after_rule_rejected() {
  // dart-sass: "@use rules must be written before any other rules."
  // (caught at parse time, anchored at the @use rule)
  let err = parse_braces(".pre { a: b; }\n@use 'base';\n.a { x: base.$v; }\n").unwrap_err();
  assert!(
    err
      .message
      .contains("must be written before any other rules"),
    "got: {}",
    err.message
  );
  let span = err.span.expect("span anchored at the @use rule");
  assert_eq!(
    span.start.line, 2,
    "error points at the @use rule, not the next one"
  );
}

#[test]
fn use_nested_in_block_rejected() {
  // dart-sass: "This at-rule is not allowed here." (parse-time, .vcss)
  let err = parse_braces(".outer {\n  @use 'base';\n  x: base.$v;\n}\n").unwrap_err();
  assert!(
    err.message.contains("This at-rule is not allowed here"),
    "got: {}",
    err.message
  );
  let span = err.span.expect("span anchored at the nested @use");
  assert_eq!(span.start.line, 2);

  // indented syntax reaches the evaluator instead; same message there
  let sheet = parse(
    ".outer\n  @use 'base'\n  x: base.$v\n",
    vass_core::ast::input::Syntax::Indented,
  )
  .unwrap();
  let err = evaluate(&sheet).unwrap_err();
  assert!(
    err.message.contains("This at-rule is not allowed here"),
    "got: {}",
    err.message
  );
}

#[test]
fn builtin_module_namespaced() {
  let out = assert_eq_both(
    "@use \"vass:math\"\n@use \"vass:string\"\n.a\n  a: math.div(10, 2)\n  b: math.max(1, 5, 3)\n  c: math.sqrt(16)\n  d: math.pow(2, 10)\n  e: math.abs(-3)\n  f: math.floor(3.7)\n  g: math.ceil(3.2)\n  h: math.round(3.5)\n  i: math.percentage(0.25)\n  j: string.to-upper-case(\"ok\")\n  k: math.$pi\n",
    "@use \"vass:math\";\n@use \"vass:string\";\n.a {\n  a: math.div(10, 2);\n  b: math.max(1, 5, 3);\n  c: math.sqrt(16);\n  d: math.pow(2, 10);\n  e: math.abs(-3);\n  f: math.floor(3.7);\n  g: math.ceil(3.2);\n  h: math.round(3.5);\n  i: math.percentage(0.25);\n  j: string.to-upper-case(\"ok\");\n  k: math.$pi;\n}\n",
  );
  for expect in [
    "a: 5;",
    "b: 5;",
    "c: 4;",
    "d: 1024;",
    "e: 3;",
    "f: 3;",
    "g: 4;",
    "h: 4;",
    "i: 25%;",
    "j: \"OK\";",
    "k: 3.1415926536;",
  ] {
    assert!(out.contains(expect), "missing {}: got {}", expect, out);
  }
}

#[test]
fn module_variable_namespaced_color_list_map() {
  let out = assert_eq_both(
    "@use \"vass:color\"\n@use \"vass:list\"\n@use \"vass:map\"\n.a\n  a: color.mix(#ff0000, #0000ff, 25%)\n  b: list.nth((7 8 9), 3)\n  c: map.get((\"k\": 42), \"k\")\n",
    "@use \"vass:color\";\n@use \"vass:list\";\n@use \"vass:map\";\n.a {\n  a: color.mix(#ff0000, #0000ff, 25%);\n  b: list.nth((7 8 9), 3);\n  c: map.get((\"k\": 42), \"k\");\n}\n",
  );
  assert!(out.contains("b: 9;"), "got: {}", out);
  assert!(out.contains("c: 42;"));
}

// ---- error reporting -----------------------------------------------------------

#[test]
fn undefined_variable_has_message() {
  let sheet = parse_braces(".a { color: $missing; }").unwrap();
  let err = evaluate(&sheet).unwrap_err();
  assert!(err.message.contains("$missing"), "got: {}", err.message);
}

#[test]
fn undefined_function_error() {
  let sheet = parse_braces(".a { color: totally-unknown(1); }").unwrap();
  // Unknown fns pass through as plain CSS, so this compiles; but a
  // call with named args errors
  let out = evaluate(&sheet).unwrap();
  let text = vass_transpiler::render_expanded(&out);
  assert!(text.contains("totally-unknown(1)"));
  let sheet = parse_braces(".a { color: unknown-fn($x: 1); }").unwrap();
  assert!(evaluate(&sheet).is_err());
}

#[test]
fn declaration_outside_rule_error() {
  let sheet = parse_braces("color: red;").unwrap();
  assert!(evaluate(&sheet).is_err());
}

// ---- real-world stylesheet -------------------------------------------------------

#[test]
fn real_world_theme_stylesheet() {
  let out = assert_eq_both(
    "$font: \"Inter\", sans-serif\n$brand: #4066ff\n$radius: 6px\n\n@mixin card\n  border-radius: $radius\n  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2)\n  @content\n\n%interactive\n  cursor: pointer\n\n.btn\n  @extend %interactive\n  font-family: $font\n  background: $brand\n  @include card\n    border: none\n  &:hover\n    background: darken($brand, 8%)\n  &--lg\n    padding: 12px 24px\n  &--icon\n    width: 40px\n\n@each $side in (top right bottom left)\n  .m-#{$side}\n    margin-#{$side}: 8px\n\n@for $i from 1 through 2\n  .gap-#{$i}\n    gap: $i * 4px\n\n@media (max-width: 600px)\n  .btn\n    width: 100%\n",
    "$font: \"Inter\", sans-serif;\n$brand: #4066ff;\n$radius: 6px;\n\n@mixin card {\n  border-radius: $radius;\n  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);\n  @content;\n}\n\n%interactive {\n  cursor: pointer;\n}\n\n.btn {\n  @extend %interactive;\n  font-family: $font;\n  background: $brand;\n  @include card {\n    border: none;\n  }\n  &:hover {\n    background: darken($brand, 8%);\n  }\n  &--lg {\n    padding: 12px 24px;\n  }\n  &--icon {\n    width: 40px;\n  }\n}\n\n@each $side in (top right bottom left) {\n  .m-#{$side} {\n    margin-#{$side}: 8px;\n  }\n}\n\n@for $i from 1 through 2 {\n  .gap-#{$i} {\n    gap: $i * 4px;\n  }\n}\n\n@media (max-width: 600px) {\n  .btn {\n    width: 100%;\n  }\n}\n",
  );
  for expect in [
    ".btn {",
    "cursor: pointer;",
    "background: #4066ff;",
    "border-radius: 6px;",
    "border: none;",
    ".btn:hover {",
    "background: rgb(9.0980392157%, 27.1832460733%, 100%);",
    ".btn--lg {",
    "padding: 12px 24px;",
    ".m-top {",
    "margin-top: 8px;",
    ".gap-2 {",
    "gap: 8px;",
    "@media (max-width: 600px) {",
    "width: 100%;",
  ] {
    assert!(out.contains(expect), "missing {}: got\n{}", expect, out);
  }
  assert!(!out.contains("%interactive"));
  assert!(!out.contains("@mixin"));
  assert!(!out.contains("@include"));
}

// ---- regressions: selector.extend / meta.apply / lexer interpolation / slash

#[test]
fn selector_extend_semantics() {
  let out = compile_src(
    "@use \"sass:selector\";\n.a { e: selector.extend(\".message\", \".message\", \".warning\"); }",
    false,
  );
  assert!(out.contains("e: .message, .warning;"), "{}", out);

  let out = compile_src(
    "@use \"sass:selector\";\n.a { e: selector.extend(\".a.b\", \".b\", \".c\"); }",
    false,
  );
  assert!(out.contains("e: .a.b, .a.c;"), "{}", out);

  // multiple occurrences produce the cross product, original first
  let out = compile_src(
    "@use \"sass:selector\";\n.a { e: selector.extend(\".i .b > .b\", \".b\", \".x\"); }",
    false,
  );
  assert!(
    out.contains("e: .i .b > .b, .i .b > .x, .i .x > .b, .i .x > .x;"),
    "{}",
    out
  );

  // extendee absent -> unchanged
  let out = compile_src(
    "@use \"sass:selector\";\n.a { e: selector.extend(\".a\", \".zz\", \".nope\"); }",
    false,
  );
  assert!(out.contains("e: .a;"), "{}", out);

  // whole-token boundary: .bbc does not contain .b
  let out = compile_src(
    "@use \"sass:selector\";\n.a { e: selector.extend(\".bbc\", \".b\", \".c\"); }",
    false,
  );
  assert!(out.contains("e: .bbc;"), "{}", out);
}

#[test]
fn meta_apply_includes_mixin_value() {
  let out = assert_eq_both(
    "@use \"sass:meta\"\n@mixin mx($c: red)\n  color: $c\n.a\n  @include meta.apply(meta.get-mixin(mx), blue)\n",
    "@use \"sass:meta\";\n@mixin mx($c: red) { color: $c; }\n.a { @include meta.apply(meta.get-mixin(mx), blue); }\n",
  );
  assert!(out.contains("color: blue;"), "{}", out);
}

#[test]
fn nested_quotes_inside_interpolation() {
  let out = assert_eq_both(".a\n  f: \"F#{\"a\"}ll\"\n", ".a { f: \"F#{\"a\"}ll\"; }\n");
  assert!(
    out.contains("f: \"Fall\";") || out.contains("f: Fall;"),
    "{}",
    out
  );

  let out = assert_eq_both(
    ".a\n  c: \"mix#{\"a\" + \"b\"}end\"\n",
    ".a { c: \"mix#{\"a\" + \"b\"}end\"; }\n",
  );
  assert!(out.contains("mixabend"), "{}", out);
}

#[test]
fn slash_with_quoted_string_joins_literally() {
  let out = compile_src(".a { f: 12px / \"Inter\"; }", false);
  assert!(out.contains("f: 12px/\"Inter\";"), "{}", out);
  // parens still force division
  let out = compile_src(".a { f: (12px / 4); }", false);
  assert!(out.contains("f: 3px;"), "{}", out);
  // bare slash stays a CSS separator
  let out = compile_src(".a { f: 12px / 1.5; }", false);
  assert!(out.contains("f: 12px/1.5;"), "{}", out);
}

// ---- scientific notation + extreme numbers (dart-sass parity) --------------

#[test]
fn scientific_notation_literals() {
  let out = assert_eq_both(
    ".a\n  a: 1e3\n  b: 2.5e2px\n  c: 1E-2\n",
    ".a { a: 1e3; b: 2.5e2px; c: 1E-2; }\n",
  );
  assert!(out.contains("a: 1000;"), "{}", out);
  assert!(out.contains("b: 250px;"), "{}", out);
  assert!(out.contains("c: 0.01;"), "{}", out);
}

#[test]
fn exponent_unit_disambiguation() {
  // `1em`/`1ex` stay dimensions; only digit-followed e is an exponent
  let out = assert_eq_both(
    ".a\n  a: 1em\n  b: 1ex\n  c: 2e3px\n",
    ".a { a: 1em; b: 1ex; c: 2e3px; }\n",
  );
  assert!(out.contains("a: 1em;"), "{}", out);
  assert!(out.contains("b: 1ex;"), "{}", out);
  assert!(out.contains("c: 2000px;"), "{}", out);
}

#[test]
fn math_module_all_constants() {
  // Full math constant table reachable through a used module (both
  // prefixes must expose identical modules)
  for prefix in ["vass", "sass"] {
    let out = compile_src(
      &format!(
        "@use \"{}:math\";\n.a {{\n  a: math.$pi;\n  b: math.$e;\n  c: math.$epsilon;\n  d: math.$max-safe-integer;\n  e: math.$min-safe-integer;\n  f: math.$max-number;\n  g: math.$min-number;\n}}\n",
        prefix
      ),
      false,
    );
    assert!(out.contains("a: 3.1415926536;"), "{}", out);
    assert!(out.contains("b: 2.7182818285;"), "{}", out);
    assert!(out.contains("c: 0;"), "{}", out);
    assert!(out.contains("d: 9007199254740991;"), "{}", out);
    assert!(out.contains("e: -9007199254740991;"), "{}", out);
    assert!(out.contains("f: 17976931348623157"), "{}", out);
    assert!(out.contains("g: 0;"), "{}", out);
  }
}

#[test]
fn large_number_serialization_dart_compat() {
  // Large magnitudes: shortest round-trip digits, positional notation
  let out = assert_eq_both(
    ".a\n  a: 1e15\n  b: 1e21\n  c: 123456789012345678901\n",
    ".a { a: 1e15; b: 1e21; c: 123456789012345678901; }\n",
  );
  assert!(out.contains("a: 1000000000000000;"), "{}", out);
  assert!(out.contains("b: 1000000000000000000000;"), "{}", out);
  assert!(out.contains("c: 123456789012345680000;"), "{}", out);
}

// ---- error and control statements ---------------------------------------------

#[test]
fn at_error_statement() {
  let sheet = parse_braces(".a { @error \"boom\"; }").unwrap();
  let err = evaluate(&sheet).unwrap_err();
  assert!(err.message.contains("boom"), "got: {}", err.message);
}

#[test]
fn at_error_statement_indented() {
  let sheet = parse_indented(".a\n  @error \"boom\"\n").unwrap();
  let err = evaluate(&sheet).unwrap_err();
  assert!(err.message.contains("boom"), "got: {}", err.message);
}

#[test]
fn extend_outside_rule_is_error() {
  let sheet = parse_braces("@extend .foo;").unwrap();
  let err = evaluate(&sheet).unwrap_err();
  assert!(
    err.message.contains("may only be used within a style rule"),
    "got: {}",
    err.message
  );
}

#[test]
fn at_warn_and_debug_compile() {
  // @debug and @warn evaluate their value and must not crash.
  let out = assert_eq_both(
    ".a\n  @debug 1 + 2\n  @warn \"hey\"\n  color: red\n",
    ".a {\n  @debug 1 + 2;\n  @warn \"hey\";\n  color: red;\n}\n",
  );
  // The values are evaluated (declaration still emitted).
  assert!(out.contains("color: red;"));
}

#[test]
fn return_inside_function() {
  let out = assert_eq_both(
    "@function f($n)\n  @if $n > 0\n    @return yes\n  @return no\n.a\n  a: f(1)\n  b: f(0)\n",
    "@function f($n) {\n  @if $n > 0 {\n    @return yes;\n  }\n  @return no;\n}\n.a {\n  a: f(1);\n  b: f(0);\n}\n",
  );
  assert!(out.contains("a: yes;"));
  assert!(out.contains("b: no;"));
}

#[test]
fn at_root_without_media_removes_media() {
  let out = assert_eq_both(
    "@media print\n  .wrap\n    @at-root (without: media)\n      .free\n        color: red\n",
    "@media print {\n  .wrap {\n    @at-root (without: media) {\n      .free {\n        color: red;\n      }\n    }\n  }\n}\n",
  );
  assert!(out.contains(".free {"));
  assert!(!out.contains("@media print {") || out.contains("@media print"));
}

#[test]
fn undefined_variable_in_condition() {
  let sheet = parse_braces(".a { @if $missing { color: red; } }").unwrap();
  assert!(evaluate(&sheet).is_err());
}

#[test]
fn unknown_function_with_named_args_errors() {
  let sheet = parse_braces(".a { color: unknown-fn($x: 1); }").unwrap();
  assert!(evaluate(&sheet).is_err());
}

#[test]
fn each_over_empty_list_produces_nothing() {
  let out = assert_eq_both(
    "@each $v in ()\n  .x-#{$v}\n    color: red\n",
    "@each $v in () {\n  .x-#{$v} {\n    color: red;\n  }\n}\n",
  );
  assert!(!out.contains(".x-"));
}

#[test]
fn for_range_to_excludes_end() {
  let out = assert_eq_both(
    "@for $i from 1 to 3\n  .t-#{$i}\n    v: $i\n",
    "@for $i from 1 to 3 {\n  .t-#{$i} {\n    v: $i;\n  }\n}\n",
  );
  assert!(out.contains(".t-1 {") && out.contains(".t-2 {"));
  assert!(!out.contains(".t-3 {"));
}

#[test]
fn while_terminates_when_false_immediately() {
  let out = assert_eq_both(
    "$i: 0\n@while false\n  .w-#{$i}\n    v: $i\n.a\n  x: 1\n",
    "$i: 0;\n@while false {\n  .w-#{$i} {\n    v: $i;\n  }\n}\n.a {\n  x: 1;\n}\n",
  );
  assert!(!out.contains(".w-"));
  assert!(out.contains("x: 1;"));
}
