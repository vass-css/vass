# VASS — CSS Preprocessor (Cross-Platform Release v0.2)

This document describes the archived v0.2 package. Current release workflow details for v0.3.0 and later are below.

Prebuilt release binaries of the VASS CSS preprocessor for **5 platform targets**, all built from the same source (Rust 1.99.1, release profile, LTO-optimized).

## Contents

```
vass-release/
├── vass-linux-x86_64        # Linux Intel/AMD64 — static musl, runs anywhere
├── vass-linux-aarch64       # Linux ARM64 (Raspberry Pi 4/5, AWS Graviton) — static musl
├── vass-macos-x86_64        # macOS 11+ Intel
├── vass-macos-aarch64       # macOS 11+ Apple Silicon (M1/M2/M3/M4)
├── vass-windows-x86_64.exe  # Windows 10/11 x64 (console)
├── examples/
│   ├── demo.vass            # Indented syntax feature demo
│   └── demo.vcss            # Brace syntax feature demo
└── README.md                # This file
```

## Install

```bash
# Linux / macOS — pick your platform binary
chmod +x vass-linux-x86_64
sudo mv vass-linux-x86_64 /usr/local/bin/vass

# Windows — rename and add to PATH
ren vass-windows-x86_64.exe vass.exe
```

Not sure which one? Run `uname -m` (Linux/macOS): `x86_64` = Intel, `aarch64`/`arm64` = ARM.

## Quick Start

```bash
# Compile to a file
vass run examples/input.vass -o output.css

# Compile to stdout
vass run examples/test.vcss

# Minified output
vass run examples/input.vass --minify

# Watch mode — recompiles automatically on every save
vass watch examples/input.vass                 # writes input.css next to it
vass watch examples/input.vass -o dist/app.css # custom output (dirs auto-created)

# All options
vass run --help
vass watch --help
```

## Two Syntaxes

| Extension | Style                            | Notes            |
| --------- | -------------------------------- | ---------------- |
| `.vass`   | Indented (no braces/semicolons)  | HAML/Pug-like    |
| `.vcss`   | VCSS-style (braces + semicolons) | Drop-in familiar |

Syntax is auto-detected from the file extension.

## Supported Features (v0.2 — full language)

- **Variables** with scoping, `!default`, `!global`
- **Nesting** with `&` (hover, suffix, cartesian products, child combinators)
- **Mixins** — params, defaults, named args, rest args (`$args...`), `@content` with `using`
- **Functions** — params, defaults, rest args, recursion (`fib`, etc.)
- **Control flow** — `@if/@else if/@else`, `@each` (lists + map destructuring), `@for` (`to`/`through`, reverse), `@while`
- **Interpolation** `#{...}` — values, selectors, properties, media params, custom props
- **Maps & lists** — literals, `map-get/merge/keys/values/has-key/remove/deep-merge`, `nth/join/append/zip/index`
- **@extend** — transitive, compound-aware; `%placeholder` selectors
- **Color engine** — hex/rgba()/hsl(), `lighten/darken/saturate/grayscale/mix/invert/adjust-hue/adjust-color/scale-color/change-color`, named colors
- **Built-in modules** — `vass:math` (`div/sqrt/pow/trig/clamp/...`), `vass:string`, `vass:list`, `vass:map`, `vass:color`, `vass:selector`, `vass:meta`
- **Module system** — `@use "file" as ns with (...)`, `@use ... as *`, `@forward ... as prefix-* show/hide`, `@import` (inline + plain-CSS passthrough), partials (`_name`), index files
- **At-rules** — `@media` (bubbling + `and` merging), `@keyframes`, `@supports`, `@font-face`, `@charset`, `@at-root`
- **Custom properties** — `--x: raw value` (interpolation evaluated, never parsed)
- **Nested properties** — `font: 12px { family: serif; }`
- **Operators** — full arithmetic with unit conversion (`96px + 1in`), comparisons, `and/or/not`, `if()` (lazy)
- **Slash semantics** — `$a/2` divides, `12px/30px` stays a CSS separator (use `math.div`)
- **Output** — expanded + compressed (`.5` numbers, named colors, hex preserved), comments preserved
- **Error handling** — `@debug`/`@warn` to stderr, `@error` fatal, `file:line:col` diagnostics

