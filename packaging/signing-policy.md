# Vass — Code Signing Policy

This document describes the optional Windows code-signing path for Vass
release artifacts. SignPath Foundation may provide a signing certificate
for this path; signing is not enabled or guaranteed for every release.

## What is signed

| Artifact                                           | Signed                                   | Notes                                                                  |
| -------------------------------------------------- | ---------------------------------------- | ---------------------------------------------------------------------- |
| `vass-<version>-x86_64-pc-windows-msvc.zip`        | no                                       | Default Windows archive; unsigned                                      |
| `vass-<version>-x86_64-pc-windows-msvc-signed.zip` | yes (Authenticode), if signing completes | Separate archive produced by the optional SignPath workflow            |
| Linux / macOS archives                             | no                                       | Platform-native verification (Sigstore attestation + SHA256) applies   |
| `SHA256SUMS.txt`                                   | n/a                                      | Lists SHA-256 checksums for release archives and binaries              |
| `SHA256SUMS.txt.sha256`                            | n/a                                      | SHA-256 checksum for the combined checksum manifest                    |
| Build provenance attestations                      | yes (Sigstore)                           | Generated for archives, binaries, and both checksum files listed above |

## Who signs

Signing is an opt-in path in the GitHub Actions release pipeline
(`.github/workflows/release.yml`). It runs only when the repository
variable `SIGNPATH_ENABLED` is set to `true` and the required SignPath
configuration is present. Configure the repository secret
`SIGNPATH_ORGANIZATION_ID`, the repository secret `SIGNPATH_API_TOKEN`,
and the repository variable `SIGNPATH_ENABLED=true`. Signing runs only
for validated version-tag refs. The workflow submits the Windows
executable for signing; the default Windows archive remains unsigned.
The private key is managed by SignPath and does not leave its Hardware
Security Module (HSM).

## Team roles

| Role     | Responsibility                                                          | Members                |
| -------- | ----------------------------------------------------------------------- | ---------------------- |
| Author   | Writes code, prepares releases                                          | Repository maintainers |
| Reviewer | Reviews and approves pull requests before merge                         | Repository maintainers |
| Approver | Approves each signing request in SignPath before any artifact is signed | Repository owner       |

When the opt-in signing path is enabled, each signing request requires
**manual approval** by the Approver before SignPath produces the
separate `vass-<version>-x86_64-pc-windows-msvc-signed.zip` artifact.
Without the opt-in configuration and approval, the release contains the
unsigned Windows archive only.

## Build provenance

Every release archive and binary is built by GitHub Actions directly
from the tagged commit in this repository. The Sigstore build provenance
attestations cover the release archives, binaries, `SHA256SUMS.txt`, and
`SHA256SUMS.txt.sha256`. Anyone can verify that a downloaded artifact
corresponds to this source code via the attestation attached to that release
(`gh attestation verify <file> -o <owner>`), in addition to the
published SHA-256 checksums.

## Security practices

- All team members use **multi-factor authentication** on both the
  source repository (GitHub) and SignPath.
- Only artifacts produced by this repository's own CI pipeline from
  its own source code are submitted for signing.
- Signing requests are limited to the `release-signing` policy, which
  only accepts Windows PE executables produced by the release
  workflow.

## Privacy policy

The signing process does not collect or transmit any user data. The
only information processed is the artifact binary itself and its
build metadata (product name, version, CI run identifier), which is
embedded in the signature and visible in the release.

## Reporting problems

If you believe a signed Vass binary is not genuine, please open an
issue at the project repository immediately.
