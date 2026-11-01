# VASS - A Modern CSS Preprocessor

<div align="center">

**A fast, feature-rich CSS preprocessor written in Rust**

[Getting Started](#getting-started) • [Features](#features) • [Documentation](#documentation) • [Contributing](#contributing)

</div>

---

## Overview

VASS is a modern CSS preprocessor that compiles `.vass` (indented syntax) and `.vcss` (VCSS-style syntax) files into optimized CSS. Built from the ground up in Rust, VASS provides exceptional performance while maintaining compatibility with the VASS ecosystem.

### Why VASS?

- **Blazingly Fast**: Built in Rust with parallel processing and incremental compilation
- **VASS Compatible**: Drop-in replacement for most VASS/VCSS projects
- **Modern Architecture**: Clean separation of concerns with a modular crate structure
- **Extensible**: Plugin system for custom functions and importers
- **Developer Friendly**: Built-in LSP support, formatter, and linter

## File Extensions

| Extension | Description                       |
| --------- | --------------------------------- |
| `.vass`   | Indented syntax (similar to SASS) |
| `.vcss`   | VCSS-style bracket syntax         |
| `.css`    | Output CSS files                  |

## Features

### Core Features

- **Variables**: Store and reuse values throughout your stylesheets
- **Nesting**: Write cleaner, more organized CSS with nested rules
- **Mixins**: Create reusable chunks of CSS with parameters
- **Functions**: Define custom functions for complex calculations
- **Control Flow**: `@if`, `@for`, `@each`, `@while` for dynamic styles
- **Modules**: `@use`, `@forward`, and `@import` for code organization
- **Extend**: Share styles between selectors with `@extend`

### Advanced Features

- **Modern Color Spaces**: Full support for `oklab`, `oklch`, `display-p3`, and more
- **Calculations**: Native CSS `calc()` with compile-time optimization
- **Source Maps**: Accurate source maps for debugging
- **Embedded Protocol**: Host-compiler communication for IDE integration

## Getting Started

### Installation

```bash
# Using cargo
cargo install vass

# Using npm
# The npm package is not currently published, so npm installation is unavailable.
```

The npm distribution is intended to use the `vass-css` package name; registry ownership has not been verified. Its installer supports Linux x64 (GNU), macOS x64/arm64, and Windows x64 (MSVC), and requires Node.js 18 or newer. Linux ARM and Windows ARM are not supported by this npm distribution.

### Quick Start

Create your first `.vass` file:

```vass
// variables.vass
$primary-color: #3498db
$spacing: 1rem

// styles.vass
@use 'variables'

.button
  background: variables.$primary-color
  padding: $spacing

  &:hover
    background: darken(variables.$primary-color, 10%)

  &--large
    padding: $spacing * 1.5
```

Compile to CSS:

```bash
vass compile styles.vass output.css
```

Output:

```css
.button {
  background: #3498db;
  padding: 1rem;
}
.button:hover {
  background: #2980b9;
}
.button--large {
  padding: 1.5rem;
}
```

### CLI Commands

```bash
# Compile a single file (alias: `vass run`)
vass compile input.vass output.css
vass compile --style compressed input.vcss

# Build every stylesheet in a directory (partials `_*.vass` are skipped)
vass build src/ --out-dir dist/

# Check files for errors without writing output (CI friendly)
vass check src/*.vass
vass check --parse-only styles.vcss

# Format source files (rustfmt-style modes)
vass fmt styles.vass                # print to stdout
vass fmt --write styles.vass        # rewrite in place
vass fmt --check src/               # exit 1 if not formatted

# Evaluate inline code (quick experiments, scripting)
vass exec '@use "vass:math"; .a { w: math.div(120, 3) * 1px; }'
printf '@use "vass:math"\na\n  w: math.div(120, 3) * 1px\n' | vass exec --stdin --indented

# Introspect the built-in library
vass list modules                   # math, color, string, list, map, selector, meta
vass list functions math            # functions of one module (or all)
vass list variables math            # exported constants ($pi, $e, ...)
vass list globals                   # global functions

# Print compile statistics
vass stats styles.vass

# Watch for changes
vass watch src/ dist/

# Start the Language Server (stdio JSON-RPC) for editors/IDEs
vass lsp

# Start a REPL
vass repl

# Initialize a new project
vass init my-project

# Manage plugins
vass plugin list
vass plugin install <name>

# Diagnose the installation
vass doctor

# Generate shell completions
vass completions bash > /etc/bash_completion.d/vass   # also zsh, fish, powershell, elvish

# Detailed version info
vass version
```

## Documentation

- [Getting Started](docs/src/getting-started.md)
- [Language Reference](docs/src/reference.md)
- [Migration from VASS](docs/src/migration-from-vass.md)
- [Plugin Development](docs/src/plugins.md)
- [API Documentation](https://docs.rs/vass)

## Development Phases

VASS is developed in 40 incremental phases, each building on the previous:

| Phase | Description                                   |
| ----- | --------------------------------------------- |
| 1     | Project Scaffold & Foundation                 |
| 2-19  | Core Crate (AST, Values, Errors, Tokens)      |
| 20-23 | Parser (Lexer & Parser)                       |
| 24-28 | Evaluator (Expression evaluation & Functions) |
| 29    | Transpiler (CSS output)                       |
| 30-40 | IO, Embedded, Plugins, LSP, WASM, CLI, Tests  |

## Development

### Prerequisites

- Rust 1.75 or later
- cargo

### Building

```bash
# Clone the repository
git clone https://github.com/vass-css/vass.git
cd vass

# Build all crates
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy --all-targets --all-features

# Format code
cargo fmt
```

### Running Tests

```bash
# Run all tests
cargo test --all

# Run tests for a specific crate
cargo test -p vass-core

# Run integration tests
cargo test --test integration

# Run VASS spec tests
cargo test --test vass_spec
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Ways to Contribute

- Report bugs and request features via Issues
- Submit Pull Requests for bug fixes and features
- Improve documentation
- Add test cases

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Acknowledgments

SASS is inspired by and pays tribute to:

- [Dart SASS](https://github.com/sass/dart-sass) - The reference implementation
- [SASS](https://sass-lang.com/) - The original CSS preprocessor
- The Rust community for excellent tooling and libraries

---

<div align="center">

**[Report Bug](https://github.com/vass-css/vass/issues) • [Request Feature](https://github.com/vass-css/vass/issues) • [Ask Question](https://github.com/vass-css/vass/discussions)**

</div>
