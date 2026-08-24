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

## Usage

```toml
[dependencies]
cribra-wasm = "0.4"
```

The crate targets wasm32-unknown-unknown and is intended to be processed with
wasm-bindgen.

See the repository
[WASM interoperability documentation](https://github.com/Metalymph/cribra/blob/main/docs/WASM_INTEROP.md)
for the complete contract, semantic parity policy, browser validation, and
production build guidance.

## License

MIT