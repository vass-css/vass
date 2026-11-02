//! Comprehensive integration tests covering the full VASS pipeline:
//! parse → evaluate → transpile, plus unit tests for each crate.

use vass_evaluator::{EvalOptions, evaluate_stylesheet};
use vass_parser::{Syntax, parse};
use vass_transpiler::{OutputStyle, render};

fn compile(source: &str, syntax: Syntax) -> String {
  let sheet = parse(source, syntax).expect("parse failed");
  let css = evaluate_stylesheet(&sheet).expect("eval failed");
  render(&css, OutputStyle::Expanded)
}

// ── vass-core: value types ──
#[test]
fn core_value_null_bool_number() {
  let v = vass_core::value::Value::Null;
  assert!(!v.is_truthy());
  assert_eq!(v.type_name(), "null");

  let v = vass_core::value::Value::Bool(true);
  assert!(v.is_truthy());
  assert_eq!(v.type_name(), "bool");

  let n = vass_core::value::Number::unitless(42.0);
  let v = vass_core::value::Value::Number(n);
  assert_eq!(v.type_name(), "number");

  let s = vass_core::value::BassString::unquoted("hello");
  let v = vass_core::value::Value::Str(s);
  assert_eq!(v.type_name(), "string");
}

#[test]
fn core_value_list_map() {
  let list = vass_core::value::ListValue::new(
    vec![
      vass_core::value::Value::Number(vass_core::value::Number::unitless(1.0)),
      vass_core::value::Value::Null,
    ],
    vass_core::value::Separator::Space,
  );
  assert_eq!(list.to_css(), "1");

  let map = vass_core::value::BassMap::new();
  assert!(map.entries.is_empty());
  let v = vass_core::value::Value::Map(map);
  assert_eq!(v.type_name(), "map");
}

#[test]
fn core_value_color() {
  let c = vass_core::value::Color::rgb(255, 0, 0);
  assert_eq!(c.to_css(), "red");

  let c = vass_core::value::Color::from_hex("#ff0000").unwrap();
  assert_eq!(c.to_css(), "#ff0000");

  let c = vass_core::value::Color::from_named("rebeccapurple").unwrap();
  assert_eq!(c.hex(), "#663399");
}

#[test]
fn core_value_inspect_debug() {
  let v = vass_core::value::Value::Str(vass_core::value::BassString::quoted("hi".to_string()));
  assert_eq!(v.to_debug_string(), "hi");

  let v = vass_core::value::Value::Number(vass_core::value::Number::unitless(1.5));
  assert_eq!(v.inspect(), "1.5");
}

#[test]
fn core_values_equal() {
  use vass_core::value::values_equal;
  assert!(values_equal(
    &vass_core::value::Value::Null,
    &vass_core::value::Value::Null
  ));
  assert!(!values_equal(
    &vass_core::value::Value::Bool(true),
    &vass_core::value::Value::Bool(false)
  ));
}

#[test]
fn core_separator_text() {
  assert_eq!(vass_core::value::Separator::Space.text(), " ");
  assert_eq!(vass_core::value::Separator::Comma.text(), ", ");
  assert_eq!(vass_core::value::Separator::Slash.text(), " / ");
  assert_eq!(vass_core::value::Separator::Undecided.text(), " ");
}

#[test]
fn core_list_to_css() {
  let list = vass_core::value::ListValue::bracketed(
    vec![
      vass_core::value::Value::from("a"),
      vass_core::value::Value::from("b"),
    ],
    vass_core::value::Separator::Comma,
  );
  assert_eq!(list.to_css(), "[a, b]");
}

#[test]
fn core_map_ops() {
  let mut m = vass_core::value::BassMap::new();
  m.set(
    vass_core::value::Value::from("a"),
    vass_core::value::Value::from(1.0),
  );
  m.set(
    vass_core::value::Value::from("a"),
    vass_core::value::Value::from(2.0),
  );
  assert_eq!(m.entries.len(), 1);
  assert_eq!(
    m.get(&vass_core::value::Value::from("a")),
    Some(&vass_core::value::Value::from(2.0))
  );
  assert!(m.remove(&vass_core::value::Value::from("a")).is_some());
  assert!(m.entries.is_empty());
}

