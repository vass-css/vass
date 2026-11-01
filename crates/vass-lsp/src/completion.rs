//! Completion and hover providers, backed by a curated table of VASS
//! built-in functions and at-rules plus the per-document symbol index.

use serde_json::Value;

use crate::analysis::{Index, SymbolKind};

/// One built-in function / at-rule entry
pub struct BuiltinEntry {
  /// Callable name (`floor`, `adjust-color`, ...)
  pub name: &'static str,
  /// Signature shown in completion detail / hover
  pub signature: &'static str,
  /// Owning module (`math`, `color`, ...); empty for globals
  pub module: &'static str,
  /// Short documentation
  pub doc: &'static str,
}

macro_rules! entries {
    ($($name:literal [$module:literal] ($sig:literal) => $doc:literal),* $(,)?) => {
        &[ $(BuiltinEntry { name: $name, signature: $sig, module: $module, doc: $doc }),* ]
    };
}

/// Built-in functions by module (names match the evaluator's registry)
pub static BUILTINS: &[BuiltinEntry] = entries! {
    // ---- global ----
    "rgb" [""] ("rgb($red, $green, $blue)") => "Create a color from red, green and blue channels",
    "rgba" [""] ("rgba($red, $green, $blue, $alpha)") => "Create a color with alpha transparency",
    "hsl" [""] ("hsl($hue, $saturation, $lightness)") => "Create a color from HSL channels",
    "hsla" [""] ("hsla($hue, $saturation, $lightness, $alpha)") => "HSL color with alpha",
    "if" [""] ("if($condition, $if-true, $if-false)") => "Conditional expression, lazily evaluated",
    "type-of" [""] ("type-of($value)") => "Type of a value: number, string, color, list, map, bool, null, function, arglist",
    "inspect" [""] ("inspect($value)") => "Source-like representation of any value",
    "quote" [""] ("quote($string)") => "Add quotes to a string",
    "unquote" [""] ("unquote($string)") => "Remove quotes from a string",
    "lighten" [""] ("lighten($color, $amount)") => "Increase HSL lightness",
    "darken" [""] ("darken($color, $amount)") => "Decrease HSL lightness",
    "saturate" [""] ("saturate($color, $amount)") => "Increase HSL saturation",
    "desaturate" [""] ("desaturate($color, $amount)") => "Decrease HSL saturation",
    "opacify" [""] ("opacify($color, $amount)") => "Increase opacity (alias fade-in)",
    "transparentize" [""] ("transparentize($color, $amount)") => "Decrease opacity (alias fade-out)",
    "adjust-hue" [""] ("adjust-hue($color, $degrees)") => "Rotate the hue of a color",
    "change-color" [""] ("change-color($color, $args...)") => "Change one or more HSL/RGB properties",
    "scale-color" [""] ("scale-color($color, $args...)") => "Scale a color's properties fluidly",
    "mix" [""] ("mix($color1, $color2, $weight)") => "Blend two colors",
    "invert" [""] ("invert($color, $weight)") => "Invert a color",
    "grayscale" [""] ("grayscale($color)") => "Convert a color to grayscale",
    "complement" [""] ("complement($color)") => "Return the complementary color",
    "ie-hex-str" [""] ("ie-hex-str($color)") => "IE-compatible #AARRGGBB hex",
    "red" [""] ("red($color)") => "Red channel of a color",
    "green" [""] ("green($color)") => "Green channel of a color",
    "blue" [""] ("blue($color)") => "Blue channel of a color",
    "hue" [""] ("hue($color)") => "Hue of a color in degrees",
    "saturation" [""] ("saturation($color)") => "HSL saturation of a color",
    "lightness" [""] ("lightness($color)") => "HSL lightness of a color",
    "alpha" [""] ("alpha($color)") => "Alpha channel of a color (alias opacity)",
    "opacity" [""] ("opacity($color)") => "Alpha channel of a color",
    "length" [""] ("length($list)") => "Number of elements in a list",
    "nth" [""] ("nth($list, $n)") => "Element at 1-based index n",
    "set-nth" [""] ("set-nth($list, $n, $value)") => "Replace the element at index n",
    "join" [""] ("join($list1, $list2, $separator)") => "Join two lists",
    "append" [""] ("append($list, $val, $separator)") => "Append a value to a list",
    "zip" [""] ("zip($lists...)") => "Zip lists into a list of tuples",
    "index" [""] ("index($list, $value)") => "1-based index of a value in a list",
    "list-separator" [""] ("list-separator($list)") => "Separator of a list: comma or space",
    "is-bracketed" [""] ("is-bracketed($list)") => "Whether a list uses [] brackets",
    "map-get" [""] ("map-get($map, $key)") => "Value for a key in a map",
    "map-has-key" [""] ("map-has-key($map, $key)") => "Whether a map contains a key",
    "map-keys" [""] ("map-keys($map)") => "All keys of a map",
    "map-values" [""] ("map-values($map)") => "All values of a map",
    "map-merge" [""] ("map-merge($map1, $map2)") => "Merge two maps",
    "map-remove" [""] ("map-remove($map, $keys...)") => "Remove keys from a map",
    "floor" [""] ("floor($number)") => "Round down",
    "ceil" [""] ("ceil($number)") => "Round up",
    "round" [""] ("round($number)") => "Round to nearest",
    "abs" [""] ("abs($number)") => "Absolute value",
    "min" [""] ("min($numbers...)") => "Smallest of several values",
    "max" [""] ("max($numbers...)") => "Largest of several values",
    "random" [""] ("random($limit)") => "Random number",
    "percentage" [""] ("percentage($number)") => "Convert a unitless number to a percentage",
    "unit" [""] ("unit($number)") => "Unit of a number as a string",
    "unitless" [""] ("unitless($number)") => "Whether a number has no unit",
    "comparable" [""] ("comparable($number1, $number2)") => "Whether two numbers can be added",
    "variable-exists" [""] ("variable-exists($name)") => "Whether a variable is defined",
    "global-variable-exists" [""] ("global-variable-exists($name)") => "Whether a global variable is defined",
    "function-exists" [""] ("function-exists($name)") => "Whether a function is defined",
    "mixin-exists" [""] ("mixin-exists($name)") => "Whether a mixin is defined",
    "content-exists" [""] ("content-exists()") => "Whether the current mixin was passed a @content block",
    "feature-exists" [""] ("feature-exists($feature)") => "Whether a VASS feature is supported",
    "get-function" [""] ("get-function($name, $css)") => "Look up a function value",
    "get-mixin" [""] ("get-mixin($name)") => "Look up a mixin value",
    "call" [""] ("call($function, $args...)") => "Dynamically invoke a function",
    "unique-id" [""] ("unique-id()") => "Generate a unique CSS id",
    // ---- sass:math ----
    "ceil" ["math"] ("math.ceil($number)") => "Round up",
    "floor" ["math"] ("math.floor($number)") => "Round down",
    "round" ["math"] ("math.round($number)") => "Round to nearest",
    "abs" ["math"] ("math.abs($number)") => "Absolute value",
    "max" ["math"] ("math.max($numbers...)") => "Largest value",
    "min" ["math"] ("math.min($numbers...)") => "Smallest value",
    "hypot" ["math"] ("math.hypot($numbers...)") => "Euclidean norm",
    "log" ["math"] ("math.log($number, $base)") => "Logarithm",
    "pow" ["math"] ("math.pow($base, $exponent)") => "Exponentiation",
    "sqrt" ["math"] ("math.sqrt($number)") => "Square root",
    "cos" ["math"] ("math.cos($number)") => "Cosine",
    "sin" ["math"] ("math.sin($number)") => "Sine",
    "tan" ["math"] ("math.tan($number)") => "Tangent",
    "acos" ["math"] ("math.acos($number)") => "Inverse cosine",
    "asin" ["math"] ("math.asin($number)") => "Inverse sine",
    "atan" ["math"] ("math.atan($number)") => "Inverse tangent",
    "atan2" ["math"] ("math.atan2($y, $x)") => "Two-argument arctangent",
    "exp" ["math"] ("math.exp($number)") => "e raised to a power",
    "percentage" ["math"] ("math.percentage($number)") => "Convert to a percentage",
    "random" ["math"] ("math.random($limit)") => "Random number",
    "unit" ["math"] ("math.unit($number)") => "Unit of a number",
    "unitless" ["math"] ("math.unitless($number)") => "Whether a number has no unit",
    "compatible" ["math"] ("math.compatible($number1, $number2)") => "Whether two numbers can be added",
    "is-unitless" ["math"] ("math.is-unitless($number)") => "Whether a number has no unit",
    "clamp" ["math"] ("math.clamp($min, $number, $max)") => "Clamp a number into a range",
    "div" ["math"] ("math.div($number1, $number2)") => "Division",
    // ---- sass:color ----
    "adjust" ["color"] ("color.adjust($color, $args...)") => "Adjust color channels by relative amounts",
    "scale" ["color"] ("color.scale($color, $args...)") => "Scale color channels fluidly",
    "change" ["color"] ("color.change($color, $args...)") => "Set color channels to new values",
    "mix" ["color"] ("color.mix($color1, $color2, $weight)") => "Blend two colors",
    "invert" ["color"] ("color.invert($color, $weight)") => "Invert a color",
    "complement" ["color"] ("color.complement($color)") => "Complementary color",
    "grayscale" ["color"] ("color.grayscale($color)") => "Grayscale a color",
    "lighten" ["color"] ("color.lighten($color, $amount)") => "Increase lightness",
    "darken" ["color"] ("color.darken($color, $amount)") => "Decrease lightness",
    "saturate" ["color"] ("color.saturate($color, $amount)") => "Increase saturation",
    "desaturate" ["color"] ("color.desaturate($color, $amount)") => "Decrease saturation",
    "opacify" ["color"] ("color.opacify($color, $amount)") => "Increase opacity",
    "transparentize" ["color"] ("color.transparentize($color, $amount)") => "Decrease opacity",
    "adjust-hue" ["color"] ("color.adjust-hue($color, $degrees)") => "Rotate hue",
    "red" ["color"] ("color.red($color)") => "Red channel",
    "green" ["color"] ("color.green($color)") => "Green channel",
    "blue" ["color"] ("color.blue($color)") => "Blue channel",
    "hue" ["color"] ("color.hue($color)") => "Hue in degrees",
    "saturation" ["color"] ("color.saturation($color)") => "HSL saturation",
    "lightness" ["color"] ("color.lightness($color)") => "HSL lightness",
    "alpha" ["color"] ("color.alpha($color)") => "Alpha channel",
    "blackness" ["color"] ("color.blackness($color)") => "HWB blackness",
    "whiteness" ["color"] ("color.whiteness($color)") => "HWB whiteness",
    "channel" ["color"] ("color.channel($color, $channel)") => "Value of a named channel",
    "is-in-gamut" ["color"] ("color.is-in-gamut($color)") => "Whether a color is in gamut",
    "to-gamut" ["color"] ("color.to-gamut($color, $method)") => "Map a color into gamut",
    "is-legacy" ["color"] ("color.is-legacy($color)") => "Whether a color is in the legacy space",
    "ie-hex-str" ["color"] ("color.ie-hex-str($color)") => "IE-compatible hex",
    "space" ["color"] ("color.space($color)") => "Color space of a color",
    "to-space" ["color"] ("color.to-space($color, $space)") => "Convert to a color space",
    "same" ["color"] ("color.same($color1, $color2)") => "Whether two colors are identical",
    // ---- sass:string ----
    "quote" ["string"] ("string.quote($string)") => "Add quotes",
    "unquote" ["string"] ("string.unquote($string)") => "Remove quotes",
    "to-upper-case" ["string"] ("string.to-upper-case($string)") => "Uppercase a string",
    "to-lower-case" ["string"] ("string.to-lower-case($string)") => "Lowercase a string",
    "length" ["string"] ("string.length($string)") => "Number of characters",
    "insert" ["string"] ("string.insert($string, $insert, $index)") => "Insert a substring",
    "index" ["string"] ("string.index($string, $substring)") => "First index of a substring",
    "slice" ["string"] ("string.slice($string, $start-at, $end-at)") => "Substring slice",
    "split" ["string"] ("string.split($string, $separator, $limit)") => "Split into a list",
    "unique-id" ["string"] ("string.unique-id()") => "Unique CSS id",
    // ---- sass:list ----
    "length" ["list"] ("list.length($list)") => "Number of elements",
    "nth" ["list"] ("list.nth($list, $n)") => "Element at index n",
    "set-nth" ["list"] ("list.set-nth($list, $n, $value)") => "Replace element at index n",
    "join" ["list"] ("list.join($list1, $list2, $separator)") => "Join two lists",
    "append" ["list"] ("list.append($list, $val, $separator)") => "Append a value",
    "zip" ["list"] ("list.zip($lists...)") => "Zip lists",
    "index" ["list"] ("list.index($list, $value)") => "Index of a value",
    "separator" ["list"] ("list.separator($list)") => "Separator of a list",
    "is-bracketed" ["list"] ("list.is-bracketed($list)") => "Whether a list is bracketed",
    "slash" ["list"] ("list.slash($elements...)") => "Create a slash-separated list",
    // ---- sass:map ----
    "get" ["map"] ("map.get($map, $key)") => "Value for a key",
    "merge" ["map"] ("map.merge($map1, $map2)") => "Merge maps",
    "remove" ["map"] ("map.remove($map, $keys...)") => "Remove keys",
    "keys" ["map"] ("map.keys($map)") => "All keys",
    "values" ["map"] ("map.values($map)") => "All values",
    "has-key" ["map"] ("map.has-key($map, $key)") => "Whether a key exists",
    "deep-merge" ["map"] ("map.deep-merge($map1, $map2)") => "Recursively merge maps",
    "deep-remove" ["map"] ("map.deep-remove($map, $key, $keys...)") => "Remove a nested key",
    "set" ["map"] ("map.set($map, $key, $value)") => "Set a key",
    // ---- sass:selector ----
    "parse" ["selector"] ("selector.parse($selector)") => "Parse a selector into a structure",
    "is-superselector" ["selector"] ("selector.is-superselector($super, $sub)") => "Whether one selector matches all of another",
    "extend" ["selector"] ("selector.extend($selector, $extendee, $extender)") => "Extend a selector like @extend",
    "replace" ["selector"] ("selector.replace($selector, $original, $replacement)") => "Replace a selector",
    "unify" ["selector"] ("selector.unify($selector1, $selector2)") => "Unify two selectors",
    "nest" ["selector"] ("selector.nest($parent, $selectors...)") => "Nest selectors like source nesting",
    "append" ["selector"] ("selector.append($selectors...)") => "Append to a selector without nesting",
    // ---- sass:meta ----
    "type-of" ["meta"] ("meta.type-of($value)") => "Type of a value",
    "inspect" ["meta"] ("meta.inspect($value)") => "Source-like representation",
    "variable-exists" ["meta"] ("meta.variable-exists($name)") => "Whether a variable exists",
    "global-variable-exists" ["meta"] ("meta.global-variable-exists($name)") => "Whether a global variable exists",
    "function-exists" ["meta"] ("meta.function-exists($name)") => "Whether a function exists",
    "mixin-exists" ["meta"] ("meta.mixin-exists($name)") => "Whether a mixin exists",
    "content-exists" ["meta"] ("meta.content-exists()") => "Whether a @content block was passed",
    "get-function" ["meta"] ("meta.get-function($name, $css)") => "Look up a function",
    "get-mixin" ["meta"] ("meta.get-mixin($name)") => "Look up a mixin",
    "call" ["meta"] ("meta.call($function, $args...)") => "Dynamically call a function",
    "keywords" ["meta"] ("meta.keywords($args)") => "Keyword arguments of an arglist",
    "module-variables" ["meta"] ("meta.module-variables($module)") => "All variables of a module",
    "module-functions" ["meta"] ("meta.module-functions($module)") => "All functions of a module",
    "calc-name" ["meta"] ("meta.calc-name($calc)") => "Name of a calc expression",
    "calc-args" ["meta"] ("meta.calc-args($calc)") => "Arguments of a calc expression",
};

