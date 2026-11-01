# vass

VASS — A modern, fast CSS preprocessor CLI written in Rust. Compiles `.vass` (indented syntax) and `.vcss` (bracket syntax) files into optimized, standards-compliant CSS.

[![GitHub release](https://img.shields.io/github/v/release/jubayerhossaindev/vass)](https://github.com/jubayerhossaindev/vass/releases)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Node.js](https://img.shields.io/badge/node-%3E%3D18-brightgreen.svg)](https://nodejs.org/)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://rust-lang.org/)
[![Build Status](https://img.shields.io/github/actions/workflow/status/jubayerhossaindev/vass/ci.yml/badge.svg)](https://github.com/jubayerhossaindev/vass/actions)

## Installation

```bash
npm install --save-dev vass
```

## Usage

Compile a `.vass` or `.vcss` file:

```bash
npx vass compile style.vass style.css
npx vass compile style.vcss style.css
```

Or add to your `package.json` scripts:

```json
{
  "scripts": {
    "build:css": "vass compile style.vass style.css"
  }
}
```

## Supported Platforms

The npm package downloads a prebuilt binary for your platform:

| Platform | Architecture |
|----------|-------------|
| Linux | x64 (GNU) |
| macOS | x64, arm64 |
| Windows | x64 (MSVC) |

Linux ARM and Windows ARM are not currently supported.

## Security

The postinstall script downloads only the matching executable and `SHA256SUMS.txt` from the official [vass-css/vass](https://github.com/vass-css/vass) GitHub releases. It verifies the SHA-256 digest before installing the binary. The downloaded binary is never executed during installation.

## License

Licensed under the [Apache License 2.0](LICENSE).

## Support

- [File an Issue](https://github.com/vass-css/vass/issues)
- [Request a Feature](https://github.com/vass-css/vass/issues)
- [Ask a Question](https://github.com/vass-css/vass/discussions)
- [Documentation](https://github.com/vass-css/vass#readme)