#[test]
fn core_map_deep_merge() {
  let mut a = vass_core::value::BassMap::new();
  a.set(
    vass_core::value::Value::from("color"),
    vass_core::value::Value::from(1.0),
  );
  let mut b = vass_core::value::BassMap::new();
  b.set(
    vass_core::value::Value::from("width"),
    vass_core::value::Value::from(2.0),
  );
  let merged = a.deep_merge(&b);
  assert_eq!(merged.entries.len(), 2);
}

// ── vass-core: token kinds ──
#[test]
fn core_token_kind_methods() {
  assert!(vass_core::token::Kind::Number.is_literal());
  assert!(!vass_core::token::Kind::Plus.is_literal());
  assert!(vass_core::token::Kind::Whitespace.is_whitespace());
  assert!(vass_core::token::Kind::SingleLineComment.is_comment());
  assert!(vass_core::token::Kind::Whitespace.is_trivia());
  assert!(vass_core::token::Kind::Plus.is_operator());
  assert!(vass_core::token::Kind::AtImport.is_at_rule_keyword());
  assert!(vass_core::token::Kind::Eof.is_eof());
}

#[test]
fn core_token_display() {
  assert_eq!(vass_core::token::Kind::Number.to_string(), "number");
  assert_eq!(vass_core::token::Kind::AtImport.to_string(), "@import");
  assert!(
    vass_core::token::Kind::Unknown('?')
      .to_string()
      .contains("unknown")
  );
}

// ── vass-core: token span ──
#[test]
fn core_token_span_methods() {
  let span = vass_core::token::Span::new(0, 10, 1, 1);
  assert_eq!(span.len(), 10);
  assert!(!span.is_empty());

  let pos = vass_core::token::Position::new(5, 1, 6);
  assert!(span.contains(pos));

  let merged = span.merge(&vass_core::token::Span::new(10, 15, 2, 1));
  assert_eq!(merged.end.offset, 15);
}

#[test]
fn core_token_struct() {
  let tok = vass_core::token::Token::simple(vass_core::token::Kind::Ident, "color");
  assert_eq!(tok.text, "color");
  assert!(!tok.is_eof());

  let eof = vass_core::token::Token::eof();
  assert!(eof.is_eof());
}

#[test]
fn core_file_span() {
  let fs = vass_core::token::FileSpan::new("test.vass", vass_core::token::Span::new(0, 5, 1, 1));
  assert_eq!(fs.to_string(), "test.vass:1:1");
  let dummy = vass_core::token::FileSpan::dummy();
  assert_eq!(dummy.to_string(), "1:1");
}

// ── vass-core: error types ──
#[test]
fn core_error_all_kinds() {
  assert!(matches!(
    vass_core::error::Error::parse("x").kind,
    vass_core::error::ErrorKind::Parse
  ));
  assert!(matches!(
    vass_core::error::Error::syntax("x").kind,
    vass_core::error::ErrorKind::Syntax
  ));
  assert!(matches!(
    vass_core::error::Error::type_error("x").kind,
    vass_core::error::ErrorKind::Type
  ));
  assert!(matches!(
    vass_core::error::Error::undefined_variable("x").kind,
    vass_core::error::ErrorKind::UndefinedVariable
  ));
  assert!(matches!(
    vass_core::error::Error::undefined_function("x").kind,
    vass_core::error::ErrorKind::UndefinedFunction
  ));
  assert!(matches!(
    vass_core::error::Error::undefined_mixin("x").kind,
    vass_core::error::ErrorKind::UndefinedMixin
  ));
  assert!(matches!(
    vass_core::error::Error::import("x").kind,
    vass_core::error::ErrorKind::Import
  ));
  assert!(matches!(
    vass_core::error::Error::io("x").kind,
    vass_core::error::ErrorKind::Io
  ));
  assert!(matches!(
    vass_core::error::Error::runtime("x").kind,
    vass_core::error::ErrorKind::Runtime
  ));
  assert!(matches!(
    vass_core::error::Error::internal("x").kind,
    vass_core::error::ErrorKind::Internal
  ));
}

#[test]
fn core_error_codes() {
  assert_eq!(vass_core::error::ErrorKind::Parse.code(), "E0001");
  assert_eq!(vass_core::error::ErrorKind::Internal.code(), "E0012");
}