/// Built-in module namespaces
pub static MODULES: &[&str] = &["math", "color", "string", "list", "map", "selector", "meta"];

/// At-rules with short docs
pub static AT_RULES: &[(&str, &str)] = &[
  ("@mixin", "Declare a mixin: @mixin name($args) { ... }"),
  (
    "@include",
    "Include a mixin: @include name(args) { content }",
  ),
  (
    "@function",
    "Declare a function: @function name($args) { @return v; }",
  ),
  ("@return", "Return a value from a function"),
  ("@if", "Conditional: @if cond { } @else if { } @else { }"),
  ("@else", "Else branch of @if"),
  ("@for", "Loop: @for $i from 1 through 5 { }"),
  ("@each", "Loop: @each $item in $list { }"),
  ("@while", "Loop: @while cond { }"),
  ("@extend", "Extend a selector: @extend .cls !optional"),
  ("@at-root", "Emit nested content at document root"),
  ("@use", "Load a module: @use \"vass:math\" as m"),
  ("@forward", "Re-export a module's members"),
  (
    "@import",
    "Import files or CSS: @import \"a\", \"b\" screen",
  ),
  ("@debug", "Print a value to stderr"),
  ("@warn", "Emit a warning"),
  ("@error", "Abort compilation with an error"),
  ("@content", "Inject a @content block inside a mixin"),
  ("@media", "Media query block"),
  ("@supports", "Feature query block"),
  ("@keyframes", "Keyframes animation block"),
  ("@font-face", "Font face block"),
  ("@charset", "Charset declaration"),
];

