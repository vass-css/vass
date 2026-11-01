# Migration from VASS

VASS is designed to be compatible with most VASS/VCSS projects. This guide helps you migrate your existing VASS codebase to VASS.

## Key Differences

### File Extensions

VASS uses different file extensions:

| VASS | VASS |
|------|------|
| `.vass` | `.vass` |
| `.vcss` | `.vcss` |

### Supported Features

Most VASS features are supported:

- ✅ Variables (`$name: value`)
- ✅ Nesting
- ✅ Mixins (`@mixin`, `@include`)
- ✅ Functions (`@function`, `@return`)
- ✅ Control Flow (`@if`, `@for`, `@each`, `@while`)
- ✅ Modules (`@use`, `@forward`)
- ✅ Extend (`@extend`)
- ✅ Built-in modules (vass:color, vass:list, etc.)

### Minor Differences

#### Import Syntax

```vass
// VASS
@import 'variables';

// VASS (use modern syntax)
@use 'variables';
```

#### Division

```vass
// VASS (deprecated)
$width: 10px / 2;

// VASS (use math.div)
@use 'vass:math';
$width: math.div(10px, 2);
```

## Migration Steps

1. **Rename files**: Change `.vass` to `.vass` and `.vcss` to `.vcss`

2. **Update imports**: Replace `@import` with `@use`:

```vass
// Before
@import 'variables';
@import 'mixins';

// After
@use 'variables';
@use 'mixins';
```

3. **Update division**: Use `vass:math` for division:

```vass
// Before
$width: 100px / 3;

// After
@use 'vass:math';
$width: math.div(100px, 3);
```

4. **Update namespaced references**:

```vass
// Before
$primary: $primary-color;

// After
@use 'variables' as *;  // or keep namespace
$primary: variables.$primary-color;
```

## Compatibility Mode

VASS provides a compatibility mode for easier migration:

```bash
vass compile --compat vass input.vass output.css
```

This mode:
- Accepts `@import` (deprecated but supported)
- Provides warnings for deprecated features
- Suggests modern alternatives

## Testing Your Migration

1. Keep your original VASS files as backup
2. Compile both versions and compare outputs
3. Run visual regression tests if needed
4. Use `vass lint` to catch issues

## Getting Help

- Check the [documentation](SUMMARY.md)
- Open an issue on [GitHub](https://github.com/vass-css/vass/issues)
- Join our [discussions](https://github.com/vass-css/vass/discussions)
