//! `vass explain <CODE>`: a friendly, self-contained knowledge base for
//! every diagnostic code. Each rendered error ends with a pointer to this
//! command, so a confused user always has a one-step path from "error" to
//! "what it means, why it happened, how to fix it".

use clap::Args as ClapArgs;

/// One diagnostic's documentation entry
struct Entry {
  code: &'static str,
  /// Short title shown in the list view
  title: &'static str,
  /// What the code means (one paragraph)
  meaning: &'static str,
  /// The usual reasons this error appears
  causes: &'static [&'static str],
  /// Concrete, ordered steps to resolve it
  fixes: &'static [&'static str],
  /// A small worked example
  example: &'static [&'static str],
}

use Entry as E;

/// The full diagnostic index (keep in sync with `ErrorKind::code`)
const ENTRIES: &[Entry] = &[
  E {
    code: "E0001",
    title: "Parse / syntax error",
    meaning: "The file could not be tokenized or parsed - something in \
                  the text does not form a valid VASS statement.",
    causes: &[
      "a missing `;`, `}` or `)` somewhere above the reported line",
      "an unterminated string or comment",
      "an at-rule written where it is not allowed (e.g. nested `@use`)",
    ],
    fixes: &[
      "look at the reported line AND the one above it - most syntax \
             errors are caused by the previous statement",
      "check that every `{` has a matching `}` (and every `(` a `)`)",
      "run `vass fmt <file>` - the formatter catches many of these",
    ],
    example: &[
      ".card { color: red }   // OK",
      ".card { color: red        // E0001: missing `}`",
    ],
  },
  E {
    code: "E0003",
    title: "Type / value error",
    meaning: "A value has the wrong type for the operation - typically \
                  incompatible units (`1px + 1s`) or a non-number where a \
                  number is required.",
    causes: &[
      "adding/subtracting values with incompatible units",
      "passing a color or string to a math function",
      "dividing by zero",
    ],
    fixes: &[
      "check the units on both sides of the operator",
      "use `math.compatible($a, $b)` or `math.convert` to inspect units",
      "print the value with `@debug $var;` to see what it holds",
    ],
    example: &[
      "$w: 10px + 2s;      // E0003: Incompatible units 1px + 1s.",
      "$w: 10px + 2px;     // OK -> 12px",
    ],
  },
  E {
    code: "E0004",
    title: "Undefined variable",
    meaning: "A `$variable` was read before it was assigned - or a \
                  module member (`base.$x`) does not exist in that module.",
    causes: &[
      "typo in the variable name (names are case-sensitive)",
      "the variable is defined in another file that was never `@use`d",
      "the variable is defined AFTER the line that reads it",
      "for `ns.$x`: the module exists but has no `$x` member",
    ],
    fixes: &[
      "declare it first: `$name: value;` (top of the file or before use)",
      "for another file's variables: add `@use 'that-file';` and read \
             `ns.$name`",
      "for `ns.$x`: check the module's members - the error lists every \
             variable the module defines",
    ],
    example: &[
      "$accent: #e63946;       // declare first",
      ".btn { color: $accent; }   // then use",
    ],
  },
  E {
    code: "E0005",
    title: "Undefined function",
    meaning: "A function call has no matching user `@function` and no \
                  built-in function of that name.",
    causes: &[
      "typo in the function name",
      "the function lives in a module that was never `@use`d",
      "the function is defined after the call site",
    ],
    fixes: &[
      "`@function name($args) { @return ...; }` must appear before use",
      "module functions: `@use 'vass:math';` then `math.div(10, 2)`",
      "run `vass list functions` to see every built-in",
    ],
    example: &["@use 'vass:math';", ".a { w: math.div(10, 2); }   // 5"],
  },
  E {
    code: "E0006",
    title: "Undefined mixin",
    meaning: "An `@include` named a mixin that does not exist (or is \
                  not visible at that point).",
    causes: &[
      "typo in the mixin name",
      "the mixin lives in another file - namespaces are required",
      "the mixin is defined inside another rule's body",
    ],
    fixes: &[
      "define it first: `@mixin name { ... }`",
      "for another file: `@use 'mixins';` then `@include mixins.name;`",
      "if you used `@use ... as *`, drop the namespace prefix",
    ],
    example: &[
      "@mixin center { display: flex; justify-content: center; }",
      ".box { @include center; }",
    ],
  },
  E {
    code: "E0007",
    title: "Duplicate declaration",
    meaning: "The same name was declared twice where duplicates are \
                  not allowed.",
    causes: &[
      "two `@function`/`@mixin` with the same name in one scope",
      "the same module loaded under the same namespace twice",
    ],
    fixes: &[
      "rename one of the declarations",
      "give the second `@use` an alias: `@use 'other' as other;`",
    ],
    example: &[
      "@use 'colors';",
      "@use 'theme' as theme;   // different alias",
    ],
  },
  E {
    code: "E0008",
    title: "Can't find module",
    meaning: "An `@use`/`@forward`/`@import` URL did not match any file \
                  on disk. Vass searched every candidate name in every load \
                  directory - the note line shows the full list.",
    causes: &[
      "the module file does not exist (yet)",
      "typo in the URL or the file name",
      "the file lives in another folder - URLs resolve relative to \
             the IMPORTING file, not the working directory",
      "the file is named `base.vcss` while a `_base.vcss` partial \
             also exists (ambiguous)",
    ],
    fixes: &[
      "create the module file (e.g. `_base.vcss`) next to the \
             importing file - the error's help line names the exact file",
      "fix the URL; `@use 'base'` finds `base.vcss`, `_base.vcss`, \
             `base/index.vcss` or `base/_index.vcss`",
      "for files elsewhere use a relative URL (`@use '../base';`) or \
             add a search dir with `--load-path <dir>`",
    ],
    example: &[
      "main.vcss:",
      "  @use 'base';                    <- URL",
      "_base.vcss (same folder):",
      "  $primary-color: #333;           <- module file",
    ],
  },
  E {
    code: "E0009",
    title: "I/O error",
    meaning: "A file could not be read - the path exists check passed \
                  but the OS refused the read, or the file disappeared \
                  mid-compile.",
    causes: &[
      "missing read permissions",
      "the file was deleted/renamed while compiling (watch mode)",
      "the file is a directory, not a file",
    ],
    fixes: &[
      "check the path in the message exists and is readable",
      "in watch mode just save the file again",
    ],
    example: &["vass compile src/main.vcss"],
  },
  E {
    code: "E0010",
    title: "Configuration error",
    meaning: "A command-line option or project configuration value is \
                  invalid (bad `--style`, unreadable config file, ...).",
    causes: &[
      "an option value outside the allowed set",
      "a malformed config file",
    ],
    fixes: &[
      "read the option's accepted values in `vass <command> --help`",
      "run `vass doctor` to diagnose the installation",
    ],
    example: &["vass compile in.vcss --style expanded"],
  },
  E {
    code: "E0011",
    title: "Runtime error (no such module namespace)",
    meaning: "A `ns.$var` or `ns.fn()` access named a namespace that \
                  was never registered - usually because no `@use` rule \
                  loads that module, or the namespace spelling differs.",
    causes: &[
      "no `@use` rule for the module in this file",
      "namespace spelled with different case: `@use 'base' as Base` \
             registers `Base`, and namespaces are case-sensitive",
      "the module was loaded with `as *`, which registers NO \
             namespace (members are used bare)",
      "an `@error \"...\"` directive fired (same code, message = text)",
    ],
    fixes: &[
      "add `@use 'base';` at the TOP of the file (the editor's \
             quick-fix offers this - look for the lightbulb)",
      "match the case: if `@use 'base' as Base;` was written, use \
             `Base.$x`",
      "with `as *` use `$x` without the `base.` prefix",
    ],
    example: &[
      "@use 'base';                      <- add this first",
      ".inverse {",
      "  background-color: base.$primary-color;",
      "}",
    ],
  },
  E {
    code: "E0012",
    title: "Internal error",
    meaning: "An unexpected state inside Vass itself - this is a bug, \
                  not a mistake in your code.",
    causes: &["a Vass compiler bug"],
    fixes: &[
      "note the exact input that triggered it and file an issue with \
             the full error block",
      "try `vass doctor` to rule out a broken installation",
    ],
    example: &["Please report: https://github.com/ - include the full output"],
  },
  E {
    code: "E0014",
    title: "Unbalanced bracket",
    meaning: "A `{`, `(` or `[` was opened but never closed, or a `}`, \
                  `)` or `]` appeared with nothing to match it. Reported \
                  before parsing starts, so every unbalanced bracket in the \
                  file is listed at once.",
    causes: &[
      "a `}` deleted by mistake (or an extra one pasted in)",
      "a multi-line declaration where the closing `)` landed on a \
             different line than expected",
      "brackets inside an unterminated string, which the string scan \
             then reads as code",
    ],
    fixes: &[
      "count the brackets on the reported line and the one above",
      "run `vass fmt <file>` - the formatter indents every block, so \
             the missing `}` becomes obvious",
      "editor users: the matching-bracket highlight finds the partner",
    ],
    example: &[
      ".b { width: (10px; }    // E0014: Unclosed `(`",
      ".b { width: (10px); }   // OK",
    ],
  },
];