#[test]
fn core_error_code_or_default() {
  let err = vass_core::error::Error::syntax("test");
  assert_eq!(err.code_or_default(), "E0001");
  let mut err = err;
  err = err.with_code("E0099");
  assert_eq!(err.code_or_default(), "E0099");
}

#[test]
fn core_error_with_help_fix() {
  let mut err = vass_core::error::Error::syntax("test");
  err = err.with_help("try this");
  assert_eq!(err.help(), Some("try this"));
  err = err.with_fix(
    "fix it",
    vass_core::error::FixKind::Replace("x".to_string()),
  );
  assert!(err.fix().is_some());
}

#[test]
fn core_error_notes() {
  let mut err = vass_core::error::Error::syntax("test");
  err = err.with_note("related");
  assert_eq!(err.notes.len(), 1);
}

#[test]
fn core_error_from_io() {
  let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
  let err: vass_core::error::Error = io_err.into();
  assert_eq!(err.kind, vass_core::error::ErrorKind::Io);
}

#[test]
fn core_error_display() {
  let err = vass_core::error::Error::parse("test")
    .with_file("test.vass")
    .with_span(vass_core::token::Span::new(0, 10, 1, 5));
  let s = err.to_string();
  assert!(s.contains("test.vass"));
  assert!(s.contains("1:5"));
}

// ── vass-core: util character ──
#[test]
fn core_util_character() {
  assert!(vass_core::util::is_ident_start('a'));
  assert!(!vass_core::util::is_ident_start('0'));
  assert!(vass_core::util::is_hex_digit('a'));
  assert!(vass_core::util::is_newline('\n'));
  assert!(vass_core::util::is_whitespace(' '));
  assert_eq!(vass_core::util::hex_digit_value('F'), Some(15));
  assert_eq!(vass_core::util::escape_char('a'), "\\a");
}

// ── vass-core: util string ──
#[test]
fn core_util_string() {
  assert!(vass_core::util::is_valid_ident("color"));
  assert!(!vass_core::util::is_valid_ident("123"));
  assert!(vass_core::util::needs_quoting("123"));
  assert!(!vass_core::util::needs_quoting("color"));
  assert_eq!(vass_core::util::to_css_ident("color"), "color");
  assert!(vass_core::util::is_color_name("red"));
  assert!(!vass_core::util::is_color_name("notacolor"));
  assert_eq!(vass_core::util::count_newlines("a\nb\n"), 2);
  assert_eq!(vass_core::util::get_indentation("  hi"), 2);
}

// ── vass-core: value string ──
#[test]
fn core_bass_string() {
  assert_eq!(
    vass_core::value::BassString::unquoted("solid").to_css(),
    "solid"
  );
  assert_eq!(
    vass_core::value::BassString::quoted("hello").to_css(),
    "\"hello\""
  );
  assert_eq!(
    vass_core::value::BassString::quoted("a\"b").to_css(),
    "'a\"b'"
  );
}

// ── vass-core: ast css ──
#[test]
fn core_css_stylesheet() {
  let sheet = vass_core::ast::css::CssStylesheet::default();
  assert!(sheet.nodes.is_empty());
}

// ── vass-parser: tokenizer ──
#[test]
fn parser_tokenize_various() {
  let toks = vass_parser::lexer::tokenize(".a { color: red; }").unwrap();
  assert!(toks.last().unwrap().kind == vass_parser::lexer::Tok::Eof);

  let toks = vass_parser::lexer::tokenize("#fff #x").unwrap();
  assert_eq!(toks[0].kind, vass_parser::lexer::Tok::Color);
  assert!(
    toks.iter().any(|t| t.kind == vass_parser::lexer::Tok::Hash),
    "no Hash token found in {:?}",
    toks
  );

  let toks = vass_parser::lexer::tokenize("// comment\n/* multi */").unwrap();
  assert!(toks[0].is_silent_comment());
  assert!(toks[2].is_loud_comment());

  let toks = vass_parser::lexer::tokenize("$x: 1 !default").unwrap();
  assert!(
    toks.iter().any(|t| t.is_flag()),
    "no flag token found in {:?}",
    toks
  );
}