fn item(label: &str, kind: u32, detail: &str, doc: &str) -> Value {
  serde_json::json!({
      "label": label,
      "kind": kind,
      "detail": detail,
      "documentation": { "kind": "markdown", "value": doc },
  })
}

fn module_function_items(module: &str) -> Vec<Value> {
  BUILTINS
    .iter()
    .filter(|e| e.module == module)
    .map(|e| item(e.name, 3, e.signature, e.doc))
    .collect()
}

/// Compute completion items for the position.
///
/// `index` is the document's symbol index; the other args are 0-based
/// line/character of the cursor.
pub fn completions(index: &Index, source: &str, line: u32, character: u32) -> Vec<Value> {
  let line_text = source.lines().nth(line as usize).unwrap_or("");
  let before: String = line_text.chars().take(character as usize).collect();
  let trimmed_before = before.trim_start();
  // current token: text from the last whitespace / `(` boundary to the cursor
  let token_start = before
    .rfind(|c: char| c.is_whitespace() || c == '(' || c == ',')
    .map(|i| i + 1)
    .unwrap_or(0);
  let token = &before[token_start..];

  // 1. `$` variable context (typing a variable mid-word or right after `$`)
  if token.starts_with('$') {
    let mut items: Vec<Value> = index
      .symbols
      .iter()
      .filter(|s| s.kind == SymbolKind::Variable)
      .map(|s| {
        item(
          &format!("${}", s.name),
          6,
          &s.detail,
          "Variable declared in this document",
        )
      })
      .collect();
    items.push(item(
      "!default",
      14,
      "!default",
      "Assign only if the variable is not already defined",
    ));
    items.push(item("!global", 14, "!global", "Assign to the global scope"));
    items.push(item("!important", 14, "!important", "CSS importance flag"));
    return items;
  }

  // 2. at-rule context
  if trimmed_before.starts_with('@') {
    let mut items: Vec<Value> = AT_RULES
      .iter()
      .map(|(name, doc)| item(name, 14, name, doc))
      .collect();
    // after `@include ` offer mixins
    if trimmed_before.starts_with("@include") {
      return index
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Mixin)
        .map(|s| item(&s.name, 2, &s.detail, "Mixin declared in this document"))
        .collect();
    }
    // after `@use "/"@forward` offer builtin modules
    if trimmed_before.starts_with("@use") || trimmed_before.starts_with("@forward") {
      for m in MODULES {
        items.push(item(
          &format!("vass:{m}"),
          9,
          &format!("vass:{m}"),
          "Built-in module",
        ));
      }
    }
    return items;
  }

  // 3. module member context: `math.`, `color.`, ...
  for m in MODULES {
    let prefix = format!("{m}.");
    if before.trim_end().ends_with(&prefix) || trimmed_before.starts_with(&prefix) {
      return module_function_items(m);
    }
  }

  // 4. generic context: locals + globals + modules
  let mut items: Vec<Value> = Vec::new();
  for s in &index.symbols {
    let kind = match s.kind {
      SymbolKind::Variable => 6,
      SymbolKind::Mixin => 2,
      SymbolKind::Function => 3,
      _ => continue,
    };
    items.push(item(&s.name, kind, &s.detail, "Declared in this document"));
  }
  for e in BUILTINS.iter().filter(|e| e.module.is_empty()) {
    items.push(item(e.name, 3, e.signature, e.doc));
  }
  for m in MODULES {
    items.push(item(
      m,
      9,
      &format!("{m}."),
      &format!("sass:{m} built-in module"),
    ));
  }
  for (name, doc) in AT_RULES {
    items.push(item(name, 14, name, doc));
  }
  items
}

