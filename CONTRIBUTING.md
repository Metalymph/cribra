# Contributing

Cribra is a privacy-first reusable detection core. Changes should preserve its
application-agnostic boundary, deterministic public contracts, and local-only
execution model.

Before opening a substantial pull request, open an issue describing:

- the proposed behavior;
- public API and SemVer impact;
- privacy and portability implications;
- false-positive and false-negative tradeoffs;
- native and WASM implications where relevant;
- test coverage and benchmark impact where relevant.

## Development requirements

Cribra declares Rust 1.97 as its MSRV. Stable Rust is used for normal
development and CI. The supported browser/WASM contract is
`wasm32-unknown-unknown` with default features or the optional `serde` feature;
`parallel` is a native-oriented opt-in and is not part of the browser/WASM
support contract.

[`just`](https://github.com/casey/just) is the preferred local task runner.
A `Makefile` mirrors the common commands for contributors and environments that
prefer Make.

The local security recipe requires `cargo-audit`.

## Common commands

| Command | Purpose |
| --- | --- |
| `just gate` | Full normal local quality gate |
| `just release-gate` | Release-oriented gate including MSRV, RustSec, package and publish dry run |
| `just test` | Default-feature tests |
| `just test-serde` | Serde contract tests |
| `just test-all` | Full native feature test surface |
| `just clippy` | Clippy for all targets/features with warnings denied |
| `just doc-all` | Full-feature rustdoc build |
| `just wasm` | Default browser/WASM compatibility check |
| `just wasm-serde` | Browser/WASM + Serde compatibility check |
| `just msrv` | Rust 1.97 check/test gate |
| `just audit` | RustSec dependency audit |
| `just package` | Build and verify the crates.io package |
| `just publish-dry-run` | crates.io publication dry run |
| `just bench` | Historical benchmark suite |
| `just bench-v02` | v0.2 semantic-path benchmark suite |
| `just bench-diagnostics` | Contextual isolation diagnostics |
| `just clean` | Remove Cargo build artifacts |

The equivalent Make targets use the same names, for example `make gate`,
`make wasm`, and `make release-gate`.

During release preparation, when version/changelog changes are intentionally
uncommitted, `just package-dirty` and `just publish-dry-run-dirty` mirror Cargo's
`--allow-dirty` release checks.

## Local validation

For ordinary changes, run:

```text
just gate
```

If `just` is unavailable:

```text
make gate
```

The normal gate checks formatting, all-feature compilation, default/Serde/all
feature tests, doctests, Clippy, rustdoc, supported WASM configurations, and
`git diff --check`.

Before a release, run:

```text
just release-gate
```

This additionally checks the declared MSRV, runs RustSec, verifies the packaged
crate, and performs a crates.io publication dry run.

## Pull requests

Keep pull requests focused. Changes to detector semantics should include
positive, negative, adversarial, and regression coverage appropriate to the
change.

Do not regenerate or relax golden/corpus expectations merely to make a test
pass. Treat a corpus mismatch as a semantic change that must first be reviewed.

Performance changes should be measurement-driven. Detection quality, privacy,
semantic correctness, and maintainability take precedence over synthetic
benchmark improvements.

Never add network access, filesystem traversal, persistence, UI concerns, or
application-specific business logic to the Cribra core without first revisiting
the project's architectural boundary.
