//! # VASS WASM
//!
//! WASM bindings for the VASS CSS preprocessor.
//!
//! ## Features
//!
//! - Browser-based compilation
//! - JavaScript interop
//! - Playground support

#![warn(missing_docs)]
#![allow(dead_code)]

use vass_core::error::Error;
use vass_parser::{Syntax, parse};
use vass_transpiler::{OutputStyle, render};
use wasm_bindgen::prelude::*;

/// Internal compile function: parse (braces first, then indented), evaluate,
/// and render with the given output style.
fn compile_inner(source: &str, output_style: OutputStyle) -> Result<String, Error> {
  // Try brace syntax first, then indented syntax.
  let sheet = parse(source, Syntax::Braces).or_else(|_| parse(source, Syntax::Indented))?;
  let css = vass_evaluator::evaluate_stylesheet(&sheet)?;
  Ok(render(&css, output_style))
}

/// Compile VASS source to CSS using expanded output style.
///
/// The syntax is auto-detected: brace syntax (`.vcss`) is tried first,
/// falling back to indented syntax (`.vass`).
#[wasm_bindgen]
pub fn compile(source: &str) -> Result<String, JsValue> {
  compile_inner(source, OutputStyle::Expanded).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Compile VASS source to CSS with a specific output style.
///
/// `style` must be `"expanded"` or `"compressed"`.
/// The syntax is auto-detected: brace syntax (`.vcss`) is tried first,
/// falling back to indented syntax (`.vass`).
#[wasm_bindgen]
pub fn compile_with_style(source: &str, style: &str) -> Result<String, JsValue> {
  let output_style = match style {
    "expanded" => OutputStyle::Expanded,
    "compressed" => OutputStyle::Compressed,
    _ => {
      return Err(JsValue::from_str(
        "invalid style: expected 'expanded' or 'compressed'",
      ));
    }
  };
  compile_inner(source, output_style).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Initialize the WASM module.
///
/// Sets up a panic hook so Rust panics are surfaced as JavaScript exceptions.
#[wasm_bindgen(start)]
pub fn init() {
  #[cfg(feature = "console_error_panic_hook")]
  console_error_panic_hook::set_once();
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn compile_basic_braces() {
    let result = compile_inner(".a { color: red; }", OutputStyle::Expanded).unwrap();
    assert_eq!(result, ".a {\n  color: red;\n}\n");
  }

  #[test]
  fn compile_basic_indented() {
    let result =
      compile_inner(".a\n  color: red\n  padding: 1px\n", OutputStyle::Expanded).unwrap();
    assert_eq!(result, ".a {\n  color: red;\n  padding: 1px;\n}\n");
  }

  #[test]
  fn compile_with_variables() {
    let result =
      compile_inner("$color: red;\n.a { color: $color; }", OutputStyle::Expanded).unwrap();
    assert!(result.contains("color: red"));
  }

  #[test]
  fn compile_with_nesting() {
    let result = compile_inner(".a { .b { color: red; } }", OutputStyle::Expanded).unwrap();
    assert!(result.contains(".a .b"));
  }

  #[test]
  fn compile_compressed_style() {
    let result = compile_inner(".a { color: red; }", OutputStyle::Compressed).unwrap();
    assert_eq!(result, ".a{color:red}\n");
  }

  #[test]
  fn compile_variables_and_math() {
    let result = compile_inner(
      "$base: 10px;\n.a { width: $base * 2; }",
      OutputStyle::Expanded,
    )
    .unwrap();
    assert!(result.contains("width: 20px"));
  }

  #[test]
  fn compile_empty_source() {
    let result = compile_inner("", OutputStyle::Expanded).unwrap();
    assert_eq!(result, "");
  }

  #[test]
  fn compile_comments() {
    let result = compile_inner("/* comment */\n.a { color: red; }", OutputStyle::Expanded).unwrap();
    assert!(result.contains(".a"));
    assert!(result.contains("comment"));
  }

  #[test]
  fn compile_syntax_error() {
    let result = compile_inner(".a { color: ; }", OutputStyle::Expanded);
    assert!(result.is_err());
  }

  #[test]
  fn compile_wasm_wrapper_braces() {
    let result = compile(".a { color: red; }").unwrap();
    assert_eq!(result, ".a {\n  color: red;\n}\n");
  }
}