/// Hover for the word at a position; returns markdown or `None`.
pub fn hover(index: &Index, source: &str, line: u32, character: u32) -> Option<Value> {
  let (word, _start, _end) = crate::analysis::word_at(source, line, character)?;

  // local variable
  if let Some(name) = word.strip_prefix('$') {
    if let Some(s) = index
      .symbols
      .iter()
      .find(|s| s.kind == SymbolKind::Variable && s.name == name)
    {
      let md = format!(
        "```vass\n{}\n```\n*variable* declared at line {}",
        s.detail,
        s.line + 1
      );
      return Some(serde_json::json!({
          "contents": { "kind": "markdown", "value": md },
      }));
    }
    return None;
  }

  // local mixin / function
  if let Some(s) = index
    .symbols
    .iter()
    .find(|s| s.name == word && (s.kind == SymbolKind::Mixin || s.kind == SymbolKind::Function))
  {
    let what = if s.kind == SymbolKind::Mixin {
      "mixin"
    } else {
      "function"
    };
    let md = format!(
      "```vass\n{}\n```\n*{what}* declared at line {}",
      s.detail,
      s.line + 1
    );
    return Some(serde_json::json!({
        "contents": { "kind": "markdown", "value": md },
    }));
  }

  // built-in (bare or module-qualified)
  let qualified = {
    let line_text = source.lines().nth(line as usize).unwrap_or("");
    let ch = character as usize;
    let before: String = line_text.chars().take(ch).collect();
    let stripped = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '-' || c == '_');
    let stripped = stripped.trim_end_matches('.');
    let prefix = stripped
      .chars()
      .rev()
      .take_while(|c| c.is_alphanumeric())
      .collect::<String>();
    let prefix = prefix.chars().rev().collect::<String>();
    if prefix.is_empty() {
      word.clone()
    } else {
      format!("{prefix}.{word}")
    }
  };
  let (module, name) = match qualified.split_once('.') {
    Some((m, n)) => (m, n),
    None => ("", qualified.as_str()),
  };
  if let Some(e) = BUILTINS
    .iter()
    .find(|e| e.name == name && e.module == module)
  {
    let md = format!("```vass\n{}\n```\n{}", e.signature, e.doc);
    return Some(serde_json::json!({
        "contents": { "kind": "markdown", "value": md },
    }));
  }
  // at-rule hover
  if let Some((name, doc)) = AT_RULES.iter().find(|(n, _)| format!("@{word}") == *n) {
    return Some(serde_json::json!({
        "contents": { "kind": "markdown", "value": format!("**{name}**\n\n{doc}") },
    }));
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::analysis::analyze;

  const SRC: &str = "$accent: #e63946;\n@mixin card($pad) {\n  padding: $pad;\n}\n@function double($n) { @return $n * 2; }\n.a { color: $accent; }\n";

  #[test]
  fn variable_context_offers_local_vars() {
    let idx = analyze(SRC);
    // line 0, right after `$`
    let items = completions(&idx, SRC, 0, 1);
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"$accent"), "{labels:?}");
    assert!(labels.contains(&"!default"));
  }

  #[test]
  fn at_rule_context_offers_rules_and_include_mixins() {
    let idx = analyze(SRC);
    let items = completions(&idx, SRC, 0, 1); // `$` first
    assert!(!items.is_empty());
    // `@include ` offers mixin
    let src2 = "@include \n";
    let items = completions(&idx, src2, 0, 9);
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"card"), "{labels:?}");
    // bare `@`
    let items = completions(&idx, "@\n", 0, 1);
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"@mixin"));
    assert!(labels.contains(&"@media"));
  }

  #[test]
  fn module_context_offers_module_functions() {
    let idx = analyze(SRC);
    let items = completions(&idx, "a: math.\n", 0, 9);
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"floor"), "{labels:?}");
    assert!(labels.contains(&"clamp"));
    let items = completions(&idx, "a: color.\n", 0, 10);
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"adjust"));
  }

  #[test]
  fn generic_context_offers_locals_and_globals() {
    let idx = analyze(SRC);
    let items = completions(&idx, "color: \n", 0, 7);
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"card"));
    assert!(labels.contains(&"double"));
    assert!(labels.contains(&"rgb"));
    assert!(labels.contains(&"map-get"));
    assert!(labels.contains(&"math"));
  }

  #[test]
  fn hover_variable_and_mixin() {
    let idx = analyze(SRC);
    let h = hover(&idx, SRC, 5, 17).unwrap(); // `$accent` in `.a` (line 5)
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(md.contains("$accent: #e63946"), "{md}");
    let h = hover(&idx, SRC, 1, 8).unwrap(); // `card` in @mixin line
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(md.contains("@mixin card($pad)"), "{md}");
  }

  #[test]
  fn hover_builtin() {
    let idx = analyze(SRC);
    let h = hover(&idx, "a: floor(1.5);\n", 0, 8).unwrap();
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(md.contains("floor"), "{md}");
    let h = hover(&idx, "a: math.floor(1.5);\n", 0, 13).unwrap();
    let md = h["contents"]["value"].as_str().unwrap();
    assert!(md.contains("math.floor"), "{md}");
  }
}
