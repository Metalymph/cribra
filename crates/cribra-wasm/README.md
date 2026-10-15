# cribra-wasm

Typed WebAssembly and JavaScript interoperability adapter for
[Cribra](https://github.com/Metalymph/cribra).

`cribra-wasm` exposes the authoritative Cribra detection and transformation
semantics through a typed `wasm-bindgen` boundary suitable for browser and
JavaScript consumers.

## Scope

The adapter provides:

- current Cribra built-in rules;
- custom literal, prefix, suffix, and pattern rules;
- typed findings and review candidates;
- explanations and remediation metadata;
- redaction and templating;
- keyed pseudonymization and deterministic synthesis.

The adapter does not route through Cribra's native C ABI and does not use JSON
serialization as its transport boundary.

Source text and transformation keys remain caller-owned. Browser lifecycle,
Workers, service workers, storage, caching, CSP, and UI policy remain the
responsibility of the integrating application.

## Configurations

The WASM adapter supports two configurations:

| Configuration | Features | API |
| --- | --- | --- |
| Core | `--no-default-features` | Scanner, built-ins, custom rules, findings, candidates, explanations and reporting |
| Flow | `transform` (enabled by default) | Core plus all existing transformation APIs |

Flow is the default and the official published configuration. Its JavaScript
API and transformation semantics are unchanged. Core omits the `ScanResult`
methods `redact`, `redactWith`, `template`, `templateWith`, `pseudonymize` and
`synthesize`; scanning and reporting remain available.

Build Core explicitly:

```sh
cargo build -p cribra-wasm --release --target wasm32-unknown-unknown --no-default-features
```

Build Flow explicitly:

```sh
cargo build -p cribra-wasm --release --target wasm32-unknown-unknown --no-default-features --features transform
```

Omitting both feature flags also builds Flow. Process the resulting WASM
with `wasm-bindgen` for JavaScript consumption.

Rust core Fuse adds `parallel` and `serde` to Flow. These features are not
part of the supported browser WASM API; the adapter has no Fuse configuration.
Streaming and incremental sessions remain Rust-core APIs and are not exposed by
this WASM adapter.

## Runtime and release qualification

Run the generated JavaScript/TypeScript API contract and runtime smoke checks
for both variants from the repository root (Node.js 22+ and the Rust
`wasm32-unknown-unknown` target are required):

```sh
node crates/cribra-wasm/tests/runtime/qualify.mjs --install-bindgen
```

The runner installs `wasm-bindgen-cli` at the exact `wasm-bindgen` version in
`Cargo.lock` when needed, then builds each variant with explicit features and
`--locked --release`. Omit `--install-bindgen` to require a compatible CLI
already on PATH. Pass `core` or `flow` to qualify only that variant.

Bindings are generated with `--target web` under
`target/wasm-qualification/{core,flow}`. Node initializes the module from
WASM bytes, without DOM, fetch or browser lifecycle dependencies. The checks
cover built-in and custom scanning, findings, candidates, explanations, UTF-8
report coordinates and error handling. Flow executes all six transformation
methods; Core must omit them from both JS and TypeScript. A checked-in API
contract verifies exported classes, enums, member names and TypeScript
signatures, with runtime checks for JS methods, arity and getters.

The WASM workflow qualifies both variants. The release workflow qualifies
Flow explicitly and rejects a manifest whose defaults omit `transform`,
before the existing packaging and publishing steps. This protects the official
distribution from an accidentally reduced API. These smoke checks complement
the existing package/parity checks; they do not qualify browser integration,
Workers, CSP or optimized production artifact performance.

## 0.4.6 compatibility release

Version 0.4.6 depends on published `cribra ^0.5` (core 0.5.0). It preserves
the existing JavaScript/WASM API and whole-source behavior. No streaming
bindings, new features or third-party dependency upgrades are introduced.
Rust 1.97 remains the MSRV.

Publication is owned by the existing `Release WASM` workflow, triggered by
pushing `cribra-wasm-v0.4.6` at the qualified main commit. Core tag `v0.5.0`
is independent and must remain unchanged.

## Usage

```toml
[dependencies]
cribra-wasm = "0.4.6"
```

The crate targets wasm32-unknown-unknown and is intended to be processed with
wasm-bindgen.

See the repository
[WASM interoperability documentation](https://github.com/Metalymph/cribra/blob/main/docs/WASM_INTEROP.md)
for the complete contract, semantic parity policy, browser validation, and
production build guidance.

## License

MIT
