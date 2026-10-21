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
- Core-owned incremental UTF-8 documents, revisioned edits and owned snapshots;
- redaction and templating;
- keyed pseudonymization and deterministic synthesis.

The adapter does not route through Cribra's native C ABI and does not use JSON
serialization as its transport boundary.

Whole-source inputs and transformation keys remain caller-owned. Incremental
sessions give Core exclusive ownership of an editable document. Browser lifecycle,
Workers, service workers, storage, caching, CSP, and UI policy remain the
responsibility of the integrating application.

## Configurations

The WASM adapter supports two configurations:

| Configuration | Features | API |
| --- | --- | --- |
| Core | `--no-default-features` | Scanner, built-ins, custom rules, findings, candidates, explanations, reporting, admitted byte streams and incremental documents |
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
Streaming and incremental sessions are available in both configurations.

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

## Incremental documents

`ScanEngine.incremental(source: string)` opens an unlimited Core document;
`incrementalWithLimits(source: string, limits: IncrementalLimits)` adds four
independent optional budgets. Incremental scanning supports the existing built-in
and custom scanner configurations; streaming admission restrictions do not apply.
Core exclusively owns source, revision, matching and transaction state. The
adapter has no source cache or independent edit engine.

```js
const engine = new ScanEngine();
let session, snapshot, result;
try {
  session = engine.incremental("");
  const revision = session.applyEdit(0n, 0, 0, "password=CorrectHorseBatteryStaple");
  snapshot = session.snapshot();
  result = snapshot.result;
  console.log(revision === snapshot.revision, result.sourceBytes);
} catch (error) {
  if (!(error instanceof IncrementalError)) throw error;
  try {
    console.error(error.code, error.terminal, error.message);
  } finally {
    error.free();
  }
} finally {
  result?.free();
  snapshot?.free();
  session?.free();
  engine.free();
}
```

`IncrementalSession.applyEdit(expectedRevision: bigint, start: number,
end: number, replacement: string): bigint` replaces the half-open UTF-8 byte
range `[start, end)`. The initial revision is `0n`; every successful edit,
including an unchanged replacement or empty no-op, increments it once.
Revisions remain exact u64/bigint values. The adapter rejects non-bigint,
negative or greater-than-u64 revision inputs with `InvalidRevision`, without
coercion or wrapping. Offsets use the same lossless JS number validation as
stream limits: invalid numeric/non-number inputs throw `OffsetOutOfRange`
before invoking Core. Valid WASM32 offsets range from zero to `4294967295`;
Core then validates document range, revision and UTF-8 scalar boundaries with
its existing error precedence.

`IncrementalLimits` is a type-only TypeScript interface:

| Property | Core accounting |
| --- | --- |
| `maxCandidates` | Newly emitted raw rule candidates per construction/edit; rejected and duplicate emissions count |
| `maxRetainedCacheRecords` | Simultaneously owned old and staged cache records, including rejected and hidden records |
| `maxMaterializedResults` | Constructed findings and structural result copies per operation; suppression refunds no tickets |
| `maxTransactionStorageBytes` | Conservative peak logical transaction payload including the old committed snapshot |

Missing/undefined means unlimited; zero (including `-0`) is meaningful. Values
use the same strict number and WASM32 width checks as streaming; invalid inputs
throw `InvalidLimit` before Core construction. Transaction storage is not a total
heap limit: capacity, allocator overhead, shared configuration, opaque regex and
validator workspace, and caller-owned report clones are outside Core accounting.
See [Core incremental scanning](../../docs/incremental.md) for the exact budget
and matching contract.

`StaleRevision`, `InvalidRange`, `InvalidUtf8Boundary`, `RevisionOverflow` and
`DocumentLengthOverflow` preserve an active Core session. Resource/invariant
failures make edits terminal while preserving the last committed snapshot.
The adapter keeps the Core session after these failures. `isActive()` delegates
to Core; subsequent valid edits report `NotActive`, and `snapshot()` remains
available. Invalid JS arguments are rejected before Core even on inactive
sessions, without changing their state.

`IncrementalError` is an owned wasm-bindgen object with readonly `code`,
`terminal` and source-free `message`. Each Core variant has a distinct stable
`IncrementalErrorCode`, plus `UnknownCoreError`, `InvalidLimit`,
`OffsetOutOfRange` and `InvalidRevision`. For edit errors, `terminal` reads
`!Core.is_active()` after the failure, including adapter input errors.
Construction errors create no session and set `terminal` to false. These thrown
objects are not native JavaScript `Error` instances; free each caught object.

`snapshot()` atomically captures one committed revision, source byte count and
complete report into an owned `IncrementalSnapshot`. Its readonly `revision`
is a bigint. **Each access to `snapshot.result` clones a fresh owned `ScanResult`**;
save it and free it independently of the snapshot. The existing finding,
candidate, explanation and Flow transformation APIs apply to that result.
Transformations still require callers to supply the matching source explicitly.
No document/source getter or matched sensitive value is exposed.

Sessions outlive their creating engine. Snapshots and their results outlive
later edits, terminal failures and session destruction. Freeing a snapshot does
not free results already obtained from it. Free sessions, snapshots and every
returned result separately; do not use any object after its generated `free()`.
Calls remain synchronous with no additional runtime dependencies or parallelism.

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
signatures (including the input-only limit interfaces), with runtime
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

A second direct-Core oracle exercises deterministic incremental edit sequences,
UTF-8 boundaries, exact bigint inputs/outputs, recoverable errors, all four
budgets including zero, construction and terminal failures, committed snapshot
preservation and independent ownership. Both variants compare full findings,
candidates, metadata and explanations. Flow compares all six transformation
methods, including equivalent overlap rejections. Native tests check every Core
error mapping; impractical revision/document/resource overflows and invariant
failures are mapping checks, not induced WASM runtime failures.

To exercise the same generated artifacts under an available Bun installation:

```sh
bun crates/cribra-wasm/tests/runtime/smoke.mjs core target/wasm-qualification/core
bun crates/cribra-wasm/tests/runtime/smoke.mjs flow target/wasm-qualification/flow
```

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