// ── vass-parser: parser entry points ──
#[test]
fn parser_parse_all_syntaxes() {
  assert!(parse(".a { color: red; }", Syntax::Braces).is_ok());
  assert!(parse(".a\n  color: red\n", Syntax::Indented).is_ok());

  let path = std::path::Path::new("test.vass");
  assert!(vass_parser::parse_file(".a\n  color: red\n", path).is_ok());
  let path = std::path::Path::new("test.vcss");
  assert!(vass_parser::parse_file(".a { color: red; }", path).is_ok());
  let path = std::path::Path::new("test.css");
  assert_eq!(vass_parser::detect_syntax(path), Syntax::Braces);
}

#[test]
fn parser_parse_errors() {
  assert!(parse(".a { color: ; }", Syntax::Braces).is_err());
  assert!(parse("@else { }", Syntax::Braces).is_err());
}

// ── vass-parser: expression parsing ──
#[test]
fn parser_expressions() {
  use vass_core::ast::input::expression::{BinaryOp, Expr};
  let mut p = vass_parser::parser::base::Parser::new("1 + 2 * 3").unwrap();
  let e = p.parse_value_expr().unwrap();
  assert!(matches!(
    e,
    Expr::Binary {
      op: BinaryOp::Add,
      ..
    }
  ));

  let e = vass_parser::parser::base::Parser::new("12px/30px")
    .unwrap()
    .parse_value_expr()
    .unwrap();
  assert!(matches!(
    e,
    Expr::Binary {
      op: BinaryOp::Div,
      allow_slash_div: false,
      ..
    }
  ));

  let e = vass_parser::parser::base::Parser::new("$a/2")
    .unwrap()
    .parse_value_expr()
    .unwrap();
  assert!(matches!(
    e,
    Expr::Binary {
      op: BinaryOp::Div,
      allow_slash_div: true,
      ..
    }
  ));

  let e = vass_parser::parser::base::Parser::new("true or false and not true")
    .unwrap()
    .parse_value_expr()
    .unwrap();
  assert!(matches!(
    e,
    Expr::Binary {
      op: BinaryOp::Or,
      ..
    }
  ));
}

// ── vass-parser: statement parsing ──
#[test]
fn parser_statements_vcss() {
  let sheet = parse(".a { color: red; width: 10px; }", Syntax::Braces).unwrap();
  assert_eq!(sheet.body.len(), 1);

  let sheet = parse("$x: 10px;\n.a { width: $x; }", Syntax::Braces).unwrap();
  assert!(matches!(
    sheet.body[0],
    vass_core::ast::input::statement::Statement::VariableDecl { .. }
  ));

  let sheet = parse(
    "@media (min-width: 100px) { .a { color: red; } }",
    Syntax::Braces,
  )
  .unwrap();
  assert!(matches!(
    sheet.body[0],
    vass_core::ast::input::statement::Statement::AtRule { .. }
  ));

  let sheet = parse("@mixin flex($dir: row) { display: flex; }", Syntax::Braces).unwrap();
  assert!(matches!(
    sheet.body[0],
    vass_core::ast::input::statement::Statement::MixinDecl { .. }
  ));

  let sheet = parse(
    "@if true { .x { a: b } } @else { .y { a: b } }",
    Syntax::Braces,
  )
  .unwrap();
  assert!(matches!(
    sheet.body[0],
    vass_core::ast::input::statement::Statement::If { .. }
  ));
}

#[test]
fn parser_statements_indented() {
  let sheet = parse(".a\n  color: red\n  padding: 1px\n", Syntax::Indented).unwrap();
  assert_eq!(sheet.body.len(), 1);
}

// ── full pipeline: basic ──
#[test]
fn pipeline_basic_css() {
  let out = compile(".a { color: red; }", Syntax::Braces);
  assert!(out.contains(".a"));
  assert!(out.contains("color: red"));
}

#[test]
fn pipeline_variables() {
  let out = compile("$color: red;\n.a { color: $color; }", Syntax::Braces);
  assert!(out.contains("color: red"));
}

#[test]
fn pipeline_nesting() {
  let out = compile(".a { .b { color: red; } }", Syntax::Braces);
  assert!(out.contains(".a .b"));
}

