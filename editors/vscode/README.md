# VASS Language Support for Visual Studio Code

Syntax highlighting, **file icons**, IntelliSense, live diagnostics, hover
docs, go-to-definition, outline symbols and document formatting for the
**VASS CSS preprocessor** (`.vass` indented syntax and `.vcss` braced
syntax), powered by the VASS Language Server (`vass lsp`).

## What's new in 0.2.1

- **File icons** — `.vass` and `.vcss` files get Sass-style pink icons in
  the Explorer, tabs and search results (staircase = indented syntax,
  curly braces = braced syntax). Works with any icon theme because the
  icons ride on the language definition.
- **Deepest-yet syntax highlighting** — both grammars were rewritten from
  ~180 lines to full-coverage TextMate grammars (see below).
- **Language-aware snippets** — `.vass` snippets now use the indented
  style (no braces / semicolons); `.vcss` snippets use braces.
- **Per-language editor behavior** — `.vcss` auto-closes `{}`, improves
  indentation after `{`; `.vass` doesn't (braces are invalid there), and
  double-clicking `$primary-color` selects the whole variable in both.

## File icons

| Extension | Icon | Meaning |
| --- | --- | --- |
| `.vass` | pink tile + indentation staircase | indented (Sass-like) syntax |
| `.vcss` | pink tile + `{ }` braces | braced (SCSS-like) syntax |

The icons appear automatically in file trees, editor tabs, breadcrumbs
and quick-open — no icon-theme switching needed.

## Syntax highlighting depth

Both grammars are driven by the **actual VASS builtin surfaces**
(`crates/vass-evaluator/src/builtins/`), so module members you use are
highlighted accurately:

- **Variables** — `$name`, declarations with `!default` / `!global`,
  and module variables like `math.$pi`, `vass:math.$e`.
- **Modules** — `math` `color` `string` `list` `map` `selector` `meta`
  namespaces, their exact function sets (`math.div`, `color.adjust`,
  `map.deep-get`, `meta.get-mixin`, …) and plain member references.
- **All at-rules with parsed preludes** — `@use` / `@forward` (`as`,
  `with`, `show`, `hide`), `@mixin` / `@function` (parameters, defaults,
  `...` rest args), `@include` (+ content blocks), `@return`, `@if` /
  `@else if` / `@else`, `@each` (incl. `in` + destructuring), `@for`
  (`from` / `through` / `to`), `@while`, `@extend` (+ `!optional`),
  `@at-root` (`with:` / `without:`), `@content(args)`, `@debug` /
  `@warn` / `@error`, plus CSS at-rules: `@media` (features, types,
  logical operators, range syntax), `@supports` (nested conditions with
  declarations), `@keyframes` (`from` / `to` / `%` selectors), `@font-face`,
  `@property`, `@layer`, `@container`, `@scope`, `@page`, `@charset`,
  `@namespace`, `@apply` and a generic fallback.
- **Properties** — ~380 known CSS properties (logical, grid, container,
  scroll-timeline, modern color…) with vendor-prefix detection
  (`-webkit-…`), custom properties `--x` (declarations **and** `var()`
  references), and Sass *nested properties* (`font: { family: … }`).
- **Values** — ~300 value keywords, numbers with the full unit set
  (`px em rem % vw vh svh lvh dvh cqw cqh deg turn s ms fr dpi …`),
  hex colors (3/4/6/8/12 digits), all 148 CSS named colors, color
  functions (`rgb hsl hwb lab lch oklab oklch color-mix light-dark`),
  CSS functions (`calc var clamp env url image-set gradients, transforms…`),
  `true` / `false` / `null`, strings with escapes, `#{…}` interpolation
  (recursive, also inside strings and loud comments), map/list literals,
  and Sass operators (`== != < > and or not + - * / %`, `...` splat).
- **Selectors** — tags (HTML + SVG), classes, ids, pseudo-classes and
  pseudo-elements, attribute selectors with operators, parent selector
  `&` (incl. `&-suffix`), `%placeholder`s, combinators and keyframe
  selectors.
- **Comments** — `//` silent and `/* */` loud comments.

## Prerequisites

1. The **VASS compiler** must be installed and runnable (`vass --version`
   prints `vass <version>`).
2. The language server is started as `vass lsp --stdio` by default.

## Install

### Option A — from the VSIX (recommended)

```powershell
code --install-extension vass-vscode-0.2.1.vsix
```

(Or open the Extensions view `Ctrl+Shift+X`, choose
`…` > `Install from VSIX…`, and pick the file.)

Upgrades cleanly over 0.2.0 — same extension identity
(`vasslang.vass-vscode`).

### Option B — from the source folder

Copy this folder into your VS Code extensions directory and restart VS Code:

```powershell
Copy-Item -Recurse editors\vscode "$env:USERPROFILE\.vscode\extensions\vasslang.vass-vscode-1.0.0"
```

No `npm install` is needed — the extension has **zero dependencies**.

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `vass.server.path` | `vass` | Path to the VASS executable. Set a full path like `D:\Tools\vass\vass.exe` if `vass` is not on your `PATH`. |
| `vass.server.args` | `["lsp", "--stdio"]` | Server launch arguments. |
| `vass.trace.server` | `off` | `messages` / `verbose` to trace server traffic in the output channel. |

## What you get

- **File icons** for `.vass` / `.vcss` in Explorer, tabs and search.
- **Diagnostics** — parse/eval errors and warnings appear in the Problems
  panel as you type (with file/line/column).
- **Completion** — variables, mixins, functions, module members
  (`math.`, `color.`, `string.`, `list.`, `map.`, `selector.`, `meta.`),
  CSS properties; triggers `$`, `.`, `@`, `-`.
- **Hover** — documentation for variables, mixins, functions.
- **Go to definition** — F12 on variables/mixins/functions.
- **Outline** — the document symbols view shows rules, mixins, variables.
- **Formatting** — Shift+Alt+F formats the document (braced or indented
  style is chosen by the file extension).
- **Snippets** — `mixin`, `include`, `fun`, `if`, `each`, `for`, `vard`,
  `use`, `forward`, `kf`, `ff`, `flexc` and more — with indented bodies
  in `.vass` and braced bodies in `.vcss`.

## Troubleshooting

- *“Could not start the VASS language server”* — run `vass --version` in a
  terminal. If that fails, install VASS or set `vass.server.path`.
- Nothing happens after changing settings — run the command
  **VASS: Restart Language Server**.
- Server logs live in the **VASS Language Server** output channel
  (View > Output).
- Icons not visible after updating — run **Developer: Reload Window**
  once so VS Code re-registers the languages.

## Previous versions note

Versions of this extension (or of any editor integration) that launched
`vass lsp --stdio` against an older VASS build failed with:

```
error: unexpected argument '--stdio' found
```

VASS `0.2.x` accepts `--stdio` (it is equivalent to `--transport stdio`),
so both launch styles work. Update the compiler and restart the server.
