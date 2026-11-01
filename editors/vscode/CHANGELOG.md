# Changelog

## 0.2.1

### Added

- File icons for `.vass` (indentation staircase tile) and `.vcss`
  (curly-braces tile) contributed on the language definitions, so they
  show up under any icon theme — Explorer, tabs, breadcrumbs, quick open.
- Deeply extended TextMate grammars for both syntaxes:
  - exact VASS builtin module surfaces (`math` `color` `string` `list`
    `map` `selector` `meta`) with `ns.member()` calls and `ns.$var`
    references;
  - parsed at-rule preludes (`@use/@forward` with `as|with|show|hide`,
    `@mixin/@function/@include` parameters with defaults and `...` rest,
    `@each`/`@for`/`@while`, `@extend !optional`, `@at-root`,
    `@content(args)`, `@debug/@warn/@error`);
  - CSS at-rules with structured preludes: `@media` (features, types,
    logical keywords, range syntax), `@supports` (nested conditions and
    declarations), `@keyframes` (`from`/`to`/`%`), `@font-face`,
    `@property`, `@layer`, `@container`, `@scope`, `@page`, `@charset`,
    `@namespace`, `@apply`;
  - ~380 CSS properties with vendor prefixes, custom properties and Sass
    nested-property blocks;
  - full unit set, 148 named colors, 3/4/6/8/12-digit hex, ~300 value
    keywords, recursive `#{…}` interpolation (also inside strings and
    loud comments), map literals, Sass operators, `true`/`false`/`null`;
  - full selector coverage: tags, classes, ids, pseudos, attribute
    selectors, `&` parent (incl. suffix), `%placeholders`, combinators,
    keyframe selectors.
- Language-aware snippets: indented bodies for `.vass`, braced bodies
  for `.vcss` (36 and 41 snippets).
- Per-language editor behavior: `.vcss` gets `{}` auto-close + brace
  indentation; `.vass` intentionally doesn't (braces are invalid in the
  indented syntax); both get `#{}` interpolation auto-close and a
  `$var`-aware word pattern.

### Fixed

- Publisher/identity mismatch between `package.json` (`vass-lang`) and
  the packaged VSIX manifest (`vasslang`) — everything is now
  `vasslang.vass-vscode`, so 0.2.1 upgrades 0.2.0 in place.
- Removed stale `src/extension.js` (referenced the `vscode-languageclient`
  npm package, contradicting the zero-dependency client actually shipped
  in `extension.js`) and the duplicated `dist/` bundle.
- Extension `license` field now matches the bundled Apache-2.0 LICENSE.

## 0.2.0

- Initial release: language definitions, grammars, shared snippets,
  zero-dependency LSP client (diagnostics, completion, hover,
  definitions, symbols, formatting), commands and settings.
