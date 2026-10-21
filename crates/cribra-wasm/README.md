# cribra-wasm

See [Getting started](../../docs/GETTING_STARTED.md) and
[Compatibility](../../docs/COMPATIBILITY.md) for integration and distribution status.

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
- Core-owned byte streaming for admitted rulesets, with optional resource budgets;
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
| Core | `--no-default-features` | Scanner, built-ins, custom rules, findings, candidates, explanations, reporting and admitted byte streams |
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
Streaming is available in both configurations. Incremental sessions are not
exposed by this WASM adapter.

## Byte streaming

`ScanEngine.stream()` opens an unlimited stream;
`ScanEngine.streamWithLimits(limits: StreamLimits)` accepts three independently
optional budgets. Both delegate admission and execution to Core. The current
built-in scanner is not streamable; unsupported configurations throw
`StreamErrorCode.UnsupportedRuleSet`. There is no whole-source fallback.
Use a configuration admitted by [Core streaming](../../docs/streaming.md):

```js
const builder = new ScanEngineBuilder(false);
builder.addLiteral("custom/secret", "SECRET", FindingSeverity.High);
const engine = builder.build(); // consumes builder
let stream;
try {
  stream = engine.streamWithLimits({
    maxCandidates: 1000,
    maxStreamStorageBytes: 1024 * 1024,
    maxRetainedResults: 1000,
  });
  stream.push(new TextEncoder().encode("SECRET"));
  const result = stream.finish();
  try {
    console.log(result.findingCount(), result.sourceBytes);
  } finally {
    result.free();
  }
} catch (error) {
  if (!(error instanceof StreamError)) throw error;
  try {
    console.error(error.code, error.message);
  } finally {
    error.free();
  }
} finally {
  stream?.free();
  engine.free();
}
```

`StreamLimits` is a TypeScript interface, with no JavaScript constructor.
Missing or `undefined` properties mean unlimited; explicit zero (including `-0`)
is a real limit. Supplied values must be numbers that are finite, nonnegative,
integral, safe JavaScript integers and representable on the WASM target
(`0` through `4294967295` on WASM32). Invalid numbers and non-number property
values, including `null`, throw `InvalidLimit` before Core admission.

`maxCandidates` bounds cumulative raw rule candidates; `maxRetainedResults`
bounds findings plus review candidates before final overlap suppression.
`maxStreamStorageBytes` bounds Core's logical executor storage at admission,
not process memory. These retain Core's exact accounting and error precedence.

`StreamSession.push(Uint8Array)` borrows a chunk for the duration of the call;
callers may immediately reuse its buffer. Scalars may span chunks, and empty
chunks are accepted on active streams. Core owns UTF-8 decoding, matching and
ordering. The adapter keeps no source buffer. A stream may outlive its engine.
Calls are synchronous, with no added runtime dependencies or parallel execution.

Any Core push failure invalidates the session. `finish()` signals genuine EOF
and consumes Core state on success or failure; malformed or incomplete UTF-8
and resource failures return no partial result. Later `push` or `finish` calls
throw `NotActive`. The outer object remains valid until its generated `free()`;
freeing an active stream cancels it. A successful finish returns the existing,
independently owned `ScanResult`, preserving metadata, candidates, explanations
and Flow transforms. Results never retain source text.

Streaming failures are exported `StreamError` objects, with readonly `code`
and source-free `message` getters. Each Core streaming failure has a distinct
stable `StreamErrorCode`; future unknown failures map to `UnknownCoreError`.
These objects are not native JavaScript `Error` instances: catch them, inspect
their properties, and call their generated `free()` exactly once. Sessions,
results and engines also require their own `free()` calls. As with existing
wasm-bindgen objects, do not use an object after freeing it.

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
signatures (including the input-only `StreamLimits` interface), with runtime
checks for JS methods, arity, getters and generated `free()` methods.

The runner also generates a direct-Core oracle from the shared synthetic
streaming fixture. Both generated variants compare complete reports across
every two-chunk byte partition and several chunk sizes, including split UTF-8
scalars, empty sources and chunks. Runtime checks cover invalid JS limit
values, all three budgets including zero, unsupported admission, malformed and
incomplete UTF-8, terminal operations, result ownership, metadata, explanation
authority and Flow transform compatibility. Arithmetic/invariant errors that
cannot be induced practically are covered by native error-mapping tests;
adapter byte-counter overflow uses a test-only injected counter.

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