#[test]
fn pipeline_parent_selector() {
  let out = compile(".a { &:hover { color: red; } }", Syntax::Braces);
  assert!(out.contains(".a:hover"));
}

#[test]
fn pipeline_control_flow() {
  let out = compile(
    ".a { @if true { color: red; } @else { color: blue; } }",
    Syntax::Braces,
  );
  assert!(out.contains("color: red"));
}

#[test]
fn pipeline_control_flow_false() {
  let out = compile(
    ".a { @if false { color: red; } @else { color: blue; } }",
    Syntax::Braces,
  );
  assert!(out.contains("color: blue"));
}

#[test]
fn pipeline_each() {
  let out = compile(
    ".a { @each $v in (1px, 2px) { margin: $v; } }",
    Syntax::Braces,
  );
  assert!(out.contains("margin: 1px"));
  assert!(out.contains("margin: 2px"));
}

#[test]
fn pipeline_for() {
  let out = compile(
    ".a { @for $i from 1 through 3 { margin: $i * 1px; } }",
    Syntax::Braces,
  );
  assert!(out.contains("margin: 1px"));
  assert!(out.contains("margin: 2px"));
  assert!(out.contains("margin: 3px"));
}

#[test]
fn pipeline_while() {
  let out = compile(
    ".a { $i: 0; @while $i < 3 { margin: $i * 1px; $i: $i + 1; } }",
    Syntax::Braces,
  );
  assert!(out.contains("margin: 0px"));
  assert!(out.contains("margin: 1px"));
  assert!(out.contains("margin: 2px"));
}

#[test]
fn pipeline_interpolation_selector() {
  let out = compile("$ns: test;\n.#{$ns} { color: red; }", Syntax::Braces);
  assert!(out.contains(".test"));
}

#[test]
fn pipeline_interpolation_value() {
  let out = compile(
    "$ns: test;\n.a { content: \"a-#{$ns}-b\"; }",
    Syntax::Braces,
  );
  assert!(out.contains("a-test-b"));
}

#[test]
fn pipeline_math_functions() {
  let out = compile(
    "@use \"vass:math\";\n.a { width: math.abs(-5px); }",
    Syntax::Braces,
  );
  assert!(out.contains("width: 5px"));

  let out = compile(
    "@use \"vass:math\";\n.a { width: math.ceil(3.5px); }",
    Syntax::Braces,
  );
  assert!(out.contains("width: 4px"));

  let out = compile(
    "@use \"vass:math\";\n.a { width: math.floor(3.5px); }",
    Syntax::Braces,
  );
  assert!(out.contains("width: 3px"));

  let out = compile(
    "@use \"vass:math\";\n.a { width: math.max(1px, 10px); }",
    Syntax::Braces,
  );
  assert!(out.contains("width: 10px"));

  let out = compile(
    "@use \"vass:math\";\n.a { width: math.min(1px, 10px); }",
    Syntax::Braces,
  );
  assert!(out.contains("width: 1px"));
}

#[test]
fn pipeline_color_functions() {
  let out = compile(
    "@use \"vass:color\";\n.a { color: color.red(red); }",
    Syntax::Braces,
  );
  assert!(!out.is_empty(), "color.red output was empty: {:?}", out);

  let out = compile(
    "@use \"vass:color\";\n.a { color: color.mix(red, blue, 50%); }",
    Syntax::Braces,
  );
  assert!(
    out.contains("purple") || out.contains("800080") || out.contains("50%") || out.contains("128")
  );

  let out = compile(
    "@use \"vass:color\";\n.a { color: color.invert(red); }",
    Syntax::Braces,
  );
  assert!(
    out.contains("aqua")
      || out.contains("#00ffff")
      || out.contains("0, 255, 255")
      || out.contains("0,255,255")
  );
}

#[test]
fn pipeline_string_functions() {
  let out = compile(
    "@use \"vass:string\";\n.a { content: string.to-upper-case(hello); }",
    Syntax::Braces,
  );
  assert!(out.contains("HELLO"));

  let out = compile(
    "@use \"vass:string\";\n.a { content: string.length(\"hello\"); }",
    Syntax::Braces,
  );
  assert!(out.contains("5"));
}

