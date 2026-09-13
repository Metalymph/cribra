# Changelog

All notable changes to Cribra are documented in this file.

The project follows semantic versioning from the first public release.

## [Unreleased]

## [0.4.4] - 2026-09-13

Cribra 0.4.4 establishes the canonical Cribra command-line interface without
changing core detection semantics.

### Added

- Added the dedicated `cribra-cli` crate containing both the reusable CLI
  library and the thin standalone `cribra` executable.
- Added explicit UTF-8 file scanning.
- Added stdin scanning through `cribra scan -`.
- Added stable human-readable output.
- Added deterministic metadata-only JSON output through `--format json`.
- Added a reusable public command model with `Command`, `ScanCommand`,
  `ScanInput`, and `OutputFormat`.
- Added reusable command parsing and execution APIs so downstream Rust
  consumers can use canonical Cribra CLI semantics without spawning a
  subprocess.
- Added integration tests against the real executable.
- Added file/stdin semantic-equivalence coverage.
- Added clean, review-only, and finding-bearing CLI regression coverage.
- Added exit-code regression coverage.
- Added invalid UTF-8 and missing-input privacy-safe failure coverage.

### Changed

- Established Cribra as the authority for its CLI semantics rather than leaving
  command behavior to downstream applications.
- Kept the standalone executable as a thin adapter over the reusable
  `cribra-cli` library.
- Kept detection, validation, findings, candidates, remediation, and
  transformations authoritative in the `cribra` core.
- Kept CLI dependencies intentionally minimal: the CLI does not introduce an
  argument-parsing framework, serialization framework, or error framework.
- Kept directory traversal, repository discovery, globbing, and recursive
  filesystem walking outside the v0.4.4 CLI contract.
- Documented the release-line policy: additive work remains on `0.4.x`; a
  future `0.5` is reserved for a deliberate architectural or public-contract
  change.

### CLI contract

- `cribra scan <FILE>` scans one explicit UTF-8 file.
- `cribra scan -` scans UTF-8 from stdin.
- `--format human` is the default presentation.
- `--format json` emits stable machine-readable metadata.
- Exit code `0` means the command executed successfully, including clean,
  review-only, and finding-bearing scans.
- Exit code `1` represents input or execution failure.
- Exit code `2` represents command-line usage or parse failure.

### Security

- CLI output contains classification metadata and source coordinates but never
  intentionally renders matched secret values.
- Review candidates remain metadata-only and are never promoted into findings
  by the CLI.
- Invalid UTF-8 is rejected deterministically.
- CLI diagnostics do not include source contents or matched values.
- The CLI does not add network access, persistence, uploads, or remote
  validation.

### Validation

- Formatting, check, test, Clippy, and integration gates cover `cribra-cli`.
- The CLI is validated against real file and stdin execution.
- Human and JSON output contracts are regression-tested.
- Core Rust, native C ABI, and WebAssembly release gates remain authoritative
  for their existing interoperability surfaces.

## [0.4.3] - 2026-09-12

Cribra 0.4.3 substantially completed the high-confidence secret-detection
baseline while preserving conservative false-positive requirements and
semantic parity across Rust, C, and WebAssembly.

Highlights:

- GitLab credential families;
- database connection passwords;
- quoted password/passphrase support;
- HTTP Basic authentication;
- PGP private keys;
- WireGuard credentials;
- Docker registry authentication;
- npm registry credentials;
- `.netrc` credentials;
- `/etc/shadow` and `.htpasswd` password verifiers;
- password-verifier remediation semantics;
- expanded adversarial corpus;
- Rust/C/WebAssembly parity validation.

## [0.4.2] - 2026-09-09

Security-hardening release.

Highlights:

- atomic scan-and-build construction for share-safe bundles;
- exact source/report pairing guidance;
- transformation-to-rescan safety coverage;
- PKCS#8, encrypted PKCS#8, RSA, EC, and OpenSSH private keys;
- HTTP Bearer authentication;
- JSON-escaped GCP service-account private keys;
- Rust/C/WASM release validation.

## [0.4.1] - 2026-08-24

Published `cribra-wasm` as an independently consumable WebAssembly
interoperability adapter with packaging, semantic parity, and browser
validation.

## [0.4.0] - 2026-08-24

Introduced the typed WebAssembly interoperability layer while preserving the
Rust core as semantic authority and keeping WASM independent from the native
C ABI.

## [0.3.x]

Introduced native interoperability through the dedicated `cribra-capi` adapter,
including stable ownership/lifetime rules, C header generation, real C consumer
validation, cross-platform gates, and Rust/C/WASM semantic parity.

## [0.2.x]

Established the Cribra name and the current detection, ambiguity,
explainability, query, remediation, and share-safe transformation model.

## [0.1.x]

Initial public release line, originally published under the `silens-scan`
crate name before the reusable engine was renamed Cribra.