/// Explain a diagnostic code (or list every code)
#[derive(ClapArgs, Debug)]
pub struct Args {
  /// Diagnostic code to explain, e.g. `E0008` (or `vass explain` to list all)
  #[arg(value_name = "CODE")]
  pub code: Option<String>,
}

/// Run the explain command
pub async fn run(args: Args) -> anyhow::Result<()> {
  match &args.code {
    None => list_all(),
    Some(raw) => explain(raw.trim()),
  }
}

/// `vass explain` - one-line index of every code
fn list_all() -> anyhow::Result<()> {
  println!("Vass diagnostic codes - run `vass explain <CODE>` for details:\n");
  for e in ENTRIES {
    println!("  {:<6} {}", e.code, e.title);
  }
  println!();
  Ok(())
}

/// `vass explain E0008` - the full entry
fn explain(raw: &str) -> anyhow::Result<()> {
  // Accept `E0008`, `e0008`, `0008` and the short `8` / `11` forms by
  // comparing the digit part, zero-padded to the canonical width.
  let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
  let padded = format!("{:0>width$}", digits, width = 4);
  let entry = ENTRIES
    .iter()
    .find(|e| e.code.trim_start_matches('E') == padded)
    .ok_or_else(|| {
      let list = ENTRIES
        .iter()
        .map(|e| e.code)
        .collect::<Vec<_>>()
        .join(", ");
      anyhow::anyhow!(
        "Unknown diagnostic code `{raw}`. Valid codes: {list}. \
                 Run `vass explain` to list them."
      )
    })?;

  let bar = "=".repeat(66);
  println!("{bar}");
  println!(" {} - {}", entry.code, entry.title);
  println!("{bar}\n");
  println!("{}", entry.meaning);

  println!("\nCommon causes:");
  for c in entry.causes {
    println!("  - {c}");
  }

  println!("\nHow to fix:");
  for (i, f) in entry.fixes.iter().enumerate() {
    println!("  {}. {f}", i + 1);
  }

  if !entry.example.is_empty() {
    println!("\nExample:");
    for l in entry.example {
      println!("  {l}");
    }
  }
  println!();
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_code_is_documented() {
    // Keep the index complete: E0001..E0012 minus the retired E0002 gap
    let codes: Vec<&str> = ENTRIES.iter().map(|e| e.code).collect();
    for c in ["E0001", "E0003", "E0004", "E0008", "E0011", "E0012"] {
      assert!(codes.contains(&c), "missing explain entry for {c}");
    }
  }

  #[test]
  fn explain_matches_case_insensitively() {
    assert!(explain("e0008").is_ok());
    assert!(explain("E0008").is_ok());
    assert!(explain("0008").is_ok());
    assert!(explain("banana").is_err());
  }
}