#[test]
fn pipeline_list_functions() {
  let out = compile(
    "@use \"vass:list\";\n.a { content: list.length(1px 2px 3px); }",
    Syntax::Braces,
  );
  assert!(out.contains("3"));

  let out = compile(
    "@use \"vass:list\";\n.a { content: list.nth(1px 2px 3px, 2); }",
    Syntax::Braces,
  );
  assert!(out.contains("2px"));
}

#[test]
fn pipeline_map_functions() {
  let out = compile(
    "@use \"vass:map\";\n.a { content: map.get((color: red, width: 10px), color); }",
    Syntax::Braces,
  );
  assert!(out.contains("red"));
}

#[test]
fn pipeline_meta_functions() {
  let out = compile(
    "@use \"vass:meta\";\n.a { content: meta.type-of(12px); }",
    Syntax::Braces,
  );
  assert!(out.contains("number"));

  let _out = compile(
    "@use \"vass:meta\";\n.a { content: meta.inspect(red); }",
    Syntax::Braces,
  );
}

#[test]
fn pipeline_mixin() {
  let out = compile(
    "@mixin flex($dir: row) { display: flex; }\n.a { @include flex; }",
    Syntax::Braces,
  );
  assert!(out.contains("display: flex"));
}

#[test]
fn pipeline_mixin_content() {
  let out = compile(
    "@mixin wrapper { @content; }\n.a { @include wrapper { color: red; } }",
    Syntax::Braces,
  );
  assert!(out.contains("color: red"));
}

#[test]
fn pipeline_function() {
  let out = compile(
    "@function double($n) { @return $n * 2; }\n.a { width: double(10px); }",
    Syntax::Braces,
  );
  assert!(out.contains("width: 20px"));
}

#[test]
fn pipeline_extend() {
  let _out = compile(".a { color: red; }\n.b { @extend .a; }", Syntax::Braces);
}

#[test]
fn pipeline_debug_warn() {
  let _out = compile(".a { @debug 12px; }", Syntax::Braces);
  let _out = compile(".a { @warn \"test\"; }", Syntax::Braces);
  let sheet = parse(".a { @error \"boom\"; }", Syntax::Braces).unwrap();
  let result = evaluate_stylesheet(&sheet);
  assert!(result.is_err());
}

#[test]
fn pipeline_import_css() {
  let out = compile("@import \"https://example.com/style.css\";", Syntax::Braces);
  assert!(out.contains("https://example.com/style.css"));
}

#[test]
fn pipeline_empty() {
  let out = compile("", Syntax::Braces);
  assert_eq!(out, "");
}

#[test]
fn pipeline_compressed() {
  let sheet = parse(".a { color: red; }", Syntax::Braces).unwrap();
  let css = evaluate_stylesheet(&sheet).unwrap();
  let out = vass_transpiler::render(&css, OutputStyle::Compressed);
  assert_eq!(out, ".a{color:red}\n");
}

#[test]
fn pipeline_indented_syntax() {
  let out = compile(".a\n  color: red\n  padding: 1px\n", Syntax::Indented);
  assert!(out.contains(".a"));
  assert!(out.contains("color: red"));
  assert!(out.contains("padding: 1px"));
}

// ── vass-core: number arithmetic ──
#[test]
fn core_number_add_sub_mul_div() {
  use vass_core::value::Number;
  let r = Number::with_unit(1.0, "px")
    .add(&Number::unitless(2.0))
    .unwrap();
  assert_eq!(r.to_css(), "3px");

  let r = Number::with_unit(2.0, "px")
    .mul(&Number::unitless(3.0))
    .unwrap();
  assert_eq!(r.to_css(), "6px");

  let r = Number::with_unit(10.0, "px")
    .div(&Number::unitless(4.0))
    .unwrap();
  assert_eq!(r.to_css(), "2.5px");
}

#[test]
fn core_number_modulo() {
  use vass_core::value::Number;
  let r = Number::unitless(7.0)
    .modulo(&Number::unitless(3.0))
    .unwrap();
  assert_eq!(r.to_css(), "1");
}

#[test]
fn core_number_incompatible_units() {
  use vass_core::value::Number;
  let a = Number::with_unit(1.0, "px");
  let b = Number::with_unit(1.0, "em");
  assert!(a.add(&b).is_err());
}

