# Getting Started with VASS

Welcome to VASS, a modern CSS preprocessor built in Rust. This guide will help you get up and running quickly.

## Installation

### Using Cargo

The easiest way to install VASS is via Cargo:

```bash
cargo install vass
```

### Using npm

The `vass-css` package is not currently published to the npm registry. Until it is published, install VASS with Cargo as shown above or download a prebuilt binary from [GitHub Releases](https://github.com/vass-css/vass/releases).

### From Source

To build from source:

```bash
git clone https://github.com/vass-css/vass.git
cd vass
cargo install --path bins/vass
```

## Your First VASS File

Create a file named `main.vass`:

```vass
// Variables
$primary-color: #3498db
$spacing: 1rem

// Styles
.container
  max-width: 1200px
  margin: 0 auto
  padding: $spacing

.button
  background: $primary-color
  color: white
  padding: $spacing * 0.5 $spacing
  border-radius: 4px

  &:hover
    background: darken($primary-color, 10%)
```

## Compiling

To compile your VASS file:

```bash
vass compile main.vass output.css
```

This will produce:

```css
.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 1rem;
}

.button {
  background: #3498db;
  color: white;
  padding: 0.5rem 1rem;
  border-radius: 4px;
}
.button:hover {
  background: #2980b9;
}
```

## Watch Mode

For development, use watch mode to automatically recompile on changes:

```bash
vass watch src/ dist/
```

## Next Steps

- Read the [Syntax Overview](syntax/overview.md) to learn more about the VASS language
- Check out [Built-in Modules](builtins/color.md) for available functions
- Learn about [Modules](syntax/modules.md) for organizing your code