## Quick feature tour

```bash
./vass-linux-x86_64 run examples/demo.vass
./vass-linux-x86_64 run examples/demo.vass --minify
./vass-linux-x86_64 watch examples/demo.vass -o demo.css
```

## Windows Defender / SmartScreen notice

When you first run `vass.exe`, Windows may show a **"Security scan required"**
notification from Virus & threat protection, or a SmartScreen popup saying
**"Windows protected your PC"**. This is expected for any unsigned command-line
tool and does **not** mean VASS contains malware — Windows simply scans every
newly downloaded executable once (the scan takes a few seconds) and SmartScreen
flags binaries without an Authenticode signature.

If SmartScreen blocks the download or first run:

1. Click **More info** → **Run anyway** (SmartScreen popup), and/or
2. Whitelist the folder once in PowerShell (as Administrator):

```powershell
Add-MpPreference -ExclusionPath "C:\path\to\vass"
```

3. Or verify the binary yourself before running: it is a plain Rust build with
   symbols stripped and no installer, no network access, no registry writes.
   Full source is in `vass-project.zip` — you can rebuild `vass.exe` locally
   with `cargo build --release` (see _Build From Source_ below) to be 100% sure.

This release is built with a hardened profile (full LTO, `panic = "abort"`,
symbols stripped, size-optimized) which also keeps the exe small (~1.4 MB) and
reduces antivirus false-positive risk.

## Build From Source

See `vass-project.zip` (full Rust workspace). To reproduce these cross-builds:

```bash
cargo install cargo-zigbuild
rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl \
                 x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu
cargo zigbuild --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-unknown-linux-musl
cargo zigbuild --release --target x86_64-apple-darwin
cargo zigbuild --release --target aarch64-apple-darwin
cargo zigbuild --release --target x86_64-pc-windows-gnu
```

## License

MIT OR Apache-2.0

## npm Distribution (v0.3.0+)

The npm package source is in `packaging/npm`. It is named `vass-css`, declares Node.js 18 or newer, and stages the repository's combined `LICENSE` file into the package during `prepack`. The release workflow checks that the npm package version, Cargo workspace version, and version tag agree.

At installation, the package selects only one of these release binaries:

| npm platform | Rust target                | Release asset                     |
| ------------ | -------------------------- | --------------------------------- |
| Linux x64    | `x86_64-unknown-linux-gnu` | `vass-x86_64-unknown-linux-gnu`   |
| macOS x64    | `x86_64-apple-darwin`      | `vass-x86_64-apple-darwin`        |
| macOS arm64  | `aarch64-apple-darwin`     | `vass-aarch64-apple-darwin`       |
| Windows x64  | `x86_64-pc-windows-msvc`   | `vass-x86_64-pc-windows-msvc.exe` |

The installer downloads only the matching raw binary and `SHA256SUMS.txt` from the fixed GitHub release URL, follows HTTPS redirects only to its allowlisted GitHub asset hosts, and verifies the digest before installing the executable. It does not execute the binary during installation. Unsupported platforms, download failures, malformed manifests, and checksum mismatches stop installation with an error.

The current release workflow continues to use version tags for its GitHub Actions references. These tags are mutable; immutable action commit SHAs have not been added because their identities were not verified for this change. Review and pin verified SHAs before treating action pinning as complete.

Suggested local checks when Node.js is available:

```sh
node packaging/npm/scripts/check-version.js
node --check packaging/npm/bin/vass.js
node --check packaging/npm/scripts/postinstall.js
cd packaging/npm && npm pack --dry-run
```

The npm package name's registry ownership and publication status are not verified by this repository. Do not infer either from the package metadata or release workflow.