#[test]
fn core_number_compound() {
  use vass_core::value::Number;
  let prod = Number::with_unit(5.0, "px")
    .mul(&Number::with_unit(10.0, "px"))
    .unwrap();
  assert_eq!(prod.to_css(), "calc(50px * 1px)");
}

#[test]
fn core_number_div_by_zero() {
  use vass_core::value::Number;
  let r = Number::unitless(1.0).div(&Number::unitless(0.0)).unwrap();
  assert_eq!(r.to_css(), "calc(infinity)");
}

#[test]
fn core_number_negate_zero() {
  use vass_core::value::Number;
  assert!(Number::unitless(0.0).is_zero());
  assert_eq!(Number::unitless(5.0).negate().to_css(), "-5");
}

#[test]
fn core_number_format_f64() {
  assert_eq!(vass_core::value::format_f64(0.0000000001), "0.0000000001");
  assert_eq!(vass_core::value::format_f64(-0.0), "-0");
  assert_eq!(vass_core::value::format_f64_compressed(0.5), ".5");
}

// ── vass-core: color functions ──
#[test]
fn core_color_adjust_lightness_saturation_hue() {
  let c = vass_core::value::Color::from_hex("#336699").unwrap();
  let lightened = c.adjust_lightness(0.1).to_css();
  assert!(lightened.contains("rgb") || lightened.contains("#"));
  assert_ne!(c.adjust_hue(240.0).hex(), c.hex());
}

#[test]
fn core_color_complement_grayscale_invert() {
  let c = vass_core::value::Color::from_hex("#ff0000").unwrap();
  assert_eq!(c.complement().hex(), "#00ffff");
  assert!(c.grayscale().to_css().contains("rgb"));
  assert_eq!(c.invert(1.0).to_css(), "aqua");
}

#[test]
fn core_color_opacify_transparentize_mix() {
  let c = vass_core::value::Color::rgba(255, 0, 0, 0.5);
  let result = c.opacify(0.5).to_css();
  assert!(result == "red" || result == "rgba(255, 0, 0, 1)" || result.contains("255"));

  let a = vass_core::value::Color::from_hex("#ff0000").unwrap();
  let b = vass_core::value::Color::from_hex("#0000ff").unwrap();
  assert_eq!(a.mix(&b, 0.5).hex(), "#800080");
}

#[test]
fn core_color_from_hwb_hsl_raw() {
  let c = vass_core::value::Color::from_hwb_channels([Some(0.0), Some(0.0), Some(0.0)], 1.0);
  assert_eq!(c.to_css(), "hwb(0 0% 0%)");

  let c = vass_core::value::Color::from_hsl_raw(0.0, 200.0, 150.0, 1.0);
  assert!(c.is_hsl_space());
}

#[test]
fn core_color_missing_channels() {
  let c =
    vass_core::value::Color::from_legacy_channels([None, Some(255.0), Some(0.0)], None, false);
  assert!(c.has_missing());
  assert_eq!(c.first_missing_channel(), Some("red"));
}

// ── vass-evaluator: eval options ──
#[test]
fn evaluator_options_default() {
  let opts = EvalOptions::default();
  assert!(opts.entry.is_none());
  assert!(opts.load_paths.is_empty());
}

// ── vass-evaluator: error paths ──
#[test]
fn evaluator_undefined_variable() {
  let sheet = parse(".a { color: $undefined_var; }", Syntax::Braces).unwrap();
  let result = evaluate_stylesheet(&sheet);
  assert!(result.is_err());
}

#[test]
fn evaluator_empty_block() {
  let sheet = parse(".a { }", Syntax::Braces).unwrap();
  let result = evaluate_stylesheet(&sheet);
  assert!(result.is_ok());
}

#[test]
fn evaluator_interpolation_selector() {
  let sheet = compile("$ns: test;\n.#{$ns} { color: red; }", Syntax::Braces);
  assert!(sheet.contains(".test"));
}

// ── vass-transpiler: source_map ──
#[test]
fn transpiler_source_map_none() {
  let sheet = vass_core::ast::css::CssStylesheet { nodes: vec![] };
  assert_eq!(vass_transpiler::source_map::generate(&sheet), None);
}
