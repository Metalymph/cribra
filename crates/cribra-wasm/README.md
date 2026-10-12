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