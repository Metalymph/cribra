# Contributing

Cribra is a privacy-first reusable detection core. Changes should preserve its
application-agnostic boundary, deterministic public contracts, and local-only
execution model.

Before opening a substantial pull request, open an issue describing:

- the proposed behavior;
- public API and SemVer impact;
- privacy and portability implications;
- false-positive and false-negative tradeoffs;
- native, C ABI, and WASM implications where relevant;
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

Additional tooling is needed only for the surfaces being changed:

- native Rust development requires the Rust toolchain;
- native C ABI development uses `cbindgen` plus a platform C compiler;
- WASM adapter development uses `wasm-bindgen-cli` and the
  `wasm32-unknown-unknown` target;
- WASM optimization comparison uses Binaryen `wasm-opt`;
- WASM structural validation uses WABT `wasm-validate`;
- transfer-size comparison additionally uses Brotli;
- the real-browser WASM benchmark harness is served with dependency-free
  Node.js;
- the local security recipe requires `cargo-audit`.

On Apple Silicon macOS, Homebrew may leave WABT unlinked because Binaryen and
WABT provide conflicting utility names. The WASM validation recipes therefore
first use `wasm-validate` from `PATH`, then fall back to
`/opt/homebrew/opt/wabt/bin/wasm-validate`.

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
| `just capi` | Check, lint, and build the native C adapter |
| `just capi-header` | Generate `include/cribra.h` with `cbindgen` |
| `just capi-header-check` | Verify that the generated C header is reproducible |
| `just capi-smoke` | Build and execute the local C ABI smoke test |
| `just capi-symbols` | Verify the public native `cribra_*` export surface |
| `just capi-static` | Verify the native static-library artifact |
| `just capi-smoke-all` | Run the complete local native C ABI validation surface |
| `just wasm` | Default browser/WASM core compatibility check |
| `just wasm-serde` | Browser/WASM + Serde core compatibility check |
| `just wasm-adapter` | Build and validate the reusable browser JS/WASM adapter |
| `just wasm-parity-oracle` | Generate the Rust-native semantic oracle |
| `just wasm-parity-prepare` | Prepare the production WASM artifact and semantic oracle |
| `just wasm-parity` | Compare the production Binaryen `-Oz` adapter against the Rust semantic oracle |
| `just wasm-opt-prepare` | Build, validate, and size the base/`-Os`/`-Oz`/`-O3` Binaryen matrix |
| `just wasm-production` | Build the single production Binaryen `-Oz` WASM artifact set |
| `just wasm-bench-prepare` | Prepare isolated browser benchmark variants |
| `just wasm-bench-serve` | Serve the real-browser WASM benchmark harness with Node.js |
| `just wasm-opt-clean` | Remove generated Binaryen/benchmark/production outputs |
| `just wasm-clean` | Remove all generated reusable WASM adapter outputs |
| `just msrv` | Rust 1.97 check/test gate |
| `just audit` | RustSec dependency audit |
| `just package` | Build and verify the crates.io package |
| `just publish-dry-run` | crates.io publication dry run |
| `just bench` | Historical benchmark suite |
| `just bench-v02` | v0.2 semantic-path benchmark suite |
| `just bench-diagnostics` | Contextual isolation diagnostics |
| `just clean` | Remove Cargo build artifacts |

The equivalent Make targets use the same names, for example `make gate`,
`make capi-smoke-all`, `make wasm-production`, and `make release-gate`.

During release preparation, when version/changelog changes are intentionally
uncommitted, `just package-dirty` and `just publish-dry-run-dirty` mirror Cargo's
`--allow-dirty` release checks.

## Development workflow

Use the narrowest validation surface while iterating, then finish with the
appropriate complete gate. Do not substitute an adapter-specific check for the
normal workspace gate when a change can affect shared core semantics.

### Rust core changes

For ordinary Rust-native changes, iterate with:

```text
cargo fmt --all
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

Before considering the change complete, run:

```text
just gate
```

Use maintained benchmarks when the change can plausibly affect runtime cost:

```text
just bench-all
```

Detector-semantic changes should also exercise the relevant corpus,
adversarial, golden, explainability, and transformation tests.

### Native C ABI changes

The C ABI is a dedicated adapter over the Rust core. Do not move native pointer,
ownership, error, or ABI concerns into the core merely to simplify FFI.

During C ABI development, run:

```text
cargo fmt --all
cargo check -p cribra-capi
cargo clippy -p cribra-capi --all-targets -- -D warnings
cargo test -p cribra-capi

just capi-header
just capi-header-check
just capi-smoke
just capi-symbols
just capi-static
just capi-smoke-all

git diff --check
```

`include/cribra.h` is generated from the intentionally FFI-shaped adapter
surface and must remain reproducible.

Native ABI ownership and error contracts must fail closed. Public C symbols and
public C type names use the `cribra_` namespace. Pointer hardening, export
validation, static/dynamic artifacts, and platform smoke coverage are part of
the ABI gate rather than optional follow-up work.

After adapter-specific validation, run the normal workspace gate:

```text
just gate
```

### WASM interoperability changes

The browser adapter lives in `cribra-wasm` and is independent from
`cribra-capi`. Do not route browser/WASM calls through the C ABI.

For typed adapter changes, iterate with:

```text
cargo fmt --all
cargo check -p cribra-wasm
cargo clippy -p cribra-wasm --all-targets -- -D warnings
cargo test -p cribra-wasm

just wasm-adapter

cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

git diff --check
```

The supported public boundary prefers typed `wasm-bindgen` projections over
JSON serialization when the typed surface is sufficient. Source text and
transformation keys remain caller-owned and are not retained by the reusable
adapter.

Before performance work, semantic changes to the adapter or core must pass the
production-artifact parity gate:

```text
just wasm-parity
```

This runs the real Binaryen `-Oz` artifact against a Rust-native semantic oracle
covering canonical fixtures plus dedicated Unicode, candidate, explanation,
remediation, and transform cases. Representation differences are allowed;
semantic differences are release blockers.

The single production WASM profile selected by the v0.4 cross-engine benchmark
gate is Binaryen `-Oz`:

```text
just wasm-production
```

The production pipeline is:

```text
Rust wasm32-unknown-unknown release
        ->
wasm-bindgen --target web
        ->
Binaryen wasm-opt -Oz
        ->
single production JS/TypeScript/WASM artifact set
```

The base/`-Os`/`-Oz`/`-O3` variants are retained only as a reproducible
regression matrix:

```text
just wasm-opt-prepare
just wasm-bench-prepare
just wasm-bench-serve
```

Run the browser benchmark in current stable representatives of the three
supported engine families when code shape, Rust/LLVM, `wasm-bindgen`, Binaryen,
or browser-engine behavior changes materially:

- Chrome for V8/Chromium;
- Safari for WebKit/JavaScriptCore;
- Firefox for Gecko/SpiderMonkey.

The benchmark is comparative rather than an absolute browser score. Use the
same generated baseline for every Binaryen variant and prefer medians for
startup decisions. Micro-workloads are measured in repeated batches and
normalized per operation; larger scans, typed traversal, transforms, and the
zero-rule source-transfer floor remain the stronger architectural evidence.

Generated optimization and benchmark artifacts can be removed with:

```text
just wasm-opt-clean
```

To remove all reusable WASM artifacts:

```text
just wasm-clean
```

### Final cross-surface validation

When a change touches the core plus either adapter, or changes a public contract,
finish with:

```text
just gate
```

For release preparation use:

```text
just release-gate
```

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
